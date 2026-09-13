#!/bin/sh
# 13b live arm: throwaway tarball adopt + real harness CLIs.
# Fail closed: missing grok/claude/codex → INDEPENDENCE_UNAVAILABLE exit 1.
# Fail closed: grok/claude/codex cannot auth under empty HOME →
# INDEPENDENCE_UNAVAILABLE: <cli> cannot auth (exit 1). Do not grok-only PASS.
# Host auth/config is copied (grok auth.json+config.toml, claude.json+settings.json,
# codex auth.json+config.toml). Skills/bundled/sessions are not copied.
# All three auth: four live CLI processes (not architecture-agent.sh /
# critique-agent.sh), distinct PIDs, LOW map, one wm loop.
# PATH-stripped arm still exit 1 INDEPENDENCE_UNAVAILABLE. Not a required CI gate.
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
command -v git >/dev/null 2>&1 && ok || bad 'git required'
command -v tar >/dev/null 2>&1 && ok || bad 'tar required'

# --- fail closed: all three harness CLIs on PATH, else stop (not a fixture PASS)
LIVE_GROK=0
LIVE_CLAUDE=0
LIVE_CODEX=0
command -v grok >/dev/null 2>&1 && LIVE_GROK=1
command -v claude >/dev/null 2>&1 && LIVE_CLAUDE=1
command -v codex >/dev/null 2>&1 && LIVE_CODEX=1
if [ "$LIVE_GROK" -ne 1 ] || [ "$LIVE_CLAUDE" -ne 1 ] || [ "$LIVE_CODEX" -ne 1 ]; then
  die_unavail "live grok/claude/codex CLI missing (grok=$LIVE_GROK claude=$LIVE_CLAUDE codex=$LIVE_CODEX)"
fi

GROK_BIN=$(command -v grok)
CLAUDE_BIN=$(command -v claude)
CODEX_BIN=$(command -v codex)
export GROK_BIN CLAUDE_BIN CODEX_BIN
GROK_AGENT_DASHBOARD=0
export GROK_AGENT_DASHBOARD

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
# dropping other keys. Copy claude.json + settings.json; codex auth + config.
# Never copy skills/, bundled/, or sessions/.
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
copy_if_file "$HOST_HOME/.claude.json" "$HOME/.claude.json"
copy_if_file "$HOST_HOME/.claude/settings.json" "$HOME/.claude/settings.json"
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
printf 'reply with pong only\n' > "$PROBE_DIR/ping.txt"
if ( CDPATH=; cd "$PROBE_DIR" && run_timeout 25 \
  "$GROK_BIN" --always-approve --no-subagents --disable-web-search \
  --output-format plain --max-turns 1 --prompt-file "$PROBE_DIR/ping.txt" ); then
  ok
else
  die_unavail "grok cannot auth"
fi
if ( CDPATH=; cd "$PROBE_DIR" && run_timeout 25 \
  "$CLAUDE_BIN" -p --output-format text --permission-mode dontAsk \
  --dangerously-skip-permissions pong ); then
  ok
else
  die_unavail "claude cannot auth"
fi
if ( CDPATH=; cd "$PROBE_DIR" && run_timeout 25 \
  "$CODEX_BIN" exec --skip-git-repo-check --dangerously-bypass-approvals-and-sandbox pong ); then
  ok
else
  die_unavail "codex cannot auth"
fi

assert_no_home_skill_trees() {
  hits=$(find "$EMPTY_HOME" \( \
    -path '*/.grok/skills' -o -path '*/.grok/skills/*' \
    -o -path '*/.claude/skills' -o -path '*/.claude/skills/*' \
    -o -path '*/.agents/skills' -o -path '*/.agents/skills/*' \
    \) -print 2>/dev/null || true)
  if [ -n "$hits" ]; then
    bad "harness skill trees under HOME: $hits"
    return
  fi
  hits=$(find "$EMPTY_HOME" -name SKILL.md ! -path '*/.grok/bundled/*' -print 2>/dev/null || true)
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

  mkdir -p product architecture tools reviews

  printf 'an idea: one-file hello product\n' > IDEA.md

  cat > architecture/modules.md <<'EOF'
module_id	root_path	public_contracts	test_entrypoint	pattern_instance	live_write
product	product	product/hello.txt	product/hello.txt	product/.keep	no
EOF
  printf 'keep\n' > product/.keep

  cat > SPEC.md <<'EOF'
## Goal
one file product/hello.txt contains exactly the word hello
## Non-goals
network, extra modules, live systems
## Owned files
- product/hello.txt
## Test files
- product/hello.txt
## Acceptance criteria
- product/hello.txt exists
- the file contains exactly one line: hello
## Focused falsifier
MAKER-WRITES
## Stop conditions
stop-ask on live write
## Risk
LOW
SPEC-AUTHOR: operator
EOF

  cat > tools/WORKER.md <<'EOF'
You are a working-mode brick worker in a throwaway git repo. Read the brief
file passed to you and this card. Do only the role named in the brief.
Do not push. Do not use the network. Do not write under $HOME. Do not spawn
subagents. Do not edit wm.sh. Do not write .wm/CLOSED or verdicts if you are
the maker.

Agent ids: mapper=alice critique=bob maker=carol reviewer=dave.

## mapper (agent alice)
Write MAP.md with exactly these bytes (tabs between columns):

MAPPER: alice

id	module	owned_paths	depends_on	risk
s1	product	product/hello.txt	-	LOW

Then run: .wm/bin/wm record-mapper --from MAP.md
Do not implement the product. Do not be the maker.

## critique (agent bob)
Write reviews/critique.md with Invert, Adversarial, and Simple sections.
Write .wm/return/bob.md with:
WORD: MAP-ACCEPT
AGENT: bob
MAP: MAP.md
Then run: .wm/bin/wm check-map-word .wm/return/bob.md
Do not be the mapper or maker.

## maker-falsify (agent carol)
Do not create or edit product/hello.txt.
Write exactly one line to .wm/FALSIFIER:
grep -qx hello product/hello.txt
Meta: compute sha256 of .wm/FALSIFIER (shasum -a 256 or sha256sum).
Write .wm/FALSIFIER.meta as three lines:
agent: carol
work-id: <git rev-parse --short=12 HEAD>
sha256: <hex digest of .wm/FALSIFIER>
Then: git add .wm/FALSIFIER .wm/FALSIFIER.meta && git commit -m 'maker-falsify s1'
Do not implement the product. Do not write .wm/return or reviews.

## maker-build (agent carol)
Write product/hello.txt containing exactly the word hello (printf 'hello\n').
git add product/hello.txt && git commit -m 'maker-build s1'
Do not write verdicts, CLOSED, or return files.

## reviewer (agent dave)
Re-run the named falsifier. Capture the evidence path:
cmd=$(sed -n '1p' .wm/FALSIFIER)
ev=$(.wm/bin/wm evidence dave -- sh -c "$cmd")
Write reviews/review.md with ## Code and ## Testing (short is fine).
Write .wm/return/dave.md with:
WORD: PASS
EVIDENCE: <exact path printed by wm evidence>
If the falsifier failed, use WORD: FAIL with the same EVIDENCE line.
Append one line to .wm/reviewer-reran: reran <cmd>
Do not edit product files. Do not be the maker.
EOF

  cat > tools/live-mapper.sh <<'EOF'
#!/bin/sh
set -eu
mkdir -p .wm
printf 'pid %s\n' "$$" > .wm/mapper-pid
printf 'ran\n' > .wm/mapper-ran
[ -n "${GROK_BIN:-}" ] || GROK_BIN=$(command -v grok)
prompt=.wm/live-mapper.prompt
{
  printf 'role: mapper\nagent: alice\n'
  cat tools/WORKER.md
} > "$prompt"
exec "$GROK_BIN" --always-approve --no-subagents --disable-web-search \
  --output-format plain --max-turns 30 --prompt-file "$prompt"
EOF

  cat > tools/live-critique.sh <<'EOF'
#!/bin/sh
set -eu
mkdir -p .wm
printf 'pid %s\n' "$$" > .wm/critique-pid
printf 'ran\n' > .wm/critique-ran
[ -n "${CLAUDE_BIN:-}" ] || CLAUDE_BIN=$(command -v claude)
prompt=.wm/live-critique.prompt
{
  printf 'role: critique\nagent: bob\n'
  cat tools/WORKER.md
} > "$prompt"
exec "$CLAUDE_BIN" -p --output-format text --permission-mode dontAsk \
  --dangerously-skip-permissions "$(cat "$prompt")"
EOF

  cat > tools/live-maker.sh <<'EOF'
#!/bin/sh
set -eu
printf 'pid %s\n' "$$" > .wm/maker-pid
printf 'ran\n' >> .wm/maker-ran
brief=${1:-${BRIEF:-}}
[ -n "$brief" ] && [ -f "$brief" ] || { printf 'live-maker: brief missing\n' >&2; exit 1; }
[ -n "${GROK_BIN:-}" ] || GROK_BIN=$(command -v grok)
prompt=.wm/live-maker.prompt
{
  cat "$brief"
  printf '\n'
  cat tools/WORKER.md
} > "$prompt"
exec "$GROK_BIN" --always-approve --no-subagents --disable-web-search \
  --output-format plain --max-turns 30 --prompt-file "$prompt"
EOF

  cat > tools/live-reviewer.sh <<'EOF'
#!/bin/sh
set -eu
printf 'pid %s\n' "$$" > .wm/reviewer-pid
printf 'ran\n' > .wm/reviewer-ran
brief=${1:-${BRIEF:-}}
[ -n "$brief" ] && [ -f "$brief" ] || { printf 'live-reviewer: brief missing\n' >&2; exit 1; }
[ -n "${CODEX_BIN:-}" ] || CODEX_BIN=$(command -v codex)
prompt=.wm/live-reviewer.prompt
{
  cat "$brief"
  printf '\n'
  cat tools/WORKER.md
} > "$prompt"
exec "$CODEX_BIN" exec --dangerously-bypass-approvals-and-sandbox - < "$prompt"
EOF

  chmod +x tools/live-mapper.sh tools/live-critique.sh \
    tools/live-maker.sh tools/live-reviewer.sh

  if ! "$WM" init >"$OUT" 2>"$ERR"; then
    bad "wm init refused: $(cat "$OUT") $(cat "$ERR")"
  else
    ok
  fi
  "$WM" cast coordinator parent grok - >/dev/null 2>"$ERR" || true

  if ./tools/live-mapper.sh >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "live mapper refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  [ -f MAP.md ] && grep -q '^s1	product	product/hello.txt	-	LOW$' MAP.md \
    && ok || bad 'live mapper did not write LOW one-slice MAP.md'
  [ -f .wm/mapper-ran ] && ok || bad 'mapper process did not run'

  if "$WM" map-ready >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "map-ready refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  [ -f slices.tsv ] && ok || bad 'map-ready did not write slices.tsv'

  if ./tools/live-critique.sh >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "live critique refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  [ -f .wm/critique-ran ] && ok || bad 'critique process did not run'
  if "$WM" map-verdict .wm/return/bob.md >"$OUT" 2>"$ERR"; then
    grep -q 'MAP-ACCEPT' "$OUT" && ok || bad "map-verdict wanted MAP-ACCEPT, got $(cat "$OUT")"
  else
    bad "map-verdict refused: $(cat "$OUT") $(cat "$ERR")"
  fi

  if "$WM" cast maker carol grok './tools/live-maker.sh {BRIEF}' >"$OUT" 2>"$ERR"; then
    ok
  else
    bad "cast maker refused: $(cat "$OUT") $(cat "$ERR")"
  fi
  if "$WM" cast reviewer dave codex './tools/live-reviewer.sh {BRIEF}' >"$OUT" 2>"$ERR"; then
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
  if git -c user.email=wm@local -c user.name=working-mode commit -qm 'live LOW map+spec+workers'; then
    ok
  else
    bad 'fixture commit of map/spec/workers refused'
  fi

  printf 'NONE\n' > .wm/lesson

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

  closed_word=
  if [ -f .wm/CLOSED ]; then
    closed_word=$(awk 'NF { print; exit }' .wm/CLOSED)
  fi
  if [ "$LOOP_RC" -eq 0 ] && [ -f .wm/CLOSED ] \
    && grep -Eq 'CLOSED (PASS|NO-BUILD)' .wm/CLOSED; then
    ok
  else
    bad "one wm loop wanted CLOSED PASS or CLOSED NO-BUILD rc=0, got rc=$LOOP_RC closed=$closed_word out=$(cat "$OUT") err=$(cat "$ERR") worker.err=$(cat .wm/worker.err 2>/dev/null || true)"
  fi

  [ -f .wm/reviewer-ran ] && ok || bad "reviewer CLI not exec'd"
  [ -f .wm/maker-ran ] && ok || bad "maker process did not run"
  [ -f .wm/maker-pid ] && ok || bad "maker pid missing"
  [ -f .wm/reviewer-pid ] && ok || bad "reviewer pid missing"

  if [ -f .wm/mapper-pid ] && [ -f .wm/critique-pid ] \
    && [ -f .wm/maker-pid ] && [ -f .wm/reviewer-pid ]; then
    mp=$(awk '{print $2}' .wm/mapper-pid)
    cp=$(awk '{print $2}' .wm/critique-pid)
    mkp=$(awk '{print $2}' .wm/maker-pid)
    rp=$(awk '{print $2}' .wm/reviewer-pid)
    uniq=$(printf '%s\n' "$mp" "$cp" "$mkp" "$rp" | awk 'NF' | sort -u | wc -l | tr -d ' ')
    if [ "$uniq" -eq 4 ]; then
      ok
    else
      bad "mapper/critique/maker/reviewer PIDs not 4 distinct (mapper=$mp critique=$cp maker=$mkp reviewer=$rp uniq=$uniq)"
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

  PRODUCT=product/hello.txt
  if grep -q 'CLOSED PASS' .wm/CLOSED 2>/dev/null; then
    if [ -f "$PRODUCT" ] && grep -qx hello "$PRODUCT"; then
      ok
    else
      bad "CLOSED PASS but product path $PRODUCT is not hello"
    fi
  else
    ok
  fi

  closed_rows=$(awk -F '\t' 'NR>1 && $6=="CLOSED" {c++} END{print c+0}' slices.tsv)
  [ "$closed_rows" -eq 1 ] && ok || bad "slices.tsv CLOSED rows wanted 1, got $closed_rows"

  printf 'LIVE_GATE %s product=%s invoke=.wm/invoke/reviewer.log writer: wm-run\n' \
    "${closed_word:-FAIL}" "$PRODUCT"
  printf 'LIVE_PIDS mapper=%s critique=%s maker=%s reviewer=%s\n' \
    "$(awk '{print $2}' .wm/mapper-pid 2>/dev/null || echo -)" \
    "$(awk '{print $2}' .wm/critique-pid 2>/dev/null || echo -)" \
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
