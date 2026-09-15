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

# ---------------------------------------------------------------------------
# Fixture helpers (echo/sh stubs). No Grok/Claude/Codex.
# ---------------------------------------------------------------------------
setup_map_repo() {
  name=$1
  mkdir -p "$BASE/$name"
  CDPATH= cd "$BASE/$name"
  git init -q
  git config user.email 'wm@local'
  git config user.name 'working-mode'
  printf 'an idea\n' > IDEA.md
  cat > INTENT.md <<'EOF'
## User
fixture operator
## Job
tiny widget
## Non-goals
live systems
EOF
  if ! "$WM" init >"$OUT" 2>"$ERR"; then
    printf 'FIXTURE BROKEN: %s init failed\n%s\n%s\n' "$name" "$(cat "$OUT")" "$(cat "$ERR")" >&2
    exit 1
  fi
  "$WM" cast coordinator parent grok - >"$OUT" 2>"$ERR" || true
}

write_architecture_fixture() {
  agent=${1:-alice}
  risk=${2:-LOW}
  live=${3:-no}
  mkdir -p src/widget tests/widget architecture tools
  printf '# widget api\nprint("widget")\n' > src/widget/api.py
  printf '# test widget\n' > tests/widget/test_api.py
  printf 'module_id\troot_path\tpublic_contracts\ttest_entrypoint\tpattern_instance\tlive_write\n' > architecture/modules.md
  printf 'widget\tsrc/widget\tsrc/widget/api.py\ttests/widget\tsrc/widget/api.py\t%s\n' "$live" >> architecture/modules.md
  printf 'MAPPER: %s\n\nid\tmodule\towned_paths\tdepends_on\trisk\ns1\twidget\tsrc/widget/api.py\t-\t%s\n' \
    "$agent" "$risk" > MAP.md
}

write_spec_fit() {
  cat > SPEC.md <<'EOF'
## Goal
extend the existing widget module
## Non-goals
new architecture
## Owned files
- src/widget/api.py
## Test files
- tests/widget/test_api.py
## Acceptance criteria
- widget still imports
## Focused falsifier
MAKER-WRITES
## Stop conditions
stop-ask on live write
## Risk
LOW
SPEC-AUTHOR: operator
EOF
}

# sh stub: architecture agent writes Plane A inventory + MAP, records mapper
write_arch_agent() {
  cat > tools/architecture-agent.sh <<'EOF'
#!/bin/sh
set -eu
agent=${AGENT:-alice}
mkdir -p src/widget tests/widget architecture
printf '# widget api\nprint("widget")\n' > src/widget/api.py
printf '# test widget\n' > tests/widget/test_api.py
cat > architecture/modules.md <<'MOD'
module_id	root_path	public_contracts	test_entrypoint	pattern_instance	live_write
widget	src/widget	src/widget/api.py	tests/widget	src/widget/api.py	no
MOD
cat > MAP.md <<MAP
MAPPER: $agent

id	module	owned_paths	depends_on	risk
s1	widget	src/widget/api.py	-	LOW
MAP
.wm/bin/wm record-mapper --from MAP.md
EOF
  chmod +x tools/architecture-agent.sh
}

# sh stub: critique agent writes invert+adversarial+simple and a map WORD
write_critique_agent() {
  cat > tools/critique-agent.sh <<'EOF'
#!/bin/sh
set -eu
agent=${AGENT:-bob}
word=${WORD:-MAP-ACCEPT}
mkdir -p reviews .wm/return
cat > reviews/critique.md <<'REV'
## Invert
Named map claims one module. Failure-first: a path outside widget would silently invent a second pattern.

## Adversarial
Attack: owned path not under a named module root. Residual risk: mapper later acting as maker.

## Simple
Complected: treating MAP-ACCEPT as CLOSED PASS. Keep map words distinct from brick close words.
REV
printf 'WORD: %s\nAGENT: %s\nMAP: MAP.md\n' "$word" "$agent" > ".wm/return/${agent}.md"
.wm/bin/wm check-map-word ".wm/return/${agent}.md"
EOF
  chmod +x tools/critique-agent.sh
}

# echo stub: loop-design note (craft/audit/debrief only)
write_loop_note() {
  mkdir -p loop-design
  printf '%s\n' \
    '## Craft' \
    'observe then choose then act then verify then record then stop' \
    '## Audit' \
    'Ready' \
    '## Debrief' \
    'proposals only; do not apply; do not run product; not the delivery walker' \
    > loop-design/NOTE.md
}

# Fixture maker: writes FALSIFIER in the foreground. No harness CLI.
write_maker_falsify_stub() {
  mkdir -p tools
  cat > tools/maker-falsify.sh <<'EOF'
#!/bin/sh
set -eu
mkdir -p .wm
printf 'test -f IDEA.md\n' > .wm/FALSIFIER
if command -v sha256sum >/dev/null 2>&1; then
  h=$(sha256sum .wm/FALSIFIER | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
  h=$(shasum -a 256 .wm/FALSIFIER | awk '{print $1}')
else
  h=$(openssl dgst -sha256 .wm/FALSIFIER | awk '{print $NF}')
fi
who=${MAKER_AGENT:-carol}
printf 'agent: %s\nwork-id: fixture\nsha256: %s\n' "$who" "$h" > .wm/FALSIFIER.meta
EOF
  chmod +x tools/maker-falsify.sh
}

write_map_return() {
  _wmr_agent=${1:-bob}
  _wmr_word=${2:-MAP-ACCEPT}
  mkdir -p .wm/return
  printf 'WORD: %s\nAGENT: %s\nMAP: MAP.md\n' "$_wmr_word" "$_wmr_agent" > ".wm/return/${_wmr_agent}.md"
}

cast_brick_panel() {
  _cbp_maker=${1:-carol}
  _cbp_rev=${2:-dave}
  _cbp_mkind=${3:-grok}
  _cbp_rkind=${4:-grok}
  write_maker_falsify_stub
  "$WM" cast maker "$_cbp_maker" "$_cbp_mkind" './tools/maker-falsify.sh' >"$OUT" 2>"$ERR"
  "$WM" cast reviewer "$_cbp_rev" "$_cbp_rkind" 'sh -c "echo reviewer {BRIEF}"' >"$OUT" 2>"$ERR"
}

# Fixture brick workers for a map-accepted slice walk (Task 6).
write_map_loop_brick() {
  mkdir -p tools
  cat > tools/map-loop-maker.sh <<'EOF'
#!/bin/sh
set -eu
role=
if [ -f .wm/dispatch ]; then
  role=$(awk -F ': ' '$1=="role"{print $2; exit}' .wm/dispatch)
fi
if [ -z "$role" ] && [ -n "${BRIEF:-}" ] && [ -f "$BRIEF" ]; then
  role=$(awk -F ': ' '$1=="role"{print $2; exit}' "$BRIEF")
fi
sha_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  fi
}
sid=s1
if [ -f .wm/slice-in-flight ]; then
  sid=$(awk -F ': ' '$1=="id"{print $2; exit}' .wm/slice-in-flight)
fi
[ -n "$sid" ] || sid=s1
marker="WM-SLICE-$sid"
if [ "$role" = maker-build ]; then
  printf '\n%s\n' "$marker" >> src/widget/api.py
  git add src/widget/api.py
  git commit -qm maker-build
  exit 0
fi
mkdir -p .wm
printf 'grep -q %s src/widget/api.py && test -d tests/widget\n' "$marker" > .wm/FALSIFIER
h=$(sha_of .wm/FALSIFIER)
wid=NOCOMMIT
if git rev-parse --verify HEAD >/dev/null 2>&1; then
  wid=$(git rev-parse --short=12 HEAD)
fi
printf 'agent: carol\nwork-id: %s\nsha256: %s\n' "$wid" "$h" > .wm/FALSIFIER.meta
EOF
  cat > tools/map-loop-reviewer.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return reviews
cmd=$(sed -n '1p' .wm/FALSIFIER)
ev=$(.wm/bin/wm evidence dave -- sh -c "$cmd")
cat > reviews/review.md <<'REV'
## Code
scope ok
## Testing
re-run named falsifier
REV
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/dave.md
EOF
  chmod +x tools/map-loop-maker.sh tools/map-loop-reviewer.sh
}

run_map_loop() {
  set +e
  "$WM" loop >"$OUT" 2>"$ERR"
  LOOP_RC=$?
  set -e
}

assert_map_loop_foreground() {
  _mlf=$1
  if pgrep -f "$WM loop" >/dev/null 2>&1; then
    bad "$_mlf: leftover wm.sh loop process"
  else
    ok
  fi
}

plant_map_accept() {
  mkdir -p .wm
  printf 'WORD: MAP-ACCEPT\nAGENT: bob\nMAP: MAP.md\n' > .wm/map-verdict
  printf 'id\tmodule\towned_paths\tdepends_on\trisk\tstatus\n' > slices.tsv
  printf 's1\twidget\tsrc/widget/api.py\t-\t%s\tREADY\n' "${1:-LOW}" >> slices.tsv
}

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

write_map_human() {
  _wmh_who=${1:-operator}
  _wmh_map=${2:-MAP.md}
  _wmh_key=${3:-SHA256}
  _wmh_h=$(sha256_file "$_wmh_map")
  printf 'SIGNED: %s\nMAP: %s\n%s: %s\n' "$_wmh_who" "$_wmh_map" "$_wmh_key" "$_wmh_h" > MAP-HUMAN
}

extract_fn() {
  awk -v n="$1" '
    $0 ~ "^" n "\\(\\)" { p=1; next }
    p && /^[a-z_][a-z0-9_]*\\(\\)/ { exit }
    p { print }
  ' "$WM"
}

kernel_fn_calls() {
  _kfc_fn=$1
  _kfc_need=$2
  _kfc_label=$3
  if extract_fn "$_kfc_fn" | grep -q "$_kfc_need"; then
    ok
  else
    bad "$_kfc_label"
  fi
}

# ---------------------------------------------------------------------------
# Identity CHECKs (must refuse, not only document). Fixture agents required.
# ---------------------------------------------------------------------------

# Honest architecture fixture records mapper from MAP.md
setup_map_repo t-arch-honest
mkdir -p tools
write_arch_agent
AGENT=alice
export AGENT
if HOME="$EMPTY_HOME" AGENT=alice ./tools/architecture-agent.sh >"$OUT" 2>"$ERR"; then
  ok
else
  err=$(cat "$ERR" 2>/dev/null || true)
  if printf '%s\n' "$err" | grep -q 'unknown command'; then
    bad "architecture fixture: record-mapper kernel hook missing"
  else
    bad "architecture fixture refused: $err"
  fi
fi
[ -f architecture/modules.md ] && ok || bad 'architecture fixture did not write architecture/modules.md'
[ -f MAP.md ] && grep -F -q 'MAPPER: alice' MAP.md && ok || bad 'architecture fixture did not write MAPPER in MAP.md'
if [ -f .wm/mapper ]; then
  grep -q '^id: alice$' .wm/mapper && ok || bad "mapper id not alice: $(cat .wm/mapper)"
else
  bad 'mapper id not recorded (.wm/mapper missing)'
fi
impl_ok 'check-module-fit honest widget path' "$WM" check-module-fit --path src/widget/api.py \
  || true
write_spec_fit
impl_ok 'check-module-fit honest SPEC owned path' "$WM" check-module-fit \
  || true

# mapper id recorded; later cast maker equal to mapper → refuse (7c)
setup_map_repo t-mapper-eq-maker
write_architecture_fixture alice
if impl_ok 'record-mapper --from MAP.md' "$WM" record-mapper --from MAP.md; then
  :
fi
refuses 'cast maker equal to mapper refused' 'refused:.*mapper cannot be maker' \
  "$WM" cast maker alice grok 'sh -c "echo maker {BRIEF}"'
expect 'cast maker distinct from mapper' 'cast maker=carol' \
  "$WM" cast maker carol grok 'sh -c "echo maker {BRIEF}"'
expect 'cast reviewer distinct from mapper' 'cast reviewer=dave' \
  "$WM" cast reviewer dave grok 'sh -c "echo reviewer {BRIEF}"'

# record-mapper refuses if maker already equals mapper
setup_map_repo t-maker-then-mapper
"$WM" cast maker alice grok 'sh -c "echo maker {BRIEF}"' >/dev/null
write_architecture_fixture alice
refuses 'record-mapper when maker already is mapper' 'refused:.*mapper cannot be maker' \
  "$WM" record-mapper --from MAP.md

# architecture author id ≠ critique author id; critique self-ACCEPT refuses (8b)
setup_map_repo t-critique-identity
mkdir -p tools
write_arch_agent
write_critique_agent
if HOME="$EMPTY_HOME" AGENT=alice ./tools/architecture-agent.sh >"$OUT" 2>"$ERR"; then
  ok
else
  err=$(cat "$ERR" 2>/dev/null || true)
  if printf '%s\n' "$err" | grep -q 'unknown command'; then
    bad 'architecture fixture (critique identity) missing record-mapper'
  else
    bad "architecture fixture (critique identity) refused: $err"
  fi
fi
refuses 'critique self-ACCEPT on a map it authored' 'refused:' \
  env AGENT=alice WORD=MAP-ACCEPT ./tools/critique-agent.sh
refuses 'critique author equal to architecture author (MAP-REVISE)' 'refused:' \
  env AGENT=alice WORD=MAP-REVISE ./tools/critique-agent.sh
if HOME="$EMPTY_HOME" AGENT=bob WORD=MAP-ACCEPT ./tools/critique-agent.sh >"$OUT" 2>"$ERR"; then
  ok
  [ -f reviews/critique.md ] && grep -q '## Invert' reviews/critique.md && ok \
    || bad 'honest critique did not write Invert'
  grep -q '## Adversarial' reviews/critique.md && ok || bad 'honest critique did not write Adversarial'
  grep -q '## Simple' reviews/critique.md && ok || bad 'honest critique did not write Simple'
else
  err=$(cat "$ERR" 2>/dev/null || true)
  if printf '%s\n' "$err" | grep -q 'unknown command'; then
    bad 'check-map-word kernel hook missing'
  else
    bad "honest critique MAP-ACCEPT refused: $err"
  fi
fi

# MAP-ACCEPT / MAP-REVISE / MAP-STOP-ASK are the only map words (not CLOSED PASS)
setup_map_repo t-map-words
write_architecture_fixture alice
impl_ok 'record-mapper for map-word tests' "$WM" record-mapper --from MAP.md || true
mkdir -p .wm/return
printf 'WORD: CLOSED PASS\nAGENT: bob\nMAP: MAP.md\n' > .wm/return/bob.md
refuses 'map word CLOSED PASS refused' 'refused:' "$WM" check-map-word .wm/return/bob.md
printf 'WORD: PASS\nAGENT: bob\nMAP: MAP.md\n' > .wm/return/bob.md
refuses 'map word PASS refused' 'refused:' "$WM" check-map-word .wm/return/bob.md
printf 'WORD: MAP-ACCEPT\nAGENT: bob\nMAP: MAP.md\n' > .wm/return/bob.md
impl_ok 'map word MAP-ACCEPT from distinct critique' "$WM" check-map-word .wm/return/bob.md || true
printf 'WORD: MAP-REVISE\nAGENT: bob\nMAP: MAP.md\n' > .wm/return/bob.md
impl_ok 'map word MAP-REVISE from distinct critique' "$WM" check-map-word .wm/return/bob.md || true
printf 'WORD: MAP-STOP-ASK\nAGENT: bob\nMAP: MAP.md\n' > .wm/return/bob.md
impl_ok 'map word MAP-STOP-ASK from distinct critique' "$WM" check-map-word .wm/return/bob.md || true

# owned paths must be under named modules or refuse (14a)
setup_map_repo t-owned-outside
write_architecture_fixture alice
impl_ok 'record-mapper for module-fit' "$WM" record-mapper --from MAP.md || true
refuses 'owned path outside named modules' 'refused:' \
  "$WM" check-module-fit --path src/other/x.py
refuses 'fairy-tale owned path rooms/throne.md' 'refused:' \
  "$WM" check-module-fit --path rooms/throne.md
cat > MAP.md <<'EOF'
MAPPER: alice

id	module	owned_paths	depends_on	risk
s1	dungeon	rooms/throne.md	-	LOW
EOF
refuses 'MAP.md owned path outside modules' 'refused:' "$WM" check-module-fit
write_spec_fit
cat > SPEC.md <<'EOF'
## Goal
x
## Non-goals
y
## Owned files
- src/other/x.py
## Test files
- (none)
## Acceptance criteria
- a
## Focused falsifier
MAKER-WRITES
## Stop conditions
stop
## Risk
LOW
SPEC-AUTHOR: operator
EOF
refuses 'SPEC owned path outside modules' 'refused:' "$WM" check-module-fit

# Extra inventory grammars must not widen MODULE-FIT (TSV root_path only).
setup_map_repo t-fit-tsv-only
write_architecture_fixture alice
impl_ok 'record-mapper for TSV-only fit' "$WM" record-mapper --from MAP.md || true
mkdir -p src/widget-extra
printf '# extra\n' > src/widget-extra/x.py
refuses 'sibling prefix src/widget-extra vs TSV src/widget' 'refused:' \
  "$WM" check-module-fit --path src/widget-extra/x.py
printf '\nroot: src\n' >> architecture/modules.md
refuses 'root: src line must not widen MODULE-FIT' 'refused:' \
  "$WM" check-module-fit --path src/widget-extra/x.py
printf '\n| module_id | root_path |\n| extra | src |\n' >> architecture/modules.md
refuses 'markdown table must not widen MODULE-FIT' 'refused:' \
  "$WM" check-module-fit --path src/widget-extra/x.py
impl_ok 'honest TSV path still fits after ignored extra grammars' \
  "$WM" check-module-fit --path src/widget/api.py || true

# CHANGES-ARCHITECTURE → STOP (no silent second pattern)
setup_map_repo t-changes-arch
write_architecture_fixture alice
impl_ok 'record-mapper for CHANGES-ARCHITECTURE' "$WM" record-mapper --from MAP.md || true
printf '\nCHANGES-ARCHITECTURE\n' >> MAP.md
refuses 'CHANGES-ARCHITECTURE in MAP.md is STOP' 'STOP|CHANGES-ARCHITECTURE' \
  "$WM" check-module-fit --path src/widget/api.py
# restore MAP without the marker; mark modules.md instead
write_architecture_fixture alice
printf '\nCHANGES-ARCHITECTURE\n' >> architecture/modules.md
refuses 'CHANGES-ARCHITECTURE in modules.md is STOP' 'STOP|CHANGES-ARCHITECTURE' \
  "$WM" check-module-fit --path src/widget/api.py

# Review: map words are not CLOSED PASS; brick WORD MAP-ACCEPT refused
setup_map_repo t-review-map-word
write_architecture_fixture alice
impl_ok 'record-mapper for review map-word' "$WM" record-mapper --from MAP.md || true
"$WM" cast maker carol grok 'sh -c "echo maker {BRIEF}"' >/dev/null
"$WM" cast reviewer dave grok 'sh -c "echo reviewer {BRIEF}"' >/dev/null
mkdir -p .wm/return reviews
printf 'WORD: MAP-ACCEPT\nAGENT: dave\nEVIDENCE: none\n' > .wm/return/dave.md
refuses 'reviewer WORD MAP-ACCEPT is not a brick verdict' 'refused:' \
  "$WM" verdict .wm/return/dave.md
printf 'WORD: CLOSED PASS\nAGENT: dave\nEVIDENCE: none\n' > .wm/return/dave.md
refuses 'reviewer WORD CLOSED PASS refused' 'refused:' \
  "$WM" verdict .wm/return/dave.md

# Review fixture (sh stub): code + testing lenses; must mention re-run
mkdir -p reviews
cat > reviews/review.md <<'EOF'
## Code
scope and correctness of the actual diff; no mutation.
## Testing
re-run the named falsifier; do not skip the test.
EOF
grep -q '## Code' reviews/review.md && ok || bad 'review fixture missing Code lens'
grep -q '## Testing' reviews/review.md && ok || bad 'review fixture missing Testing lens'
grep -q 're-run' reviews/review.md && ok || bad 'review fixture missing re-run'

# Loop-design fixture (echo stub): craft/audit/debrief note
setup_map_repo t-loop-design
write_loop_note
[ -f loop-design/NOTE.md ] && ok || bad 'loop-design fixture did not write NOTE.md'
grep -q '## Craft' loop-design/NOTE.md && ok || bad 'loop-design note missing Craft'
grep -q '## Audit' loop-design/NOTE.md && ok || bad 'loop-design note missing Audit'
grep -q '## Debrief' loop-design/NOTE.md && ok || bad 'loop-design note missing Debrief'
grep -q '^Ready$' loop-design/NOTE.md && ok || bad 'loop-design note missing Ready'
grep -q 'delivery walker' loop-design/NOTE.md && ok || bad 'loop-design note must not claim to be the walker (mentions the fence)'

# record-mapper --from missing MAPPER refuses
setup_map_repo t-mapper-missing
printf 'no mapper key\n' > MAP.md
refuses 'record-mapper --from MAP.md without MAPPER' 'refused:' \
  "$WM" record-mapper --from MAP.md

# check-module-fit without inventory refuses
setup_map_repo t-no-modules
refuses 'check-module-fit without architecture/modules.md' 'refused:' \
  "$WM" check-module-fit --path src/widget/api.py

# D1: greenfield relative root is created then fitted; absolute/`..` still die.
setup_map_repo t-greenfield-mkdir
mkdir -p architecture
printf 'module_id\troot_path\tpublic_contracts\ttest_entrypoint\tpattern_instance\tlive_write\n' \
  > architecture/modules.md
printf 'widget\tsrc/widget\tsrc/widget/api.py\ttests/widget\tsrc/widget/api.py\tno\n' \
  >> architecture/modules.md
if [ ! -d src/widget ]; then
  ok
else
  bad 'greenfield fit fixture must start without src/widget'
fi
if impl_ok 'greenfield check-module-fit creates missing root' "$WM" check-module-fit; then
  grep -q MODULE-FIT "$OUT" && ok || bad "greenfield fit wanted MODULE-FIT, got $(cat "$OUT")"
fi
[ -d src/widget ] && ok || bad 'check-module-fit must mkdir src/widget'
[ -f src/widget/.gitkeep ] && ok || bad 'check-module-fit must touch src/widget/.gitkeep'

setup_map_repo t-greenfield-map-ready
mkdir -p architecture
printf 'module_id\troot_path\tpublic_contracts\ttest_entrypoint\tpattern_instance\tlive_write\n' \
  > architecture/modules.md
printf 'widget\tsrc/widget\tsrc/widget/api.py\ttests/widget\tsrc/widget/api.py\tno\n' \
  >> architecture/modules.md
printf 'MAPPER: alice\n\nid\tmodule\towned_paths\tdepends_on\trisk\ns1\twidget\tsrc/widget/api.py\t-\tLOW\n' \
  > MAP.md
if [ ! -d src/widget ]; then
  ok
else
  bad 'greenfield map-ready fixture must start without src/widget'
fi
impl_ok 'record-mapper greenfield map-ready' "$WM" record-mapper --from MAP.md || true
if impl_ok 'map-ready greenfield creates missing root' "$WM" map-ready; then
  grep -q MAP-READY "$OUT" && ok || bad "greenfield map-ready wanted MAP-READY, got $(cat "$OUT")"
fi
[ -d src/widget ] && ok || bad 'map-ready must mkdir src/widget'
[ -f src/widget/.gitkeep ] && ok || bad 'map-ready must touch src/widget/.gitkeep'

setup_map_repo t-greenfield-two-roots
mkdir -p architecture
printf 'module_id\troot_path\tpublic_contracts\ttest_entrypoint\tpattern_instance\tlive_write\n' \
  > architecture/modules.md
printf 'widget\tsrc/widget\tsrc/widget/api.py\ttests/widget\tsrc/widget/api.py\tno\n' \
  >> architecture/modules.md
printf 'gadget\tsrc/gadget\tsrc/gadget/api.py\ttests/gadget\tsrc/gadget/api.py\tno\n' \
  >> architecture/modules.md
[ ! -d src/widget ] && [ ! -d src/gadget ] && ok || bad 'two-root greenfield must start empty'
impl_ok 'greenfield two missing roots still mkdir' "$WM" check-module-fit || true
[ -d src/widget ] && [ -d src/gadget ] && ok || bad 'greenfield two-root must mkdir both packages'

# Brownfield: existing src/widget, modules invent src/gadget → refuse without QUESTIONS.
setup_map_repo t-shape-new-pkg-no-q
write_architecture_fixture alice LOW no
printf 'gadget\tsrc/gadget\tsrc/gadget/api.py\ttests/gadget\tsrc/gadget/api.py\tno\n' \
  >> architecture/modules.md
[ -d src/widget ] && ok || bad 'brownfield fixture must already have src/widget'
[ ! -d src/gadget ] && ok || bad 'brownfield fixture must start without src/gadget'
refuses 'new top-level package without QUESTIONS.md' 'QUESTIONS.md' \
  "$WM" check-module-fit
if [ -d src/gadget ]; then
  bad 'must not mkdir src/gadget without QUESTIONS.md'
else
  ok
fi

# QUESTIONS.md without ANSWERS.md still refuses; no mkdir.
setup_map_repo t-shape-new-pkg-q-no-a
write_architecture_fixture alice LOW no
printf 'gadget\tsrc/gadget\tsrc/gadget/api.py\ttests/gadget\tsrc/gadget/api.py\tno\n' \
  >> architecture/modules.md
printf 'Should gadget be a new package or live under widget?\n' > QUESTIONS.md
refuses 'new package QUESTIONS without ANSWERS' 'ANSWERS.md' \
  "$WM" check-module-fit
if [ -d src/gadget ]; then
  bad 'must not mkdir src/gadget without ANSWERS.md'
else
  ok
fi

# QUESTIONS + ANSWERS: mkdir allowed (human chose the packaging).
setup_map_repo t-shape-new-pkg-answered
write_architecture_fixture alice LOW no
printf 'gadget\tsrc/gadget\tsrc/gadget/api.py\ttests/gadget\tsrc/gadget/api.py\tno\n' \
  >> architecture/modules.md
printf 'Should gadget be a new package or live under widget?\n' > QUESTIONS.md
printf 'New package src/gadget.\n' > ANSWERS.md
impl_ok 'new package with QUESTIONS+ANSWERS fits' "$WM" check-module-fit || true
[ -d src/gadget ] && ok || bad 'answered QUESTIONS must allow mkdir src/gadget'
[ -f src/gadget/.gitkeep ] && ok || bad 'answered QUESTIONS mkdir must touch .gitkeep'

# Existing module only (src/widget already there): no QUESTIONS required.
setup_map_repo t-shape-existing-pkg
write_architecture_fixture alice LOW no
impl_ok 'existing package fit without QUESTIONS' "$WM" check-module-fit || true

setup_map_repo t-map-ready-no-intent
write_architecture_fixture alice LOW no
impl_ok 'record-mapper for INTENT.md missing' "$WM" record-mapper --from MAP.md || true
rm -f INTENT.md
refuses 'map-ready without INTENT.md' 'INTENT.md missing' "$WM" map-ready

setup_map_repo t-fairy-abs-root
mkdir -p architecture
_abs_root="$BASE/must-not-create-abs"
printf 'module_id\troot_path\tpublic_contracts\ttest_entrypoint\tpattern_instance\tlive_write\n' \
  > architecture/modules.md
printf 'widget\t%s\tsrc/widget/api.py\ttests/widget\tsrc/widget/api.py\tno\n' "$_abs_root" \
  >> architecture/modules.md
refuses 'absolute module root refused' 'invalid module root' "$WM" check-module-fit
if [ -e "$_abs_root" ]; then
  bad 'absolute module root must not be created'
else
  ok
fi

setup_map_repo t-fairy-dotdot-root
mkdir -p architecture
printf 'module_id\troot_path\tpublic_contracts\ttest_entrypoint\tpattern_instance\tlive_write\n' \
  > architecture/modules.md
printf 'widget\t../x\tsrc/widget/api.py\ttests/widget\tsrc/widget/api.py\tno\n' \
  >> architecture/modules.md
refuses 'dot-dot module root refused' 'invalid module root' "$WM" check-module-fit
if [ -e "$BASE/x" ] || [ -e ../x ]; then
  bad 'dot-dot module root must not mkdir ../x'
else
  ok
fi

# ---------------------------------------------------------------------------
# Task 5: map cadence + human sign (6b, 8c, 3d). Identity CHECKs above stay GREEN.
# Falsifier-first: HIGH/live without MAP-HUMAN must not start maker; LOW local may.
# ---------------------------------------------------------------------------

kernel_fn_calls cmd_map_verdict cmd_check_map_word \
  'map-verdict must call check-map-word (not reimplement identity)'
kernel_fn_calls cmd_map_ready cmd_check_module_fit \
  'map-ready must call check-module-fit (not reimplement fit)'
kernel_fn_calls cmd_map_verdict cmd_check_module_fit \
  'map-verdict must call check-module-fit (not reimplement fit)'

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
require_fgrep "$HERE/docs/working-mode.md" 'map-ready' \
  'docs/working-mode.md must name wm map-ready'
require_fgrep "$HERE/docs/working-mode.md" 'map-verdict' \
  'docs/working-mode.md must name wm map-verdict'
require_fgrep "$HERE/docs/working-mode.md" 'SUBAGENT-ISOLATED' \
  'docs/working-mode.md must name SUBAGENT-ISOLATED for one-kind HIGH'
require_fgrep "$HERE/docs/working-mode.md" 'STOP-ASK' \
  'docs/working-mode.md must name STOP-ASK'
if [ -f "$HERE/docs/working-mode.md" ] && grep -q 'fake CROSS-FAMILY' "$HERE/docs/working-mode.md"; then
  ok
else
  bad 'docs/working-mode.md must refuse fake CROSS-FAMILY (3d is label+ROUTING)'
fi

# Load-bearing 8c: planted MAP-ACCEPT HIGH/live must not exec maker without MAP-HUMAN.
# (Full cadence also uses map-verdict; this path is RED if run currently accepts.)
setup_map_repo t-human-high
write_architecture_fixture alice HIGH no
impl_ok 'record-mapper HIGH fixture' "$WM" record-mapper --from MAP.md || true
cast_brick_panel carol dave grok grok
plant_map_accept HIGH
rm -f MAP-HUMAN .wm/FALSIFIER
refuses 'HIGH MAP-ACCEPT without MAP-HUMAN cannot start maker' 'MAP-HUMAN' \
  "$WM" run maker-falsify
if [ -f .wm/FALSIFIER ]; then
  bad 'HIGH without MAP-HUMAN must not exec maker (FALSIFIER written)'
else
  ok
fi

setup_map_repo t-human-live
write_architecture_fixture alice LOW yes
impl_ok 'record-mapper live fixture' "$WM" record-mapper --from MAP.md || true
cast_brick_panel carol dave grok grok
plant_map_accept LOW
rm -f MAP-HUMAN .wm/FALSIFIER
refuses 'live fence MAP-ACCEPT without MAP-HUMAN cannot start maker' 'MAP-HUMAN' \
  "$WM" run maker-falsify
if [ -f .wm/FALSIFIER ]; then
  bad 'live without MAP-HUMAN must not exec maker (FALSIFIER written)'
else
  ok
fi

setup_map_repo t-human-low
write_architecture_fixture alice LOW no
impl_ok 'record-mapper LOW fixture' "$WM" record-mapper --from MAP.md || true
cast_brick_panel carol dave grok grok
plant_map_accept LOW
rm -f MAP-HUMAN .wm/FALSIFIER
impl_ok 'LOW local MAP-ACCEPT can start maker without MAP-HUMAN' \
  "$WM" run maker-falsify || true
[ -f .wm/FALSIFIER ] && ok || bad 'LOW local maker-falsify must write FALSIFIER'

# 8c is a sign (SIGNED + MAP keys), not a non-empty token. HIGH + two kinds so 3d
# does not mask a dummy file. Mapper=alice maker=carol reviewer=dave.
setup_high_twokind_accept() {
  setup_map_repo "$1"
  write_architecture_fixture alice HIGH no
  impl_ok "record-mapper $1" "$WM" record-mapper --from MAP.md || true
  cast_brick_panel carol dave grok claude
  plant_map_accept HIGH
  rm -f .wm/FALSIFIER
}

refuse_high_sign_no_exec() {
  _rhs_label=$1
  refuses "$_rhs_label" 'MAP-HUMAN' "$WM" run maker-falsify
  if [ -f .wm/FALSIFIER ]; then
    bad "$_rhs_label must not exec maker (FALSIFIER written)"
  else
    ok
  fi
}

setup_high_twokind_accept t-human-x
printf 'x' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH MAP-HUMAN printf x is not a sign'

setup_high_twokind_accept t-human-space
printf ' \n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH MAP-HUMAN whitespace is not a sign'

setup_high_twokind_accept t-human-nosigned
printf 'MAP: MAP.md\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH MAP-HUMAN without SIGNED is not a sign'

setup_high_twokind_accept t-human-nomap
printf 'SIGNED: operator\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH MAP-HUMAN without MAP is not a sign'

setup_high_twokind_accept t-human-badmap
printf 'SIGNED: operator\nMAP: nosuch-map.md\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH MAP-HUMAN MAP must name an existing map file'

setup_high_twokind_accept t-human-nosha
printf 'SIGNED: operator\nMAP: MAP.md\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH MAP-HUMAN without SHA256 is not a sign'

setup_high_twokind_accept t-human-wrong-sha
printf 'SIGNED: operator\nMAP: MAP.md\nSHA256: deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH MAP-HUMAN wrong SHA256 is not a sign'

setup_high_twokind_accept t-human-stale-hash
write_map_human operator MAP.md
printf 'x' >> MAP.md
refuse_high_sign_no_exec 'HIGH MAP-HUMAN stale after MAP.md rewrite'
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -q 'STOP-ASK MAP-HUMAN' \
    && ok || bad "stale MAP-HUMAN next must STOP-ASK MAP-HUMAN, got $card"
else
  bad "stale MAP-HUMAN next refused: $(cat "$ERR")"
fi

setup_high_twokind_accept t-human-signed-maker
write_map_human carol MAP.md
refuse_high_sign_no_exec 'HIGH SIGNED maker refused'

setup_high_twokind_accept t-human-signed-mapper
write_map_human alice MAP.md
refuse_high_sign_no_exec 'HIGH SIGNED mapper refused'

setup_high_twokind_accept t-human-signed-reviewer
write_map_human dave MAP.md
refuse_high_sign_no_exec 'HIGH SIGNED reviewer refused'

setup_high_twokind_accept t-human-signed-parent
write_map_human parent MAP.md
refuse_high_sign_no_exec 'HIGH SIGNED parent refused'

setup_high_twokind_accept t-human-signed-coordinator
write_map_human coordinator MAP.md
refuse_high_sign_no_exec 'HIGH SIGNED coordinator refused'

setup_high_twokind_accept t-human-signed-loop
write_map_human loop MAP.md
refuse_high_sign_no_exec 'HIGH SIGNED loop refused'

# Full cadence: map-ready / map-verdict (unknown command is RED until implemented).
setup_map_repo t-cadence-low
write_architecture_fixture alice LOW no
impl_ok 'record-mapper cadence LOW' "$WM" record-mapper --from MAP.md || true
if impl_ok 'map-ready LOW local' "$WM" map-ready; then
  [ -f slices.tsv ] && ok || bad 'map-ready did not write slices.tsv'
  grep -q '^id	module	owned_paths	depends_on	risk	status$' slices.tsv \
    && ok || bad 'slices.tsv missing required header'
  grep -q '^s1	widget	src/widget/api.py	-	LOW	' slices.tsv \
    && ok || bad 'slices.tsv missing LOW slice s1'
fi
write_map_return alice MAP-ACCEPT
refuses 'map-verdict self-ACCEPT refused' 'refused:' \
  "$WM" map-verdict .wm/return/alice.md
write_map_return bob 'CLOSED PASS'
refuses 'map-verdict CLOSED PASS refused via check-map-word' \
  'map words are MAP-ACCEPT\|MAP-REVISE\|MAP-STOP-ASK' \
  "$WM" map-verdict .wm/return/bob.md
write_map_return bob MAP-ACCEPT
if impl_ok 'map-verdict MAP-ACCEPT from distinct critique' \
  "$WM" map-verdict .wm/return/bob.md; then
  grep -q '^WORD: MAP-ACCEPT$' .wm/map-verdict \
    && ok || bad 'map-verdict did not record MAP-ACCEPT'
  grep -q 'READY' slices.tsv && ok || bad 'MAP-ACCEPT did not mark slices READY'
fi
cast_brick_panel carol dave grok grok
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q 'NEXT SLICE s1' \
    && ok || bad "after MAP-ACCEPT next must emit first READY slice, got $card"
  printf '%s\n' "$card" | grep -q 'CROSS-FAMILY' \
    && bad "LOW next must not print CROSS-FAMILY (got $card)" || ok
else
  bad "after MAP-ACCEPT next refused: $(cat "$ERR")"
fi
rm -f MAP-HUMAN .wm/FALSIFIER
impl_ok 'LOW local after map-verdict can start maker' "$WM" run maker-falsify || true

setup_map_repo t-cadence-revise
write_architecture_fixture alice LOW no
impl_ok 'record-mapper REVISE' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready before REVISE' "$WM" map-ready || true
write_map_return bob MAP-REVISE
impl_ok 'map-verdict MAP-REVISE' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -q 'NEXT MAP' \
    && ok || bad "MAP-REVISE next must be NEXT MAP, got $card"
else
  bad "MAP-REVISE next refused: $(cat "$ERR")"
fi
refuses 'MAP-REVISE cannot start maker' 'MAP-ACCEPT' "$WM" run maker-falsify

setup_map_repo t-sendback-revise-overlay
write_architecture_fixture alice LOW no
impl_ok 'record-mapper sendback overlay' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready sendback overlay' "$WM" map-ready || true
write_map_return bob MAP-REVISE
impl_ok 'map-verdict sendback overlay' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
mkdir -p .crucible/skills/critique
cp "$HERE/skills/critique/SKILL.md" .crucible/skills/critique/SKILL.md
cp "$HERE/skills/critique/CONTRACT.md" .crucible/skills/critique/CONTRACT.md
# Rewrite only the TSV data row; keep header.
awk 'BEGIN{FS=OFS="\t"}
  $1=="MAP-REVISE" { $2="STOP-ASK"; print; next }
  { print }
' .crucible/skills/critique/CONTRACT.md > .crucible/skills/critique/CONTRACT.md.tmp
mv .crucible/skills/critique/CONTRACT.md.tmp .crucible/skills/critique/CONTRACT.md
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q '^STOP-ASK' \
    && ok || bad "overlay critique Send-back MAP-REVISE must emit STOP-ASK, got $card"
  printf '%s\n' "$card" | grep -q 'NEXT MAP' \
    && bad "overlay must not keep hardcoded NEXT MAP, got $card" || ok
else
  # STOP-ASK may be printed then exit 0 from next; if next dies, still ok if err matches
  err=$(cat "$ERR")
  printf '%s\n' "$err" | grep -q 'STOP-ASK' \
    && ok || bad "overlay MAP-REVISE next refused without STOP-ASK: $err"
fi

setup_map_repo t-cadence-stop
write_architecture_fixture alice LOW no
impl_ok 'record-mapper STOP-ASK map word' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready before STOP-ASK' "$WM" map-ready || true
write_map_return bob MAP-STOP-ASK
impl_ok 'map-verdict MAP-STOP-ASK' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -q 'STOP-ASK' \
    && ok || bad "MAP-STOP-ASK next must be STOP-ASK, got $card"
else
  bad "MAP-STOP-ASK next refused: $(cat "$ERR")"
fi

setup_map_repo t-fit-on-map-ready
write_architecture_fixture alice LOW no
impl_ok 'record-mapper for map-ready fit' "$WM" record-mapper --from MAP.md || true
cat > MAP.md <<'EOF'
MAPPER: alice

id	module	owned_paths	depends_on	risk
s1	dungeon	rooms/throne.md	-	LOW
EOF
refuses 'map-ready owned path outside modules' 'refused:' "$WM" map-ready

setup_map_repo t-map-ready-changes
write_architecture_fixture alice LOW no
impl_ok 'record-mapper for map-ready CHANGES-ARCHITECTURE' "$WM" record-mapper --from MAP.md || true
printf '\nCHANGES-ARCHITECTURE\n' >> MAP.md
refuses 'map-ready CHANGES-ARCHITECTURE is STOP' 'STOP|CHANGES-ARCHITECTURE' "$WM" map-ready

# 3d superseded 2026-09-15: HIGH + one kind + distinct agents + MAP-HUMAN proceeds.
# Label SUBAGENT-ISOLATED. Never fake CROSS-FAMILY.
setup_map_repo t-high-one-kind
write_architecture_fixture alice HIGH no
impl_ok 'record-mapper HIGH one-kind' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready HIGH one-kind' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict HIGH one-kind' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
write_map_human operator MAP.md
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q 'NEXT SLICE s1' \
    && ok || bad "HIGH + one kind with MAP-HUMAN next must emit NEXT SLICE, got $card"
  printf '%s\n' "$card" | grep -q 'STOP-ASK' \
    && bad "HIGH + one kind must not STOP-ASK when agents differ (got $card)" || ok
  printf '%s\n' "$card" | grep -q 'CROSS-FAMILY' \
    && bad "HIGH + one kind must not fake CROSS-FAMILY (got $card)" || ok
else
  bad "HIGH + one kind next refused: $(cat "$ERR")"
fi
"$WM" status >/dev/null 2>"$ERR" || true
if [ -f .wm/FLOOR.md ] && grep -q '^independence: SUBAGENT-ISOLATED$' .wm/FLOOR.md; then
  ok
else
  bad "HIGH one-kind FLOOR must say independence: SUBAGENT-ISOLATED, got $(cat .wm/FLOOR.md 2>/dev/null || echo ABSENT)"
fi
if grep -q 'CROSS-FAMILY' .wm/FLOOR.md 2>/dev/null; then
  bad 'HIGH one-kind FLOOR must not say CROSS-FAMILY'
else
  ok
fi
rm -f .wm/FALSIFIER
impl_ok 'HIGH + one kind with MAP-HUMAN can start maker' "$WM" run maker-falsify || true
[ -f .wm/FALSIFIER ] && ok || bad 'HIGH one-kind maker-falsify must write FALSIFIER'

setup_map_repo t-high-two-kind
write_architecture_fixture alice HIGH no
impl_ok 'record-mapper HIGH two-kind' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready HIGH two-kind' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict HIGH two-kind' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok claude
rm -f MAP-HUMAN .wm/FALSIFIER
refuses 'HIGH two-kind without MAP-HUMAN cannot start maker' 'MAP-HUMAN' \
  "$WM" run maker-falsify
write_map_human operator MAP.md
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q 'NEXT SLICE s1' \
    && ok || bad "HIGH two-kind with MAP-HUMAN next must emit READY slice, got $card"
  printf '%s\n' "$card" | grep -q 'CROSS-FAMILY' \
    && bad "3d must not print CROSS-FAMILY as a second engine (got $card)" || ok
else
  bad "HIGH two-kind next refused: $(cat "$ERR")"
fi
"$WM" status >/dev/null 2>"$ERR" || true
if [ -f .wm/FLOOR.md ] && grep -q '^independence: CROSS-FAMILY$' .wm/FLOOR.md; then
  ok
else
  bad "HIGH two-kind FLOOR must say independence: CROSS-FAMILY, got $(cat .wm/FLOOR.md 2>/dev/null || echo ABSENT)"
fi
impl_ok 'HIGH two-kind with MAP-HUMAN can start maker' "$WM" run maker-falsify || true
[ -f .wm/FALSIFIER ] && ok || bad 'HIGH two-kind maker-falsify must write FALSIFIER'

setup_high_twokind_accept t-human-map-sha256
write_map_human operator MAP.md MAP-SHA256
impl_ok 'HIGH MAP-SHA256 alias can start maker' "$WM" run maker-falsify || true
[ -f .wm/FALSIFIER ] && ok || bad 'HIGH MAP-SHA256 maker-falsify must write FALSIFIER'

setup_map_repo t-live-signed
write_architecture_fixture alice LOW yes
impl_ok 'record-mapper live signed' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready live signed' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict live signed' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
write_map_human operator MAP.md
impl_ok 'live + MAP-HUMAN + same kind can start maker (3d is HIGH-only)' \
  "$WM" run maker-falsify || true

# Task 6 walker: STOP-ASK / ESCALATE / CLOSE are terminal; no leftover processes.
# t-live-signed already ran maker-falsify (true); walker may stop at echo reviewer.
set +e
"$WM" loop >"$OUT" 2>"$ERR"
loop_rc=$?
set -e
if pgrep -f "$WM loop" >/dev/null 2>&1; then
  bad 'wm loop left a leftover wm.sh loop process'
else
  ok
fi
if [ "$loop_rc" -eq 0 ] || [ "$loop_rc" -eq 1 ]; then
  ok
else
  bad "wm loop unexpected exit $loop_rc"
fi
printf '%s\n' "$(cat "$OUT")" | grep -q 'LOOP STUB' \
  && bad 'wm loop is still LOOP STUB (Task 6 walker missing)' || ok

# STOP-ASK MAP-HUMAN is terminal — do not background-wait for a human.
setup_map_repo t-loop-map-human
write_architecture_fixture alice HIGH no
impl_ok 'record-mapper loop MAP-HUMAN' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready loop MAP-HUMAN' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict loop MAP-HUMAN' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok claude
rm -f MAP-HUMAN .wm/FALSIFIER
write_spec_fit
git add -A
git commit -qm 'spec for HIGH unsigned loop' >/dev/null
run_map_loop
assert_map_loop_foreground 't-loop-map-human'
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'STOP-ASK MAP-HUMAN' \
  && ok || bad "HIGH unsigned loop wanted STOP-ASK MAP-HUMAN, got out=$(cat "$OUT") err=$(cat "$ERR")"
[ "$LOOP_RC" -ne 0 ] && ok || bad 'STOP-ASK MAP-HUMAN loop must not exit 0'
if [ -f .wm/FALSIFIER ]; then
  bad 'STOP-ASK MAP-HUMAN loop must not start maker'
else
  ok
fi
if [ -f .wm/CLOSED ]; then
  bad 'STOP-ASK MAP-HUMAN loop must not write CLOSED'
else
  ok
fi

# NEXT MAP is terminal when specifier/scout have no CLI (do not invent a map).
# D3: MAP-REVISE without specifier CLI still STOP-ASK even if scout is cast.
setup_map_repo t-loop-next-map
write_architecture_fixture alice LOW no
impl_ok 'record-mapper loop NEXT MAP' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready loop NEXT MAP' "$WM" map-ready || true
write_map_return bob MAP-REVISE
impl_ok 'map-verdict loop REVISE' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
mkdir -p tools
cat > tools/scout-map-accept-ok.sh <<'EOF'
#!/bin/sh
set -eu
agent=bob
if [ -n "${BRIEF:-}" ] && [ -f "$BRIEF" ]; then
  a=$(awk -F ': ' '$1=="agent"{print $2; exit}' "$BRIEF")
  [ -n "$a" ] && agent=$a
fi
mkdir -p .wm/return
printf 'WORD: MAP-ACCEPT\nAGENT: %s\nMAP: MAP.md\n' "$agent" > ".wm/return/${agent}.md"
EOF
chmod +x tools/scout-map-accept-ok.sh
"$WM" cast scout bob grok './tools/scout-map-accept-ok.sh {BRIEF}' >/dev/null
run_map_loop
assert_map_loop_foreground 't-loop-next-map'
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'STOP-ASK' \
  && ok || bad "MAP-REVISE loop wanted STOP-ASK, got out=$(cat "$OUT") err=$(cat "$ERR")"
[ "$LOOP_RC" -ne 0 ] && ok || bad 'NEXT MAP loop must not exit 0'
if grep -q 'CLOSED PASS' "$OUT" 2>/dev/null; then
  bad 'MAP-REVISE without specifier CLI must not CLOSED PASS'
else
  ok
fi
if [ -f .wm/map-verdict ] && grep -q '^WORD: MAP-REVISE$' .wm/map-verdict; then
  ok
else
  bad 'MAP-REVISE without specifier must not run scout (verdict must stay MAP-REVISE)'
fi
if [ -f .wm/specifier-revise ]; then
  bad 'MAP-REVISE without specifier CLI must not touch specifier-revise'
else
  ok
fi

# D3: MAP-REVISE with specifier CLI re-runs specifier (even if MAP.md exists), then scout.
setup_map_repo t-loop-revise-specifier
write_architecture_fixture alice LOW no
impl_ok 'record-mapper loop MAP-REVISE specifier' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready loop MAP-REVISE specifier' "$WM" map-ready || true
write_map_return bob MAP-REVISE
impl_ok 'map-verdict loop MAP-REVISE specifier' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
mkdir -p tools
cat > tools/specifier-revise.sh <<'EOF'
#!/bin/sh
set -eu
mkdir -p .wm
touch .wm/specifier-revise
printf '\nREVISED\n' >> MAP.md
EOF
cat > tools/scout-map-accept-ok.sh <<'EOF'
#!/bin/sh
set -eu
agent=bob
if [ -n "${BRIEF:-}" ] && [ -f "$BRIEF" ]; then
  a=$(awk -F ': ' '$1=="agent"{print $2; exit}' "$BRIEF")
  [ -n "$a" ] && agent=$a
fi
mkdir -p .wm/return
printf 'WORD: MAP-ACCEPT\nAGENT: %s\nMAP: MAP.md\n' "$agent" > ".wm/return/${agent}.md"
EOF
chmod +x tools/specifier-revise.sh tools/scout-map-accept-ok.sh
"$WM" cast specifier eve grok './tools/specifier-revise.sh {BRIEF}' >/dev/null
"$WM" cast scout bob grok './tools/scout-map-accept-ok.sh {BRIEF}' >/dev/null
run_map_loop
assert_map_loop_foreground 't-loop-revise-specifier'
[ -f .wm/specifier-revise ] && ok || bad 'MAP-REVISE with specifier CLI must run specifier (.wm/specifier-revise missing)'
grep -q REVISED MAP.md && ok || bad 'MAP-REVISE specifier must be allowed to rewrite MAP.md'
if [ -f .wm/map-verdict ] && grep -q '^WORD: MAP-ACCEPT$' .wm/map-verdict; then
  ok
else
  bad "MAP-REVISE with specifier must then run scout MAP-ACCEPT, got $(cat .wm/map-verdict 2>/dev/null || echo ABSENT)"
fi
if grep -q 'CLOSED PASS' "$OUT" 2>/dev/null; then
  bad 'MAP-REVISE specifier loop without brick workers must not CLOSED PASS'
else
  ok
fi

# Specifier cannot be maker (mapper≠maker).
setup_map_repo t-specifier-eq-maker
"$WM" cast maker alice grok 'sh -c "echo maker {BRIEF}"' >/dev/null
refuses 'cast specifier equal to maker refused' \
  'specifier cannot be maker|mapper cannot be maker' \
  "$WM" cast specifier alice grok 'sh -c "echo specifier {BRIEF}"'

# Scout MAP-ACCEPT as mapper: map-verdict refuse (via wm run scout).
setup_map_repo t-run-scout-mapper-accept
write_architecture_fixture eve LOW no
impl_ok 'record-mapper scout self-ACCEPT' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready scout self-ACCEPT' "$WM" map-ready || true
mkdir -p tools
cat > tools/scout-map-accept.sh <<'EOF'
#!/bin/sh
set -eu
agent=bob
if [ -n "${BRIEF:-}" ] && [ -f "$BRIEF" ]; then
  a=$(awk -F ': ' '$1=="agent"{print $2; exit}' "$BRIEF")
  [ -n "$a" ] && agent=$a
fi
mapper=eve
if [ -f MAP.md ]; then
  m=$(awk -F ': ' '$1=="MAPPER"{print $2; exit}' MAP.md)
  [ -n "$m" ] && mapper=$m
fi
mkdir -p .wm/return
printf 'WORD: MAP-ACCEPT\nAGENT: %s\nMAP: MAP.md\n' "$mapper" > ".wm/return/${agent}.md"
EOF
chmod +x tools/scout-map-accept.sh
"$WM" cast scout bob grok './tools/scout-map-accept.sh {BRIEF}' >/dev/null
refuses 'wm run scout MAP-ACCEPT as mapper refused' 'refused:' \
  "$WM" run scout
if [ -f .wm/verdicts/bob.md ]; then
  bad 'scout MAP-ACCEPT must not brick-ingest cmd_verdict'
else
  ok
fi

# Honest scout MAP-ACCEPT via wm run scout ingest map-verdict, not brick verdict.
setup_map_repo t-run-scout-map-accept
write_architecture_fixture eve LOW no
impl_ok 'record-mapper scout MAP-ACCEPT' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready scout MAP-ACCEPT' "$WM" map-ready || true
mkdir -p tools
cat > tools/scout-map-accept-ok.sh <<'EOF'
#!/bin/sh
set -eu
agent=bob
if [ -n "${BRIEF:-}" ] && [ -f "$BRIEF" ]; then
  a=$(awk -F ': ' '$1=="agent"{print $2; exit}' "$BRIEF")
  [ -n "$a" ] && agent=$a
fi
mkdir -p .wm/return
printf 'WORD: MAP-ACCEPT\nAGENT: %s\nMAP: MAP.md\n' "$agent" > ".wm/return/${agent}.md"
EOF
chmod +x tools/scout-map-accept-ok.sh
"$WM" cast scout bob grok './tools/scout-map-accept-ok.sh {BRIEF}' >/dev/null
expect 'wm run scout MAP-ACCEPT ingest map-verdict' 'MAP-ACCEPT' \
  "$WM" run scout
if [ -f .wm/map-verdict ] && grep -q '^WORD: MAP-ACCEPT$' .wm/map-verdict; then
  ok
else
  bad 'wm run scout MAP-ACCEPT must write .wm/map-verdict'
fi
if [ -f .wm/verdicts/bob.md ]; then
  bad 'wm run scout MAP-ACCEPT must not write brick verdicts/bob.md'
else
  ok
fi
if [ -f .wm/briefs/scout.* ] || ls .wm/briefs/scout.* >/dev/null 2>&1; then
  brief=$(ls .wm/briefs/scout.* 2>/dev/null | head -1)
  grep -E -q 'MAP-ACCEPT|MAP-REVISE|MAP-STOP-ASK' "$brief" \
    && ok || bad 'scout brief for empty map word must ask MAP-ACCEPT|MAP-REVISE|MAP-STOP-ASK'
else
  bad 'scout brief missing after wm run scout'
fi

# HIGH + one kind + MAP-HUMAN + brick workers → CLOSED PASS, SUBAGENT-ISOLATED.
setup_map_repo t-loop-high-one-kind
write_architecture_fixture alice HIGH no
impl_ok 'record-mapper loop HIGH one-kind' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready loop HIGH one-kind' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict loop HIGH one-kind' "$WM" map-verdict .wm/return/bob.md || true
write_map_human operator MAP.md
write_spec_fit
write_map_loop_brick
"$WM" cast maker carol grok './tools/map-loop-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer dave grok './tools/map-loop-reviewer.sh {BRIEF}' >/dev/null
git add -A
git commit -qm 'HIGH one-kind loop workers' >/dev/null
run_map_loop
assert_map_loop_foreground 't-loop-high-one-kind'
if grep -q 'CLOSED PASS' "$OUT" && [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  ok
else
  bad "HIGH one-kind loop wanted CLOSED PASS, got out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT) err=$(cat "$ERR")"
fi
[ "$LOOP_RC" -eq 0 ] && ok || bad "HIGH one-kind loop exit $LOOP_RC err=$(cat "$ERR")"
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'CROSS-FAMILY' \
  && bad 'HIGH one-kind loop must not fake CROSS-FAMILY' || ok
if grep -q '^independence: SUBAGENT-ISOLATED$' .wm/CLOSED \
  && grep -q '^independence: SUBAGENT-ISOLATED$' .wm/FLOOR.md; then
  ok
else
  bad "HIGH one-kind CLOSED/FLOOR must record SUBAGENT-ISOLATED, closed=$(cat .wm/CLOSED) floor=$(cat .wm/FLOOR.md 2>/dev/null || echo ABSENT)"
fi
[ -f .wm/reviewer-ran ] && ok || bad 'HIGH one-kind loop did not exec the reviewer CLI'

# Honest LOW map: NEXT SLICE then brick walk to CLOSED PASS (one slice in flight).
setup_map_repo t-loop-slice-pass
write_architecture_fixture alice LOW no
cat > MAP.md <<'EOF'
MAPPER: alice

id	module	owned_paths	depends_on	risk
s1	widget	src/widget/api.py	-	LOW
s2	widget	src/widget/api.py	s1	LOW
EOF
impl_ok 'record-mapper loop slice' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready loop slice' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict loop slice' "$WM" map-verdict .wm/return/bob.md || true
write_spec_fit
write_map_loop_brick
"$WM" cast maker carol grok './tools/map-loop-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer dave grok './tools/map-loop-reviewer.sh {BRIEF}' >/dev/null
git add -A
git commit -qm 'spec map loop workers' >/dev/null
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q 'NEXT SLICE s1' \
    && ok || bad "map loop next must emit NEXT SLICE s1, got $card"
else
  bad "map loop next refused: $(cat "$ERR")"
fi
run_map_loop
assert_map_loop_foreground 't-loop-slice-pass'
if grep -q 'CLOSED PASS' "$OUT" && [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  ok
else
  bad "map loop wanted CLOSED PASS, got out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT) err=$(cat "$ERR")"
fi
[ -f .wm/reviewer-ran ] && ok || bad 'map loop did not exec the reviewer CLI'
[ "$LOOP_RC" -eq 0 ] && ok || bad "map loop exit $LOOP_RC err=$(cat "$ERR")"
closed_rows=$(awk -F '\t' 'NR>1 && $6=="CLOSED" {c++} END{print c+0}' slices.tsv)
[ "$closed_rows" -eq 2 ] && ok || bad "map loop slices.tsv CLOSED rows wanted 2, got $closed_rows"
# After s1 close, s2 depends_on is satisfied — one loop must walk s2 (not harness reset)
grep -q 'NEXT SLICE s2' "$OUT" \
  && ok || bad 'after s1 CLOSED, loop must emit NEXT SLICE s2 (deps satisfied)'
if [ -f .wm/slice-in-flight ]; then
  grep -q 's2' .wm/slice-in-flight \
    && ok || bad "last slice-in-flight must be s2, got $(cat .wm/slice-in-flight)"
else
  ok
fi
grep -q 'WM-SLICE-s1' src/widget/api.py && ok || bad 'map loop s1 maker-build did not land'
grep -q 'WM-SLICE-s2' src/widget/api.py && ok || bad 'map loop s2 maker-build did not land'

# Session: each wm run mints a distinct session line in the brief.
setup_map_repo t-session-ids
write_architecture_fixture alice LOW no
impl_ok 'record-mapper session' "$WM" record-mapper --from MAP.md || true
"$WM" cast specifier spec0 grok 'true' >/dev/null
"$WM" run specifier >/dev/null 2>"$ERR" || true
b1=$(ls .wm/briefs/specifier.* 2>/dev/null | head -1)
s1=
[ -n "$b1" ] && s1=$(awk -F ': ' '$1=="session"{print $2; exit}' "$b1")
"$WM" run specifier >/dev/null 2>"$ERR" || true
b2=$(ls -t .wm/briefs/specifier.* 2>/dev/null | head -1)
s2=
[ -n "$b2" ] && s2=$(awk -F ': ' '$1=="session"{print $2; exit}' "$b2")
if [ -n "$s1" ] && [ -n "$s2" ] && [ "$s1" != "$s2" ]; then
  ok
else
  bad "wm run must mint distinct session ids, got s1=$s1 s2=$s2"
fi
printf '%s\n' "$s1" | grep -E -q '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' \
  && ok || bad "session id must be a UUID, got $s1"

# Per-station skill packs: specifier/scout/reviewer briefs embed ROUTING battery SKILL+CONTRACT.
setup_map_repo t-pack-specifier
write_architecture_fixture alice LOW no
"$WM" cast specifier spec0 grok 'true' >/dev/null
"$WM" run specifier >/dev/null 2>"$ERR" || true
sb=$(ls -t .wm/briefs/specifier.* 2>/dev/null | head -1)
[ -n "$sb" ] && grep -q 'RULE 26' "$sb" \
  && ok || bad "specifier brief must embed architecture SKILL (RULE 26), brief=$(cat $sb 2>/dev/null || echo ABSENT)"
[ -n "$sb" ] && grep -q 'You verify; you do not improve' "$sb" \
  && bad 'specifier brief must not embed review SKILL' || ok
[ -n "$sb" ] && grep -q '^session: ' "$sb" \
  && ok || bad 'specifier brief must keep session: line before station pack'

setup_map_repo t-pack-scout
write_architecture_fixture alice LOW no
impl_ok 'record-mapper pack scout' "$WM" record-mapper --from MAP.md || true
mkdir -p tools
printf '#!/bin/sh\nset -eu\nagent=bob\nmkdir -p .wm/return\nprintf "WORD: MAP-ACCEPT\\nAGENT: %s\\nMAP: MAP.md\\n" "$agent" > .wm/return/bob.md\n' > tools/scout-ok.sh
chmod +x tools/scout-ok.sh
"$WM" cast scout bob grok './tools/scout-ok.sh {BRIEF}' >/dev/null
"$WM" run scout >/dev/null 2>"$ERR" || true
scb=$(ls -t .wm/briefs/scout.* 2>/dev/null | head -1)
[ -n "$scb" ] && grep -q 'MAP-ACCEPT' "$scb" && grep -qi invert "$scb" \
  && ok || bad "scout brief must embed critique SKILL, brief=$(cat $scb 2>/dev/null || echo ABSENT)"
[ -n "$scb" ] && grep -q 'You are the mapper' "$scb" \
  && bad 'scout brief must not embed architecture mapper job' || ok

setup_map_repo t-pack-reviewer
write_architecture_fixture alice LOW no
"$WM" cast maker carol grok 'true' >/dev/null
"$WM" cast reviewer dave grok 'false' >/dev/null
"$WM" run reviewer >/dev/null 2>"$ERR" || true
rb=$(ls -t .wm/briefs/reviewer.* 2>/dev/null | head -1)
if [ -n "$rb" ] && grep -q 're-run' "$rb" && grep -q 'You verify; you do not improve' "$rb"; then
  ok
else
  bad "reviewer brief must embed review SKILL, brief=$(cat $rb 2>/dev/null || echo ABSENT)"
fi
[ -n "$rb" ] && grep -q 'You are the mapper' "$rb" \
  && bad 'reviewer brief must not embed architecture mapper job' || ok

setup_map_repo t-pack-maker
write_architecture_fixture alice LOW no
impl_ok 'record-mapper pack maker' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready pack maker' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict pack maker' "$WM" map-verdict .wm/return/bob.md || true
write_spec_fit
write_maker_falsify_stub
"$WM" cast maker carol grok './tools/maker-falsify.sh {BRIEF}' >/dev/null
"$WM" cast reviewer dave grok 'false' >/dev/null
"$WM" run maker-falsify >/dev/null 2>"$ERR" || true
mb=$(ls -t .wm/briefs/maker-falsify.* 2>/dev/null | head -1)
[ -n "$mb" ] && grep -q 'You are the mapper' "$mb" \
  && bad 'maker brief must not embed architecture mapper job' || ok
[ -n "$mb" ] && grep -q 'You verify; you do not improve' "$mb" \
  && bad 'maker brief must not embed review judge job' || ok
[ -n "$mb" ] && grep -q '^session: ' "$mb" \
  && ok || bad 'maker brief must keep session: line (no station pack)'

# Walker copy (.wm/bin/wm / wm loop) must embed the pack, not only package wm.sh.
setup_map_repo t-pack-copied-kernel
write_architecture_fixture alice LOW no
"$WM" cast specifier spec0 grok 'true' >/dev/null
WM_ENGINE= .wm/bin/wm run specifier >/dev/null 2>"$ERR" || true
kb=$(ls -t .wm/briefs/specifier.* 2>/dev/null | head -1)
[ -n "$kb" ] && grep -q 'RULE 26' "$kb" && grep -q '## Station pack' "$kb" \
  && ok || bad "copied .wm/bin/wm specifier brief must embed station pack (RULE 26), brief=$(cat $kb 2>/dev/null || echo ABSENT)"

# Reviewer/scout that mutate owned product paths are refused (CHECK after exec, no rollback).
setup_map_repo t-rev-owned-wall
write_architecture_fixture alice LOW no
impl_ok 'record-mapper wall' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready wall' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict wall' "$WM" map-verdict .wm/return/bob.md || true
write_spec_fit
mkdir -p tools
cat > tools/reviewer-mutates.sh <<'EOF'
#!/bin/sh
set -eu
printf '\nMUTATED\n' >> src/widget/api.py
mkdir -p .wm/return reviews
printf '## Code\nx\n## Testing\ny\n' > reviews/review.md
printf 'WORD: FAIL\nEVIDENCE: none\n' > .wm/return/dave.md
EOF
chmod +x tools/reviewer-mutates.sh
"$WM" cast maker carol grok 'true' >/dev/null
"$WM" cast reviewer dave grok './tools/reviewer-mutates.sh {BRIEF}' >/dev/null
git add -A && git commit -qm wall >/dev/null
# Need a last-maker-run? reviewer may run without FALSIFIER for this CHECK — cmd_run reviewer does not require FALSIFIER.
refuses 'reviewer that writes owned path is refused' 'owned path' \
  "$WM" run reviewer
grep -q MUTATED src/widget/api.py && ok || bad 'fixture must have attempted the write (CHECK is after exec)'

setup_map_repo t-scout-owned-wall
write_architecture_fixture alice LOW no
impl_ok 'record-mapper scout wall' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready scout wall' "$WM" map-ready || true
write_spec_fit
mkdir -p tools
cat > tools/scout-mutates.sh <<'EOF'
#!/bin/sh
set -eu
printf '\nMUTATED\n' >> src/widget/api.py
mkdir -p .wm/return
printf 'WORD: MAP-ACCEPT\nAGENT: bob\nMAP: MAP.md\n' > .wm/return/bob.md
EOF
chmod +x tools/scout-mutates.sh
"$WM" cast scout bob grok './tools/scout-mutates.sh {BRIEF}' >/dev/null
git add -A && git commit -qm scout-wall >/dev/null
refuses 'scout that writes owned path is refused' 'owned path' \
  "$WM" run scout
grep -q MUTATED src/widget/api.py && ok || bad 'scout fixture must have attempted the write (CHECK is after exec)'

# 11c still holds
if grep -E -q 'skills/(architecture|critique|review|loop-design)' "$WM"; then
  bad 'wm.sh hardcodes battery paths; replacing a directory would require editing wm.sh'
else
  ok
fi
if grep -q 'sendback_lookup' "$WM"; then
  ok
else
  bad 'wm.sh must define sendback_lookup'
fi

# Home leak: empty HOME must stay empty of skills (git may write nothing; we used GIT_CONFIG_*)
home_leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
if [ -z "$home_leftovers" ]; then
  ok
else
  bad "wrote under HOME: $home_leftovers"
fi

# Four batteries only in package skills/
extra=
for d in "$HERE/skills"/*; do
  [ -d "$d" ] || continue
  n=${d##*/}
  case $n in
    architecture|critique|review|loop-design|working-mode|research|repo-scout) ;;
    *) extra="$extra $n" ;;
  esac
done
if [ -z "$extra" ]; then
  ok
else
  bad "unexpected package battery:$extra"
fi

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
