#!/bin/sh
# 13b live arm: throwaway tarball adopt + real harness CLIs.
# Zero of grok/kiro-cli/codex on PATH → INDEPENDENCE_UNAVAILABLE exit 1.
# One kind proceeds SUBAGENT-ISOLATED (k2=k1; distinct agent ids).
# Two kinds split maker/reviewer CROSS-FAMILY. Never fake CROSS-FAMILY.
# Claude Code is not required.
# Fail closed: those present binaries cannot auth under empty HOME →
# INDEPENDENCE_UNAVAILABLE: <cli> cannot auth (exit 1). Do not grok-only PASS.
# Host auth/config is copied (grok auth.json+config.toml, kiro settings/cli.json,
# codex auth.json+config.toml). Skills/bundled/sessions are not copied.
# Do not use kiro-cli acp (JSON-RPC server) as wm run argv.
# One-or-more auth: health-check IDEA (not hello); specifier/scout/maker/reviewer
# live CLIs (not architecture-agent.sh / critique-agent.sh); prefer wm go;
# four distinct PIDs; one go/loop. PATH-stripped still exit 1
# INDEPENDENCE_UNAVAILABLE. Not a required CI gate.
# Run from the worktree against an extracted tarball (prefer not as a tarball payload).
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
HOST_HOME=${HOME:-}
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }
die_unavail() {
  printf 'INDEPENDENCE_UNAVAILABLE: %s\n' "$1"
  exit 1
}

if [ ! -f "$HERE/wm.sh" ] || [ ! -f "$HERE/scripts/package-release.sh" ]; then
  printf 'RED wm.sh or package-release.sh missing\n' >&2
  exit 1
fi

sh -n "$HERE/scripts/verify-working-mode-live.sh" && ok \
  || bad 'verify-working-mode-live.sh is not valid POSIX sh'
sh -n "$HERE/wm.sh" && ok || bad 'wm.sh is not valid POSIX sh'

# This script must not reimplement kernel slice-close (Task 1 owns that).
if grep -E -q 'mark_slice_closed\(\)|reset_brick\(\)' \
  "$HERE/scripts/verify-working-mode-live.sh"; then
  bad 'live script must not define mark_slice_closed or reset_brick'
else
  ok
fi
if grep -E -q '\./tools/(architecture|critique)-agent\.sh' \
  "$HERE/scripts/verify-working-mode-live.sh"; then
  bad 'live walk must not use architecture-agent.sh / critique-agent.sh as workers'
else
  ok
fi
# F3: walk product is a health-check app, not hello. These greps are the
# script's own contract; they run before PATH/auth so PATH-stripped still
# proves the IDEA is not hello, then die_unavail as today.
# Pattern is split so this CHECK line does not match itself.
if grep -E -q 'one-file[[:space:]]+hello[[:space:]]+product' \
  "$HERE/scripts/verify-working-mode-live.sh"; then
  bad 'live IDEA must not be the hello product'
else
  ok
fi
if grep -E -q 'HTTP GET /health|tests/test_health' \
  "$HERE/scripts/verify-working-mode-live.sh"; then
  ok
else
  bad 'live IDEA must be a health-check app'
fi
if grep -F -q '"$WM" go' "$HERE/scripts/verify-working-mode-live.sh"; then
  ok
else
  bad 'live walk must prefer wm go when present'
fi
command -v git >/dev/null 2>&1 && ok || bad 'git required'
command -v tar >/dev/null 2>&1 && ok || bad 'tar required'

# --- fail closed: need >=1 of grok/kiro-cli/codex (not 3 including claude)
LIVE_GROK=0
LIVE_KIRO=0
LIVE_CODEX=0
command -v grok >/dev/null 2>&1 && LIVE_GROK=1
command -v kiro-cli >/dev/null 2>&1 && LIVE_KIRO=1
command -v codex >/dev/null 2>&1 && LIVE_CODEX=1
LIVE_N=$((LIVE_GROK + LIVE_KIRO + LIVE_CODEX))
if [ "$LIVE_N" -lt 1 ]; then
  die_unavail "live grok/kiro-cli/codex CLI missing (need >=1; grok=$LIVE_GROK kiro-cli=$LIVE_KIRO codex=$LIVE_CODEX)"
fi

GROK_BIN=
KIRO_BIN=
CODEX_BIN=
[ "$LIVE_GROK" -eq 1 ] && GROK_BIN=$(command -v grok)
[ "$LIVE_KIRO" -eq 1 ] && KIRO_BIN=$(command -v kiro-cli)
[ "$LIVE_CODEX" -eq 1 ] && CODEX_BIN=$(command -v codex)
export GROK_BIN KIRO_BIN CODEX_BIN
GROK_AGENT_DASHBOARD=0
export GROK_AGENT_DASHBOARD

_live_k1=
_live_k2=
if [ "$LIVE_GROK" -eq 1 ]; then
  if [ -z "$_live_k1" ]; then _live_k1=grok; else _live_k2=grok; fi
fi
if [ "$LIVE_KIRO" -eq 1 ]; then
  if [ -z "$_live_k1" ]; then _live_k1=kiro; else
    [ -n "$_live_k2" ] || _live_k2=kiro
  fi
fi
if [ "$LIVE_CODEX" -eq 1 ]; then
  if [ -z "$_live_k1" ]; then _live_k1=codex; else
    [ -n "$_live_k2" ] || _live_k2=codex
  fi
fi
[ -n "$_live_k2" ] || _live_k2=$_live_k1
LIVE_SPEC_KIND=$_live_k1
LIVE_MAKE_KIND=$_live_k1
LIVE_SCOUT_KIND=$_live_k2
LIVE_REV_KIND=$_live_k2
export LIVE_SPEC_KIND LIVE_MAKE_KIND LIVE_SCOUT_KIND LIVE_REV_KIND

VERSION=$(sed -n '1p' "$HERE/VERSION")
[ -n "$VERSION" ] || { printf 'RED VERSION missing\n' >&2; exit 1; }

BASE=$(mktemp -d "${TMPDIR:-/tmp}/wm-live.XXXXXX")
EMPTY_HOME=$(mktemp -d "${TMPDIR:-/tmp}/wm-live-home.XXXXXX")
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

# Auth/config only (no harness skill trees). Do not invent secrets; do not print them.
# Copy host grok auth.json + config.toml; overlay yolo/always-approve without
# dropping other keys. Copy kiro settings/cli.json; codex auth + config.
# Never copy skills/, bundled/, sessions/, or kiro-cli acp state.
copy_if_file() {
  src=$1
  dst=$2
  if [ -n "$HOST_HOME" ] && [ -f "$src" ]; then
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
    chmod 600 "$dst"
  fi
}

# Rewrite yolo / permission_mode in place. Do not replace the rest of the file.
overlay_grok_ui() {
  cfg=$1
  [ -f "$cfg" ] || return 0
  tmp=$cfg.overlay.$$
  awk '
    BEGIN { saw_yolo = 0; saw_perm = 0 }
    /^[[:space:]]*yolo[[:space:]]*=/ {
      print "yolo = true"
      saw_yolo = 1
      next
    }
    /^[[:space:]]*permission_mode[[:space:]]*=/ {
      print "permission_mode = \"always-approve\""
      saw_perm = 1
      next
    }
    { print }
    END {
      if (!saw_yolo || !saw_perm) {
        print ""
        print "[ui]"
        if (!saw_perm) print "permission_mode = \"always-approve\""
        if (!saw_yolo) print "yolo = true"
      }
    }
  ' "$cfg" > "$tmp"
  mv "$tmp" "$cfg"
  chmod 600 "$cfg"
}

run_timeout() {
  secs=$1
  shift
  _rt_rc=0
  if command -v timeout >/dev/null 2>&1; then
    timeout -s TERM "$secs" "$@" >/dev/null 2>/dev/null || _rt_rc=$?
    return "$_rt_rc"
  fi
  python3 -c '
import subprocess, sys
t=int(sys.argv[1])
cmd=sys.argv[2:]
try:
    r=subprocess.run(cmd, timeout=t, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    sys.exit(r.returncode)
except subprocess.TimeoutExpired:
    sys.exit(124)
except FileNotFoundError:
    sys.exit(127)
' "$secs" "$@" || _rt_rc=$?
  return "$_rt_rc"
}

copy_if_file "$HOST_HOME/.grok/auth.json" "$HOME/.grok/auth.json"
if [ -f "$HOST_HOME/.grok/config.toml" ]; then
  copy_if_file "$HOST_HOME/.grok/config.toml" "$HOME/.grok/config.toml"
  overlay_grok_ui "$HOME/.grok/config.toml"
elif [ -f "$HOME/.grok/auth.json" ]; then
  mkdir -p "$HOME/.grok"
  cat > "$HOME/.grok/config.toml" <<'EOF'
[ui]
permission_mode = "always-approve"
yolo = true
[memory]
enabled = false
[marketplace]
official_marketplace_auto_installed = true
default_skills_installs_purged = true
EOF
  chmod 600 "$HOME/.grok/config.toml"
fi
copy_if_file "$HOST_HOME/.kiro/settings/cli.json" "$HOME/.kiro/settings/cli.json"
copy_if_file "$HOST_HOME/.kiro/settings/permissions.yaml" "$HOME/.kiro/settings/permissions.yaml"
copy_if_file "$HOST_HOME/.codex/auth.json" "$HOME/.codex/auth.json"
copy_if_file "$HOST_HOME/.codex/config.toml" "$HOME/.codex/config.toml"

PROBE_DIR="$BASE/probe"
mkdir -p "$PROBE_DIR"
printf 'probe\n' > "$PROBE_DIR/README"
(
  CDPATH=
  cd "$PROBE_DIR"
  git init -q
  git config user.email 'wm@local'
  git config user.name 'working-mode'
  git add README
  git -c user.email=wm@local -c user.name=working-mode commit -qm probe
) >/dev/null 2>"$ERR" || true

# One-shot ping under empty HOME. Discard output (do not print secrets).
# Probe PATH CLIs; drop any that cannot auth. Zero usable → unavailable.
# Do not die because claude is missing. Do not die because one of three fails.
printf 'reply with pong only\n' > "$PROBE_DIR/ping.txt"
if [ "$LIVE_GROK" -eq 1 ]; then
  if ( CDPATH=; cd "$PROBE_DIR" && run_timeout 25 \
    "$GROK_BIN" --always-approve --no-subagents --disable-web-search \
    --output-format plain --max-turns 1 --prompt-file "$PROBE_DIR/ping.txt" ); then
    ok
  else
    LIVE_GROK=0
  fi
fi
if [ "$LIVE_KIRO" -eq 1 ]; then
  if ( CDPATH=; cd "$PROBE_DIR" && run_timeout 25 \
    "$KIRO_BIN" chat --no-interactive --trust-all-tools pong ); then
    ok
  else
    LIVE_KIRO=0
  fi
fi
if [ "$LIVE_CODEX" -eq 1 ]; then
  if ( CDPATH=; cd "$PROBE_DIR" && run_timeout 25 \
    "$CODEX_BIN" exec --skip-git-repo-check --dangerously-bypass-approvals-and-sandbox pong ); then
    ok
  else
    LIVE_CODEX=0
  fi
fi
LIVE_N=$((LIVE_GROK + LIVE_KIRO + LIVE_CODEX))
if [ "$LIVE_N" -lt 1 ]; then
  die_unavail "none of grok/kiro-cli/codex could auth (grok=$LIVE_GROK kiro-cli=$LIVE_KIRO codex=$LIVE_CODEX)"
fi
_live_k1=
_live_k2=
if [ "$LIVE_GROK" -eq 1 ]; then
  if [ -z "$_live_k1" ]; then _live_k1=grok; else _live_k2=grok; fi
fi
if [ "$LIVE_KIRO" -eq 1 ]; then
  if [ -z "$_live_k1" ]; then _live_k1=kiro; else
    [ -n "$_live_k2" ] || _live_k2=kiro
  fi
fi
if [ "$LIVE_CODEX" -eq 1 ]; then
  if [ -z "$_live_k1" ]; then _live_k1=codex; else
    [ -n "$_live_k2" ] || _live_k2=codex
  fi
fi
[ -n "$_live_k2" ] || _live_k2=$_live_k1
LIVE_SPEC_KIND=$_live_k1
LIVE_MAKE_KIND=$_live_k1
LIVE_SCOUT_KIND=$_live_k2
LIVE_REV_KIND=$_live_k2
export LIVE_SPEC_KIND LIVE_MAKE_KIND LIVE_SCOUT_KIND LIVE_REV_KIND

assert_no_home_skill_trees() {
  hits=$(find "$EMPTY_HOME" \( \
    -path '*/.grok/skills' -o -path '*/.grok/skills/*' \
    -o -path '*/.claude/skills' -o -path '*/.claude/skills/*' \
    -o -path '*/.agents/skills' -o -path '*/.agents/skills/*' \
    -o -path '*/.kiro/skills' -o -path '*/.kiro/skills/*' \
    \) -print 2>/dev/null || true)
  if [ -n "$hits" ]; then
    bad "harness skill trees under HOME: $hits"
    return
  fi
  hits=$(find "$EMPTY_HOME" -name SKILL.md \
    ! -path '*/.grok/bundled/*' \
    ! -path '*/.codex/*' \
    ! -path '*/.npm/*' \
    ! -path '*/playwright-core/*' \
    -print 2>/dev/null || true)
  if [ -n "$hits" ]; then
    bad "SKILL.md under HOME outside vendor bundled: $hits"
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

# Tarball of HEAD, extracted outside the worktree (src != dst).
PKG_OUT="$BASE/pkg"
mkdir -p "$PKG_OUT"
if "$HERE/scripts/package-release.sh" "$VERSION" HEAD "$PKG_OUT" >"$OUT" 2>"$ERR"; then
  ok
else
  bad "package-release refused: $(cat "$OUT") $(cat "$ERR")"
fi
TAR="$PKG_OUT/crucible-$VERSION.tar.gz"
EXTRACT=
if [ -f "$TAR" ]; then
  ok
  mkdir -p "$BASE/extract"
  tar -xzf "$TAR" -C "$BASE/extract"
  EXTRACT="$BASE/extract/crucible-$VERSION"
  [ -x "$EXTRACT/crucible" ] && ok || bad 'extracted crucible is not executable'
  [ -f "$EXTRACT/wm.sh" ] && ok || bad 'extracted package missing wm.sh'
else
  bad "package-release did not write $TAR"
fi

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
  [ -f "$AD/.crucible/skills/review/SKILL.md" ] && ok || bad 'canonical review battery missing'
  assert_no_home_skill_trees
fi

walk_rc=1
if [ -n "$AD" ] && [ -f "$AD/.crucible/work/wm.sh" ]; then
  WM="$AD/.crucible/work/wm.sh"
  chmod +x "$WM" 2>/dev/null || true
  export WM_ENGINE="$WM"
  GIT_AUTHOR_EMAIL=wm@local
  GIT_AUTHOR_NAME=working-mode
  GIT_COMMITTER_EMAIL=wm@local
  GIT_COMMITTER_NAME=working-mode
  export GIT_AUTHOR_EMAIL GIT_AUTHOR_NAME GIT_COMMITTER_EMAIL GIT_COMMITTER_NAME
  CDPATH=
  cd "$AD"

  mkdir -p architecture tools reviews tests

  cat > IDEA.md <<'EOF'
# IDEA

Local health-check app (not a hello-world product).

HTTP GET /health returns 200; pytest/python test in tests/test_health.py.

First user: an operator running that test in this repository.
First observable artifact: tests/test_health.py exits 0 when GET /health is 200.
v1 is local-only: Python stdlib, in-process handler (no public bind, no deploy).
Auth and billing are non-goals. Data: none.
Risk: LOW. live_write=no. No MAP-HUMAN.
Out of scope: SaaS, payments, credentials, curl/wget, public network.
EOF

  {
    cat <<'EOF'
You are a working-mode brick worker in a throwaway git repo. Read the brief
file passed to you and this card. Do only the role named in the brief.
Do not push. Do not use the network. Do not write under $HOME. Do not spawn
subagents. Do not edit wm.sh. Do not write .wm/CLOSED or verdicts if you are
the maker. Specifier must read IDEA.md. This product is a health-check app.
Do not own product/hello.txt. Do not implement a hello-world file.

Agent ids: specifier=spec0 scout=scout0 maker=make0 reviewer=rev0.

## specifier (agent spec0)
If the brief says write RESEARCH.md only, or RESEARCH.md is missing:
Write RESEARCH.md from IDEA.md (stack survey, constraints, non-goals,
competitors). Do not write SPEC.md or MAP.md yet. git add RESEARCH.md &&
git commit -m 'specifier: RESEARCH'. Exit.

Otherwise read IDEA.md. The idea is HTTP GET /health returns 200 with a
python test in tests/test_health.py. If underspecified, write QUESTIONS.md
(at most 7) and stop. When specified, write INTENT.md with headings:
## User
## Job
## Non-goals
(operator; GET /health 200; no network/auth/billing). Write SPEC.md with
required headings (Goal, Non-goals, Owned files, Test files, Acceptance
criteria, Focused falsifier MAKER-WRITES, Stop conditions, Risk LOW).
Owned files: health/app.py, health/test_health.py. Test files:
health/test_health.py. Do not author the falsifier command (MAKER-WRITES
only). mkdir -p health. Plant health/test_health.py as exactly:
import sys; sys.exit(1)
so the test_entrypoint exists at maker-falsify. Maker-build overwrites
this file. Keep te path health/test_health.py. All owned paths sit under
the health/ module root.

Write architecture/modules.md as tab-separated bytes:
EOF
    printf 'module_id\troot_path\tpublic_contracts\ttest_entrypoint\tpattern_instance\tlive_write\n'
    printf 'health\thealth\thealth/app.py\thealth/test_health.py\thealth/app.py\tno\n'
    cat <<'EOF'

Write MAP.md starting with MAPPER: spec0 then a blank line then tab-separated:
EOF
    printf 'id\tmodule\towned_paths\tdepends_on\trisk\n'
    printf 's1\thealth\thealth/app.py,health/test_health.py\t-\tLOW\n'
    cat <<'EOF'
git add INTENT.md SPEC.md architecture/modules.md MAP.md health/test_health.py && git commit -m 'specifier: SPEC MAP modules'
Do not implement the product. Do not write MAP-ACCEPT. Do not be the maker.

## scout (agent scout0)
Write reviews/critique.md with Invert, Adversarial, and Simple sections.
Write .wm/return/scout0.md with:
WORD: MAP-ACCEPT
AGENT: scout0
MAP: MAP.md
Then run: .wm/bin/wm check-map-word .wm/return/scout0.md
Do not author MAP.md. Do not be the specifier or maker.

## maker-falsify (agent make0)
Do not create health/app.py yet.
Write exactly one line to .wm/FALSIFIER:
python3 health/test_health.py
The command must include the test_entrypoint path health/test_health.py.
Do not use curl or wget.
Meta: compute sha256 of .wm/FALSIFIER (shasum -a 256 or sha256sum).
Write .wm/FALSIFIER.meta as three lines:
agent: make0
work-id: <git rev-parse --short=12 HEAD>
sha256: <hex digest of .wm/FALSIFIER>
Then: git add .wm/FALSIFIER .wm/FALSIFIER.meta && git commit -m 'maker-falsify s1'
Do not implement the product. Do not write .wm/return or reviews.

## maker-build (agent make0)
Implement a stdlib in-process GET /health -> 200 handler (no live bind).
Write health/app.py (handle GET /health returns 200) and
health/test_health.py (stdlib unittest; python3 health/test_health.py
exits 0). Optional empty health/__init__.py. git add those files &&
git commit -m 'maker-build s1'
Do not write verdicts, CLOSED, or return files. Do not use pip.

## reviewer (agent rev0)
Re-run the named falsifier. Capture the evidence path:
cmd=$(sed -n '1p' .wm/FALSIFIER)
ev=$(.wm/bin/wm evidence rev0 -- sh -c "$cmd")
Write reviews/review.md with ## Code and ## Testing (short is fine).
Write .wm/return/rev0.md with:
WORD: PASS
EVIDENCE: <exact path printed by wm evidence>
If the falsifier failed, use WORD: FAIL with the same EVIDENCE line.
Append one line to .wm/reviewer-reran: reran <cmd>
Do not edit product files. Do not be the maker.
EOF
  } > tools/WORKER.md

  cat > tools/live-exec.sh <<'EOF'
#!/bin/sh
set -eu
kind=${1:-}
brief=${2:-}
[ -n "$kind" ] || { printf 'live-exec: kind missing\n' >&2; exit 1; }
[ -n "$brief" ] && [ -f "$brief" ] || { printf 'live-exec: brief missing\n' >&2; exit 1; }
mkdir -p .wm
prompt=.wm/live-"$kind".prompt
{
  cat "$brief"
  printf '\n'
  cat tools/WORKER.md
} > "$prompt"
case $kind in
  grok)
    [ -n "${GROK_BIN:-}" ] || GROK_BIN=$(command -v grok)
    exec "$GROK_BIN" --always-approve --no-subagents --disable-web-search \
      --output-format plain --max-turns 40 --prompt-file "$prompt"
    ;;
  kiro)
    [ -n "${KIRO_BIN:-}" ] || KIRO_BIN=$(command -v kiro-cli)
    exec "$KIRO_BIN" chat --no-interactive --trust-all-tools "$(cat "$prompt")"
    ;;
  codex)
    [ -n "${CODEX_BIN:-}" ] || CODEX_BIN=$(command -v codex)
    exec "$CODEX_BIN" exec --dangerously-bypass-approvals-and-sandbox - < "$prompt"
    ;;
  *)
    printf 'live-exec: unknown kind %s\n' "$kind" >&2
    exit 1
    ;;
esac
EOF

  cat > tools/live-specifier.sh <<EOF
#!/bin/sh
set -eu
mkdir -p .wm
printf 'pid %s\\n' "\$\$" > .wm/specifier-pid
printf 'ran\\n' > .wm/specifier-ran
brief=\${1:-\${BRIEF:-}}
[ -n "\$brief" ] && [ -f "\$brief" ] || { printf 'live-specifier: brief missing\\n' >&2; exit 1; }
exec ./tools/live-exec.sh ${LIVE_SPEC_KIND} "\$brief"
EOF

  cat > tools/live-scout.sh <<EOF
#!/bin/sh
set -eu
mkdir -p .wm
printf 'pid %s\\n' "\$\$" > .wm/scout-pid
printf 'ran\\n' > .wm/scout-ran
brief=\${1:-\${BRIEF:-}}
[ -n "\$brief" ] && [ -f "\$brief" ] || { printf 'live-scout: brief missing\\n' >&2; exit 1; }
exec ./tools/live-exec.sh ${LIVE_SCOUT_KIND} "\$brief"
EOF

  cat > tools/live-maker.sh <<EOF
#!/bin/sh
set -eu
printf 'pid %s\\n' "\$\$" > .wm/maker-pid
printf 'ran\\n' >> .wm/maker-ran
brief=\${1:-\${BRIEF:-}}
[ -n "\$brief" ] && [ -f "\$brief" ] || { printf 'live-maker: brief missing\\n' >&2; exit 1; }
set +e
./tools/live-exec.sh ${LIVE_MAKE_KIND} "\$brief"
rc=\$?
set -e
role=
if [ -f .wm/dispatch ]; then
  role=\$(awk -F ': ' '\$1=="role"{print \$2; exit}' .wm/dispatch)
fi
if [ "\$role" = maker-build ]; then
  git add health 2>/dev/null || true
  if ! git diff --cached --quiet; then
    git -c user.email=wm@local -c user.name=working-mode commit -qm 'maker-build s1'
  fi
fi
exit \$rc
EOF

  cat > tools/live-reviewer.sh <<EOF
#!/bin/sh
set -eu
printf 'pid %s\\n' "\$\$" > .wm/reviewer-pid
printf 'ran\\n' > .wm/reviewer-ran
brief=\${1:-\${BRIEF:-}}
[ -n "\$brief" ] && [ -f "\$brief" ] || { printf 'live-reviewer: brief missing\\n' >&2; exit 1; }
exec ./tools/live-exec.sh ${LIVE_REV_KIND} "\$brief"
EOF

  chmod +x tools/live-exec.sh tools/live-specifier.sh tools/live-scout.sh \
    tools/live-maker.sh tools/live-reviewer.sh

  if ! "$WM" init >"$OUT" 2>"$ERR"; then
    bad "wm init refused: $(cat "$OUT") $(cat "$ERR")"
  else
    ok
  fi
  "$WM" cast coordinator parent "$LIVE_SPEC_KIND" - >/dev/null 2>"$ERR" || true

  if "$WM" cast specifier spec0 "$LIVE_SPEC_KIND" './tools/live-specifier.sh {BRIEF}' \
    >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "cast specifier refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  if "$WM" cast scout scout0 "$LIVE_SCOUT_KIND" './tools/live-scout.sh {BRIEF}' \
    >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "cast scout refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  if "$WM" cast maker make0 "$LIVE_MAKE_KIND" './tools/live-maker.sh {BRIEF}' \
    >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "cast maker refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  if "$WM" cast reviewer rev0 "$LIVE_REV_KIND" './tools/live-reviewer.sh {BRIEF}' \
    >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "cast reviewer refused: $(cat "$OUT") $(cat "$ERR")"
  fi

  spec_id=$(awk -F '\t' '$1=="specifier"{print $2; exit}' .wm/PANEL.tsv)
  make_id=$(awk -F '\t' '$1=="maker"{print $2; exit}' .wm/PANEL.tsv)
  scout_id=$(awk -F '\t' '$1=="scout"{print $2; exit}' .wm/PANEL.tsv)
  rev_id=$(awk -F '\t' '$1=="reviewer"{print $2; exit}' .wm/PANEL.tsv)
  [ "$spec_id" = spec0 ] && ok || bad "specifier id wanted spec0, got $spec_id"
  [ "$scout_id" = scout0 ] && ok || bad "scout id wanted scout0, got $scout_id"
  [ "$make_id" = make0 ] && ok || bad "maker id wanted make0, got $make_id"
  [ "$rev_id" = rev0 ] && ok || bad "reviewer id wanted rev0, got $rev_id"
  [ "$spec_id" != "$make_id" ] && ok || bad 'specifier is maker'
  [ "$make_id" != "$rev_id" ] && ok || bad 'maker is reviewer'

  git add -A
  if git -c user.email=wm@local -c user.name=working-mode \
    commit -qm 'live health-check IDEA+workers'; then
    ok
  else
    bad 'fixture commit of IDEA/workers refused'
  fi

  printf 'NONE\n' > .wm/lesson

  set +e
  if [ -f "$(dirname "$WM")/wm-go.sh" ]; then
    "$WM" go >"$OUT" 2>"$ERR"
    LOOP_RC=$?
    walk_kind=go
  else
    "$WM" loop >"$OUT" 2>"$ERR"
    LOOP_RC=$?
    walk_kind=loop
  fi
  set -e
  leftover=0
  if pgrep -f "$WM loop" >/dev/null 2>&1; then
    leftover=1
  fi
  if pgrep -f "$WM go" >/dev/null 2>&1; then
    leftover=1
  fi
  if [ "$leftover" -eq 1 ]; then
    bad "leftover wm.sh loop/go process"
  else
    ok
  fi

  closed_word=
  if [ -f .wm/CLOSED ]; then
    closed_word=$(awk 'NF { print; exit }' .wm/CLOSED)
  fi
  if [ "$LOOP_RC" -eq 0 ] && [ -f .wm/CLOSED ] \
    && grep -Eq 'CLOSED (PASS|NO-BUILD)' .wm/CLOSED; then
    ok
  else
    bad "one wm $walk_kind wanted CLOSED PASS or CLOSED NO-BUILD rc=0, got rc=$LOOP_RC closed=$closed_word out=$(cat "$OUT") err=$(cat "$ERR") worker.err=$(cat .wm/worker.err 2>/dev/null || true)"
  fi

  iso=
  if [ -f .wm/CLOSED ]; then
    iso=$(awk -F ': ' '$1=="independence"{print $2; exit}' .wm/CLOSED)
  fi
  if [ "$LIVE_N" -ge 2 ]; then
    [ "$iso" = CROSS-FAMILY ] && ok || bad "two-kind live CLOSED wanted CROSS-FAMILY, got $iso"
  else
    [ "$iso" = SUBAGENT-ISOLATED ] && ok || bad "one-kind live CLOSED wanted SUBAGENT-ISOLATED, got $iso"
    grep -q CROSS-FAMILY .wm/CLOSED && bad 'one-kind must not fake CROSS-FAMILY' || ok
  fi

  [ -f .wm/specifier-ran ] && ok || bad "specifier CLI not exec'd"
  [ -f .wm/scout-ran ] && ok || bad "scout CLI not exec'd"
  [ -f .wm/maker-ran ] && ok || bad "maker process did not run"
  [ -f .wm/reviewer-ran ] && ok || bad "reviewer CLI not exec'd"
  [ -f .wm/specifier-pid ] && ok || bad "specifier pid missing"
  [ -f .wm/scout-pid ] && ok || bad "scout pid missing"
  [ -f .wm/maker-pid ] && ok || bad "maker pid missing"
  [ -f .wm/reviewer-pid ] && ok || bad "reviewer pid missing"

  if [ -f .wm/specifier-pid ] && [ -f .wm/scout-pid ] \
    && [ -f .wm/maker-pid ] && [ -f .wm/reviewer-pid ]; then
    sp=$(awk '{print $2}' .wm/specifier-pid)
    sc=$(awk '{print $2}' .wm/scout-pid)
    mkp=$(awk '{print $2}' .wm/maker-pid)
    rp=$(awk '{print $2}' .wm/reviewer-pid)
    uniq=$(printf '%s\n' "$sp" "$sc" "$mkp" "$rp" | awk 'NF' | sort -u | wc -l | tr -d ' ')
    if [ "$uniq" -eq 4 ]; then
      ok
    else
      bad "specifier/scout/maker/reviewer PIDs not 4 distinct (specifier=$sp scout=$sc maker=$mkp reviewer=$rp uniq=$uniq)"
    fi
  else
    bad 'four role pid files missing'
  fi

  if [ -f .wm/invoke/reviewer.log ] && grep -q 'writer: wm-run' .wm/invoke/reviewer.log; then
    ok
  else
    bad "missing invoke.log writer: wm-run"
  fi

  fals_cmd=
  if [ -f .wm/FALSIFIER ]; then
    fals_cmd=$(sed -n '1p' .wm/FALSIFIER)
  fi
  reran=0
  if [ -n "$fals_cmd" ] && [ -f .wm/reviewer-reran ] && grep -F -q "$fals_cmd" .wm/reviewer-reran; then
    reran=1
  fi
  if [ "$reran" -eq 0 ] && [ -n "$fals_cmd" ] && [ -d .wm/evidence ]; then
    if grep -F -l "$fals_cmd" .wm/evidence/*.txt >/dev/null 2>&1; then
      reran=1
    fi
  fi
  if [ "$reran" -eq 1 ]; then
    ok
  else
    bad "reviewer did not re-run named falsifier (cmd=$fals_cmd)"
  fi

  if [ -f IDEA.md ] && grep -E -qi 'health|/health|test_health' IDEA.md; then
    ok
  else
    bad 'IDEA.md must be a health-check app'
  fi
  spec_or_map=
  health_doc=0
  hello_owned=0
  if [ -f SPEC.md ]; then
    spec_or_map=1
    grep -E -qi 'health|/health|test_health' SPEC.md && health_doc=1
    grep -E -q '^- product/hello.txt' SPEC.md && hello_owned=1
  fi
  if [ -f MAP.md ]; then
    spec_or_map=1
    grep -E -qi 'health|/health|test_health' MAP.md && health_doc=1
    grep -E -q '^- product/hello.txt' MAP.md && hello_owned=1
  fi
  if [ -n "$spec_or_map" ] && [ "$health_doc" -eq 1 ]; then
    ok
  else
    bad 'SPEC.md or MAP.md must mention health (specifier must read IDEA)'
  fi
  if [ "$hello_owned" -eq 0 ]; then
    ok
  else
    bad 'SPEC.md/MAP.md must not own product/hello.txt'
  fi

  PRODUCT=health/test_health.py
  if grep -q 'CLOSED PASS' .wm/CLOSED 2>/dev/null; then
    if [ -f health/test_health.py ] || [ -f health/app.py ] || [ -f tests/test_health.py ]; then
      ok
    else
      bad "CLOSED PASS but health-check files missing"
    fi
    if [ -f product/hello.txt ]; then
      bad "CLOSED PASS must not be the hello product"
    else
      ok
    fi
  else
    ok
  fi

  if [ -f slices.tsv ]; then
    closed_rows=$(awk -F '\t' 'NR>1 && $6=="CLOSED" {c++} END{print c+0}' slices.tsv)
    [ "$closed_rows" -eq 1 ] && ok || bad "slices.tsv CLOSED rows wanted 1, got $closed_rows"
  else
    bad 'slices.tsv missing after walk'
  fi

  printf 'LIVE_GATE %s product=%s invoke=.wm/invoke/reviewer.log writer: wm-run\n' \
    "${closed_word:-FAIL}" "$PRODUCT"
  printf 'LIVE_PIDS specifier=%s scout=%s maker=%s reviewer=%s\n' \
    "$(awk '{print $2}' .wm/specifier-pid 2>/dev/null || echo -)" \
    "$(awk '{print $2}' .wm/scout-pid 2>/dev/null || echo -)" \
    "$(awk '{print $2}' .wm/maker-pid 2>/dev/null || echo -)" \
    "$(awk '{print $2}' .wm/reviewer-pid 2>/dev/null || echo -)"

  if [ "$LOOP_RC" -eq 0 ] && [ "$FAIL" -eq 0 ]; then
    walk_rc=0
  fi
  cd "$HERE"
fi

assert_no_home_skill_trees

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
[ "$walk_rc" -eq 0 ]
