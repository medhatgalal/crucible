#!/bin/sh
# Drive outer loop: cycle first, one legal coordinator action, never implement.
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
C="$HERE/crucible"
PASS=0; FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }
expect() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) || { bad "$label: command refused: $out"; return; }
  if printf '%s\n' "$out" | grep -E -q "$pattern"; then ok; else bad "$label: wanted $pattern, got $out"; fi
}
refuses() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) && { bad "$label: command accepted"; return; }
  if printf '%s\n' "$out" | grep -E -q "$pattern"; then ok; else bad "$label: wanted $pattern, got $out"; fi
}

write_agents() {
  prog=$1
  cmd=${2:-'echo {BRIEF}'}
  {
    printf 'c0\tkindA\tm\thigh\t%s\n' "$cmd"
    printf 'a1\tkindA\tm\thigh\techo {BRIEF}\n'
    printf 'a2\tkindB\tm\thigh\techo {BRIEF}\n'
    printf 'mk1\tkindA\tm\thigh\ttouch src/maker-invoked\n'
    printf 'j1\tkindB\tm\thigh\techo {BRIEF}\n'
    printf 'j2\tkindB\tm\thigh\techo {BRIEF}\n'
  } > "$prog/agents.tsv"
}

write_panel() {
  prog=$1
  cat > "$prog/PANEL.md" <<'EOF'
# Panel
## Agents
c0, a1, a2, mk1, j1, j2
## Roles
See PANEL.ASSIGN.tsv
## Risk posture
LOW
## Isolation transport
multi-agent preferred; acp before subagent
## Independence ladder
1 multi-agent 2 acp 3 subagent 4 stop
## Waivers
NONE
EOF
  cat > "$prog/PANEL.ASSIGN.tsv" <<'EOF'
role	agent	required	notes
coordinator	c0	yes
claim-auditor	a1	yes
claim-auditor	a2	yes
scout	a1	no
maker	mk1	yes
reviewer	j1	yes
contract-auditor	j2	yes
EOF
}

setup_repo() {
  base=$(mktemp -d "${TMPDIR:-/tmp}/crucible-drive.XXXXXX")
  repo="$base/repo"; mkdir -p "$repo/src"
  (
    cd "$repo"
    git init -q -b main
    printf 'product\n' > src/keep.txt
    git add src/keep.txt
    git -c user.name=test -c user.email=test@example.invalid commit -qm baseline
    "$C" adopt work --managed >/dev/null
  )
  printf '%s' "$repo"
}

# --- Docs SSOT ---
if awk 'NR<=8 && /drive/ && /STATUS.md/' "$HERE/START.md" | grep -q drive; then ok
else bad 'START.md missing drive-first paragraph'; fi
[ -f "$HERE/docs/drive.md" ] && ok || bad 'docs/drive.md missing'
grep -q 'keep looping' "$HERE/docs/drive.md" && ok || bad 'drive doc missing keep-looping lesson'
grep -q 'keep looping' "$HERE/START.md" && ok || bad 'START.md missing keep-looping lesson'

# --- INVESTIGATE tick dispatches, does not edit src/ ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
"$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.' >/dev/null
out=$("$P/crucible" drive tick 2>&1) || { bad "investigate tick refused: $out"; out=; }
printf '%s\n' "$out" | grep -E -q 'NEXT INVESTIGATE|dispatch|claim-auditor' && ok \
  || bad "investigate tick should cycle+dispatch: $out"
[ -f "$P/STATUS.md" ] && grep -q 'state:' "$P/STATUS.md" && ok || bad 'STATUS.md not rewritten on tick'
disp=$(find "$P/claims" -path '*dispatches*claim-auditor*' -type f 2>/dev/null | wc -l | tr -d ' ')
[ "$disp" -ge 1 ] && ok || bad "investigate tick did not dispatch (count=$disp)"
[ -f "$repo/src/keep.txt" ] && [ ! -f "$repo/src/maker-invoked" ] && \
  [ "$(cat "$repo/src/keep.txt")" = product ] && \
  [ "$(find "$repo/src" -type f | wc -l | tr -d ' ')" = 1 ] && ok \
  || bad 'investigate tick edited src/ or invoked maker'

# --- WAIT APPROVAL exits 0 without invoking maker ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'No code change is actually required.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'code change required' 'No code change is actually required.')
# Two FALSE verdicts with sealed attempts → investigation COMPLETE, then proposal.
seal_claim() {
  prog=$1; agent=$2
  id=
  for ad in "$prog"/attempts/A*; do
    [ -d "$ad" ] || continue
    a=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
    [ "$a" = "$agent" ] || continue
    id=${ad##*/}
  done
  [ -n "$id" ] || return 1
  "$prog/crucible" attempt transport "$id" multi-agent >/dev/null
  "$prog/crucible" contract-audit "$id" j2 PASS >/dev/null
}
for agent in a1 a2; do
  "$P/crucible" run-claim "$cn" "$agent" -- sh -c 'echo checked' >/dev/null
  "$P/crucible" dispatch "$cn" claim-auditor "$agent" >/dev/null
  seal_claim "$P" "$agent"
  "$P/crucible" claim verdict "$cn" "$agent" FALSE >/dev/null
done
cat > "$P/PROPOSAL.md" <<'EOF'
# Proposal
## Verified problem
The allegation is false.
## Proposed outcome
No code change.
## Non-goals
No speculative work.
## Backlog
Empty.
## Verification
Two recorded falsifications.
EOF
expect 'proposal waits for human' '^WAIT APPROVAL ' "$P/crucible" cycle
set +e
out=$("$P/crucible" drive tick 2>&1)
rc=$?
set -e
[ "$rc" -eq 0 ] && ok || bad "WAIT APPROVAL tick exit $rc: $out"
printf '%s\n' "$out" | grep -E -q 'WAIT APPROVAL|HUMAN|approve' && ok \
  || bad "WAIT APPROVAL tick should name the human action: $out"
[ ! -f "$repo/src/maker-invoked" ] && ok || bad 'WAIT APPROVAL tick invoked maker'

# --- Coordinator that touches an owned product path fails the gate ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P" 'mkdir -p src && printf pwned\\n > src/pwned'
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
"$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.' >/dev/null
refuses 'coordinator product-path write is refused' 'owned|product path|verdict|merge|src/' \
  "$P/crucible" drive tick
[ ! -f "$repo/src/pwned" ] || bad 'owned-path refuse left the product write in place'
[ ! -f "$repo/src/pwned" ] && ok || true

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
