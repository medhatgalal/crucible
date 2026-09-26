#!/bin/sh
# Working-mode cut-over CHECKs. Wrapper + rust binary. Fixture dirs only.
# Falsifier-first: RED when wm.sh is missing or greps dumped sh kernel bodies.
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
WM="$HERE/wm.sh"
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }

if [ ! -f "$WM" ]; then
  printf 'RED wm.sh missing (working-mode wrapper not present)\n' >&2
  exit 1
fi
chmod +x "$WM" 2>/dev/null || true

EMPTY_HOME=$(mktemp -d "${TMPDIR:-/tmp}/wm-empty-home.XXXXXX")
BASE=$(mktemp -d "${TMPDIR:-/tmp}/wm-verify.XXXXXX")
OUT="$BASE/out.txt"
ERR="$BASE/err.txt"
HOME="$EMPTY_HOME"
export HOME
# shellcheck disable=SC1091
. "$HERE/scripts/cargo-env.sh"
GIT_CONFIG_GLOBAL=/dev/null
GIT_CONFIG_SYSTEM=/dev/null
export GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM
trap 'rm -rf "$BASE" "$EMPTY_HOME"' 0
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 129' 1
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 130' 2
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 143' 15

VERSION=$(sed -n '1p' "$HERE/VERSION")
[ "$VERSION" = 1.21.0 ] && ok || bad "VERSION wanted 1.21.0 got $VERSION"

sh -n "$WM" && ok || bad 'wm.sh is not valid POSIX sh'
if grep -q 'WM_WRAPPER' "$WM" && grep -q 'exec' "$WM" && grep -q 'crucible' "$WM"; then
  ok
else
  bad 'wm.sh must be an exec wrapper (WM_WRAPPER + exec bindir/crucible)'
fi
if grep -E -q '^cmd_loop\(|LOOP_BOUND|source wm-go|WM_GO=' "$WM"; then
  bad 'wm.sh must not contain dumped kernel function bodies'
else
  ok
fi

if [ ! -f "$HERE/Cargo.toml" ]; then
  printf 'RED Cargo.toml missing\n' >&2
  exit 1
fi
if ! ( CDPATH=; cd -- "$HERE" && cargo build --release --locked >/dev/null ); then
  bad 'cargo build --release failed'
  printf '%s passed, %s failed\n' "$PASS" "$FAIL"
  exit 1
else
  ok
fi
RUST="$HERE/target/release/crucible"
[ -x "$RUST" ] && ok || bad 'target/release/crucible missing'

STAGE="$BASE/stage"
mkdir -p "$STAGE"
cp "$WM" "$STAGE/wm.sh"
cp "$RUST" "$STAGE/crucible"
chmod +x "$STAGE/wm.sh" "$STAGE/crucible"
WM="$STAGE/wm.sh"
export WM_ENGINE="$WM"

ver=$("$STAGE/crucible" --version 2>"$ERR") || ver=
[ "$ver" = 1.21.0 ] && ok || bad "crucible --version wanted 1.21.0 got $ver"
ver=$("$STAGE/crucible" -V 2>"$ERR") || ver=
[ "$ver" = 1.21.0 ] && ok || bad "crucible -V wanted 1.21.0 got $ver"

# Wrapper execs the sibling binary (not POSIX ./crucible).
wrap=$("$WM" --version 2>"$ERR") || wrap=
[ "$wrap" = 1.21.0 ] && ok || bad "wm.sh --version (wrapper exec) wanted 1.21.0 got $wrap err=$(cat "$ERR")"

# go without IDEA.md is STOP-ASK INTAKE (rust). Do not use wm.sh go as the
# product walker except to prove the wrapper execs rust.
(
  CDPATH=
  cd "$BASE"
  mkdir -p intake
  cd intake
  git init -q
  git config user.email 'wm@local'
  git config user.name 'working-mode'
  printf 'tracked\n' > README
  git add README
  git commit -qm baseline
  set +e
  "$WM" go >"$OUT" 2>"$ERR"
  rc=$?
  set -e
  if [ "$rc" -ne 0 ] && grep -q 'STOP-ASK INTAKE' "$OUT"; then
    ok
  else
    bad "wrapper go without IDEA wanted STOP-ASK INTAKE rc!=0, got rc=$rc out=$(cat "$OUT") err=$(cat "$ERR")"
  fi
  if [ -f .wm/FLOOR.md ] && grep -q 'card: STOP-ASK INTAKE' .wm/FLOOR.md; then
    ok
  else
    bad 'wrapper go without IDEA must write FLOOR STOP-ASK INTAKE'
  fi
)

# Repo-root crucible is the thin exec, not the guided kernel.
if [ -f "$HERE/crucible" ]; then
  _sig=$(dd if="$HERE/crucible" bs=2 count=1 2>/dev/null || true)
  [ "$_sig" = '#!' ] && ok || bad 'repo-root ./crucible must stay #!/bin/sh'
else
  bad 'repo-root ./crucible missing'
fi
help_out=$("$HERE/target/release/crucible" help 2>"$ERR") || help_out=
printf '%s\n' "$help_out" | grep -q -- '--working-mode' && ok \
  || bad "release crucible help must print --working-mode, got $(printf '%s' "$help_out") err=$(cat "$ERR")"

# Task 3: START/BOOTSTRAP discoverability — one opt-in pointer; guided default
# stays adopt work --managed without --working-mode.
if grep -qi 'working-mode' "$HERE/START.md" \
  && grep -q 'docs/working-mode.md' "$HERE/START.md"; then
  ok
else
  bad 'START.md must point to docs/working-mode.md (opt-in)'
fi
if grep -qi 'working-mode' "$HERE/BOOTSTRAP.md" \
  && grep -q 'docs/working-mode.md' "$HERE/BOOTSTRAP.md"; then
  ok
else
  bad 'BOOTSTRAP.md must point to docs/working-mode.md (opt-in)'
fi
if grep -q 'adopt work --managed' "$HERE/BOOTSTRAP.md"; then
  ok
else
  bad 'BOOTSTRAP.md missing guided adopt work --managed'
fi
if grep -E 'adopt work --managed' "$HERE/BOOTSTRAP.md" | grep -q -- '--working-mode'; then
  bad 'BOOTSTRAP guided adopt must not add --working-mode'
else
  ok
fi
if grep -E 'adopt (work|NAME|<program>) --managed --working-mode' "$HERE/START.md" >/dev/null; then
  bad 'START.md must not show guided adopt with --working-mode'
else
  ok
fi
if grep -q '.crucible/<program>/wm.sh' "$HERE/docs/working-mode.md"; then
  ok
else
  bad 'docs/working-mode.md must invoke .crucible/<program>/wm.sh from the target root'
fi
if grep -E '^wm[[:space:]]' "$HERE/docs/working-mode.md" >/dev/null; then
  bad 'docs/working-mode.md must not show bare wm commands'
else
  ok
fi
[ -f "$HERE/WORKING-MODE.md" ] && ok || bad 'WORKING-MODE.md missing'
if [ -f "$HERE/WORKING-MODE.md" ]; then
  if grep -E '^wm[[:space:]]' "$HERE/WORKING-MODE.md" >/dev/null; then
    bad 'WORKING-MODE.md must not show bare wm commands'
  else
    ok
  fi
fi

if grep -q 'next READY' "$HERE/docs/working-mode.md" \
  && grep -qi 'next-slice' "$HERE/docs/working-mode.md"; then
  ok
else
  bad 'docs/working-mode.md must say MAP-HUMAN is next-slice / next READY scoped (8c)'
fi
if grep -q 'Quickstart' "$HERE/docs/working-mode.md" \
  && grep -F -q '.crucible/work/wm.sh' "$HERE/docs/working-mode.md" \
  && grep -F -q 'adopt work --managed --working-mode' "$HERE/docs/working-mode.md"; then
  ok
else
  bad 'docs/working-mode.md must have operator Quickstart using .crucible/work/wm.sh after adopt --working-mode'
fi
if grep -F -q 'HOME=$(mktemp -d) scripts/verify-working-mode.sh' \
  "$HERE/docs/working-mode.md"; then
  ok
else
  bad 'docs/working-mode.md must show empty-HOME kernel CHECKs from the source tree'
fi
if grep -F -q 'docs/examples/working-mode' "$HERE/docs/working-mode.md" \
  && grep -F -q 'CLOSED PASS' "$HERE/docs/working-mode.md"; then
  ok
else
  bad 'docs/working-mode.md must mention docs/examples/working-mode and CLOSED PASS'
fi
if [ -f "$HERE/docs/examples/working-mode/MAP.md" ]; then
  ok
else
  bad 'docs/examples/working-mode/MAP.md missing'
fi
if [ -f "$HERE/scripts/verify-working-mode-quickstart.sh" ] \
  && [ -x "$HERE/scripts/verify-working-mode-quickstart.sh" ]; then
  ok
else
  bad 'scripts/verify-working-mode-quickstart.sh missing or not executable'
fi
if grep -F 'docs/working-mode.md' "$HERE/README.md" | grep -q 'quickstart'; then
  ok
else
  bad 'README Go-deeper working-mode bullet must mention (quickstart)'
fi

if grep -q '## Send-back' "$HERE/skills/architecture/CONTRACT.md" \
  && grep -q '## Send-back' "$HERE/skills/critique/CONTRACT.md" \
  && grep -q '## Send-back' "$HERE/skills/review/CONTRACT.md"; then
  ok
else
  bad 'architecture/critique/review CONTRACT.md must have ## Send-back'
fi

# (A5) blank-home walk must not define mark_slice_closed or reset_brick
if grep -E -q 'mark_slice_closed\(\)|reset_brick\(\)' \
  "$HERE/scripts/verify-working-mode-blank-home.sh"; then
  bad 'A5: blank-home must not define mark_slice_closed or reset_brick'
else
  ok
fi

home_leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
if [ -z "$home_leftovers" ]; then
  ok
else
  bad "wrote under HOME: $home_leftovers"
fi

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
