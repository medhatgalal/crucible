#!/bin/sh
# Focused contract test for the opt-in v2 state kernel.

set -eu

HERE=$(cd "$(dirname "$0")/.." && pwd)
C="$HERE/crucible"
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }
expect() {
  label=$1
  pattern=$2
  shift 2
  out=$("$@" 2>&1) || { bad "$label: command refused: $out"; return; }
  printf '%s\n' "$out" | grep -q "$pattern" && ok || bad "$label: wanted $pattern, got $out"
}
refuses() {
  label=$1
  pattern=$2
  shift 2
  out=$("$@" 2>&1) && { bad "$label: command accepted"; return; }
  printf '%s\n' "$out" | grep -q "$pattern" && ok || bad "$label: wanted $pattern, got $out"
}

tmp=$(mktemp -d)
repo="$tmp/repo"
mkdir -p "$repo"
(
  cd "$repo"
  git init -q -b main
  printf 'baseline\n' > tracked.txt
  git add tracked.txt
  git -c user.name=test -c user.email=test@example.invalid commit -qm baseline
  "$C" adopt p >/dev/null
)

P="$repo/.crucible/p"
printf 'FORMAT_VERSION=2\n' > "$P/FORMAT_VERSION"
printf 'item\tstatus\tstage\twork_id\trisk\tinflight_attempt\tblock_code\tupdated_epoch\n' > "$P/STATE.tsv"
"$P/crucible" state >/dev/null
[ "$(cat "$P/FORMAT_VERSION")" = 'FORMAT_VERSION=2' ] && ok || bad 'fixture did not activate format 2'
header=$(printf 'item\tstatus\tstage\twork_id\trisk\tinflight_attempt\tblock_code\tupdated_epoch')
[ "$(sed -n '1p' "$P/STATE.tsv")" = "$header" ] \
  && ok || bad 'STATE.tsv header differs from the v2 contract'
grep -q '^Generated from `STATE.tsv`' "$P/STATE.md" && ok || bad 'STATE.md is not visibly generated'
state_inode_before=$(ls -i "$P/STATE.tsv" | awk '{print $1}')
"$P/crucible" state >/dev/null
state_inode_after=$(ls -i "$P/STATE.tsv" | awk '{print $1}')
[ "$state_inode_before" = "$state_inode_after" ] && ok || bad 'state rewrote authoritative STATE.tsv'

(
  cd "$repo"
  .crucible/p/crucible add alpha 'state kernel' >/dev/null
)
grep -q '^PHASE:' "$P/items/alpha/ITEM.md" && bad 'v2 ITEM.md retained a PHASE header' || ok
grep -q '^STATUS:' "$P/items/alpha/ITEM.md" && bad 'v2 ITEM.md retained a STATUS header' || ok
grep -q "^alpha\tACTIVE\tDRAFT\t" "$P/STATE.tsv" && ok || bad 'new item is not ACTIVE DRAFT'

cp "$P/STATE.tsv" "$P/STATE.keep"
awk -F '\t' -v OFS='\t' 'NR == 1 { print; next } { $4="*"; print }' "$P/STATE.keep" > "$P/STATE.tsv"
refuses 'unsafe state token' 'invalid work_id' "$P/crucible" next
mv "$P/STATE.keep" "$P/STATE.tsv"

before=$(find "$P" -type f -exec cksum {} \; | LC_ALL=C sort | cksum)
expect 'DRAFT next action' '^NEXT alpha READY ' "$P/crucible" next
after=$(find "$P" -type f -exec cksum {} \; | LC_ALL=C sort | cksum)
[ "$before" = "$after" ] && ok || bad 'next mutated program files'

refuses 'incomplete contract' 'focused falsifier' "$P/crucible" ready alpha
cat > "$P/items/alpha/ITEM.md" <<'EOF'
# alpha — state kernel

## Goal

Create the v2 state kernel.

## Non-goals

No attempt ledger in this slice.

## Risk

MEDIUM

## Owned files

- crucible
- scripts/verify-v2-state.sh

## Acceptance criteria

- [ ] A1: v2 state is authoritative.

## Focused falsifier

scripts/verify-v2-state.sh

## Expensive evidence

NONE

## Stop conditions

Stop if v1 compatibility breaks.
EOF

expect 'ready transition' 'alpha is now READY' "$P/crucible" ready alpha
grep -q "^alpha\tACTIVE\tREADY\t" "$P/STATE.tsv" && ok || bad 'ready did not update STATE.tsv'
grep -q '| alpha | ACTIVE | READY |' "$P/STATE.md" && ok || bad 'ready did not regenerate STATE.md'
refuses 'cannot skip BUILD' 'transition READY -> REVIEW' "$P/crucible" phase alpha REVIEW
expect 'BUILD transition' 'alpha is now in BUILD' "$P/crucible" phase alpha BUILD
expect 'REVIEW transition' 'alpha is now in REVIEW' "$P/crucible" phase alpha REVIEW
refuses 'cannot move backward' 'transition REVIEW -> BUILD' "$P/crucible" phase alpha BUILD
expect 'REVIEW next action' '^NEXT alpha CHECK ' "$P/crucible" next

(
  cd "$repo"
  git checkout -qb ai/alpha
  printf 'implemented\n' >> tracked.txt
  git add tracked.txt
  git -c user.name=test -c user.email=test@example.invalid commit -qm implementation
  .crucible/p/crucible brief alpha maker mk1 >/dev/null 2>&1
  .crucible/p/crucible run alpha j1 -- sh -c 'echo focused-j1' >/dev/null
  .crucible/p/crucible run alpha j2 -- sh -c 'echo focused-j2' >/dev/null
  wid=$(.crucible/p/crucible workid alpha)
  e1=$(basename "$(ls .crucible/p/items/alpha/evidence/j1.*."$wid".txt)")
  e2=$(basename "$(ls .crucible/p/items/alpha/evidence/j2.*."$wid".txt)")
  printf 'VERDICT: PASS\nWORK-ID: %s\nfocused check: %s\n' "$wid" "$e1" \
    > .crucible/p/items/alpha/verdicts/j1.md
  printf 'VERDICT: PASS\nWORK-ID: %s\nindependent check: %s\n' "$wid" "$e2" \
    > .crucible/p/items/alpha/verdicts/j2.md
)
expect 'v2 check is closeable' '^CLOSEABLE ' "$P/crucible" check alpha
expect 'v2 close' '^closed alpha at ' "$P/crucible" close alpha 'state is authoritative'
grep -q "^alpha\tCLOSED\tREVIEW\t" "$P/STATE.tsv" && ok || bad 'close did not update authoritative state'
expect 'closed program is done' '^DONE$' "$P/crucible" next

legacy="$tmp/legacy"
mkdir -p "$legacy"
cp "$C" "$legacy/crucible"
chmod +x "$legacy/crucible"
(
  cd "$legacy"
  ./crucible add old 'legacy item' >/dev/null
)
grep -q '^PHASE: SPEC$' "$legacy/items/old/ITEM.md" && ok || bad 'unmarked program lost v1 phase behavior'
grep -q '^STATUS: OPEN$' "$legacy/items/old/ITEM.md" && ok || bad 'unmarked program lost v1 status behavior'

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
