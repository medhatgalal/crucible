#!/bin/sh
# Working-mode skill layout CHECKs (15b, 16a). Fixture dirs only. No harness CLIs.
# Falsifier-first: RED when project-skills.sh is missing.
# Task 3 fills adopt --working-mode / --refresh / tarball assertions (gated below).
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
.grok/skills
.claude/skills
.agents/skills
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

# Package source: four batteries only, CONTRACT headings, ROUTING names them.
extra=
for d in "$HERE/skills"/*; do
  [ -d "$d" ] || continue
  n=${d##*/}
  case $n in
    architecture|critique|review|loop-design) ;;
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
    if [ -L "$view" ]; then
      target=$(readlink "$view")
      case $target in
        /*) bad "absolute view symlink $vdir/$name -> $target" ;;
        *) ok ;;
      esac
      case $target in
        *"$EMPTY_HOME"*) bad "view symlink into HOME: $vdir/$name -> $target" ;;
        *) ok ;;
      esac
    fi
  done
done

# ---------------------------------------------------------------------------
# Task 3 (gated): adopt --working-mode, --refresh, tarball pin.
# Skip until cmd_adopt accepts --working-mode. Do not implement adopt here.
#
# adopt --working-mode into temp git repo
# assert .crucible/PROGRAM/wm.sh and .crucible/skills/architecture/SKILL.md
# assert .grok/skills/architecture/SKILL.md exists (projection)
# HOME=empty: no writes under the empty home except maybe .git config we set
# --refresh with src==dst refuses
# tarball via package-release includes wm.sh and skills/architecture/SKILL.md
# ---------------------------------------------------------------------------

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
