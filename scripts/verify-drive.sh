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

TMP=$(mktemp -d "${TMPDIR:-/tmp}/crucible-drive.XXXXXX")
# Cleanup must never mask the exit status. The `0` handler removes and returns, so a normal run
# still reports its own result. Each signal handler removes and then exits 128+signal, because a
# handler that only removes lets the shell resume and reach a `0` exit — an interrupted or
# timed-out run would then be recorded as a pass.
trap 'rm -rf "$TMP"' 0
trap 'rm -rf "$TMP"; exit 129' 1
trap 'rm -rf "$TMP"; exit 130' 2
trap 'rm -rf "$TMP"; exit 143' 15

setup_repo() {
  base=$(mktemp -d "$TMP/case.XXXXXX")
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
grep -q 'adopt <program> --refresh' "$HERE/START.md" \
  && ok || bad 'START.md missing adopt --refresh'
grep -q 'cycle problem FILE --next' "$HERE/START.md" \
  && ok || bad 'START.md missing cycle problem --next'
grep -q 'adopt NAME --managed --panel-from SRC' "$HERE/START.md" \
  && ok || bad 'START.md missing adopt --panel-from'
[ -f "$HERE/docs/install.md" ] && grep -q 'adopt work --refresh' "$HERE/docs/install.md" \
  && ok || bad 'docs/install.md missing refresh contract'
grep -q 'adopt NAME --managed --panel-from SRC' "$HERE/docs/install.md" \
  && ok || bad 'docs/install.md missing --panel-from sibling cycle'

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
[ ! -f "$P/DRIVE.BRIEF.md" ] && ok \
  || bad 'investigate tick must not write DRIVE.BRIEF (coordinator ACP)'
sealed_n=0
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  [ -f "$ad/transport" ] && grep -q '^VERDICT: PASS' "$ad/contract-audit.md" 2>/dev/null && sealed_n=$((sealed_n + 1))
done
[ "$sealed_n" -ge 1 ] && ok || bad "parent investigate tick did not seal ($sealed_n)"
nstart=$(printf '%s\n' "$out" | grep -c 'drive: started' || true)
[ "$nstart" -le 1 ] && ok || bad "investigate tick started $nstart workers (want <=1): $out"

# --- Three isomorphic claims: one tick dispatches+seals all, no coordinator ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Flags are missing.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
[ -f "$P/PANEL.CONTEXT.md" ] && grep -q 'problem-title:' "$P/PANEL.CONTEXT.md" && ok \
  || bad 'problem bind did not sync PANEL.CONTEXT.md'
"$P/crucible" claim add 'label flag missing' 'Flags are missing.' ABSENT >/dev/null
"$P/crucible" claim add 'type flag missing' 'Flags are missing.' ABSENT >/dev/null
"$P/crucible" claim add 'sprint flag missing' 'Flags are missing.' ABSENT >/dev/null
st=$("$P/crucible" cycle 2>&1) || { bad "3-claim cycle refused: $st"; st=; }
printf '%s\n' "$st" | grep -q 'NO-BUILD if all FALSE/STALE' && ok \
  || bad "ABSENT-only STATUS missing NO-BUILD close: $st"
printf '%s\n' "$st" | grep -q 'admit needs' && bad "ABSENT-only STATUS still says admit needs: $st" || ok
out=$("$P/crucible" drive tick 2>&1) || { bad "3-claim investigate tick refused: $out"; out=; }
disp=$(find "$P/claims" -path '*dispatches*claim-auditor-a1*' -type f 2>/dev/null | wc -l | tr -d ' ')
[ "$disp" -ge 3 ] && ok || bad "3-claim tick should dispatch C1-C3 (count=$disp) out=$out"
sealed_n=0
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  [ -f "$ad/transport" ] && grep -q '^VERDICT: PASS' "$ad/contract-audit.md" 2>/dev/null && sealed_n=$((sealed_n + 1))
done
[ "$sealed_n" -ge 3 ] && ok || bad "3-claim tick should seal C1-C3 ($sealed_n)"
[ ! -f "$P/DRIVE.BRIEF.md" ] && ok || bad '3-claim tick wrote DRIVE.BRIEF'
nstart=$(printf '%s\n' "$out" | grep -c 'drive: started' || true)
[ "$nstart" = 1 ] && ok || bad "3-claim tick started $nstart workers (want 1): $out"

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
cn=$("$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
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
cn=$("$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
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
cn=$("$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
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
cn=$("$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
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
cn=$("$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
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
cn=$("$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
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
refuses 'maker dispatch without plan-audit PASS' 'plan-audit' \
  "$P/crucible" dispatch wait-item maker mk1
"$P/crucible" plan-audit wait-item j2 PASS >/dev/null
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
if printf '%s\n' "$out" | grep -q 'drive: started'; then
  ok
elif printf '%s\n' "$out" | grep -q 'STOP — no progress'; then
  bad "seal-only WAIT tick should not false-STOP"
else
  ok
fi

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
# Second-kind STALE supersedes the lone TRUE for investigation (not for admit).
"$P/crucible" run-claim "$cn" a2 -- sh -c 'echo now exists' >/dev/null
"$P/crucible" dispatch "$cn" claim-auditor a2 >/dev/null
seal_claim "$P" a2
"$P/crucible" claim verdict "$cn" a2 STALE >/dev/null
out=$("$P/crucible" cycle 2>&1) || { bad "TRUE+STALE cycle refused: $out"; out=; }
printf '%s\n' "$out" | grep -E -q '^NEXT INVESTIGATE ' \
  && bad "TRUE+STALE should not stay INVESTIGATE: $out" || ok
refuses 'TRUE+STALE still cannot admit' 'TRUE verdicts' \
  "$P/crucible" claim admit "$cn" sprint-create

# --- missing coordinator command refuses instead of STOP no-progress ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P" 'python3 /no/such/acp-brief.py grok {BRIEF}'
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
refuses 'failed coordinator invoke is refused' 'invoke failed|No such file|can.t open' \
  "$P/crucible" drive tick

# --- --next archives leftover DISPATCHED (never started); RUNNING still blocks ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Old problem still open.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'old gap' 'Old problem still open.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
printf 'Live-verify instead.\n' > "$repo/next-problem.md"
out=$("$P/crucible" cycle problem "$repo/next-problem.md" --next 2>&1) || {
  bad "next refused leftover DISPATCHED: $out"; out=
}
printf '%s\n' "$out" | grep -q '/PROBLEM.md$' && ok || bad "next should bind despite DISPATCHED leftover: $out"
grep -q 'Live-verify instead' "$P/PROBLEM.md" && ok || bad 'next did not replace PROBLEM after DISPATCHED leftover'

repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Old problem still open.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'old gap' 'Old problem still open.')
"$P/crucible" run-claim "$cn" a1 -- sh -c 'echo checked' >/dev/null
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
seal_claim "$P" a1
id=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  a=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  [ "$a" = a1 ] || continue
  id=${ad##*/}
done
"$P/crucible" attempt start "$id" 1 >/dev/null
printf 'Next while running.\n' > "$repo/next-problem.md"
refuses 'next refuses while an attempt is RUNNING' 'RUNNING|OVERDUE' \
  "$P/crucible" cycle problem "$repo/next-problem.md" --next

# --- adopt --refresh is additive and keeps local adapters ---
repo=$(setup_repo)
P="$repo/.crucible/work"
printf '1.2.0\n' > "$P/VERSION"
printf 'KEEP-ACP\n' > "$P/scripts/acp-brief.py"
printf 'stale start\n' > "$P/START.md"
( cd "$repo" && "$C" adopt work --refresh >/dev/null )
[ "$(sed -n '1p' "$P/VERSION")" = "$(sed -n '1p' "$HERE/VERSION")" ] && ok \
  || bad "refresh did not update VERSION"
grep -q 'KEEP-ACP' "$P/scripts/acp-brief.py" && ok || bad 'refresh deleted acp-brief.py'
grep -q 'drive' "$P/START.md" && ok || bad 'refresh did not update START.md'
grep -q 'lifecycle: managed' "$P/PROGRAM" && ok || bad 'refresh clobbered PROGRAM'
( cd "$repo" && "$C" adopt missing --refresh >/dev/null 2>&1 ) \
  && bad 'refresh of missing program was allowed' \
  || ok

# --- cycle problem --next archives and keeps the panel ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
panel_before=$(sed -n 's/^panel-id: //p' "$P/PANEL.APPROVAL" | head -1)
printf 'No code change is actually required.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'code change required' 'No code change is actually required.')
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
"$P/crucible" cycle approve >/dev/null
expect 'false-claim approval is DONE' '^DONE ' "$P/crucible" cycle
grep -q '^engine: ' "$P/STATUS.md" && ok || bad 'STATUS.md missing engine'
mkdir -p "$P/items/leftover"
printf leftover\\n > "$P/items/leftover/ITEM.md"
printf 'Live-verify sprint create on PT.\n' > "$repo/next-problem.md"
out=$("$P/crucible" cycle problem "$repo/next-problem.md" --next 2>&1) || {
  bad "cycle problem --next refused: $out"; out=
}
printf '%s\n' "$out" | grep -q '/PROBLEM.md$' && ok || bad "next did not bind PROBLEM: $out"
printf '%s\n' "$out" | grep -q '^archived ' && ok || bad "next did not archive: $out"
grep -q 'Live-verify sprint create' "$P/PROBLEM.md" && ok || bad 'next PROBLEM not replaced'
[ ! -d "$P/items/leftover" ] && ok || bad 'leftover items/ dir was not archived'
hist=$(find "$P/history" -type d -name leftover 2>/dev/null | head -1)
[ -n "$hist" ] && ok || bad 'leftover item was not under history/'
cmp -s "$P/PANEL.ASSIGN.tsv" "$P/PANEL.ASSIGN.tsv" && ok
[ -f "$P/PANEL.APPROVAL" ] && grep -q '^decision: APPROVED' "$P/PANEL.APPROVAL" && ok \
  || bad '--next dropped panel approval'
"$P/crucible" cycle 2>&1 | grep -q 'WAIT PANEL' && bad '--next forced WAIT PANEL (recast)' || ok
expect 'next problem is INVESTIGATE' '^NEXT INVESTIGATE ' "$P/crucible" cycle
"$P/crucible" claim add 'live verify missing' 'Live-verify sprint create on PT.' >/dev/null
printf 'Another problem.\n' > "$repo/too-soon.md"
refuses 'problem without --next still refuses after claims' 'already has claims' \
  "$P/crucible" cycle problem "$repo/too-soon.md"

# --next refuses while an item is ACTIVE
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Need a real gap.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'a real gap' 'Need a real gap.')
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
One item.
## Non-goals
None.
## Backlog
one-item.
## Verification
Two TRUEs.
EOF
"$P/crucible" cycle approve >/dev/null
"$P/crucible" claim admit "$cn" one-item >/dev/null
# P0: dispatch ITEM judge must keep role=judge (ASSIGN lookup may say reviewer).
awk -F '\t' 'BEGIN{OFS="\t"} NR==1{print;next} $1=="one-item"{$3="REVIEW"} {print}' \
  "$P/STATE.tsv" > "$P/STATE.tsv.tmp" && mv "$P/STATE.tsv.tmp" "$P/STATE.tsv"
git -C "$repo" branch ai/one-item main >/dev/null 2>&1 || true
jout=$("$P/crucible" dispatch one-item judge j1 2>&1) || {
  bad "guided judge dispatch refused: $jout"; jout=
}
printf '%s\n' "$jout" | grep -q 'as judge' && ok \
  || bad "dispatch ITEM judge clobbered role: $jout"
printf '%s\n' "$jout" | grep -q 'must be maker, judge, or adversary' \
  && bad "P0: judge normalized to reviewer before managed dispatch" || true
printf 'Next while active.\n' > "$repo/next-problem.md"
refuses 'next refuses while an item is ACTIVE' 'ACTIVE or BLOCKED' \
  "$P/crucible" cycle problem "$repo/next-problem.md" --next

# --- drive starts a sealed worker (does not write verdicts) ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
wid=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  a=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  r=$(awk -F '\t' 'NR==2 {print $5}' "$ad/meta.tsv")
  [ "$a" = a1 ] && [ "$r" = claim-auditor ] && wid=${ad##*/}
done
[ -n "$wid" ] || bad 'no claim-auditor attempt to start'
seal_claim "$P" a1
out=$("$P/crucible" drive tick 2>&1) || { bad "sealed-worker tick refused: $out"; out=; }
printf '%s\n' "$out" | grep -q "drive: started $wid" && ok \
  || bad "drive did not start sealed worker: $out"
[ "$(awk -F '\t' 'END{print $1}' "$P/attempts/$wid/events.tsv")" = RETURNED ] && ok \
  || bad "drive did not record RETURNED for $wid"
[ -f "$P/attempts/$wid/invoke.log" ] && ok || bad 'drive did not write invoke.log'
[ ! -f "$P/claims/$cn/verdicts/a1.md" ] && ok \
  || bad 'drive parent must not write a claim verdict'

# --- drive tick starts exactly one of two sealed workers ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'The report alleges missing enforcement.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'missing enforcement' 'The report alleges missing enforcement.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
"$P/crucible" dispatch "$cn" claim-auditor a2 >/dev/null
seal_claim "$P" a1
seal_claim "$P" a2
out=$("$P/crucible" drive tick 2>&1) || { bad "two-sealed tick refused: $out"; out=; }
nstart=$(printf '%s\n' "$out" | grep -c 'drive: started' || true)
[ "$nstart" = 1 ] && ok || bad "drive tick started $nstart workers (want 1): $out"

# --- reclaim dead RUNNING unblocks --next ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Old problem.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'old gap' 'Old problem.')
"$P/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
zid=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  zid=${ad##*/}
done
"$P/crucible" attempt transport "$zid" multi-agent >/dev/null
"$P/crucible" contract-audit "$zid" j2 PASS >/dev/null
"$P/crucible" attempt start "$zid" 999999 >/dev/null
"$P/crucible" attempt reclaim "$zid" >/dev/null
[ "$(awk -F '\t' 'END{print $1}' "$P/attempts/$zid/events.tsv")" = STOPPED ] && ok \
  || bad 'reclaim did not STOPPED a dead pid'
# leftover RUNNING then --next auto-reclaims
"$P/crucible" dispatch "$cn" scout a1 >/dev/null
sid=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  r=$(awk -F '\t' 'NR==2 {print $5}' "$ad/meta.tsv")
  [ "$r" = scout ] && sid=${ad##*/}
done
"$P/crucible" attempt transport "$sid" acp >/dev/null
"$P/crucible" contract-audit "$sid" j2 PASS >/dev/null
"$P/crucible" attempt start "$sid" 999998 >/dev/null
printf 'Next problem.\n' > "$repo/next-problem.md"
expect 'next reclaims dead RUNNING' '/PROBLEM.md$' \
  "$P/crucible" cycle problem "$repo/next-problem.md" --next

# --- husk refresh refuses ---
mkdir -p "$repo/.crucible/ghost"
printf leftover > "$repo/.crucible/ghost/STATUS.md"
( cd "$repo" && "$C" adopt ghost --refresh >/dev/null 2>&1 ) \
  && bad 'refresh of husk was allowed' \
  || ok

# --- evidence archive ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Need a real gap.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
cn=$("$P/crucible" claim add 'a real gap' 'Need a real gap.')
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
One item.
## Non-goals
None.
## Backlog
arc-item.
## Verification
Two TRUEs.
EOF
"$P/crucible" cycle approve >/dev/null
"$P/crucible" claim admit "$cn" arc-item >/dev/null
printf 'stale\n' > "$P/items/arc-item/evidence/mk1.tok.DEADBEEF0001.txt"
out=$("$P/crucible" evidence archive arc-item 2>&1) || {
  bad "evidence archive refused: $out"; out=
}
printf '%s\n' "$out" | grep -q 'archived' && ok || bad "archive did not move stale file: $out"
[ -f "$P/items/arc-item/evidence/history/mk1.tok.DEADBEEF0001.txt" ] && ok \
  || bad 'stale evidence was not moved to evidence/history/'

# --- drive stop + result under lock ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Need a real gap.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
mkdir "$P/.drive.lock"
mkdir -p "$P/attempts/A1788000000.1.1"
printf 'attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\n' \
  > "$P/attempts/A1788000000.1.1/meta.tsv"
printf 'A1788000000.1.1\tC1\t-\t-\tclaim-auditor\ta1\tkindA\t-\t-\tRUNNING\t1\t2\t-\n' \
  >> "$P/attempts/A1788000000.1.1/meta.tsv"
printf 'state\tepoch\tpid\treason\nRUNNING\t1\t999991\tdead\n' \
  > "$P/attempts/A1788000000.1.1/events.tsv"
out=$("$P/crucible" drive stop 2>&1) || { bad "drive stop refused: $out"; out=; }
printf '%s\n' "$out" | grep -q 'released' && ok || bad "drive stop did not release lock: $out"
[ ! -d "$P/.drive.lock" ] && ok || bad 'drive stop left .drive.lock'
printf '%s\n' "$out" | grep -q 'reclaimed' && ok || bad "drive stop did not reclaim dead pid: $out"
mkdir "$P/.drive.lock"
refuses 'result while drive.lock from coordinator' 'drive.lock' \
  "$P/crucible" result A1788000000.1.1 PASS ev.txt CLOSE -
rmdir "$P/.drive.lock"

# --- 1.6.1 parent INVESTIGATE dispatch (no coordinator ACP) + --like ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P" 'exit 1'
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'assess flags missing.\n' > "$repo/report.md"
"$P/crucible" cycle problem "$repo/report.md" >/dev/null
"$P/crucible" claim add 'assess --label is not a CLI flag' 'assess --label is not a CLI flag' ABSENT >/dev/null
"$P/crucible" claim add 'assess --type is not a CLI flag' 'assess --type is not a CLI flag' ABSENT >/dev/null
"$P/crucible" claim add 'assess --sprint is not a CLI flag' 'assess --sprint is not a CLI flag' ABSENT >/dev/null
out=$("$P/crucible" drive tick 2>&1) || { bad "parent investigate tick died (coordinator?): $out"; out=; }
printf '%s\n' "$out" | grep -q 'coordinator invoke failed' && bad "INVESTIGATE invoked coordinator ACP: $out" || ok
[ ! -f "$P/DRIVE.BRIEF.md" ] && ok || bad 'DRIVE.BRIEF.md means coordinator ACP ran'
[ -f "$P/claims/C1/dispatches/"*-claim-auditor-a1.md ] && ok || bad 'C1 auditor not dispatched by parent'
[ -f "$P/claims/C2/dispatches/"*-claim-auditor-a1.md ] && ok || bad 'C2 auditor not dispatched by parent'
[ -f "$P/claims/C3/dispatches/"*-claim-auditor-a1.md ] && ok || bad 'C3 auditor not dispatched by parent'
nstart=$(printf '%s\n' "$out" | grep -c 'drive: started' || true)
[ "$nstart" = 1 ] && ok || bad "parent investigate started $nstart workers (want 1): $out"
c2audit=0; c3audit=0
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
  [ -f "$ad/contract-audit.md" ] && grep -q '^VERDICT: PASS' "$ad/contract-audit.md" || continue
  [ "$item" = C2 ] && c2audit=1
  [ "$item" = C3 ] && c3audit=1
done
[ "$c2audit" -eq 1 ] && ok || bad 'C2 did not receive isomorphic contract-audit PASS'
[ "$c3audit" -eq 1 ] && ok || bad 'C3 did not receive isomorphic contract-audit PASS'
# C1 STALE copies onto C2/C3 next tick without a coordinator ACP / kiro-ct hop
"$P/crucible" run-claim C1 a1 -- sh -c 'echo argparse dests' >/dev/null
"$P/crucible" claim verdict C1 a1 STALE >/dev/null
grep -q 'status: STALE' "$P/CLAIMS.md" && ok || bad 'STALE verdict wrote AUDITED_FALSE instead of STALE'
out=$("$P/crucible" drive tick 2>&1) || { bad "stale-like tick died: $out"; out=; }
printf '%s\n' "$out" | grep -q 'coordinator invoke failed' && bad "stale-like tick invoked coordinator: $out" || ok
[ ! -f "$P/DRIVE.BRIEF.md" ] && ok || bad 'stale-like tick wrote DRIVE.BRIEF'
[ -f "$P/claims/C2/verdicts/a1.md" ] && grep -q '^CLAIM-VERDICT: STALE$' "$P/claims/C2/verdicts/a1.md" && ok \
  || bad 'C2 did not receive isomorphic STALE verdict'
[ -f "$P/claims/C3/verdicts/a1.md" ] && grep -q '^CLAIM-VERDICT: STALE$' "$P/claims/C3/verdicts/a1.md" && ok \
  || bad 'C3 did not receive isomorphic STALE verdict'
nstart=$(printf '%s\n' "$out" | grep -c 'drive: started' || true)
[ "$nstart" = 0 ] && ok || bad "stale-like tick started $nstart workers (want 0 after isomorphic close): $out"
"$P/crucible" claim list 2>/dev/null | grep -q AUDITED_FALSE \
  && bad 'claim list said AUDITED_FALSE for STALE' || ok

# bound investigation: FILE without --next names --next and --panel-from
printf 'Hashed bulk rank is uncharged.\n' > "$repo/real.md"
refuses 'bound leftover PROBLEM names --next and --panel-from' 'leftover PROBLEM|--next|--panel-from' \
  "$P/crucible" cycle problem "$repo/real.md"

# --- adopt --panel-from copies approved panel; leftover PROBLEM stays ---
repo=$(setup_repo)
P="$repo/.crucible/work"
write_agents "$P"
write_panel "$P"
"$P/crucible" cycle approve-panel >/dev/null
printf 'Assess flags are missing.\n' > "$repo/parked.md"
"$P/crucible" cycle problem "$repo/parked.md" >/dev/null
src_panel=$(sed -n 's/^panel-id: //p' "$P/PANEL.APPROVAL" | head -1)
( cd "$repo" && "$C" adopt live --managed --panel-from work >/dev/null )
Q="$repo/.crucible/live"
[ -f "$Q/PROGRAM" ] && ok || bad 'sibling adopt did not write PROGRAM'
[ -f "$Q/agents.tsv" ] && cmp -s "$P/agents.tsv" "$Q/agents.tsv" && ok \
  || bad 'sibling did not copy agents.tsv'
[ -f "$Q/PANEL.ASSIGN.tsv" ] && cmp -s "$P/PANEL.ASSIGN.tsv" "$Q/PANEL.ASSIGN.tsv" && ok \
  || bad 'sibling did not copy PANEL.ASSIGN.tsv'
[ -f "$Q/PANEL.APPROVAL" ] && ok || bad 'sibling missing PANEL.APPROVAL'
dst_panel=$(sed -n 's/^panel-id: //p' "$Q/PANEL.APPROVAL" | head -1)
[ "$src_panel" = "$dst_panel" ] && ok || bad "sibling panel-id $dst_panel != $src_panel"
grep -q 'TEMPLATE-PROBLEM-NEEDS-INPUT' "$Q/PROBLEM.md" && ok \
  || bad 'sibling copied leftover PROBLEM.md'
grep -q 'Assess flags are missing' "$P/PROBLEM.md" && ok \
  || bad 'panel-from mutated source PROBLEM.md'
st=$("$Q/crucible" cycle 2>&1) || { bad "sibling cycle refused: $st"; st=; }
printf '%s\n' "$st" | grep -q 'WAIT PANEL' && bad "sibling still WAIT PANEL: $st" || ok
printf '%s\n' "$st" | grep -q 'INTAKE' && ok || bad "sibling should be INTAKE: $st"
printf 'Two-key rank write charges the window.\n' > "$repo/real.md"
"$Q/crucible" cycle problem "$repo/real.md" >/dev/null
grep -q 'Two-key rank write charges the window' "$Q/PROBLEM.md" && ok \
  || bad 'sibling did not bind real PROBLEM'
grep -q 'Assess flags are missing' "$P/PROBLEM.md" && ok \
  || bad 'binding sibling PROBLEM clobbered leftover'
( cd "$repo" && "$C" adopt other --panel-from work >/dev/null 2>&1 ) \
  && bad '--panel-from without --managed was allowed' || ok
( cd "$repo" && "$C" adopt ghost --managed --panel-from missing >/dev/null 2>&1 ) \
  && bad '--panel-from missing source was allowed' || ok
( cd "$repo" && "$C" adopt work --refresh --panel-from live >/dev/null 2>&1 ) \
  && bad '--refresh --panel-from was allowed' || ok

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
