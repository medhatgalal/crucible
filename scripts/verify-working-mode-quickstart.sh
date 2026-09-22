#!/bin/sh
# Extra proof of docs/working-mode.md Quickstart Example A (copy-paste LOW
# fixture) and unsigned HIGH → STOP-ASK MAP-HUMAN (Example B). Empty HOME.
# Adopt from this tree ($HERE/crucible). Fixture echo workers only.
# Not a required CI gate. Do not add to .github/workflows/selftest.yml.
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }

EX="$HERE/docs/examples/working-mode"

if [ ! -f "$EX/MAP.md" ]; then
  printf 'RED docs/examples/working-mode/MAP.md missing\n' >&2
  exit 1
fi

sh -n "$HERE/scripts/verify-working-mode-quickstart.sh" && ok \
  || bad 'verify-working-mode-quickstart.sh is not valid POSIX sh'
sh -n "$HERE/wm.sh" && ok || bad 'wm.sh is not valid POSIX sh'
sh -n "$HERE/crucible" && ok || bad 'crucible is not valid POSIX sh'
sh -n "$EX/tools/maker.sh" && ok || bad 'example maker.sh is not valid POSIX sh'
sh -n "$EX/tools/reviewer.sh" && ok || bad 'example reviewer.sh is not valid POSIX sh'

if grep -E -q 'mark_slice_closed\(\)|reset_brick\(\)' \
  "$HERE/scripts/verify-working-mode-quickstart.sh"; then
  bad 'quickstart script must not define mark_slice_closed or reset_brick'
else
  ok
fi

if grep -E -q "pgrep[[:space:]]+-f[[:space:]]+['\"]wm\\.sh loop['\"]" \
  "$HERE/scripts/verify-working-mode-quickstart.sh"; then
  bad 'pgrep leftover-loop is unscoped (must match this $WM)'
else
  ok
fi

command -v git >/dev/null 2>&1 && ok || bad 'git required'

for rel in IDEA.md SPEC.md architecture/modules.md MAP.md \
  tools/maker.sh tools/reviewer.sh README.md product/.gitkeep; do
  if [ -e "$EX/$rel" ]; then
    ok
  else
    bad "example missing $rel"
  fi
done

[ -x "$EX/tools/maker.sh" ] && ok || bad 'example maker.sh is not executable'
[ -x "$EX/tools/reviewer.sh" ] && ok || bad 'example reviewer.sh is not executable'

if grep -q '^s1	product	product/hello.txt	-	LOW$' "$EX/MAP.md" \
  && grep -q '^MAPPER: alice$' "$EX/MAP.md"; then
  ok
else
  bad 'example MAP.md wanted MAPPER alice and s1 product LOW'
fi

if grep -q 'product/hello.txt' "$EX/SPEC.md" \
  && grep -q '^MAKER-WRITES$' "$EX/SPEC.md"; then
  ok
else
  bad 'example SPEC.md must own product/hello.txt with MAKER-WRITES'
fi

if grep -E '^wm[[:space:]]' "$EX/README.md" >/dev/null; then
  bad 'example README.md must not show bare wm commands'
else
  ok
fi
if grep -E 'Desktop|BEGIN [A-Z]+ PRIVATE|sk-[A-Za-z0-9]{10,}' "$EX/README.md" \
  >/dev/null; then
  bad 'example README.md contains Desktop path or secrets material'
else
  ok
fi
if grep -q 'docs/working-mode.md' "$EX/README.md" \
  && grep -q 'verify-working-mode-quickstart.sh' "$EX/README.md"; then
  ok
else
  bad 'example README.md must point at working-mode Quickstart and this script'
fi

if grep -q './tools/maker.sh' "$EX/tools/maker.sh" \
  || grep -q 'WORD: PASS' "$EX/tools/maker.sh"; then
  bad 'example maker.sh must stay distinct from reviewer'
else
  ok
fi

BASE=$(mktemp -d "${TMPDIR:-/tmp}/wm-quickstart.XXXXXX")
EMPTY_HOME=$(mktemp -d "${TMPDIR:-/tmp}/wm-quickstart-home.XXXXXX")
OUT="$BASE/out.txt"
ERR="$BASE/err.txt"
HOME="$EMPTY_HOME"
export HOME
GIT_CONFIG_GLOBAL=/dev/null
GIT_CONFIG_SYSTEM=/dev/null
export GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM

trap 'rm -rf "$BASE" "$EMPTY_HOME"' 0
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 129' 1
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 130' 2
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 143' 15

assert_home_empty() {
  leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
  if [ -z "$leftovers" ]; then
    ok
  else
    bad "wrote under HOME: $leftovers"
  fi
}

assert_no_home_skills() {
  hits=$(find "$EMPTY_HOME" \( -name SKILL.md -o -name CONTRACT.md \) -print 2>/dev/null || true)
  if [ -n "$hits" ]; then
    bad "skill files under HOME: $hits"
    return
  fi
  hits=$(find "$EMPTY_HOME" \( -path '*/.grok/skills/*' -o -path '*/.claude/skills/*' \
    -o -path '*/.agents/skills/*' -o -path '*/.kiro/skills/*' \
    -o -path '*/.codex/skills/*' \) -print 2>/dev/null || true)
  if [ -n "$hits" ]; then
    bad "harness skill trees under HOME: $hits"
  else
    ok
  fi
}

init_git_repo() {
  dir=$1
  mkdir -p "$dir"
  (
    CDPATH=
    cd "$dir"
    git init -q
    git checkout -B main >/dev/null 2>&1 || git checkout -b main >/dev/null 2>&1 || true
    git config user.email 'wm@local'
    git config user.name 'working-mode'
    printf 'tracked\n' > README
    git add README
    git -c user.email=wm@local -c user.name=working-mode commit -qm baseline
  )
}

run_adopt() {
  dir=$1
  shift
  ( CDPATH=; cd "$dir" && "$@" >"$OUT" 2>"$ERR" )
}

prepare_example() {
  dir=$1
  risk=${2:-LOW}
  rkind=${3:-grok}
  init_git_repo "$dir"
  if run_adopt "$dir" "$HERE/crucible" adopt work --managed --working-mode; then
    ok
  else
    bad "adopt --working-mode refused: $(cat "$OUT") $(cat "$ERR")"
    return 1
  fi
  cp -R "$EX/." "$dir/"
  chmod +x "$dir/tools/maker.sh" "$dir/tools/reviewer.sh"
  mkdir -p "$dir/product"
  if [ "$risk" = HIGH ]; then
    {
      printf 'MAPPER: alice\n\n'
      printf 'id\tmodule\towned_paths\tdepends_on\trisk\n'
      printf 's1\tproduct\tproduct/hello.txt\t-\tHIGH\n'
    } > "$dir/MAP.md"
  fi
  WM="$dir/.crucible/work/wm.sh"
  if [ ! -f "$WM" ]; then
    bad 'adopt missing .crucible/work/wm.sh'
    return 1
  fi
  chmod +x "$WM" 2>/dev/null || true
  if ! (
    CDPATH=
    cd "$dir"
    export WM_ENGINE="$WM"
    "$WM" init >"$OUT" 2>"$ERR" || exit 1
    "$WM" cast coordinator parent grok - >/dev/null 2>"$ERR" || true
    "$WM" cast maker carol grok './tools/maker.sh {BRIEF}' >/dev/null 2>"$ERR" || exit 1
    "$WM" cast reviewer dave "$rkind" './tools/reviewer.sh {BRIEF}' >/dev/null 2>"$ERR" || exit 1
    "$WM" record-mapper --from MAP.md >/dev/null 2>"$ERR" || exit 1
    "$WM" map-ready >/dev/null 2>"$ERR" || exit 1
    mkdir -p .wm/return
    printf 'WORD: MAP-ACCEPT\nAGENT: bob\nMAP: MAP.md\n' > .wm/return/bob.md
    "$WM" map-verdict .wm/return/bob.md >/dev/null 2>"$ERR" || exit 1
    git add -A
    git -c user.email=wm@local -c user.name=working-mode \
      commit -qm 'quickstart example map+spec+workers' || true
  ); then
    bad "prepare wm steps refused: $(cat "$OUT" 2>/dev/null || true) $(cat "$ERR" 2>/dev/null || true)"
    return 1
  fi
}

# --- Example A: LOW copy-paste walk ---
AD="$BASE/example-a"
if prepare_example "$AD" LOW grok; then
  ok
else
  bad 'Example A prepare failed'
fi
WM="$AD/.crucible/work/wm.sh"
export WM_ENGINE="$WM"
CDPATH=
cd "$AD"

mapper=$(awk -F ': ' '$1=="id"{print $2; exit}' .wm/mapper)
[ "$mapper" = alice ] && ok || bad "mapper id wanted alice, got $mapper"
[ "$mapper" != carol ] && ok || bad 'mapper is maker (alice==carol)'
[ "$mapper" != dave ] && ok || bad 'mapper is reviewer'

set +e
"$WM" loop >"$OUT" 2>"$ERR"
LOOP_RC=$?
set -e
if pgrep -f "$WM loop" >/dev/null 2>&1; then
  bad "leftover wm.sh loop process"
else
  ok
fi
if [ "$LOOP_RC" -eq 0 ] && grep -q 'CLOSED PASS' "$OUT" \
  && [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  ok
else
  bad "Example A wm loop wanted CLOSED PASS rc=0, got rc=$LOOP_RC out=$(cat "$OUT") err=$(cat "$ERR")"
fi
[ -f .wm/reviewer-ran ] && ok || bad 'Example A reviewer CLI not execd'
if [ -f product/hello.txt ] && grep -qx hello product/hello.txt \
  && [ "$(cat product/hello.txt)" = hello ]; then
  ok
else
  bad "Example A product/hello.txt wanted hello, got $(cat product/hello.txt 2>/dev/null || echo ABSENT)"
fi
if [ -f .wm/FALSIFIER ] && grep -qx 'grep -qx hello product/hello.txt' .wm/FALSIFIER; then
  ok
else
  bad 'Example A FALSIFIER wanted grep -qx hello product/hello.txt'
fi
closed_rows=$(awk -F '\t' 'NR>1 && $6=="CLOSED" {c++} END{print c+0}' slices.tsv)
[ "$closed_rows" -eq 1 ] && ok || bad "Example A slices.tsv CLOSED rows wanted 1, got $closed_rows"

cd "$HERE"
assert_home_empty
assert_no_home_skills

# --- Example B: unsigned HIGH → STOP-ASK MAP-HUMAN ---
BD="$BASE/example-b"
if prepare_example "$BD" HIGH claude; then
  ok
else
  bad 'Example B prepare failed'
fi
WM="$BD/.crucible/work/wm.sh"
export WM_ENGINE="$WM"
cd "$BD"
rm -f MAP-HUMAN product/hello.txt .wm/FALSIFIER
set +e
"$WM" loop >"$OUT" 2>"$ERR"
LOOP_RC=$?
set -e
if pgrep -f "$WM loop" >/dev/null 2>&1; then
  bad "Example B leftover wm.sh loop process"
else
  ok
fi
if [ "$LOOP_RC" -ne 0 ] && grep -q 'STOP-ASK MAP-HUMAN' "$OUT"; then
  ok
else
  bad "Example B unsigned HIGH wanted STOP-ASK MAP-HUMAN rc!=0, got rc=$LOOP_RC out=$(cat "$OUT") err=$(cat "$ERR")"
fi
if [ -f .wm/FALSIFIER ]; then
  bad 'Example B maker must not run (FALSIFIER present)'
else
  ok
fi
if [ -f product/hello.txt ]; then
  bad 'Example B must not write product/hello.txt'
else
  ok
fi
if [ -f MAP-HUMAN ]; then
  bad 'Example B must not auto-write MAP-HUMAN'
else
  ok
fi

cd "$HERE"
assert_home_empty
assert_no_home_skills

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
