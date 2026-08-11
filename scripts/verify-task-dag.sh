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

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
expect 'ready freezes a valid task DAG' 'alpha is now READY' "$P/crucible" ready alpha
[ -f "$P/items/alpha/TASKS.id" ] && [ -f "$P/items/alpha/TASKS.md" ] \
  && ok || bad 'ready did not publish frozen task artifacts'
expect 'task list renders the generated graph' '^# TASKS — alpha' "$P/crucible" task list alpha
ready=$($P/crucible task ready alpha)
[ "$ready" = 'READY T1' ] && ok || bad "dependency-ready view was not exactly T1: $ready"
expect 'BUILD accepts the frozen task graph' 'alpha is now in BUILD' "$P/crucible" phase alpha BUILD
expect 'next points to the task-ready view' '^NEXT alpha TASK_READY ' "$P/crucible" next
refuses 'item-wide maker dispatch cannot bypass task ownership' 'task-bound maker dispatch' \
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
printf 'src/four\n' > "$P/items/alpha/tasks/T2.paths"
refuses 'task ownership outside the item contract refuses' 'undeclared item path' "$P/crucible" ready alpha

repo=$(fresh); P="$repo/.crucible/p"; valid_dag "$repo"
chmod -x "$P/items/alpha/tasks/T2.verify.sh"
refuses 'non-executable task verifier refuses' 'is not executable' "$P/crucible" ready alpha

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
