#!/bin/sh
# Focused contract test for cleanup: it must work in THIS repository and in ANY target repo.
#
# Fixture-only. The agents are echo commands, nothing reaches the network, and no real agent
# CLI is invoked. One managed+guided program is adopted into a throwaway repository and driven
# to DONE, then reused by every assertion, because standing a program up costs about ten
# seconds and this suite has to stay runnable on every push.
#
# The load-bearing assertion is the dirty-worktree refusal. `git worktree remove` without
# --force refuses to destroy uncommitted work, which is correct; what is verified here is that
# the operator is told which worktree, why, and the exact command that clears it — first in the
# --dry-run preview, then in the refusal — and that running the named command lets a retry pass.

set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
C="$HERE/crucible"
# A real tab. `\t` inside a grep BRE is a literal `t` under GNU grep, so any tab-anchored
# assertion must interpolate this instead.
tab=$(printf '\t')
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }
expect() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) || { bad "$label: command refused: $out"; return; }
  printf '%s\n' "$out" | grep -q "$pattern" && ok || bad "$label: wanted $pattern, got $out"
}
refuses() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) && { bad "$label: command accepted"; return; }
  printf '%s\n' "$out" | grep -q "$pattern" && ok || bad "$label: wanted $pattern, got $out"
}

write_agents() {
  prog=$1
  {
    printf 'c0%skindA%sm%shigh%secho {BRIEF}\n' "$tab" "$tab" "$tab" "$tab"
    printf 'a1%skindA%sm%shigh%secho {BRIEF}\n' "$tab" "$tab" "$tab" "$tab"
    printf 'a2%skindB%sm%shigh%secho {BRIEF}\n' "$tab" "$tab" "$tab" "$tab"
    printf 'mk1%skindA%sm%shigh%secho {BRIEF}\n' "$tab" "$tab" "$tab" "$tab"
    printf 'j1%skindB%sm%shigh%secho {BRIEF}\n' "$tab" "$tab" "$tab" "$tab"
    printf 'j2%skindB%sm%shigh%secho {BRIEF}\n' "$tab" "$tab" "$tab" "$tab"
  } > "$prog/agents.tsv"
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

LOW for fixture cleanup verification.

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
  {
    printf 'role%sagent%srequired%snotes\n' "$tab" "$tab" "$tab"
    printf 'coordinator%sc0%syes%sthis session; not maker/reviewer\n' "$tab" "$tab" "$tab"
    printf 'claim-auditor%sa1%syes\n' "$tab" "$tab"
    printf 'claim-auditor%sa2%syes\n' "$tab" "$tab"
    printf 'scout%sa1%sno\n' "$tab" "$tab"
    printf 'maker%smk1%syes\n' "$tab" "$tab"
    printf 'reviewer%sj1%syes\n' "$tab" "$tab"
    printf 'contract-auditor%sj2%syes\n' "$tab" "$tab"
  } > "$prog/PANEL.ASSIGN.tsv"
}

seal_claim_attempt() {
  # Seal the newest unsealed DISPATCHED claim attempt for one agent and print its id.
  # Glob order is lexical, not chronological, so the last A* can be an older sibling.
  prog=$1; agent=$2
  id=
  for ad in "$prog"/attempts/A*; do
    [ -d "$ad" ] || continue
    a=$(awk -F "$tab" 'NR == 2 { print $6 }' "$ad/meta.tsv")
    [ "$a" = "$agent" ] || continue
    st=$(awk -F "$tab" 'END { print $1 }' "$ad/events.tsv")
    [ "$st" = DISPATCHED ] || continue
    [ -f "$ad/transport" ] && [ -f "$ad/contract-audit.md" ] && continue
    id=${ad##*/}
  done
  [ -n "$id" ] || { printf 'no unsealed claim attempt for %s\n' "$agent" >&2; return 1; }
  "$prog/crucible" attempt transport "$id" multi-agent >/dev/null
  "$prog/crucible" contract-audit "$id" j2 PASS >/dev/null
  printf '%s' "$id"
}

tmp=$(mktemp -d "${TMPDIR:-/tmp}/crucible-cleanup.XXXXXX")
# Normalise: a TMPDIR with a trailing slash yields `//` here, and the engine prints its own
# root through `cd && pwd`, so the two spellings would never compare equal in an assertion.
tmp=$(cd "$tmp" && pwd)
trap 'rm -rf "$tmp"' 0 1 2 15

# Everything below runs against a foreign target repository, never against this one.
repo="$tmp/target"
mkdir -p "$repo"
(
  cd "$repo"
  git init -q -b main
  # Repo-local identity, not just -c on the fixture's own commits: cleanup and its recovery
  # commands operate inside linked worktrees of this repo, which share this config. Disable
  # rerere and signing so a developer's global git config cannot change what is measured.
  git config user.name test
  git config user.email test@example.invalid
  git config commit.gpgsign false
  git config rerere.enabled false
  printf 'baseline\n' > tracked.txt
  git add tracked.txt
  git commit -qm baseline
  "$C" adopt work --managed >/dev/null
)
Q="$repo/.crucible/work"

snapshot() { find "$Q" -type f -exec cksum {} \; | LC_ALL=C sort | cksum; }
tmp_entries() { ls -A "${1:-${TMPDIR:-/tmp}}" 2>/dev/null | wc -l | tr -d ' '; }

# Drive the adopted program to DONE the cheapest honest way: one claim the panel falsifies,
# so the cycle reaches NO-BUILD DONE without any maker or review work.
write_agents "$Q"
write_panel "$Q"
"$Q/crucible" cycle approve-panel >/dev/null
printf 'No code change is actually required.\n' > "$repo/report.md"
"$Q/crucible" cycle problem "$repo/report.md" >/dev/null
claim=$("$Q/crucible" claim add 'code change required' 'No code change is actually required.')
for agent in a1 a2; do
  "$Q/crucible" run-claim "$claim" "$agent" -- sh -c 'echo checked current behavior' >/dev/null
  "$Q/crucible" dispatch "$claim" claim-auditor "$agent" >/dev/null
  seal_claim_attempt "$Q" "$agent" >/dev/null
  "$Q/crucible" claim verdict "$claim" "$agent" FALSE >/dev/null
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
"$Q/crucible" cycle approve >/dev/null

# --- 1. cleanup works in a target repository -----------------------------------------------
expect 'an adopted target-repo program reaches DONE' '^DONE ' "$Q/crucible" cycle
before_preview=$(snapshot)
preview=$("$Q/crucible" cycle clean --dry-run 2>&1) || bad 'dry-run refused a DONE cycle'
printf '%s\n' "$preview" | grep -q "^KEEP $Q/agents.tsv " \
  && ok || bad "dry-run must KEEP agents.tsv: $preview"
printf '%s\n' "$preview" | grep -q "^KEEP $Q/PANEL.ASSIGN.tsv " \
  && ok || bad "dry-run must KEEP PANEL.ASSIGN.tsv: $preview"
printf '%s\n' "$preview" | grep -q "^PRESERVE $Q " \
  && ok || bad "dry-run must PRESERVE the program directory: $preview"
[ "$(snapshot)" = "$before_preview" ] && ok || bad 'dry-run modified the program'

# --- 2. cleanup refuses while an attempt is live, and damages nothing ----------------------
"$Q/crucible" dispatch "$claim" claim-auditor a1 >/dev/null
live=$(seal_claim_attempt "$Q" a1)
# $$ is this script, so the recorded pid is alive for as long as the refusal is measured.
"$Q/crucible" attempt start "$live" "$$" >/dev/null
# Settle the generated views first. A new ledger entry rewrites the independence receipt on the
# next status read, and that refresh is not the damage this assertion is looking for.
"$Q/crucible" cycle >/dev/null
before_live=$(snapshot)
refuses 'cleanup refuses while an attempt is live' "attempt $live is RUNNING (live pid)" \
  "$Q/crucible" cycle clean --apply
[ "$(snapshot)" = "$before_live" ] && ok || bad 'the live-attempt refusal changed the program'
"$Q/crucible" attempt finish "$live" ABANDONED 'fixture releases the live attempt' >/dev/null

# --- 3. drive stop releases a leftover lock, and cleanup then proceeds ---------------------
mkdir "$Q/.drive.lock"
expect 'a leftover lock is reported to the operator' '^stale-lock: .drive.lock$' \
  "$Q/crucible" cycle clean --dry-run
expect 'drive stop releases the leftover lock' "^released $Q/.drive.lock$" \
  "$Q/crucible" drive stop
[ ! -e "$Q/.drive.lock" ] && ok || bad 'drive stop left .drive.lock in place'
"$Q/crucible" cycle clean --dry-run 2>&1 | grep -q '^stale-lock:' \
  && bad 'cleanup still reports a stale lock after drive stop' || ok

# --- 4. a clean worktree is removed; a mid-cherry-pick worktree refuses, actionably --------
# Two commits that cannot be cherry-picked onto each other, so the integration worktree can be
# left in exactly the state a non-conflict cherry-pick failure leaves it in.
(
  cd "$repo"
  git checkout -q -b side main
  printf 'side\n' > tracked.txt
  git commit -qam side
  git checkout -q main
  printf 'mainward\n' > tracked.txt
  git commit -qam mainward
)
mkdir -p "$Q/worktrees"
task_wt="$Q/worktrees/task"
integrate_wt="$Q/worktrees/integrate-cleanup"
git -C "$repo" worktree add -q -b ai/cleanup-task "$task_wt" main
git -C "$repo" worktree add -q -b integrate-cleanup "$integrate_wt" main
git -C "$integrate_wt" cherry-pick side >/dev/null 2>&1 \
  && bad 'fixture cherry-pick was expected to fail and did not' || ok
integrate_gitdir=$(git -C "$integrate_wt" rev-parse --absolute-git-dir)
[ -e "$integrate_gitdir/CHERRY_PICK_HEAD" ] \
  && ok || bad 'fixture did not leave the integration worktree mid-cherry-pick'

before_blocked=$(snapshot)
preview=$("$Q/crucible" cycle clean --dry-run 2>&1) || bad 'dry-run refused with worktrees present'
printf '%s\n' "$preview" | grep -q '^REMOVE_WORKTREE .*/worktrees/task$' \
  && ok || bad "dry-run must preview the task worktree: $preview"
printf '%s\n' "$preview" | grep -q '^PRESERVE_BRANCH .* ai/cleanup-task$' \
  && ok || bad "dry-run must preview branch preservation: $preview"
printf '%s\n' "$preview" \
  | grep -q '^BLOCKED_WORKTREE .*/worktrees/integrate-cleanup .* in-progress cherry-pick; clear it with: git -C .* cherry-pick --abort' \
  && ok || bad "dry-run must name the blocked worktree and its recovery command: $preview"
[ "$(snapshot)" = "$before_blocked" ] && ok || bad 'the blocking dry-run modified the program'

refusal=$("$Q/crucible" cycle clean --apply 2>&1) && bad 'apply accepted a mid-cherry-pick worktree' || :
# Scope every assertion to the refusal line itself. `--apply` prints the whole preview before it
# refuses, so a diagnosis found anywhere in that output proves nothing about the refusal.
refusal_line=$(printf '%s\n' "$refusal" | grep 'could not safely remove worktree:' | head -1)
printf '%s\n' "$refusal_line" | grep -q 'could not safely remove worktree: .*/worktrees/integrate-cleanup' \
  && ok || bad "apply must refuse and name the worktree: $refusal"
printf '%s\n' "$refusal_line" | grep -q 'in-progress cherry-pick' \
  && ok || bad "the refusal must report why the worktree is dirty: $refusal"
printf '%s\n' "$refusal_line" | grep -q 'then retry: crucible cycle clean --apply' \
  && ok || bad "the refusal must name the verb to retry: $refusal"
[ -d "$integrate_wt" ] && [ -e "$integrate_gitdir/CHERRY_PICK_HEAD" ] \
  && ok || bad 'the refused apply destroyed the in-progress cherry-pick it refused to remove'

# Run the recovery exactly as the refusal spells it. A command the operator cannot copy out and
# run is not an actionable message, so it is read back out of the message rather than retyped.
recovery=$(printf '%s\n' "$refusal_line" \
  | sed -n 's/.*clear it with: \(git -C .* cherry-pick --abort\),.*/\1/p' | head -1)
[ -n "$recovery" ] && ok || bad "the refusal named no recovery command: $refusal"
if [ -n "$recovery" ] && sh -c "$recovery" >/dev/null 2>&1; then ok; else bad "the documented recovery command failed: $recovery"; fi

evidence=$(find "$Q/claims" -type f -name '*.txt' | LC_ALL=C sort | head -1)
[ -n "$evidence" ] || bad 'fixture recorded no claim evidence to preserve'
expect 'a retry after the documented recovery succeeds' '^session cleanup applied' \
  "$Q/crucible" cycle clean --apply
[ ! -e "$task_wt" ] && ok || bad 'apply did not remove the clean task worktree'
git -C "$repo" show-ref --verify --quiet refs/heads/ai/cleanup-task \
  && ok || bad 'apply destroyed the task branch it promised to preserve'
[ ! -e "$integrate_wt" ] && ok || bad 'apply did not remove the recovered integration worktree'
git -C "$repo" show-ref --verify --quiet refs/heads/integrate-cleanup \
  && ok || bad 'apply destroyed the integration branch it promised to preserve'
[ ! -e "$Q/worktrees" ] && ok || bad 'apply left the worktrees tree behind'
[ -f "$Q/agents.tsv" ] && [ -f "$Q/PANEL.ASSIGN.tsv" ] \
  && ok || bad 'apply destroyed panel identity'
[ -f "$Q/PROBLEM.md" ] && [ -f "$Q/PROPOSAL.md" ] && [ -f "$evidence" ] \
  && ok || bad 'apply destroyed the problem, the proposal, or recorded evidence'

# --- 5. an uncommitted change refuses with the other recovery, and clears the same way -----
mkdir -p "$Q/worktrees"
dirty_wt="$Q/worktrees/dirty"
git -C "$repo" worktree add -q -b ai/cleanup-dirty "$dirty_wt" main
printf 'uncommitted\n' >> "$dirty_wt/tracked.txt"
expect 'an uncommitted change is previewed as blocked, with a recovery' \
  '^BLOCKED_WORKTREE .*/worktrees/dirty .* uncommitted changes; commit them, or discard with: git -C .* stash --include-untracked' \
  "$Q/crucible" cycle clean --dry-run
refuses 'apply refuses a worktree with uncommitted changes' \
  'could not safely remove worktree: .*/worktrees/dirty .* uncommitted changes' \
  "$Q/crucible" cycle clean --apply
grep -q '^uncommitted$' "$dirty_wt/tracked.txt" \
  && ok || bad 'the refusal destroyed the uncommitted change it refused to remove'
git -C "$dirty_wt" commit -qam 'operator commits the pending change'
expect 'apply succeeds once the operator has committed' '^session cleanup applied' \
  "$Q/crucible" cycle clean --apply
[ ! -e "$dirty_wt" ] && ok || bad 'apply did not remove the committed worktree'

# --- 6. a representative fast suite leaks nothing into TMPDIR ------------------------------
# Measured against a private TMPDIR, not the shared one. The shared directory is written by
# unrelated processes on a developer machine, so a count taken across it reports their entries
# as this suite's leak. A private TMPDIR the child honours measures only the child.
leak_suite="$HERE/scripts/verify-managed-lifecycle.sh"
if [ -x "$leak_suite" ]; then
  leak_probe="$tmp/tmpdir-probe"
  mkdir -p "$leak_probe"
  before_tmp=$(tmp_entries "$leak_probe")
  TMPDIR="$leak_probe" "$leak_suite" >/dev/null 2>&1 \
    || bad 'the representative suite failed, so its leak count means nothing'
  after_tmp=$(tmp_entries "$leak_probe")
  [ "$after_tmp" -le "$before_tmp" ] \
    && ok || bad "a suite run grew its TMPDIR from $before_tmp to $after_tmp entries"
else
  bad "no representative suite to measure for leaks: $leak_suite"
fi

# --- 7. this suite cleans up after itself --------------------------------------------------
# Remove explicitly and assert, rather than trusting the trap silently. The trap stays armed
# until this point so an early exit still cleans up.
rm -rf "$tmp"
trap - 0 1 2 15
[ ! -e "$tmp" ] && ok || bad "this suite left its own scratch directory behind: $tmp"

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
