#!/bin/sh
# Working-mode skill layout + adopt CHECKs (15b, 16a, 9c, 11c). Fixture dirs only.
# No harness CLIs. Falsifier-first: RED when project-skills.sh is missing, and
# again when cmd_adopt does not accept --working-mode.
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
PROJECT="$HERE/scripts/project-skills.sh"
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }

if [ ! -f "$PROJECT" ]; then
  printf 'RED project-skills.sh missing (15b/16a projections not present)\n' >&2
  exit 1
fi
chmod +x "$PROJECT" 2>/dev/null || true

BATTERIES='architecture critique review loop-design'

# Nested harness views plus thin repo-root projections (15b). Canonical is not a view.
VIEWS='
.crucible/.grok/skills
.crucible/.claude/skills
.crucible/.agents/skills
.crucible/.kiro/skills
.grok/skills
.claude/skills
.agents/skills
.kiro/skills
'

BASE=$(mktemp -d "${TMPDIR:-/tmp}/wm-adopt-verify.XXXXXX")
EMPTY_HOME=$(mktemp -d "${TMPDIR:-/tmp}/wm-empty-home.XXXXXX")
DST="$BASE/target"
OUT="$BASE/out.txt"
ERR="$BASE/err.txt"
trap 'rm -rf "$BASE" "$EMPTY_HOME"' 0
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 129' 1
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 130' 2
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 143' 15

mkdir -p "$DST"
(
  CDPATH=
  cd "$DST"
  GIT_CONFIG_GLOBAL=/dev/null
  GIT_CONFIG_SYSTEM=/dev/null
  HOME="$EMPTY_HOME"
  export GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM HOME
  git init -q
  git config user.email 'wm@local'
  git config user.name 'working-mode'
)

sh -n "$PROJECT" && ok || bad 'project-skills.sh is not valid POSIX sh'

if HOME="$EMPTY_HOME" "$PROJECT" "$HERE/skills" "$DST" >"$OUT" 2>"$ERR"; then
  ok
else
  bad "project-skills.sh refused: $(cat "$OUT") $(cat "$ERR")"
fi

home_leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
if [ -z "$home_leftovers" ]; then
  ok
else
  bad "wrote under HOME: $home_leftovers"
fi

# Engine repo harness dirs are real copies of skills/ (one canonical tree). Not symlinks. Not $HOME.
# Kiro CLI: .kiro/skills wins over ~/.kiro/skills. Codex stays .agents (no .codex).
engine_skills=0
for src in "$HERE/skills"/*; do
  [ -d "$src" ] || continue
  [ -f "$src/SKILL.md" ] || continue
  name=${src##*/}
  engine_skills=$((engine_skills + 1))
  for harness in .grok .claude .agents .kiro; do
    view="$HERE/$harness/skills/$name"
    if [ -L "$view" ] || [ ! -d "$view" ]; then
      bad "engine view is not a real directory: $harness/skills/$name"
      continue
    fi
    cmp -s "$src/SKILL.md" "$view/SKILL.md" \
      && ok || bad "engine view drifted: $harness/skills/$name"
  done
done
[ "$engine_skills" -gt 0 ] && ok || bad 'engine skills/ has no SKILL.md'
[ ! -e "$HERE/.codex/skills" ] && ok || bad 'engine wrote .codex/skills (duplicate Codex tree)'
if git -C "$HERE" check-ignore -q -- ".kiro/skills/crucible"; then
  bad 'gitignore hides .kiro/skills (Kiro would not see a committed view)'
else
  ok
fi

ENG="$BASE/engine-layout"
mkdir -p "$ENG/skills/demo"
printf 'demo\n' > "$ENG/skills/demo/SKILL.md"
if HOME="$EMPTY_HOME" "$PROJECT" --engine "$ENG/skills" "$ENG" >"$OUT" 2>"$ERR"; then
  ok
else
  bad "project-skills --engine refused: $(cat "$OUT") $(cat "$ERR")"
fi
[ ! -e "$ENG/.crucible/skills" ] && ok || bad '--engine wrote .crucible/skills'
if [ -d "$ENG/.kiro/skills/demo" ] && [ ! -L "$ENG/.kiro/skills/demo" ] \
  && [ -d "$ENG/.grok/skills/demo" ] && [ ! -L "$ENG/.grok/skills/demo" ] \
  && [ -d "$ENG/.agents/skills/demo" ] && [ ! -L "$ENG/.agents/skills/demo" ]; then
  ok
else
  bad '--engine did not write real harness directories'
fi
cmp -s "$ENG/skills/demo/SKILL.md" "$ENG/.kiro/skills/demo/SKILL.md" \
  && ok || bad '--engine kiro copy drifted'
cmp -s "$ENG/skills/demo/SKILL.md" "$ENG/.grok/skills/demo/SKILL.md" \
  && ok || bad '--engine grok copy drifted'
cmp -s "$ENG/skills/demo/SKILL.md" "$ENG/.agents/skills/demo/SKILL.md" \
  && ok || bad '--engine agents copy drifted'
[ ! -e "$ENG/.codex/skills" ] && ok || bad '--engine wrote .codex/skills'
home_leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
if [ -z "$home_leftovers" ]; then
  ok
else
  bad "engine mode wrote under HOME: $home_leftovers"
fi

# Package source: four batteries only, CONTRACT headings, ROUTING names them.
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

[ -f "$HERE/ROUTING.tsv" ] && ok || bad 'package ROUTING.tsv missing'
if [ -f "$HERE/ROUTING.tsv" ]; then
  grep -q 'battery' "$HERE/ROUTING.tsv" && ok || bad 'ROUTING.tsv missing battery column'
fi

for name in $BATTERIES; do
  [ -f "$HERE/skills/$name/SKILL.md" ] && ok || bad "package skills/$name/SKILL.md missing"
  [ -f "$HERE/skills/$name/CONTRACT.md" ] && ok || bad "package skills/$name/CONTRACT.md missing"
  if [ -f "$HERE/skills/$name/CONTRACT.md" ]; then
    grep -q 'must-write' "$HERE/skills/$name/CONTRACT.md" && ok || bad "$name CONTRACT.md missing must-write heading"
    grep -qi 'must-not' "$HERE/skills/$name/CONTRACT.md" && ok || bad "$name CONTRACT.md missing must-not heading"
  fi
  if [ -f "$HERE/ROUTING.tsv" ]; then
    awk -F '\t' -v n="$name" 'NR>1 && $3==n { found=1 } END { exit !found }' "$HERE/ROUTING.tsv" \
      && ok || bad "ROUTING.tsv battery column does not name $name"
  fi
done

canon_root="$DST/.crucible/skills"
for name in $BATTERIES; do
  canon="$canon_root/$name"
  if [ -d "$canon" ] && [ ! -L "$canon" ]; then
    ok
  else
    bad "canonical missing or is a view: $canon"
  fi
  [ -f "$canon/SKILL.md" ] && ok || bad "canonical SKILL.md missing: $name"
  [ -f "$canon/CONTRACT.md" ] && ok || bad "canonical CONTRACT.md missing: $name"
  if [ -f "$HERE/skills/$name/SKILL.md" ] && [ -f "$canon/SKILL.md" ]; then
    cmp -s "$HERE/skills/$name/SKILL.md" "$canon/SKILL.md" && ok || bad "canonical SKILL.md != package for $name"
  fi
  if [ -f "$canon/CONTRACT.md" ]; then
    grep -q 'must-write' "$canon/CONTRACT.md" && ok || bad "projected $name CONTRACT.md missing must-write"
    grep -qi 'must-not' "$canon/CONTRACT.md" && ok || bad "projected $name CONTRACT.md missing must-not"
  fi

  for vdir in $VIEWS; do
    view="$DST/$vdir/$name"
    if [ -f "$view/SKILL.md" ]; then
      ok
    else
      bad "view missing $vdir/$name/SKILL.md"
      continue
    fi
    if [ -f "$canon/SKILL.md" ]; then
      cmp -s "$canon/SKILL.md" "$view/SKILL.md" && ok || bad "SKILL.md differs: $vdir/$name"
    fi
    if [ -f "$canon/CONTRACT.md" ] && [ -f "$view/CONTRACT.md" ]; then
      cmp -s "$canon/CONTRACT.md" "$view/CONTRACT.md" && ok || bad "CONTRACT.md differs: $vdir/$name"
    elif [ -f "$canon/CONTRACT.md" ]; then
      bad "view missing CONTRACT.md: $vdir/$name"
    fi
    if [ -L "$view" ] || [ ! -d "$view" ]; then
      bad "view is not a real directory: $vdir/$name"
    else
      ok
    fi
  done
done

# ---------------------------------------------------------------------------
# Task 3: adopt --working-mode, --refresh src==dst, KEEP, tarball pin, HOME empty.
# ---------------------------------------------------------------------------

CRUCIBLE="$HERE/crucible"
VERSION=$(sed -n '1p' "$HERE/VERSION")
WM="$HERE/wm.sh"
# shellcheck disable=SC1091
. "$HERE/scripts/cargo-env.sh"
if ! ( CDPATH=; cd -- "$HERE" && cargo build --release --locked >/dev/null ); then
  printf 'RED cargo build --release failed\n' >&2
  exit 1
fi
RUST_BIN="$HERE/target/release/crucible"
[ -x "$RUST_BIN" ] || { printf 'RED target/release/crucible missing\n' >&2; exit 1; }
export CRUCIBLE_RUST_BIN="$RUST_BIN"

assert_home_empty() {
  leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
  if [ -z "$leftovers" ]; then
    ok
  else
    bad "wrote under HOME: $leftovers"
  fi
}

init_git_repo() {
  dir=$1
  mkdir -p "$dir"
  (
    CDPATH=
    cd "$dir"
    GIT_CONFIG_GLOBAL=/dev/null
    GIT_CONFIG_SYSTEM=/dev/null
    HOME="$EMPTY_HOME"
    export GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM HOME
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
  HOME="$EMPTY_HOME"
  GIT_CONFIG_GLOBAL=/dev/null
  GIT_CONFIG_SYSTEM=/dev/null
  export HOME GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM
  ( CDPATH=; cd "$dir" && "$@" >"$OUT" 2>"$ERR" )
}

write_ready_spec() {
  dir=$1
  printf 'an idea\n' > "$dir/IDEA.md"
  cat > "$dir/SPEC.md" <<'EOF'
## Goal
tiny throwaway product file
## Non-goals
live systems
## Owned files
- product.txt
## Test files
- (none)
## Acceptance criteria
- product.txt exists
## Focused falsifier
MAKER-WRITES
## Stop conditions
stop-ask on live write
## Risk
LOW
SPEC-AUTHOR: operator
EOF
}

if [ ! -f "$CRUCIBLE" ]; then
  printf 'RED crucible missing\n' >&2
  exit 1
fi
if ! grep -q -- '--working-mode' "$CRUCIBLE"; then
  printf 'RED crucible adopt does not accept --working-mode\n' >&2
  exit 1
fi

# Default adopt (no --working-mode) must stay guided 1.6.6: no wm/skills/projections.
init_git_repo "$BASE/guided"
if run_adopt "$BASE/guided" "$CRUCIBLE" adopt work --managed; then
  ok
else
  bad "default adopt --managed refused: $(cat "$OUT") $(cat "$ERR")"
fi
[ ! -f "$BASE/guided/.crucible/work/wm.sh" ] && ok || bad 'default adopt copied wm.sh'
[ ! -e "$BASE/guided/.crucible/skills" ] && ok || bad 'default adopt copied .crucible/skills'
[ ! -e "$BASE/guided/.grok/skills" ] && ok || bad 'default adopt copied .grok/skills'
[ ! -e "$BASE/guided/.claude/skills" ] && ok || bad 'default adopt copied .claude/skills'
[ ! -e "$BASE/guided/.agents/skills" ] && ok || bad 'default adopt copied .agents/skills'
[ ! -e "$BASE/guided/.kiro/skills" ] && ok || bad 'default adopt copied .kiro/skills'
[ ! -f "$BASE/guided/.crucible/work/ENGINE-SOURCE" ] && ok || bad 'default adopt wrote ENGINE-SOURCE'
assert_home_empty

# adopt --working-mode into a temp git repo.
init_git_repo "$BASE/adopted"
if run_adopt "$BASE/adopted" "$CRUCIBLE" adopt work --managed --working-mode; then
  ok
else
  bad "adopt --working-mode refused: $(cat "$OUT") $(cat "$ERR")"
fi
AD="$BASE/adopted"
[ -f "$AD/.crucible/work/wm.sh" ] && ok || bad 'adopt --working-mode missing .crucible/work/wm.sh'
[ -x "$AD/.crucible/work/wm.sh" ] && ok || bad '.crucible/work/wm.sh is not executable'
sh -n "$AD/.crucible/work/wm.sh" && ok || bad 'adopted wm.sh is not valid POSIX sh'
[ -x "$AD/.crucible/work/crucible" ] && ok || bad 'adopt --working-mode missing rust crucible binary'
_ad_sig=$(dd if="$AD/.crucible/work/crucible" bs=2 count=1 2>/dev/null || true)
[ "$_ad_sig" != '#!' ] && ok || bad 'adopted .crucible/work/crucible must be the rust binary'
_ad_ver=$("$AD/.crucible/work/crucible" --version 2>/dev/null || true)
[ "$_ad_ver" = "$VERSION" ] && ok || bad "adopted crucible --version wanted $VERSION got $_ad_ver"
[ -f "$AD/.crucible/work/WORKING-MODE.md" ] && ok || bad 'adopt --working-mode missing WORKING-MODE.md'
[ -f "$AD/.crucible/skills/crucible/SKILL.md" ] && ok || bad 'adopt missing outer-loop skills/crucible'
[ -f "$AD/.grok/skills/crucible/SKILL.md" ] && ok || bad 'adopt missing grok view skills/crucible'
[ -f "$AD/.crucible/skills/architecture/SKILL.md" ] && ok || bad 'canonical skills/architecture/SKILL.md missing'
[ -d "$AD/.crucible/skills/architecture" ] && [ ! -L "$AD/.crucible/skills/architecture" ] \
  && ok || bad 'canonical architecture is missing or is a view'
[ -f "$AD/.grok/skills/architecture/SKILL.md" ] && ok || bad '.grok/skills/architecture/SKILL.md projection missing'
[ -f "$AD/.claude/skills/architecture/SKILL.md" ] && ok || bad '.claude/skills/architecture/SKILL.md projection missing'
[ -f "$AD/.agents/skills/architecture/SKILL.md" ] && ok || bad '.agents/skills/architecture/SKILL.md projection missing'
[ -f "$AD/.kiro/skills/architecture/SKILL.md" ] && ok || bad '.kiro/skills/architecture/SKILL.md projection missing'
if [ -d "$AD/.kiro/skills/architecture" ] && [ ! -L "$AD/.kiro/skills/architecture" ]; then
  ok
else
  bad '.kiro/skills/architecture is not a real directory'
fi
cmp -s "$AD/.crucible/skills/architecture/SKILL.md" "$AD/.kiro/skills/architecture/SKILL.md" \
  && ok || bad '.kiro/skills/architecture copy drifted from canonical'
[ ! -e "$AD/.codex/skills" ] && ok || bad 'adopt wrote .codex/skills'
[ -f "$AD/.crucible/.grok/skills/architecture/SKILL.md" ] && ok || bad 'nested grok view missing'
[ -f "$AD/.crucible/.kiro/skills/architecture/SKILL.md" ] && ok || bad 'nested kiro view missing'
[ -f "$AD/.crucible/work/ROUTING.tsv" ] && ok || bad 'program ROUTING.tsv missing'
[ -f "$AD/.crucible/ROUTING.tsv" ] && ok || bad '.crucible/ROUTING.tsv missing'
[ -f "$AD/.crucible/work/ENGINE-SOURCE" ] && ok || bad 'ENGINE-SOURCE missing'
if [ -f "$AD/.crucible/work/ENGINE-SOURCE" ]; then
  grep -q "^version: $VERSION\$" "$AD/.crucible/work/ENGINE-SOURCE" \
    && ok || bad "ENGINE-SOURCE missing version $VERSION"
  grep -Eq '^sha256: [0-9a-f]{64}$' "$AD/.crucible/work/ENGINE-SOURCE" \
    && ok || bad 'ENGINE-SOURCE missing sha256 of installing tree'
fi
if [ -f "$AD/.crucible/work/PROGRAM" ]; then
  grep -q '^working-mode: yes$' "$AD/.crucible/work/PROGRAM" \
    && ok || bad 'PROGRAM missing working-mode: yes'
fi
if [ -f "$AD/.crucible/.gitignore" ]; then
  if grep -Eq '^skills/?$|^\*/skills/?$|^skills/' "$AD/.crucible/.gitignore"; then
    bad '.crucible/.gitignore ignores skills/'
  else
    ok
  fi
else
  bad '.crucible/.gitignore missing after adopt'
fi
for name in $BATTERIES; do
  [ -f "$AD/.crucible/skills/$name/SKILL.md" ] && ok || bad "adopt missing canonical $name"
  [ -f "$AD/.grok/skills/$name/SKILL.md" ] && ok || bad "adopt missing grok view $name"
done
assert_home_empty

# --refresh with src==dst refuses (installed binary refreshing itself).
if run_adopt "$AD" "$AD/.crucible/work/crucible" adopt work --refresh; then
  bad "self-refresh src==dst was allowed: $(cat "$OUT")"
else
  if grep -E -q 'refused' "$ERR" "$OUT" 2>/dev/null; then
    ok
  else
    bad "self-refresh wanted refused, got out=$(cat "$OUT") err=$(cat "$ERR")"
  fi
fi
# Guided self-refresh also refuses (same CHECK, not working-mode-only).
if run_adopt "$BASE/guided" "$BASE/guided/.crucible/work/crucible" adopt work --refresh; then
  bad "guided self-refresh src==dst was allowed"
else
  grep -E -q 'refused' "$ERR" "$OUT" 2>/dev/null && ok \
    || bad "guided self-refresh wanted refused, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi

# Refresh from a different tree (this source) is allowed and KEEP honors .keep / KEEP list.
printf 'PATCHED-ARCH\n' > "$AD/.crucible/skills/architecture/SKILL.md"
touch "$AD/.crucible/skills/architecture/.keep"
printf 'PATCHED-CRITIQUE\n' > "$AD/.crucible/skills/critique/SKILL.md"
printf 'PATCHED-REVIEW\n' > "$AD/.crucible/skills/review/SKILL.md"
printf 'review\n' > "$AD/.crucible/skills/KEEP"
printf 'PATCHED-LOOP\n' > "$AD/.crucible/skills/loop-design/SKILL.md"
if run_adopt "$AD" "$CRUCIBLE" adopt work --refresh; then
  ok
else
  bad "refresh from source refused: $(cat "$OUT") $(cat "$ERR")"
fi
grep -q 'PATCHED-ARCH' "$AD/.crucible/skills/architecture/SKILL.md" \
  && ok || bad 'KEEP .keep did not preserve architecture'
grep -q 'PATCHED-REVIEW' "$AD/.crucible/skills/review/SKILL.md" \
  && ok || bad 'KEEP list did not preserve review'
grep -q 'PATCHED-CRITIQUE' "$AD/.crucible/skills/critique/SKILL.md" \
  && bad 'un-KEPT critique survived refresh' \
  || ok
grep -q 'PATCHED-LOOP' "$AD/.crucible/skills/loop-design/SKILL.md" \
  && bad 'un-KEPT loop-design survived refresh' \
  || ok
# Views still resolve after KEEP restore.
cmp -s "$AD/.crucible/skills/architecture/SKILL.md" "$AD/.grok/skills/architecture/SKILL.md" \
  && ok || bad 'KEEP architecture view diverged from canonical'
if run_adopt "$AD" "$CRUCIBLE" adopt work --refresh --overwrite-batteries; then
  ok
else
  bad "refresh --overwrite-batteries refused: $(cat "$OUT") $(cat "$ERR")"
fi
grep -q 'PATCHED-ARCH' "$AD/.crucible/skills/architecture/SKILL.md" \
  && bad '--overwrite-batteries left KEEP architecture' \
  || ok
cmp -s "$HERE/skills/architecture/SKILL.md" "$AD/.crucible/skills/architecture/SKILL.md" \
  && ok || bad '--overwrite-batteries did not restore package architecture'

# install.md commit recipe must name repo-root harness views (Grok does not scan
# .crucible/.grok/skills). Nested views under .crucible/ are not enough.
if awk '
  /```sh/ { insh=1; block=""; next }
  insh && /```/ {
    if (block ~ /git add/ && block ~ /\.crucible/) rec=block
    insh=0
    next
  }
  insh { block = block $0 "\n" }
  END {
    if (rec ~ /\.grok\/skills/ && rec ~ /\.claude\/skills/ && rec ~ /\.agents\/skills/ && rec ~ /\.kiro\/skills/) exit 0
    exit 1
  }
' "$HERE/docs/install.md"; then
  ok
else
  bad 'docs/install.md commit recipe omits repo-root .{grok,claude,agents,kiro}/skills'
fi

# KEEP snapshot must be restored if project-skills.sh fails after wiping canonical dirs.
KEEPFAIL_SRC="$BASE/keepfail-src"
mkdir -p "$KEEPFAIL_SRC/scripts" "$KEEPFAIL_SRC/skills"
cp "$CRUCIBLE" "$KEEPFAIL_SRC/crucible"
cp "$HERE/VERSION" "$KEEPFAIL_SRC/VERSION"
cp "$WM" "$KEEPFAIL_SRC/wm.sh"
mkdir -p "$KEEPFAIL_SRC/target/release"
cp "$RUST_BIN" "$KEEPFAIL_SRC/target/release/crucible"
cp "$HERE/ROUTING.tsv" "$KEEPFAIL_SRC/ROUTING.tsv"
cp -R "$HERE/skills/." "$KEEPFAIL_SRC/skills/"
cat > "$KEEPFAIL_SRC/scripts/project-skills.sh" <<'EOF'
#!/bin/sh
# Fixture: wipe canonical batteries then fail (projector crash after rm).
set -eu
DST=$2
[ -n "$DST" ] && [ "$DST" != / ] || exit 1
skills="$DST/.crucible/skills"
if [ -d "$skills" ]; then
  for d in "$skills"/*; do
    [ -d "$d" ] || continue
    case $d in
      "$skills"/*) rm -rf "$d" ;;
    esac
  done
fi
printf 'project-skills: fixture fail after wipe\n' >&2
exit 1
EOF
chmod +x "$KEEPFAIL_SRC/crucible" "$KEEPFAIL_SRC/wm.sh" "$KEEPFAIL_SRC/scripts/project-skills.sh"
init_git_repo "$BASE/keep-fail"
if run_adopt "$BASE/keep-fail" "$CRUCIBLE" adopt work --managed --working-mode; then
  ok
else
  bad "keep-fail initial adopt refused: $(cat "$OUT") $(cat "$ERR")"
fi
printf 'PATCHED-KEEP-FAIL\n' > "$BASE/keep-fail/.crucible/skills/architecture/SKILL.md"
touch "$BASE/keep-fail/.crucible/skills/architecture/.keep"
printf 'UNKEPT-CRITIQUE\n' > "$BASE/keep-fail/.crucible/skills/critique/SKILL.md"
if run_adopt "$BASE/keep-fail" "$KEEPFAIL_SRC/crucible" adopt work --refresh; then
  bad 'refresh with failing projector was allowed'
else
  grep -E -q 'refused' "$ERR" "$OUT" 2>/dev/null && ok \
    || bad "failing projector wanted refused, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi
grep -q 'PATCHED-KEEP-FAIL' "$BASE/keep-fail/.crucible/skills/architecture/SKILL.md" \
  && ok || bad 'KEEP architecture not restored after projector wipe+fail'
[ -e "$BASE/keep-fail/.crucible/skills/architecture/.keep" ] \
  && ok || bad '.keep marker missing after projector wipe+fail'
if grep -q 'UNKEPT-CRITIQUE' "$BASE/keep-fail/.crucible/skills/critique/SKILL.md" 2>/dev/null; then
  bad 'failing projector did not wipe un-KEPT critique (fixture did not exercise wipe)'
else
  ok
fi
keep_left=$(find "$BASE/keep-fail/.crucible/work" -name '.keep-snapshot.*' -print 2>/dev/null || true)
if [ -z "$keep_left" ]; then
  ok
else
  bad "leftover KEEP snapshot after failed projection: $keep_left"
fi

# Missing required battery in ROUTING → adopt prints refused (no generalist-fallback).
FAKE="$BASE/fake-src"
mkdir -p "$FAKE/scripts" "$FAKE/skills"
cp "$CRUCIBLE" "$FAKE/crucible"
cp "$HERE/VERSION" "$FAKE/VERSION"
cp "$WM" "$FAKE/wm.sh"
mkdir -p "$FAKE/target/release"
cp "$RUST_BIN" "$FAKE/target/release/crucible"
cp "$PROJECT" "$FAKE/scripts/project-skills.sh"
cp -R "$HERE/skills/." "$FAKE/skills/"
{
  printf 'phase\tjob\tbattery\trole\tstake\trequired\n'
  printf 'MAP\tdecompose\tarchitecture\tplanner\tspec\tyes\n'
  printf 'X\tmissing\tno-such-battery\toperator\tspec\tyes\n'
} > "$FAKE/ROUTING.tsv"
chmod +x "$FAKE/crucible" "$FAKE/wm.sh" "$FAKE/scripts/project-skills.sh"
init_git_repo "$BASE/missing-bat"
if run_adopt "$BASE/missing-bat" "$FAKE/crucible" adopt work --managed --working-mode; then
  bad 'adopt with missing required battery was allowed'
else
  grep -E -q 'refused' "$ERR" "$OUT" 2>/dev/null && ok \
    || bad "missing battery wanted refused, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi
[ ! -f "$BASE/missing-bat/.crucible/work/wm.sh" ] && ok \
  || bad 'refused adopt still installed wm.sh'

# ROUTING required=no: missing optional battery does not fail adopt.
if awk -F '\t' '$1=="RESEARCH" && $3=="research" && $6=="no" { found=1 } END { exit !found }' \
  "$HERE/ROUTING.tsv"; then
  ok
else
  bad 'package ROUTING.tsv missing RESEARCH survey research specifier spec required=no'
fi
if awk -F '\t' '$1=="REPO" && $3=="repo-scout" && $6=="no" { found=1 } END { exit !found }' \
  "$HERE/ROUTING.tsv"; then
  ok
else
  bad 'package ROUTING.tsv missing REPO inventory repo-scout specifier spec required=no'
fi
FAKE_OPT="$BASE/fake-src-opt"
mkdir -p "$FAKE_OPT/scripts" "$FAKE_OPT/skills"
cp "$CRUCIBLE" "$FAKE_OPT/crucible"
cp "$HERE/VERSION" "$FAKE_OPT/VERSION"
cp "$WM" "$FAKE_OPT/wm.sh"
mkdir -p "$FAKE_OPT/target/release"
cp "$RUST_BIN" "$FAKE_OPT/target/release/crucible"
cp "$PROJECT" "$FAKE_OPT/scripts/project-skills.sh"
cp -R "$HERE/skills/." "$FAKE_OPT/skills/"
rm -rf "$FAKE_OPT/skills/research"
{
  printf 'phase\tjob\tbattery\trole\tstake\trequired\n'
  printf 'MAP\tdecompose\tarchitecture\tplanner\tspec\tyes\n'
  printf 'RESEARCH\tsurvey\tresearch\tspecifier\tspec\tno\n'
} > "$FAKE_OPT/ROUTING.tsv"
chmod +x "$FAKE_OPT/crucible" "$FAKE_OPT/wm.sh" "$FAKE_OPT/scripts/project-skills.sh"
init_git_repo "$BASE/missing-opt"
if run_adopt "$BASE/missing-opt" "$FAKE_OPT/crucible" adopt work --managed --working-mode; then
  ok
else
  bad "adopt with missing optional research battery refused: $(cat "$OUT") $(cat "$ERR")"
fi
[ -f "$BASE/missing-opt/.crucible/work/wm.sh" ] && ok \
  || bad 'optional missing research must still install wm.sh'
[ ! -f "$BASE/missing-opt/.crucible/skills/research/SKILL.md" ] && ok \
  || bad 'optional missing research must not invent the battery'

# wm ready is POSIX kernel (dumped). rust unknown command until later.
READY_OK="$BASE/ready-ok"
mkdir -p "$READY_OK"
write_ready_spec "$READY_OK"
STAGE_WM="$BASE/stage-wm"
mkdir -p "$STAGE_WM"
cp "$WM" "$STAGE_WM/wm.sh"
cp "$RUST_BIN" "$STAGE_WM/crucible"
chmod +x "$STAGE_WM/wm.sh" "$STAGE_WM/crucible"
if ( CDPATH=; cd "$READY_OK" && "$STAGE_WM/wm.sh" ready >"$OUT" 2>"$ERR" ); then
  bad "rust ready must not be a silent POSIX kernel: $(cat "$OUT")"
else
  grep -E -q 'unknown command' "$ERR" "$OUT" 2>/dev/null && ok \
    || bad "ready wanted unknown command, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi

# tarball via package-release includes wm.sh and skills/architecture/SKILL.md;
# adopt from the extracted pin (not the git worktree path).
PKG_OUT="$BASE/pkg"
mkdir -p "$PKG_OUT"
if HOME="$EMPTY_HOME" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
  "$HERE/scripts/package-release.sh" "$VERSION" HEAD "$PKG_OUT" >"$OUT" 2>"$ERR"; then
  ok
else
  bad "package-release refused: $(cat "$OUT") $(cat "$ERR")"
fi
TAR="$PKG_OUT/crucible-$VERSION.tar.gz"
if [ -f "$TAR" ]; then
  CONTENTS=$(tar -tzf "$TAR")
  printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/wm.sh\$" \
    && ok || bad 'tarball missing wm.sh'
  printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/skills/architecture/SKILL.md\$" \
    && ok || bad 'tarball missing skills/architecture/SKILL.md'
  printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/ROUTING.tsv\$" \
    && ok || bad 'tarball missing ROUTING.tsv'
  mkdir -p "$BASE/extract"
  tar -xzf "$TAR" -C "$BASE/extract"
  EXTRACT="$BASE/extract/crucible-$VERSION"
  init_git_repo "$BASE/from-tar"
  if run_adopt "$BASE/from-tar" "$EXTRACT/crucible" adopt work --managed --working-mode; then
    ok
  else
    bad "adopt from tarball refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  [ -f "$BASE/from-tar/.crucible/work/wm.sh" ] && ok || bad 'tarball adopt missing wm.sh'
  [ -f "$BASE/from-tar/.crucible/skills/architecture/SKILL.md" ] && ok \
    || bad 'tarball adopt missing canonical architecture'
  [ -f "$BASE/from-tar/.grok/skills/architecture/SKILL.md" ] && ok \
    || bad 'tarball adopt missing grok projection'
  [ -f "$BASE/from-tar/.kiro/skills/architecture/SKILL.md" ] && ok \
    || bad 'tarball adopt missing kiro projection'
  [ -f "$BASE/from-tar/.crucible/work/ENGINE-SOURCE" ] && ok \
    || bad 'tarball adopt missing ENGINE-SOURCE'
else
  bad "package-release did not write $TAR"
fi
assert_home_empty

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
