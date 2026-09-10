#!/bin/sh
# Project package skills into a target repo: one canonical tree plus harness views.
# Views are relative symlinks to .crucible/skills/<name>/ (16a). No $HOME writes.
set -eu

usage() {
  printf 'usage: project-skills.sh SRC DST\n' >&2
  exit 2
}

[ $# -eq 2 ] || usage

absdir() {
  [ -d "$1" ] || {
    printf 'project-skills: not a directory: %s\n' "$1" >&2
    exit 1
  }
  (unset CDPATH; cd -- "$1" && pwd)
}

SRC=$(absdir "$1")
DST=$(absdir "$2")

[ -n "$DST" ] && [ "$DST" != / ] || {
  printf 'project-skills: refusing DST=%s\n' "$DST" >&2
  exit 1
}

under_dst() {
  case $1 in
    "$DST"|"$DST"/*) ;;
    *)
      printf 'project-skills: path outside DST: %s\n' "$1" >&2
      exit 1
      ;;
  esac
}

rm_under_dst() {
  under_dst "$1"
  if [ -e "$1" ] || [ -L "$1" ]; then
    rm -rf "$1"
  fi
}

link_view() {
  view=$1
  rel=$2
  under_dst "$view"
  mkdir -p "$(dirname "$view")"
  rm_under_dst "$view"
  ln -s "$rel" "$view"
}

found=0
for src in "$SRC"/*; do
  [ -d "$src" ] || continue
  [ -f "$src/SKILL.md" ] || continue
  name=${src##*/}
  case $name in
    ''|.*|*/*) continue ;;
  esac
  found=1

  canon="$DST/.crucible/skills/$name"
  rm_under_dst "$canon"
  mkdir -p "$canon"
  cp -R "$src/." "$canon/"

  link_view "$DST/.crucible/.grok/skills/$name" "../../skills/$name"
  link_view "$DST/.crucible/.claude/skills/$name" "../../skills/$name"
  link_view "$DST/.crucible/.agents/skills/$name" "../../skills/$name"
  link_view "$DST/.grok/skills/$name" "../../.crucible/skills/$name"
  link_view "$DST/.claude/skills/$name" "../../.crucible/skills/$name"
  link_view "$DST/.agents/skills/$name" "../../.crucible/skills/$name"
done

if [ "$found" -eq 0 ]; then
  printf 'project-skills: no SKILL.md batteries in %s\n' "$SRC" >&2
  exit 1
fi
