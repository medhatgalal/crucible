#!/bin/sh
# Focused contract test for immutable managed-lifecycle attempts.

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
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) || { bad "$label: command refused: $out"; return; }
  printf '%s\n' "$out" | grep -q "$pattern" && ok || bad "$label: wanted $pattern, got $out"
}
refuses() {
  label=$1; pattern=$2; shift 2
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

TMP=$(mktemp -d "${TMPDIR:-/tmp}/crucible-attempt-ledger.XXXXXX")
# Cleanup must never mask the exit status. The `0` handler removes and returns, so a normal run
# still reports its own result. Each signal handler removes and then exits 128+signal, because a
# handler that only removes lets the shell resume and reach a `0` exit — an interrupted or
# timed-out run would then be recorded as a pass.
trap 'rm -rf "$TMP"' 0
trap 'rm -rf "$TMP"; exit 129' 1
trap 'rm -rf "$TMP"; exit 130' 2
trap 'rm -rf "$TMP"; exit 143' 15

fresh() {
  base=$(mktemp -d "$TMP/case.XXXXXX")
  repo="$base/repo"
  mkdir -p "$repo"
  (
    cd "$repo"
    git init -q -b main
    printf 'baseline\n' > tracked.txt
    git add tracked.txt
    git -c user.name=test -c user.email=test@example.invalid commit -qm baseline
    "$C" adopt p >/dev/null
    .crucible/p/crucible lifecycle enable --apply >/dev/null
    .crucible/p/crucible add alpha 'attempt ledger' >/dev/null
    cat > .crucible/p/items/alpha/ITEM.md <<'EOF'
# alpha — attempt ledger

## Goal

Record every dispatch and result immutably.

## Non-goals

No task DAG in this slice.

## Risk

MEDIUM

## Owned files

- crucible
- scripts/verify-attempt-ledger.sh
- tracked.txt

## Acceptance criteria

- [ ] A1: duplicate live attempts refuse.
- [ ] A2: observed lifecycle is durable.
- [ ] A3: results are immutable and evidence-bound.

## Focused falsifier

scripts/verify-attempt-ledger.sh

## Expensive evidence

NONE

## Stop conditions

Stop if item-file behavior changes.
EOF
    .crucible/p/crucible ready alpha >/dev/null
    .crucible/p/crucible phase alpha BUILD >/dev/null
    git checkout -qb ai/alpha
  )
  printf '%s' "$repo"
}

repo=$(fresh)
P="$repo/.crucible/p"
refuses 'dispatch criterion must exist in the item' 'not declared' "$P/crucible" dispatch alpha maker mk1 A9 FOCUSED
refuses 'invalid budget does not allocate an attempt' 'positive integer' env CRUCIBLE_MAKER_SECONDS=invalid \
  "$P/crucible" dispatch alpha maker mk1 A1 FOCUSED
[ -z "$(find "$P/attempts" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | head -1)" ] \
  && ok || bad 'refused preflight left an attempt directory'
mkdir "$P/.dispatch.lock"
refuses 'concurrent managed dispatch is serialized' 'already in progress' \
  "$P/crucible" dispatch alpha maker mk1 A1 FOCUSED
rmdir "$P/.dispatch.lock"
contract=$($P/crucible dispatch alpha maker mk1 A1 FOCUSED 2>/dev/null)
id=$(basename "$(dirname "$contract")")
[ -f "$contract" ] && ok || bad 'dispatch did not create an immutable contract before launch'
[ -f "$P/attempts/$id/meta.tsv" ] && [ -f "$P/attempts/$id/events.tsv" ] \
  && ok || bad 'dispatch did not create attempt metadata and event ledger'
[ "$(awk -F '\t' 'NR == 2 { print $10 }' "$P/attempts/$id/meta.tsv")" = DISPATCHED ] \
  && ok || bad 'attempt metadata does not bind the dispatch state'
grep -q "${tab}${id}${tab}" "$P/STATE.tsv" && ok || bad 'state does not name the in-flight attempt'
refuses 'duplicate live attempt' 'in-flight attempt\|live attempt' "$P/crucible" dispatch alpha maker mk1 A1 FOCUSED
refuses 'finish requires an observed start' 'requires RUNNING or OVERDUE' "$P/crucible" attempt finish "$id" RETURNED impossible-return
refuses 'attempt ids cannot traverse paths' 'invalid attempt id' "$P/crucible" attempt start 'A1../../outside' 1
refuses 'phase cannot erase an in-flight attempt' 'still in flight' "$P/crucible" phase alpha REVIEW
expect 'next waits before launch observation' "^WAIT $id " "$P/crucible" next

meta_before=$(cksum "$P/attempts/$id/meta.tsv")
bind_independence "$P" "$id"
expect 'start records observed PID' "$id RUNNING pid" "$P/crucible" attempt start "$id" "$$"
refuses 'overdue cannot be inferred early' 'has not passed' "$P/crucible" attempt overdue "$id"
expect 'next waits on running process' "^WAIT $id " "$P/crucible" next
(
  cd "$repo"
  printf 'implemented\n' >> tracked.txt
  git add tracked.txt
  git -c user.name=test -c user.email=test@example.invalid commit -qm implementation
)
eout=$(cd "$repo" && .crucible/p/crucible run alpha mk1 -- sh -c 'echo maker-focused')
evidence=$(basename "$(printf '%s' "$eout" | awk '{print $1}')")
expect 'returned is an observed terminal event' 'RETURNED; record its result next' "$P/crucible" attempt finish "$id" RETURNED observed-exit-zero
expect 'next requests a result after return' '^NEXT alpha RESULT ' "$P/crucible" next
refuses 'result requires existing evidence' 'missing regular evidence' "$P/crucible" result "$id" PASS missing.txt CLOSE -
refuses 'result outcome and next action must agree' 'incompatible result' "$P/crucible" result "$id" PASS "$evidence" FIX -
result=$($P/crucible result "$id" PASS "$evidence" CLOSE -)
[ -f "$result" ] && ok || bad 'result was not written'
grep -q '^OUTCOME: PASS$' "$result" && grep -q "^ATTEMPT-ID: $id$" "$result" \
  && ok || bad 'result headers are not bound to the attempt'
grep -q "^DISPATCH-WORK-ID: $(awk -F '\t' 'NR == 2 { print $4 }' "$P/attempts/$id/meta.tsv")$" "$result" \
  && ok || bad 'maker result does not preserve its dispatch input work id'
[ "$meta_before" = "$(cksum "$P/attempts/$id/meta.tsv")" ] && ok || bad 'attempt metadata changed after lifecycle events'
refuses 'result is immutable' 'immutable' "$P/crucible" result "$id" PASS "$evidence" CLOSE -
grep -q "${tab}-${tab}-${tab}" "$P/STATE.tsv" && ok || bad 'result did not clear the in-flight attempt'
awk -F '\t' -v OFS='\t' -v a="$id" 'NR == 1 { print; next } { $6=a; print }' \
  "$P/STATE.tsv" > "$P/STATE.reconcile"
mv "$P/STATE.reconcile" "$P/STATE.tsv"
expect 'matching immutable result repairs interrupted state publication' 'reconciled publication' \
  "$P/crucible" result "$id" PASS "$evidence" CLOSE -
grep -q "${tab}-${tab}-${tab}" "$P/STATE.tsv" && ok || bad 'result reconciliation did not clear the in-flight attempt'
refuses 'current-work pass refuses duplicate dispatch' 'current-work PASS' "$P/crucible" dispatch alpha maker mk1 A1 FOCUSED

$P/crucible phase alpha REVIEW >/dev/null
jcontract=$($P/crucible dispatch alpha judge j1 A1 FOCUSED 2>/dev/null)
jid=$(basename "$(dirname "$jcontract")")
grep -q '^## Work — work id ' "$jcontract" && grep -q '^## Recorded evidence$' "$jcontract" \
  && ok || bad 'judge contract does not contain the work and recorded evidence'
grep -q 'Relation to recorded maker families: `SAME-FAMILY`' "$jcontract" \
  && ok || bad 'same-family review is not labelled in the judge contract'
bind_independence "$P" "$jid"
$P/crucible attempt start "$jid" "$$" >/dev/null
jeout=$(cd "$repo" && .crucible/p/crucible run alpha j1 -- sh -c 'echo judge-focused')
jevidence=$(basename "$(printf '%s' "$jeout" | awk '{print $1}')")
$P/crucible attempt finish "$jid" RETURNED observed-exit-zero >/dev/null
$P/crucible result "$jid" PASS "$jevidence" CLOSE - >/dev/null
grep -q '^REVIEW-RELATION: SAME-FAMILY$' "$P/attempts/$jid/result.md" \
  && ok || bad 'same-family correlation risk is missing from the result'
grep -q '^VERDICT: PASS$' "$P/items/alpha/verdicts/j1.md" && ok || bad 'judge result did not publish the compatibility verdict'
refuses 'judge current-work pass refuses duplicate' 'current-work PASS' "$P/crucible" dispatch alpha judge j1 A1 FOCUSED

fcontract=$($P/crucible dispatch alpha judge j2 A2 FULL_SUITE 2>/dev/null)
fid=$(basename "$(dirname "$fcontract")")
bind_independence "$P" "$fid"
$P/crucible attempt start "$fid" "$$" >/dev/null
feout=$(cd "$repo" && .crucible/p/crucible run alpha j2 -- sh -c 'echo canonical-suite')
fevidence=$(basename "$(printf '%s' "$feout" | awk '{print $1}')")
$P/crucible attempt finish "$fid" RETURNED observed-exit-zero >/dev/null
$P/crucible result "$fid" PASS "$fevidence" CLOSE - >/dev/null
grep -q '^REVIEW-RELATION: CROSS-FAMILY$' "$P/attempts/$fid/result.md" \
  && ok || bad 'cross-family review is not labelled in the result'
refuses 'canonical expensive pass refuses duplicate' 'canonical FULL_SUITE PASS' "$P/crucible" dispatch alpha judge j1 A3 FULL_SUITE

r1contract=$($P/crucible dispatch alpha judge j1 A3 FOCUSED 2>/dev/null)
r1=$(basename "$(dirname "$r1contract")")
bind_independence "$P" "$r1"
$P/crucible attempt start "$r1" "$$" >/dev/null
r1out=$(cd "$repo" && .crucible/p/crucible run alpha j1 -- sh -c 'echo finding-one')
r1e=$(basename "$(printf '%s' "$r1out" | awk '{print $1}')")
$P/crucible attempt finish "$r1" RETURNED observed-reject >/dev/null
refuses 'evidence cannot be reused across attempts' 'attempt id does not match' \
  "$P/crucible" result "$r1" REJECT "$jevidence" FIX abcdef123456
$P/crucible result "$r1" REJECT "$r1e" FIX abcdef123456 >/dev/null
r2contract=$($P/crucible dispatch alpha judge j2 A3 FOCUSED 2>/dev/null)
r2=$(basename "$(dirname "$r2contract")")
bind_independence "$P" "$r2"
$P/crucible attempt start "$r2" "$$" >/dev/null
r2out=$(cd "$repo" && .crucible/p/crucible run alpha j2 -- sh -c 'echo finding-two')
r2e=$(basename "$(printf '%s' "$r2out" | awk '{print $1}')")
$P/crucible attempt finish "$r2" RETURNED observed-reject >/dev/null
$P/crucible result "$r2" REJECT "$r2e" FIX abcdef123456 >/dev/null
grep -q "^alpha${tab}BLOCKED${tab}REVIEW${tab}.*${tab}REPEATED_FINDING${tab}" "$P/STATE.tsv" \
  && ok || bad 'repeated finding did not block the item'
expect 'next exposes repeated finding block' '^BLOCKED alpha REPEATED_FINDING ' "$P/crucible" next

timeout_repo=$(fresh)
TP="$timeout_repo/.crucible/p"
tcontract=$(CRUCIBLE_MAKER_SECONDS=30 $TP/crucible dispatch alpha maker mk1 A1 FOCUSED 2>/dev/null)
tid=$(basename "$(dirname "$tcontract")")
# non-guided timeout path: no independence seal required before start
$TP/crucible attempt start "$tid" "$$" >/dev/null
refuses 'deadline still cannot infer timeout' 'has not passed' "$TP/crucible" attempt overdue "$tid"
deadline=$(awk -F '\t' 'NR == 2 { print $12 }' "$TP/attempts/$tid/meta.tsv")
# Force the deadline into the past without waiting wall-clock 30s.
awk -F '\t' -v OFS='\t' -v now="$(date +%s)" 'NR==1{print; next} {$12=now-1; print}' \
  "$TP/attempts/$tid/meta.tsv" > "$TP/attempts/$tid/meta.tsv.tmp"
mv "$TP/attempts/$tid/meta.tsv.tmp" "$TP/attempts/$tid/meta.tsv"
expect 'deadline crossing records overdue only' 'OVERDUE; process outcome is still unobserved' "$TP/crucible" attempt overdue "$tid"
expect 'next exposes overdue process' '^BLOCKED alpha OVERDUE_PROCESS ' "$TP/crucible" next
expect 'first observed timeout permits one retry' 'one retry is available' "$TP/crucible" attempt finish "$tid" TIMEOUT launcher-observed-timeout
expect 'next prints the exact bounded retry' "^NEXT alpha RETRY .* $tid$" "$TP/crucible" next
refuses 'retry must preserve the complete dispatch key' 'retry key does not match' \
  "$TP/crucible" dispatch alpha maker mk1 A1 FULL_SUITE "$tid"
retry_contract=$(CRUCIBLE_MAKER_SECONDS=1 $TP/crucible dispatch alpha maker mk1 A1 FOCUSED "$tid" 2>&1) \
  || { bad "one matching retry was refused: $retry_contract"; printf '%s passed, %s failed\n' "$PASS" "$FAIL"; exit 1; }
retry=$(basename "$(dirname "$retry_contract")")
$TP/crucible attempt start "$retry" "$$" >/dev/null
expect 'second observed timeout exhausts retry' 'item blocked RETRY_EXHAUSTED' "$TP/crucible" attempt finish "$retry" TIMEOUT launcher-observed-timeout
grep -q "^alpha${tab}BLOCKED${tab}BUILD${tab}.*${tab}RETRY_EXHAUSTED${tab}" "$TP/STATE.tsv" \
  && ok || bad 'retry exhaustion did not persist a typed block'
expect 'next exposes retry exhaustion' '^BLOCKED alpha RETRY_EXHAUSTED ' "$TP/crucible" next
refuses 'retry of a retry refuses' 'only one infrastructure retry' "$TP/crucible" dispatch alpha maker mk1 A1 FOCUSED "$retry"

decision_repo=$(fresh)
DP="$decision_repo/.crucible/p"
dcontract=$($DP/crucible dispatch alpha maker mk1 A1 FOCUSED 2>/dev/null)
did=$(basename "$(dirname "$dcontract")")
$DP/crucible attempt start "$did" "$$" >/dev/null
(
  cd "$decision_repo"
  printf 'needs context\n' >> tracked.txt
  git add tracked.txt
  git -c user.name=test -c user.email=test@example.invalid commit -qm needs-context
)
dout=$(cd "$decision_repo" && .crucible/p/crucible run alpha mk1 -- sh -c 'echo missing operator decision')
devidence=$(basename "$(printf '%s' "$dout" | awk '{print $1}')")
$DP/crucible attempt finish "$did" RETURNED observed-needs-context >/dev/null
$DP/crucible result "$did" NEEDS_CONTEXT "$devidence" DECIDE - >/dev/null
grep -q "^alpha${tab}BLOCKED${tab}BUILD${tab}.*${tab}NEEDS_CONTEXT${tab}" "$DP/STATE.tsv" \
  && ok || bad 'NEEDS_CONTEXT did not persist a typed block'
expect 'next exposes the returned decision block' '^BLOCKED alpha NEEDS_CONTEXT ' "$DP/crucible" next

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
