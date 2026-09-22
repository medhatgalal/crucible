#!/bin/sh
# Project package skills into a target repo: one canonical tree plus real harness copies.
# Product mode: copy into .crucible/skills/<name>/, then copy that tree into each
# harness directory. No symlinks.
# --engine: canonical tree is skills/. Copy each skill into .grok/.claude/.agents/.kiro.
# Do not write .crucible/skills. No $HOME writes.
# Codex discovery is .agents/skills. Do not also write .codex/skills (duplicate names).
set -eu

usage() {
  printf 'usage: project-skills.sh [--engine] SRC DST\n' >&2
  exit 2
}

engine=0
if [ "${1:-}" = "--engine" ]; then
  engine=1
  shift
fi
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

if [ "$engine" -eq 1 ] && [ "$SRC" != "$DST/skills" ]; then
  printf 'project-skills: --engine requires SRC to be DST/skills\n' >&2
  exit 1
fi

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

copy_tree() {
  dest=$1
  from=$2
  under_dst "$dest"
  [ -d "$from" ] && [ ! -L "$from" ] || {
    printf 'project-skills: refusing to copy a non-directory: %s\n' "$from" >&2
    exit 1
  }
  rm_under_dst "$dest"
  mkdir -p "$dest"
  cp -R "$from/." "$dest/"
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

  if [ "$engine" -eq 1 ]; then
    copy_tree "$DST/.grok/skills/$name" "$src"
    copy_tree "$DST/.claude/skills/$name" "$src"
    copy_tree "$DST/.agents/skills/$name" "$src"
    copy_tree "$DST/.kiro/skills/$name" "$src"
    continue
  fi

  canon="$DST/.crucible/skills/$name"
  rm_under_dst "$canon"
  mkdir -p "$canon"
  cp -R "$src/." "$canon/"

  copy_tree "$DST/.crucible/.grok/skills/$name" "$canon"
  copy_tree "$DST/.crucible/.claude/skills/$name" "$canon"
  copy_tree "$DST/.crucible/.agents/skills/$name" "$canon"
  copy_tree "$DST/.crucible/.kiro/skills/$name" "$canon"
  copy_tree "$DST/.grok/skills/$name" "$canon"
  copy_tree "$DST/.claude/skills/$name" "$canon"
  copy_tree "$DST/.agents/skills/$name" "$canon"
  copy_tree "$DST/.kiro/skills/$name" "$canon"
done

if [ "$found" -eq 0 ]; then
  printf 'project-skills: no SKILL.md batteries in %s\n' "$SRC" >&2
  exit 1
fi
