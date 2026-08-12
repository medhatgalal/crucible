#!/bin/sh
# Focused contract test for frozen managed task DAGs.

set -eu

HERE=$(cd "$(dirname "$0")/.." && pwd)
C="$HERE/crucible"
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

fresh() {
  base=$(mktemp -d); repo="$base/repo"; mkdir -p "$repo"
  (
    cd "$repo"
    git init -q -b main
    printf 'baseline\n' > tracked.txt
    git add tracked.txt
    git -c user.name=test -c user.email=test@example.invalid commit -qm baseline
    "$C" adopt p >/dev/null
    .crucible/p/crucible lifecycle enable --apply >/dev/null
    .crucible/p/crucible add alpha 'bounded parallel tasks' >/dev/null
    cat > .crucible/p/items/alpha/ITEM.md <<'EOF'
# alpha — bounded parallel tasks

## Goal

Freeze and expose a safe task dependency graph.

## Non-goals

No automatic agent launch or conflict resolution.

## Risk

MEDIUM

## Owned files

- src/one
- src/two
- src/three
- src/four
- src/five

## Acceptance criteria

- [ ] A1: malformed task graphs refuse before BUILD.
- [ ] A2: only dependency-ready tasks are exposed.

## Focused falsifier

scripts/verify-task-dag.sh

## Expensive evidence

NONE

## Stop conditions

Stop if task dispatch can bypass the frozen graph.
EOF
  )
  printf '%s' "$repo"
}

valid_dag() {
  repo=$1; d="$repo/.crucible/p/items/alpha"; mkdir -p "$d/tasks"
  cat > "$d/TASKS.tsv" <<'EOF'
task_id	depends_on	paths_file	verify_script
T1	-	tasks/T1.paths	tasks/T1.verify.sh
T2	T1	tasks/T2.paths	tasks/T2.verify.sh
T3	T1	tasks/T3.paths	tasks/T3.verify.sh
EOF
  printf 'src/one\n' > "$d/tasks/T1.paths"
  printf 'src/two\n' > "$d/tasks/T2.paths"
  printf 'src/three\n' > "$d/tasks/T3.paths"
  for task in T1 T2 T3; do
    cat > "$d/tasks/$task.verify.sh" <<'EOF'
#!/bin/sh
test -n "$1"
EOF
    chmod +x "$d/tasks/$task.verify.sh"
  done
}

execution_dag() {
  repo=$1; valid_dag "$repo"; d="$repo/.crucible/p/items/alpha"
  printf 'T4\t-\ttasks/T4.paths\ttasks/T4.verify.sh\n' >> "$d/TASKS.tsv"
  printf 'T5\t-\ttasks/T5.paths\ttasks/T5.verify.sh\n' >> "$d/TASKS.tsv"
  printf 'src/four\n' > "$d/tasks/T4.paths"
  printf 'src/five\n' > "$d/tasks/T5.paths"
  for task in T4 T5; do
    cat > "$d/tasks/$task.verify.sh" <<'EOF'
#!/bin/sh
test -n "$1"
EOF
    chmod +x "$d/tasks/$task.verify.sh"
  done
}

attempt_id_from_contract() { basename "$(dirname "$1")"; }

complete_task() {
  program=$1; attempt=$2; agent=$3; changed_path=$4
  worktree=$(awk -F '\t' 'NR == 2 { print $3 }' "$program/attempts/$attempt/task.tsv")
  mkdir -p "$worktree/$(dirname "$changed_path")"
  printf '%s\n' "$attempt" > "$worktree/$changed_path"
  git -C "$worktree" add "$changed_path"
  git -C "$worktree" -c user.name=test -c user.email=test@example.invalid commit -qm "$attempt"
  output=$(cd "$worktree" && "$program/crucible" run alpha "$agent" -- sh -c 'echo task-focused')
  evidence=$(basename "$(printf '%s' "$output" | awk '{print $1}')")
  "$program/crucible" attempt finish "$attempt" RETURNED observed-exit-zero >/dev/null
  bind_independence "$program" "$attempt"
  "$program/crucible" result "$attempt" PASS "$evidence" CLOSE - >/dev/null
}

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
expect 'ready freezes a valid task DAG' 'alpha is now READY' "$P/crucible" ready alpha
[ -f "$P/items/alpha/TASKS.id" ] && [ -f "$P/items/alpha/TASKS.md" ] \
  && ok || bad 'ready did not publish frozen task artifacts'
expect 'task list renders the generated graph' '^# TASKS — alpha' "$P/crucible" task list alpha
ready=$($P/crucible task ready alpha)
[ "$ready" = 'READY T1' ] && ok || bad "dependency-ready view was not exactly T1: $ready"
expect 'BUILD accepts the frozen task graph' 'alpha is now in BUILD' "$P/crucible" phase alpha BUILD
expect 'next points to the task-ready view' '^NEXT alpha TASK_READY ' "$P/crucible" next
refuses 'item-wide maker dispatch cannot bypass task ownership' 'dispatch through' \
  "$P/crucible" dispatch alpha maker mk1 A1 FOCUSED
printf '# changed after freeze\n' >> "$P/items/alpha/tasks/T1.verify.sh"
refuses 'frozen task graph drift refuses' 'changed after READY' "$P/crucible" task list alpha

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
sed 's/^T2\tT1\t/T2\tT9\t/' "$P/items/alpha/TASKS.tsv" > "$P/items/alpha/TASKS.new"
mv "$P/items/alpha/TASKS.new" "$P/items/alpha/TASKS.tsv"
refuses 'unknown dependency refuses' 'unknown dependency T9' "$P/crucible" ready alpha

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
sed -e 's/^T1\t-\t/T1\tT2\t/' -e 's/^T3\tT1\t/T3\t-\t/' \
  "$P/items/alpha/TASKS.tsv" > "$P/items/alpha/TASKS.new"
mv "$P/items/alpha/TASKS.new" "$P/items/alpha/TASKS.tsv"
refuses 'dependency cycle refuses' 'dependency cycle' "$P/crucible" ready alpha

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
sed 's/^T3\t/T2\t/' "$P/items/alpha/TASKS.tsv" > "$P/items/alpha/TASKS.new"
mv "$P/items/alpha/TASKS.new" "$P/items/alpha/TASKS.tsv"
refuses 'duplicate task id refuses' 'duplicate task id T2' "$P/crucible" ready alpha

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
printf 'src/one/child\n' > "$P/items/alpha/tasks/T2.paths"
refuses 'prefix ownership overlap refuses' 'ownership overlap' "$P/crucible" ready alpha

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
printf 'src/*\n' > "$P/items/alpha/tasks/T2.paths"
refuses 'glob ownership refuses' 'unsafe owned path' "$P/crucible" ready alpha

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
printf 'src/six\n' > "$P/items/alpha/tasks/T2.paths"
refuses 'task ownership outside the item contract refuses' 'undeclared item path' "$P/crucible" ready alpha

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
chmod -x "$P/items/alpha/tasks/T2.verify.sh"
refuses 'non-executable task verifier refuses' 'is not executable' "$P/crucible" ready alpha

repo=$(fresh); P="$repo/.crucible/p"; execution_dag "$repo"
$P/crucible ready alpha >/dev/null
$P/crucible phase alpha BUILD >/dev/null
t1_contract=$($P/crucible task dispatch alpha T1 mk1 A1 FOCUSED 2>/dev/null)
t4_contract=$($P/crucible task dispatch alpha T4 j1 A1 FOCUSED 2>/dev/null)
t5_contract=$($P/crucible task dispatch alpha T5 j2 A1 FOCUSED 2>/dev/null)
t1=$(attempt_id_from_contract "$t1_contract")
t4=$(attempt_id_from_contract "$t4_contract")
t5=$(attempt_id_from_contract "$t5_contract")
for attempt in "$t1" "$t4" "$t5"; do "$P/crucible" attempt start "$attempt" "$$" >/dev/null; done
[ "$(awk -F '\t' 'NR == 2 { print $3 }' "$P/attempts/$t1/meta.tsv")" = T1 ] \
  && [ -d "$(awk -F '\t' 'NR == 2 { print $3 }' "$P/attempts/$t1/task.tsv")" ] \
  && ok || bad 'task dispatch did not bind an isolated worktree'
complete_task "$P" "$t1" mk1 src/one
ready=$($P/crucible task ready alpha)
printf '%s\n' "$ready" | grep -q '^READY T2$' && printf '%s\n' "$ready" | grep -q '^READY T3$' \
  && ok || bad "dependency PASS did not release T2 and T3: $ready"
t2_contract=$($P/crucible task dispatch alpha T2 mk1 A1 FOCUSED 2>/dev/null)
t2=$(attempt_id_from_contract "$t2_contract")
$P/crucible attempt start "$t2" "$$" >/dev/null
refuses 'default parallel cap refuses a fourth live maker' 'parallel maker limit reached: 3/3' \
  "$P/crucible" task dispatch alpha T3 mk1 A1 FOCUSED
complete_task "$P" "$t4" j1 src/four
complete_task "$P" "$t5" j2 src/five
complete_task "$P" "$t2" mk1 src/two
t3_contract=$($P/crucible task dispatch alpha T3 j1 A1 FOCUSED 2>/dev/null)
t3=$(attempt_id_from_contract "$t3_contract")
$P/crucible attempt start "$t3" "$$" >/dev/null
complete_task "$P" "$t3" j1 src/three
expect 'all passing tasks expose integration' '^READY INTEGRATE$' "$P/crucible" task ready alpha
expect 'next exposes the integration barrier' '^NEXT alpha INTEGRATE ' "$P/crucible" next
expect 'stable integration applies every passing task' '^integrated alpha at ' "$P/crucible" task integrate alpha
[ "$(awk 'END { print NR-1 }' "$P/items/alpha/INTEGRATION.tsv")" -eq 5 ] \
  && ok || bad 'integration did not record all five task results'
expect 'integrated task graph can enter review' 'alpha is now in REVIEW' "$P/crucible" phase alpha REVIEW
refuses 'any prior task maker is barred from integrated-work review' 'maker of alpha' \
  "$P/crucible" dispatch alpha judge mk1 A1 FOCUSED
refuses 'task evidence cannot close integrated work' 'no usable evidence' "$P/crucible" check alpha

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
