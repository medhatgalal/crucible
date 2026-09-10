#!/bin/sh
# Working-mode kernel CHECKs. Fixture git repos only. No harness CLIs.
# Falsifier-first: RED when wm.sh is missing or a CHECK is absent.
set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
WM="$HERE/wm.sh"
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }

if [ ! -f "$WM" ]; then
  printf 'RED wm.sh missing (working-mode CHECKs not present)\n' >&2
  exit 1
fi
chmod +x "$WM" 2>/dev/null || true
export WM_ENGINE="$WM"

BASE=$(mktemp -d "${TMPDIR:-/tmp}/wm-verify.XXXXXX")
EMPTY_HOME=$(mktemp -d "${TMPDIR:-/tmp}/wm-empty-home.XXXXXX")
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

write_low_spec() {
  cat > SPEC.md <<'EOF'
## Goal
tiny throwaway product file
## Non-goals
live systems
## Owned files
- product.txt
## Test files
- (none)
## Acceptance criteria
- product.txt exists
## Focused falsifier
MAKER-WRITES
## Stop conditions
stop-ask on live write
## Risk
LOW
SPEC-AUTHOR: operator
EOF
}

write_falsifier() {
  cmd=$1
  agent=${2:-alice}
  mkdir -p .wm
  printf '%s\n' "$cmd" > .wm/FALSIFIER
  h=$(sha256_file .wm/FALSIFIER)
  wid=NOCOMMIT
  if git rev-parse --verify HEAD >/dev/null 2>&1; then
    wid=$(git rev-parse --short=12 HEAD)
  fi
  printf 'agent: %s\nwork-id: %s\nsha256: %s\n' "$agent" "$wid" "$h" > .wm/FALSIFIER.meta
}

commit_msg() {
  git add -A
  if git diff --cached --quiet; then
    return 0
  fi
  git commit -qm "$1"
}

setup_repo() {
  name=$1
  mkdir -p "$BASE/$name"
  CDPATH= cd "$BASE/$name"
  git init -q
  git config user.email 'wm@local'
  git config user.name 'working-mode'
  printf 'an idea\n' > IDEA.md
  write_low_spec
  if ! "$WM" init >"$OUT" 2>"$ERR"; then
    printf 'FIXTURE BROKEN: %s init failed\n%s\n%s\n' "$name" "$(cat "$OUT")" "$(cat "$ERR")" >&2
    exit 1
  fi
  "$WM" cast coordinator parent grok - >"$OUT" 2>"$ERR" || true
  if ! "$WM" cast maker alice grok 'sh -c "echo maker {BRIEF}"' >"$OUT" 2>"$ERR"; then
    printf 'FIXTURE BROKEN: %s cast maker failed\n%s\n' "$name" "$(cat "$ERR")" >&2
    exit 1
  fi
  if ! "$WM" cast reviewer bob grok 'sh -c "echo reviewer {BRIEF}"' >"$OUT" 2>"$ERR"; then
    printf 'FIXTURE BROKEN: %s cast reviewer failed\n%s\n' "$name" "$(cat "$ERR")" >&2
    exit 1
  fi
  commit_msg 'setup spec idea panel'
}

reach_green() {
  "$WM" record-pre-falsify >"$OUT" 2>"$ERR" || {
    printf 'FIXTURE BROKEN: record-pre-falsify\n%s\n' "$(cat "$ERR")" >&2
    exit 1
  }
  write_falsifier 'test -f product.txt' alice
  commit_msg 'maker-falsify'
  if ! "$WM" red >"$OUT" 2>"$ERR"; then
    printf 'FIXTURE BROKEN: red (expected RED)\n%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" >&2
    exit 1
  fi
  grep -q '^RED$' "$OUT" || grep -q '^red$' .wm/red.status || {
    printf 'FIXTURE BROKEN: expected RED, got %s status=%s\n' "$(cat "$OUT")" "$(cat .wm/red.status 2>/dev/null || true)" >&2
    exit 1
  }
  printf 'built\n' > product.txt
  commit_msg 'maker-build'
  if ! "$WM" built >"$OUT" 2>"$ERR"; then
    printf 'FIXTURE BROKEN: built\n%s\n' "$(cat "$ERR")" >&2
    exit 1
  fi
  if ! "$WM" green >"$OUT" 2>"$ERR"; then
    printf 'FIXTURE BROKEN: green\n%s\n' "$(cat "$ERR")" >&2
    exit 1
  fi
}

# Fixture maker: falsify then implement. Dispatch role chooses the step.
write_loop_maker_pass() {
  mkdir -p tools
  cat > tools/loop-maker.sh <<'EOF'
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
if [ "$role" = maker-build ]; then
  printf 'built\n' > product.txt
  git add product.txt
  git commit -qm maker-build
  exit 0
fi
mkdir -p .wm
printf 'test -f product.txt\n' > .wm/FALSIFIER
h=$(sha_of .wm/FALSIFIER)
wid=NOCOMMIT
if git rev-parse --verify HEAD >/dev/null 2>&1; then
  wid=$(git rev-parse --short=12 HEAD)
fi
printf 'agent: alice\nwork-id: %s\nsha256: %s\n' "$wid" "$h" > .wm/FALSIFIER.meta
EOF
  chmod +x tools/loop-maker.sh
}

# Fixture reviewer: return WORD only after this CLI is exec'd.
write_loop_reviewer_pass() {
  mkdir -p tools
  cat > tools/loop-reviewer.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return
ev=$(.wm/bin/wm evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
EOF
  chmod +x tools/loop-reviewer.sh
}

write_loop_maker_nobuild() {
  mkdir -p tools
  cat > tools/loop-maker-nobuild.sh <<'EOF'
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
wid=NOCOMMIT
if git rev-parse --verify HEAD >/dev/null 2>&1; then
  wid=$(git rev-parse --short=12 HEAD)
fi
printf 'agent: alice\nwork-id: %s\nsha256: %s\n' "$wid" "$h" > .wm/FALSIFIER.meta
EOF
  chmod +x tools/loop-maker-nobuild.sh
}

write_loop_reviewer_nobuild() {
  mkdir -p tools
  cat > tools/loop-reviewer-nobuild.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return
ev=$(.wm/bin/wm evidence bob -- true)
printf 'WORD: NO-BUILD\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
EOF
  chmod +x tools/loop-reviewer-nobuild.sh
}

# Task 7: honest reviewer exec so close/LESSONS CHECKs can run.
install_rev_pass() {
  mkdir -p tools
  cat > tools/rev-pass.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return
ev=$(.wm/bin/wm evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
EOF
  chmod +x tools/rev-pass.sh
  "$WM" cast reviewer bob grok './tools/rev-pass.sh {BRIEF}' >/dev/null
}

reach_reviewer_pass() {
  reach_green
  install_rev_pass
  if ! "$WM" run reviewer >"$OUT" 2>"$ERR"; then
    printf 'FIXTURE BROKEN: run reviewer\n%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" >&2
    exit 1
  fi
}

install_maker_dump() {
  mkdir -p tools .wm
  cat > tools/maker-dump.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/maker-ran
if [ -n "${BRIEF:-}" ] && [ -f "$BRIEF" ]; then
  cp "$BRIEF" .wm/dumped-brief.md
fi
exit 0
EOF
  chmod +x tools/maker-dump.sh
}

run_wm_loop() {
  set +e
  "$WM" loop >"$OUT" 2>"$ERR"
  LOOP_RC=$?
  set -e
}

assert_loop_foreground() {
  _alf_label=$1
  if pgrep -f 'wm.sh loop' >/dev/null 2>&1; then
    bad "$_alf_label: leftover wm.sh loop process"
  else
    ok
  fi
  if [ -f .wm/pid ]; then
    _alf_pid=$(cat .wm/pid)
    if [ -n "$_alf_pid" ] && kill -0 "$_alf_pid" 2>/dev/null; then
      bad "$_alf_label: leftover pid $_alf_pid still live"
    else
      ok
    fi
  else
    ok
  fi
}

closed_pass_present() {
  if grep -q 'CLOSED PASS' "$OUT" 2>/dev/null; then
    return 0
  fi
  if [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
    return 0
  fi
  return 1
}

# Task 1 (A1–A4): multi-slice walker fixtures. Script must not rewrite slices.tsv
# or rm .wm/CLOSED around wm loop.
write_multi_spec() {
  cat > SPEC.md <<'EOF'
## Goal
tiny throwaway product files a.txt b.txt c.txt
## Non-goals
live systems
## Owned files
- a.txt
- b.txt
- c.txt
## Test files
- (none)
## Acceptance criteria
- a.txt b.txt c.txt exist
## Focused falsifier
MAKER-WRITES
## Stop conditions
stop-ask on live write
## Risk
LOW
SPEC-AUTHOR: operator
EOF
}

plant_map_accept_slices() {
  mkdir -p .wm
  printf 'WORD: MAP-ACCEPT\nAGENT: bob\nMAP: MAP.md\nwhen: 0\n' > .wm/map-verdict
  printf 'id\tmodule\towned_paths\tdepends_on\trisk\tstatus\n' > slices.tsv
  cat >> slices.tsv
}

write_multi_maker() {
  mkdir -p tools
  cat > tools/multi-maker.sh <<'EOF'
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
printf 'ran %s %s\n' "$role" "$sid" >> .wm/maker-ran
sha_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  fi
}
if [ "$role" = maker-build ]; then
  case $sid in
    s1)
      printf 'alpha\n' > a.txt
      git add a.txt
      git commit -qm maker-build-s1
      ;;
    s2)
      printf 'beta\n' > b.txt
      git add b.txt
      git commit -qm maker-build-s2
      ;;
    s3)
      printf 'seam\n' > c.txt
      git add c.txt
      git commit -qm maker-build-s3
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
  s1) printf 'test -f a.txt\n' > .wm/FALSIFIER ;;
  s2) printf 'test -f b.txt\n' > .wm/FALSIFIER ;;
  s3) printf 'test -f c.txt\n' > .wm/FALSIFIER ;;
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
printf 'agent: alice\nwork-id: %s\nsha256: %s\n' "$wid" "$h" > .wm/FALSIFIER.meta
EOF
  chmod +x tools/multi-maker.sh
}

write_multi_reviewer() {
  mkdir -p tools
  cat > tools/multi-reviewer.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return
cmd=$(sed -n '1p' .wm/FALSIFIER)
ev=$(.wm/bin/wm evidence bob -- sh -c "$cmd")
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
EOF
  chmod +x tools/multi-reviewer.sh
}

cast_multi_panel() {
  write_multi_maker
  write_multi_reviewer
  "$WM" cast maker alice grok './tools/multi-maker.sh {BRIEF}' >/dev/null
  "$WM" cast reviewer bob grok './tools/multi-reviewer.sh {BRIEF}' >/dev/null
}

# ---------------------------------------------------------------------------
# (1) SPEC ## Focused falsifier must be exactly MAKER-WRITES
# ---------------------------------------------------------------------------
setup_repo t01-spec
expect 'ready with MAKER-WRITES' '^READY$' "$WM" ready

cat > SPEC.md <<'EOF'
## Goal
x
## Non-goals
y
## Owned files
- product.txt
## Test files
- (none)
## Acceptance criteria
- a
## Focused falsifier
false
## Stop conditions
stop
## Risk
LOW
SPEC-AUTHOR: operator
EOF
refuses 'ready refuses concrete falsifier command in SPEC' 'refused:' "$WM" ready

cat > SPEC.md <<'EOF'
## Goal
x
## Non-goals
y
## Owned files
- product.txt
## Test files
- (none)
## Acceptance criteria
- a
## Focused falsifier
TEMPLATE-FALSIFIER-UNWRITTEN
## Stop conditions
stop
## Risk
LOW
SPEC-AUTHOR: operator
EOF
refuses 'ready refuses template falsifier' 'refused:' "$WM" ready

write_low_spec
expect 'ready restored MAKER-WRITES' '^READY$' "$WM" ready

# ---------------------------------------------------------------------------
# (2) red: missing product → RED; falsifier already green on clean tree → NO-BUILD
# ---------------------------------------------------------------------------
setup_repo t02-red-missing
"$WM" record-pre-falsify >/dev/null
write_falsifier 'test -f product.txt' alice
commit_msg 'falsify missing product'
if ! "$WM" red >"$OUT" 2>"$ERR"; then
  bad "red missing product refused: $(cat "$OUT") $(cat "$ERR")"
else
  if grep -q '^RED$' "$OUT" && grep -q '^red$' .wm/red.status; then
    ok
  else
    bad "red missing product: wanted RED, got $(cat "$OUT") status=$(cat .wm/red.status 2>/dev/null || true)"
  fi
fi
grep -q '^no-build$' .wm/red.status 2>/dev/null && bad 'missing product must not be no-build' || ok

setup_repo t02-red-nobuild
"$WM" record-pre-falsify >/dev/null
write_falsifier 'true' alice
commit_msg 'falsify already-green no product change'
set +e
"$WM" red >"$OUT" 2>"$ERR"
set -e
if grep -q '^NO-BUILD$' "$OUT" && grep -q '^no-build$' .wm/red.status; then
  ok
else
  bad "already-green clean: wanted NO-BUILD, got $(cat "$OUT") status=$(cat .wm/red.status 2>/dev/null || true) err=$(cat "$ERR")"
fi
grep -q 'early-implement' .wm/red.status 2>/dev/null && bad 'already-green clean must not be early-implement' || ok

# ---------------------------------------------------------------------------
# (3) product dirty vs pre-falsify on red → early-implement, not NO-BUILD
# ---------------------------------------------------------------------------
setup_repo t03-early
"$WM" record-pre-falsify >/dev/null
write_falsifier 'true' alice
printf 'early product\n' > product.txt
commit_msg 'falsify with product-path change'
refuses 'red product-path change is early-implement' 'refused:|early implement' "$WM" red
if [ -f .wm/red.status ] && grep -q '^early-implement$' .wm/red.status; then
  ok
else
  bad "product-path change must set red.status=early-implement, got $(cat .wm/red.status 2>/dev/null || echo ABSENT)"
fi
if [ -f .wm/red.status ] && grep -q '^no-build$' .wm/red.status; then
  bad 'product-path change must not be no-build'
else
  ok
fi

# ---------------------------------------------------------------------------
# (4) FALSIFIER containing `&` or nohup → refuse before exec
# ---------------------------------------------------------------------------
setup_repo t04-amp
"$WM" record-pre-falsify >/dev/null
write_falsifier 'false &' alice
commit_msg 'falsify amp'
refuses 'red FALSIFIER false & refused before exec' 'refused:' "$WM" red
if [ -f .wm/red.status ] && grep -Eq '^(no-build|red)$' .wm/red.status; then
  bad "false & must not write red.status=$(cat .wm/red.status)"
else
  ok
fi

write_falsifier 'nohup true' alice
commit_msg 'falsify nohup'
refuses 'red FALSIFIER nohup refused before exec' 'refused:' "$WM" red
if [ -f .wm/red.status ] && grep -q '^no-build$' .wm/red.status; then
  bad 'nohup true must not take NO-BUILD'
else
  ok
fi

write_falsifier 'sleep 98765 &' alice
commit_msg 'falsify leftover sleep'
refuses 'red FALSIFIER sleep & refused before exec' 'refused:' "$WM" red
if pgrep -f 'sleep 98765' >/dev/null 2>&1; then
  pkill -f 'sleep 98765' >/dev/null 2>&1 || true
  bad 'sleep 98765 & left a leftover pid'
else
  ok
fi

# redirect 2>&1 is not job-control background
write_falsifier 'false 2>&1' alice
commit_msg 'falsify redirect'
if ! "$WM" red >"$OUT" 2>"$ERR"; then
  bad "false 2>&1 should be observed-red, not background refuse: $(cat "$ERR")"
elif grep -q '^red$' .wm/red.status; then
  ok
else
  bad "false 2>&1 expected red.status=red, got $(cat .wm/red.status 2>/dev/null || echo ABSENT)"
fi

# ---------------------------------------------------------------------------
# (5) maker cannot verdict PASS
# ---------------------------------------------------------------------------
setup_repo t05-maker-verdict
reach_green
ev=$("$WM" evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
refuses 'controller argv stamps verdict WORD' 'refused:' "$WM" verdict bob PASS "$ev"
refuses 'maker cannot verdict PASS (argv)' 'refused:' "$WM" verdict alice PASS "$ev"
mkdir -p .wm/return
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/alice.md
refuses 'maker cannot verdict PASS (return file)' 'refused:' "$WM" verdict .wm/return/alice.md

# ---------------------------------------------------------------------------
# (6) planted .wm/verdicts without reviewer run → close refuses
# ---------------------------------------------------------------------------
setup_repo t06-planted
reach_green
mkdir -p .wm/verdicts
printf 'VERDICT: PASS\nAGENT: bob\nWORK-ID: %s\nISOLATION: SUBAGENT-ISOLATED\nMODEL-SWITCH: UNVERIFIED\n' "$("$WM" workid)" > .wm/verdicts/bob.md
refuses 'close planted PASS without reviewer run' 'refused:' "$WM" close NONE

# ---------------------------------------------------------------------------
# (7) after maker-build, next is NEXT RUN reviewer (ignore leftover reviewer receipts)
# ---------------------------------------------------------------------------
setup_repo t07-leftover
reach_green
mkdir -p .wm/verdicts .wm/return .wm/invoke .wm/spawn .wm/briefs
printf 'WORD: PASS\nEVIDENCE: planted\n' > .wm/return/bob.md
printf 'VERDICT: PASS\nAGENT: bob\nWORK-ID: %s\nEVIDENCE: planted\nINGEST: return\nISOLATION: SUBAGENT-ISOLATED\nMODEL-SWITCH: UNVERIFIED\n' "$("$WM" workid)" > .wm/verdicts/bob.md
printf 't: 1\nabsent_return: yes\npath: .wm/return/bob.md\nrole: reviewer\nrun-id: leftover.old\n' > .wm/spawn/bob.stamp
printf 'Read this file and follow it exactly.\nrole: reviewer\nagent: bob\n' > .wm/briefs/reviewer.leftover.md
printf 'role: reviewer\nagent: bob\nwriter: wm-run\nafter-maker: leftover-old\nISOLATION: SUBAGENT-ISOLATED\n' > .wm/invoke/reviewer.log
card=$("$WM" next)
printf '%s\n' "$card" | grep -q 'NEXT CLOSE' && bad "leftover reviewer receipts must not NEXT CLOSE (got $card)" || ok
printf '%s\n' "$card" | grep -q 'NEXT RUN reviewer' && ok || bad "after maker-build expected NEXT RUN reviewer, got $card"

# ---------------------------------------------------------------------------
# (8) CLOSED PASS requires wm run reviewer actually exec'd (invoke.log after last maker-*)
# ---------------------------------------------------------------------------
setup_repo t08-forge-invoke
reach_green
mkdir -p .wm/verdicts .wm/return .wm/invoke .wm/spawn
ev=$("$WM" evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
{
  printf 'VERDICT: PASS\n'
  printf 'AGENT: bob\n'
  printf 'WORK-ID: %s\n' "$("$WM" workid)"
  printf 'EVIDENCE: %s\n' "$ev"
  printf 'INGEST: return\n'
  printf 'RUN-ID: forged.1\n'
  printf 'ISOLATION: SUBAGENT-ISOLATED\n'
  printf 'MODEL-SWITCH: UNVERIFIED\n'
} > .wm/verdicts/bob.md
printf 't: 1\nabsent_return: yes\npath: .wm/return/bob.md\nrole: reviewer\nrun-id: forged.1\n' > .wm/spawn/bob.stamp
printf 'role: reviewer\nagent: bob\nwriter: wm-run\nafter-maker: forged\nISOLATION: SUBAGENT-ISOLATED\n' > .wm/invoke/reviewer.log
refuses 'close forged invoke.log not after last maker-*' 'refused:' "$WM" close NONE
card=$("$WM" next)
printf '%s\n' "$card" | grep -q 'NEXT CLOSE' && bad "forged invoke.log must not NEXT CLOSE (got $card)" || ok
printf '%s\n' "$card" | grep -q 'NEXT RUN reviewer' && ok || bad "forged receipts should still need reviewer, got $card"

setup_repo t08-run-reviewer
reach_green
mkdir -p tools
cat > tools/rev-pass.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return
ev=$(.wm/bin/wm evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
EOF
chmod +x tools/rev-pass.sh
"$WM" cast reviewer bob grok './tools/rev-pass.sh {BRIEF}' >/dev/null
if ! "$WM" run reviewer >"$OUT" 2>"$ERR"; then
  bad "run reviewer failed: $(cat "$OUT") $(cat "$ERR")"
else
  ok
fi
[ -f .wm/reviewer-ran ] && ok || bad 'wm run reviewer did not exec the reviewer command'
[ -f .wm/invoke/reviewer.log ] && grep -q 'writer: wm-run' .wm/invoke/reviewer.log && ok || bad 'invoke.log missing writer: wm-run'
if [ -f .wm/last-maker-run ] && [ -f .wm/invoke/reviewer.log ]; then
  need=$(awk -F ': ' '$1=="id"{print $2; exit}' .wm/last-maker-run)
  got=$(awk -F ': ' '$1=="after-maker"{print $2; exit}' .wm/invoke/reviewer.log)
  [ -n "$need" ] && [ "$got" = "$need" ] && ok || bad "invoke.log after-maker='$got' != last-maker-run id='$need'"
else
  bad 'missing last-maker-run or invoke/reviewer.log'
fi
expect 'close CLOSED PASS after reviewer exec' 'CLOSED PASS' "$WM" close NONE

# loopfull: maker-build writes judge artifacts without exec'ing reviewer
setup_repo t08-loopfull
"$WM" record-pre-falsify >/dev/null
write_falsifier 'test -f product.txt' alice
commit_msg 'falsify'
"$WM" red >/dev/null || {
  printf 'FIXTURE BROKEN: red loopfull\n' >&2
  exit 1
}
mkdir -p tools
cat > tools/forge-build.sh <<'EOF'
#!/bin/sh
set -eu
printf 'built\n' > product.txt
git add product.txt
git commit -qm maker-build
mkdir -p .wm/spawn .wm/return .wm/briefs .wm/verdicts .wm/invoke
printf 'role: reviewer\nagent: bob\n' > .wm/dispatch
ev=$(.wm/bin/wm evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
t=$(date +%s)
runid="${t}.forged"
printf 't: %s\nabsent_return: yes\npath: .wm/return/bob.md\nrole: reviewer\nrun-id: %s\n' "$t" "$runid" > .wm/spawn/bob.stamp
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
wid=$(git rev-parse --short=12 HEAD)
printf 'Read this file and follow it exactly.\nrole: reviewer\nagent: bob\n' > ".wm/briefs/reviewer.${wid}.md"
printf 'VERDICT: PASS\nAGENT: bob\nWORK-ID: %s\nEVIDENCE: %s\nINGEST: return\nRUN-ID: %s\nISOLATION: SUBAGENT-ISOLATED\nMODEL-SWITCH: UNVERIFIED\n' "$wid" "$ev" "$runid" > .wm/verdicts/bob.md
printf 'role: reviewer\nagent: bob\nwriter: wm-run\nafter-maker: forged\n' > .wm/invoke/reviewer.log
EOF
chmod +x tools/forge-build.sh
"$WM" cast maker alice grok './tools/forge-build.sh {BRIEF}' >/dev/null
refuses 'maker-build that forges reviewer receipts' 'refused:' "$WM" run maker-build
card=$("$WM" next)
printf '%s\n' "$card" | grep -q 'NEXT CLOSE' && bad "loopfull next must not be NEXT CLOSE (got $card)" || ok
refuses 'close after loopfull forge without reviewer exec' 'refused:' "$WM" close NONE

# ---------------------------------------------------------------------------
# (9) live/push-main/rm -rf in falsifier or evidence argv → refuse (2c)
# ---------------------------------------------------------------------------
setup_repo t09-fence
refuses 'evidence git push main refused before exec' 'refused:' "$WM" evidence bob -- git push origin main
refuses 'evidence nohup refused' 'refused:' "$WM" evidence bob -- nohup true
sentinel="$BASE/t09-sentinel"
mkdir -p "$sentinel/keep"
printf 'keep\n' > "$sentinel/keep/file"
refuses 'evidence rm -rf refused before exec' 'refused:' "$WM" evidence bob -- rm -rf "$sentinel"
if [ -f "$sentinel/keep/file" ]; then
  ok
else
  bad 'rm -rf in evidence argv was execd (sentinel gone)'
fi
refuses 'evidence curl live write refused' 'refused:' "$WM" evidence bob -- curl http://example.invalid
"$WM" record-pre-falsify >/dev/null
write_falsifier 'git push origin main' alice
commit_msg 'bad falsifier push'
refuses 'red FALSIFIER push-main refused before exec' 'refused:' "$WM" red
write_falsifier "rm -rf $sentinel" alice
commit_msg 'bad falsifier rm'
refuses 'red FALSIFIER rm -rf refused before exec' 'refused:' "$WM" red
[ -f "$sentinel/keep/file" ] && ok || bad 'rm -rf in FALSIFIER was execd (sentinel gone)'
write_falsifier 'curl http://example.invalid' alice
commit_msg 'bad falsifier curl'
refuses 'red FALSIFIER live curl refused before exec' 'refused:' "$WM" red

# ---------------------------------------------------------------------------
# (10) no background: loop does not daemonize
# ---------------------------------------------------------------------------
setup_repo t10-loop
set +e
"$WM" loop >"$OUT" 2>"$ERR"
loop_rc=$?
set -e
if pgrep -f 'wm.sh loop' >/dev/null 2>&1; then
  bad 'wm loop left a leftover wm.sh loop process'
else
  ok
fi
if [ -f .wm/pid ]; then
  lpid=$(cat .wm/pid)
  if [ -n "$lpid" ] && kill -0 "$lpid" 2>/dev/null; then
    bad "wm loop leftover pid $lpid still live"
  else
    ok
  fi
else
  ok
fi
if pgrep -f 'sleep 424242' >/dev/null 2>&1; then
  pkill -f 'sleep 424242' >/dev/null 2>&1 || true
  bad 'wm loop left a leftover sleep process'
else
  ok
fi
# cmd_loop body must not nohup/setsid or job-control-background
loop_body=$(awk '
  /^cmd_loop\(/ { p=1 }
  p { print }
  p && /^}$/ { exit }
' "$WM")
if printf '%s\n' "$loop_body" | grep -E -q 'nohup|setsid'; then
  bad 'cmd_loop uses nohup/setsid'
else
  ok
fi
if printf '%s\n' "$loop_body" | grep -E -q '[^>&][[:space:]]*&[[:space:]]*$'; then
  bad 'cmd_loop job-control-backgrounds with &'
else
  ok
fi
[ "$loop_rc" -eq 0 ] || [ "$loop_rc" -eq 1 ] && ok || bad "wm loop unexpected exit $loop_rc"

# ---------------------------------------------------------------------------
# Task 6: terminal walker. Fixture reviewer writes WORD after exec (5c).
# Stub (LOOP STUB) must fail the honest walk — RED before cmd_loop exists.
# ---------------------------------------------------------------------------

sh -n "$WM" && ok || bad 'wm.sh is not valid POSIX sh'

# (11) Honest walk: fixture maker + reviewer → CLOSED PASS and reviewer exec
setup_repo t11-loop-pass
write_loop_maker_pass
write_loop_reviewer_pass
"$WM" cast maker alice grok './tools/loop-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer bob grok './tools/loop-reviewer.sh {BRIEF}' >/dev/null
commit_msg 'loop pass workers'
run_wm_loop
assert_loop_foreground 't11-loop-pass'
if [ "$LOOP_RC" -eq 0 ]; then
  ok
else
  bad "honest wm loop exit $LOOP_RC out=$(cat "$OUT") err=$(cat "$ERR")"
fi
if grep -q 'CLOSED PASS' "$OUT" && [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  ok
else
  bad "honest wm loop wanted CLOSED PASS, got out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT)"
fi
[ -f .wm/reviewer-ran ] && ok || bad 'honest wm loop did not exec the reviewer CLI'
[ -f .wm/invoke/reviewer.log ] && grep -q 'writer: wm-run' .wm/invoke/reviewer.log \
  && ok || bad 'honest wm loop missing invoke.log writer: wm-run'
if [ -f .wm/last-maker-run ] && [ -f .wm/invoke/reviewer.log ]; then
  need=$(awk -F ': ' '$1=="id"{print $2; exit}' .wm/last-maker-run)
  got=$(awk -F ': ' '$1=="after-maker"{print $2; exit}' .wm/invoke/reviewer.log)
  [ -n "$need" ] && [ "$got" = "$need" ] \
    && ok || bad "honest loop after-maker='$got' != last-maker-run id='$need'"
else
  bad 'honest loop missing last-maker-run or invoke/reviewer.log'
fi
[ -f .wm/return/bob.md ] && grep -q '^WORD: PASS$' .wm/return/bob.md \
  && ok || bad 'fixture reviewer did not write WORD: PASS after exec'
# Honest re-entry: close() already enforced 5c; a second loop may reprint CLOSED PASS.
run_wm_loop
assert_loop_foreground 't11-loop-pass-reentry'
[ "$LOOP_RC" -eq 0 ] && ok || bad "honest reentry loop exit $LOOP_RC out=$(cat "$OUT") err=$(cat "$ERR")"
grep -q 'CLOSED PASS' "$OUT" && ok || bad "honest reentry wanted CLOSED PASS, got $(cat "$OUT")"

# (12) Leftover reviewer receipts after maker-build: still exec reviewer (CHECK 7 via loop)
setup_repo t12-loop-leftover
reach_green
write_loop_reviewer_pass
"$WM" cast reviewer bob grok './tools/loop-reviewer.sh {BRIEF}' >/dev/null
mkdir -p .wm/verdicts .wm/return .wm/invoke .wm/spawn .wm/briefs
printf 'WORD: PASS\nEVIDENCE: planted\n' > .wm/return/bob.md
printf 'VERDICT: PASS\nAGENT: bob\nWORK-ID: %s\nEVIDENCE: planted\nINGEST: return\nISOLATION: SUBAGENT-ISOLATED\nMODEL-SWITCH: UNVERIFIED\n' "$("$WM" workid)" > .wm/verdicts/bob.md
printf 't: 1\nabsent_return: yes\npath: .wm/return/bob.md\nrole: reviewer\nrun-id: leftover.old\n' > .wm/spawn/bob.stamp
printf 'Read this file and follow it exactly.\nrole: reviewer\nagent: bob\n' > .wm/briefs/reviewer.leftover.md
printf 'role: reviewer\nagent: bob\nwriter: wm-run\nafter-maker: leftover-old\nISOLATION: SUBAGENT-ISOLATED\n' > .wm/invoke/reviewer.log
rm -f .wm/reviewer-ran
run_wm_loop
assert_loop_foreground 't12-loop-leftover'
[ -f .wm/reviewer-ran ] && ok || bad 'leftover receipts must not skip reviewer exec'
if grep -q 'CLOSED PASS' "$OUT" && [ -f .wm/reviewer-ran ]; then
  ok
else
  bad "leftover-receipts loop wanted CLOSED PASS after reviewer exec, got out=$(cat "$OUT") ran=$(test -f .wm/reviewer-ran && echo yes || echo no)"
fi

# (13) Named cheat: planted matching receipts after maker-build must not CLOSED PASS
# without exec'ing the reviewer (echo CLI writes no WORD). Evidence is recorded
# under a planted reviewer dispatch so next is NEXT CLOSE (same-uid residual shape).
setup_repo t13-loop-planted
reach_green
"$WM" cast reviewer bob grok 'sh -c "echo reviewer {BRIEF}"' >/dev/null
mkdir -p .wm/verdicts .wm/return .wm/invoke .wm/spawn .wm/briefs
printf 'role: reviewer\nagent: bob\n' > .wm/dispatch
ev=$("$WM" evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
rm -f .wm/dispatch
need=$(awk -F ': ' '$1=="id"{print $2; exit}' .wm/last-maker-run)
t=$(date +%s)
runid="${t}.planted"
printf 't: %s\nabsent_return: yes\npath: .wm/return/bob.md\nrole: reviewer\nrun-id: %s\n' "$t" "$runid" > .wm/spawn/bob.stamp
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
wid=$("$WM" workid)
printf 'Read this file and follow it exactly.\nrole: reviewer\nagent: bob\n' > ".wm/briefs/reviewer.${wid}.md"
{
  printf 'VERDICT: PASS\n'
  printf 'AGENT: bob\n'
  printf 'WORK-ID: %s\n' "$wid"
  printf 'EVIDENCE: %s\n' "$ev"
  printf 'INGEST: return\n'
  printf 'RUN-ID: %s\n' "$runid"
  printf 'ISOLATION: SUBAGENT-ISOLATED\n'
  printf 'MODEL-SWITCH: UNVERIFIED\n'
} > .wm/verdicts/bob.md
printf 'role: reviewer\nagent: bob\nwriter: wm-run\nafter-maker: %s\nISOLATION: SUBAGENT-ISOLATED\n' "$need" > .wm/invoke/reviewer.log
card=$("$WM" next)
printf '%s\n' "$card" | grep -q 'NEXT CLOSE' \
  && ok || bad "planted matching receipts should look closeable to next (got $card)"
run_wm_loop
assert_loop_foreground 't13-loop-planted'
if closed_pass_present; then
  bad "planted receipts after maker-build must not CLOSED PASS (out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT))"
else
  ok
fi
[ -f .wm/reviewer-ran ] && bad 'echo reviewer must not have been a PASS exec' || ok

# (14) Loopfull via wm loop: maker-build forges judge artifacts; no CLOSED PASS
setup_repo t14-loopfull-loop
mkdir -p tools
cat > tools/loopfull-maker.sh <<'EOF'
#!/bin/sh
set -eu
if [ ! -f .wm/FALSIFIER ]; then
  printf 'test -f product.txt\n' > .wm/FALSIFIER
  if command -v sha256sum >/dev/null 2>&1; then
    h=$(sha256sum .wm/FALSIFIER | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    h=$(shasum -a 256 .wm/FALSIFIER | awk '{print $1}')
  else
    h=$(openssl dgst -sha256 .wm/FALSIFIER | awk '{print $NF}')
  fi
  printf 'agent: alice\nwork-id: %s\nsha256: %s\n' "$(git rev-parse --short=12 HEAD)" "$h" > .wm/FALSIFIER.meta
  exit 0
fi
printf 'built\n' > product.txt
git add product.txt
git commit -qm maker-build
mkdir -p .wm/spawn .wm/return .wm/briefs .wm/verdicts .wm/invoke
printf 'role: reviewer\nagent: bob\n' > .wm/dispatch
ev=$(.wm/bin/wm evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
t=$(date +%s)
runid="${t}.forged"
printf 't: %s\nabsent_return: yes\npath: .wm/return/bob.md\nrole: reviewer\nrun-id: %s\n' "$t" "$runid" > .wm/spawn/bob.stamp
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
wid=$(git rev-parse --short=12 HEAD)
printf 'Read this file and follow it exactly.\nrole: reviewer\nagent: bob\n' > ".wm/briefs/reviewer.${wid}.md"
printf 'VERDICT: PASS\nAGENT: bob\nWORK-ID: %s\nEVIDENCE: %s\nINGEST: return\nRUN-ID: %s\nISOLATION: SUBAGENT-ISOLATED\nMODEL-SWITCH: UNVERIFIED\n' "$wid" "$ev" "$runid" > .wm/verdicts/bob.md
printf 'role: reviewer\nagent: bob\nwriter: wm-run\nafter-maker: forged\n' > .wm/invoke/reviewer.log
EOF
cat > tools/rev-should-not.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return
ev=$(.wm/bin/wm evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
EOF
chmod +x tools/loopfull-maker.sh tools/rev-should-not.sh
"$WM" cast maker alice grok './tools/loopfull-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer bob grok './tools/rev-should-not.sh {BRIEF}' >/dev/null
commit_msg 'loopfull workers'
run_wm_loop
assert_loop_foreground 't14-loopfull-loop'
if closed_pass_present; then
  bad 'loopfull via wm loop must not CLOSED PASS'
else
  ok
fi
if [ -f .wm/reviewer-ran ]; then
  bad 'loopfull via wm loop must not exec reviewer (REVIEWER_RAN present)'
else
  ok
fi
[ "$LOOP_RC" -ne 0 ] && ok || bad 'loopfull via wm loop must not exit 0'
grep -q 'maker wrote judge artifacts' "$ERR" || grep -q '^refused:' "$ERR" \
  && ok || bad "loopfull via wm loop expected refused: (err=$(cat "$ERR"))"

# (15) 2c: loop must not bypass live/push-main/rm -rf guards
setup_repo t15-loop-push
"$WM" record-pre-falsify >/dev/null
write_falsifier 'git push origin main' alice
commit_msg 'loop push falsifier'
run_wm_loop
assert_loop_foreground 't15-loop-push'
if closed_pass_present; then
  bad 'loop must not CLOSED PASS on push-main falsifier'
else
  ok
fi
[ "$LOOP_RC" -ne 0 ] && ok || bad 'loop push-main must not exit 0'
grep -E -q 'refused:|STOP-ASK|push-main' "$ERR" "$OUT" \
  && ok || bad "loop push-main expected refuse/STOP-ASK, got out=$(cat "$OUT") err=$(cat "$ERR")"

setup_repo t15-loop-rm
sentinel="$BASE/t15-loop-sentinel"
mkdir -p "$sentinel/keep"
printf 'keep\n' > "$sentinel/keep/file"
"$WM" record-pre-falsify >/dev/null
write_falsifier "rm -rf $sentinel" alice
commit_msg 'loop rm falsifier'
run_wm_loop
assert_loop_foreground 't15-loop-rm'
[ -f "$sentinel/keep/file" ] && ok || bad 'loop execd rm -rf (sentinel gone)'
if closed_pass_present; then
  bad 'loop must not CLOSED PASS on rm -rf falsifier'
else
  ok
fi

# (16) ESCALATE EARLY_IMPLEMENT is terminal (no wait)
setup_repo t16-loop-escalate
"$WM" record-pre-falsify >/dev/null
write_falsifier 'true' alice
printf 'early product\n' > product.txt
commit_msg 'loop early product'
run_wm_loop
assert_loop_foreground 't16-loop-escalate'
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'ESCALATE' \
  && ok || bad "loop early-implement wanted ESCALATE, got out=$(cat "$OUT") err=$(cat "$ERR")"
[ "$LOOP_RC" -ne 0 ] && ok || bad 'loop ESCALATE must not exit 0'
if closed_pass_present; then
  bad 'loop ESCALATE must not CLOSED PASS'
else
  ok
fi

# (17) CLOSED NO-BUILD still requires reviewer exec
setup_repo t17-loop-nobuild
write_loop_maker_nobuild
write_loop_reviewer_nobuild
"$WM" cast maker alice grok './tools/loop-maker-nobuild.sh {BRIEF}' >/dev/null
"$WM" cast reviewer bob grok './tools/loop-reviewer-nobuild.sh {BRIEF}' >/dev/null
commit_msg 'loop nobuild workers'
run_wm_loop
assert_loop_foreground 't17-loop-nobuild'
if grep -q 'CLOSED NO-BUILD' "$OUT" && [ -f .wm/CLOSED ] && grep -q 'CLOSED NO-BUILD' .wm/CLOSED; then
  ok
else
  bad "nobuild loop wanted CLOSED NO-BUILD, got out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT)"
fi
[ -f .wm/reviewer-ran ] && ok || bad 'nobuild loop did not exec the reviewer CLI'
[ "$LOOP_RC" -eq 0 ] && ok || bad "nobuild loop exit $LOOP_RC err=$(cat "$ERR")"

# (18) Planted .wm/CLOSED without reviewer exec must not make wm loop CLOSED PASS
setup_repo t18-planted-closed
printf 'CLOSED PASS\n' > .wm/CLOSED
run_wm_loop
assert_loop_foreground 't18-planted-closed'
if grep -q 'CLOSED PASS' "$OUT"; then
  bad "planted .wm/CLOSED must not make loop print CLOSED PASS (out=$(cat "$OUT") rc=$LOOP_RC)"
else
  ok
fi
[ "$LOOP_RC" -ne 0 ] && ok || bad 'planted .wm/CLOSED loop must not exit 0'
[ -f .wm/reviewer-ran ] && bad 'planted CLOSED must not count as reviewer exec' || ok
if [ -f .wm/invoke/reviewer.log ]; then
  bad 'planted CLOSED loop must not skip to a reviewer invoke.log'
else
  ok
fi

# (19) Maker-build that writes only .wm/CLOSED is a judge artifact / not CLOSED PASS
setup_repo t19-maker-closed
mkdir -p tools
cat > tools/maker-closed.sh <<'EOF'
#!/bin/sh
set -eu
if [ ! -f .wm/FALSIFIER ]; then
  printf 'test -f product.txt\n' > .wm/FALSIFIER
  if command -v sha256sum >/dev/null 2>&1; then
    h=$(sha256sum .wm/FALSIFIER | awk '{print $1}')
  elif command -v shasum >/dev/null 2>&1; then
    h=$(shasum -a 256 .wm/FALSIFIER | awk '{print $1}')
  else
    h=$(openssl dgst -sha256 .wm/FALSIFIER | awk '{print $NF}')
  fi
  printf 'agent: alice\nwork-id: %s\nsha256: %s\n' "$(git rev-parse --short=12 HEAD)" "$h" > .wm/FALSIFIER.meta
  exit 0
fi
printf 'built\n' > product.txt
git add product.txt
git commit -qm maker-build
printf 'CLOSED PASS\n' > .wm/CLOSED
EOF
cat > tools/rev-should-not-closed.sh <<'EOF'
#!/bin/sh
set -eu
printf 'ran\n' > .wm/reviewer-ran
mkdir -p .wm/return
ev=$(.wm/bin/wm evidence bob -- sh -c 'echo test -f product.txt; test -f product.txt')
printf 'WORD: PASS\nEVIDENCE: %s\n' "$ev" > .wm/return/bob.md
EOF
chmod +x tools/maker-closed.sh tools/rev-should-not-closed.sh
"$WM" cast maker alice grok './tools/maker-closed.sh {BRIEF}' >/dev/null
"$WM" cast reviewer bob grok './tools/rev-should-not-closed.sh {BRIEF}' >/dev/null
commit_msg 'maker-closed workers'
run_wm_loop
assert_loop_foreground 't19-maker-closed'
if grep -q 'CLOSED PASS' "$OUT"; then
  bad "maker-written CLOSED must not make loop print CLOSED PASS (out=$(cat "$OUT"))"
else
  ok
fi
if [ -f .wm/reviewer-ran ]; then
  bad 'maker-written CLOSED must not exec reviewer (REVIEWER_RAN present)'
else
  ok
fi
[ "$LOOP_RC" -ne 0 ] && ok || bad 'maker-written CLOSED loop must not exit 0'
grep -q 'maker wrote judge artifacts' "$ERR" || grep -q '^refused:' "$ERR" \
  && ok || bad "maker-written CLOSED expected refused: (err=$(cat "$ERR"))"

# ---------------------------------------------------------------------------
# Task 7: Learning (12e) — LESSONS.md + ARCH fence. Kernel fixtures without
# adopt write repo-root LESSONS.md (not $HOME, not .wm). NONE is a valid line.
# ---------------------------------------------------------------------------

# (20) close without a lesson line refuses (does not CLOSED PASS, no LESSONS)
setup_repo t20-close-no-lesson
reach_reviewer_pass
refuses 'close without a lesson line refuses' 'refused:' "$WM" close
if [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  bad 'close without lesson must not write CLOSED PASS'
else
  ok
fi
if [ -f LESSONS.md ] && [ -s LESSONS.md ]; then
  bad 'close without lesson must not append LESSONS.md'
else
  ok
fi
if [ -f "$HOME/LESSONS.md" ]; then
  bad 'close without lesson wrote $HOME/LESSONS.md'
else
  ok
fi

# (21) close NONE: exactly one line at repo-root LESSONS.md; not $HOME
setup_repo t21-close-none
reach_reviewer_pass
expect 'close NONE is CLOSED PASS' 'CLOSED PASS' "$WM" close NONE
[ -f LESSONS.md ] && ok || bad 'close NONE must write repo-root LESSONS.md'
n=0
[ -f LESSONS.md ] && n=$(wc -l < LESSONS.md | tr -d ' ')
[ "$n" = 1 ] && ok || bad "close NONE must append exactly one line, got $n"
if [ -f LESSONS.md ] && grep -q '^NONE$' LESSONS.md; then
  ok
else
  bad "close NONE wanted ^NONE$, got $(cat LESSONS.md 2>/dev/null || echo ABSENT)"
fi
if [ -f "$HOME/LESSONS.md" ]; then
  bad 'close wrote LESSONS.md under $HOME'
else
  ok
fi
if [ -f .wm/LESSONS.md ]; then
  bad 'kernel fixture must use repo-root LESSONS.md, not .wm/LESSONS.md'
else
  ok
fi

# (22) adopted program dir: append .crucible/<prog>/LESSONS.md, not repo-root
setup_repo t22-close-program
reach_reviewer_pass
mkdir -p .crucible/work
printf 'name: work\n' > .crucible/work/PROGRAM
expect 'close NONE with program dir' 'CLOSED PASS' "$WM" close NONE
[ -f .crucible/work/LESSONS.md ] && ok || bad 'program dir close must write .crucible/work/LESSONS.md'
grep -q '^NONE$' .crucible/work/LESSONS.md \
  && ok || bad "program LESSONS wanted ^NONE$, got $(cat .crucible/work/LESSONS.md 2>/dev/null || echo ABSENT)"
n=$(wc -l < .crucible/work/LESSONS.md | tr -d ' ')
[ "$n" = 1 ] && ok || bad "program LESSONS must be exactly one line, got $n"
if [ -f LESSONS.md ]; then
  bad 'program dir close must not also write repo-root LESSONS.md'
else
  ok
fi
if [ -f "$HOME/LESSONS.md" ]; then
  bad 'program dir close wrote $HOME/LESSONS.md'
else
  ok
fi

# (23) ARCH: close → STOP-ASK; do not start next maker
setup_repo t23-arch-close
reach_reviewer_pass
install_maker_dump
"$WM" cast maker alice grok './tools/maker-dump.sh {BRIEF}' >/dev/null
set +e
"$WM" close 'ARCH: add a second pattern' >"$OUT" 2>"$ERR"
arch_rc=$?
set -e
if grep -q 'CLOSED PASS' "$OUT" && [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  ok
else
  bad "ARCH close wanted CLOSED PASS then fence, got out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT) err=$(cat "$ERR")"
fi
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'STOP-ASK' \
  && ok || bad "ARCH: lesson must STOP-ASK (out=$(cat "$OUT") err=$(cat "$ERR"))"
[ -f LESSONS.md ] && grep -q '^ARCH: add a second pattern$' LESSONS.md \
  && ok || bad "ARCH close must append ^ARCH: line, got $(cat LESSONS.md 2>/dev/null || echo ABSENT)"
n=$(wc -l < LESSONS.md | tr -d ' ')
[ "$n" = 1 ] && ok || bad "ARCH close must append exactly one line, got $n"
rm -f .wm/maker-ran .wm/dumped-brief.md
set +e
"$WM" run maker-build >"$OUT" 2>"$ERR"
mb_rc=$?
set -e
[ "$mb_rc" -ne 0 ] && ok || bad "ARCH fence: wm run maker-build must not succeed (out=$(cat "$OUT"))"
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'STOP-ASK' \
  && ok || bad "ARCH fence maker-build wanted STOP-ASK, got out=$(cat "$OUT") err=$(cat "$ERR")"
if [ -f .wm/maker-ran ]; then
  bad 'ARCH fence must not exec the next maker (maker-ran present)'
else
  ok
fi
rm -f .wm/maker-ran
set +e
"$WM" run maker-falsify >"$OUT" 2>"$ERR"
mf_rc=$?
set -e
[ "$mf_rc" -ne 0 ] && ok || bad 'ARCH fence: wm run maker-falsify must not succeed'
if [ -f .wm/maker-ran ]; then
  bad 'ARCH fence must not exec maker-falsify'
else
  ok
fi
card=$("$WM" next)
printf '%s\n' "$card" | grep -q 'STOP-ASK' \
  && ok || bad "after ARCH close, next must be STOP-ASK not a maker card (got $card)"
printf '%s\n' "$card" | grep -q 'NEXT RUN maker' \
  && bad "after ARCH close, next must not start maker (got $card)" || ok

# (24) planted ^ARCH: line fences makers without going through close
setup_repo t24-arch-planted
printf 'ARCH: planted second pattern\n' > LESSONS.md
install_maker_dump
"$WM" cast maker alice grok './tools/maker-dump.sh {BRIEF}' >/dev/null
card=$("$WM" next)
printf '%s\n' "$card" | grep -q 'STOP-ASK' \
  && ok || bad "planted ARCH: next wanted STOP-ASK, got $card"
printf '%s\n' "$card" | grep -E -q 'NEXT RUN maker|NEXT RECORD PRE-FALSIFY' \
  && bad "planted ARCH: next must not start maker (got $card)" || ok
rm -f .wm/maker-ran .wm/FALSIFIER
refuses 'planted ARCH: run maker-falsify STOP-ASK' 'STOP-ASK' "$WM" run maker-falsify
if [ -f .wm/maker-ran ] || [ -f .wm/FALSIFIER ]; then
  bad 'planted ARCH: must not exec maker-falsify'
else
  ok
fi
run_wm_loop
assert_loop_foreground 't24-arch-planted'
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'STOP-ASK' \
  && ok || bad "planted ARCH loop wanted STOP-ASK, got out=$(cat "$OUT") err=$(cat "$ERR")"
[ "$LOOP_RC" -ne 0 ] && ok || bad 'planted ARCH loop must not exit 0'
if [ -f .wm/maker-ran ] || [ -f .wm/FALSIFIER ]; then
  bad 'planted ARCH loop must not start a maker'
else
  ok
fi
if closed_pass_present; then
  bad 'planted ARCH loop must not CLOSED PASS'
else
  ok
fi

# (25) RULE 23 analogue: next maker brief concatenates LESSONS (or NONE)
setup_repo t25-brief-concat
reach_reviewer_pass
expect 'close behavioral lesson' 'CLOSED PASS' "$WM" close 'check issue-key shape before interpolating a path'
grep -q '^check issue-key shape before interpolating a path$' LESSONS.md \
  && ok || bad "behavioral close must append the lesson line, got $(cat LESSONS.md 2>/dev/null || echo ABSENT)"
install_maker_dump
"$WM" cast maker alice grok './tools/maker-dump.sh {BRIEF}' >/dev/null
if ! "$WM" run maker-build >"$OUT" 2>"$ERR"; then
  bad "behavioral lesson must not fence maker-build: $(cat "$OUT") $(cat "$ERR")"
else
  ok
fi
[ -f .wm/maker-ran ] && ok || bad 'behavioral lesson must still exec next maker'
if [ -f .wm/dumped-brief.md ]; then
  grep -q 'check issue-key shape before interpolating a path' .wm/dumped-brief.md \
    && ok || bad "maker brief must concatenate LESSONS, got $(cat .wm/dumped-brief.md)"
  grep -qi 'Lessons' .wm/dumped-brief.md \
    && ok || bad 'maker brief must name Lessons (RULE 23 analogue)'
else
  bad 'maker-build did not dump BRIEF'
fi
# reviewer brief must not include LESSONS (RULE 10 analogue)
if ! "$WM" run reviewer >"$OUT" 2>"$ERR"; then
  bad "reviewer re-run after close failed: $(cat "$OUT") $(cat "$ERR")"
else
  ok
fi
revb=
for f in .wm/briefs/reviewer.*; do
  [ -f "$f" ] || continue
  revb=$f
done
if [ -n "$revb" ] && [ -f "$revb" ]; then
  grep -qi 'Lessons from earlier' "$revb" \
    && bad "reviewer brief must not concatenate LESSONS (got $revb)" || ok
  grep -q 'check issue-key shape' "$revb" \
    && bad 'reviewer brief must not contain the lesson line' || ok
else
  bad 'reviewer brief missing after re-run'
fi

# NONE in the next maker brief when that was the lesson
setup_repo t25-brief-none
reach_reviewer_pass
expect 'close NONE for brief' 'CLOSED PASS' "$WM" close NONE
install_maker_dump
"$WM" cast maker alice grok './tools/maker-dump.sh {BRIEF}' >/dev/null
if ! "$WM" run maker-build >"$OUT" 2>"$ERR"; then
  bad "NONE lesson must not fence maker-build: $(cat "$OUT") $(cat "$ERR")"
else
  ok
fi
if [ -f .wm/dumped-brief.md ]; then
  grep -q 'NONE' .wm/dumped-brief.md \
    && ok || bad "maker brief must include NONE, got $(cat .wm/dumped-brief.md)"
else
  bad 'NONE brief dump missing'
fi

# ARCH: lines are not maker-binding bullets (fence already covers run; brief skip is extra)
setup_repo t25-brief-arch-skip
reach_reviewer_pass
set +e
"$WM" close 'ARCH: never bind this' >"$OUT" 2>"$ERR"
set -e
# If a brief were written, it must not bind the ARCH line. Fence must prevent the run.
install_maker_dump
"$WM" cast maker alice grok './tools/maker-dump.sh {BRIEF}' >/dev/null
rm -f .wm/dumped-brief.md .wm/maker-ran
set +e
"$WM" run maker-build >"$OUT" 2>"$ERR"
set -e
if [ -f .wm/dumped-brief.md ] && grep -q 'never bind this' .wm/dumped-brief.md; then
  bad 'ARCH: lesson must not be concatenated as a maker-binding bullet'
else
  ok
fi
if [ -f .wm/maker-ran ]; then
  bad 'ARCH: brief-skip fixture must not exec maker'
else
  ok
fi

# (26) loop-design debrief may write proposals/ only; kernel does not apply patches
setup_repo t26-proposals
reach_reviewer_pass
mkdir -p proposals .crucible/skills/loop-design
printf 'ORIGINAL-SKILL\n' > .crucible/skills/loop-design/SKILL.md
printf 'PATCH-SHOULD-NOT-APPLY\n' > proposals/loop-design.patch
expect 'close does not apply proposals' 'CLOSED PASS' "$WM" close NONE
grep -q 'ORIGINAL-SKILL' .crucible/skills/loop-design/SKILL.md \
  && ok || bad 'close must not apply proposals/ onto skills'
grep -q 'PATCH-SHOULD-NOT-APPLY' .crucible/skills/loop-design/SKILL.md \
  && bad 'close applied proposals/ patch into skills' || ok
[ -f proposals/loop-design.patch ] && ok || bad 'proposals/ must remain (debrief writes there only)'
# wm.sh must not copy proposals/ into skills/ (human / refresh KEEP)
if grep -E 'proposals/' "$WM" | grep -E -q 'skills/|cp |mv '; then
  bad 'wm.sh must not auto-apply proposals/ into skills/'
else
  ok
fi

# (27) wm loop with WM_LESSON=ARCH: STOP-ASK, does not start a further maker
setup_repo t27-loop-arch
write_loop_maker_pass
write_loop_reviewer_pass
"$WM" cast maker alice grok './tools/loop-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer bob grok './tools/loop-reviewer.sh {BRIEF}' >/dev/null
commit_msg 'loop arch workers'
WM_LESSON='ARCH: add a cache layer'
export WM_LESSON
run_wm_loop
unset WM_LESSON
assert_loop_foreground 't27-loop-arch'
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'STOP-ASK' \
  && ok || bad "ARCH loop wanted STOP-ASK, got out=$(cat "$OUT") err=$(cat "$ERR")"
[ -f LESSONS.md ] && grep -q '^ARCH: add a cache layer$' LESSONS.md \
  && ok || bad "ARCH loop must append ^ARCH: line, got $(cat LESSONS.md 2>/dev/null || echo ABSENT)"
if grep -q 'CLOSED PASS' "$OUT" || { [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; }; then
  ok
else
  bad "ARCH loop should still close the brick (out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT))"
fi
[ "$LOOP_RC" -ne 0 ] && ok || bad 'ARCH loop must not exit 0 (would proceed)'
install_maker_dump
"$WM" cast maker alice grok './tools/maker-dump.sh {BRIEF}' >/dev/null
rm -f .wm/maker-ran
refuses 'ARCH loop then maker-build STOP-ASK' 'STOP-ASK' "$WM" run maker-build
if [ -f .wm/maker-ran ]; then
  bad 'ARCH loop must not allow a next maker'
else
  ok
fi

# (28) honest loop without WM_LESSON appends NONE (valid lesson line)
setup_repo t28-loop-none
write_loop_maker_pass
write_loop_reviewer_pass
"$WM" cast maker alice grok './tools/loop-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer bob grok './tools/loop-reviewer.sh {BRIEF}' >/dev/null
commit_msg 'loop none workers'
run_wm_loop
assert_loop_foreground 't28-loop-none'
[ "$LOOP_RC" -eq 0 ] && ok || bad "honest loop with implicit NONE exit $LOOP_RC err=$(cat "$ERR")"
grep -q 'CLOSED PASS' "$OUT" && ok || bad "honest NONE loop wanted CLOSED PASS, got $(cat "$OUT")"
[ -f LESSONS.md ] && grep -q '^NONE$' LESSONS.md \
  && ok || bad "honest loop must append NONE, got $(cat LESSONS.md 2>/dev/null || echo ABSENT)"
n=$(wc -l < LESSONS.md | tr -d ' ')
[ "$n" = 1 ] && ok || bad "honest loop must append exactly one lesson line, got $n"

# (29) close multi-line lesson refuses; no append
setup_repo t29-multiline
reach_reviewer_pass
ml=$(printf 'line one\nline two')
refuses 'close multi-line lesson refuses' 'refused:' "$WM" close "$ml"
if [ -f LESSONS.md ] && [ -s LESSONS.md ]; then
  bad 'multi-line close must not append LESSONS.md'
else
  ok
fi
if [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  bad 'multi-line close must not write CLOSED PASS'
else
  ok
fi

# ---------------------------------------------------------------------------
# Task 1: multi-slice walker (A1–A5). One wm loop; no harness slices.tsv rewrite.
# ---------------------------------------------------------------------------

# first_ready_slice: depends_on tokens split on comma; parent status must be CLOSED
setup_repo t-a1-deps-comma
write_multi_spec
plant_map_accept_slices <<'EOF'
s1	alpha	a.txt	-	LOW	CLOSED
s2	beta	b.txt	-	LOW	CLOSED
s3	seam	c.txt	s1,s2	LOW	READY
EOF
commit_msg 'A1 comma deps'
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q '^NEXT SLICE s3$' \
    && ok || bad "A1 comma deps: next wanted NEXT SLICE s3, got $card"
else
  bad "A1 comma deps: next refused $(cat "$ERR")"
fi
plant_map_accept_slices <<'EOF'
s1	alpha	a.txt	-	LOW	CLOSED
s2	beta	b.txt	-	LOW	READY
s3	seam	c.txt	s1,s2	LOW	READY
EOF
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q '^NEXT SLICE s2$' \
    && ok || bad "A1 comma deps blocked: next wanted NEXT SLICE s2, got $card"
  printf '%s\n' "$card" | grep -q 'NEXT SLICE s3' \
    && bad "A1 comma deps: s3 must wait for s2 CLOSED (got $card)" || ok
else
  bad "A1 comma deps blocked: next refused $(cat "$ERR")"
fi

setup_repo t-a1-deps-chain
write_multi_spec
plant_map_accept_slices <<'EOF'
s1	alpha	a.txt	-	LOW	CLOSED
s2	beta	b.txt	s1	LOW	READY
s3	seam	c.txt	s2	LOW	READY
EOF
commit_msg 'A1 chain deps'
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q '^NEXT SLICE s2$' \
    && ok || bad "A1 chain: next wanted NEXT SLICE s2 after s1 CLOSED, got $card"
else
  bad "A1 chain: next refused $(cat "$ERR")"
fi

# (A1) three LOW slices, s2 depends_on=s1, s3 depends_on=s2; ONE wm loop
setup_repo t-a1-walk
write_multi_spec
plant_map_accept_slices <<'EOF'
s1	alpha	a.txt	-	LOW	READY
s2	beta	b.txt	s1	LOW	READY
s3	seam	c.txt	s2	LOW	READY
EOF
cast_multi_panel
commit_msg 'A1 three-slice workers'
slices_before=$(cat slices.tsv)
run_wm_loop
assert_loop_foreground 't-a1-walk'
[ "$LOOP_RC" -eq 0 ] && ok || bad "A1 walk exit $LOOP_RC out=$(cat "$OUT") err=$(cat "$ERR")"
if grep -q 'CLOSED PASS' "$OUT" && [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  ok
else
  bad "A1 walk wanted work-level CLOSED PASS, got out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT)"
fi
closed_rows=$(awk -F '\t' 'NR>1 && $6=="CLOSED" {c++} END{print c+0}' slices.tsv)
[ "$closed_rows" -eq 3 ] && ok || bad "A1 walk slices.tsv CLOSED rows wanted 3, got $closed_rows ($(cat slices.tsv))"
grep -q '^alpha$' a.txt && ok || bad 'A1 walk s1 did not land a.txt'
grep -q '^beta$' b.txt && ok || bad 'A1 walk s2 did not land b.txt'
grep -q '^seam$' c.txt && ok || bad 'A1 walk s3 did not land c.txt'
# harness must not have rewritten slices.tsv except via the kernel
if printf '%s\n' "$slices_before" | grep -q 'READY' && [ "$closed_rows" -eq 3 ]; then
  ok
else
  bad 'A1 walk: slices.tsv was not closed by the kernel'
fi
[ -f .wm/reviewer-ran ] && ok || bad 'A1 walk missing reviewer exec on last brick'

# (A2) planted CLOSED before loop must not CLOSED PASS / exit 0 without reviewer
setup_repo t-a2-planted
write_multi_spec
plant_map_accept_slices <<'EOF'
s1	alpha	a.txt	-	LOW	READY
s2	beta	b.txt	s1	LOW	READY
s3	seam	c.txt	s2	LOW	READY
EOF
commit_msg 'A2 planted CLOSED'
printf 'CLOSED PASS\n' > .wm/CLOSED
run_wm_loop
assert_loop_foreground 't-a2-planted'
if grep -q 'CLOSED PASS' "$OUT"; then
  bad "A2 planted .wm/CLOSED must not make loop print CLOSED PASS (out=$(cat "$OUT") rc=$LOOP_RC)"
else
  ok
fi
[ "$LOOP_RC" -ne 0 ] && ok || bad 'A2 planted CLOSED loop must not exit 0'
[ -f .wm/reviewer-ran ] && bad 'A2 planted CLOSED must not count as reviewer exec' || ok

# (A3) after honest s1, plant CLOSED during s2 brick — s2 must not CLOSED PASS without reviewer
setup_repo t-a3-plant-s2
write_multi_spec
plant_map_accept_slices <<'EOF'
s1	alpha	a.txt	-	LOW	READY
s2	beta	b.txt	s1	LOW	READY
EOF
mkdir -p tools
cat > tools/a3-maker.sh <<'EOF'
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
printf 'ran %s %s\n' "$role" "$sid" >> .wm/maker-ran
sha_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  fi
}
if [ "$role" = maker-build ]; then
  printf 'alpha\n' > a.txt
  git add a.txt
  git commit -qm maker-build-s1
  exit 0
fi
mkdir -p .wm
if [ "$sid" = s2 ]; then
  printf 'test -f b.txt\n' > .wm/FALSIFIER
  h=$(sha_of .wm/FALSIFIER)
  wid=$(git rev-parse --short=12 HEAD)
  printf 'agent: alice\nwork-id: %s\nsha256: %s\n' "$wid" "$h" > .wm/FALSIFIER.meta
  printf 'CLOSED PASS\n' > .wm/CLOSED
  exit 0
fi
printf 'test -f a.txt\n' > .wm/FALSIFIER
h=$(sha_of .wm/FALSIFIER)
wid=$(git rev-parse --short=12 HEAD)
printf 'agent: alice\nwork-id: %s\nsha256: %s\n' "$wid" "$h" > .wm/FALSIFIER.meta
EOF
write_multi_reviewer
chmod +x tools/a3-maker.sh
"$WM" cast maker alice grok './tools/a3-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer bob grok './tools/multi-reviewer.sh {BRIEF}' >/dev/null
commit_msg 'A3 plant CLOSED on s2'
run_wm_loop
assert_loop_foreground 't-a3-plant-s2'
s2st=$(awk -F '\t' '$1=="s2"{print $6; exit}' slices.tsv)
[ "$s2st" != CLOSED ] && ok || bad "A3 s2 must not be CLOSED without reviewer exec (status=$s2st)"
[ "$LOOP_RC" -ne 0 ] && ok || bad "A3 planted CLOSED during s2 must not exit 0 (out=$(cat "$OUT"))"
if grep -q ' s2$' .wm/maker-ran 2>/dev/null && [ "$s2st" = CLOSED ] && [ ! -f .wm/reviewer-ran ]; then
  bad 'A3 s2 CLOSED PASS skipped reviewer'
else
  ok
fi

# (A4) s1 LOW then s2 HIGH unsigned: STOP-ASK after s1; s2 maker not exec'd
setup_repo t-a4-high-unsigned
write_multi_spec
plant_map_accept_slices <<'EOF'
s1	alpha	a.txt	-	LOW	READY
s2	beta	b.txt	s1	HIGH	READY
EOF
cast_multi_panel
rm -f MAP-HUMAN
commit_msg 'A4 LOW then HIGH unsigned'
run_wm_loop
assert_loop_foreground 't-a4-high-unsigned'
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -E -q 'STOP-ASK( MAP-HUMAN)?' \
  && ok || bad "A4 wanted STOP-ASK after s1, got out=$(cat "$OUT") err=$(cat "$ERR")"
[ "$LOOP_RC" -ne 0 ] && ok || bad 'A4 HIGH unsigned loop must not exit 0'
s1st=$(awk -F '\t' '$1=="s1"{print $6; exit}' slices.tsv)
[ "$s1st" = CLOSED ] && ok || bad "A4 s1 should CLOSED before HIGH stop, got $s1st"
grep -q '^alpha$' a.txt && ok || bad 'A4 s1 did not land a.txt'
if grep -q ' s2$' .wm/maker-ran 2>/dev/null; then
  bad 'A4 s2 maker must not be execd'
else
  ok
fi
if [ -f .wm/FALSIFIER ] && grep -q 'b.txt' .wm/FALSIFIER; then
  bad 'A4 s2 FALSIFIER must be absent'
else
  ok
fi
if [ -f MAP-HUMAN ]; then
  bad 'A4 must not auto-write MAP-HUMAN'
else
  ok
fi

# (A5) blank-home walk must not define mark_slice_closed or reset_brick
if grep -E -q 'mark_slice_closed\(\)|reset_brick\(\)' \
  "$HERE/scripts/verify-working-mode-blank-home.sh"; then
  bad 'A5: blank-home must not define mark_slice_closed or reset_brick'
else
  ok
fi

# Task 3: START/BOOTSTRAP discoverability — one opt-in pointer; guided default
# stays adopt work --managed without --working-mode.
if grep -qi 'working-mode' "$HERE/START.md" \
  && grep -q 'docs/working-mode.md' "$HERE/START.md"; then
  ok
else
  bad 'START.md must point to docs/working-mode.md (opt-in)'
fi
if grep -qi 'working-mode' "$HERE/BOOTSTRAP.md" \
  && grep -q 'docs/working-mode.md' "$HERE/BOOTSTRAP.md"; then
  ok
else
  bad 'BOOTSTRAP.md must point to docs/working-mode.md (opt-in)'
fi
if grep -q 'adopt work --managed' "$HERE/BOOTSTRAP.md"; then
  ok
else
  bad 'BOOTSTRAP.md missing guided adopt work --managed'
fi
if grep -E 'adopt work --managed' "$HERE/BOOTSTRAP.md" | grep -q -- '--working-mode'; then
  bad 'BOOTSTRAP guided adopt must not add --working-mode'
else
  ok
fi
if grep -E 'adopt (work|NAME|<program>) --managed --working-mode' "$HERE/START.md" >/dev/null; then
  bad 'START.md must not show guided adopt with --working-mode'
else
  ok
fi
if grep -q '.crucible/<program>/wm.sh' "$HERE/docs/working-mode.md"; then
  ok
else
  bad 'docs/working-mode.md must invoke .crucible/<program>/wm.sh from the target root'
fi
if grep -E '^wm[[:space:]]' "$HERE/docs/working-mode.md" >/dev/null; then
  bad 'docs/working-mode.md must not show bare wm commands'
else
  ok
fi

# Task 3: wm run maker-build whose command is false exits non-zero; no CLOSED PASS
setup_repo t-run-rc-false
"$WM" cast maker alice grok false >/dev/null
rm -f .wm/CLOSED
set +e
"$WM" run maker-build >"$OUT" 2>"$ERR"
run_rc=$?
set -e
[ "$run_rc" -ne 0 ] && ok || bad "wm run maker-build of false must exit non-zero (rc=$run_rc out=$(cat "$OUT") err=$(cat "$ERR"))"
if closed_pass_present; then
  bad "wm run maker-build of false must not CLOSED PASS (out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT))"
else
  ok
fi

# Home leak: empty HOME must stay empty (no skills, no LESSONS)
home_leftovers=$(find "$EMPTY_HOME" -mindepth 1 -print | sort || true)
if [ -z "$home_leftovers" ]; then
  ok
else
  bad "wrote under HOME: $home_leftovers"
fi

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
