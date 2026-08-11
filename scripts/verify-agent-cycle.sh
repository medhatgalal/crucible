#!/bin/sh
# Focused cold-onboarding contract for the operator-facing agent cycle.

set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
C="$HERE/crucible"
PASS=0; FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }
expect() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) || { bad "$label: command refused: $out"; return; }
  if printf '%s\n' "$out" | grep -q "$pattern"; then ok; else bad "$label: wanted $pattern, got $out"; fi
}
refuses() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) && { bad "$label: command accepted"; return; }
  if printf '%s\n' "$out" | grep -q "$pattern"; then ok; else bad "$label: wanted $pattern, got $out"; fi
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
expect 'cold agent sees one intake state' '^NEXT INTAKE ' "$P/crucible" cycle
help=$($P/crucible help)
if printf '%s\n' "$help" | grep -q 'crucible attempt'; then bad 'public help exposes attempt mechanics'; else ok; fi
if printf '%s\n' "$help" | grep -q 'Operators normally do not run these commands'; then
  ok
else
  bad 'public help does not explain the agent-owned boundary'
fi

printf '# Broken behavior\n\nThe program does not preserve approved scope.\n' > "$repo/report.md"
expect 'problem report enters the cycle' '/PROBLEM.md$' "$P/crucible" cycle problem "$repo/report.md"
expect 'bound problem advances to investigation' '^NEXT INVESTIGATE ' "$P/crucible" cycle
refuses 'low-level item creation cannot bypass proposal approval' 'not operator-approved' \
  "$P/crucible" add bypass 'must not start yet'

printf 'a1\tkindA\tm\thigh\techo {BRIEF}\n' > "$P/agents.tsv"
printf 'a2\tkindB\tm\thigh\techo {BRIEF}\n' >> "$P/agents.tsv"
cn=$($P/crucible claim add 'approved scope is not preserved' 'The program does not preserve approved scope.')
$P/crucible run-claim "$cn" a1 -- sh -c 'echo observed source and behavior' >/dev/null
$P/crucible run-claim "$cn" a2 -- sh -c 'echo independently reproduced' >/dev/null
$P/crucible claim verdict "$cn" a1 TRUE >/dev/null
$P/crucible claim verdict "$cn" a2 TRUE >/dev/null
$P/crucible run-claim "$cn" a1 -- sh -c 'echo searched for existing enforcement' >/dev/null
$P/crucible claim scout "$cn" ABSENT a1 >/dev/null
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
if grep -q 'Do not conduct a long setup interview' "$P/BOOTSTRAP.md"; then
  ok
else
  bad 'bootstrap still permits command-heavy onboarding'
fi

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
printf 'No code change is actually required.\n' > "$clean_repo/report.md"
$Q/crucible cycle problem "$clean_repo/report.md" >/dev/null
printf 'a1\tkindA\tm\thigh\techo {BRIEF}\na2\tkindB\tm\thigh\techo {BRIEF}\n' > "$Q/agents.tsv"
qc=$($Q/crucible claim add 'code change required' 'No code change is actually required.')
for agent in a1 a2; do
  $Q/crucible run-claim "$qc" "$agent" -- sh -c 'echo checked current behavior' >/dev/null
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
