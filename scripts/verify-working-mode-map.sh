#!/bin/sh
# Working-mode battery contracts + identity CHECKs (10c, 7c, 8b, 11c, 14a).
# Fixture git repos and echo/sh stubs only. No harness CLIs. No $HOME skills.
# Falsifier-first: stub CONTRACT/SKILL bodies and missing kernel identity
# hooks must fail must-write / refuse CHECKs (RED) before the batteries are filled.
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
WM="$HERE/wm.sh"
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }

if [ ! -f "$WM" ]; then
  printf 'RED wm.sh missing (working-mode kernel not present)\n' >&2
  exit 1
fi
chmod +x "$WM" 2>/dev/null || true
export WM_ENGINE="$WM"

BATTERIES='architecture critique review loop-design'

EMPTY_HOME=$(mktemp -d "${TMPDIR:-/tmp}/wm-map-empty-home.XXXXXX")
BASE=$(mktemp -d "${TMPDIR:-/tmp}/wm-map-verify.XXXXXX")
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

sh -n "$HERE/scripts/verify-working-mode-map.sh" && ok || bad 'verify-working-mode-map.sh is not valid POSIX sh'
sh -n "$WM" && ok || bad 'wm.sh is not valid POSIX sh'

section_body() {
  _sb_h=$1
  _sb_file=$2
  awk -v h="$_sb_h" '
    $0 == h { p=1; next }
    /^## / { p=0 }
    p { print }
  ' "$_sb_file"
}

body_has_substance() {
  section_body "$1" "$2" | awk '
    NF && $0 != "(not yet ported)" && $0 !~ /not yet ported/ { found=1 }
    END { exit found ? 0 : 1 }
  '
}

require_file() {
  if [ -f "$1" ]; then
    ok
  else
    bad "$2"
  fi
}

require_grep() {
  _rg_file=$1
  _rg_pat=$2
  _rg_label=$3
  if [ -f "$_rg_file" ] && grep -q "$_rg_pat" "$_rg_file"; then
    ok
  else
    bad "$_rg_label"
  fi
}

require_fgrep() {
  _rf_file=$1
  _rf_pat=$2
  _rf_label=$3
  if [ -f "$_rf_file" ] && grep -F -q "$_rf_pat" "$_rf_file"; then
    ok
  else
    bad "$_rf_label"
  fi
}

forbid_grep() {
  _fg_file=$1
  _fg_pat=$2
  _fg_label=$3
  if [ -f "$_fg_file" ] && grep -E -q "$_fg_pat" "$_fg_file"; then
    bad "$_fg_label"
  else
    ok
  fi
}

expect() {
  label=$1
  pattern=$2
  shift 2
  if ! out=$("$@" 2>"$ERR"); then
    err=$(cat "$ERR" 2>/dev/null || true)
    bad "$label: command refused: $out $err"
    return
  fi
  if printf '%s\n' "$out" | grep -E -q "$pattern"; then
    ok
  else
    bad "$label: wanted $pattern, got $out"
  fi
}

refuses() {
  label=$1
  pattern=$2
  shift 2
  if out=$("$@" 2>"$ERR"); then
    bad "$label: command accepted: $out"
    return
  fi
  err=$(cat "$ERR" 2>/dev/null || true)
  if printf '%s\n%s\n' "$out" "$err" | grep -E -q "$pattern"; then
    ok
  else
    bad "$label: wanted $pattern, got out=$out err=$err"
  fi
}

impl_ok() {
  label=$1
  shift
  if out=$("$@" 2>"$ERR"); then
    printf '%s\n' "$out" >"$OUT"
    ok
    return 0
  fi
  err=$(cat "$ERR" 2>/dev/null || true)
  if printf '%s\n' "$err" | grep -q 'unknown command'; then
    bad "$label: kernel hook missing ($err)"
  else
    bad "$label: refused: $err"
  fi
  return 1
}

# ---------------------------------------------------------------------------
# Contract tests: stub "not yet ported" / empty Out bodies fail must-write.
# ---------------------------------------------------------------------------
HEADINGS='## In
## Out (must-write paths / words)
## Must-not
## Swap'

for name in $BATTERIES; do
  skill="$HERE/skills/$name/SKILL.md"
  contract="$HERE/skills/$name/CONTRACT.md"
  require_file "$skill" "package skills/$name/SKILL.md missing"
  require_file "$contract" "package skills/$name/CONTRACT.md missing"

  forbid_grep "$skill" 'not yet ported' "$name SKILL.md still not yet ported"
  forbid_grep "$contract" 'not yet ported' "$name CONTRACT.md still not yet ported"
  forbid_grep "$skill" 'engos-' "$name SKILL.md contains engos- name"
  forbid_grep "$contract" 'engos-' "$name CONTRACT.md contains engos- name"

  printf '%s\n' "$HEADINGS" | while IFS= read -r h; do
    [ -n "$h" ] || continue
    if [ -f "$contract" ] && grep -F -q "$h" "$contract"; then
      printf 'HEADING_OK\n'
    else
      printf 'HEADING_BAD %s\n' "$h"
    fi
  done >"$BASE/headings.$name"
  if grep -q '^HEADING_BAD' "$BASE/headings.$name"; then
    bad "$name CONTRACT.md missing required heading"
  else
    ok
  fi

  if [ -f "$contract" ]; then
    if body_has_substance '## In' "$contract"; then
      ok
    else
      bad "$name CONTRACT.md ## In is empty/placeholder"
    fi
    if body_has_substance '## Out (must-write paths / words)' "$contract"; then
      ok
    else
      bad "$name CONTRACT.md ## Out (must-write paths / words) is empty/placeholder"
    fi
    if body_has_substance '## Must-not' "$contract"; then
      ok
    else
      bad "$name CONTRACT.md ## Must-not is empty/placeholder"
    fi
    swap=$(section_body '## Swap' "$contract")
    printf '%s\n' "$swap" | grep -F -q 'Replacing this directory must not require editing wm.sh.' \
      && ok || bad "$name CONTRACT.md ## Swap missing verbatim swap line"
  fi
done

# Architecture must-write / must-not (7c, 14a)
require_fgrep "$HERE/skills/architecture/CONTRACT.md" 'architecture/modules.md' \
  'architecture CONTRACT Out must name architecture/modules.md'
require_fgrep "$HERE/skills/architecture/CONTRACT.md" 'MAP.md' \
  'architecture CONTRACT Out must name MAP.md'
require_fgrep "$HERE/skills/architecture/CONTRACT.md" 'MAPPER' \
  'architecture CONTRACT Out must name MAPPER id'
require_fgrep "$HERE/skills/architecture/CONTRACT.md" 'CHANGES-ARCHITECTURE' \
  'architecture CONTRACT must name CHANGES-ARCHITECTURE'
require_fgrep "$HERE/skills/architecture/CONTRACT.md" 'MAP-ACCEPT' \
  'architecture CONTRACT Must-not must mention MAP-ACCEPT'
require_fgrep "$HERE/skills/architecture/SKILL.md" 'architecture/modules.md' \
  'architecture SKILL.md must name architecture/modules.md'
require_fgrep "$HERE/skills/architecture/SKILL.md" 'RULE 26' \
  'architecture SKILL.md must name RULE 26'
require_fgrep "$HERE/skills/architecture/SKILL.md" 'record-mapper' \
  'architecture SKILL.md must record mapper via wm record-mapper'
require_fgrep "$HERE/skills/architecture/SKILL.md" 'greenfield' \
  'architecture SKILL.md must allow greenfield empty packages then fit'
require_fgrep "$HERE/skills/architecture/SKILL.md" 'QUESTIONS.md' \
  'architecture SKILL.md must send two packagings to QUESTIONS.md'
require_fgrep "$HERE/skills/architecture/SKILL.md" 'brownfield' \
  'architecture SKILL.md must refuse brownfield silent mkdir of a new package'
require_fgrep "$HERE/skills/architecture/CONTRACT.md" 'silent new top-level package' \
  'architecture CONTRACT must refuse silent new top-level package without QUESTIONS'

# Critique: invert + adversarial + simple only; map words; no self-ACCEPT (8b)
require_fgrep "$HERE/skills/critique/CONTRACT.md" 'MAP-ACCEPT' \
  'critique CONTRACT Out must name MAP-ACCEPT'
require_fgrep "$HERE/skills/critique/CONTRACT.md" 'MAP-REVISE' \
  'critique CONTRACT Out must name MAP-REVISE'
require_fgrep "$HERE/skills/critique/CONTRACT.md" 'MAP-STOP-ASK' \
  'critique CONTRACT Out must name MAP-STOP-ASK'
require_fgrep "$HERE/skills/critique/CONTRACT.md" 'a map it authored' \
  'critique CONTRACT Must-not must refuse MAP-ACCEPT on a map it authored'
require_fgrep "$HERE/skills/critique/CONTRACT.md" '/full' \
  'critique CONTRACT Must-not must skip /full'
require_fgrep "$HERE/skills/critique/CONTRACT.md" '/grade' \
  'critique CONTRACT Must-not must skip /grade'
require_fgrep "$HERE/skills/critique/CONTRACT.md" '/ult' \
  'critique CONTRACT Must-not must skip /ult'
require_fgrep "$HERE/skills/critique/CONTRACT.md" 'word	card	cap	andon' \
  'critique CONTRACT Send-back must include word/card/cap/andon TSV'
require_fgrep "$HERE/skills/critique/CONTRACT.md" 'ESCALATE MAP_REVISE' \
  'critique CONTRACT MAP-REVISE andon must be ESCALATE MAP_REVISE'
require_fgrep "$HERE/skills/review/CONTRACT.md" 'word	card	cap	andon' \
  'review CONTRACT Send-back must include word/card/cap/andon TSV'
require_grep "$HERE/skills/critique/SKILL.md" '[Ii]nvert' \
  'critique SKILL.md must include invert'
require_grep "$HERE/skills/critique/SKILL.md" '[Aa]dversarial' \
  'critique SKILL.md must include adversarial'
require_grep "$HERE/skills/critique/SKILL.md" '[Ss]imple' \
  'critique SKILL.md must include simple'
require_fgrep "$HERE/skills/critique/SKILL.md" 'CLOSED PASS' \
  'critique SKILL.md must refuse CLOSED PASS as a map word'
require_fgrep "$HERE/skills/critique/SKILL.md" 'check-map-word' \
  'critique SKILL.md must use wm check-map-word'

# Review: code/testing lenses; re-run falsifier; map words are not CLOSED PASS
require_grep "$HERE/skills/review/CONTRACT.md" '[Cc]ode' \
  'review CONTRACT must name code lens'
require_grep "$HERE/skills/review/CONTRACT.md" '[Tt]esting' \
  'review CONTRACT must name testing lens'
require_grep "$HERE/skills/review/CONTRACT.md" '[Ff]alsifier' \
  'review CONTRACT must name falsifier'
require_grep "$HERE/skills/review/SKILL.md" 're-run' \
  'review SKILL.md must require re-run of the named falsifier'
require_fgrep "$HERE/skills/review/SKILL.md" 'CLOSED PASS' \
  'review SKILL.md must say map words are not CLOSED PASS'
require_fgrep "$HERE/skills/review/SKILL.md" 'Do not write owned product paths' \
  'review SKILL.md Must-not must refuse writing owned product paths'
require_fgrep "$HERE/skills/review/SKILL.md" 'extra-proof' \
  'review SKILL.md must name extra-proof'

# Loop-design: craft/audit/debrief only — not the delivery walker
require_grep "$HERE/skills/loop-design/CONTRACT.md" '[Cc]raft' \
  'loop-design CONTRACT must name craft'
require_grep "$HERE/skills/loop-design/CONTRACT.md" '[Aa]udit' \
  'loop-design CONTRACT must name audit'
require_grep "$HERE/skills/loop-design/CONTRACT.md" '[Dd]ebrief' \
  'loop-design CONTRACT must name debrief'
require_fgrep "$HERE/skills/loop-design/CONTRACT.md" 'Ready' \
  'loop-design CONTRACT Out must name Ready'
require_fgrep "$HERE/skills/loop-design/CONTRACT.md" 'Repair needed' \
  'loop-design CONTRACT Out must name Repair needed'
require_fgrep "$HERE/skills/loop-design/CONTRACT.md" 'Not actually a loop' \
  'loop-design CONTRACT Out must name Not actually a loop'
require_fgrep "$HERE/skills/loop-design/CONTRACT.md" 'Loop Library' \
  'loop-design CONTRACT Must-not must skip Loop Library'
if [ -f "$HERE/skills/loop-design/SKILL.md" ] && \
  grep -E -q 'delivery walker|wm loop' "$HERE/skills/loop-design/SKILL.md"; then
  ok
else
  bad 'loop-design SKILL.md must not be the delivery walker'
fi

# Swap: wm.sh must not hardcode battery directories (11c)
if grep -E -q 'skills/(architecture|critique|review|loop-design)' "$WM"; then
  bad 'wm.sh hardcodes battery paths; replacing a directory would require editing wm.sh'
else
  ok
fi

# Optional repo-scout battery (ROUTING required=no).
require_file "$HERE/skills/repo-scout/SKILL.md" 'package skills/repo-scout/SKILL.md missing'
require_file "$HERE/skills/repo-scout/CONTRACT.md" 'package skills/repo-scout/CONTRACT.md missing'
require_fgrep "$HERE/skills/repo-scout/CONTRACT.md" 'REPO.md' \
  'repo-scout CONTRACT Out must name REPO.md'
require_fgrep "$HERE/skills/repo-scout/CONTRACT.md" 'SPEC.md' \
  'repo-scout CONTRACT Must-not must mention SPEC.md'
require_fgrep "$HERE/skills/repo-scout/CONTRACT.md" 'MAP-ACCEPT' \
  'repo-scout CONTRACT Must-not must mention MAP-ACCEPT'
require_fgrep "$HERE/skills/repo-scout/CONTRACT.md" 'CLOSED PASS' \
  'repo-scout CONTRACT Must-not must mention CLOSED PASS'
require_fgrep "$HERE/skills/repo-scout/SKILL.md" 'REPO.md' \
  'repo-scout SKILL.md must name REPO.md'
require_fgrep "$HERE/skills/repo-scout/SKILL.md" 'SPEC.md' \
  'repo-scout SKILL.md must refuse SPEC.md on this pass'
if [ -f "$HERE/skills/repo-scout/CONTRACT.md" ]; then
  swap=$(section_body '## Swap' "$HERE/skills/repo-scout/CONTRACT.md")
  printf '%s\n' "$swap" | grep -F -q 'Replacing this directory must not require editing wm.sh.' \
    && ok || bad 'repo-scout CONTRACT.md ## Swap missing verbatim swap line'
fi
if awk -F '\t' '$1=="REPO" && $3=="repo-scout" && $6=="no" { found=1 } END { exit !found }' \
  "$HERE/ROUTING.tsv"; then
  ok
else
  bad 'ROUTING.tsv missing REPO inventory repo-scout specifier spec required=no'
fi

# Optional research battery (ROUTING required=no).
require_file "$HERE/skills/research/SKILL.md" 'package skills/research/SKILL.md missing'
require_file "$HERE/skills/research/CONTRACT.md" 'package skills/research/CONTRACT.md missing'
require_fgrep "$HERE/skills/research/CONTRACT.md" 'RESEARCH.md' \
  'research CONTRACT Out must name RESEARCH.md'
require_fgrep "$HERE/skills/research/CONTRACT.md" 'MAP-ACCEPT' \
  'research CONTRACT Must-not must mention MAP-ACCEPT'
require_fgrep "$HERE/skills/research/CONTRACT.md" 'CLOSED PASS' \
  'research CONTRACT Must-not must mention CLOSED PASS'
require_fgrep "$HERE/skills/research/CONTRACT.md" 'FALSIFIER' \
  'research CONTRACT Must-not must mention FALSIFIER'
require_fgrep "$HERE/skills/research/SKILL.md" 'RESEARCH.md' \
  'research SKILL.md must name RESEARCH.md'
require_fgrep "$HERE/skills/research/SKILL.md" 'SPEC.md' \
  'research SKILL.md must refuse SPEC.md on this pass'
if [ -f "$HERE/skills/research/CONTRACT.md" ]; then
  swap=$(section_body '## Swap' "$HERE/skills/research/CONTRACT.md")
  printf '%s\n' "$swap" | grep -F -q 'Replacing this directory must not require editing wm.sh.' \
    && ok || bad 'research CONTRACT.md ## Swap missing verbatim swap line'
fi
if awk -F '\t' '$1=="RESEARCH" && $3=="research" && $6=="no" { found=1 } END { exit !found }' \
  "$HERE/ROUTING.tsv"; then
  ok
else
  bad 'ROUTING.tsv missing RESEARCH row with required=no'
fi

# Swap: wrapper must not hardcode battery directories (already checked via $WM).
if grep -q 'WM_WRAPPER' "$WM" && grep -q 'exec' "$WM"; then
  ok
else
  bad 'wm.sh must be an exec wrapper after rust cut-over'
fi
if grep -E -q '^cmd_map_verdict\(|^cmd_map_ready\(|^extract_fn' "$WM"; then
  bad 'wm.sh must not contain dumped map kernel bodies'
else
  ok
fi

require_file "$HERE/docs/working-mode.md" 'docs/working-mode.md missing'
require_fgrep "$HERE/docs/working-mode.md" 'MAP-HUMAN' \
  'docs/working-mode.md must name MAP-HUMAN (8c)'
require_fgrep "$HERE/docs/working-mode.md" 'SIGNED:' \
  'docs/working-mode.md must name SIGNED: as the human sign key'
require_fgrep "$HERE/docs/working-mode.md" 'SHA256:' \
  'docs/working-mode.md must name SHA256: as the MAP-HUMAN hash-bind key'
require_fgrep "$HERE/docs/working-mode.md" 'MAP-SHA256:' \
  'docs/working-mode.md must name MAP-SHA256: as the SHA256 alias'
require_fgrep "$HERE/docs/working-mode.md" 'not mapper' \
  'docs/working-mode.md must refuse mapper/maker/reviewer as SIGNED'
require_fgrep "$HERE/docs/working-mode.md" 'slices.tsv' \
  'docs/working-mode.md must name slices.tsv (6b)'
require_fgrep "$HERE/docs/working-mode.md" 'STOP-ASK' \
  'docs/working-mode.md must name STOP-ASK'
if [ -f "$HERE/docs/working-mode.md" ] && grep -q 'fake CROSS-FAMILY' "$HERE/docs/working-mode.md"; then
  ok
else
  bad 'docs/working-mode.md must refuse fake CROSS-FAMILY (3d is label+ROUTING)'
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

sha256_file() {
  f=$1
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$f" | awk '{print $1}'
  else
    openssl dgst -sha256 "$f" | awk '{print $NF}'
  fi
}

init_git_product() {
  dir=$1
  mkdir -p "$dir"
  (
    CDPATH=
    cd "$dir"
    git init -q
    git config user.email 'wm@local'
    git config user.name 'working-mode'
    printf 'product\n' > README
    git add README
    git commit -qm init
  )
}

# HIGH MAP without MAP-HUMAN: rust STOP-ASK MAP-HUMAN (beats injected red).
map_dir="$BASE/map-human"
init_git_product "$map_dir"
CDPATH=; cd "$map_dir"
printf 'receipt\n' > IDEA.md
printf 'id\tmodule\towned_paths\tdepends_on\trisk\tstatus\n' > slices.tsv
printf 's1\twidget\tsrc/widget/api.py\t-\tHIGH\tREADY\n' >> slices.tsv
printf 'slice s1 is HIGH\n' > MAP.md
set +e
CRUCIBLE_RED_PROGRAM=/bin/sh
CRUCIBLE_RED_ARGS='-c
echo FAIL > .wm/FALSIFIER; pwd > marker'
export CRUCIBLE_RED_PROGRAM CRUCIBLE_RED_ARGS
"$WM" go >"$OUT" 2>"$ERR"
rc=$?
set -e
if [ "$rc" -ne 0 ] && grep -q 'STOP-ASK MAP-HUMAN' "$OUT"; then
  ok
else
  bad "HIGH without MAP-HUMAN wanted STOP-ASK MAP-HUMAN, rc=$rc out=$(cat "$OUT") err=$(cat "$ERR")"
fi
if [ -f .wm/FLOOR.md ] && grep -q 'card: STOP-ASK MAP-HUMAN' .wm/FLOOR.md; then
  ok
else
  bad 'FLOOR must be STOP-ASK MAP-HUMAN'
fi
if [ -f marker ] || [ -f .wm/FALSIFIER ]; then
  bad 'injected red must not run when MAP-HUMAN is missing'
else
  ok
fi
unset CRUCIBLE_RED_PROGRAM CRUCIBLE_RED_ARGS
cd "$HERE"

# Valid MAP-HUMAN continues to injected NEXT RED.
map_ok="$BASE/map-ok"
init_git_product "$map_ok"
CDPATH=; cd "$map_ok"
printf 'receipt\n' > IDEA.md
printf 'id\tmodule\towned_paths\tdepends_on\trisk\tstatus\n' > slices.tsv
printf 's1\twidget\tsrc/widget/api.py\t-\tHIGH\tREADY\n' >> slices.tsv
printf 'slice s1 is HIGH\n' > MAP.md
h=$(sha256_file MAP.md)
printf 'SIGNED: operator\nMAP: MAP.md\nSHA256: %s\n' "$h" > MAP-HUMAN
set +e
CRUCIBLE_RED_PROGRAM=/bin/sh
CRUCIBLE_RED_ARGS='-c
echo FAIL > .wm/FALSIFIER; pwd > marker'
export CRUCIBLE_RED_PROGRAM CRUCIBLE_RED_ARGS
"$WM" go >"$OUT" 2>"$ERR"
rc=$?
set -e
if [ "$rc" -eq 0 ] && grep -q 'NEXT RED' "$OUT"; then
  ok
else
  bad "valid MAP-HUMAN wanted NEXT RED rc=0, rc=$rc out=$(cat "$OUT") err=$(cat "$ERR")"
fi
unset CRUCIBLE_RED_PROGRAM CRUCIBLE_RED_ARGS
cd "$HERE"

home_leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
if [ -z "$home_leftovers" ]; then
  ok
else
  bad "wrote under HOME: $home_leftovers"
fi

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
