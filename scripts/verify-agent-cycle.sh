#!/bin/sh
# Focused cold-onboarding contract for the operator-facing agent cycle.
# Proves configure → panel approve → problem → investigate → proposal → done,
# and that solo-theatre shortcuts are refused.

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
  {
    printf 'c0\tkindA\tm\thigh\techo {BRIEF}\n'
    printf 'a1\tkindA\tm\thigh\techo {BRIEF}\n'
    printf 'a2\tkindB\tm\thigh\techo {BRIEF}\n'
    printf 'mk1\tkindA\tm\thigh\techo {BRIEF}\n'
    printf 'j1\tkindB\tm\thigh\techo {BRIEF}\n'
    printf 'j2\tkindB\tm\thigh\techo {BRIEF}\n'
  } > "$prog/agents.tsv"
}


seal_claim_agent() {
  # After claim dispatch, seal the independence ledger attempt for that agent.
  # Pick an unsealed DISPATCHED attempt: glob order is lexical, not chronological,
  # so "last A*" can be an already SUPERSEDED sibling.
  prog=$1; agent=$2; transport=${3:-multi-agent}
  id=
  for ad in "$prog"/attempts/A*; do
    [ -d "$ad" ] || continue
    aid=${ad##*/}
    a=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
    [ "$a" = "$agent" ] || continue
    st=$(awk -F '\t' 'END{print $1}' "$ad/events.tsv")
    [ "$st" = DISPATCHED ] || continue
    [ -f "$ad/transport" ] && [ -f "$ad/contract-audit.md" ] && continue
    id=$aid
  done
  [ -n "$id" ] || { echo "no claim attempt for $agent" >&2; return 1; }
  auditor=$(awk -F '\t' '$1=="contract-auditor"{print $2; exit}' "$prog/PANEL.ASSIGN.tsv")
  [ -n "$auditor" ] || auditor=j2
  "$prog/crucible" attempt transport "$id" "$transport" >/dev/null
  "$prog/crucible" contract-audit "$id" "$auditor" PASS >/dev/null
}

write_panel() {
  prog=$1
  cat > "$prog/PANEL.md" <<'EOF'
# Panel

## Agents

- c0, a1, a2, mk1, j1, j2 (echo fixtures; mixed kinds)

## Roles

Cast in PANEL.ASSIGN.tsv: coordinator=c0; claim-auditors=a1,a2; maker=mk1; reviewer=j1; contract-auditor=j2.

## Risk posture

LOW for fixture onboarding verification.

## Isolation transport

Prefer multi-agent. Same-family only with label. ACP before subagent on single-product hosts.

## Independence ladder

1. multi-agent
2. acp
3. subagent after ACP probe failure
4. stop if none invocable

## Waivers

NONE for this fixture.
EOF
  cat > "$prog/PANEL.ASSIGN.tsv" <<'EOF'
role	agent	required	notes
coordinator	c0	yes	this session; not maker/reviewer
claim-auditor	a1	yes
claim-auditor	a2	yes
scout	a1	no
maker	mk1	yes
reviewer	j1	yes	≠ maker
contract-auditor	j2	yes
EOF
}

TMP=$(mktemp -d "${TMPDIR:-/tmp}/crucible-agent-cycle.XXXXXX")
# Cleanup must never mask the exit status. The `0` handler removes and returns, so a normal run
# still reports its own result. Each signal handler removes and then exits 128+signal, because a
# handler that only removes lets the shell resume and reach a `0` exit — an interrupted or
# timed-out run would then be recorded as a pass.
trap 'rm -rf "$TMP"' 0
trap 'rm -rf "$TMP"; exit 129' 1
trap 'rm -rf "$TMP"; exit 130' 2
trap 'rm -rf "$TMP"; exit 143' 15
base=$(mktemp -d "$TMP/base.XXXXXX")
repo="$base/repo"; mkdir -p "$repo"
(
  cd "$repo"
  git init -q -b main
  printf 'baseline\n' > tracked.txt
  git add tracked.txt
  git -c user.name=test -c user.email=test@example.invalid commit -qm baseline
  "$C" adopt work --managed >/dev/null
)
P="$repo/.crucible/work"

if grep -q '^lifecycle: managed$' "$P/PROGRAM" && [ -f "$P/STATE.tsv" ]; then
  ok
else
  bad 'cold adoption did not select managed behavior atomically'
fi
expect 'cold agent must configure panel first' '^NEXT CONFIGURE ' "$P/crucible" cycle
help=$($P/crucible help)
if printf '%s\n' "$help" | grep -q 'crucible attempt'; then bad 'public help exposes attempt mechanics'; else ok; fi
if printf '%s\n' "$help" | grep -q 'Operators normally do not run these commands'; then
  ok
else
  bad 'public help does not explain the agent-owned boundary'
fi
if printf '%s\n' "$help" | grep -q 'approve-panel'; then ok; else bad 'public help omits approve-panel'; fi

printf '# Broken behavior\n\nThe program does not preserve approved scope.\n' > "$repo/report.md"
refuses 'problem cannot bind before panel approval' 'approve the agent panel' \
  "$P/crucible" cycle problem "$repo/report.md"

# Placeholder agents still block even with a panel draft.
write_panel "$P"
refuses 'placeholder agents.tsv cannot approve panel' 'placeholders|incomplete|PANEL' \
  "$P/crucible" cycle approve-panel

write_agents "$P"
# Casting table required: panel without ASSIGN refuses.
rm -f "$P/PANEL.ASSIGN.tsv"
refuses 'panel without role casting refuses' 'ASSIGN|casting|incomplete' \
  "$P/crucible" cycle approve-panel
write_panel "$P"
write_agents "$P"
# maker=reviewer same agent refuses
printf 'role\tagent\trequired\tnotes\ncoordinator\tc0\tyes\nclaim-auditor\ta1\tyes\nmaker\tmk1\tyes\nreviewer\tmk1\tyes\ncontract-auditor\tj2\tyes\n' \
  > "$P/PANEL.ASSIGN.tsv"
refuses 'maker and reviewer same agent refuses' 'incomplete|ASSIGN|casting' \
  "$P/crucible" cycle approve-panel
write_panel "$P"
write_agents "$P"
expect 'panel approval is content-bound' '^approved panel ' "$P/crucible" cycle approve-panel
expect 'approved panel advances to problem intake' '^NEXT INTAKE ' "$P/crucible" cycle
expect 'problem report enters the cycle' '/PROBLEM.md$' "$P/crucible" cycle problem "$repo/report.md"
expect 'bound problem advances to investigation' '^NEXT INVESTIGATE ' "$P/crucible" cycle
refuses 'low-level item creation cannot bypass proposal approval' 'not operator-approved' \
  "$P/crucible" add bypass 'must not start yet'

cn=$($P/crucible claim add 'approved scope is not preserved' 'The program does not preserve approved scope.')
$P/crucible run-claim "$cn" a1 -- sh -c 'echo observed source and behavior' >/dev/null
$P/crucible run-claim "$cn" a2 -- sh -c 'echo independently reproduced' >/dev/null
refuses 'guided TRUE requires claim dispatch' 'claim dispatch' \
  "$P/crucible" claim verdict "$cn" a1 TRUE
refuses 'uncast agent cannot claim-audit' 'not cast' \
  "$P/crucible" dispatch "$cn" claim-auditor mk1
$P/crucible dispatch "$cn" claim-auditor a1 >/dev/null
seal_claim_agent "$P" a1
$P/crucible dispatch "$cn" claim-auditor a2 >/dev/null
seal_claim_agent "$P" a2
$P/crucible claim verdict "$cn" a1 TRUE >/dev/null
$P/crucible claim verdict "$cn" a2 TRUE >/dev/null
$P/crucible run-claim "$cn" a1 -- sh -c 'echo searched for existing enforcement' >/dev/null
$P/crucible dispatch "$cn" scout a1 >/dev/null
$P/crucible dispatch "$cn" scout a1 >/dev/null
scout_contract=$(ls "$P/claims/$cn/dispatches/"2-scout-a1.md)
if grep -F -q "claim verdict $cn a1 TRUE" "$scout_contract"; then
  ok
else
  bad 'scout contract omits its optional exact TRUE-verdict command'
fi
if grep -q "claims/$cn/evidence/" "$scout_contract" && grep -q 'claim scout' "$scout_contract"; then
  ok
else
  bad 'scout contract omits report path or claim scout result'
fi
first_scout=
last_scout=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  role=$(awk -F '\t' 'NR==2 {print $5}' "$ad/meta.tsv")
  agent=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  [ "$role" = scout ] && [ "$agent" = a1 ] || continue
  [ -z "$first_scout" ] && first_scout=${ad##*/}
  last_scout=${ad##*/}
done
[ -n "$first_scout" ] && [ "$first_scout" != "$last_scout" ] && ok \
  || bad 'need two scout attempts to prove latest-sealed bind'
if [ -n "$last_scout" ] && [ -f "$P/attempts/$last_scout/contract.md" ]; then
  if grep -q 'Independence seal (before verdict)' "$scout_contract" \
    && grep -F -q "attempt transport $last_scout multi-agent|acp|subagent" "$scout_contract" \
    && grep -F -q "contract-audit $last_scout <contract-auditor> PASS" "$scout_contract" \
    && grep -q 'Independence seal (before verdict)' "$P/attempts/$last_scout/contract.md" \
    && grep -F -q "attempt transport $last_scout multi-agent|acp|subagent" "$P/attempts/$last_scout/contract.md" \
    && grep -F -q "contract-audit $last_scout <contract-auditor> PASS" "$P/attempts/$last_scout/contract.md"; then
    ok
  else
    bad 'scout dispatch and attempt contract must carry the exact pre-verdict independence seal commands'
  fi
fi
# Two kinds are registered (kindA + kindB). Same-product ACP hop must still
# be labelable as acp — the cycle is multi-agent; this hop is not.
scout_aid=
for ad in "$P"/attempts/A*; do
  [ -d "$ad" ] || continue
  item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
  agent=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
  role=$(awk -F '\t' 'NR==2 {print $5}' "$ad/meta.tsv")
  [ "$item" = "$cn" ] && [ "$agent" = a1 ] && [ "$role" = scout ] && scout_aid=${ad##*/}
done
[ -n "$scout_aid" ] || { bad 'no scout attempt to label acp'; scout_aid=; }
if [ -n "$scout_aid" ]; then
  expect 'acp transport allowed on multi-kind panel' "transport acp" \
    "$P/crucible" attempt transport "$scout_aid" acp
fi
seal_claim_agent "$P" a1 acp
if [ -n "$last_scout" ]; then
  grep -q '^FAILURES: none$' "$P/attempts/$last_scout/contract-audit.md" \
    && grep -q '^REQUIRED_FIX: none$' "$P/attempts/$last_scout/contract-audit.md" \
    && ok || bad 'PASS must record FAILURES: none and REQUIRED_FIX: none'
fi
$P/crucible claim scout "$cn" ABSENT a1 >/dev/null
refuses 'dispatch with no args prints usage' 'usage: crucible dispatch' \
  "$P/crucible" dispatch
expect 'verified claims advance to proposal' '^NEXT PROPOSE ' "$P/crucible" cycle

cat > "$P/PROPOSAL.md" <<'EOF'
# Proposal

## Verified problem

Current behavior does not preserve approved scope; see claim evidence.

## Proposed outcome

Bind executable work to the approved proposal content.

## Non-goals

No service or permanent agent memory.

## Backlog

One bounded item enforcing approval before admission.

## Verification

Admission refuses before approval and after proposal drift.
EOF
expect 'complete proposal waits for the human' '^WAIT APPROVAL ' "$P/crucible" cycle
refuses 'work cannot be admitted before approval' 'not operator-approved' \
  "$P/crucible" claim admit "$cn" preserve-scope
expect 'explicit approval is content-bound' '^approved proposal ' "$P/crucible" cycle approve
expect 'approval releases planning' '^NEXT PLAN ' "$P/crucible" cycle
expect 'approved verified claim becomes bounded work' '^admitted C1 as item preserve-scope$' \
  "$P/crucible" claim admit "$cn" preserve-scope
expect 'second admit attaches to the ACTIVE slug' '^admitted C1 as item preserve-scope$' \
  "$P/crucible" claim admit "$cn" preserve-scope
expect 'active bounded work stays in the loop' '^NEXT PLAN preserve-scope ' "$P/crucible" cycle

printf '\nChanged after approval.\n' >> "$P/PROPOSAL.md"
expect 'proposal drift invalidates approval' '^WAIT APPROVAL ' "$P/crucible" cycle

if grep -q 'same-family' "$P/START.md" && grep -q 'make.*review.*fix' "$P/START.md"; then
  ok
else
  bad 'fresh-agent prompt does not teach same-family risk and iterative review'
fi
if grep -qE 'compact configure|Role casting|role casting' "$P/BOOTSTRAP.md" \
  && grep -qE 'PANEL.ASSIGN|role casting|Role casting' "$P/BOOTSTRAP.md" \
  && grep -qE 'independence ladder|Independence ladder|multi-agent' "$P/BOOTSTRAP.md"; then
  ok
else
  bad 'bootstrap does not teach configure, role casting, and independence ladder'
fi
if grep -q 'PANEL.ASSIGN' "$P/START.md" && grep -qE 'Role casting|role casting' "$P/START.md"; then
  ok
else
  bad 'START.md does not teach PANEL.ASSIGN role casting'
fi
if grep -q 'Do not conduct a long setup interview' "$P/BOOTSTRAP.md"; then
  bad 'bootstrap still locks anti-interview policy that skipped agent configuration'
else
  ok
fi
if grep -qE 'contract-auditor|contract auditor' "$P/START.md"; then
  ok
else
  bad 'START.md does not mention contract-auditor'
fi

# No-work proposal path with panel (FALSE still requires dispatch + independence seal).
clean_repo="$base/clean-repo"; mkdir -p "$clean_repo"
(
  cd "$clean_repo"
  git init -q -b main
  printf 'baseline\n' > tracked.txt
  git add tracked.txt
  git -c user.name=test -c user.email=test@example.invalid commit -qm baseline
  "$C" adopt work --managed >/dev/null
)
Q="$clean_repo/.crucible/work"
write_agents "$Q"
write_panel "$Q"
$Q/crucible cycle approve-panel >/dev/null
printf 'No code change is actually required.\n' > "$clean_repo/report.md"
$Q/crucible cycle problem "$clean_repo/report.md" >/dev/null
qc=$($Q/crucible claim add 'code change required' 'No code change is actually required.')
for agent in a1 a2; do
  $Q/crucible run-claim "$qc" "$agent" -- sh -c 'echo checked current behavior' >/dev/null
  $Q/crucible dispatch "$qc" claim-auditor "$agent" >/dev/null
  seal_claim_agent "$Q" "$agent"
  $Q/crucible claim verdict "$qc" "$agent" FALSE >/dev/null
done
cat > "$Q/PROPOSAL.md" <<'EOF'
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
expect 'FALSE complete waits approval' '^WAIT APPROVAL ' "$Q/crucible" cycle
grep -q '^worth: NO-BUILD$' "$Q/STATUS.md" && ok || bad 'FALSE-complete should project worth: NO-BUILD'
$Q/crucible cycle approve >/dev/null
expect 'verified no-work proposal reaches done' '^DONE ' "$Q/crucible" cycle
out=$("$Q/crucible" cycle 2>&1) || true
printf '%s\n' "$out" | grep -q '^CLEANUP —' && ok || bad "DONE missing CLEANUP card: $out"
printf '%s\n' "$out" | grep -q 'panel-title-stale' \
  && bad "DONE CLEANUP panel-title-stale after CONTEXT sync: $out" || ok
printf '%s\n' "$out" | grep -q 'KEEP panel: agents.tsv' && ok \
  || bad "CLEANUP card must KEEP panel: $out"
printf '%s\n' "$out" | grep -q 'NOT cycle clean: Jira' && ok || bad 'CLEANUP card must exclude Jira'
printf '%s\n' "$out" | grep -q 'cycle clean --dry-run' && ok || bad 'DONE must name cycle clean --dry-run'
mkdir -p "$Q/worktrees"
git -C "$clean_repo" worktree add -q -b cleanup-session "$Q/worktrees/session" main
expect 'cleanup keeps panel identity' 'KEEP .*agents.tsv' "$Q/crucible" cycle clean --dry-run
outc=$("$Q/crucible" cycle clean --dry-run 2>&1) || true
printf '%s\n' "$outc" | grep -q 'DELETE .*agents.tsv' && bad 'NO-BUILD dry-run must not DELETE agents.tsv' || ok
printf '%s\n' "$outc" | grep -q 'KEEP .*PANEL.ASSIGN.tsv' && ok || bad 'dry-run must KEEP PANEL.ASSIGN.tsv'
expect 'cleanup preview includes isolated worktrees' 'REMOVE_WORKTREE .*worktrees/session' \
  "$Q/crucible" cycle clean --dry-run
[ -f "$Q/agents.tsv" ] && ok || bad 'cleanup dry-run changed the agent registry'
expect 'approved cleanup preserves durable evidence' 'session cleanup applied' "$Q/crucible" cycle clean --apply
if [ -f "$Q/agents.tsv" ] && [ ! -e "$Q/worktrees/session" ] \
  && [ -f "$Q/PROBLEM.md" ] && [ -f "$Q/PROPOSAL.md" ] \
  && git -C "$clean_repo" show-ref --verify --quiet refs/heads/cleanup-session; then
  ok
else
  bad 'session cleanup must keep agents.tsv, drop worktree, keep PROBLEM/PROPOSAL'
fi

# --like copies a real PASS onto an isomorphic DISPATCHED scout
like_base=$(mktemp -d "$TMP/like.XXXXXX")
like_repo="$like_base/repo"; mkdir -p "$like_repo"
(
  cd "$like_repo"
  git init -q -b main
  printf 'x\n' > t; git add t
  git -c user.name=test -c user.email=test@example.invalid commit -qm i
  "$C" adopt work --managed >/dev/null
)
L="$like_repo/.crucible/work"
write_agents "$L"
write_panel "$L"
"$L/crucible" cycle approve-panel >/dev/null
printf 'Two gaps exist.\nAlso a second gap.\n' > "$like_repo/report.md"
"$L/crucible" cycle problem "$like_repo/report.md" >/dev/null
c1=$("$L/crucible" claim add 'two gaps exist' 'Two gaps exist.')
c2=$("$L/crucible" claim add 'also a second gap' 'Also a second gap.')
"$L/crucible" dispatch "$c1" scout a1 >/dev/null
"$L/crucible" dispatch "$c2" scout a1 >/dev/null
id1=; id2=
for ad in "$L"/attempts/A*; do
  [ -d "$ad" ] || continue
  item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
  role=$(awk -F '\t' 'NR==2 {print $5}' "$ad/meta.tsv")
  [ "$role" = scout ] || continue
  [ "$item" = "$c1" ] && id1=${ad##*/}
  [ "$item" = "$c2" ] && id2=${ad##*/}
done
[ -n "$id1" ] && [ -n "$id2" ] || bad 'like-test missing scout attempts'
"$L/crucible" attempt transport "$id1" acp >/dev/null
"$L/crucible" attempt transport "$id2" acp >/dev/null
"$L/crucible" contract-audit "$id1" j2 PASS >/dev/null
expect 'PASS --like copies isomorphic scout audit' 'like '"$id1" \
  "$L/crucible" contract-audit "$id1" j2 PASS --like "$id2"
grep -q '^VERDICT: PASS$' "$L/attempts/$id2/contract-audit.md" && ok \
  || bad '--like did not write PASS onto the sibling attempt'

# Ten same-agent claim-auditor dispatches expose numeric-order resolution: dispatch 1 remains on
# the dispatch ledger but is deliberately made non-matching, so it must be skipped; among the
# remaining matches, 2 is earlier than 10 even though the shell glob lists 10 first.
# Only attempt 10 is sealed, which lets the verdict be written on the pre-fix engine. Every later
# consumer must nevertheless resolve the auditor to attempt 2 and refuse/report that blocker.
nbase=$(mktemp -d "$TMP/numeric.XXXXXX")
nrepo="$nbase/repo"; mkdir -p "$nrepo"
(
  cd "$nrepo"
  git init -q -b main
  printf 'x\n' > t; git add t
  git -c user.name=test -c user.email=test@example.invalid commit -qm i
  "$C" adopt work --managed >/dev/null
)
N="$nrepo/.crucible/work"
write_agents "$N"
write_panel "$N"
"$N/crucible" cycle approve-panel >/dev/null
printf 'Numeric dispatch order must be stable.\n' > "$nrepo/report.md"
"$N/crucible" cycle problem "$nrepo/report.md" >/dev/null
nc=$("$N/crucible" claim add 'numeric dispatch order is not stable' \
  'Numeric dispatch order must be stable.' ABSENT)
"$N/crucible" run-claim "$nc" a1 -- sh -c 'echo searched' >/dev/null
"$N/crucible" dispatch "$nc" scout a1 >/dev/null
n_scout=$(sed -n 's/^attempt-id: //p' "$N/claims/$nc/dispatches/1-scout-a1.md" | head -1)
"$N/crucible" attempt transport "$n_scout" multi-agent >/dev/null
"$N/crucible" contract-audit "$n_scout" j2 PASS >/dev/null
"$N/crucible" claim scout "$nc" ABSENT a1 >/dev/null
ni=1
while [ "$ni" -le 10 ]; do
  "$N/crucible" dispatch "$nc" claim-auditor a1 >/dev/null
  ni=$((ni + 1))
done
n1=$(sed -n 's/^attempt-id: //p' "$N/claims/$nc/dispatches/1-claim-auditor-a1.md" | head -1)
n2=$(sed -n 's/^attempt-id: //p' "$N/claims/$nc/dispatches/2-claim-auditor-a1.md" | head -1)
n10=$(sed -n 's/^attempt-id: //p' "$N/claims/$nc/dispatches/10-claim-auditor-a1.md" | head -1)
# Dispatch 1 remains on the dispatch ledger but no longer stamps a matching claim attempt, so every
# resolver must skip it. This isolates numeric ordering among the remaining matches.
awk -F '\t' 'BEGIN { OFS="\t" } NR == 2 { $2="C999" } { print }' \
  "$N/attempts/$n1/meta.tsv" > "$N/attempts/$n1/meta.tsv.tmp"
mv "$N/attempts/$n1/meta.tsv.tmp" "$N/attempts/$n1/meta.tsv"
"$N/crucible" attempt transport "$n10" multi-agent >/dev/null
"$N/crucible" contract-audit "$n10" j2 PASS >/dev/null
"$N/crucible" run-claim "$nc" a1 -- sh -c 'echo reproduced' >/dev/null
"$N/crucible" claim verdict "$nc" a1 TRUE >/dev/null
n_dispatches=$(printf '%s\n' "$N/claims/$nc/dispatches/"*-claim-auditor-a1.md | wc -l | tr -d ' ')
if [ "$n_dispatches" -eq 10 ] && [ -n "$n1" ] && [ -n "$n2" ] && [ -n "$n10" ] \
  && [ "$n1" != "$n2" ] && [ "$n2" != "$n10" ] \
  && [ "$(awk -F '\t' 'NR == 2 { print $2 }' "$N/attempts/$n1/meta.tsv")" = C999 ] \
  && [ ! -e "$N/attempts/$n2/transport" ] \
  && [ -e "$N/attempts/$n10/transport" ]; then
  ok
else
  bad "B5 fixture: expected ten claim-auditor dispatches with non-matching 1, unsealed numeric-earliest match 2 ($n2), and sealed 10 ($n10); found $n_dispatches dispatches"
fi
n_triage=$(CRUCIBLE_MIN_AUDITORS=1 "$N/crucible" triage 2>&1) || true
n_rec=$(printf '%s\n' "$n_triage" | sed -n 's/^ *-> //p' | head -1)
n_cycle=$(CRUCIBLE_MIN_AUDITORS=1 "$N/crucible" cycle 2>&1) || true
set +e
n_admit=$(CRUCIBLE_MIN_AUDITORS=1 "$N/crucible" claim admit "$nc" numeric-probe 2>&1)
n_admit_rc=$?
set -e
case $n_rec in ADMIT*) n_triage_safe= ;; *) n_triage_safe=1 ;; esac
case $n_cycle in 'NEXT PROPOSE'*) n_cycle_safe= ;; *) n_cycle_safe=1 ;; esac
if [ -n "$n_triage_safe" ] && [ -n "$n_cycle_safe" ] \
  && [ "$n_admit_rc" -ne 0 ] \
  && printf '%s\n' "$n_admit" | grep -q "attempt $n2 has no transport"; then
  ok
else
  bad "B5: after ten dispatches with 1 skipped, every consumer must resolve the earliest numeric claim-auditor match 2, not 10 — triage: ${n_rec:-none}; cycle: $(printf '%s\n' "$n_cycle" | head -1); claim admit: $n_admit"
fi

# Scout TRUE is optional but fully gate-visible when its own scout attempt is sealed. A second
# claim proves the converse by hand-placing the same canonical TRUE artifact behind an unsealed
# scout attempt: triage, cycle, and admission must all keep it below the default floor.
sbase=$(mktemp -d "$TMP/scout-true.XXXXXX")
srepo="$sbase/repo"; mkdir -p "$srepo"
(
  cd "$srepo"
  git init -q -b main
  printf 'x\n' > t; git add t
  git -c user.name=test -c user.email=test@example.invalid commit -qm i
  "$C" adopt work --managed >/dev/null
)
S="$srepo/.crucible/work"
write_agents "$S"
write_panel "$S"
printf 'scout\ta2\tno\toptional affirmative scout verdict\n' >> "$S/PANEL.ASSIGN.tsv"
"$S/crucible" cycle approve-panel >/dev/null
printf 'Scout verdict eligibility must track its own seal.\n' > "$srepo/report.md"
"$S/crucible" cycle problem "$srepo/report.md" >/dev/null
spos=$("$S/crucible" claim add 'sealed scout TRUE is not eligible' \
  'Scout verdict eligibility must track its own seal.' ABSENT)
for swho in a1 a2; do
  "$S/crucible" run-claim "$spos" "$swho" -- sh -c 'echo independently checked' >/dev/null
done
"$S/crucible" dispatch "$spos" claim-auditor a1 >/dev/null
seal_claim_agent "$S" a1
"$S/crucible" claim verdict "$spos" a1 TRUE >/dev/null
"$S/crucible" dispatch "$spos" scout a2 >/dev/null
seal_claim_agent "$S" a2
"$S/crucible" claim scout "$spos" ABSENT a2 >/dev/null
"$S/crucible" claim verdict "$spos" a2 TRUE >/dev/null
spos_triage=$("$S/crucible" triage 2>&1) || true
spos_rec=$(printf '%s\n' "$spos_triage" | sed -n 's/^ *-> //p' | head -1)
spos_cycle=$("$S/crucible" cycle 2>&1) || true
set +e
spos_admit=$(CRUCIBLE_MIN_KINDS=9 "$S/crucible" claim admit "$spos" scout-positive-probe 2>&1)
spos_admit_rc=$?
set -e
if [ "$spos_rec" = 'ADMIT — audited TRUE by 2 agent(s) across 2 kind(s), and absent from the repo.' ] \
  && printf '%s\n' "$spos_cycle" | grep -q '^NEXT PROPOSE ' \
  && [ "$spos_admit_rc" -ne 0 ] \
  && printf '%s\n' "$spos_admit" | grep -q 'audited by 2 kind(s), need 9'; then
  ok
else
  bad "sealed scout TRUE plus one sealed auditor TRUE must reach the default floor — triage: ${spos_rec:-none}; cycle: $(printf '%s\n' "$spos_cycle" | head -1); admit probe: $spos_admit"
fi

sneg=$("$S/crucible" claim add 'unsealed scout TRUE is eligible' \
  'Scout verdict eligibility must track its own seal.' ABSENT)
for swho in a1 a2; do
  "$S/crucible" run-claim "$sneg" "$swho" -- sh -c 'echo independently checked' >/dev/null
done
"$S/crucible" dispatch "$sneg" claim-auditor a1 >/dev/null
seal_claim_agent "$S" a1
"$S/crucible" claim verdict "$sneg" a1 TRUE >/dev/null
"$S/crucible" dispatch "$sneg" scout a1 >/dev/null
seal_claim_agent "$S" a1
"$S/crucible" claim scout "$sneg" ABSENT a1 >/dev/null
"$S/crucible" dispatch "$sneg" scout a2 >/dev/null
sneg_attempt=$(sed -n 's/^attempt-id: //p' \
  "$S/claims/$sneg/dispatches/1-scout-a2.md" | head -1)
printf 'CLAIM-VERDICT: TRUE\nAGENT: a2\nKIND: kindB\nCITATION: claims/%s/evidence/a2.txt\n' \
  "$sneg" > "$S/claims/$sneg/verdicts/a2.md"
sneg_triage=$("$S/crucible" triage 2>&1) || true
sneg_rec=$(printf '%s\n' "$sneg_triage" \
  | sed -n '/^C2  /,/^$/{s/^ *-> //p;}' | head -1)
sneg_cycle=$("$S/crucible" cycle 2>&1) || true
set +e
sneg_admit=$("$S/crucible" claim admit "$sneg" scout-negative-probe 2>&1)
sneg_admit_rc=$?
set -e
case $sneg_rec in
  "INDEPENDENCE INCOMPLETE — 2 TRUE on file, 1 counted across 1 kind(s); need 2 across 1. a2 resolves to attempt $sneg_attempt, which has no transport"*) sneg_triage_safe=1 ;;
  *) sneg_triage_safe= ;;
esac
if [ -n "$sneg_triage_safe" ] \
  && [ "$sneg_cycle" = 'NEXT INVESTIGATE — independently fact-check every unresolved ABSENT claim (NO-BUILD if all FALSE/STALE)' ] \
  && [ "$sneg_admit_rc" -ne 0 ] \
  && printf '%s\n' "$sneg_admit" | grep -q "refused: attempt $sneg_attempt has no transport"; then
  ok
else
  bad "unsealed scout TRUE must remain below the default floor on every surface — triage: ${sneg_rec:-none}; cycle: $(printf '%s\n' "$sneg_cycle" | head -1); admit: $sneg_admit"
fi

# --- 1.5.0 claim grammar, verdict history, polarity, REVIEW->BUILD, PANEL.CONTEXT ---
gbase=$(mktemp -d "$TMP/gram.XXXXXX")
grepo="$gbase/repo"; mkdir -p "$grepo"
(
  cd "$grepo"
  git init -q -b main
  printf 'x\n' > t; git add t
  git -c user.name=test -c user.email=test@example.invalid commit -qm i
  "$C" adopt work --managed >/dev/null
)
G="$grepo/.crucible/work"
write_agents "$G"
write_panel "$G"
"$G/crucible" cycle approve-panel >/dev/null
printf 'A report.\n' > "$grepo/report.md"
"$G/crucible" cycle problem "$grepo/report.md" >/dev/null
refuses 'bundled claim refuses' 'one predicate|bundle' \
  "$G/crucible" claim add 'assess and status JSON and freshness' 'source'
refuses 'desired-behavior claim refuses' 'desired behavior|not a gap' \
  "$G/crucible" claim add 'Init writes AGENTS.md' 'Init writes AGENTS.md'
cn=$("$G/crucible" claim add 'workgraph nosuchverb is not a CLI verb' \
  'workgraph nosuchverb is not a CLI verb' ABSENT)
grep -q 'polarity: ABSENT' "$G/CLAIMS.md" && ok || bad 'polarity not recorded'
[ -f "$G/PANEL.CONTEXT.md" ] && grep -q 'problem-title:' "$G/PANEL.CONTEXT.md" && ok \
  || bad 'cycle problem did not sync PANEL.CONTEXT.md'
st=$("$G/crucible" cycle 2>&1) || true
printf '%s\n' "$st" | grep -q 'NO-BUILD if all FALSE/STALE' && ok \
  || bad "ABSENT-only STATUS missing NO-BUILD close: $st"
printf '%s\n' "$st" | grep -q 'admit needs' && bad "ABSENT-only STATUS still says admit needs: $st" || ok
# verdict never clobbers; an implicit citation must stay bound to usable regular evidence even
# when a lexically later non-empty directory also matches the evidence glob.
implicit_evidence_out=$("$G/crucible" run-claim "$cn" a1 -- sh -c 'echo ev')
implicit_evidence=${implicit_evidence_out%% *}
implicit_expected="claims/$cn/evidence/$(basename "$implicit_evidence")"
mkdir -p "$G/claims/$cn/evidence/a1.zzz.txt"
printf 'not evidence\n' > "$G/claims/$cn/evidence/a1.zzz.txt/child"
"$G/crucible" dispatch "$cn" claim-auditor a1 >/dev/null
seal_claim_agent "$G" a1
printf 'WRITEUP keep me\n' > "$G/claims/$cn/verdicts/a1.md"
"$G/crucible" claim verdict "$cn" a1 FALSE >/dev/null
implicit_citation=$(sed -n 's/^CITATION: //p' "$G/claims/$cn/verdicts/a1.md" | head -1)
implicit_mark=$(awk -F "'" '/^MARK=/ { print $2; exit }' "$G/crucible")
if [ "$implicit_citation" = "$implicit_expected" ] \
  && [ -f "$G/$implicit_citation" ] && [ -r "$G/$implicit_citation" ] \
  && [ -s "$G/$implicit_citation" ] \
  && [ "$(head -1 "$G/$implicit_citation")" = "$implicit_mark" ]; then
  ok
else
  bad "B4: implicit citation must select the regular usable evidence file despite a later non-empty directory — expected $implicit_expected, got ${implicit_citation:-none}"
fi
[ -f "$G/claims/$cn/verdicts/a1.md" ] && grep -q 'CITATION:' "$G/claims/$cn/verdicts/a1.md" && ok \
  || bad 'verdict missing CITATION'
ls "$G/claims/$cn/verdicts/history/"a1.*.md >/dev/null 2>&1 && ok \
  || bad 'prior verdict writeup was not archived'
grep -q 'status: AUDITED_FALSE' "$G/CLAIMS.md" && ok || bad 'FALSE did not set AUDITED_FALSE'
cs=$("$G/crucible" claim add 'assess --label is not a CLI flag' \
  'assess --label is not a CLI flag' ABSENT)
cb=$("$G/crucible" claim add 'assess --type is not a CLI flag' \
  'assess --type is not a CLI flag' ABSENT)
"$G/crucible" run-claim "$cs" a1 -- sh -c 'echo argparse' >/dev/null
"$G/crucible" run-claim "$cb" a1 -- sh -c 'echo argparse' >/dev/null
"$G/crucible" dispatch "$cs" claim-auditor a1 >/dev/null
"$G/crucible" dispatch "$cb" claim-auditor a1 >/dev/null
for want in "$cs" "$cb"; do
  wid=
  for ad in "$G"/attempts/A*; do
    [ -d "$ad" ] || continue
    item=$(awk -F '\t' 'NR==2 {print $2}' "$ad/meta.tsv")
    agent=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
    [ "$item" = "$want" ] && [ "$agent" = a1 ] || continue
    wid=${ad##*/}
  done
  [ -n "$wid" ] || { bad "missing $want attempt to seal"; continue; }
  "$G/crucible" attempt transport "$wid" multi-agent >/dev/null 2>&1 || true
  "$G/crucible" contract-audit "$wid" j2 PASS >/dev/null 2>&1 || true
done
"$G/crucible" claim verdict "$cs" a1 STALE --like "$cb" >/dev/null
grep -q "### $cs " "$G/CLAIMS.md" && awk -v c="### $cs " '
  $0 ~ "^"c {p=1} p && /^    status:/ {print; exit} /^### C/ && $0 !~ "^"c {p=0}
' "$G/CLAIMS.md" | grep -q 'status: STALE' && ok || bad 'STALE did not set status STALE'
[ -f "$G/claims/$cb/verdicts/a1.md" ] && grep -q '^CLAIM-VERDICT: STALE$' "$G/claims/$cb/verdicts/a1.md" && ok \
  || bad 'claim verdict --like did not copy STALE onto sibling'
# REVIEW -> BUILD after judge FIX
# reuse first cycle item path: admit needs 2 TRUEs + scout ABSENT
# (a1 already FALSE — new claim)
cfix=$("$G/crucible" claim add 'fixreentry is not a CLI verb' 'fixreentry is not a CLI verb' ABSENT)
for agent in a1 a2; do
  "$G/crucible" run-claim "$cfix" "$agent" -- sh -c 'echo checked' >/dev/null
  "$G/crucible" dispatch "$cfix" claim-auditor "$agent" >/dev/null
  seal_claim_agent "$G" "$agent"
  "$G/crucible" claim verdict "$cfix" "$agent" TRUE >/dev/null
done
"$G/crucible" run-claim "$cfix" a1 -- sh -c 'echo searched' >/dev/null
"$G/crucible" dispatch "$cfix" scout a1 >/dev/null
seal_claim_agent "$G" a1
"$G/crucible" claim scout "$cfix" ABSENT a1 >/dev/null
cat > "$G/PROPOSAL.md" <<'EOF'
# Proposal
## Verified problem
fixreentry is not a CLI verb.
## Proposed outcome
One item.
## Non-goals
None.
## Backlog
fix-item.
## Verification
Two TRUEs.
EOF
"$G/crucible" cycle approve >/dev/null
"$G/crucible" claim admit "$cfix" fix-item >/dev/null
grep -q 'status: ADMITTED' "$G/CLAIMS.md" && ok || bad 'admit did not set ADMITTED'
# fake REVIEW + judge FIX result
awk -F '\t' 'BEGIN{OFS="\t"} NR==1{print;next} $1=="fix-item"{$3="REVIEW"} {print}' \
  "$G/STATE.tsv" > "$G/STATE.tsv.tmp" && mv "$G/STATE.tsv.tmp" "$G/STATE.tsv"
mkdir -p "$G/attempts/A1787070000.1.1"
printf 'item\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\n' \
  > "$G/attempts/A1787070000.1.1/meta.tsv"
printf 'fix-item\t-\tWID\tjudge\tj1\tkindB\tA1\tlog\tRETURNED\t1\t2\t-\n' >> "$G/attempts/A1787070000.1.1/meta.tsv"
printf 'OUTCOME: REJECT\nITEM: fix-item\nROLE: judge\nNEXT: FIX\nFINDING-FINGERPRINT: abcdef123456\n' \
  > "$G/attempts/A1787070000.1.1/result.md"
printf 'state\tpid\treason\nRETURNED\t-\tfixture\n' > "$G/attempts/A1787070000.1.1/events.tsv"
expect 'REVIEW->BUILD after judge FIX' 'is now in BUILD' \
  "$G/crucible" phase fix-item BUILD
awk -F '\t' 'BEGIN{OFS="\t"} NR==1{print;next} $1=="fix-item"{$3="REVIEW";$6="A1787070000.1.1"} {print}' \
  "$G/STATE.tsv" > "$G/STATE.tsv.tmp" && mv "$G/STATE.tsv.tmp" "$G/STATE.tsv"
expect 'REVIEW->BUILD while RETURNED judge is inflight' 'is now in BUILD' \
  "$G/crucible" phase fix-item BUILD
# --next context after close-less archive: close would need full check; just call refresh via --next
# after making item not ACTIVE
awk -F '\t' 'BEGIN{OFS="\t"} NR==1{print;next} $1=="fix-item"{$2="CLOSED"} {print}' \
  "$G/STATE.tsv" > "$G/STATE.tsv.tmp" && mv "$G/STATE.tsv.tmp" "$G/STATE.tsv"
printf 'Next title here.\n' > "$grepo/next-p.md"
nxt=$("$G/crucible" cycle problem "$grepo/next-p.md" --next 2>&1) || {
  bad "--next failed: $nxt"; nxt=
}
if [ -f "$G/PANEL.CONTEXT.md" ] && grep -q 'problem-title: Next title here' "$G/PANEL.CONTEXT.md"; then
  ok
else
  ctx=
  [ -f "$G/PANEL.CONTEXT.md" ] && ctx=$(cat "$G/PANEL.CONTEXT.md")
  bad "--next did not refresh PANEL.CONTEXT.md ($nxt) context=[$ctx]"
fi
printf 'pr-status leftover\n' > "$grepo/pr-status.md"
refuses '--next refuses leftover pr-status title' 'pr-status' \
  "$G/crucible" cycle problem "$grepo/pr-status.md" --next

printf '`workgraph nosuchverb` is not a CLI verb.\n' > "$grepo/nosuch.md"
refuses 'FILE refuses nosuchverb non-problem' 'not a CLI verb|not a PROBLEM' \
  "$G/crucible" cycle problem "$grepo/nosuch.md" --next
printf 'RFC leftover remainder\n\n- assess ABSENT\n- reset ABSENT\n' > "$grepo/rfc-left.md"
refuses 'FILE refuses leftover remainder catalog' 'leftover|remainder|not a PROBLEM' \
  "$G/crucible" cycle problem "$grepo/rfc-left.md" --next

abd=$("$G/crucible" cycle problem --abandon 'leftover FILE --next of a non-problem' 2>&1) || {
  bad "abandon refused: $abd"; abd=
}
if printf '%s\n' "$abd" | grep -q abandoned && [ ! -f "$G/PROBLEM.md" ]; then
  ok
else
  bad "abandon did not archive PROBLEM.md ($abd)"
fi
ls "$G"/history/*/ABANDON.md >/dev/null 2>&1 && ok || bad 'abandon missing ABANDON.md'
"$G/crucible" cycle 2>&1 | grep -q 'NEXT INTAKE' && ok || bad 'abandon did not return to INTAKE'

{
  printf '# leftover remainder catalog\n\n'
  i=1
  while [ "$i" -le 8 ]; do
    printf 'workgraph verb%s is not a CLI verb.\n' "$i"
    i=$((i + 1))
  done
  printf '\n## Falsifier\n\nuv run pytest -q\n'
} > "$grepo/laundry.md"
refuses 'FILE refuses 8+ CLI-verb catalog even with falsifier' 'leftover catalog|not a PROBLEM' \
  "$G/crucible" cycle problem "$grepo/laundry.md"

printf 'One bounded gap: help -h must stay non-zero for nosuch.\n' > "$grepo/one.md"
"$G/crucible" cycle problem "$grepo/one.md" >/dev/null
c1=$("$G/crucible" claim add 'alpha is not a CLI verb' 'alpha is not a CLI verb' ABSENT)
c2=$("$G/crucible" claim add 'beta is not a CLI verb' 'beta is not a CLI verb' ABSENT)
c3=$("$G/crucible" claim add 'gamma is not a CLI verb' 'gamma is not a CLI verb' ABSENT)
[ -n "$c1$c2$c3" ] && ok || bad 'three NEW claims should add'
refuses 'fourth NEW claim refuses over cap' 'NEW claims|CRUCIBLE_MAX_NEW_CLAIMS' \
  "$G/crucible" claim add 'delta is not a CLI verb' 'delta is not a CLI verb' ABSENT

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
