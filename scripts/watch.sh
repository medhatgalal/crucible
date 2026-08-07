#!/bin/sh
# watch.sh - one live view of a crucible run, redrawn on an interval.
#
#   scripts/watch.sh gate|verdicts|evidence|tail|git|workids|memory [SECONDS]
#
# Each view shows artifacts, never narration. If a view and an agent disagree about
# whether something is done, the view is right. Run from the repository root.

set -eu
PROG_DIR=$(cd "$(dirname "$0")/.." && pwd)
PROG=$(basename "$PROG_DIR")
I=".crucible/$PROG/items"
CL=".crucible/$PROG/claims"
C=".crucible/$PROG/crucible"
WHAT=gate; EVERY=5; ONCE=0
for a in "$@"; do
  case $a in
    --once) ONCE=1 ;;
    ''|*[!0-9]*) WHAT=$a ;;
    *) EVERY=$a ;;
  esac
done

hdr() { printf '\033[1m── %s ──\033[0m\n' "$1"; }

draw_gate() {
  hdr "next / gate"
  if [ -x "$C" ]; then "$C" next 2>&1 | head -40; else echo "no program at $C"; fi
}

draw_verdicts() {
  hdr "item verdicts, as their authors wrote them"
  found=0
  for v in "$I"/*/verdicts/*.md; do
    [ -f "$v" ] || continue; found=1
    printf '%-30s %s\n' "$(printf '%s' "$v" | sed "s|$I/||; s|/verdicts||")" "$(sed -n 1p "$v")"
  done
  [ "$found" = 1 ] || echo "(none yet)"
  echo; hdr "claim verdicts"
  found=0
  for v in "$CL"/*/verdicts/*.md; do
    [ -f "$v" ] || continue; found=1
    printf '%-24s %s  %s\n' "$(printf '%s' "$v" | sed "s|$CL/||; s|/verdicts||")" \
      "$(sed -n 's/^CLAIM-VERDICT: //p' "$v" | head -1)" "$(sed -n 's/^KIND: /kind=/p' "$v" | head -1)"
  done
  [ "$found" = 1 ] || echo "(none yet)"
}

draw_evidence() {
  hdr "recorded evidence — only crucible run can write these"
  n=0
  for e in $(ls -t "$I"/*/evidence/*.txt 2>/dev/null | head -14); do
    n=$((n+1))
    printf '%-38s %s\n' "$(basename "$e")" "$(grep -m1 '^--- exit' "$e" 2>/dev/null)"
  done
  [ "$n" -gt 0 ] || echo "(none yet)"
}

draw_tail() {
  f=$(ls -t "$I"/*/evidence/*.txt 2>/dev/null | head -1)
  if [ -n "${f:-}" ]; then hdr "$(basename "$f")"; tail -34 "$f"
  else hdr "newest evidence"; echo "(none yet)"; fi
}

draw_git() {
  hdr "branches and commits"
  git log --oneline --graph --all --decorate -16 2>/dev/null || echo "(no commits)"
  echo; hdr "working tree"
  s=$(git status --short 2>/dev/null | head -12); [ -n "$s" ] && printf '%s\n' "$s" || echo "(clean)"
}

draw_workids() {
  hdr "work id per item — any commit changes it and voids prior verdicts"
  n=0
  for d in "$I"/*/; do
    [ -d "$d" ] || continue; n=$((n+1)); s=$(basename "$d")
    printf '%-22s %-14s %-10s %s\n' "$s" \
      "$([ -x "$C" ] && "$C" workid "$s" 2>/dev/null)" \
      "$(sed -n 's/^PHASE: //p' "$d/ITEM.md" 2>/dev/null | head -1)" \
      "$(sed -n 's/^STATUS: //p' "$d/ITEM.md" 2>/dev/null | head -1)"
  done
  [ "$n" -gt 0 ] || echo "(no items yet)"
}

draw_memory() {
  hdr "LESSONS.md — concatenated into every later maker brief"
  if [ -s ".crucible/$PROG/LESSONS.md" ]; then tail -16 ".crucible/$PROG/LESSONS.md"
  else echo "(nothing learned yet)"; fi
  echo; hdr "CLAIMS.md"
  grep -E '^### C|^    status:|^    scout:|^    item:' ".crucible/$PROG/CLAIMS.md" 2>/dev/null \
    | head -28 || echo "(no claims yet)"
}

case $WHAT in
  gate|verdicts|evidence|tail|git|workids|memory) : ;;
  *) echo "usage: watch.sh gate|verdicts|evidence|tail|git|workids|memory [SECONDS]"; exit 2 ;;
esac

# --once renders a single frame; the selftest cannot wait on a loop.
[ "$ONCE" = 1 ] && { "draw_$WHAT"; exit 0; }

while :; do
  printf '\033[H\033[2J'
  "draw_$WHAT" 2>&1
  printf '\n\033[2m%s · every %ss · ctrl-c to stop\033[0m\n' "$WHAT" "$EVERY"
  sleep "$EVERY"
done
