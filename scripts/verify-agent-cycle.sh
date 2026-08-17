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
  prog=$1; agent=$2; transport=${3:-multi-agent}
  id=
  for ad in "$prog"/attempts/A*; do
    [ -d "$ad" ] || continue
    aid=${ad##*/}
    a=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
    [ "$a" = "$agent" ] || continue
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

base=$(mktemp -d "${TMPDIR:-/tmp}/crucible-agent-cycle.XXXXXX")
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
scout_contract=$(ls "$P/claims/$cn/dispatches/"*-scout-a1.md)
if grep -E -q "claim verdict $cn a1" "$scout_contract"; then
  bad 'scout contract embeds claim-auditor verdict surface'
else
  ok
fi
if grep -q "claims/$cn/evidence/" "$scout_contract" && grep -q 'claim scout' "$scout_contract"; then
  ok
else
  bad 'scout contract omits report path or claim scout result'
fi
seal_claim_agent "$P" a1
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
$Q/crucible cycle approve >/dev/null
expect 'verified no-work proposal reaches done' '^DONE ' "$Q/crucible" cycle
mkdir -p "$Q/worktrees"
git -C "$clean_repo" worktree add -q -b cleanup-session "$Q/worktrees/session" main
expect 'cleanup previews exact machine state' '^DELETE .*agents.tsv' "$Q/crucible" cycle clean --dry-run
expect 'cleanup preview includes isolated worktrees' 'REMOVE_WORKTREE .*worktrees/session' \
  "$Q/crucible" cycle clean --dry-run
[ -f "$Q/agents.tsv" ] && ok || bad 'cleanup dry-run changed the agent registry'
expect 'approved cleanup preserves durable evidence' 'session cleanup applied' "$Q/crucible" cycle clean --apply
if [ ! -e "$Q/agents.tsv" ] && [ ! -e "$Q/worktrees/session" ] \
  && [ -f "$Q/PROBLEM.md" ] && [ -f "$Q/PROPOSAL.md" ] \
  && git -C "$clean_repo" show-ref --verify --quiet refs/heads/cleanup-session; then
  ok
else
  bad 'session cleanup removed durable evidence/branch or retained machine state'
fi

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
