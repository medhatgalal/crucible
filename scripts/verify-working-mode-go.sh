#!/bin/sh
# 1.8.0 Task 1: wm go + no-args help + discovery skill.
# Empty HOME. Fixture git repos and fake grok/kiro-cli. Do not require fake claude.
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

if grep -q '{SESSION}' "$HERE/wm-go.sh" \
  && grep -q -- '--session-id' "$HERE/wm-go.sh"; then
  ok
else
  bad 'wm-go.sh grok command must pass --session-id {SESSION}'
fi
if grep -q -- '-p --prompt-file' "$HERE/wm-go.sh"; then
  bad 'grok -p is --single PROMPT; discover must use --prompt-file without -p'
else
  ok
fi
if grep -q -- '--prompt-file {BRIEF}' "$HERE/wm-go.sh" \
  && grep -q -- '--always-approve' "$HERE/wm-go.sh"; then
  ok
else
  bad 'wm-go.sh grok discover must use --always-approve --prompt-file {BRIEF}'
fi

# 1.14 live walk source contract (no live CLIs).
# 1.14.1: kiro probe/exec inherit HOST_HOME (keychain OIDC). Empty HOME hang
# is not logout. Do not exec kiro-cli acp as wm argv.
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
  && grep -q 'commands: go status help' "$OUT" \
  && grep -q 'harness: read WORKING-MODE.md then go' "$OUT"; then
  ok
else
  bad "no-args missing help tokens, got out=$(cat "$OUT")"
fi
if grep -q 'debug: init cast loop bound next' "$OUT"; then
  ok
else
  bad "no-args wanted debug verbs, got out=$(cat "$OUT")"
fi

# No CLI workers: INDEPENDENCE_UNAVAILABLE, nonzero.
nocli="$BASE/nocli"
init_git_repo "$nocli"
printf 'an idea\n' > "$nocli/IDEA.md"
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

# Fake kiro-cli only: discovered as kind kiro. PATH-stripped already UNAVAILABLE.
BIN_KIRO="$BASE/bin-kiro"
mkdir -p "$BIN_KIRO"
printf '#!/bin/sh\nexit 0\n' > "$BIN_KIRO/kiro-cli"
chmod +x "$BIN_KIRO/kiro-cli"
konly="$BASE/fake-kiro"
init_git_repo "$konly"
printf 'build a tiny product\n' > "$konly/IDEA.md"
set +e
(
  CDPATH=
  cd "$konly"
  PATH="$BIN_KIRO:/usr/bin:/bin"
  export PATH
  "$WM" go
) >"$OUT" 2>"$ERR"
konly_rc=$?
set -e
[ "$konly_rc" -ne 0 ] && ok || bad "fake kiro-cli go must STOP-ASK (rc=$konly_rc)"
k_kind=
k_spec=
k_make=
if [ -f "$konly/.wm/PANEL.tsv" ]; then
  k_kind=$(awk -F '\t' '$1=="specifier"{print $3; exit}' "$konly/.wm/PANEL.tsv")
  k_spec=$(awk -F '\t' '$1=="specifier"{print $2; exit}' "$konly/.wm/PANEL.tsv")
  k_make=$(awk -F '\t' '$1=="maker"{print $2; exit}' "$konly/.wm/PANEL.tsv")
fi
if [ "$k_kind" = kiro ] && [ "$k_spec" = spec0 ] && [ "$k_make" = make0 ] \
  && [ "$k_spec" != "$k_make" ]; then
  ok
else
  bad "fake kiro-cli wanted kind kiro spec0!=make0, got kind=$k_kind spec=$k_spec make=$k_make panel=$(cat "$konly/.wm/PANEL.tsv" 2>/dev/null || echo ABSENT)"
fi

# Two CLIs: grok + kiro-cli; maker kind != reviewer kind. Do not require fake claude.
printf '#!/bin/sh\nexit 0\n' > "$BIN/kiro-cli"
chmod +x "$BIN/kiro-cli"
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
case $mk_kind$rk_kind in
  *kiro*) ok ;;
  *) bad "two CLIs wanted kiro as a kind, got make=$mk_kind rev=$rk_kind" ;;
esac

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
  # S1: start path is go/status, not "then loop".
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

# Arena POSIX: sidecar flag, $0 resolve, quoted prompts, ./IDEA.md, no type cmd_go.
if grep -q 'WM_GO_LOADED=1' "$HERE/wm-go.sh"; then
  ok
else
  bad 'wm-go.sh must set WM_GO_LOADED=1'
fi
if grep -q 'WM_GO_LOADED' "$WM"; then
  ok
else
  bad 'wm.sh must test WM_GO_LOADED (not type cmd_go)'
fi
if grep -E 'type[[:space:]]+cmd_go' "$WM" >/dev/null; then
  bad 'wm.sh must not use type cmd_go to detect sidecar'
else
  ok
fi
if grep -q '_wm_self' "$WM" && grep -F 'command -v "$_wm_self"' "$WM" >/dev/null; then
  ok
else
  bad 'wm.sh must resolve wm-go.sh via command -v when $0 has no slash'
fi
if grep -F "kiro-cli chat --no-interactive --trust-all-tools 'read {BRIEF} and follow it exactly'" \
  "$HERE/wm-go.sh" >/dev/null \
  && grep -F "codex exec -- 'read {BRIEF} and follow it exactly'" \
  "$HERE/wm-go.sh" >/dev/null; then
  ok
else
  bad 'wm-go.sh Kiro/Codex templates must keep adapter single quotes'
fi
# S0: discover loop is grok kiro-cli codex only (no claude).
if grep -E 'for _go_cli in grok kiro-cli codex; do' "$HERE/wm-go.sh" >/dev/null; then
  ok
else
  bad 'discover loop must be grok kiro-cli codex'
fi
if awk '/for _go_cli in / { print; exit }' "$HERE/wm-go.sh" | grep -q claude; then
  bad 'S0 discover loop must not include claude'
else
  ok
fi
if grep -q 'kiro-cli' "$HERE/wm-go.sh"; then
  ok
else
  bad 'wm-go.sh must discover kiro-cli'
fi
if grep -q 'cp "$_go_idea" ./IDEA.md' "$HERE/wm-go.sh"; then
  ok
else
  bad 'go must cp idea onto ./IDEA.md'
fi
if grep -q 'go_ensure_panel' "$HERE/wm-go.sh"; then
  ok
else
  bad 'wm-go.sh must define go_ensure_panel'
fi

# Help program from $0 .crucible/<prog>/wm.sh; unmatched glob must not print *.
help_prog="$BASE/help-prog"
init_git_repo "$help_prog"
mkdir -p "$help_prog/.crucible/demo"
cp "$WM" "$help_prog/.crucible/demo/wm.sh"
cp "$HERE/wm-go.sh" "$help_prog/.crucible/demo/wm-go.sh"
chmod +x "$help_prog/.crucible/demo/wm.sh"
set +e
(
  CDPATH=
  cd "$help_prog"
  "$help_prog/.crucible/demo/wm.sh"
) >"$OUT" 2>"$ERR"
hp_rc=$?
set -e
[ "$hp_rc" -eq 0 ] && ok || bad "demo-program help wanted exit 0, got $hp_rc"
if grep -q 'run: .crucible/demo/wm.sh go' "$OUT"; then
  ok
else
  bad "help from \$0 wanted .crucible/demo/wm.sh, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi
if grep -q '\*' "$OUT"; then
  bad "help must not print literal glob *, got out=$(cat "$OUT")"
else
  ok
fi

# Idea path starting with - is refused.
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
[ "$dash_rc" -ne 0 ] && ok || bad "go -n must be nonzero"
if grep -q 'must not start with -' "$OUT" "$ERR"; then
  ok
else
  bad "go -n wanted idea path refuse, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi

# QUESTIONS only → STOP-ASK before panel; QUESTIONS+ANSWERS does not.
qonly="$BASE/q-only"
init_git_repo "$qonly"
printf 'an idea\n' > "$qonly/IDEA.md"
printf 'Who is the user?\n' > "$qonly/QUESTIONS.md"
set +e
(
  CDPATH=
  cd "$qonly"
  PATH=/usr/bin:/bin
  export PATH
  "$WM" go
) >"$OUT" 2>"$ERR"
q_rc=$?
set -e
[ "$q_rc" -ne 0 ] && ok || bad "QUESTIONS-only go must be nonzero"
if grep -q 'STOP-ASK QUESTIONS' "$OUT" "$ERR"; then
  ok
else
  bad "QUESTIONS-only go wanted STOP-ASK QUESTIONS, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi
if grep -q 'INDEPENDENCE_UNAVAILABLE' "$OUT" "$ERR"; then
  bad 'QUESTIONS-only go must stop before CLI discovery'
else
  ok
fi

qans="$BASE/q-ans"
init_git_repo "$qans"
printf 'an idea\n' > "$qans/IDEA.md"
printf 'Who is the user?\n' > "$qans/QUESTIONS.md"
printf 'A local hello file is enough.\n' > "$qans/ANSWERS.md"
set +e
(
  CDPATH=
  cd "$qans"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" go
) >"$OUT" 2>"$ERR"
qa_rc=$?
set -e
[ "$qa_rc" -ne 0 ] && ok || bad "QUESTIONS+ANSWERS go must not CLOSED PASS without SPEC"
if grep -q 'STOP-ASK QUESTIONS' "$OUT" "$ERR"; then
  bad "QUESTIONS+ANSWERS go must not STOP-ASK QUESTIONS, got out=$(cat "$OUT") err=$(cat "$ERR")"
else
  ok
fi

# Maker+reviewer valid, specifier/scout missing → still discover-cast those roles.
partial="$BASE/partial-panel"
init_git_repo "$partial"
printf 'an idea\n' > "$partial/IDEA.md"
set +e
(
  CDPATH=
  cd "$partial"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" init >/dev/null
  "$WM" cast maker make0 grok 'true' >/dev/null
  "$WM" cast reviewer rev0 grok 'true' >/dev/null
  "$WM" go
) >"$OUT" 2>"$ERR"
part_rc=$?
set -e
[ "$part_rc" -ne 0 ] && ok || bad "partial-panel go must not CLOSED PASS"
spec_id=
scout_id=
if [ -f "$partial/.wm/PANEL.tsv" ]; then
  spec_id=$(awk -F '\t' '$1=="specifier"{print $2; exit}' "$partial/.wm/PANEL.tsv")
  scout_id=$(awk -F '\t' '$1=="scout"{print $2; exit}' "$partial/.wm/PANEL.tsv")
fi
if [ "$spec_id" = spec0 ] && [ "$scout_id" = scout0 ]; then
  ok
else
  bad "go_ensure_panel wanted spec0+scout0, got spec=$spec_id scout=$scout_id panel=$(cat "$partial/.wm/PANEL.tsv" 2>/dev/null || echo ABSENT)"
fi

# Q1: go --next copies READY idea; second --next with open MAP.md dies.
nextbl="$BASE/go-next"
init_git_repo "$nextbl"
printf 'first backlog idea\n' > "$nextbl/idea1.md"
printf 'second backlog idea\n' > "$nextbl/idea2.md"
printf 'id\tsize\trisk\tidea_path\tstatus\n' > "$nextbl/BACKLOG.tsv"
printf 'b1\tsmall\tLOW\tidea1.md\tREADY\n' >> "$nextbl/BACKLOG.tsv"
printf 'b2\tsmall\tLOW\tidea2.md\tREADY\n' >> "$nextbl/BACKLOG.tsv"
set +e
(
  CDPATH=
  cd "$nextbl"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" go --next
) >"$OUT" 2>"$ERR"
gn_rc=$?
set -e
[ "$gn_rc" -ne 0 ] && ok || bad "go --next fake grok must not CLOSED PASS"
if grep -q 'first backlog idea' "$nextbl/IDEA.md"; then
  ok
else
  bad "go --next must copy idea1.md onto IDEA.md, got $(cat "$nextbl/IDEA.md" 2>/dev/null || echo ABSENT)"
fi
b1st=$(awk -F '\t' '$1=="b1"{print $5; exit}' "$nextbl/BACKLOG.tsv")
[ "$b1st" = INFLIGHT ] && ok || bad "go --next wanted b1 INFLIGHT, got $b1st"
printf 'MAPPER: x\n' > "$nextbl/MAP.md"
set +e
(
  CDPATH=
  cd "$nextbl"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" go --next
) >"$OUT" 2>"$ERR"
gn2_rc=$?
set -e
[ "$gn2_rc" -ne 0 ] && ok || bad 'second go --next with open MAP must be nonzero'
if grep -q 'finish current map first' "$OUT" "$ERR"; then
  ok
else
  bad "second go --next wanted finish current map first, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi
if grep -q 'second backlog idea' "$nextbl/IDEA.md"; then
  bad 'second go --next must not copy idea2 while MAP.md is open'
else
  ok
fi

# go without --next still copies a missing IDEA.md from the argument (unchanged).
plain="$BASE/go-plain"
init_git_repo "$plain"
printf 'plain idea\n' > "$plain/seed.md"
set +e
(
  CDPATH=
  cd "$plain"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" go seed.md
) >"$OUT" 2>"$ERR"
set -e
if [ -f "$plain/IDEA.md" ] && grep -q 'plain idea' "$plain/IDEA.md"; then
  ok
else
  bad "go without --next must still copy seed.md when IDEA.md is missing"
fi
if [ -f "$plain/BACKLOG.tsv" ]; then
  bad 'go without --next must not invent BACKLOG.tsv'
else
  ok
fi

# S2: two backlog rows, go with no IDEA acts like --next; second go refuses until close.
s2bl="$BASE/go-default-next"
init_git_repo "$s2bl"
printf 'first backlog idea\n' > "$s2bl/idea1.md"
printf 'second backlog idea\n' > "$s2bl/idea2.md"
printf 'id\tsize\trisk\tidea_path\tstatus\n' > "$s2bl/BACKLOG.tsv"
printf 'b1\tsmall\tLOW\tidea1.md\tREADY\n' >> "$s2bl/BACKLOG.tsv"
printf 'b2\tsmall\tLOW\tidea2.md\tREADY\n' >> "$s2bl/BACKLOG.tsv"
set +e
(
  CDPATH=
  cd "$s2bl"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" go
) >"$OUT" 2>"$ERR"
s2_rc=$?
set -e
[ "$s2_rc" -ne 0 ] && ok || bad "S2 go (no flags) fake grok must not CLOSED PASS"
if grep -q 'first backlog idea' "$s2bl/IDEA.md"; then
  ok
else
  bad "S2 go must copy idea1.md onto IDEA.md, got $(cat "$s2bl/IDEA.md" 2>/dev/null || echo ABSENT)"
fi
s2b1=$(awk -F '\t' '$1=="b1"{print $5; exit}' "$s2bl/BACKLOG.tsv")
[ "$s2b1" = INFLIGHT ] && ok || bad "S2 go wanted b1 INFLIGHT, got $s2b1"
printf 'MAPPER: x\n' > "$s2bl/MAP.md"
set +e
(
  CDPATH=
  cd "$s2bl"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" go
) >"$OUT" 2>"$ERR"
s2b_rc=$?
set -e
[ "$s2b_rc" -ne 0 ] && ok || bad 'S2 second go with open MAP must be nonzero'
if grep -q 'second backlog idea' "$s2bl/IDEA.md"; then
  bad 'S2 second go must not copy idea2 until close'
else
  ok
fi
s2b2=$(awk -F '\t' '$1=="b2"{print $5; exit}' "$s2bl/BACKLOG.tsv")
[ "$s2b2" = READY ] && ok || bad "S2 second go wanted b2 still READY, got $s2b2"

# S2+S3: init+go with no IDEA and no backlog → STOP-ASK INTAKE; FLOOR mentions INTAKE or ANDON.
s3in="$BASE/intake-floor"
init_git_repo "$s3in"
set +e
(
  CDPATH=
  cd "$s3in"
  PATH="$BIN:/usr/bin:/bin"
  export PATH
  "$WM" init >/dev/null
  "$WM" go
) >"$OUT" 2>"$ERR"
s3_rc=$?
set -e
[ "$s3_rc" -ne 0 ] && ok || bad "S3 go without IDEA must be nonzero"
if grep -q 'STOP-ASK INTAKE' "$OUT" "$ERR"; then
  ok
else
  bad "S3 wanted STOP-ASK INTAKE, got out=$(cat "$OUT") err=$(cat "$ERR")"
fi
if [ -f "$s3in/.wm/FLOOR.md" ] \
  && grep -E -q 'INTAKE|ANDON' "$s3in/.wm/FLOOR.md"; then
  ok
else
  bad "S3 FLOOR.md must exist and mention INTAKE or ANDON, got $(cat "$s3in/.wm/FLOOR.md" 2>/dev/null || echo ABSENT)"
fi
if [ -f "$s3in/.wm/TRACE.tsv" ] \
  && grep -q 'when	card	outcome' "$s3in/.wm/TRACE.tsv"; then
  ok
else
  bad "S3 TRACE.tsv must exist with header, got $(cat "$s3in/.wm/TRACE.tsv" 2>/dev/null || echo ABSENT)"
fi

# S3: status writes FLOOR.md and does not invent a FAIL count.
s3st="$BASE/status-floor"
init_git_repo "$s3st"
set +e
(
  CDPATH=
  cd "$s3st"
  "$WM" init >/dev/null
  "$WM" status
) >"$OUT" 2>"$ERR"
s3st_rc=$?
set -e
[ "$s3st_rc" -eq 0 ] && ok || bad "S3 status wanted exit 0, got $s3st_rc"
if [ -f "$s3st/.wm/FLOOR.md" ] \
  && grep -E -q 'INTAKE|ANDON|SHAPE' "$s3st/.wm/FLOOR.md"; then
  ok
else
  bad "S3 status FLOOR.md missing INTAKE/ANDON/SHAPE, got $(cat "$s3st/.wm/FLOOR.md" 2>/dev/null || echo ABSENT)"
fi
if [ -f "$s3st/.wm/review-fail-count" ]; then
  bad 'S3 status must not write review-fail-count'
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
