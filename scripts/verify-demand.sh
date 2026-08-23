#!/bin/sh
# RED contract for demand: work is admissible with no user-visible job on file.
#
# These three assertions PASS on current main. They are deliberately-passing tests
# of a hole, not of a guarantee. When demand becomes a gate, A1 and A2 invert to
# `refuses` and this header comes out.
#
# Verified fact that motivates the file (read from the engine, not assumed):
#   `cycle_worth` has exactly three sites in ./crucible —
#     * 2320  the definition itself
#     * 2821  the `printf 'worth: %s\n'` line inside write_cycle_status (STATUS.md)
#     * 3701  a message-selection `case` in cmd_next (which NEXT PROPOSE wording to print)
#   and ZERO references in cmd_add or cmd_claim.
#   Therefore `worth:` is advisory DISPLAY. It never refuses anything. The commands
#   that actually create work are `claim admit` and `cmd_add`, so a demand gate has
#   to live there. Future readers must not mistake `worth:` for a gate: making
#   cycle_worth stricter would change the printed advice and admit the work anyway.
#
# Defects documented here:
#   D1 (A1) `claim admit` admits work from a PROBLEM.md that names no user-visible
#           job — an absence statement about the tool's own surface is enough.
#   D2 (A2) the lexical guard in cycle_refuse_non_problem counts the literal string
#           'is not a CLI verb' (>= 8 occurrences), so the same leftover catalog
#           rephrased as 'the CLI has no `X` subcommand' binds freely.
#
# A3 is the positive control for A1: cycle_worth returns UNKNOWN unless
# cycle_investigation_state is COMPLETE, so a half-built fixture would print
# `worth: UNKNOWN` and A1's pass would be an artifact of broken setup, not a defect.

set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
C="$HERE/crucible"
PASS=0; FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }
# Same helper as scripts/verify-agent-cycle.sh. Its sibling `refuses` is deliberately
# absent: every assertion in this file is a reason-specific SUCCESS, and grepping for
# "any refusal" is exactly the sloppiness that would make a RED file lie.
expect() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) || { bad "$label: command refused: $out"; return; }
  if printf '%s\n' "$out" | grep -E -q "$pattern"; then ok; else bad "$label: wanted $pattern, got $out"; fi
}
# Fixture steps are not assertions. A broken fixture must be loud, never a silent pass.
fixture() {
  out=$("$@" 2>&1) || { printf 'FIXTURE BROKEN: %s\n%s\n' "$*" "$out" >&2; exit 1; }
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

write_panel() {
  prog=$1
  cat > "$prog/PANEL.md" <<'EOF'
# Panel

## Agents

- c0, a1, a2, mk1, j1, j2 (echo fixtures; mixed kinds)

## Roles

Cast in PANEL.ASSIGN.tsv: coordinator=c0; claim-auditors=a1,a2; maker=mk1; reviewer=j1; contract-auditor=j2.

## Risk posture

LOW for fixture demand verification.

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

# Seal the independence ledger attempt for a dispatched claim role.
# Glob order is lexical, not chronological, so pick an unsealed DISPATCHED attempt.
seal_claim_agent() {
  prog=$1; agent=$2; transport=${3:-multi-agent}
  id=
  for ad in "$prog"/attempts/A*; do
    [ -d "$ad" ] || continue
    a=$(awk -F '\t' 'NR==2 {print $6}' "$ad/meta.tsv")
    [ "$a" = "$agent" ] || continue
    st=$(awk -F '\t' 'END{print $1}' "$ad/events.tsv")
    [ "$st" = DISPATCHED ] || continue
    [ -f "$ad/transport" ] && [ -f "$ad/contract-audit.md" ] && continue
    id=${ad##*/}
  done
  [ -n "$id" ] || { printf 'FIXTURE BROKEN: no unsealed attempt for %s\n' "$agent" >&2; exit 1; }
  auditor=$(awk -F '\t' '$1=="contract-auditor"{print $2; exit}' "$prog/PANEL.ASSIGN.tsv")
  [ -n "$auditor" ] || auditor=j2
  fixture "$prog/crucible" attempt transport "$id" "$transport"
  fixture "$prog/crucible" contract-audit "$id" "$auditor" PASS
}

# A FRESH adopted managed+guided program with an APPROVED panel and no claims.
# Freshness is load-bearing: on a reused program a bind can fail for reasons that
# have nothing to do with demand ('leftover PROBLEM is still bound',
# 'approve the agent panel before binding a problem').
fresh_program() {
  name=$1
  repo="$BASE/$name"; mkdir -p "$repo"
  (
    cd "$repo"
    git init -q -b main
    printf 'baseline\n' > tracked.txt
    git add tracked.txt
    git -c user.name=test -c user.email=test@example.invalid commit -qm baseline
    "$C" adopt work --managed >/dev/null
  )
  prog="$repo/.crucible/work"
  grep -q '^lifecycle: managed$' "$prog/PROGRAM" \
    || { printf 'FIXTURE BROKEN: %s is not managed\n' "$name" >&2; exit 1; }
  write_agents "$prog"
  write_panel "$prog"
  fixture "$prog/crucible" cycle approve-panel
  [ -f "$prog/PANEL.APPROVAL" ] \
    || { printf 'FIXTURE BROKEN: %s panel not approved\n' "$name" >&2; exit 1; }
  # adopt drops a placeholder PROBLEM.md; anything else means a real bind is leftover.
  if [ -e "$prog/PROBLEM.md" ] \
    && ! grep -q 'TEMPLATE-PROBLEM-NEEDS-INPUT' "$prog/PROBLEM.md"; then
    printf 'FIXTURE BROKEN: %s already has a bound PROBLEM\n' "$name" >&2; exit 1
  fi
  [ "$("$prog/crucible" claim list 2>/dev/null | grep -c '^### C' || true)" = 0 ] \
    || { printf 'FIXTURE BROKEN: %s already has claims\n' "$name" >&2; exit 1; }
  printf '%s' "$prog"
}

BASE=$(mktemp -d "${TMPDIR:-/tmp}/crucible-demand.XXXXXX")
# Cleanup must never mask the exit status. The `0` handler removes and returns, so a normal run
# still reports its own result. Each signal handler removes and then exits 128+signal, because a
# handler that only removes lets the shell resume and reach a `0` exit — an interrupted or
# timed-out run would then be recorded as a pass.
trap 'rm -rf "$BASE"' 0
trap 'rm -rf "$BASE"; exit 129' 1
trap 'rm -rf "$BASE"; exit 130' 2
trap 'rm -rf "$BASE"; exit 143' 15

# --- A1 + A3: demandless claim becomes bounded work ------------------------------
P=$(fresh_program admit)
PR=${P%/.crucible/work}

# A PROBLEM.md that names no user-visible job: a one-line absence statement about
# the tool's own command surface. No user, no outcome, no falsifier.
printf 'The CLI has no `demand` subcommand.\n' > "$PR/problem.md"
fixture "$P/crucible" cycle problem "$PR/problem.md"

cn=$("$P/crucible" claim add 'the CLI has no demand subcommand' \
  'The CLI has no `demand` subcommand.' ABSENT)
[ "$cn" = C1 ] || { printf 'FIXTURE BROKEN: expected C1, got %s\n' "$cn" >&2; exit 1; }
grep -q '^    polarity: ABSENT$' "$P/CLAIMS.md" \
  || { printf 'FIXTURE BROKEN: polarity ABSENT not recorded\n' >&2; exit 1; }

# Sealed TRUE verdicts across the configured kinds (a1=kindA, a2=kindB).
for agent in a1 a2; do
  fixture "$P/crucible" run-claim "$cn" "$agent" -- sh -c 'echo read the command table'
  fixture "$P/crucible" dispatch "$cn" claim-auditor "$agent"
  seal_claim_agent "$P" "$agent"
  fixture "$P/crucible" claim verdict "$cn" "$agent" TRUE
done
fixture "$P/crucible" run-claim "$cn" a1 -- sh -c 'echo searched for existing subcommand'
fixture "$P/crucible" dispatch "$cn" scout a1
seal_claim_agent "$P" a1
fixture "$P/crucible" claim scout "$cn" ABSENT a1

# A3 (positive control, must be checked BEFORE A1 is read as a defect):
# the investigation is COMPLETE, so worth: is a real projection and not UNKNOWN.
# `cycle` reaching the post-investigation NEXT PROPOSE line proves the state is
# COMPLETE (EMPTY/NEEDS_AUDIT/NEEDS_SCOUT all return earlier with NEXT INVESTIGATE),
# and the BUILD wording is the branch cmd_next takes only when worth is not
# DOCS/NO-BUILD. STATUS.md then carries the literal projection.
a3out=$("$P/crucible" cycle 2>&1) || a3out="COMMAND REFUSED: $a3out"
if printf '%s\n' "$a3out" | grep -E -q '^NEXT PROPOSE — write a refined, evidence-grounded PROPOSAL\.md$' \
  && grep -q '^worth: BUILD$' "$P/STATUS.md"; then
  ok
else
  bad "investigation reached COMPLETE: wanted NEXT PROPOSE + worth: BUILD, got cycle=[$a3out] status-worth=[$(grep '^worth:' "$P/STATUS.md" 2>/dev/null || echo missing)]"
fi

cat > "$P/PROPOSAL.md" <<'EOF'
# Proposal

## Verified problem

The CLI has no `demand` subcommand; two sealed TRUE verdicts and an ABSENT scout.

## Proposed outcome

Add the missing subcommand.

## Non-goals

No service and no permanent agent memory.

## Backlog

One bounded item adding the subcommand.

## Verification

The subcommand exists and is invocable.
EOF
fixture "$P/crucible" cycle approve

# A1 — THE DEFECT. Nothing in the admission path asks who wants this or what job
# it does. `claim admit` succeeds on a PROBLEM.md with no user-visible demand.
expect 'demandless claim is admissible' '^admitted C1 as item demand-subcommand$' \
  "$P/crucible" claim admit "$cn" demand-subcommand

# --- A2: rephrased leftover catalog binds ----------------------------------------
# Separate FRESH program: approved panel, no prior claims, nothing bound. Without
# that, a refusal about a leftover PROBLEM or an unapproved panel could masquerade
# as this defect (or hide it), so the assertion checks the SUCCESS message —
# cycle_bind_problem printing the bound path — not the absence of an error.
Q=$(fresh_program catalog)
QR=${Q%/.crucible/work}

# 12 rows, every one an absence statement about a CLI subcommand. The engine's
# guard counts the literal 'is not a CLI verb' and needs >= 8; this phrasing
# scores 0, so the size-based refusal never fires.
: > "$QR/catalog.md"
i=1
while [ "$i" -le 12 ]; do
  printf 'The CLI has no `verb%s` subcommand.\n' "$i" >> "$QR/catalog.md"
  i=$((i + 1))
done
rows=$(grep -c 'subcommand' "$QR/catalog.md")
[ "$rows" -eq 12 ] || { printf 'FIXTURE BROKEN: catalog has %s rows\n' "$rows" >&2; exit 1; }
grep -qiE 'is not a CLI verb' "$QR/catalog.md" \
  && { printf 'FIXTURE BROKEN: catalog trips the literal guard\n' >&2; exit 1; }

expect 'rephrased catalog binds' '/PROBLEM\.md$' \
  "$Q/crucible" cycle problem "$QR/catalog.md"

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
