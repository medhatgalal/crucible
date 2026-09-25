#!/bin/sh
# 13b: harness adapters + blank-HOME tarball adopt + 3-slice fixture walk.
# Falsifier-first: RED when adapters/docs/VERSION 1.18.0 pin are absent.
# Fixture echo/python stubs only. Live grok/kiro-cli/codex are not invoked.
# If none of those CLIs are on PATH: record INDEPENDENCE_UNAVAILABLE
# (not a fixture fail). Claude Code is not required.
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }

# --- load-bearing RED (13b pin) ---
if [ ! -f "$HERE/adapters/grok.md" ] \
  || [ ! -f "$HERE/adapters/kiro.md" ] \
  || [ ! -f "$HERE/adapters/claude.md" ] \
  || [ ! -f "$HERE/adapters/codex.md" ]; then
  printf 'RED adapters/{grok,kiro,claude,codex}.md missing (13b)\n' >&2
  exit 1
fi

VERSION=$(sed -n '1p' "$HERE/VERSION")
if [ "$VERSION" != 1.18.0 ]; then
  printf 'RED VERSION is %s, want 1.18.0\n' "$VERSION" >&2
  exit 1
fi

if ! grep -q '^## \[1.17.0\] - 2026-09-21$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.17.0] - 2026-09-21\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.16.4\] - 2026-09-19$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.16.4] - 2026-09-19\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.16.3\] - 2026-09-19$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.16.3] - 2026-09-19\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.16.2\] - 2026-09-18$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.16.2] - 2026-09-18\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.16.1\] - 2026-09-18$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.16.1] - 2026-09-18\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.16.0\] - 2026-09-18$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.16.0] - 2026-09-18\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.15.2\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.15.2] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.15.1\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.15.1] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.15.0\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.15.0] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.14.2\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.14.2] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.14.1\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.14.1] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.14.0\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.14.0] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.13.0\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.13.0] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.12.0\] - 2026-09-15$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.12.0] - 2026-09-15\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.11.0\] - 2026-09-15$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.11.0] - 2026-09-15\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.10.1\] - 2026-09-15$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.10.1] - 2026-09-15\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.10.0\] - 2026-09-15$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.10.0] - 2026-09-15\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.9.0\] - 2026-09-13$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.9.0] - 2026-09-13\n' >&2
  exit 1
fi

if [ ! -f "$HERE/docs/whats-new.md" ] \
  || ! grep -qi 'working-mode' "$HERE/docs/whats-new.md"; then
  printf 'RED docs/whats-new.md does not mention working-mode\n' >&2
  exit 1
fi

sh -n "$HERE/scripts/verify-working-mode-blank-home.sh" && ok \
  || bad 'verify-working-mode-blank-home.sh is not valid POSIX sh'
sh -n "$HERE/wm.sh" && ok || bad 'wm.sh is not valid POSIX sh'
sh -n "$HERE/crucible" && ok || bad 'crucible is not valid POSIX sh'

# Adapters: how to invoke; point agents.tsv at CLIs; no secrets.
for name in grok kiro claude codex; do
  f="$HERE/adapters/$name.md"
  [ -s "$f" ] && ok || bad "adapters/$name.md empty"
  if grep -q 'agents.tsv' "$f"; then
    ok
  else
    bad "adapters/$name.md must point agents.tsv at the CLI"
  fi
  if grep -Ei 'api[_-]?key[[:space:]]*[=:][[:space:]]*[^[:space:]]|secret[[:space:]]*[=:][[:space:]]*[^[:space:]]|BEGIN [A-Z]+ PRIVATE|sk-[A-Za-z0-9]{10,}' "$f" >/dev/null; then
    bad "adapters/$name.md contains secrets material"
  else
    ok
  fi
done

# Four batteries only; adapters are not batteries.
extra=
for d in "$HERE/skills"/*; do
  [ -d "$d" ] || continue
  n=${d##*/}
  case $n in
    architecture|critique|review|loop-design|working-mode|research|repo-scout|crucible) ;;
    *) extra="$extra $n" ;;
  esac
done
if [ -z "$extra" ]; then
  ok
else
  bad "unexpected package battery:$extra"
fi

command -v git >/dev/null 2>&1 && ok || bad 'git required'
command -v cargo >/dev/null 2>&1 && ok || bad 'cargo required to host-build the rust binary'

# Live harness CLIs: detect only. Never invoke. None of
# grok/kiro-cli/codex → INDEPENDENCE_UNAVAILABLE. Claude is not required.
LIVE_GROK=0
LIVE_KIRO=0
LIVE_CODEX=0
command -v grok >/dev/null 2>&1 && LIVE_GROK=1
command -v kiro-cli >/dev/null 2>&1 && LIVE_KIRO=1
command -v codex >/dev/null 2>&1 && LIVE_CODEX=1
LIVE_N=$((LIVE_GROK + LIVE_KIRO + LIVE_CODEX))
if [ "$LIVE_N" -ge 1 ]; then
  INDEPENDENCE=live-clis-present
  printf 'LIVE_CLIS_PRESENT grok kiro-cli codex (fixture 3-slice; live agents not invoked)\n'
else
  INDEPENDENCE=INDEPENDENCE_UNAVAILABLE
  printf 'INDEPENDENCE_UNAVAILABLE: live grok/kiro-cli/codex CLI missing (need >=1; grok=%s kiro-cli=%s codex=%s); fixture 3-slice still required\n' \
    "$LIVE_GROK" "$LIVE_KIRO" "$LIVE_CODEX"
fi

BASE=$(mktemp -d "${TMPDIR:-/tmp}/wm-blank-home.XXXXXX")
EMPTY_HOME=$(mktemp -d "${TMPDIR:-/tmp}/wm-empty-home.XXXXXX")
OUT="$BASE/out.txt"
ERR="$BASE/err.txt"
HOME="$EMPTY_HOME"
export HOME
# shellcheck disable=SC1091
. "$HERE/scripts/cargo-env.sh"
GIT_CONFIG_GLOBAL=/dev/null
GIT_CONFIG_SYSTEM=/dev/null
export GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM
PYTHONDONTWRITEBYTECODE=1
export PYTHONDONTWRITEBYTECODE

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

# Tarball pin: package-release archives git REF (HEAD must record VERSION 1.18.0).
PKG_OUT="$BASE/pkg"
mkdir -p "$PKG_OUT"
if "$HERE/scripts/package-release.sh" "$VERSION" HEAD "$PKG_OUT" >"$OUT" 2>"$ERR"; then
  ok
else
  bad "package-release refused: $(cat "$OUT") $(cat "$ERR")"
fi
TAR="$PKG_OUT/crucible-$VERSION.tar.gz"
if [ -f "$TAR" ]; then
  ok
  CONTENTS=$(tar -tzf "$TAR")
  for rel in \
    wm.sh \
    adapters/grok.md \
    adapters/kiro.md \
    adapters/claude.md \
    adapters/codex.md \
    skills/architecture/SKILL.md \
    skills/critique/SKILL.md \
    skills/review/SKILL.md \
    skills/loop-design/SKILL.md \
    ROUTING.tsv \
    docs/whats-new.md \
    docs/working-mode.md \
    .grok/rules/loop-router.md \
    templates/herdr/workspace \
    templates/herdr/roles
  do
    printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/$rel\$" \
      && ok || bad "tarball missing $rel"
  done
else
  bad "package-release did not write $TAR"
  CONTENTS=
fi

EXTRACT=
if [ -f "$TAR" ]; then
  mkdir -p "$BASE/extract"
  tar -xzf "$TAR" -C "$BASE/extract"
  EXTRACT="$BASE/extract/crucible-$VERSION"
  [ -x "$EXTRACT/crucible" ] && ok || bad 'extracted crucible is not executable'
  [ -f "$EXTRACT/adapters/grok.md" ] && ok || bad 'extracted package missing adapters/grok.md'
fi

# 1b: default adopt from the tarball still has no wm/skills (guided 1.6.6).
if [ -n "$EXTRACT" ]; then
  init_git_repo "$BASE/guided"
  if run_adopt "$BASE/guided" "$EXTRACT/crucible" adopt work --managed; then
    ok
  else
    bad "default adopt --managed refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  G="$BASE/guided"
  [ ! -f "$G/.crucible/work/wm.sh" ] && ok || bad 'default adopt copied wm.sh'
  [ ! -e "$G/.crucible/skills" ] && ok || bad 'default adopt copied .crucible/skills'
  [ ! -e "$G/.grok/skills" ] && ok || bad 'default adopt copied .grok/skills'
  [ ! -e "$G/.claude/skills" ] && ok || bad 'default adopt copied .claude/skills'
  [ ! -e "$G/.agents/skills" ] && ok || bad 'default adopt copied .agents/skills'
  [ ! -e "$G/.kiro/skills" ] && ok || bad 'default adopt copied .kiro/skills'
  [ ! -d "$G/.crucible/work/adapters" ] && ok || bad 'default adopt copied adapters'
  [ -f "$G/.grok/rules/loop-router.md" ] && ok || bad 'default adopt missing .grok/rules/loop-router.md'
  [ ! -L "$G/.grok/rules/loop-router.md" ] && ok || bad 'default adopt loop-router is a symlink'
  cmp -s "$EXTRACT/.grok/rules/loop-router.md" "$G/.grok/rules/loop-router.md" \
    && ok || bad 'default adopt loop-router drifted from package'
  [ -f "$G/.crucible/herdr/workspace" ] && [ ! -L "$G/.crucible/herdr/workspace" ] \
    && ok || bad 'default adopt herdr workspace is not a regular file'
  printf 'crucible\n' > "$BASE/crucible-label"
  cmp -s "$BASE/crucible-label" "$G/.crucible/herdr/workspace" \
    && ok || bad 'default adopt herdr workspace is not crucible'
  cmp -s "$EXTRACT/templates/herdr/workspace" "$G/.crucible/work/templates/herdr/workspace" \
    && ok || bad 'default adopt program herdr workspace drifted from package'
  cmp -s "$EXTRACT/templates/herdr/roles" "$G/.crucible/work/templates/herdr/roles" \
    && ok || bad 'default adopt program herdr roles drifted from package'
  assert_home_empty
  assert_no_home_skills
fi

# Independent proof: unpack tarball, adopt --working-mode, empty HOME.
AD=
if [ -n "$EXTRACT" ]; then
  init_git_repo "$BASE/adopted"
  if run_adopt "$BASE/adopted" "$EXTRACT/crucible" adopt work --managed --working-mode; then
    ok
  else
    bad "adopt --working-mode from tarball refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  AD="$BASE/adopted"
  [ -f "$AD/.crucible/work/wm.sh" ] && ok || bad 'tarball adopt missing wm.sh'
  [ -x "$AD/.crucible/work/crucible" ] && ok || bad 'tarball adopt missing rust crucible binary'
  _ad_sig=$(dd if="$AD/.crucible/work/crucible" bs=2 count=1 2>/dev/null || true)
  [ "$_ad_sig" != '#!' ] && ok || bad 'adopted .crucible/work/crucible must be the rust binary'
  sh -n "$AD/.crucible/work/wm.sh" && ok || bad 'adopted wm.sh is not valid POSIX sh'
  _ad_ver=$("$AD/.crucible/work/crucible" --version 2>/dev/null || true)
  [ "$_ad_ver" = 1.18.0 ] && ok || bad "adopted crucible --version wanted 1.18.0 got $_ad_ver"
  [ -f "$AD/.crucible/work/adapters/grok.md" ] && ok || bad 'adopt missing adapters/grok.md'
  [ -f "$AD/.crucible/work/adapters/kiro.md" ] && ok || bad 'adopt missing adapters/kiro.md'
  [ -f "$AD/.crucible/work/adapters/claude.md" ] && ok || bad 'adopt missing adapters/claude.md'
  [ -f "$AD/.crucible/work/adapters/codex.md" ] && ok || bad 'adopt missing adapters/codex.md'
  [ -f "$AD/.crucible/skills/architecture/SKILL.md" ] && ok || bad 'canonical architecture missing'
  [ -f "$AD/.grok/skills/architecture/SKILL.md" ] && ok || bad 'repo-root grok view missing'
  [ -f "$AD/.claude/skills/architecture/SKILL.md" ] && ok || bad 'repo-root claude view missing'
  [ -f "$AD/.agents/skills/architecture/SKILL.md" ] && ok || bad 'repo-root agents view missing'
  [ -f "$AD/.kiro/skills/architecture/SKILL.md" ] && ok || bad 'repo-root kiro view missing'
  if [ -d "$AD/.kiro/skills/architecture" ] && [ ! -L "$AD/.kiro/skills/architecture" ]; then
    ok
  else
    bad 'kiro view is not a real directory'
  fi
  cmp -s "$AD/.crucible/skills/architecture/SKILL.md" "$AD/.kiro/skills/architecture/SKILL.md" \
    && ok || bad 'kiro view drifted from canonical'
  [ ! -e "$AD/.codex/skills" ] && ok || bad 'adopt wrote .codex/skills'
  [ -f "$AD/.crucible/.grok/skills/architecture/SKILL.md" ] && ok || bad 'nested grok view missing'
  [ -f "$AD/.crucible/.claude/skills/architecture/SKILL.md" ] && ok || bad 'nested claude view missing'
  [ -f "$AD/.crucible/.agents/skills/architecture/SKILL.md" ] && ok || bad 'nested agents view missing'
  [ -f "$AD/.crucible/.kiro/skills/architecture/SKILL.md" ] && ok || bad 'nested kiro view missing'
  [ -f "$AD/.grok/rules/loop-router.md" ] && ok || bad 'working-mode adopt missing .grok/rules/loop-router.md'
  [ ! -L "$AD/.grok/rules/loop-router.md" ] && ok || bad 'working-mode adopt loop-router is a symlink'
  cmp -s "$EXTRACT/.grok/rules/loop-router.md" "$AD/.grok/rules/loop-router.md" \
    && ok || bad 'working-mode adopt loop-router drifted from package'
  [ -f "$AD/.crucible/herdr/workspace" ] && [ ! -L "$AD/.crucible/herdr/workspace" ] \
    && ok || bad 'working-mode adopt herdr workspace is not a regular file'
  printf 'crucible\n' > "$BASE/crucible-label"
  cmp -s "$BASE/crucible-label" "$AD/.crucible/herdr/workspace" \
    && ok || bad 'working-mode adopt herdr workspace is not crucible'
  cmp -s "$EXTRACT/templates/herdr/workspace" "$AD/.crucible/work/templates/herdr/workspace" \
    && ok || bad 'working-mode adopt program herdr workspace drifted from package'
  cmp -s "$EXTRACT/templates/herdr/roles" "$AD/.crucible/work/templates/herdr/roles" \
    && ok || bad 'working-mode adopt program herdr roles drifted from package'
  if [ -f "$AD/.crucible/work/ENGINE-SOURCE" ]; then
    grep -q '^version: 1.18.0$' "$AD/.crucible/work/ENGINE-SOURCE" \
      && ok || bad 'ENGINE-SOURCE version is not 1.18.0'
  else
    bad 'ENGINE-SOURCE missing after working-mode adopt'
  fi
  assert_home_empty
  assert_no_home_skills
fi

# ---------------------------------------------------------------------------
# Adopted wrapper execs rust: go without IDEA is STOP-ASK INTAKE.
# Full POSIX 3-slice loop is dumped with wm.sh kernel (rollback = previous tarball).
# ---------------------------------------------------------------------------
walk_rc=1
if [ -n "$AD" ] && [ -f "$AD/.crucible/work/wm.sh" ]; then
  WM="$AD/.crucible/work/wm.sh"
  chmod +x "$WM" "$AD/.crucible/work/crucible" 2>/dev/null || true
  export WM_ENGINE="$WM"
  CDPATH=
  cd "$AD"

  wrap_ver=$("$WM" --version 2>"$ERR") || wrap_ver=
  [ "$wrap_ver" = 1.18.0 ] && ok || bad "adopted wm.sh --version wanted 1.18.0 got $wrap_ver err=$(cat "$ERR")"

  set +e
  "$WM" go >"$OUT" 2>"$ERR"
  go_rc=$?
  set -e
  if [ "$go_rc" -ne 0 ] && grep -q 'STOP-ASK INTAKE' "$OUT"; then
    ok
  else
    bad "adopted wm.sh go without IDEA wanted STOP-ASK INTAKE, rc=$go_rc out=$(cat "$OUT") err=$(cat "$ERR")"
  fi
  if [ -f .wm/FLOOR.md ] && grep -q 'card: STOP-ASK INTAKE' .wm/FLOOR.md; then
    ok
  else
    bad 'adopted go must write FLOOR STOP-ASK INTAKE'
  fi
  if grep -E -q 'mark_slice_closed\(\)|reset_brick\(\)' \
    "$HERE/scripts/verify-working-mode-blank-home.sh"; then
    bad 'A5: blank-home must not define mark_slice_closed or reset_brick'
  else
    ok
  fi
  walk_rc=0
  cd "$HERE"
fi

assert_home_empty
assert_no_home_skills

printf 'INDEPENDENCE=%s\n' "$INDEPENDENCE"
printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
[ "$walk_rc" -eq 0 ]
