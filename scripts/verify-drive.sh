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
grep -q 'drive' "$HERE/BOOTSTRAP.md" && grep -q 'STATUS.md' "$HERE/BOOTSTRAP.md" \
  && ok || bad 'BOOTSTRAP does not teach drive / STATUS.md'
grep -q 'docs/drive.md' "$HERE/README.md" && grep -q 'STATUS.md' "$HERE/README.md" \
  && ok || bad 'README does not point at drive / STATUS.md'
grep -q 'How to use an installed cycle' "$HERE/START.md" \
  && ok || bad 'START.md missing how-to-use table'

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

# Human approve verbs refuse while drive lock is held
mkdir "$P/.drive.lock"
refuses 'approve-panel refused under drive lock' 'drive|human gate' \
  "$P/crucible" cycle approve-panel
rmdir "$P/.drive.lock"

# --- git commit of product files is refused and HEAD restored ---
repo=$(setup_repo)
P="$repo/.crucible/work"
before_head=$(git -C "$repo" rev-parse HEAD)
cat > "$P/coord.sh" <<'EOF'
#!/bin/sh
printf committed\\n >> src/keep.txt
git add src/keep.txt
git -c user.name=t -c user.email=t@e.invalid commit -qm pwn
EOF
chmod +x "$P/coord.sh"
write_agents "$P" '.crucible/work/coord.sh'
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
"$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.' >/dev/null
refuses 'committed product write is refused' 'committed|merged|product history|owned' \
  "$P/crucible" drive tick
[ "$(git -C "$repo" rev-parse HEAD)" = "$before_head" ] && ok || bad 'commit tick left a product commit'
[ "$(cat "$repo/src/keep.txt")" = product ] && ok || bad 'commit tick did not restore src/keep.txt'

# --- already-dirty file whose content changes (same porcelain line) ---
repo=$(setup_repo)
P="$repo/.crucible/work"
printf 'dirty\n' >> "$repo/src/keep.txt"
cat > "$P/coord.sh" <<'EOF'
#!/bin/sh
printf more\\n >> src/keep.txt
EOF
chmod +x "$P/coord.sh"
write_agents "$P" '.crucible/work/coord.sh'
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
"$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.' >/dev/null
refuses 'dirty-file content change is refused' 'owned|product path|src/' \
  "$P/crucible" drive tick
grep -q more "$repo/src/keep.txt" && bad 'dirty-file tick left extra content' || ok

# --- worktree write under the program dir ---
repo=$(setup_repo)
P="$repo/.crucible/work"
cat > "$P/coord.sh" <<'EOF'
#!/bin/sh
mkdir -p .crucible/work/worktrees/sneak
printf pwn\\n > .crucible/work/worktrees/sneak/x
EOF
chmod +x "$P/coord.sh"
write_agents "$P" '.crucible/work/coord.sh'
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
"$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.' >/dev/null
refuses 'worktree write is refused' 'worktree|owned|product' \
  "$P/crucible" drive tick
[ ! -f "$P/worktrees/sneak/x" ] && ok || bad 'worktree write was left in place'

# --- verdict overwrite and new claim verdict ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P" '.crucible/work/coord.sh'
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
"$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.' >/dev/null
mkdir -p "$P/items/x/verdicts" "$P/claims/C1/verdicts"
printf 'VERDICT: PASS\n' > "$P/items/x/verdicts/j1.md"
cat > "$P/coord.sh" <<'EOF'
#!/bin/sh
printf 'VERDICT: OVERWRITE\n' > .crucible/work/items/x/verdicts/j1.md
printf 'CLAIM-VERDICT: TRUE\n' > .crucible/work/claims/C1/verdicts/a1.md
EOF
chmod +x "$P/coord.sh"
refuses 'verdict write is refused' 'verdict' \
  "$P/crucible" drive tick
grep -q OVERWRITE "$P/items/x/verdicts/j1.md" && bad 'item verdict overwrite left in place' || ok
[ ! -f "$P/claims/C1/verdicts/a1.md" ] && ok || bad 'new claim verdict was left in place'

# --- PROGRAM guided flip ---
repo=$(setup_repo)
P="$repo/.crucible/work"
cat > "$P/coord.sh" <<'EOF'
#!/bin/sh
sed -i.bak '/^cycle: guided$/d' .crucible/work/PROGRAM
EOF
chmod +x "$P/coord.sh"
write_agents "$P" '.crucible/work/coord.sh'
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
"$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.' >/dev/null
refuses 'removing cycle: guided is refused' 'guided' \
  "$P/crucible" drive tick
grep -q '^cycle: guided$' "$P/PROGRAM" && ok || bad 'PROGRAM guided line was not restored'

# --- WAIT: new live attempt id refused; seal counts as progress ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Need a real gap.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'a real gap' 'Need a real gap.')
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
  "$P/crucible" claim verdict "$cn" "$agent" TRUE >/dev/null
done
"$P/crucible" run-claim "$cn" a1 -- sh -c 'echo searched' >/dev/null
"$P/crucible" dispatch "$cn" scout a1 >/dev/null
seal_claim "$P" a1
"$P/crucible" claim scout "$cn" ABSENT a1 >/dev/null
cat > "$P/PROPOSAL.md" <<'EOF'
# Proposal
## Verified problem
A real gap exists.
## Proposed outcome
One bounded item.
## Non-goals
No extras.
## Backlog
One item: wait-item.
## Verification
Falsifier is true.
EOF
"$P/crucible" cycle approve >/dev/null
"$P/crucible" claim admit "$cn" wait-item >/dev/null
cat > "$P/items/wait-item/ITEM.md" <<'EOF'
# wait-item — bounded

## Goal

Fill the gap.

## Non-goals

Nothing else.

## Risk

LOW

## Owned files

- src/keep.txt

## Acceptance criteria

- [ ] A1: keep.txt still exists

## Focused falsifier

test -f src/keep.txt

## Expensive evidence

NONE

## Stop conditions

Stop if the contract must change.
EOF
"$P/crucible" ready wait-item >/dev/null
"$P/crucible" phase wait-item BUILD >/dev/null
"$P/crucible" dispatch wait-item maker mk1 >/dev/null
expect 'item inflight is WAIT' '^WAIT wait-item ' "$P/crucible" cycle
cat > "$P/coord.sh" <<'EOF'
#!/bin/sh
P=.crucible/work
id=A9999999999.1.1
mkdir -p "$P/attempts/$id"
printf 'state\tepoch\tpid\treason\nDISPATCHED\t1\t-\tfake\n' > "$P/attempts/$id/events.tsv"
printf 'attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\n' > "$P/attempts/$id/meta.tsv"
printf '%s\twait-item\t-\tCLAIM\tmaker\tmk1\tkindA\tA1\tFOCUSED\tDISPATCHED\t1\t2\t-\n' "$id" >> "$P/attempts/$id/meta.tsv"
EOF
chmod +x "$P/coord.sh"
write_agents "$P" '.crucible/work/coord.sh'
"$P/crucible" cycle approve-panel >/dev/null
refuses 'WAIT second live attempt is refused' 'second attempt|WAIT inflight' \
  "$P/crucible" drive tick
# restore echo coordinator; seal should count as progress across two ticks
write_agents "$P"
"$P/crucible" cycle approve-panel >/dev/null
"$P/crucible" drive tick >/dev/null
# seal the real inflight maker attempt
inflight=$(awk -F '\t' 'NR==2 {print $6}' "$P/STATE.tsv")
if [ -n "$inflight" ] && [ "$inflight" != - ]; then
  "$P/crucible" attempt transport "$inflight" multi-agent >/dev/null 2>&1 || true
  "$P/crucible" contract-audit "$inflight" j2 PASS >/dev/null 2>&1 || true
fi
set +e
out=$("$P/crucible" drive tick 2>&1)
rc=$?
set -e
[ "$rc" -eq 0 ] && ok || bad "WAIT tick after seal refused: $out"
printf '%s\n' "$out" | grep -q 'STOP — no progress' \
  && bad "seal-only WAIT tick should not false-STOP" || ok

# --- one TRUE is not PLAN/admit-ready ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Need a real gap.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'a real gap' 'Need a real gap.')
"$P/crucible" run-claim "$cn" a1 -- sh -c 'echo checked' >/dev/null
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
seal_claim "$P" a1
"$P/crucible" claim verdict "$cn" a1 TRUE >/dev/null
"$P/crucible" run-claim "$cn" a1 -- sh -c 'echo searched' >/dev/null
"$P/crucible" dispatch "$cn" scout a1 >/dev/null
seal_claim "$P" a1
"$P/crucible" claim scout "$cn" ABSENT a1 >/dev/null
cat > "$P/PROPOSAL.md" <<'EOF'
# Proposal
## Verified problem
A real gap exists.
## Proposed outcome
One item.
## Non-goals
None.
## Backlog
sprint-create.
## Verification
Two TRUEs required.
EOF
expect 'one TRUE stays INVESTIGATE not PLAN' '^NEXT INVESTIGATE ' "$P/crucible" cycle
refuses 'admit still needs two TRUEs' 'TRUE verdicts' \
  "$P/crucible" claim admit "$cn" sprint-create

# --- missing coordinator command refuses instead of STOP no-progress ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P" 'python3 /no/such/acp-brief.py grok {BRIEF}'
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
"$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.' >/dev/null
refuses 'failed coordinator invoke is refused' 'invoke failed|No such file|can.t open' \
  "$P/crucible" drive tick

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
