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
printf 'true\n' > .wm/FALSIFIER
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

plant_map_accept() {
  mkdir -p .wm
  printf 'WORD: MAP-ACCEPT\nAGENT: bob\nMAP: MAP.md\n' > .wm/map-verdict
  printf 'id\tmodule\towned_paths\tdepends_on\trisk\tstatus\n' > slices.tsv
  printf 's1\twidget\tsrc/widget/api.py\t-\t%s\tREADY\n' "${1:-LOW}" >> slices.tsv
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
require_fgrep "$HERE/docs/working-mode.md" 'not mapper' \
  'docs/working-mode.md must refuse mapper/maker/reviewer as SIGNED'
require_fgrep "$HERE/docs/working-mode.md" 'slices.tsv' \
  'docs/working-mode.md must name slices.tsv (6b)'
require_fgrep "$HERE/docs/working-mode.md" 'map-ready' \
  'docs/working-mode.md must name wm map-ready'
require_fgrep "$HERE/docs/working-mode.md" 'map-verdict' \
  'docs/working-mode.md must name wm map-verdict'
require_fgrep "$HERE/docs/working-mode.md" 'STOP-ASK' \
  'docs/working-mode.md must name STOP-ASK for HIGH + one kind (3d)'
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

setup_high_twokind_accept t-human-signed-maker
printf 'SIGNED: carol\nMAP: MAP.md\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH SIGNED maker refused'

setup_high_twokind_accept t-human-signed-mapper
printf 'SIGNED: alice\nMAP: MAP.md\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH SIGNED mapper refused'

setup_high_twokind_accept t-human-signed-reviewer
printf 'SIGNED: dave\nMAP: MAP.md\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH SIGNED reviewer refused'

setup_high_twokind_accept t-human-signed-parent
printf 'SIGNED: parent\nMAP: MAP.md\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH SIGNED parent refused'

setup_high_twokind_accept t-human-signed-coordinator
printf 'SIGNED: coordinator\nMAP: MAP.md\n' > MAP-HUMAN
refuse_high_sign_no_exec 'HIGH SIGNED coordinator refused'

setup_high_twokind_accept t-human-signed-loop
printf 'SIGNED: loop\nMAP: MAP.md\n' > MAP-HUMAN
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

# 3d: HIGH + one kind → STOP-ASK, not fake CROSS-FAMILY; two kinds allowed after MAP-HUMAN.
setup_map_repo t-high-one-kind
write_architecture_fixture alice HIGH no
impl_ok 'record-mapper HIGH one-kind' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready HIGH one-kind' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict HIGH one-kind' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
printf 'SIGNED: operator\nMAP: MAP.md\n' > MAP-HUMAN
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -q 'STOP-ASK' \
    && ok || bad "HIGH + one kind next must STOP-ASK, got $card"
  printf '%s\n' "$card" | grep -q 'CROSS-FAMILY' \
    && bad "HIGH + one kind must not fake CROSS-FAMILY (got $card)" || ok
else
  bad "HIGH + one kind next refused: $(cat "$ERR")"
fi
rm -f .wm/FALSIFIER
refuses 'HIGH + one kind cannot start maker' 'STOP-ASK' "$WM" run maker-falsify
if [ -f .wm/FALSIFIER ]; then
  bad 'HIGH + one kind must not exec maker'
else
  ok
fi
if err=$(cat "$ERR" 2>/dev/null || true); printf '%s\n' "$err" | grep -q 'CROSS-FAMILY'; then
  bad "HIGH + one kind refuse must not say CROSS-FAMILY: $err"
else
  ok
fi

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
printf 'SIGNED: operator\nMAP: MAP.md\n' > MAP-HUMAN
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q 'NEXT SLICE s1' \
    && ok || bad "HIGH two-kind with MAP-HUMAN next must emit READY slice, got $card"
  printf '%s\n' "$card" | grep -q 'CROSS-FAMILY' \
    && bad "3d must not print CROSS-FAMILY as a second engine (got $card)" || ok
else
  bad "HIGH two-kind next refused: $(cat "$ERR")"
fi
impl_ok 'HIGH two-kind with MAP-HUMAN can start maker' "$WM" run maker-falsify || true
[ -f .wm/FALSIFIER ] && ok || bad 'HIGH two-kind maker-falsify must write FALSIFIER'

setup_map_repo t-live-signed
write_architecture_fixture alice LOW yes
impl_ok 'record-mapper live signed' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready live signed' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict live signed' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
printf 'SIGNED: operator\nMAP: MAP.md\n' > MAP-HUMAN
impl_ok 'live + MAP-HUMAN + same kind can start maker (3d is HIGH-only)' \
  "$WM" run maker-falsify || true

# Task 6 owns the walker; cadence must not turn loop into a daemon.
if out=$("$WM" loop 2>"$ERR"); then
  printf '%s\n' "$out" | grep -q 'LOOP STUB' \
    && ok || bad "wm loop must stay a foreground stub, got $out"
else
  bad "wm loop stub refused: $(cat "$ERR")"
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
    architecture|critique|review|loop-design) ;;
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
