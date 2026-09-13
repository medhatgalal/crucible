#!/bin/sh
# 1.8.0 Task 1: wm go + no-args help + discovery skill.
# Empty HOME. Fixture git repos and a fake grok only.
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
WM="$HERE/wm.sh"
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }

if [ ! -f "$WM" ]; then
  printf 'RED wm.sh missing\n' >&2
  exit 1
fi
chmod +x "$WM" 2>/dev/null || true

BASE=$(mktemp -d "${TMPDIR:-/tmp}/wm-go-verify.XXXXXX")
EMPTY_HOME=$(mktemp -d "${TMPDIR:-/tmp}/wm-go-empty-home.XXXXXX")
OUT="$BASE/out.txt"
ERR="$BASE/err.txt"
HOME="$EMPTY_HOME"
export HOME
GIT_CONFIG_GLOBAL=/dev/null
GIT_CONFIG_SYSTEM=/dev/null
export GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM
export WM_ENGINE="$WM"

trap 'rm -rf "$BASE" "$EMPTY_HOME"' 0
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 129' 1
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 130' 2
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 143' 15

init_git_repo() {
  dir=$1
  mkdir -p "$dir"
  (
    CDPATH=
    cd "$dir"
    git init -q
    git config user.email 'wm@local'
    git config user.name 'working-mode'
  )
}

# POSIX parse.
if [ -f "$HERE/wm-go.sh" ]; then
  sh -n "$WM" && ok || bad 'wm.sh is not valid POSIX sh'
  sh -n "$HERE/wm-go.sh" && ok || bad 'wm-go.sh is not valid POSIX sh'
else
  bad 'wm-go.sh missing'
  sh -n "$WM" && ok || bad 'wm.sh is not valid POSIX sh'
fi

# No-args help: exit 0 and required tokens.
help_dir="$BASE/help"
init_git_repo "$help_dir"
set +e
(
  CDPATH=
  cd "$help_dir"
  "$WM"
) >"$OUT" 2>"$ERR"
help_rc=$?
set -e
if [ "$help_rc" -eq 0 ]; then
  ok
else
  bad "no-args wanted exit 0, got $help_rc out=$(cat "$OUT") err=$(cat "$ERR")"
fi
if grep -q 'run:' "$OUT" && grep -q 'go' "$OUT" && grep -q 'harness:' "$OUT"; then
  ok
else
  bad "no-args wanted run:/go/harness:, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi
if grep -q 'working-mode' "$OUT" \
  && grep -q 'commands: go help status init cast loop bound' "$OUT" \
  && grep -q 'harness: read WORKING-MODE.md then go' "$OUT"; then
  ok
else
  bad "no-args missing help tokens, got out=$(cat "$OUT")"
fi

# No CLI workers: INDEPENDENCE_UNAVAILABLE, nonzero.
nocli="$BASE/nocli"
init_git_repo "$nocli"
set +e
(
  CDPATH=
  cd "$nocli"
  PATH=/usr/bin:/bin
  export PATH
  "$WM" go
) >"$OUT" 2>"$ERR"
nocli_rc=$?
set -e
if [ "$nocli_rc" -ne 0 ]; then
  ok
else
  bad "PATH-stripped go must be nonzero (out=$(cat "$OUT") err=$(cat "$ERR"))"
fi
if grep -q 'INDEPENDENCE_UNAVAILABLE' "$OUT" "$ERR"; then
  ok
else
  bad "PATH-stripped go wanted INDEPENDENCE_UNAVAILABLE, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi

# Fake grok: copy idea, distinct spec0/make0, loop STOP-ASK (no SPEC).
BIN="$BASE/bin"
mkdir -p "$BIN"
printf '#!/bin/sh\nexit 0\n' > "$BIN/grok"
chmod +x "$BIN/grok"
fake="$BASE/fake-grok"
init_git_repo "$fake"
printf 'build a tiny product\n' > "$fake/seed.md"
set +e
(
  CDPATH=
  cd "$fake"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" go seed.md
) >"$OUT" 2>"$ERR"
fake_rc=$?
set -e
if [ "$fake_rc" -ne 0 ]; then
  ok
else
  bad "fake grok go must STOP-ASK (rc=0 out=$(cat "$OUT") err=$(cat "$ERR"))"
fi
if [ -f "$fake/IDEA.md" ] && grep -q 'build a tiny product' "$fake/IDEA.md"; then
  ok
else
  bad "go with seed.md must copy to IDEA.md, got $(cat "$fake/IDEA.md" 2>/dev/null || echo ABSENT)"
fi
spec_id=
make_id=
if [ -f "$fake/.wm/PANEL.tsv" ]; then
  spec_id=$(awk -F '\t' '$1=="specifier"{print $2; exit}' "$fake/.wm/PANEL.tsv")
  make_id=$(awk -F '\t' '$1=="maker"{print $2; exit}' "$fake/.wm/PANEL.tsv")
fi
if [ "$spec_id" = spec0 ] && [ "$make_id" = make0 ] && [ "$spec_id" != "$make_id" ]; then
  ok
else
  bad "panel wanted spec0 != make0, got spec=$spec_id make=$make_id panel=$(cat "$fake/.wm/PANEL.tsv" 2>/dev/null || echo ABSENT)"
fi
if grep -q 'STOP-ASK' "$OUT" "$ERR"; then
  ok
else
  bad "fake grok loop wanted STOP-ASK, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi

# Two CLIs: maker kind != reviewer kind.
printf '#!/bin/sh\nexit 0\n' > "$BIN/claude"
chmod +x "$BIN/claude"
two="$BASE/two-cli"
init_git_repo "$two"
printf 'two cli idea\n' > "$two/IDEA.md"
set +e
(
  CDPATH=
  cd "$two"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" go
) >"$OUT" 2>"$ERR"
two_rc=$?
set -e
[ "$two_rc" -ne 0 ] && ok || bad "two-cli go must not CLOSED PASS (rc=$two_rc)"
mk_kind=
rk_kind=
if [ -f "$two/.wm/PANEL.tsv" ]; then
  mk_kind=$(awk -F '\t' '$1=="maker"{print $3; exit}' "$two/.wm/PANEL.tsv")
  rk_kind=$(awk -F '\t' '$1=="reviewer"{print $3; exit}' "$two/.wm/PANEL.tsv")
fi
if [ -n "$mk_kind" ] && [ -n "$rk_kind" ] && [ "$mk_kind" != "$rk_kind" ]; then
  ok
else
  bad "two CLIs wanted maker kind != reviewer kind, got make=$mk_kind rev=$rk_kind panel=$(cat "$two/.wm/PANEL.tsv" 2>/dev/null || echo ABSENT)"
fi

# Guided adopt lines stay without --working-mode.
if grep -E 'adopt work --managed' "$HERE/START.md" | grep -q -- '--working-mode'; then
  bad 'START.md adopt line must not have --working-mode'
else
  ok
fi
if grep -E 'adopt work --managed' "$HERE/BOOTSTRAP.md" | grep -q -- '--working-mode'; then
  bad 'BOOTSTRAP.md adopt line must not have --working-mode'
else
  ok
fi

# Discovery skill + travelling card.
if [ -f "$HERE/skills/working-mode/SKILL.md" ]; then
  ok
else
  bad 'skills/working-mode/SKILL.md missing'
fi
if [ -f "$HERE/skills/working-mode/SKILL.md" ]; then
  if grep -q '^name: working-mode$' "$HERE/skills/working-mode/SKILL.md" \
    && grep -E -q '^description:.*(wm\.sh|wm.sh)' "$HERE/skills/working-mode/SKILL.md" \
    && grep -E '^description:' "$HERE/skills/working-mode/SKILL.md" | grep -q 'go' \
    && grep -E '^description:' "$HERE/skills/working-mode/SKILL.md" | grep -q 'working-mode' \
    && grep -E '^description:' "$HERE/skills/working-mode/SKILL.md" | grep -q 'build'; then
    ok
  else
    bad 'skills/working-mode/SKILL.md YAML must name working-mode and mention wm.sh, go, working-mode, build'
  fi
  if grep -F -q '.crucible/*/wm.sh' "$HERE/skills/working-mode/SKILL.md"; then
    ok
  else
    bad 'working-mode skill body must tell the harness to run .crucible/*/wm.sh'
  fi
fi
if [ -f "$HERE/skills/working-mode/CONTRACT.md" ] \
  && grep -q 'Replacing this directory must not require editing wm.sh' \
    "$HERE/skills/working-mode/CONTRACT.md"; then
  ok
else
  bad 'skills/working-mode/CONTRACT.md must include Swap: replacing directory must not require editing wm.sh'
fi
if [ -f "$HERE/WORKING-MODE.md" ]; then
  ok
else
  bad 'WORKING-MODE.md missing'
fi
if [ -f "$HERE/WORKING-MODE.md" ]; then
  wm_lines=$(wc -l < "$HERE/WORKING-MODE.md")
  if [ "$wm_lines" -le 80 ]; then
    ok
  else
    bad "WORKING-MODE.md has $wm_lines lines (want <=80)"
  fi
  if grep -E '^wm[[:space:]]' "$HERE/WORKING-MODE.md" >/dev/null; then
    bad 'WORKING-MODE.md must not have ^wm  lines'
  else
    ok
  fi
  if grep -q '&' "$HERE/WORKING-MODE.md"; then
    bad 'WORKING-MODE.md must not contain &'
  else
    ok
  fi
  if grep -qi 'drive' "$HERE/WORKING-MODE.md" \
    && grep -qi 'MAP-HUMAN' "$HERE/WORKING-MODE.md" \
    && grep -qi 'STOP-ASK' "$HERE/WORKING-MODE.md"; then
    ok
  else
    bad 'WORKING-MODE.md must mention drive, MAP-HUMAN, STOP-ASK'
  fi
fi

# wm.sh sources wm-go.sh; go/help die refresh from 1.8.0 when missing.
if grep -q 'wm-go.sh' "$WM" && grep -q 'WM_GO' "$WM"; then
  ok
else
  bad 'wm.sh must source wm-go.sh'
fi
if grep -q 'refresh from 1.8.0' "$WM"; then
  ok
else
  bad 'wm.sh must die refresh from 1.8.0 when go/help are missing'
fi
if grep -q 'wm-go.sh' "$HERE/crucible" && grep -q 'WORKING-MODE.md' "$HERE/crucible"; then
  ok
else
  bad 'adopt_install_working_mode must copy wm-go.sh and WORKING-MODE.md'
fi

home_leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
if [ -z "$home_leftovers" ]; then
  ok
else
  bad "wrote under HOME: $home_leftovers"
fi

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
