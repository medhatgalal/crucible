#!/bin/sh
# Focused contract test for managed lifecycle behavior.

set -eu

HERE=$(cd "$(dirname "$0")/.." && pwd)
C="$HERE/crucible"
# A real tab. `\t` inside a grep BRE is a literal `t` under GNU grep, so
# tab-anchored assertions against STATE.tsv must interpolate this instead.
tab=$(printf '\t')
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


bind_independence() {
  prog=$1; id=$2; transport=${3:-multi-agent}; auditor=${4:-}
  if [ -z "$auditor" ]; then
    attempt_agent=$(awk -F '	' 'NR==2 {print $6}' "$prog/attempts/$id/meta.tsv")
    role=$(awk -F '	' 'NR==2 {print $5}' "$prog/attempts/$id/meta.tsv")
    slug=$(awk -F '	' 'NR==2 {print $2}' "$prog/attempts/$id/meta.tsv")
    auditor=
    for cand in $(grep -v '^#' "$prog/agents.tsv" | awk -F '	' '$1!=""{print $1}'); do
      [ "$cand" = "$attempt_agent" ] && continue
      case $role in
        judge|adversary)
          if [ -f "$prog/items/$slug/MAKERS.tsv" ] && awk -F '	' -v a="$cand" '$1==a{found=1} END{exit !found}' "$prog/items/$slug/MAKERS.tsv"; then
            continue
          fi
          if [ -f "$prog/items/$slug/MAKER" ] && grep -qx "$cand" "$prog/items/$slug/MAKER"; then
            continue
          fi
          ;;
      esac
      auditor=$cand
      break
    done
    [ -n "$auditor" ] || auditor=$attempt_agent
  fi
  "$prog/crucible" attempt transport "$id" "$transport" >/dev/null
  "$prog/crucible" contract-audit "$id" "$auditor" PASS >/dev/null
}

tmp=$(mktemp -d "${TMPDIR:-/tmp}/crucible-managed-lifecycle.XXXXXX")
trap 'rm -rf "$tmp"' 0 1 2 15
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
[ -f "$P/docs/managed-lifecycle.md" ] && ok || bad 'adopt did not ship the managed lifecycle guide'
expect 'item-file lifecycle is the compatibility default' '^lifecycle: item-file$' "$P/crucible" lifecycle status
before_enable=$(find "$P" -type f -exec cksum {} \; | LC_ALL=C sort | cksum)
expect 'managed lifecycle dry-run declares writes' '^CREATE .*STATE.tsv' "$P/crucible" lifecycle enable --dry-run
after_enable=$(find "$P" -type f -exec cksum {} \; | LC_ALL=C sort | cksum)
[ "$before_enable" = "$after_enable" ] && ok || bad 'lifecycle dry-run changed program files'
expect 'managed lifecycle can be enabled' '^CREATE .*STATE.tsv' "$P/crucible" lifecycle enable --apply
expect 'program records behavior by name' '^lifecycle: managed$' "$P/crucible" lifecycle status
grep -q '^lifecycle: managed$' "$P/PROGRAM" && ok || bad 'PROGRAM does not declare managed lifecycle behavior'
header=$(printf 'item\tstatus\tstage\twork_id\trisk\tinflight_attempt\tblock_code\tupdated_epoch')
[ "$(sed -n '1p' "$P/STATE.tsv")" = "$header" ] \
  && ok || bad 'STATE.tsv header differs from the managed lifecycle contract'
grep -q '^Generated from `STATE.tsv`' "$P/STATE.md" && ok || bad 'STATE.md is not visibly generated'
state_inode_before=$(ls -i "$P/STATE.tsv" | awk '{print $1}')
"$P/crucible" state >/dev/null
state_inode_after=$(ls -i "$P/STATE.tsv" | awk '{print $1}')
[ "$state_inode_before" = "$state_inode_after" ] && ok || bad 'state rewrote authoritative STATE.tsv'

(
  cd "$repo"
  .crucible/p/crucible add alpha 'state kernel' >/dev/null
)
grep -q '^PHASE:' "$P/items/alpha/ITEM.md" && bad 'managed ITEM.md retained a PHASE header' || ok
grep -q '^STATUS:' "$P/items/alpha/ITEM.md" && bad 'managed ITEM.md retained a STATUS header' || ok
grep -q "^alpha${tab}ACTIVE${tab}DRAFT${tab}" "$P/STATE.tsv" && ok || bad 'new item is not ACTIVE DRAFT'

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

Create the managed lifecycle state kernel.

## Non-goals

No task-DAG parallelism in this slice.

## Risk

MEDIUM

## Owned files

- crucible
- scripts/verify-managed-lifecycle.sh
- tracked.txt

## Acceptance criteria

- [ ] A1: managed state is authoritative.

## Focused falsifier

scripts/verify-managed-lifecycle.sh

## Expensive evidence

NONE

## Stop conditions

Stop if item-file compatibility breaks.
EOF

expect 'ready transition' 'alpha is now READY' "$P/crucible" ready alpha
grep -q "^alpha${tab}ACTIVE${tab}READY${tab}" "$P/STATE.tsv" && ok || bad 'ready did not update STATE.tsv'
grep -q '| alpha | ACTIVE | READY |' "$P/STATE.md" && ok || bad 'ready did not regenerate STATE.md'
refuses 'cannot skip BUILD' 'transition READY -> REVIEW' "$P/crucible" phase alpha REVIEW
expect 'BUILD transition' 'alpha is now in BUILD' "$P/crucible" phase alpha BUILD
refuses 'review requires implemented work' 'current-work maker PASS' "$P/crucible" phase alpha REVIEW

maker_contract=$($P/crucible dispatch alpha maker mk1 A1 FOCUSED 2>/dev/null)
maker_attempt=$(basename "$(dirname "$maker_contract")")
bind_independence "$P" "$maker_attempt"
$P/crucible attempt start "$maker_attempt" "$$" >/dev/null
(
  cd "$repo"
  git checkout -qb ai/alpha
  printf 'implemented\n' >> tracked.txt
  git add tracked.txt
  git -c user.name=test -c user.email=test@example.invalid commit -qm implementation
)
maker_out=$(cd "$repo" && .crucible/p/crucible run alpha mk1 -- sh -c 'echo focused-maker')
maker_evidence=$(basename "$(printf '%s' "$maker_out" | awk '{print $1}')")
$P/crucible attempt finish "$maker_attempt" RETURNED observed-exit-zero >/dev/null
$P/crucible result "$maker_attempt" PASS "$maker_evidence" CLOSE - >/dev/null
refuses 'managed evidence requires a live attempt' 'in-flight attempt' \
  "$P/crucible" run alpha j1 -- sh -c 'echo unbound-review'

expect 'REVIEW transition' 'alpha is now in REVIEW' "$P/crucible" phase alpha REVIEW
refuses 'cannot move backward' 'REVIEW -> BUILD requires a judge result' "$P/crucible" phase alpha BUILD
expect 'REVIEW next action' '^NEXT alpha CHECK ' "$P/crucible" next

j1_contract=$($P/crucible dispatch alpha judge j1 A1 FOCUSED 2>/dev/null)
j1_attempt=$(basename "$(dirname "$j1_contract")")
bind_independence "$P" "$j1_attempt"
$P/crucible attempt start "$j1_attempt" "$$" >/dev/null
j1_out=$(cd "$repo" && .crucible/p/crucible run alpha j1 -- sh -c 'echo focused-j1')
j1_evidence=$(basename "$(printf '%s' "$j1_out" | awk '{print $1}')")
$P/crucible attempt finish "$j1_attempt" RETURNED observed-exit-zero >/dev/null
wid=$($P/crucible workid alpha)
printf 'VERDICT: PASS\nWORK-ID: %s\nmanual compatibility verdict; see %s\n' "$wid" "$j1_evidence" \
  > "$P/items/alpha/verdicts/j1.md"
refuses 'managed check rejects a hand-written verdict' 'not bound to an attempt result' \
  "$P/crucible" check alpha
$P/crucible result "$j1_attempt" PASS "$j1_evidence" CLOSE - >/dev/null

j2_contract=$($P/crucible dispatch alpha judge j2 A1 FOCUSED 2>/dev/null)
j2_attempt=$(basename "$(dirname "$j2_contract")")
bind_independence "$P" "$j2_attempt"
$P/crucible attempt start "$j2_attempt" "$$" >/dev/null
j2_out=$(cd "$repo" && .crucible/p/crucible run alpha j2 -- sh -c 'echo focused-j2')
j2_evidence=$(basename "$(printf '%s' "$j2_out" | awk '{print $1}')")
$P/crucible attempt finish "$j2_attempt" RETURNED observed-exit-zero >/dev/null
$P/crucible result "$j2_attempt" PASS "$j2_evidence" CLOSE - >/dev/null

expect 'managed check is closeable' '^CLOSEABLE ' "$P/crucible" check alpha
expect 'managed close' '^closed alpha at ' "$P/crucible" close alpha 'state is authoritative'
grep -q "^alpha${tab}CLOSED${tab}REVIEW${tab}" "$P/STATE.tsv" && ok || bad 'close did not update authoritative state'
expect 'closed program is done' '^DONE$' "$P/crucible" next

item_file="$tmp/item-file"
mkdir -p "$item_file"
cp "$C" "$item_file/crucible"
chmod +x "$item_file/crucible"
(
  cd "$item_file"
  ./crucible add old 'item-file item' >/dev/null
)
grep -q '^PHASE: SPEC$' "$item_file/items/old/ITEM.md" && ok || bad 'item-file program lost phase behavior'
grep -q '^STATUS: OPEN$' "$item_file/items/old/ITEM.md" && ok || bad 'item-file program lost status behavior'
refuses 'cannot enable after first item' 'before the first item' "$item_file/crucible" lifecycle enable --apply

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
