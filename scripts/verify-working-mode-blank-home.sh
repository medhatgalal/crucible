#!/bin/sh
# 13b: harness adapters + blank-HOME tarball adopt + 3-slice fixture walk.
# Falsifier-first: RED when adapters/docs/VERSION 1.16.3 pin are absent.
# Fixture echo/python stubs only. Live grok/kiro-cli/codex are not invoked.
# If none of those CLIs are on PATH: record INDEPENDENCE_UNAVAILABLE
# (not a fixture fail). Claude Code is not required.
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }

# --- load-bearing RED (13b pin) ---
if [ ! -f "$HERE/adapters/grok.md" ] \
  || [ ! -f "$HERE/adapters/kiro.md" ] \
  || [ ! -f "$HERE/adapters/claude.md" ] \
  || [ ! -f "$HERE/adapters/codex.md" ]; then
  printf 'RED adapters/{grok,kiro,claude,codex}.md missing (13b)\n' >&2
  exit 1
fi

VERSION=$(sed -n '1p' "$HERE/VERSION")
if [ "$VERSION" != 1.16.3 ]; then
  printf 'RED VERSION is %s, want 1.16.3\n' "$VERSION" >&2
  exit 1
fi

if ! grep -q '^## \[1.16.3\] - 2026-09-19$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.16.3] - 2026-09-19\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.16.2\] - 2026-09-18$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.16.2] - 2026-09-18\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.16.1\] - 2026-09-18$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.16.1] - 2026-09-18\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.16.0\] - 2026-09-18$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.16.0] - 2026-09-18\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.15.2\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.15.2] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.15.1\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.15.1] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.15.0\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.15.0] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.14.2\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.14.2] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.14.1\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.14.1] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.14.0\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.14.0] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.13.0\] - 2026-09-16$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.13.0] - 2026-09-16\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.12.0\] - 2026-09-15$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.12.0] - 2026-09-15\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.11.0\] - 2026-09-15$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.11.0] - 2026-09-15\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.10.1\] - 2026-09-15$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.10.1] - 2026-09-15\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.10.0\] - 2026-09-15$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.10.0] - 2026-09-15\n' >&2
  exit 1
fi

if ! grep -q '^## \[1.9.0\] - 2026-09-13$' "$HERE/CHANGELOG.md"; then
  printf 'RED CHANGELOG.md missing ## [1.9.0] - 2026-09-13\n' >&2
  exit 1
fi

if [ ! -f "$HERE/docs/whats-new.md" ] \
  || ! grep -qi 'working-mode' "$HERE/docs/whats-new.md"; then
  printf 'RED docs/whats-new.md does not mention working-mode\n' >&2
  exit 1
fi

sh -n "$HERE/scripts/verify-working-mode-blank-home.sh" && ok \
  || bad 'verify-working-mode-blank-home.sh is not valid POSIX sh'
sh -n "$HERE/wm.sh" && ok || bad 'wm.sh is not valid POSIX sh'
sh -n "$HERE/crucible" && ok || bad 'crucible is not valid POSIX sh'

# Adapters: how to invoke; point agents.tsv at CLIs; no secrets.
for name in grok kiro claude codex; do
  f="$HERE/adapters/$name.md"
  [ -s "$f" ] && ok || bad "adapters/$name.md empty"
  if grep -q 'agents.tsv' "$f"; then
    ok
  else
    bad "adapters/$name.md must point agents.tsv at the CLI"
  fi
  if grep -Ei 'api[_-]?key[[:space:]]*[=:][[:space:]]*[^[:space:]]|secret[[:space:]]*[=:][[:space:]]*[^[:space:]]|BEGIN [A-Z]+ PRIVATE|sk-[A-Za-z0-9]{10,}' "$f" >/dev/null; then
    bad "adapters/$name.md contains secrets material"
  else
    ok
  fi
done

# Four batteries only; adapters are not batteries.
extra=
for d in "$HERE/skills"/*; do
  [ -d "$d" ] || continue
  n=${d##*/}
  case $n in
    architecture|critique|review|loop-design|working-mode|research|repo-scout|crucible) ;;
    *) extra="$extra $n" ;;
  esac
done
if [ -z "$extra" ]; then
  ok
else
  bad "unexpected package battery:$extra"
fi

command -v python3 >/dev/null 2>&1 && ok \
  || bad 'python3 required for the 3-slice fixture package'

command -v git >/dev/null 2>&1 && ok || bad 'git required'

# Live harness CLIs: detect only. Never invoke. None of
# grok/kiro-cli/codex → INDEPENDENCE_UNAVAILABLE. Claude is not required.
LIVE_GROK=0
LIVE_KIRO=0
LIVE_CODEX=0
command -v grok >/dev/null 2>&1 && LIVE_GROK=1
command -v kiro-cli >/dev/null 2>&1 && LIVE_KIRO=1
command -v codex >/dev/null 2>&1 && LIVE_CODEX=1
LIVE_N=$((LIVE_GROK + LIVE_KIRO + LIVE_CODEX))
if [ "$LIVE_N" -ge 1 ]; then
  INDEPENDENCE=live-clis-present
  printf 'LIVE_CLIS_PRESENT grok kiro-cli codex (fixture 3-slice; live agents not invoked)\n'
else
  INDEPENDENCE=INDEPENDENCE_UNAVAILABLE
  printf 'INDEPENDENCE_UNAVAILABLE: live grok/kiro-cli/codex CLI missing (need >=1; grok=%s kiro-cli=%s codex=%s); fixture 3-slice still required\n' \
    "$LIVE_GROK" "$LIVE_KIRO" "$LIVE_CODEX"
fi

BASE=$(mktemp -d "${TMPDIR:-/tmp}/wm-blank-home.XXXXXX")
EMPTY_HOME=$(mktemp -d "${TMPDIR:-/tmp}/wm-empty-home.XXXXXX")
OUT="$BASE/out.txt"
ERR="$BASE/err.txt"
HOME="$EMPTY_HOME"
export HOME
GIT_CONFIG_GLOBAL=/dev/null
GIT_CONFIG_SYSTEM=/dev/null
export GIT_CONFIG_GLOBAL GIT_CONFIG_SYSTEM
PYTHONDONTWRITEBYTECODE=1
export PYTHONDONTWRITEBYTECODE

trap 'rm -rf "$BASE" "$EMPTY_HOME"' 0
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 129' 1
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 130' 2
trap 'rm -rf "$BASE" "$EMPTY_HOME"; exit 143' 15

assert_home_empty() {
  leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
  if [ -z "$leftovers" ]; then
    ok
  else
    bad "wrote under HOME: $leftovers"
  fi
}

assert_no_home_skills() {
  hits=$(find "$EMPTY_HOME" \( -name SKILL.md -o -name CONTRACT.md \) -print 2>/dev/null || true)
  if [ -n "$hits" ]; then
    bad "skill files under HOME: $hits"
    return
  fi
  hits=$(find "$EMPTY_HOME" \( -path '*/.grok/skills/*' -o -path '*/.claude/skills/*' \
    -o -path '*/.agents/skills/*' \) -print 2>/dev/null || true)
  if [ -n "$hits" ]; then
    bad "harness skill trees under HOME: $hits"
  else
    ok
  fi
}

init_git_repo() {
  dir=$1
  mkdir -p "$dir"
  (
    CDPATH=
    cd "$dir"
    git init -q
    git config user.email 'wm@local'
    git config user.name 'working-mode'
    printf 'tracked\n' > README
    git add README
    git -c user.email=wm@local -c user.name=working-mode commit -qm baseline
  )
}

run_adopt() {
  dir=$1
  shift
  ( CDPATH=; cd "$dir" && "$@" >"$OUT" 2>"$ERR" )
}

# Tarball pin: package-release archives git REF (HEAD must record VERSION 1.16.3).
PKG_OUT="$BASE/pkg"
mkdir -p "$PKG_OUT"
if "$HERE/scripts/package-release.sh" "$VERSION" HEAD "$PKG_OUT" >"$OUT" 2>"$ERR"; then
  ok
else
  bad "package-release refused: $(cat "$OUT") $(cat "$ERR")"
fi
TAR="$PKG_OUT/crucible-$VERSION.tar.gz"
if [ -f "$TAR" ]; then
  ok
  CONTENTS=$(tar -tzf "$TAR")
  for rel in \
    wm.sh \
    adapters/grok.md \
    adapters/kiro.md \
    adapters/claude.md \
    adapters/codex.md \
    skills/architecture/SKILL.md \
    skills/critique/SKILL.md \
    skills/review/SKILL.md \
    skills/loop-design/SKILL.md \
    ROUTING.tsv \
    docs/whats-new.md \
    docs/working-mode.md
  do
    printf '%s\n' "$CONTENTS" | grep -q "^crucible-$VERSION/$rel\$" \
      && ok || bad "tarball missing $rel"
  done
else
  bad "package-release did not write $TAR"
  CONTENTS=
fi

EXTRACT=
if [ -f "$TAR" ]; then
  mkdir -p "$BASE/extract"
  tar -xzf "$TAR" -C "$BASE/extract"
  EXTRACT="$BASE/extract/crucible-$VERSION"
  [ -x "$EXTRACT/crucible" ] && ok || bad 'extracted crucible is not executable'
  [ -f "$EXTRACT/adapters/grok.md" ] && ok || bad 'extracted package missing adapters/grok.md'
fi

# 1b: default adopt from the tarball still has no wm/skills (guided 1.6.6).
if [ -n "$EXTRACT" ]; then
  init_git_repo "$BASE/guided"
  if run_adopt "$BASE/guided" "$EXTRACT/crucible" adopt work --managed; then
    ok
  else
    bad "default adopt --managed refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  G="$BASE/guided"
  [ ! -f "$G/.crucible/work/wm.sh" ] && ok || bad 'default adopt copied wm.sh'
  [ ! -e "$G/.crucible/skills" ] && ok || bad 'default adopt copied .crucible/skills'
  [ ! -e "$G/.grok/skills" ] && ok || bad 'default adopt copied .grok/skills'
  [ ! -e "$G/.claude/skills" ] && ok || bad 'default adopt copied .claude/skills'
  [ ! -e "$G/.agents/skills" ] && ok || bad 'default adopt copied .agents/skills'
  [ ! -d "$G/.crucible/work/adapters" ] && ok || bad 'default adopt copied adapters'
  assert_home_empty
  assert_no_home_skills
fi

# Independent proof: unpack tarball, adopt --working-mode, empty HOME.
AD=
if [ -n "$EXTRACT" ]; then
  init_git_repo "$BASE/adopted"
  if run_adopt "$BASE/adopted" "$EXTRACT/crucible" adopt work --managed --working-mode; then
    ok
  else
    bad "adopt --working-mode from tarball refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  AD="$BASE/adopted"
  [ -f "$AD/.crucible/work/wm.sh" ] && ok || bad 'tarball adopt missing wm.sh'
  [ -f "$AD/.crucible/work/adapters/grok.md" ] && ok || bad 'adopt missing adapters/grok.md'
  [ -f "$AD/.crucible/work/adapters/kiro.md" ] && ok || bad 'adopt missing adapters/kiro.md'
  [ -f "$AD/.crucible/work/adapters/claude.md" ] && ok || bad 'adopt missing adapters/claude.md'
  [ -f "$AD/.crucible/work/adapters/codex.md" ] && ok || bad 'adopt missing adapters/codex.md'
  [ -f "$AD/.crucible/skills/architecture/SKILL.md" ] && ok || bad 'canonical architecture missing'
  [ -f "$AD/.grok/skills/architecture/SKILL.md" ] && ok || bad 'repo-root grok view missing'
  [ -f "$AD/.claude/skills/architecture/SKILL.md" ] && ok || bad 'repo-root claude view missing'
  [ -f "$AD/.agents/skills/architecture/SKILL.md" ] && ok || bad 'repo-root agents view missing'
  [ -f "$AD/.crucible/.grok/skills/architecture/SKILL.md" ] && ok || bad 'nested grok view missing'
  [ -f "$AD/.crucible/.claude/skills/architecture/SKILL.md" ] && ok || bad 'nested claude view missing'
  [ -f "$AD/.crucible/.agents/skills/architecture/SKILL.md" ] && ok || bad 'nested agents view missing'
  if [ -f "$AD/.crucible/work/ENGINE-SOURCE" ]; then
    grep -q '^version: 1.16.3$' "$AD/.crucible/work/ENGINE-SOURCE" \
      && ok || bad 'ENGINE-SOURCE version is not 1.16.3'
  else
    bad 'ENGINE-SOURCE missing after working-mode adopt'
  fi
  assert_home_empty
  assert_no_home_skills
fi

# ---------------------------------------------------------------------------
# 3-slice fixture: tiny Python package, two module slices + one seam falsifier.
# Mapper process ≠ critique process ≠ maker process. Reviewer re-runs falsifier.
# ---------------------------------------------------------------------------
walk_rc=1
if [ -n "$AD" ] && [ -f "$AD/.crucible/work/wm.sh" ]; then
  WM="$AD/.crucible/work/wm.sh"
  chmod +x "$WM" 2>/dev/null || true
  export WM_ENGINE="$WM"
  CDPATH=
  cd "$AD"

  mkdir -p pkg/alpha pkg/beta tests architecture tools reviews

  printf 'an idea: tiny ping/pong package\n' > IDEA.md

  cat > architecture/modules.md <<'EOF'
module_id	root_path	public_contracts	test_entrypoint	pattern_instance	live_write
alpha	pkg/alpha	pkg/alpha/api.py	tools/check_alpha.py	pkg/alpha/api.py	no
beta	pkg/beta	pkg/beta/api.py	tools/check_beta.py	pkg/beta/api.py	no
seam	tests	tests/test_seam.py	tests/test_seam.py	tests/test_seam.py	no
EOF

  : > pkg/__init__.py
  : > pkg/alpha/__init__.py
  : > pkg/beta/__init__.py
  printf 'def ping():\n    raise NotImplementedError\n' > pkg/alpha/api.py
  printf 'def pong():\n    raise NotImplementedError\n' > pkg/beta/api.py
  # te paths must exist at maker-falsify (1.13 CHECK). Maker-build overwrites.
  printf 'import sys; sys.exit(1)\n' > tools/check_alpha.py
  printf 'import sys; sys.exit(1)\n' > tools/check_beta.py
  printf 'import sys; sys.exit(1)\n' > tests/test_seam.py

  cat > SPEC.md <<'EOF'
## Goal
tiny python package: alpha ping, beta pong, seam test
## Non-goals
live systems, network, extra modules
## Owned files
- pkg/alpha/api.py
- pkg/beta/api.py
- tests/test_seam.py
## Test files
- tests/test_seam.py
## Acceptance criteria
- ping returns alpha; pong returns beta; seam imports both
## Focused falsifier
MAKER-WRITES
## Stop conditions
stop-ask on live write
## Risk
LOW
SPEC-AUTHOR: operator
EOF

  cat > INTENT.md <<'EOF'
## User
fixture operator
## Job
tiny python package: alpha ping, beta pong, seam test
## Non-goals
live systems, network, extra modules
EOF

  cat > tools/architecture-agent.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/mapper-ran
printf 'pid %s\n' "$$" > .wm/mapper-pid
cat > MAP.md <<'MAP'
MAPPER: alice

id	module	owned_paths	depends_on	risk
s1	alpha	pkg/alpha/api.py	-	LOW
s2	beta	pkg/beta/api.py	s1	LOW
s3	seam	tests/test_seam.py	s2	LOW
MAP
.wm/bin/wm record-mapper --from MAP.md
EOF

  cat > tools/critique-agent.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/critique-ran
printf 'pid %s\n' "$$" > .wm/critique-pid
mkdir -p reviews .wm/return
cat > reviews/critique.md <<'REV'
## Invert
Two modules plus a seam. Failure-first: a path outside alpha/beta/tests invents a fourth module.

## Adversarial
Attack: mapper later acting as maker; seam falsifier that never imports both modules.

## Simple
Complected: treating MAP-ACCEPT as CLOSED PASS. Keep map words distinct from brick close.
REV
printf 'WORD: MAP-ACCEPT\nAGENT: bob\nMAP: MAP.md\n' > .wm/return/bob.md
.wm/bin/wm check-map-word .wm/return/bob.md
EOF

  cat > tools/check_alpha.py <<'EOF'
from pkg.alpha.api import ping
assert ping() == "alpha"
EOF
  cat > tools/check_beta.py <<'EOF'
from pkg.beta.api import pong
assert pong() == "beta"
EOF

  cat > tools/maker.sh <<'EOF'
#!/bin/sh
set -eu
role=
if [ -f .wm/dispatch ]; then
  role=$(awk -F ': ' '$1=="role"{print $2; exit}' .wm/dispatch)
fi
if [ -z "$role" ] && [ -n "${BRIEF:-}" ] && [ -f "$BRIEF" ]; then
  role=$(awk -F ': ' '$1=="role"{print $2; exit}' "$BRIEF")
fi
sid=
if [ -f .wm/slice-in-flight ]; then
  sid=$(awk -F ': ' '$1=="id"{print $2; exit}' .wm/slice-in-flight)
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
printf 'ran %s %s\n' "$role" "$sid" >> .wm/maker-ran
printf 'pid %s\n' "$$" > .wm/maker-pid
if [ "$role" = maker-build ]; then
  case $sid in
    s1)
      printf 'def ping():\n    return "alpha"\n' > pkg/alpha/api.py
      git add pkg/alpha/api.py
      git commit -qm 'maker-build s1 alpha'
      ;;
    s2)
      printf 'def pong():\n    return "beta"\n' > pkg/beta/api.py
      git add pkg/beta/api.py
      git commit -qm 'maker-build s2 beta'
      ;;
    s3)
      cat > tests/test_seam.py <<'PY'
from pkg.alpha.api import ping
from pkg.beta.api import pong

def test_seam():
    assert ping() == "alpha"
    assert pong() == "beta"

if __name__ == "__main__":
    test_seam()
PY
      git add tests/test_seam.py
      git commit -qm 'maker-build s3 seam'
      ;;
    *)
      printf 'unknown slice %s\n' "$sid" >&2
      exit 1
      ;;
  esac
  exit 0
fi
mkdir -p .wm
case $sid in
  s1) printf 'PYTHONPATH=. python3 tools/check_alpha.py\n' > .wm/FALSIFIER ;;
  s2) printf 'PYTHONPATH=. python3 tools/check_beta.py\n' > .wm/FALSIFIER ;;
  s3) printf 'PYTHONPATH=. python3 tests/test_seam.py\n' > .wm/FALSIFIER ;;
  *)
    printf 'unknown slice %s\n' "$sid" >&2
    exit 1
    ;;
esac
h=$(sha_of .wm/FALSIFIER)
wid=NOCOMMIT
if git rev-parse --verify HEAD >/dev/null 2>&1; then
  wid=$(git rev-parse --short=12 HEAD)
fi
printf 'agent: carol\nwork-id: %s\nsha256: %s\n' "$wid" "$h" > .wm/FALSIFIER.meta
EOF

  cat > tools/reviewer.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/reviewer-ran
printf 'pid %s\n' "$$" > .wm/reviewer-pid
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
printf 'reran %s\n' "$cmd" >> .wm/reviewer-reran
EOF

  chmod +x tools/architecture-agent.sh tools/critique-agent.sh tools/maker.sh tools/reviewer.sh

  if ! "$WM" init >"$OUT" 2>"$ERR"; then
    bad "wm init refused: $(cat "$OUT") $(cat "$ERR")"
  else
    ok
  fi
  "$WM" cast coordinator parent grok - >/dev/null 2>"$ERR" || true

  # Distinct processes: architecture, then critique, then brick maker/reviewer.
  if ./tools/architecture-agent.sh >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "architecture agent refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  [ -f MAP.md ] && grep -q '^s1	alpha	pkg/alpha/api.py	-	LOW$' MAP.md \
    && ok || bad 'architecture agent did not write 3-slice MAP.md'
  grep -q '^s2	beta	pkg/beta/api.py	s1	LOW$' MAP.md \
    && ok || bad 'MAP.md missing s2 beta depends_on=s1'
  grep -q '^s3	seam	tests/test_seam.py	s2	LOW$' MAP.md \
    && ok || bad 'MAP.md missing s3 seam depends_on=s2'
  [ -f .wm/mapper-ran ] && ok || bad 'mapper process did not run'

  if "$WM" map-ready >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "map-ready refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  [ -f slices.tsv ] && ok || bad 'map-ready did not write slices.tsv'
  nslice=$(awk 'NR>1 && NF {c++} END{print c+0}' slices.tsv)
  [ "$nslice" -eq 3 ] && ok || bad "slices.tsv wanted 3 rows, got $nslice"

  if ./tools/critique-agent.sh >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "critique agent refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  [ -f .wm/critique-ran ] && ok || bad 'critique process did not run'
  if "$WM" map-verdict .wm/return/bob.md >"$OUT" 2>"$ERR"; then
    grep -q 'MAP-ACCEPT' "$OUT" && ok || bad "map-verdict wanted MAP-ACCEPT, got $(cat "$OUT")"
  else
    bad "map-verdict refused: $(cat "$OUT") $(cat "$ERR")"
  fi

  if "$WM" cast maker carol grok './tools/maker.sh {BRIEF}' >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "cast maker refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  if "$WM" cast reviewer dave grok './tools/reviewer.sh {BRIEF}' >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "cast reviewer refused: $(cat "$OUT") $(cat "$ERR")"
  fi

  mapper=$(awk -F ': ' '$1=="id"{print $2; exit}' .wm/mapper)
  [ "$mapper" = alice ] && ok || bad "mapper id wanted alice, got $mapper"
  [ "$mapper" != carol ] && ok || bad 'mapper is maker (alice==carol)'
  [ "$mapper" != bob ] && ok || bad 'mapper is critique'
  if [ -f .wm/mapper-pid ] && [ -f .wm/critique-pid ]; then
    mp=$(awk '{print $2}' .wm/mapper-pid)
    cp=$(awk '{print $2}' .wm/critique-pid)
    [ "$mp" != "$cp" ] && ok || bad "mapper pid equals critique pid ($mp)"
  else
    bad 'mapper/critique pids missing'
  fi

  git add -A
  if git -c user.email=wm@local -c user.name=working-mode commit -qm '3-slice fixture map+spec+workers'; then
    ok
  else
    bad 'fixture commit of map/spec/workers refused'
  fi

  # (A5) walk must not define mark_slice_closed or reset_brick
  if grep -E -q 'mark_slice_closed\(\)|reset_brick\(\)' \
    "$HERE/scripts/verify-working-mode-blank-home.sh"; then
    bad 'A5: blank-home must not define mark_slice_closed or reset_brick'
  else
    ok
  fi

  card=$("$WM" next 2>"$ERR") || {
    bad "wm next before walk refused: $(cat "$ERR")"
    card=
  }
  printf '%s\n' "$card" | grep -E -q '^NEXT SLICE s1$' \
    && ok || bad "walk next wanted NEXT SLICE s1, got $card"

  set +e
  "$WM" loop >"$OUT" 2>"$ERR"
  LOOP_RC=$?
  set -e
  if pgrep -f "$WM loop" >/dev/null 2>&1; then
    bad "leftover wm.sh loop process"
  else
    ok
  fi
  if [ "$LOOP_RC" -eq 0 ] && grep -q 'CLOSED PASS' "$OUT" \
    && [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
    ok
  else
    bad "one wm loop wanted CLOSED PASS rc=0, got rc=$LOOP_RC out=$(cat "$OUT") err=$(cat "$ERR")"
  fi
  [ -f .wm/reviewer-ran ] && ok || bad "reviewer CLI not exec'd"
  [ -f .wm/reviewer-reran ] && ok || bad "reviewer did not re-run falsifier"
  grep -q 'check_alpha' .wm/reviewer-reran 2>/dev/null \
    && ok || bad 's1 falsifier not re-run'
  grep -q 'check_beta' .wm/reviewer-reran 2>/dev/null \
    && ok || bad 's2 falsifier not re-run'
  grep -q 'test_seam' .wm/reviewer-reran 2>/dev/null \
    && ok || bad 's3 falsifier not re-run'
  [ -f .wm/maker-ran ] && ok || bad "maker process did not run"
  grep -q ' s1$' .wm/maker-ran 2>/dev/null && ok || bad 's1 maker did not run'
  grep -q ' s2$' .wm/maker-ran 2>/dev/null && ok || bad 's2 maker did not run'
  grep -q ' s3$' .wm/maker-ran 2>/dev/null && ok || bad 's3 maker did not run'
  if [ -f .wm/maker-pid ] && [ -f .wm/mapper-pid ]; then
    mkp=$(awk '{print $2}' .wm/maker-pid)
    mp=$(awk '{print $2}' .wm/mapper-pid)
    [ "$mkp" != "$mp" ] && ok || bad "maker pid equals mapper pid ($mkp)"
  else
    bad "maker/mapper pids missing"
  fi
  if [ -f .wm/invoke/reviewer.log ] && grep -q 'writer: wm-run' .wm/invoke/reviewer.log; then
    ok
  else
    bad "missing invoke.log writer: wm-run"
  fi
  grep -q 'return "alpha"' pkg/alpha/api.py && ok || bad 's1 did not land alpha ping'
  grep -q 'return "beta"' pkg/beta/api.py && ok || bad 's2 did not land beta pong'
  [ -f tests/test_seam.py ] && ok || bad 's3 did not land tests/test_seam.py'
  if PYTHONPATH=. python3 tests/test_seam.py; then
    ok
  else
    bad 'seam falsifier still failing after 3-slice walk'
  fi
  closed_rows=$(awk -F '\t' 'NR>1 && $6=="CLOSED" {c++} END{print c+0}' slices.tsv)
  [ "$closed_rows" -eq 3 ] && ok || bad "slices.tsv CLOSED rows wanted 3, got $closed_rows"
  grep -q 'NEXT SLICE s2' "$OUT" \
    && ok || bad 'one wm loop must emit NEXT SLICE s2 after s1'
  grep -q 'NEXT SLICE s3' "$OUT" \
    && ok || bad 'one wm loop must emit NEXT SLICE s3 after s2'
  walk_rc=0
  cd "$HERE"
fi

assert_home_empty
assert_no_home_skills

printf 'INDEPENDENCE=%s\n' "$INDEPENDENCE"
printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
[ "$walk_rc" -eq 0 ]
