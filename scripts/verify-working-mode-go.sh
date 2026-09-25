#!/bin/sh
# Working-mode go CHECKs after rust cut-over. Empty HOME. No harness CLIs.
# Falsifier-first: wrapper must exec rust; go without IDEA is STOP-ASK INTAKE.
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
# shellcheck disable=SC1091
. "$HERE/scripts/cargo-env.sh"
GIT_CONFIG_GLOBAL=/dev/null
GIT_CONFIG_SYSTEM=/dev/null
export GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM

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

sh -n "$WM" && ok || bad 'wm.sh is not valid POSIX sh'
if grep -q 'WM_WRAPPER' "$WM" && grep -q 'exec' "$WM"; then
  ok
else
  bad 'wm.sh must exec the sibling crucible binary'
fi
if grep -E -q '^cmd_go\(|WM_GO_LOADED|source wm-go' "$WM"; then
  bad 'wm.sh must not contain dumped go sidecar internals'
else
  ok
fi

# Outer-loop /crucible battery (parent agent; not a judging station).
SKILL_CRUCIBLE="$HERE/skills/crucible/SKILL.md"
if [ -f "$SKILL_CRUCIBLE" ] \
  && grep -q 'name: crucible' "$SKILL_CRUCIBLE" \
  && grep -q '/crucible' "$SKILL_CRUCIBLE"; then
  ok
else
  bad 'skills/crucible/SKILL.md must exist for /crucible outer loop'
fi
if grep -q 'ask_user_question' "$SKILL_CRUCIBLE" \
  && grep -q 'request_user_input' "$SKILL_CRUCIBLE" \
  && grep -q 'Kiro CLI' "$SKILL_CRUCIBLE"; then
  ok
else
  bad 'skills/crucible/SKILL.md must name ask_user_question, request_user_input, and Kiro CLI fallback'
fi
if grep -F -q '.crucible/work/wm.sh go' "$SKILL_CRUCIBLE"; then
  ok
else
  bad 'skills/crucible/SKILL.md inner loop must remain .crucible/work/wm.sh go'
fi
if grep -qi 'you do not implement the product' "$SKILL_CRUCIBLE"; then
  ok
else
  bad 'skills/crucible/SKILL.md must say the coordinator does not implement the product'
fi
if grep -Fqi 'Grok-implement the product' "$SKILL_CRUCIBLE"; then
  bad 'skill must not tell agents to Grok-implement the product'
else
  ok
fi
if grep -Eiq 'do not use `/execute-plan` as the product walker' "$SKILL_CRUCIBLE"; then
  ok
else
  bad 'skills/crucible/SKILL.md must forbid /execute-plan as the product walker for adopted repos'
fi
LIVE_SH="$HERE/scripts/verify-working-mode-live.sh"
if [ -f "$LIVE_SH" ] && grep -F -q 'need >=1' "$LIVE_SH"; then
  ok
else
  bad 'live script must fail closed with need >=1'
fi
if [ -f "$LIVE_SH" ] && grep -q 'LIVE_N" -lt 1' "$LIVE_SH"; then
  ok
else
  bad 'live script unavailable gate must be LIVE_N -lt 1'
fi
if [ -f "$LIVE_SH" ] && grep -q 'SUBAGENT-ISOLATED' "$LIVE_SH"; then
  ok
else
  bad 'live script must name SUBAGENT-ISOLATED'
fi
if [ -f "$LIVE_SH" ] && grep -q 'CROSS-FAMILY' "$LIVE_SH"; then
  ok
else
  bad 'live script must name CROSS-FAMILY'
fi
if [ -f "$LIVE_SH" ] && grep -q 'need >=2' "$LIVE_SH"; then
  bad 'live script must not keep need >=2 as the unavailable gate'
else
  ok
fi
if [ -f "$LIVE_SH" ] && grep -q 'die_unavail "kiro-cli cannot auth"' "$LIVE_SH"; then
  bad 'one CLI auth failure must not abort the live walk'
else
  ok
fi
_hh_n=0
if [ -f "$LIVE_SH" ]; then
  _hh_n=$(grep -cF 'HOME="$HOST_HOME"' "$LIVE_SH" || true)
fi
if [ -f "$LIVE_SH" ] && grep -q '^export HOST_HOME$' "$LIVE_SH" \
  && grep -F -q 'HOME="$HOST_HOME"; export HOME' "$LIVE_SH" \
  && [ "$_hh_n" -ge 2 ]; then
  ok
else
  bad 'live kiro probe/exec must use HOST_HOME (keychain ACP credentials)'
fi
unset _hh_n
if [ -f "$LIVE_SH" ] && grep -E -q 'exec ("\$KIRO_BIN"|kiro-cli) acp' "$LIVE_SH"; then
  bad 'live kiro must not exec kiro-cli acp (JSON-RPC server)'
else
  ok
fi

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
  if awk 'NR<=20' "$HERE/WORKING-MODE.md" | grep -qi 'then loop'; then
    bad 'WORKING-MODE.md start path must not say then loop'
  else
    ok
  fi
  if grep -q 'status' "$HERE/WORKING-MODE.md"; then
    ok
  else
    bad 'WORKING-MODE.md must mention status'
  fi
fi
if grep -q 'status' "$HERE/skills/working-mode/SKILL.md"; then
  ok
else
  bad 'working-mode skill must mention status'
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
cp "$HERE/wm.sh" "$STAGE/wm.sh"
cp "$RUST" "$STAGE/crucible"
chmod +x "$STAGE/wm.sh" "$STAGE/crucible"
WM="$STAGE/wm.sh"
export WM_ENGINE="$WM"

ver=$("$WM" --version 2>"$ERR") || ver=
[ "$ver" = 1.20.0 ] && ok || bad "wrapper --version wanted 1.20.0 got $ver"

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
[ "$help_rc" -eq 0 ] && ok || bad "no-args wanted exit 0, got $help_rc"
if grep -q 'go' "$OUT" && grep -q 'status' "$OUT"; then
  ok
else
  bad "no-args rust help must list go/status, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi

intake="$BASE/intake"
init_git_repo "$intake"
set +e
(
  CDPATH=
  cd "$intake"
  "$WM" go
) >"$OUT" 2>"$ERR"
intake_rc=$?
set -e
if [ "$intake_rc" -ne 0 ] && grep -q 'STOP-ASK INTAKE' "$OUT"; then
  ok
else
  bad "go without IDEA wanted STOP-ASK INTAKE, got rc=$intake_rc out=$(cat "$OUT") err=$(cat "$ERR")"
fi

dash="$BASE/dash-idea"
init_git_repo "$dash"
set +e
(
  CDPATH=
  cd "$dash"
  "$WM" go -n
) >"$OUT" 2>"$ERR"
dash_rc=$?
set -e
if [ "$dash_rc" -ne 0 ]; then
  ok
else
  bad "go -n must refuse (idea path starting with -)"
fi

next="$BASE/gonext"
init_git_repo "$next"
set +e
(
  CDPATH=
  cd "$next"
  "$WM" go --next
) >"$OUT" 2>"$ERR"
next_rc=$?
set -e
if [ "$next_rc" -ne 0 ] && grep -q 'go --next is not ported' "$ERR" "$OUT"; then
  ok
else
  bad "go --next wanted not ported, got rc=$next_rc out=$(cat "$OUT") err=$(cat "$ERR")"
fi

home_leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
if [ -z "$home_leftovers" ]; then
  ok
else
  bad "wrote under HOME: $home_leftovers"
fi

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
