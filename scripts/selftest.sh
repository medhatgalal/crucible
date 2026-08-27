#!/bin/sh
# selftest - verify that crucible actually refuses what it claims to refuse.
#
# No dependencies. Every case runs in its own temp directory. A claim in the
# documentation that is not asserted here is an unverified claim.
#
#   ./scripts/selftest.sh            run everything
#   ./scripts/selftest.sh -v         show each case as it runs
#
# Exit 0 only if every assertion holds.

set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
C="$HERE/crucible"
[ -x "$C" ] || { echo "selftest: $C is not executable"; exit 2; }
VERBOSE=0; FAST=0
for a in "$@"; do
  case $a in
    -v) VERBOSE=1 ;;
    --fast) FAST=1 ;;
  esac
done
# --fast skips the two slow groups (tmux overlay, concurrency) for quick iteration. A full run
# takes about three minutes, mostly waiting on tmux and on parallel processes; --fast is for
# development and is never sufficient for a release.

# One named scratch root. This suite used to leak nine `mktemp -d` trees per run, and it is not
# free to collect them in a shell variable instead: mkrun() runs inside `$(…)`, so a directory it
# appends to a list is recorded in a subshell that the parent's trap never sees. Nesting every
# scratch tree under this root is what lets one line of cleanup cover all of them.
#
# The handlers are split by signal, and that split is the whole point. 1.6.4 armed a single
# handler for `0 1 2 15` and its release notes claimed cleanup happened "without masking exit
# status". That was wrong for the signals: a handler that returns without calling exit hands
# control back to the shell, which carries on and can reach the final `exit 0`, so an interrupted
# or timed-out suite reported a pass. A CI wrapper or an agent that puts a clock on this suite
# reads that as green, which is the one failure mode a verification script must never have.
#
# On 0 the handler still only removes and returns, because there the suite's own status is
# already decided and calling exit would be what masks it — a failing run must keep its 1 and a
# passing run its 0. On 1, 2 and 15 the handler cleans up and exits 128+signal, the status a
# wrapper already reads as "killed by a signal". The 0 handler runs again on the way out; `rm -rf`
# on a path that is already gone is silent, which is why re-entry needs no guard.
SELFTEST_TMP=$(mktemp -d "${TMPDIR:-/tmp}/crucible-selftest.XXXXXX")
trap 'rm -rf "$SELFTEST_TMP"' 0
trap 'rm -rf "$SELFTEST_TMP"; exit 129' 1
trap 'rm -rf "$SELFTEST_TMP"; exit 130' 2
trap 'rm -rf "$SELFTEST_TMP"; exit 143' 15
PASS=0; FAIL=0; FAILED=""

say() { [ "$VERBOSE" = 1 ] && printf '    %s\n' "$*" || true; }
ok()   { PASS=$((PASS+1)); [ "$VERBOSE" = 1 ] && printf '  ok   %s\n' "$1" || printf '.'; }
bad()  { FAIL=$((FAIL+1)); FAILED="$FAILED
  FAILED: $1"; printf '\n  FAIL %s\n' "$1"; }

# A fresh run root with a registered panel, one item, a written falsifier and work.
# Prints the directory. The caller must cd into it: `cd "$(fresh)"`.
# Setup runs in a subshell so this function never changes the caller's directory.
mkrun() {
  d=$(mktemp -d "$SELFTEST_TMP/run.XXXXXX")
  ( cd "$d"
    cp "$C" ./crucible; cp -R "$HERE/roles" .; cp "$HERE/RULES.md" .
    printf 'mk\tkiro\tm\thigh\techo {BRIEF}\n'  > agents.tsv
    printf 'j1\tkiro\tm\thigh\techo {BRIEF}\n' >> agents.tsv
    printf 'j2\tgrok\tm\thigh\techo {BRIEF}\n' >> agents.tsv
    ./crucible add it "selftest item" >/dev/null
    [ "${1:-}" = nofalsifier ] || {
      sed 's|^TEMPLATE-FALSIFIER-UNWRITTEN.*|Undo the change; the named check fails.|' items/it/ITEM.md > i.tmp
      mv i.tmp items/it/ITEM.md
      # Fenced one-line declaration. Marker path is $d/mechanism — outside items/it/work/
      # so a sandbox work id does not move when the pair is recorded.
      printf '\n```\nsh -c '"'"'test -f mechanism'"'"'\n```\n' >> items/it/ITEM.md
    }
    [ "${1:-}" = nowork ] || { mkdir -p items/it/work; printf 'x=1\n' > items/it/work/a.py; }
    [ "${1:-}" = nomaker ] || printf 'mk\n' > items/it/MAKER
  )
  printf '%s' "$d"
}
fresh() { printf '%s' "$(mkrun "${1:-}")"; }

# Record a discriminating falsifier pair at the current work id.
# Declaration: sh -c 'test -f mechanism' (frame 3 2:sh 2:-c 17:test -f mechanism).
# Against an engine that does not yet accept --falsifier, this is a no-op for pair
# production (the label is refused) and must not fail the suite.
record_pair() {
  rm -f mechanism
  if ./crucible run it mk --falsifier removed -- sh -c 'test -f mechanism' >/dev/null 2>&1; then
    : > mechanism
    ./crucible run it mk --falsifier restored -- sh -c 'test -f mechanism' >/dev/null \
      || bad 'record_pair: the recorder accepted a removed direction and refused a restored one'
  else
    ./crucible run it mk --falsifier removed -- true >/dev/null 2>&1 \
      && bad 'record_pair: the recorder accepted a label but produced no pair'
    : > mechanism
  fi
}

# assert the gate refuses, and that the reason mentions PATTERN
refuses() {
  label=$1; pattern=$2
  out=$(./crucible check it 2>&1) && { bad "$label — gate ACCEPTED when it should refuse"; return; }
  printf '%s' "$out" | grep -q "$pattern" \
    && ok "$label" \
    || { bad "$label — refused but not for the stated reason (wanted: $pattern)"; say "$out"; }
}
accepts() {
  label=$1
  out=$(./crucible check it 2>&1) || { bad "$label — gate REFUSED when it should accept"; say "$out"; return; }
  ok "$label"
}

# Print the documentation paths that travel as operator documentation. This exactly mirrors
# adopt_install_engine's operator docs: five named root files plus top-level roles/*.md and
# docs/*.md. CHANGELOG.md is deliberately separate: release history does not travel.
travelling_doc_files() {
  root=$1
  for f in \
    "$root/BOOTSTRAP.md" \
    "$root/START.md" \
    "$root/RULES.md" \
    "$root/LOOP.md" \
    "$root/CONFIGURE.md" \
    "$root"/roles/*.md \
    "$root"/docs/*.md
  do
    [ -f "$f" ] && printf '%s\n' "$f"
  done
}

known_limits_section() {
  awk '
    $0 == "## Known limits" { in_limits=1; next }
    in_limits && /^## / { exit }
    in_limits { print }
  ' "$1" 2>/dev/null
}

known_limits_has() {
  printf '%s\n' "$1" | grep -Eiq -e "$2"
}

# Print a compact list of missing properties. Silence means the contract holds.
known_limits_missing() {
  root=$1
  headings=$(travelling_doc_files "$root" | while IFS= read -r f; do
    grep -c '^## Known limits$' "$f" 2>/dev/null || true
  done | awk '{ n += $1 } END { print n + 0 }')
  if [ -f "$root/docs/whats-new.md" ]; then
    target_count=$(grep -c '^## Known limits$' "$root/docs/whats-new.md" 2>/dev/null || true)
    section=$(known_limits_section "$root/docs/whats-new.md")
  else
    target_count=0
    section=""
  fi
  missing=""
  [ "$headings" -eq 1 ] && [ "$target_count" -eq 1 ] \
    || missing="$missing placement/exactly-one"

  known_limits_has "$section" 'brief' && known_limits_has "$section" 'help' \
    || missing="$missing brief/help"
  known_limits_has "$section" 'doc[- ]verb' \
    && known_limits_has "$section" 'extract' \
    && known_limits_has "$section" 'fence' \
    && known_limits_has "$section" 'README' \
    || missing="$missing doc-verb/extractor/fences/README"
  known_limits_has "$section" 'doc_set' && known_limits_has "$section" 'roles?' \
    || missing="$missing doc_set/roles"
  known_limits_has "$section" 'guided_min_judges.*(has|have)[[:space:]]+no[[:space:]]+floor' \
    || missing="$missing guided_min_judges/no-floor"
  known_limits_has "$section" 'stray' \
    && known_limits_has "$section" 'claim' \
    && known_limits_has "$section" 'attempt' \
    && known_limits_has "$section" 'ACP[- ]probe' \
    && known_limits_has "$section" 'invalid' \
    || missing="$missing stray-claim-attempt/ACP-probe-invalidation"
  known_limits_has "$section" 'quickstart' \
    && known_limits_has "$section" 'duplicat' \
    && known_limits_has "$section" 'shim' \
    || missing="$missing quickstart/duplicate-shim"
  known_limits_has "$section" 'CHANGELOG.*does[[:space:]]+not[[:space:]]+travel.*adopted[[:space:]]+trees' \
    || missing="$missing CHANGELOG/not-travelling-into-adopted-trees"
  known_limits_has "$section" '--refresh' \
    && known_limits_has "$section" 'rollback|roll back|restore' \
    || missing="$missing --refresh/rollback"
  known_limits_has "$section" 'single[- ]user' && known_limits_has "$section" 'author' \
    || missing="$missing single-user/authorship"
  known_limits_has "$section" 'falsifier pair proves' \
    && known_limits_has "$section" 'does not prove' \
    && known_limits_has "$section" 'removing the mechanism' \
    || missing="$missing falsifier-pair/no-causation"

  if [ -n "$missing" ]; then
    printf '%s\n' "$missing"
    return 1
  fi
  return 0
}

# Print forbidden explanatory prose, ignoring fenced examples. These implementation details and
# the known-false fourth-auditor divergence claim belong in tests, not release/operator prose.
internal_mechanism_prose() {
  root=$1
  {
    [ -f "$root/CHANGELOG.md" ] && printf '%s\n' "$root/CHANGELOG.md"
    travelling_doc_files "$root"
  } | while IFS= read -r f; do
    awk '
      function flush_paragraph(    lower) {
        lower = tolower(paragraph)
        if (lower ~ /the two surfaces can disagree in that state.*fourth[ -]+auditor.*claim admit.*still refuses/) {
          printf "%s:%d-%d:%s\n", FILENAME, paragraph_start, FNR, paragraph
        }
        paragraph = ""
        paragraph_start = 0
      }
      /^```/ { flush_paragraph(); fence = !fence; next }
      !fence {
        printf "%s:%d:%s\n", FILENAME, FNR, $0
        if ($0 ~ /^[[:space:]]*$/) {
          flush_paragraph()
        } else {
          if (paragraph == "") paragraph_start = FNR
          paragraph = paragraph (paragraph == "" ? "" : " ") $0
        }
      }
      END { flush_paragraph() }
    ' "$f"
  done | grep -Ei \
    -e '(^|[^[:alpha:]])subtract(s|ed|ing)?([^[:alpha:]]|$)|(^|[^[:alpha:]])fold(s|ed|ing)?([^[:alpha:]]|$)|every[ -]+TRUE|recast[ -]+checks?' \
    -e 'the two surfaces can disagree in that state.*fourth[ -]+auditor.*claim admit.*still refuses'
}

# two valid, distinct, evidence-citing PASS verdicts for the current work id
pass_two() {
  record_pair
  w=$(./crucible workid it)
  ./crucible run it j1 -- sh -c 'echo j1 ran the falsifier' >/dev/null
  ./crucible run it j2 -- sh -c 'echo j2 re-derived independently' >/dev/null
  e1=$(ls items/it/evidence | grep '^j1\.' | head -1)
  e2=$(ls items/it/evidence | grep '^j2\.' | head -1)
  printf 'VERDICT: PASS\nWORK-ID: %s\nfalsifier run, see %s\n' "$w" "$e1" > items/it/verdicts/j1.md
  printf 'VERDICT: PASS\nWORK-ID: %s\nindependent, see %s\n'   "$w" "$e2" > items/it/verdicts/j2.md
}

[ "$FAST" = 1 ] || printf 'this takes about three minutes; --fast skips the slow groups\n'
printf 'selftest: %s\n' "$(cd "$HERE" && cat VERSION 2>/dev/null || echo '?')"
printf 'absence and staleness\n'

d=$(fresh nowork); cd "$d"; refuses "empty work refuses"            "no work"
rm -rf items/it/work; mkdir -p items/it/work
d=$(fresh); cd "$d"; refuses "absent evidence refuses"             "no evidence"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null
w=$(./crucible workid it); f=$(ls items/it/evidence | head -1)
mv "items/it/evidence/$f" "items/it/evidence/mk.9.deadbeef0000.txt"
refuses "evidence for another work id refuses"            "stale evidence"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null
w=$(./crucible workid it); f=$(ls items/it/evidence | head -1)
printf 'x=2\n' >> items/it/work/a.py; w2=$(./crucible workid it)
mv "items/it/evidence/$f" "items/it/evidence/mk.1.$w2.txt"
refuses "renaming stale evidence to the new id refuses"   "renamed"

d=$(fresh); cd "$d"; printf 'not from the tool\n' > "items/it/evidence/mk.1.$(./crucible workid it).txt"
refuses "hand-written evidence refuses"                   "not produced by"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null
: > "items/it/evidence/mk.2.$(./crucible workid it).txt"
refuses "empty evidence refuses"                          "empty evidence"

printf '\nthe falsifier\n'
d=$(fresh nofalsifier); cd "$d"
./crucible run it mk -- true >/dev/null
refuses "unwritten falsifier refuses"                     "falsifier not written"

printf '\nverdict form\n'
d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
printf 'VERDICT: PASS trailing junk\nWORK-ID: %s\nx\n' "$(./crucible workid it)" > items/it/verdicts/j1.md
refuses "trailing text after the verdict word refuses"    "line 1 must be exactly"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
printf 'VERDICT: PASS\nno work id\nx\n' > items/it/verdicts/j1.md
refuses "missing WORK-ID line refuses"                    "line 2 must be exactly"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
sed 's/^WORK-ID: .*/WORK-ID: deadbeef0000/' items/it/verdicts/j1.md > t && mv t items/it/verdicts/j1.md
refuses "verdict for another work id refuses"             "stale"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
printf 'VERDICT: REJECT\nWORK-ID: %s\nfound a defect\n' "$(./crucible workid it)" > items/it/verdicts/j1.md
refuses "a REJECT blocks closure"                         "returned REJECT"

printf '\nindependence\n'
d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
cp items/it/verdicts/j1.md items/it/verdicts/j2.md
# Copying another judge's verdict is caught by the citation rule before the
# duplicate-body rule is reached: the copy cites evidence it did not record.
# That ordering makes the duplicate rule unreachable in practice, which is a
# stronger outcome than duplicate detection and is what is asserted here.
refuses "a copied verdict refuses (it cites another agent's evidence)" "without naming any evidence"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
w=$(./crucible workid it)
printf 'VERDICT: PASS\nWORK-ID: %s\nunknown agent\n' "$w" > items/it/verdicts/stranger.md
refuses "an unregistered name is not a judge"             "not registered"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
./crucible brief it maker mk >/dev/null
w=$(./crucible workid it); ./crucible run it mk -- sh -c 'echo maker check' >/dev/null
em=$(ls items/it/evidence | grep '^mk\.' | head -1)
printf 'VERDICT: PASS\nWORK-ID: %s\nself judged, see %s\n' "$w" "$em" > items/it/verdicts/mk.md
refuses "the maker may not judge its own work"            "may not judge"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
w=$(./crucible workid it)
printf 'VERDICT: PASS\nWORK-ID: %s\nI cite nothing I ran\n' "$w" > items/it/verdicts/j1.md
refuses "a PASS citing no evidence of its own refuses"    "without naming any evidence"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
out=$(CRUCIBLE_MIN_JUDGES=3 ./crucible check it 2>&1) && bad "judge count is not enforced" \
  || { printf '%s' "$out" | grep -q "need 3" && ok "too few judges refuses" || bad "wrong reason for too few judges"; }

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
mv items/it/verdicts/j2.md items/it/verdicts/j1b.md 2>/dev/null || true
printf 'j1b\tkiro\tm\th\techo {BRIEF}\n' >> agents.tsv
sed -i.bak 's/see .*/see it/' items/it/verdicts/j1b.md 2>/dev/null || true
out=$(CRUCIBLE_MIN_KINDS=2 ./crucible check it 2>&1) && bad "kind diversity is not enforced" \
  || { printf '%s' "$out" | grep -q "distinct" && ok "same-kind panel refuses when kinds are required" \
       || bad "wrong reason for kind refusal"; }

printf '\nsymlinks and names\n'
d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null
printf 'outside\n' > "$d/outside.txt"; ln -s "$d/outside.txt" items/it/work/link.py
refuses "a symlink in the work tree refuses"              "symlink not allowed"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
printf 'VERDICT: PASS\nWORK-ID: %s\nx\n' "$(./crucible workid it)" > "$d/elsewhere.md"
ln -sf "$d/elsewhere.md" items/it/verdicts/j1.md
refuses "a symlinked verdict refuses"                     "symlink"

printf '\nphases\n'
d=$(fresh); cd "$d"; out=$(./crucible phase it BUILD 2>&1) && bad "phase BUILD without TASKS.md was allowed" \
  || { printf '%s' "$out" | grep -q "requires TASKS.md" && ok "advancing to BUILD without TASKS.md refuses" \
       || bad "wrong reason for phase refusal"; }

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
sed 's/^PHASE: .*/PHASE: GRADUATE/' items/it/ITEM.md > t && mv t items/it/ITEM.md
refuses "hand-edited phase still requires its artifacts"  "phase GRADUATE requires"

printf '\nacceptance and closure\n'
d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
accepts "a complete item is closeable"

./crucible close it "a selftest lesson" >/dev/null 2>&1 \
  && grep -q 'a selftest lesson' LESSONS.md \
  && ok "close records exactly one lesson" || bad "close did not record the lesson"

./crucible close it "again" >/dev/null 2>&1 && bad "closing twice was allowed" \
  || ok "closing an already closed item refuses"

d=$(fresh); cd "$d"; ./crucible run it mk -- true >/dev/null; pass_two
./crucible close it "$(printf 'one\ntwo')" >/dev/null 2>&1 && bad "a multi-line lesson was accepted" \
  || ok "a multi-line lesson refuses"

printf '\ngit mode\n'
d=$(fresh); cd "$d"; R="$d/repo"; mkdir -p "$R"
( cd "$R" && git init -q -b main && echo a > f && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm i )
./crucible target it "$R" ai/x main >/dev/null
refuses "a target branch that does not exist refuses"     "does not exist"

( cd "$R" && git checkout -q -b ai/x && echo b >> f && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm w )
w=$(./crucible workid it)
[ "$w" != EMPTY ] && [ "$w" != NOBRANCH ] && ok "work id comes from the branch tree" \
  || bad "work id in git mode is $w"
pass_two
sed 's/^PHASE: .*/PHASE: BUILD/' items/it/ITEM.md > t && mv t items/it/ITEM.md
printf 'd\n' > items/it/DESIGN.md; printf 't\n' > items/it/TASKS.md
accepts "git-mode item with two judges is closeable"
( cd "$R" && echo sneak >> f && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm sneak )
refuses "a commit after the verdicts voids them"          "stale"


printf '\nadopt into a target repo\n'
cd "$HERE"
tr=$(mktemp -d "$SELFTEST_TMP/adopt.XXXXXX")/target; mkdir -p "$tr"
( cd "$tr" && git init -q -b main && echo code > src.py && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm i ) >/dev/null
( cd "$tr" && "$C" adopt prog >/dev/null 2>&1 )
[ -x "$tr/.crucible/prog/crucible" ] && ok "adopt installs a runnable program into the repo" \
  || bad "adopt did not install a runnable program"
rr=$(cd "$tr" && pwd -P)
grep -q "^repo: $rr" "$tr/.crucible/prog/PROGRAM" 2>/dev/null \
  && ok "adopt records the target repo it was run in" || bad "adopt did not record the repo"
( cd "$tr" && ./.crucible/prog/crucible add thing "t" >/dev/null 2>&1 )
grep -q "^branch: ai/thing" "$tr/.crucible/prog/items/thing/TARGET" 2>/dev/null \
  && ok "a new item auto-targets the enclosing repo" || bad "item did not auto-target the repo"
( cd "$tr" && git check-ignore -q .crucible/prog/agents.tsv ) \
  && ok "agents.tsv is gitignored inside the target repo" || bad "agents.tsv is not gitignored"
( cd "$tr" && ./.crucible/prog/crucible adopt prog >/dev/null 2>&1 ) \
  && bad "adopting an existing program name was allowed" \
  || ok "adopting an existing program name refuses"
out=$(cd "$(mktemp -d "$SELFTEST_TMP/norepo.XXXXXX")" && "$C" adopt x 2>&1) && bad "adopt outside a git repo was allowed" \
  || { printf '%s' "$out" | grep -q 'not inside a git repository' \
       && ok "adopt outside a git repository refuses" || bad "wrong reason outside a repo"; }


printf '\nouter loop: claims, scout, triage\n'
cd "$HERE"
ct=$(mktemp -d "$SELFTEST_TMP/outer.XXXXXX")/repo; mkdir -p "$ct"
( cd "$ct" && git init -q -b main && echo c > s.py && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm i ) >/dev/null
( cd "$ct" && "$C" adopt p >/dev/null 2>&1 )
K="$ct/.crucible/p/crucible"
printf 'a1\tkindA\tm\th\techo {BRIEF}\na2\tkindB\tm\th\techo {BRIEF}\n' > "$ct/.crucible/p/agents.tsv"
( cd "$ct" && "$K" claim add "a finding" "the exact sentence" >/dev/null )
( cd "$ct" && "$K" claim add "no source given" "" >/dev/null 2>&1 ) \
  && bad "a claim without its source sentence was accepted" \
  || ok "a claim without its source sentence refuses"
out=$(cd "$ct" && "$K" triage 2>&1) && bad "triage recommended on an unaudited claim" \
  || { printf '%s' "$out" | grep -q 'unaudited' && ok "triage refuses to recommend on unaudited claims" \
       || bad "triage refused for the wrong reason"; }
( cd "$ct" && "$K" run-claim C1 a1 -- true >/dev/null; "$K" run-claim C1 a2 -- true >/dev/null
  "$K" claim verdict C1 a1 FALSE >/dev/null; "$K" claim verdict C1 a2 FALSE >/dev/null )
( cd "$ct" && "$K" triage 2>&1 | grep -q 'DROP — auditors found it untrue' ) \
  && ok "triage drops a claim the auditors falsified" || bad "triage did not drop a falsified claim"
( cd "$ct" && "$K" claim admit C1 x >/dev/null 2>&1 ) \
  && bad "a falsified claim was admitted as work" || ok "a falsified claim cannot become work"
( cd "$ct" && "$K" claim add "second" "another sentence" >/dev/null
  "$K" run-claim C2 a1 -- true >/dev/null; "$K" run-claim C2 a2 -- true >/dev/null
  "$K" claim verdict C2 a1 TRUE >/dev/null; "$K" claim verdict C2 a2 TRUE >/dev/null
  "$K" run-claim C2 a1 -- sh -c 'echo searched' >/dev/null
  "$K" claim scout C2 FULLY-EXISTS a1 >/dev/null )
( cd "$ct" && "$K" triage 2>&1 | grep -q 'already implemented' ) \
  && ok "triage drops work the scout found already implemented" || bad "triage ignored the scout"
( cd "$ct" && "$K" claim scout C2 MAYBE a1 >/dev/null 2>&1 ) \
  && bad "an invalid scout result was accepted" || ok "an invalid scout result refuses"
( cd "$ct" && "$K" claim verdict C2 a1 MAYBE >/dev/null 2>&1 ) \
  && bad "an invalid claim verdict was accepted" || ok "an invalid claim verdict refuses"
[ -f "$ct/.crucible/p/BOOTSTRAP.md" ] \
  && [ -f "$ct/.crucible/p/docs/whats-new.md" ] \
  && grep -Eq '\]\(docs/whats-new\.md\)' "$ct/.crucible/p/START.md" \
  && ok "adopt ships BOOTSTRAP.md and a START.md link to docs/whats-new.md into the program" \
  || bad "adopt did not ship BOOTSTRAP.md with a START.md link to docs/whats-new.md"


printf '\nlive views\n'
cd "$HERE"
[ -x ./scripts/watch.sh ] && ok "watch.sh is executable" || bad "watch.sh is not executable"
/bin/sh -n ./scripts/watch.sh && ok "watch.sh parses under /bin/sh" || bad "watch.sh is not POSIX sh"
( cd "$ct" && sh .crucible/p/scripts/watch.sh nonsense --once >/dev/null 2>&1 ) \
  && bad "watch.sh accepted an unknown view" || ok "watch.sh refuses an unknown view"
for v in gate verdicts evidence tail git workids memory; do
  ( cd "$ct" && sh .crucible/p/scripts/watch.sh "$v" --once >/dev/null 2>&1 ) \
    || { bad "watch.sh view $v failed"; continue; }
done
ok "every watch.sh view renders without error"
( cd "$ct" && sh .crucible/p/scripts/watch.sh workids --once 2>/dev/null | grep -q 'work id per item' ) \
  && ok "the workids view names what it shows" || bad "workids view rendered nothing recognisable"
[ -f "$ct/.crucible/p/scripts/watch.sh" ] && ok "adopt ships the live views into the program" \
  || bad "adopt did not ship watch.sh"
[ ! -e "$ct/.crucible/p/scripts/package-release.sh" ] \
  && [ ! -e "$ct/.crucible/p/scripts/verify-package.sh" ] \
  && ok "adopt excludes release-maintainer scripts" \
  || bad "adopt copied release-maintainer scripts into the target repository"


printf '\npane overlay\n'
cd "$HERE"
out=$(cd "$ct" && env -u TMUX ./.crucible/p/crucible panes 2>&1) && bad "panes ran outside tmux" \
  || { printf '%s' "$out" | grep -q 'not inside tmux' && ok "panes refuses outside tmux" \
       || bad "panes refused outside tmux for the wrong reason"; }
if [ "$FAST" = 1 ]; then
  printf '  (--fast: tmux overlay assertions skipped)\n'
elif command -v tmux >/dev/null 2>&1; then
  tmux kill-session -t crucible-selftest 2>/dev/null || true
  tmux new-session -d -s crucible-selftest -c "$ct" -x 200 -y 50 2>/dev/null
  tmux split-window -h -d -t crucible-selftest -c "$ct" 2>/dev/null
  tmux split-window -h -d -t crucible-selftest -c "$ct" 2>/dev/null
  # Launch the probe as the pane process. Sending keystrokes raced interactive shell startup on
  # machines whose zsh startup resets the pane title, leaving three untouched `zsh` panes and
  # testing the terminal fixture rather than cmd_panes.
  tmux respawn-pane -k -t crucible-selftest.0 \
    "cd '$ct' && ./.crucible/p/crucible panes gate workids; exec /bin/sh"
  # Wait for the LAST expected view, not the first title. cmd_panes titles the operator pane
  # before it overlays anything, so waiting for "agent" exited while the views were still
  # being set up and this assertion failed about one run in three.
  titles=""
  w=0; while [ "$w" -lt 40 ]; do
    titles=$(tmux list-panes -t crucible-selftest -F '#{pane_title}' 2>/dev/null | tr '\n' ' ')
    case $titles in *agent*gate*workids*|*agent*workids*gate*) break ;; esac
    w=$((w+1)); sleep 1
  done
  case $titles in *agent*) ok "panes titles the operator pane 'agent' and leaves it alone" ;;
    *) bad "panes did not title the operator pane (saw: $titles)" ;; esac
  case $titles in *gate*workids*|*workids*gate*) ok "panes overlays the requested views onto existing panes" ;;
    *) bad "panes did not overlay the requested views (saw: $titles)" ;; esac
  body=$(tmux capture-pane -p -t crucible-selftest.1 2>/dev/null | head -6)
  case $body in *'──'*) ok "an overlaid pane is rendering a view" ;;
    *) bad "an overlaid pane rendered nothing" ;; esac
  out=$(cd "$ct" && tmux send-keys -t crucible-selftest.0 "true" C-m 2>&1; \
        TMUX=x ./.crucible/p/crucible panes nonsense 2>&1) && bad "panes accepted an unknown view" \
    || { printf '%s' "$out" | grep -q 'unknown view' && ok "panes refuses an unknown view" \
         || bad "panes refused an unknown view for the wrong reason"; }
  tmux kill-session -t crucible-selftest 2>/dev/null || true
else
  ok "tmux absent, pane overlay assertions skipped"
fi


# An adopted program contains the gate, the roles and the rules, but not the engine
# repository's own documentation. Those assertions only apply when running from the
# engine checkout, and skipping them is why an adopted program can verify itself.
if [ -f "$HERE/README.md" ] && [ -f "$HERE/VERSION" ]; then
printf '\ndocs match the script\n'
cd "$HERE"
# Only look inside fenced code blocks. An earlier version of this check scanned all
# prose and read the words "crucible checkout" as a verb named checkout — the same
# proxy failure the rules warn about: enumerate the property, not something near it.
#
# The accept side was the same proxy failure one level down, and stayed one for two releases.
# The property is "this word is a verb the engine dispatches". Both earlier forms tested
# something near it — whether the word appears in some help text — and neither form was the
# property:
#
#   pre-1.6.5  `help | grep -q "crucible $v"`  accepted 4 of the 30 real verbs. It was already
#              red on the shipped tree: `attempt` appears in a START.md block and is real, and
#              the operator help does not spell "crucible attempt". Never a working baseline.
#   1.6.5      both listings, `grep -q "\b$v\b"`  accepted any of the 197 distinct words in the
#              two help texts — 168 of them not verbs. `crucible isolation`, `ladder`,
#              `preferred`, `supporting` and `compatibility` all passed silently, because those
#              words occur in help prose.
#
# `brief`, corrected. The 1.6.6 candidate's comment here said the 1.6.5 form "FALSELY REJECTED
# `brief`". It did not, and no such rejection was ever observed: the accept side only ever sees
# the words the extractor below emits, and that extractor emitted two words on this tree —
# `attempt` and `cycle` — so `brief` was never presented to the accept side at all, under either
# form. Writing an observation that did not happen is the failure this release exists to stop,
# and it does not get an exception in this file. What is true about `brief` is a different fact
# and still worth recording: it is dispatched (`brief)` sits in the case table, `./crucible
# brief` answers "need a slug") and it appears in neither `help` nor `help protocol`, so nothing
# in this suite asserts help completeness for it. That is a gap in the help text.
#
# Enumerate it. The engine's trailing `case ${1:-help} in` dispatch table IS the set of verbs;
# nothing else decides what the script accepts. Read that table and match exactly. Note it is
# the accept-set that is authoritative here, not help: a verb missing from help is a help
# defect, and `brief` above is exactly that defect — asserting help membership on this line
# would report it as a documentation error in whichever code block happened to name it.
#
# `tr '|' '\n'` splits the `a|b)` alternation forms. The `^ *[a-z]` anchor is deliberate: it
# keeps `-h|--help)` and the `*)` catch-all out, since those are flags and a refusal, not verbs,
# and the doc extractor below can never emit a leading `-` or `*` anyway.
dispatch_verbs=$(awk '/^case .*\$\{?1/,/^esac/' ./crucible 2>/dev/null \
  | grep -oE '^ *[a-z][a-z|-]*\)' | tr -d ' )' | tr '|' '\n' | sort -u)
n_dispatch=$(printf '%s\n' "$dispatch_verbs" | grep -c '[a-z]' || true)
# Fail closed. An extractor that silently matches nothing turns this assertion into "every doc
# verb is in the empty set", which agrees with any documentation at all — the exact shape of
# failure the two forms above had. If the table cannot be read, that is a defect in this check
# and it must say so rather than pass. 30 verbs ship today; 20 is a floor, not a target.
if [ "$n_dispatch" -lt 20 ]; then
  bad "cannot enumerate the engine's verb dispatch table (extracted $n_dispatch, expected at least 20): refusing to check doc verbs against a set this check failed to build"
else
# Hyphens included, and this was a real hole: the old class `[a-z][a-z]*` read `crucible
# run-claim` as the verb `run`, which is also real, so `run-claim` passed by accident and was
# never once tested as itself. Same for `plan-audit`, `contract-audit`, `probe-acp`.
#
# THE DOCUMENTS. The hyphen repair was inert on its own, and measuring it is what showed why:
# the extractor's glob was `*.md`, and on this tree that emitted exactly two words — `attempt`
# and `cycle` — against an accept-set of thirty. `docs/` was never opened, so docs/install.md and
# docs/managed-lifecycle.md were not read at all, and no root code block spells `crucible
# run-claim`, so "run-claim is finally tested as itself" was true of the pattern and false of the
# run. An exact 30-verb accept-set that only ever sees 2 verbs is decoration.
#
# The set is what `adopt_install_engine` copies, because that is what a downstream agent is
# handed: the root documents it names — BOOTSTRAP.md, START.md, RULES.md, LOOP.md, CONFIGURE.md —
# plus top-level `docs/*.md`. Top-level only, matching the function's `"$src"/docs/*.md`; it does
# not recurse, so docs/superpowers/ and docs/problems/ are out of scope here as they are out of
# scope there. Root `*.md` is kept whole rather than narrowed to adopt's five: README.md and
# CHANGELOG.md do not travel, but they were already in scope on the line this replaces, and
# removing files an assertion already read is a narrowing whatever the justification.
doc_set=""
for f in *.md docs/*.md; do
  [ -f "$f" ] && doc_set="$doc_set $f"
done
# THE INVOCATION FORMS, and this is the other half of why two words came out. docs/
# managed-lifecycle.md line 12 defines `CP=.crucible/<program>/crucible` and the walkthrough then
# writes `$CP attempt list` for the rest of the document, so a pattern that only knows the
# literal word `crucible` reads almost none of what the docs actually teach.
#
# The variable names are read out of the documents, not listed here. Any `NAME=…/crucible`
# assignment inside a fenced block declares NAME to be the engine, and `$NAME` and `${NAME}` are
# then substituted before extraction. A hardcoded `CP` would be the same class of defect one
# level down: the walkthrough renames its variable, this line stops seeing the walkthrough, and
# it goes quiet in the direction that passes. The value's last path component must be exactly
# `crucible`, so `X=$HOME/src/crucible-notes` does not declare anything.
#
# Four forms are therefore recognised: `crucible VERB`, `<any-path>/crucible VERB`, `$NAME VERB`,
# and `$NAME/crucible VERB`. The character before the invocation must be whitespace, `/`, or one
# of the shell openers `( ` | ; &` — that last class is not decoration either, it is what admits
# `D=$($CP dispatch C1 claim-auditor a1)`, three lines in START.md that a whitespace-only left
# context silently dropped.
#
# `FNR==1 {fence=0}` resets the fence state per file. awk carries variables across the file list,
# so one document with an odd number of ``` lines would invert the in-fence/out-of-fence sense
# for every document after it, and the check would then read prose and skip code.
engine_vars=$(awk '
  FNR==1 { fence=0 }
  /^```/ { fence=!fence; next }
  !fence { next }
  {
    a=$0; sub(/^[[:space:]]*/, "", a)
    if (a ~ /^[A-Za-z_][A-Za-z0-9_]*=([^[:space:]]*\/)?crucible([[:space:]]|$)/)
      print substr(a, 1, index(a, "=") - 1)
  }' $doc_set 2>/dev/null | sort -u | tr '\n' ' ')
# Every hit carries its file and line, so a violation is reported where its owner can act on it
# rather than as a bare word.
verb_hits=$(awk -v vars="$engine_vars" '
  BEGIN {
    n = split(vars, V, " "); for (i = 1; i <= n; i++) EV[V[i]] = 1
    # Built as a string, not a /…/ literal: the left-context class contains a `/`, and how a
    # `\/` inside a bracket expression is lexed differs between awks. A string has no such
    # ambiguity. The class is the shell positions an invocation can start in — whitespace, a
    # path separator, and the openers `( ` | ; &`.
    inv = "[[:space:]/(`|;&]crucible[[:space:]]+[a-z][a-z-]*"
  }
  FNR==1 { fence=0 }
  /^```/ { fence=!fence; next }
  !fence { next }
  {
    rest = $0; out = ""
    while (match(rest, /[$][{]?[A-Za-z_][A-Za-z0-9_]*[}]?/)) {
      ref = substr(rest, RSTART, RLENGTH)
      out = out substr(rest, 1, RSTART - 1)
      rest = substr(rest, RSTART + RLENGTH)
      name = ref; gsub(/[${}]/, "", name)
      out = out ((name in EV) ? "crucible" : ref)
    }
    rest = " " out rest
    while (match(rest, inv)) {
      tok = substr(rest, RSTART, RLENGTH)
      # The consumed text is replaced by one space so the next candidate on the same line still
      # has a real left context and cannot match on a word character.
      rest = " " substr(rest, RSTART + RLENGTH)
      sub(/^.*crucible[[:space:]]+/, "", tok)
      printf "%s %s:%d\n", tok, FILENAME, FNR
    }
  }' $doc_set 2>/dev/null | sort -u)
verbs=$(printf '%s\n' "$verb_hits" | awk 'NF {print $1}' | sort -u)
n_docverbs=$(printf '%s\n' "$verbs" | grep -c '[a-z]' || true)
# Fail closed on the EXTRACT side too, for the same reason the accept side does. Two verbs out of
# thirty passed a full release as a green assertion because nothing here ever looked at how much
# the extractor saw. A pattern that stops matching now says so. 23 distinct verbs come out of the
# travelling documents today; 15 is a floor, not a target, and it leaves room for a document to
# drop an example without turning this into a tripwire.
if [ "$n_docverbs" -lt 15 ]; then
  bad "the doc verb extractor emitted only $n_docverbs distinct verb(s) from $(printf '%s' "$doc_set" | wc -w | tr -d ' ') document(s), expected at least 15: the pattern no longer reads the documents, so this assertion would agree with anything"
else
# HEAD VERB ONLY, and this is a judgement, so here is the reasoning. Docs write `claim add`,
# `attempt finish`, `lifecycle enable`; the dispatch table holds only `claim`, `attempt`,
# `lifecycle`. Validating the full two-word spelling would need a second authoritative set, and
# there is no second table to read — each subverb is parsed inside its own `cmd_*` function, in
# forms that differ per command, so any enumeration of them would itself be a proxy and would
# rot the moment one function changed. The head verb is what `case ${1:-help}` decides, so the
# head verb is what this line can assert as a fact. It keeps its teeth where it matters: a
# bogus head verb such as `crucible isolation` has no dispatch entry and is refused, which is
# the failure this check exists to catch. A wrong subverb under a real head verb is out of
# scope here and is caught where it is decided — the command's own argument parsing, asserted
# by the refusal cases above.
missing=""
for v in $verbs; do
  case " $(printf '%s' "$dispatch_verbs" | tr '\n' ' ') " in
    *" $v "*) ;;
    *) missing="$missing
    $v at $(printf '%s\n' "$verb_hits" | awk -v w="$v" '$1 == w { printf "%s%s", (c++ ? " " : ""), $2 }')" ;;
  esac
done
[ -z "$missing" ] && ok "every crucible verb shown in a doc code block is dispatched by the engine ($n_dispatch verbs in the table, $n_docverbs distinct verbs read from $(printf '%s' "$doc_set" | wc -w | tr -d ' ') travelling documents)" \
  || bad "docs use verbs the script does not dispatch:$missing"
fi
fi
for f in *.md; do
  [ -s "$f" ] || bad "$f is empty"
done
ok "no documentation file is empty"
grep -q "\[$(cat VERSION)\]" CHANGELOG.md \
  && ok "the changelog's top entry matches VERSION" || bad "CHANGELOG does not mention $(cat VERSION)"

limits_missing=""
if limits_missing=$(known_limits_missing "$HERE"); then
  ok "exactly one travelling Known limits section is docs/whats-new.md and covers all nine limits"
else
  bad "travelling Known limits contract is incomplete:$limits_missing"
fi
if mechanism_prose=$(internal_mechanism_prose "$HERE"); then
  bad "CHANGELOG or travelling docs explain internal subtract/fold/every-TRUE/recast check mechanisms:$(printf '\n%s' "$mechanism_prose")"
else
  ok "CHANGELOG and travelling docs omit internal check-mechanism prose"
fi

# Mutation proof uses only isolated fixture trees. The repository documents are never edited.
limits_fixture=$(mktemp -d "$SELFTEST_TMP/known-limits.XXXXXX")
mkdir -p "$limits_fixture/docs" "$limits_fixture/roles"
printf '# Front door\n' > "$limits_fixture/README.md"
printf '# Bootstrap\n' > "$limits_fixture/BOOTSTRAP.md"
printf '# Release history\n' > "$limits_fixture/CHANGELOG.md"
cat > "$limits_fixture/docs/near-miss.md" <<'EOF'
# Legitimate distinctions

Reporting surfaces can disagree when documentation is stale. A fourth auditor may review a
separate claim, and claim admit still refuses when that claim lacks evidence.
EOF
cat > "$limits_fixture/docs/whats-new.md" <<'EOF'
# What is new

## Known limits

- brief is dispatched but absent from help.
- The doc-verb extractor has known fence and README boundaries.
- doc_set does not include roles.
- guided_min_judges deliberately has no floor once a required reviewer row exists.
- A stray claim attempt plus ACP-probe invalidation can require recovery.
- The quickstart duplicate shim remains for compatibility.
- CHANGELOG does not travel into adopted trees.
- A bad --refresh needs rollback or restore from version control.
- Single-user authorship cannot establish independent identity.
- The falsifier pair proves that two recorded runs of the named falsifier disagreed at the current work id. It does not prove that removing the mechanism is what made them disagree, and under a single user nothing in files can prove that.

## Installation

Install normally.
EOF
cat >> "$limits_fixture/BOOTSTRAP.md" <<'EOF'

```text
subtract fold every TRUE recast check
```
EOF
if known_limits_missing "$limits_fixture" >/dev/null \
  && ! internal_mechanism_prose "$limits_fixture" >/dev/null; then
  ok "the Known limits and prose predicates accept a complete isolated fixture"
else
  bad "the documentation predicates reject their complete isolated fixture"
fi

known_limits_mutation_red() {
  label=$1; pattern=$2; opposite=${3:-}
  mutant=$(mktemp -d "$SELFTEST_TMP/known-limits-mutant.XXXXXX")
  cp -R "$limits_fixture/." "$mutant"
  grep -Ev -e "$pattern" "$mutant/docs/whats-new.md" > "$mutant/docs/whats-new.tmp"
  mv "$mutant/docs/whats-new.tmp" "$mutant/docs/whats-new.md"
  accepted=""
  if known_limits_missing "$mutant" >/dev/null; then
    accepted="missing-line"
  fi
  if [ -n "$opposite" ]; then
    opposite_mutant=$(mktemp -d "$SELFTEST_TMP/known-limits-opposite.XXXXXX")
    cp -R "$limits_fixture/." "$opposite_mutant"
    awk -v pattern="$pattern" -v replacement="$opposite" '
      $0 ~ pattern { $0 = replacement }
      { print }
    ' "$opposite_mutant/docs/whats-new.md" > "$opposite_mutant/docs/whats-new.tmp"
    mv "$opposite_mutant/docs/whats-new.tmp" "$opposite_mutant/docs/whats-new.md"
    if known_limits_missing "$opposite_mutant" >/dev/null; then
      accepted="${accepted:+$accepted, }opposite-meaning"
    fi
  fi
  if [ -n "$accepted" ]; then
    bad "Known limits mutation was accepted ($accepted): $label"
  elif [ -n "$opposite" ]; then
    ok "Known limits predicate rejects fixtures missing or contradicting $label"
  else
    ok "Known limits predicate rejects a fixture missing $label"
  fi
}

known_limits_mutation_red "brief/help" '^- brief '
known_limits_mutation_red "doc-verb extractor/fences/README" '^- The doc-verb '
known_limits_mutation_red "doc_set/roles" '^- doc_set '
known_limits_mutation_red "guided_min_judges no-floor meaning" '^- guided_min_judges ' \
  '- guided_min_judges is a deliberate floor once a required reviewer row exists.'
known_limits_mutation_red "stray claim attempt and ACP-probe invalidation" '^- A stray claim '
known_limits_mutation_red "quickstart duplicate shim" '^- The quickstart '
known_limits_mutation_red "CHANGELOG not travelling into adopted trees" '^- CHANGELOG ' \
  '- CHANGELOG travels into adopted trees.'
known_limits_mutation_red "bad --refresh rollback" '^- A bad --refresh '
known_limits_mutation_red "single-user authorship" '^- Single-user '
known_limits_mutation_red "falsifier-pair/no-causation" '^- The falsifier pair proves ' \
  '- The falsifier pair proves that removing the mechanism caused the two runs to disagree.'

limits_duplicate=$(mktemp -d "$SELFTEST_TMP/known-limits-duplicate.XXXXXX")
cp -R "$limits_fixture/." "$limits_duplicate"
printf '\n## Known limits\n\nDuplicate.\n' > "$limits_duplicate/roles/duplicate.md"
known_limits_missing "$limits_duplicate" >/dev/null \
  && bad "a second travelling Known limits section was accepted" \
  || ok "the Known limits predicate rejects a duplicate section in another travelling doc"
limits_moved=$(mktemp -d "$SELFTEST_TMP/known-limits-moved.XXXXXX")
cp -R "$limits_fixture/." "$limits_moved"
grep -v '^## Known limits$' "$limits_moved/docs/whats-new.md" > "$limits_moved/docs/whats-new.tmp"
mv "$limits_moved/docs/whats-new.tmp" "$limits_moved/docs/whats-new.md"
printf '\n## Known limits\n\nMoved.\n' > "$limits_moved/roles/moved.md"
known_limits_missing "$limits_moved" >/dev/null \
  && bad "a Known limits section outside docs/whats-new.md was accepted" \
  || ok "the Known limits predicate rejects the section when it is in the wrong travelling doc"

mechanism_mutation_red() {
  label=$1; prose=$2
  mutant=$(mktemp -d "$SELFTEST_TMP/mechanism-prose.XXXXXX")
  cp -R "$limits_fixture/." "$mutant"
  printf '%s\n' "$prose" > "$mutant/docs/internal.md"
  if internal_mechanism_prose "$mutant" >/dev/null; then
    ok "the prose predicate rejects an isolated $label explanation"
  else
    bad "the prose predicate missed an isolated $label explanation"
  fi
}
mechanism_mutation_red "subtract" "The counter subtracts TRUE verdicts before admission."
mechanism_mutation_red "fold" "The gate folds transport checks into the count."
mechanism_mutation_red "every-TRUE" "The gate walks every TRUE verdict on file."
mechanism_mutation_red "recast check" "The recast check compares the current panel."
mechanism_mutation_red "fourth-auditor false divergence" "The two surfaces can disagree in that state: triage can recommend ADMIT after counting a
fourth auditor while claim admit still refuses on the broken verdict."


# The README is the operator front door. It must stay on the cycle and must not regress
# into a catalogue of agent protocol primitives.
for v in adopt claim triage next dispatch attempt result phase run check close panes; do
  grep -q "crucible $v" README.md && bad "README exposes internal protocol verb: $v" || true
done
ok "the README keeps internal protocol commands out of operator onboarding"
grep -q 'crucible cycle' README.md && ok "the README names the one cycle interface" \
  || bad "the README does not name the cycle interface"
grep -q 'BOOTSTRAP.md' README.md && ok "the README names the entry point" \
  || bad "the README does not point at BOOTSTRAP.md"
# A prose assertion count drifted four times: the README said 60, then 63, then 68, and the
# changelog claimed an adopted suite ran 57 when it ran 75. A judge caught the last one on the
# same tree that claimed counts had been removed. Each suite prints its own total; no document
# may state one as a present fact. Historical changelog entries are exempt: recording what was
# true in a past release is what a changelog is for, and rewriting history to satisfy a check
# would be worse than the drift.
bad_count=""
for f in *.md; do
  [ "$f" = CHANGELOG.md ] && continue
  grep -qE "suite is [0-9]+|[0-9]+ assertions" "$f" 2>/dev/null && bad_count="$bad_count $f"
done
# for the changelog, only the section describing the current version
cur=$(awk -v v="$(cat VERSION 2>/dev/null)" '
  $0 ~ "^## \\[" v "\\]" {p=1; next} p && /^## \[/ {exit} p {print}' CHANGELOG.md 2>/dev/null)
printf '%s' "$cur" | grep -qE "suite is [0-9]+|[0-9]+ assertions" && bad_count="$bad_count CHANGELOG.md(current)"
[ -z "$bad_count" ] && ok "no document states an assertion count as a present fact" \
  || bad "documents state an assertion count, which drifts:$bad_count"
# The CI-workflow existence assertion used to sit on this line. It now sits with the other two
# workflow assertions under the single presence guard below, so that a tree without `.github/`
# reports one skip instead of scattering two unrelated failures through this section.
./crucible help >/dev/null 2>&1 && ok "help exits zero" || bad "help did not exit zero"
./crucible definitely-not-a-verb >/dev/null 2>&1 && bad "an unknown verb succeeded" \
  || ok "an unknown verb refuses"
helpv=$(./crucible help 2>/dev/null | grep -oE '^  crucible [a-z][a-z]*' | sed 's|  crucible ||' | sort -u)
notrouted=""
for v in $helpv; do
  grep -qE "^$v\)|^$v\|" crucible || notrouted="$notrouted $v"
done
[ -z "$notrouted" ] && ok "every verb in help is explicitly routed" \
  || bad "verbs in help are not routed:$notrouted"

# A5: a cold fresh agent cycle must cross intake, investigation, proposal and approval gates.
./scripts/verify-quickstart.sh >/dev/null 2>&1 \
  && ok "cold fresh-agent cycle binds approval before planning" \
  || bad "cold fresh-agent cycle (scripts/verify-quickstart.sh) failed"
./scripts/verify-drive.sh >/dev/null 2>&1 \
  && ok "drive tick dispatches on INVESTIGATE and refuses owned-path writes" \
  || bad "drive contract (scripts/verify-drive.sh) failed"

# A6: one writer per fact (RULES.md 17), asserted positively.
#
# The first form of this check was negative: grep the workflow for a lifecycle verb on the same
# line as the binary. An adversary escaped it three ways at d0369c11e47d, each reintroducing the
# whole deleted smoke transcription with the suite green: `c=./crucible` then `$c add demo`, a
# `./crucible \` line continuation, and `g() { ./crucible "$@"; }` then `g add demo`. A fourth
# spelling is not the answer — enumerating forbidden tokens one at a time is what terminally
# stalled the previous item on this repository. Whether a shell fragment ends up executing the
# gate is not decidable by reading it, so no forbidden-text pattern can state the real property.
# This asserts what a run: block may CONTAIN instead, because a forbidden set is fixed by whoever
# is escaping it and every novelty passes, which is the wrong direction under RULES.md 3.
#
# What the pin covers, and it is narrower than "the workflow has not been edited". A block is
# accepted if it is one invocation of a file under scripts/ — a delegation, and new ones are free.
# Every other block in a spelling this classifier opens is inline verification, and what the digest
# covers is those blocks' NORMALISED text: blank lines and whole-line comments dropped, each block
# left-trimmed to its own indentation. So for those blocks a change that survives normalisation
# refuses, and a change normalisation removes does not. Both directions were measured at
# 912633494739: two spaces added to every body line of a pinned block left this check at ok, while
# two spaces added to, or removed from, one body line moved the pin and refused. Read the ok line
# as what it says and not as more. Recompute a6_pin only when you have decided that a new inline
# block belongs; the failing message prints the value to use.
#
# The five blocks pinned at d0369c11e47d predate this check, and three of them state facts this
# suite also states, which is the same RULES.md 17 defect one level up and is recorded as its own
# item rather than fixed here.
#
# The spellings this classifier opens: `run: |` and single-line `run:`, each in the standard
# `- name:` shape and in the compact `- run:` shape a step with no name uses. The compact form was
# a hole and not a choice: the first draft anchored on `^[[:blank:]]*run:`, so a step written
# `- run: |` contributed nothing to the digest and the whole transcription could be planted under
# it with the suite green.
#
# The spellings it does not open are an OPEN SET, and this comment does not enumerate them. Every
# round of this item found more of them, so what follows are examples and not an inventory: a
# block-scalar modifier after `|`, of which `run: |-` and `run: |0` were measured; a trailing
# comment after `|` (`run: | # smoke`); a quoted key (`"run": |`); a flow-mapping step
# (`- { name: x, run: "..." }`); a space before the colon (`run : |`); and a composite action
# reached by `uses:` from outside `.github/actions/`. Each of those carried a whole gate lifecycle
# past this check at exit 0 when it was measured. Assume an unlisted spelling passes until someone
# has measured it. Completeness over the spellings a step's shell can take is not asserted here and
# is not this check's to assert: it is carried by workflow-form-completeness, filed at
# .crucible/self/items/workflow-form-completeness/ITEM.md.
#
# Two neighbours of that set behave differently and are not part of it. A folded `run: >` is seen
# as a one-line body of `>` with its continuation lines dropped, so the digest moves and the check
# refuses — a false-positive hazard rather than a silent pass. And `uses:` is met by the guard
# below, which refuses the existence of `.github/actions/` and reads no composite action's shell
# wherever that action lives. `env:`, `with:` and `defaults` are audited by nothing here.
wf=.github/workflows/selftest.yml
a6_pin=c17cd4acc3c4
# Assert-if-present, and the asymmetry is the point.
#
# `.github` is `export-ignore` in .gitattributes, so no release package contains this workflow,
# and `adopt` copies `scripts/*.sh` into every target repository without carrying `.github/` there
# either. Demanding the file unconditionally therefore made two assertions fail in every tree
# except a maintainer checkout: `no CI workflow`, and `cannot check workflow/suite agreement`.
# Every adopter who ran the command the README hands them saw a red suite, exit 1, for a reason
# that was not about their repository — a false alarm in a verification script, which costs more
# than the check is worth because it teaches the reader to discount the next red run too.
#
# So absence is expected here and says nothing, exactly as an absent `tmux` says nothing about
# the pane overlay: one explicit `ok` naming why the group did not run, in place of the group.
# Presence is unchanged. Every assertion below — the existence of the workflow, the composite
# action guard, and the a6_pin inline-verification digest — is as strict inside a maintainer
# checkout as it was before, and a workflow that is present but EDITED still moves the pin and
# still refuses. Absent means skipped; changed means failed. Those are different facts and this
# guard is the only thing that separates them.
#
# The two conditions are tested separately and not with one `&&`. `scripts/verify-quickstart.sh`
# ships in the package and travels with `adopt`, so unlike the workflow its absence is never
# expected anywhere; folding it into the skip would let a genuinely missing verifier pass as an
# expected absence. It keeps its refusal.
if [ ! -f "$wf" ]; then
  ok "no $wf in this tree, so the CI-workflow assertions are skipped (.github is export-ignore and adopt does not copy it, so a package tree or an adopted program is expected not to have one; a workflow that is present but changed still fails the pin)"
elif [ ! -f scripts/verify-quickstart.sh ]; then
  bad "cannot check workflow/suite agreement: $wf is present but scripts/verify-quickstart.sh is missing"
else
  ok "CI runs the selftest"
  # A11: `uses:` is an execution surface this classifier never reads, so a local composite action can
  # carry the whole transcription while every run: block above is a clean delegation. Measured, not
  # imagined: .github/actions/smoke/action.yml holding the smoke sequence left the check at ok. Two
  # answers were available. (a) whitelist every `uses:` value — rejected: it must be iterated over
  # every occurrence including refusals-bsd's, and it still reads no shell, so it buys enumeration
  # rather than coverage. (b) refuse the existence of the surface — chosen: this repository has no
  # .github/actions/, so a fail-closed guard costs nothing until someone adds a composite action, at
  # which point it demands a deliberate decision instead of passing in silence. The exclusion, stated
  # plainly: a composite action's contents are still not read. Its existence is what refuses.
  [ -e .github/actions ] \
    && bad ".github/actions exists and this check reads run: blocks only, so a composite action's shell is unaudited: remove it or decide the exclusion deliberately" \
    || ok "no composite action exists under .github/actions/ (a composite action reached by uses: from elsewhere is not read)"
  # Classify every run: block. Whole-line comments and blank lines are dropped and each block is
  # left-trimmed to its own indentation, so explaining or reindenting a step does not force a
  # re-pin. An inline # is kept, because stripping it would corrupt shell that contains a hash.
  # Counts come out on the first line, so no temporary file is needed and nothing races.
  # Every bracket expression is a POSIX class: [ \t] is not portable across awks, and read as
  # backslash-or-t it would mis-trim any body line beginning with t, of which there is one.
  a6_awk='
    function flush() {
      if (n == 1 && body[1] ~ /^\.?\/?scripts\/[A-Za-z0-9_.-]+\.sh([[:blank:]]+-[A-Za-z-]+)*$/) {
        deleg++
        # Counted only in the job every push runs. refusals-bsd also runs the suite, but it is
        # gated to tags and workflow_dispatch, so it cannot stand in for push coverage.
        if (job == "refusals" && body[1] ~ /scripts\/selftest\.sh/) invokes++
      } else if (n > 0) {
        inl++
        for (i = 1; i <= n; i++) out = out body[i] "\n"
      }
      n = 0
    }
    /^  [A-Za-z0-9_-]+:[[:blank:]]*$/ {
      flush(); blk = 0; job = $1; sub(/:$/, "", job); next
    }
    /^[[:blank:]]*-?[[:blank:]]*run:[[:blank:]]*\|[[:blank:]]*$/ {
      flush(); blk = 1; ind = 0; next
    }
    /^[[:blank:]]*-?[[:blank:]]*run:[[:blank:]]*[^|[:space:]]/ {
      flush(); line = $0; sub(/^[[:blank:]]*-?[[:blank:]]*run:[[:blank:]]*/, "", line)
      body[++n] = line; flush(); next
    }
    blk {
      if ($0 ~ /^[[:blank:]]*$/) next
      match($0, /[^[:blank:]]/)
      if (ind == 0) ind = RSTART
      if (RSTART < ind) { flush(); blk = 0 }
      else { if (substr($0, ind, 1) != "#") body[++n] = substr($0, ind); next }
    }
    { flush(); blk = 0 }
    END { printf "%d %d %d\n", deleg, inl, invokes; printf "%s", out }
  '
  # Explicit status capture, never `|| true`: the integrator recorded that a swallowed non-zero
  # exit from the old grep would have printed the identical ok line. An empty result is a
  # refusal here, not agreement.
  a6_all=$(awk "$a6_awk" "$wf") || a6_all=""
  a6_counts=$(printf '%s\n' "$a6_all" | sed -n 1p)
  a6_bodies=$(printf '%s\n' "$a6_all" | sed 1d)
  a6_deleg=${a6_counts%% *}; a6_rest=${a6_counts#* }
  a6_inline=${a6_rest%% *}; a6_invokes=${a6_rest##* }
  if [ -z "$a6_counts" ] || { [ "$a6_deleg" = 0 ] && [ "$a6_inline" = 0 ]; }; then
    # A parser that silently matched nothing would report agreement it never checked, which is
    # this item's own defect one level up. RULES.md 3.
    bad "$wf yielded no run: block, so workflow/suite agreement was never checked"
  elif [ "$a6_invokes" = 0 ]; then
    # j1 recorded that the only other assertion about this file is that it exists, so deleting
    # the step that runs the suite leaves the suite green while CI verifies nothing. An A6 that
    # CI never reaches is theatre, so A6 asserts its own activation on the push-gated job.
    bad "$wf job refusals has no step running scripts/selftest.sh, so a push would verify nothing"
  else
    # A pin is compared across machines, so the cksum fallback h12() tolerates for a same-machine
    # work id is not tolerable here: it would refuse honest trees. Demand a real digest and refuse
    # without one rather than compare two values that cannot be equal.
    if   command -v shasum    >/dev/null 2>&1; then a6_got=$(printf '%s' "$a6_bodies" | shasum -a 256)
    elif command -v sha256sum >/dev/null 2>&1; then a6_got=$(printf '%s' "$a6_bodies" | sha256sum)
    else a6_got=""; fi
    a6_got=$(printf '%s' "$a6_got" | cut -d' ' -f1 | cut -c1-12)
    if [ -z "$a6_got" ]; then
      bad "cannot verify $wf: without shasum or sha256sum the inline-step pin cannot be computed"
    elif [ "$a6_got" = "$a6_pin" ]; then
      ok "$wf delegates to scripts/ and its $a6_inline inline verification steps match the recorded pin"
    else
      bad "$wf added or changed an inline verification step ($a6_inline inline, $a6_deleg delegating; pin is $a6_got, expected $a6_pin): a run: block must be one scripts/ invocation, or a6_pin must be updated deliberately"
    fi
  fi
fi

else
  printf '  (adopted program: engine-repo documentation assertions skipped)\n'
fi


printf '\nevery verb is callable, and concurrency is safe\n'
cd "$HERE"
# Routing is not enough. A patch once deleted seven function bodies while leaving the
# case statement intact: sh -n passed, and the only symptom was "command not found" at
# runtime for verbs no test happened to call.
broken=""
# `selftest` is asserted through the explicit recursion guard below. Calling it here starts a
# second complete suite, ignores --fast, and turns a routing assertion into a minutes-long proxy.
for v in adopt agents target next phase dispatch brief run run-claim check close workid claim triage panes help drive; do
  o=$(./crucible "$v" 2>&1 || true)
  case "$o" in *"command not found"*) broken="$broken $v" ;; esac
done
[ -z "$broken" ] && ok "every verb resolves to a defined function" \
  || bad "verbs route to missing functions:$broken"

# The evidence recorder must be safe under concurrency. It used to pick a filename by
# asking whether one was free and then taking it, so parallel runs overwrote each other.
cr=$(mkrun); cd "$cr"
i=0; while [ $i -lt 40 ]; do ( ./crucible run it mk -- sh -c "echo concurrent-$i" >/dev/null 2>&1 ) & i=$((i+1)); done
wait
files=$(ls items/it/evidence/*.txt 2>/dev/null | wc -l | tr -d ' ')
lines=$(cat items/it/evidence/*.txt 2>/dev/null | grep -c '^concurrent-' || true)
parts=$(ls items/it/evidence/.partial.* 2>/dev/null | wc -l | tr -d ' ')
[ "$files" = 40 ] && ok "40 concurrent runs produced 40 evidence files" \
  || bad "40 concurrent runs produced $files files"
[ "$lines" = 40 ] && ok "no concurrent run lost its output" || bad "only $lines of 40 outputs survived"
[ "$parts" = 0 ] && ok "no partial evidence file is left behind" || bad "$parts partials remain"

# Closing must be atomic: parallel closes must not both append a lesson.
cd "$HERE"; cc=$(mkrun); cd "$cc"
./crucible run it mk -- true >/dev/null
w=$(./crucible workid it)
./crucible run it j1 -- sh -c 'echo j1' >/dev/null; ./crucible run it j2 -- sh -c 'echo j2' >/dev/null
e1=$(ls items/it/evidence | grep '^j1\.' | head -1); e2=$(ls items/it/evidence | grep '^j2\.' | head -1)
printf 'VERDICT: PASS\nWORK-ID: %s\nsee %s\n' "$w" "$e1" > items/it/verdicts/j1.md
printf 'VERDICT: PASS\nWORK-ID: %s\nsee %s\n' "$w" "$e2" > items/it/verdicts/j2.md
for k in 1 2 3 4 5; do ( ./crucible close it "lesson $k" >/dev/null 2>&1 ) & done
wait
lz=$(grep -c 'item it' LESSONS.md 2>/dev/null || true)
[ "${lz:-0}" = 1 ] && ok "five concurrent closes wrote exactly one lesson" \
  || bad "five concurrent closes wrote $lz lessons"
cd "$HERE"

printf '\nverbs, recursion, concurrency, isolation\n'
cd "$HERE"

# Routing is not enough. A patch once deleted seven function bodies while leaving the case
# statement intact: /bin/sh -n passed and the only symptom was "command not found" at
# runtime for verbs no test happened to call. `selftest` is excluded here because invoking
# it would re-enter this suite; its own guard is asserted separately below.
broken=""
for v in adopt agents target next phase dispatch brief run run-claim check close workid claim triage panes help drive; do
  o=$(./crucible "$v" 2>&1 || true)
  case "$o" in *"command not found"*) broken="$broken $v" ;; esac
done
[ -z "$broken" ] && ok "every verb resolves to a defined function" \
  || bad "verbs route to missing functions:$broken"

# The suite exercises every verb, and one of the verbs runs the suite. Without a guard that
# is unbounded recursion: it once produced 183 processes before anyone noticed.
o=$(CRUCIBLE_IN_SELFTEST=1 ./crucible selftest 2>&1 || true)
case "$o" in *"refusing to recurse"*) ok "a nested selftest refuses to recurse" ;;
  *) bad "a nested selftest did not refuse" ;; esac

# The recorder used to choose a filename by asking whether one was free and then taking it.
# Ten is enough to expose that and cannot wedge a CI runner the way sixty could.
cr=$(mkrun); cd "$cr"
CONC=10; [ "$FAST" = 1 ] && CONC=3
i=0; while [ $i -lt $CONC ]; do ( ./crucible run it mk -- sh -c "echo concurrent-$i" >/dev/null 2>&1 ) & i=$((i+1)); done
wait
files=$(ls items/it/evidence/*.txt 2>/dev/null | wc -l | tr -d ' ')
lines=$(cat items/it/evidence/*.txt 2>/dev/null | grep -c '^concurrent-' || true)
parts=$(ls items/it/evidence/.partial.* 2>/dev/null | wc -l | tr -d ' ')
[ "$files" = "$CONC" ] && ok "$CONC concurrent runs produced $CONC evidence files" \
  || bad "$CONC concurrent runs produced $files files"
[ "$lines" = "$CONC" ] && ok "no concurrent run lost its output" || bad "only $lines of $CONC outputs survived"
[ "$parts" = 0 ] && ok "no partial evidence file is left behind" || bad "$parts partials remain"

# Closing read STATUS and then wrote it, so two callers both saw OPEN and both appended.
cd "$HERE"; cc=$(mkrun); cd "$cc"
./crucible run it mk -- true >/dev/null
w=$(./crucible workid it)
./crucible run it j1 -- sh -c 'echo j1' >/dev/null; ./crucible run it j2 -- sh -c 'echo j2' >/dev/null
e1=$(ls items/it/evidence | grep '^j1\.' | head -1); e2=$(ls items/it/evidence | grep '^j2\.' | head -1)
printf 'VERDICT: PASS\nWORK-ID: %s\nsee %s\n' "$w" "$e1" > items/it/verdicts/j1.md
printf 'VERDICT: PASS\nWORK-ID: %s\nsee %s\n' "$w" "$e2" > items/it/verdicts/j2.md
for k in 1 2 3 4; do ( ./crucible close it "lesson $k" >/dev/null 2>&1 ) & done
wait
lz=$(grep -c 'item it' LESSONS.md 2>/dev/null || true)
[ "${lz:-0}" = 1 ] && ok "four concurrent closes wrote exactly one lesson" \
  || bad "four concurrent closes wrote ${lz:-0} lessons"

# The README claimed judge-brief isolation was asserted here, and it was not: an injected
# maker report passed the whole suite. That is a documented guarantee with nothing behind it,
# which is worse than a missing feature.
cd "$HERE"; ji=$(mkrun); cd "$ji"
printf 'MAKER-REPORT-LEAK: my rationale for the approach\n' > items/it/REPORT.md
printf 'RATIONALE-LEAK: why I chose this\n' > items/it/NOTES.md
./crucible run it mk -- true >/dev/null
b=$(./crucible brief it judge j1)
if grep -qE 'MAKER-REPORT-LEAK|RATIONALE-LEAK' "$b"; then
  bad "the judge brief leaked a maker-authored report into the judge's input"
else
  ok "a maker-authored report in the item does not reach the judge brief"
fi
d2=$(./crucible dispatch it judge j2 2>/dev/null)
if grep -qE 'MAKER-REPORT-LEAK|RATIONALE-LEAK' "$d2"; then
  bad "the judge dispatch leaked a maker-authored report into the judge's input"
else
  ok "a maker-authored report in the item does not reach the judge dispatch"
fi

# claim dispatch: an auditor judges a claim, which is not an item
cd "$HERE"; ca=$(mkrun); cd "$ca"
cn=$(./crucible claim add "a finding" "the exact sentence")
./crucible dispatch "$cn" claim-auditor j1 >/dev/null 2>&1 \
  && ok "a claim-auditor can be dispatched against a claim" \
  || bad "dispatching an auditor against a claim still refuses"
./crucible run-claim "$cn" j1 -- sh -c 'echo audited' >/dev/null 2>&1 \
  && ok "an auditor can record evidence against a claim" \
  || bad "run-claim failed"
cd "$HERE"

# The README claims closure refuses when the work changes between the check and the close. Two
# guards can catch it: the before/after work-id comparator, and the evidence binding, since a
# changed work id also stales every evidence filename. Which one fires depends on whether the
# mutation lands during the evidence scan or after it, so pinning the message made this assertion
# race. It asserts the property instead: closure must refuse, and must not close at the new id.
#
# The comparator is not redundant, verified separately by mutation: with `mv "$old" "$a"`-style
# removal of the comparator, this same scenario CLOSES at the new work id. RELEASE.md documents
# that mutation as a release step, because a race cannot assert it reliably here.
cd "$HERE"; wc=$(mkrun); cd "$wc"
./crucible run it mk -- true >/dev/null
w=$(./crucible workid it)
./crucible run it j1 -- sh -c 'echo j1' >/dev/null; ./crucible run it j2 -- sh -c 'echo j2' >/dev/null
e1=$(ls items/it/evidence | grep '^j1\.' | head -1); e2=$(ls items/it/evidence | grep '^j2\.' | head -1)
printf 'VERDICT: PASS\nWORK-ID: %s\nsee %s\n' "$w" "$e1" > items/it/verdicts/j1.md
printf 'VERDICT: PASS\nWORK-ID: %s\nsee %s\n' "$w" "$e2" > items/it/verdicts/j2.md
# enough evidence that the scan spans the mutation
n=0; while [ $n -lt 40 ]; do
  cp "items/it/evidence/$e1" "items/it/evidence/mk.pad$n.$w.txt" 2>/dev/null
  sed -i.bak "s/^agent: j1/agent: mk/" "items/it/evidence/mk.pad$n.$w.txt" 2>/dev/null
  rm -f "items/it/evidence/mk.pad$n.$w.txt.bak"
  n=$((n+1))
done
( sleep 0.2; echo 'x = 2' >> items/it/work/a.py ) &
mp=$!
out=$(./crucible close it "should refuse" 2>&1 || true)
wait "$mp" 2>/dev/null || true
case $out in
  *"closed it at"*) bad "closure completed although the work changed during the check" ;;
  *refused*)        ok "closure refuses when the work changes mid-check" ;;
  *)                bad "closure neither refused nor closed: $(printf '%s' "$out" | tail -1)" ;;
esac
grep -q '^STATUS: CLOSED' items/it/ITEM.md 2>/dev/null \
  && bad "the item was marked closed despite the work changing" \
  || ok "the item is not marked closed after a mid-check change"
cd "$HERE"

# A judge found LOOP.md claiming a FULLY-EXISTS scout result blocks admission while the code
# never read the scout field, so a claim the scout had already closed could still become work.
cd "$HERE"; sc=$(mkrun); cd "$sc"
scn=$(./crucible claim add "already built" "the sentence")
./crucible run-claim "$scn" j1 -- true >/dev/null; ./crucible run-claim "$scn" j2 -- true >/dev/null
./crucible claim verdict "$scn" j1 TRUE >/dev/null
./crucible claim verdict "$scn" j2 TRUE >/dev/null
./crucible run-claim "$scn" j1 -- sh -c 'echo searched' >/dev/null
./crucible claim scout "$scn" FULLY-EXISTS j1 >/dev/null
o=$(./crucible claim admit "$scn" dupe 2>&1) && bad "a claim the scout found already implemented was admitted" \
  || { case $o in *"already implemented"*) ok "a claim the scout closed cannot become work" ;;
       *) bad "admission refused for the wrong reason: $o" ;; esac; }
./crucible claim scout "$scn" ABSENT j1 >/dev/null
./crucible claim admit "$scn" real >/dev/null 2>&1 \
  && ok "the same claim is admissible once the scout reports it absent" \
  || bad "an audited, absent claim could not be admitted"

# START.md and RULES.md referred to STATE.md and BACKLOG.md, and adopt did not create them.
cd "$HERE"; st=$(mktemp -d "$SELFTEST_TMP/state.XXXXXX")/t; mkdir -p "$st"
( cd "$st" && git init -q -b main && echo c > a.py && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm i ) >/dev/null 2>&1
( cd "$st" && "$C" adopt p >/dev/null 2>&1 )
miss=""
for f in STATE.md BACKLOG.md LESSONS.md CLAIMS.md PROGRAM agents.tsv crucible VERSION; do
  [ -e "$st/.crucible/p/$f" ] || miss="$miss $f"
done
[ -z "$miss" ] && ok "adopt creates every file the documents refer to" \
  || bad "adopt does not create:$miss"
cd "$HERE"

# Patching once inserted the same function three times. Only the last definition takes effect,
# so the earlier copies are dead code that /bin/sh -n accepts and no behaviour test notices.
dupf=$(grep -oE '^cmd_[a-z_]+\(\)' crucible | sort | uniq -d | tr '\n' ' ')
[ -z "$dupf" ] && ok "no function is defined more than once" \
  || bad "functions defined more than once:$dupf"

# README claims claim-admission enforces kind diversity, and that was unasserted: removing the
# check left the whole suite green.
cd "$HERE"; kk=$(mkrun); cd "$kk"
ck=$(./crucible claim add "kinds" "the sentence")
./crucible run-claim "$ck" j1 -- true >/dev/null; ./crucible run-claim "$ck" mk -- true >/dev/null
./crucible claim verdict "$ck" j1 TRUE >/dev/null      # j1 and mk are both kind kiro
./crucible claim verdict "$ck" mk TRUE >/dev/null
./crucible run-claim "$ck" j1 -- sh -c 'echo searched' >/dev/null
./crucible claim scout "$ck" ABSENT j1 >/dev/null
o=$(CRUCIBLE_MIN_KINDS=2 ./crucible claim admit "$ck" a 2>&1) \
  && bad "a claim audited by one kind was admitted with CRUCIBLE_MIN_KINDS=2" \
  || { case $o in *"kind"*) ok "claim admission refuses one-kind audit when two kinds are required" ;;
       *) bad "claim admission refused for the wrong reason: $o" ;; esac; }
./crucible run-claim "$ck" j2 -- true >/dev/null
./crucible claim verdict "$ck" j2 TRUE >/dev/null      # j2 is kind grok
CRUCIBLE_MIN_KINDS=2 ./crucible claim admit "$ck" a >/dev/null 2>&1 \
  && ok "the same claim is admissible once a second kind agrees" \
  || bad "a two-kind audited claim could not be admitted"

# and a missing scout report must block admission, as LOOP.md states
cd "$HERE"; ns=$(mkrun); cd "$ns"
cs=$(./crucible claim add "no scout" "the sentence")
./crucible run-claim "$cs" j1 -- true >/dev/null; ./crucible run-claim "$cs" j2 -- true >/dev/null
./crucible claim verdict "$cs" j1 TRUE >/dev/null; ./crucible claim verdict "$cs" j2 TRUE >/dev/null
o=$(./crucible claim admit "$cs" b 2>&1) && bad "a claim with no scout report was admitted" \
  || { case $o in *"no scout report"*) ok "admission refuses a claim with no scout report" ;;
       *) bad "admission refused for the wrong reason: $o" ;; esac; }
cd "$HERE"


# START.md said cwd is the repository root and then gave commands as if cwd were the program
# directory, so its documented first commands exited 127 in a real adopted repo.
sp=$(mktemp -d "$SELFTEST_TMP/startdoc.XXXXXX")/t; mkdir -p "$sp"
( cd "$sp" && git init -q -b main && echo c > a.py && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm i ) >/dev/null 2>&1
( cd "$sp" && "$C" adopt p >/dev/null 2>&1 )
if ( cd "$sp" && CP=.crucible/p/crucible && "$CP" help >/dev/null 2>&1 \
     && cat .crucible/p/STATE.md >/dev/null 2>&1 && cat .crucible/p/BACKLOG.md >/dev/null 2>&1 \
     && "$CP" next >/dev/null 2>&1 ); then
  ok "an installed program's documented commands run from the repository root"
else
  bad "the documented commands do not run from the repository root"
fi
grep -qE '^\./crucible ' START.md \
  && bad "START.md still shows a bare ./crucible, which fails from the repository root" \
  || ok "START.md uses the installed program prefix throughout"
# The same defect appeared in START, then RULES/LOOP/CONFIGURE, then roles/orchestrator.md —
# three rounds running, because the assertion enumerated filenames. It now sweeps everything
# `adopt` ships, so a new shipped file is covered the moment it exists. README and BOOTSTRAP are
# read from the engine checkout, where ./crucible is correct, so they are exempt.
badinv=""
for f in $(ls *.md roles/*.md 2>/dev/null); do
  case $f in README.md|BOOTSTRAP.md|CHANGELOG.md|RELEASE.md|CONTRIBUTING.md|SECURITY.md) continue ;; esac
  grep -qE '(^|[^a-zA-Z0-9_/])\./crucible ' "$f" && badinv="$badinv $f"
done
[ -z "$badinv" ] && ok "nothing adopt ships invokes ./crucible, which is invalid at the repo root" \
  || bad "shipped files invoke ./crucible:$badinv"
# and the same property must hold after a real adopt, not just in the engine checkout
ap=$(mktemp -d "$SELFTEST_TMP/adopted.XXXXXX")/t; mkdir -p "$ap"
( cd "$ap" && git init -q -b main && echo c > a.py && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm i ) >/dev/null 2>&1
( cd "$ap" && "$C" adopt p >/dev/null 2>&1 )
shipbad=""
for f in $(ls "$ap"/.crucible/p/*.md "$ap"/.crucible/p/roles/*.md 2>/dev/null); do
  case ${f##*/} in README.md|BOOTSTRAP.md) continue ;; esac
  grep -qE '(^|[^a-zA-Z0-9_/])\./crucible ' "$f" && shipbad="$shipbad ${f##*/}"
done
[ -z "$shipbad" ] && ok "an adopted program ships no repo-root-invalid invocation" \
  || bad "an adopted program ships repo-root-invalid invocations:$shipbad"
# LOOP is behavioral documentation; protocol recorder commands belong in the internal guide.
grep -qE 'crucible (run|run-claim|attempt|result|phase|dispatch)' LOOP.md \
  && bad "LOOP.md exposes low-level protocol commands" \
  || ok "LOOP.md stays at the behavioral loop level"


printf '\nthe documents that travel\n'
cd "$HERE"
# Both assertions below read an installed program, not this checkout, so the set of travelling
# documents is not a list in this file: it is whatever adopt_install_engine copied. README.md and
# CHANGELOG.md do not travel, they are therefore absent from TP, and they cannot count as
# coverage here — which is the point, since both findings this section closes were invisible from
# the checkout and visible only in what actually ships.
TP="$ap/.crucible/p"
travel=""
for f in "$TP"/*.md "$TP"/roles/*.md "$TP"/docs/*.md; do
  [ -f "$f" ] && travel="$travel $f"
done

# The cold-start audit's top finding. `crucible help protocol` prints
# `claim add|list|verdict|scout|admit` and `triage`, and not one of those spellings appeared in
# any document adopt copies, so an agent that read only what it was handed could not perform the
# INVESTIGATE phase at all. The documents are written now; this is what stops the next release
# from undoing them.
#
# The verbs are read out of the help text at runtime. A list written into this file would be
# correct exactly until the next verb is added, and then it would rot silently in the direction
# that passes — the failure mode RULES.md 3 names.
#
# A spelling is the verb plus its literal sub-words: `claim` followed by `add|list|…` yields
# `claim`, `claim add` … `claim admit`. A slash separates sibling verbs, so `run / run-claim` is
# two. A token that is not a bare lowercase word ends the spelling, which is how `ATTEMPT`,
# `[ABSENT|EXISTS|DEFECT]`, `--like` and `…` are excluded without naming any of them. Matching is
# word-bounded rather than substring, so `add` must appear as the word add and is not satisfied
# by "address"; the multi-word spellings are the load-bearing ones and cannot be met by accident.
#
# Deliberately undocumented, in one place, each with its reason. None of these is a step in the
# protocol, so shipping an agent a document that teaches it would be teaching the wrong thing:
#   panes             tmux overlay for a human watching a run, not an action an agent takes
#   selftest          engine verification, run by a maintainer or by CI, never by an agent
#   workid            read-only derived value the gate computes for itself before every check
#   lifecycle status  read-only variant of `lifecycle enable`, which docs/managed-lifecycle.md
#                     does document; the setup step travels, the inspector does not
# Adding a name here is a decision and must be argued, not a way to make this assertion green.
# The assertion after it refuses any name that `help protocol` no longer prints, so the list
# cannot decay into a blanket exemption that hides a verb nobody documents any more.
protocol_exempt='panes
selftest
workid
lifecycle status'
vfile="$SELFTEST_TMP/protocol-verbs.txt"
./crucible help protocol 2>/dev/null | awk '
  /^  [a-z]/ {
    spec = substr($0, 3)
    sub(/  +.*$/, "", spec)                    # drop the description column
    n = split(spec, alt, / *\/ */)             # a slash separates sibling verbs
    for (a = 1; a <= n; a++) {
      m = split(alt[a], tok, / +/)
      chain = ""
      for (t = 1; t <= m; t++) {
        w = tok[t]
        if (w !~ /^[a-z][a-z|-]*$/) break      # placeholder, option or ellipsis: spelling ends
        k = split(w, opt, /\|/)
        for (o = 1; o <= k; o++) print (chain == "" ? opt[o] : chain " " opt[o])
        if (k > 1) break                       # alternatives are leaves
        chain = (chain == "" ? w : chain " " w)
      }
    }
  }' | sort -u > "$vfile"
nverbs=$(awk 'END {print NR}' "$vfile" 2>/dev/null || echo 0)
undocumented=""
# Read from the file, never from a pipe: a `while` on the right of a pipe runs in a subshell in
# every POSIX shell, and the accumulated list would be discarded along with it.
while IFS= read -r v; do
  [ -n "$v" ] || continue
  printf '%s\n' "$protocol_exempt" | grep -qxF "$v" && continue
  grep -qE "(^|[^A-Za-z0-9_-])$v([^A-Za-z0-9_-]|$)" $travel 2>/dev/null \
    || undocumented="$undocumented, $v"
done < "$vfile"
if [ -z "$travel" ] || [ "${nverbs:-0}" -lt 10 ]; then
  # A parser that matched nothing would print agreement it never checked. The help text names
  # well over ten spellings, so too few means the extractor broke, not that the verbs are gone.
  bad "protocol verb coverage was never checked: $nverbs verb(s) parsed from help protocol, $(printf '%s' "$travel" | wc -w | tr -d ' ') travelling document(s) found"
elif [ -n "$undocumented" ]; then
  bad "protocol verbs appear in no document adopt ships${undocumented}: document them, or record the exemption and why"
else
  ok "every protocol verb from help protocol appears in a travelling doc"
fi
# and the exemption list must not outlive the verbs it exempts
stale_exempt=""
while IFS= read -r v; do
  [ -n "$v" ] || continue
  grep -qF "$v" "$vfile" || stale_exempt="$stale_exempt, $v"
done <<EOF
$protocol_exempt
EOF
[ -z "$stale_exempt" ] && ok "every deliberately undocumented protocol verb is still a verb" \
  || bad "the undocumented-verb exemption names things help protocol no longer prints${stale_exempt}: delete them"

# The second audit finding. README.md linked CONTRIBUTING.md and RELEASE.md; both files exist in
# this repository and neither one ships, so the break was invisible from the checkout and was the
# only broken internal link in the package. Resolution therefore happens inside an installed
# program. Absolute http(s) links are out of scope on purpose: the fix for the repo-only links
# was to convert them to absolute GitHub URLs, and re-checking them here would undo that.
#
# Scoped to `[text](target)` and to nothing else. `scripts/acp-brief.py` is named in five
# travelling documents and deliberately does not ship — it is an operator-written adapter and
# CONFIGURE.md says so — but it is named in prose and backticks, never as a link, so link syntax
# excludes it without an exemption. A path exemption broad enough to cover it would also let a
# genuinely broken link through, which is the finding itself.
deadlinks=""; nlinks=0
for f in $travel; do
  dir=${f%/*}
  for t in $(grep -oE '\[[^][]*\]\([^()[:space:]]+\)' "$f" 2>/dev/null | sed 's/^.*](//; s/)$//'); do
    case $t in http://*|https://*|mailto:*) continue ;; esac
    t=${t%%#*}                                  # a fragment is not part of the file name
    [ -n "$t" ] || continue                     # a bare #anchor targets no file
    nlinks=$((nlinks+1))
    case $t in /*) p=$t ;; *) p="$dir/$t" ;; esac
    [ -e "$p" ] || deadlinks="$deadlinks, ${f#"$TP"/} -> $t"
  done
done
if [ "$nlinks" -eq 0 ]; then
  bad "no markdown link was found in any travelling document, so nothing was resolved"
elif [ -n "$deadlinks" ]; then
  bad "travelling documents link to files the package does not ship${deadlinks}"
else
  ok "every relative markdown link in a travelling doc resolves to a shipped file"
fi


printf '\nscratch directories do not leak\n'
cd "$HERE"
# 1,642 directories and 1.2 GB of abandoned scratch trees, every one of them from a script that
# called mktemp and never removed the result. Stated over the scripts rather than over the one
# that leaked most: a file that creates a temp tree must arm a trap that removes it. There is no
# exception list — verify-quickstart.sh is a five-line shim that ends in exec and creates no temp
# tree, so the mktemp test excludes it by itself, which is what an exception list would have
# hidden the next time a shim grew a mktemp.
notrapped=""; scanned=0
for f in scripts/verify-*.sh scripts/selftest.sh; do
  [ -f "$f" ] || continue
  scanned=$((scanned+1))
  grep -q 'mktemp' "$f" || continue
  grep -qE "^[[:blank:]]*trap[[:blank:]].*rm[[:blank:]]+-[rf]" "$f" \
    || notrapped="$notrapped ${f##*/}"
done
if [ "$scanned" -lt 2 ]; then
  bad "temp-directory cleanup was never checked: $scanned script(s) scanned under scripts/"
elif [ -n "$notrapped" ]; then
  bad "these scripts create a temp directory and never remove it:$notrapped"
else
  ok "every verify script that makes a temp dir removes it in a trap"
fi
cd "$HERE"

# Absence of verdicts must refuse on its own, with nothing else wrong. Every earlier case had
# another failure present, so a mutation that permitted zero judges left the suite green — a
# judge proved that by making exactly that mutation. This case is isolated: work exists, the
# falsifier is written, evidence is valid and bound, and only the verdicts are missing.
cd "$HERE"; zv=$(mkrun); cd "$zv"
./crucible run it mk -- sh -c 'echo a real check' >/dev/null
zo=$(./crucible check it 2>&1) && bad "an item with no verdicts at all was CLOSEABLE" \
  || { case $zo in
         *"0 passing judges"*) ok "zero verdicts refuses on its own, with nothing else wrong" ;;
         *) bad "zero verdicts refused for another reason: $(printf '%s' "$zo" | grep FAIL | head -1)" ;;
       esac; }
# and with only one of the two required verdicts present, still refuse for the count alone
w=$(./crucible workid it)
./crucible run it j1 -- sh -c 'echo j1 ran it' >/dev/null
e1=$(ls items/it/evidence | grep '^j1\.' | head -1)
printf 'VERDICT: PASS\nWORK-ID: %s\nsee %s\n' "$w" "$e1" > items/it/verdicts/j1.md
zo=$(./crucible check it 2>&1) && bad "one verdict satisfied a two-judge requirement" \
  || { case $zo in
         *"1 passing judges, need 2"*) ok "one verdict refuses when two are required" ;;
         *) bad "one verdict refused for another reason: $(printf '%s' "$zo" | grep FAIL | head -1)" ;;
       esac; }
cd "$HERE"

# A generated contract must contain commands the callee can run verbatim from the working
# directory the documents put it in. The self path was hardcoded to ./crucible, which does not
# exist at the repository root, so every contract handed the agent a 127. A judge caught it.
cd "$HERE"; gp=$(mktemp -d "$SELFTEST_TMP/contract.XXXXXX")/t; mkdir -p "$gp"
( cd "$gp" && git init -q -b main && echo c > a.py && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm i ) >/dev/null 2>&1
( cd "$gp" && "$C" adopt p >/dev/null 2>&1 )
printf 'a1\tkindA\tm\th\techo x\na2\tkindB\tm\th\techo x\nmk\tkindA\tm\th\techo x\n' > "$gp/.crucible/p/agents.tsv"
if ( cd "$gp"
     cn=$(.crucible/p/crucible claim add "a finding" "the sentence")
     .crucible/p/crucible dispatch "$cn" claim-auditor a1 >/dev/null 2>&1
     cmd=$(grep -m1 -oE '[^ ]*crucible run-claim [^ ]* [^ ]*' \
             ".crucible/p/claims/$cn/dispatches/1-claim-auditor-a1.md")
     [ -n "$cmd" ] || exit 1
     $cmd -- echo verbatim >/dev/null 2>&1 ); then
  ok "a generated claim contract's recorder command runs verbatim from the repo root"
else
  bad "a generated claim contract's recorder command does not run from the repo root"
fi
if ( cd "$gp"
     .crucible/p/crucible add thing "t" >/dev/null 2>&1
     sed 's|^TEMPLATE-FALSIFIER-UNWRITTEN.*|Undo it.|' .crucible/p/items/thing/ITEM.md > i.t \
       && mv i.t .crucible/p/items/thing/ITEM.md
     printf '\n```\nsh -c '"'"'test -f mechanism'"'"'\n```\n' >> .crucible/p/items/thing/ITEM.md
     d=$(.crucible/p/crucible dispatch thing maker mk 2>/dev/null)
     cmd=$(grep -m1 -oE '[^ ]*crucible run thing mk' "$d")
     [ -n "$cmd" ] || exit 1
     $cmd -- echo verbatim >/dev/null 2>&1 ); then
  ok "a generated maker contract's recorder command runs verbatim from the repo root"
else
  bad "a generated maker contract's recorder command does not run from the repo root"
fi
cd "$HERE"

# A round's findings used to vanish when the next round's verdict landed on the same filename,
# so the defect history was not reproducible even by the author. Dispatching a judge onto changed
# work now archives its superseded verdict under the work id it actually judged.
cd "$HERE"; av=$(mkrun); cd "$av"
./crucible run it mk -- true >/dev/null
aw=$(./crucible workid it)
printf 'VERDICT: REJECT\nWORK-ID: %s\nARCHIVE-MARKER a finding from the first round\n' "$aw" \
  > items/it/verdicts/j1.md
echo 'x = 2' >> items/it/work/a.py
./crucible dispatch it judge j1 >/dev/null 2>&1
if [ -f "items/it/verdicts/$aw-j1.md" ] || ls items/it/verdicts/history/*j1*.md >/dev/null 2>&1; then
  grep -qh 'ARCHIVE-MARKER' items/it/verdicts/history/*.md 2>/dev/null \
    && ok "a superseded verdict is archived under the work id it judged" \
    || bad "the superseded verdict was archived but its findings are not readable"
else
  bad "a superseded verdict was lost instead of archived"
fi
[ -f items/it/verdicts/j1.md ] && bad "the superseded verdict is still live" \
  || ok "the superseded verdict is no longer counted as current"
# and the archive must not be mistaken for a verdict
./crucible run it j1 -- sh -c 'echo j1' >/dev/null; ./crucible run it j2 -- sh -c 'echo j2' >/dev/null
rm -f "$(ls items/it/evidence/mk.*."$aw".txt 2>/dev/null | head -1)"
aw2=$(./crucible workid it)
ae1=$(ls items/it/evidence | grep '^j1\.' | tail -1); ae2=$(ls items/it/evidence | grep '^j2\.' | tail -1)
printf 'VERDICT: PASS\nWORK-ID: %s\nsee %s\n' "$aw2" "$ae1" > items/it/verdicts/j1.md
printf 'VERDICT: PASS\nWORK-ID: %s\nsee %s\n' "$aw2" "$ae2" > items/it/verdicts/j2.md
o=$(./crucible check it 2>&1 || true)
case $o in
  *"only NAME.md belongs"*) bad "the gate treats the verdict archive as a malformed verdict" ;;
  *) ok "the gate ignores the verdict archive" ;;
esac
cd "$HERE"
/bin/sh -n ./crucible && ok "gate parses under /bin/sh" || bad "gate is not POSIX sh"
/bin/sh -n ./scripts/selftest.sh && ok "selftest parses under /bin/sh" || bad "selftest is not POSIX sh"
/bin/sh -n ./scripts/verify-quickstart.sh && ok "verify-quickstart parses under /bin/sh" \
  || bad "verify-quickstart is not POSIX sh"
# The pattern is assembled at runtime so this file does not contain the literal string it
# searches for. It is deliberately structural — absolute home paths and machine-specific user
# directories — rather than a list of project names: naming other projects would itself be a
# reference to something outside this repository, and a list only ever catches what is on it.
hp="/$(printf 'U')sers/|/$(printf 'h')ome/[a-z]|~/$(printf 'D')esktop|~/$(printf 'D')ocuments|~/$(printf 'D')ownloads"
git ls-files 2>/dev/null | grep -v '^scripts/selftest.sh$' | while read -r f; do
  grep -lE "$hp" "$f" 2>/dev/null
done | grep -q . && bad "a tracked file references a machine-specific path" \
  || ok "no tracked file references a machine-specific path"
# and the check must itself be falsifiable: plant a violation and require it to fire
pv=$(mktemp -d "$SELFTEST_TMP/plant.XXXXXX")/plant; mkdir -p "$pv"; ( cd "$pv" && git init -q -b main
  printf 'see ~/%sesktop/crucible for details\n' "$(printf 'D')" > doc.md && git add -A
  git -c user.email=s@s -c user.name=s commit -qm i ) >/dev/null 2>&1
( cd "$pv" && git ls-files | while read -r f; do grep -lE "$hp" "$f" 2>/dev/null; done | grep -q . ) \
  && ok "the self-containment check fires on a planted machine-specific path" \
  || bad "the self-containment check does not catch a planted violation"

printf '\nclaims are bound to recorded work\n'
cd "$HERE"; cb=$(mkrun); cd "$cb"
c1=$(./crucible claim add "first" "s one"); c2=$(./crucible claim add "second" "s two")
./crucible claim verdict "$c1" j1 TRUE >/dev/null 2>&1 \
  && bad "a claim verdict was accepted with no evidence recorded by that agent" \
  || ok "a claim verdict with no recorded evidence refuses"
./crucible run-claim "$c1" j1 -- true >/dev/null || true
./crucible claim verdict "$c1" j1 TRUE >/dev/null 2>&1 \
  && ok "the same verdict is accepted once its author recorded a check" \
  || bad "a backed claim verdict was refused"
./crucible claim scout "$c1" ABSENT >/dev/null 2>&1 \
  && bad "a scout result was accepted without naming the scout" \
  || ok "a scout result must name the agent that searched"
./crucible claim scout "$c1" ABSENT j2 >/dev/null 2>&1 \
  && bad "a scout result was accepted from an agent that recorded no searches" \
  || ok "a scout result refuses when its author recorded no searches"
mkdir -p "claims/$c1/verdicts" "claims/$c1/evidence"
printf 'CLAIM-VERDICT: TRUE\nAGENT: ghost\nKIND: invented\n' > "claims/$c1/verdicts/ghost.md"
# Give the forged agent evidence as well, so this isolates the registration check rather than
# being masked by the evidence check. Both guards must be individually load-bearing.
printf 'crucible-run/1\nagent: ghost\nclaim: %s\nwhen: now\ncommand: invented\n--- output ---\nx\n--- exit 0 ---\n' \
  "$c1" > "claims/$c1/evidence/ghost.p0s0.txt"
./crucible claim scout "$c1" ABSENT j1 >/dev/null 2>&1 || true
o=$(CRUCIBLE_MIN_KINDS=2 ./crucible claim admit "$c1" x 2>&1) \
  && bad "a forged unregistered verdict counted toward admission" \
  || { case $o in *"need 2"*) ok "an unregistered verdict file does not count toward admission" ;;
       *) bad "admission refused for another reason" ;; esac; }
./crucible run-claim "$c1" j2 -- true >/dev/null || true
./crucible claim verdict "$c1" j2 TRUE >/dev/null || true
CRUCIBLE_MIN_KINDS=2 ./crucible claim admit "$c1" one >/dev/null 2>&1 || true
i2=$(sed -n "/^### $c2 /,\$ {s/^    item: //p;}" CLAIMS.md | head -1)
[ -z "$i2" ] && ok "admitting one claim leaves the others unadmitted" \
  || bad "admitting one claim marked another as item '$i2'"
cd "$HERE"

printf '\nfalsifier, maker, and commit identity\n'
fm=$(mkrun); cd "$fm"
./crucible run it mk -- true >/dev/null || true
cp items/it/ITEM.md items/it/ITEM.keep
awk '/^## Falsifier/{exit} {print}' items/it/ITEM.md > i.t && mv i.t items/it/ITEM.md
./crucible check it 2>&1 | grep -q 'no ## Falsifier section' \
  && ok "deleting the falsifier section refuses" || bad "a missing falsifier section was accepted"
printf '\n## Falsifier\n\nTODO\n' >> items/it/ITEM.md
./crucible check it 2>&1 | grep -q 'empty or too short' \
  && ok "an empty falsifier refuses" || bad "an empty falsifier was accepted"
mv items/it/ITEM.keep items/it/ITEM.md
rm -f items/it/MAKER
./crucible check it 2>&1 | grep -q 'no maker recorded' \
  && ok "an item with no recorded maker refuses" || bad "a missing maker was accepted"
cd "$HERE"; ec=$(mkrun); cd "$ec"
er="$ec/r"; mkdir -p "$er"
( cd "$er" && git init -q -b main && echo a > f && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm i ) >/dev/null 2>&1
./crucible target it "$er" ai/x main >/dev/null 2>&1 || true
( cd "$er" && git checkout -q -b ai/x )
b1=$(./crucible workid it)
( cd "$er" && git -c user.email=s@s -c user.name=s commit -q --allow-empty -m sneak ) >/dev/null 2>&1
b2=$(./crucible workid it)
[ "$b1" != "$b2" ] && ok "an empty commit changes the work id and voids prior verdicts" \
  || bad "an empty commit left the work id unchanged, so verdicts survived a commit"
cd "$HERE"

# ---------------------------------------------------------------------------
# Falsifier-pair contract (red against the pre-change engine; green after T2).
# Each assertion names a covered mechanism from the item catalogue. Against an
# engine that has not yet landed the pair gate / envelope / recorder, these
# fail by design — that is the intentional red interval between T1 and T2.
# ---------------------------------------------------------------------------
printf '\nfalsifier-pair contract\n'

# M29 — RULES.md rule 6 is a CHECK once the pair is the gate's subject.
grep -q '^6\. \*\*CHECK — Closure needs a falsifier' "$HERE/RULES.md" \
  && ok "RULES.md rule 6 is labelled CHECK" \
  || bad "RULES.md rule 6 is not labelled CHECK"

# M30 — travelling A7-ENFORCED bytes are byte-identical across copies.
# Canonical tracked copy is docs/whats-new.md (one writer for the shipped sentence).
a7_line1='Closure refuses unless the item'\''s falsifier was recorded by `crucible run` at the current work id'
fence_copies=0
canon_file=""
for f in \
  "$HERE/docs/whats-new.md" \
  "$HERE/RULES.md" \
  "$HERE/roles/maker.md" \
  "$HERE/roles/judge.md" \
  "$HERE/roles/specifier.md" \
  "$HERE/roles/contract-auditor.md" \
  "$HERE/docs/managed-lifecycle.md"
do
  [ -f "$f" ] || continue
  if grep -qxF "$a7_line1" "$f"; then
    # Pull the three-line paragraph starting at the fence's first line.
    para=$(awk -v s="$a7_line1" '
      $0 == s { print; getline; print; getline; print; exit }
    ' "$f")
    if [ -z "$canon_file" ]; then
      canon_file=$(mktemp "$SELFTEST_TMP/a7e.XXXXXX")
      printf '%s\n' "$para" > "$canon_file"
      fence_copies=1
    else
      para_file=$(mktemp "$SELFTEST_TMP/a7e2.XXXXXX")
      printf '%s\n' "$para" > "$para_file"
      if cmp -s "$canon_file" "$para_file"; then
        fence_copies=$((fence_copies + 1))
      else
        say "A7-ENFORCED drift in $f"
      fi
    fi
  fi
done
[ "$fence_copies" -ge 3 ] \
  && ok "travelling A7-ENFORCED text is byte-identical in at least three documents" \
  || bad "travelling A7-ENFORCED text is missing or drifted (found $fence_copies copies)"

# M31 — Known limits carries the A7-LIMIT bullet (already gated by known_limits_missing).
known_limits_has "$(known_limits_section "$HERE/docs/whats-new.md")" 'falsifier pair proves' \
  && ok "docs/whats-new.md Known limits names the falsifier-pair/no-causation limit" \
  || bad "docs/whats-new.md Known limits lacks the falsifier-pair/no-causation limit"

# M32 — no travelling document claims the pair proves causation.
causal=0
for f in $(travelling_doc_files "$HERE"); do
  grep -Eqi 'falsifier pair proves that removing|pair proves that .* caused|proves that removing the mechanism is what' "$f" \
    && causal=$((causal + 1)) && say "causal claim in $f"
done
[ "$causal" -eq 0 ] \
  && ok "no travelling document claims the falsifier pair proves causation" \
  || bad "a travelling document claims the falsifier pair proves causation"

# M33 — record_pair helper is present and records both directions by name.
grep -q '^record_pair() {' "$HERE/scripts/selftest.sh" \
  && grep -q -- '--falsifier restored' "$HERE/scripts/selftest.sh" \
  && ok "the suite record_pair helper records both directions" \
  || bad "the suite record_pair helper is missing a direction"

# Helpers the built engine must expose (red until T2 lands them).
grep -q '^evidence_envelope() {' "$HERE/crucible" && ok "crucible defines evidence_envelope" || bad "crucible does not define evidence_envelope"
grep -q '^evidence_exit() {' "$HERE/crucible" && ok "crucible defines evidence_exit" || bad "crucible does not define evidence_exit"
grep -q '^falsifier_command() {' "$HERE/crucible" && ok "crucible defines falsifier_command" || bad "crucible does not define falsifier_command"
grep -q '^falsifier_argv_frame() {' "$HERE/crucible" && ok "crucible defines falsifier_argv_frame" || bad "crucible does not define falsifier_argv_frame"
grep -q 'no falsifier run pair' "$HERE/crucible" && ok "engine names the no falsifier run pair stem" || bad "engine does not name the no falsifier run pair stem"

# K1 / K2 — trailer-read assertions against evidence_exit extracted from the engine.
# Red until T2 writes the helper; then these fixtures discriminate the two mechanisms.
if grep -q '^evidence_exit() {' "$HERE/crucible"; then
  ht=$(mktemp -d "$SELFTEST_TMP/ht.XXXXXX")
  {
    printf '#!/bin/sh\nVERBOSE=1\n'
    sed -n '49,54p' "$HERE/scripts/selftest.sh"
    printf '\n'
    # Extract the function body as shipped.
    awk '
      /^evidence_exit\(\) \{/ { print; inb=1; next }
      inb { print; if ($0 == "}") exit }
    ' "$HERE/crucible"
    cat <<'HT'
F=$(mktemp -d "${TMPDIR:-/tmp}/ht.XXXXXX")
trap 'rm -rf "$F"' 0
printf 'crucible-run/1\nagent: mk\n--- output ---\nhello\n--- exit 0 ---\n'  > "$F/normal"
printf 'crucible-run/1\nagent: mk\n--- output ---\nhello\n--- exit 7 ---\n'  > "$F/nonzero"
printf -- '--- exit 9 ---\nmiddle\n--- exit 0 ---\n'                         > "$F/firstline"
printf 'crucible-run/1\n--- output ---\n--- exit 5 ------ exit 0 ---\n'      > "$F/joined"
printf 'crucible-run/1\n--- output ---\ndone.--- exit 3 ---\n'               > "$F/joinedplain"
[ "$(evidence_exit "$F/normal")" = 0 ] && ok 'the trailer read returns the recorded zero status' || bad 'the trailer read lost a recorded zero status'
[ "$(evidence_exit "$F/nonzero")" = 7 ] && ok 'the trailer read returns a recorded nonzero status' || bad 'the trailer read lost a recorded nonzero status'
[ "$(evidence_exit "$F/firstline")" = 0 ] && ok 'the trailer read takes the last physical line, not the first' || bad 'the trailer read took a trailer-shaped first line instead of the last physical line'
[ "$(evidence_exit "$F/joined")" = 0 ] && ok 'the trailer read finds a trailer joined to another trailer' || bad 'the trailer read could not read a trailer joined to another trailer'
[ "$(evidence_exit "$F/joinedplain")" = 3 ] && ok 'the trailer read finds a trailer joined to plain output bytes' || bad 'the trailer read could not read a trailer joined to plain output bytes'
printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ] || exit 1
HT
  } > "$ht/h.sh"
  chmod +x "$ht/h.sh"
  ht_out=$(PASS=0 FAIL=0 FAILED= sh "$ht/h.sh" 2>&1) && st=0 || st=$?
  printf '%s\n' "$ht_out" | grep -q 'the trailer read returns the recorded zero status' \
    || printf '%s\n' "$ht_out" | grep -q 'ok.*trailer read returns the recorded zero' \
    || true
  if [ "$st" -eq 0 ]; then
    ok "evidence_exit trailer-read fixtures hold (K1/K2)"
  else
    bad "evidence_exit trailer-read fixtures failed"
    say "$ht_out"
  fi
else
  bad "evidence_exit trailer-read fixtures cannot run — helper missing"
fi

# Gate / recorder / envelope contract fixtures (each red until T2).
fp=$(mkrun); cd "$fp"
./crucible run it mk -- true >/dev/null
# Absence of a pair must refuse with the unique stem once the gate lands (M24 / A1).
out=$(./crucible check it 2>&1) && st=0 || st=$?
case $out in
  *'falsifier run pair'*) ok "no-pair check names the falsifier run pair stem" ;;
  *) bad "no-pair check does not name the falsifier run pair stem" ; say "$out" ;;
esac
# Forged header lines in ordinary recordings must not manufacture a pair (M1 envelope).
./crucible run it mk -- sh -c 'printf "falsifier: removed\nfalsifier-argv: 1 4:true\n"; exit 1' >/dev/null
./crucible run it mk -- sh -c 'printf "falsifier: restored\nfalsifier-argv: 1 4:true\n"; exit 0' >/dev/null
pass_two
out=$(./crucible check it 2>&1) && st=0 || st=$?
case $out in
  *CLOSEABLE*) bad "forged falsifier headers in ordinary output manufactured a closeable pair" ;;
  *'falsifier run pair'*) ok "forged falsifier headers in ordinary output do not count as a pair" ;;
  *) bad "forged-header fixture refused for an unexpected reason" ; say "$out" ;;
esac
cd "$HERE"

# Recorder: --falsifier direction validation (M12, M13).
fp=$(mkrun); cd "$fp"
out=$(./crucible run it mk --falsifier bogus -- true 2>&1) && st=0 || st=$?
case $out in
  *'--falsifier direction must be removed or restored'*) ok "bogus --falsifier direction refuses" ;;
  *) bad "bogus --falsifier direction was not refused as specified" ; say "$out" ;;
esac
out=$(./crucible run it mk --falsifier -- true 2>&1) && st=0 || st=$?
case $out in
  *'--falsifier needs a direction'*) ok "missing --falsifier direction refuses" ;;
  *) bad "missing --falsifier direction was not refused as specified" ; say "$out" ;;
esac
# Labelled recording writes both envelope fields (M10) when the engine lands.
rm -f mechanism
if ./crucible run it mk --falsifier removed -- sh -c 'test -f mechanism' >/dev/null 2>&1; then
  f=$(ls items/it/evidence/mk.* | head -1)
  grep -q '^falsifier: removed$' "$f" \
    && grep -q '^falsifier-argv: ' "$f" \
    && ok "a labelled removed run writes both falsifier header fields" \
    || bad "a labelled removed run did not write both falsifier header fields"
else
  bad "labelled --falsifier removed run was refused before the pair could be recorded"
fi
cd "$HERE"

# Polarity / disagreement clauses (M25–M27) — require a built gate; red until then.
fp=$(mkrun); cd "$fp"
# Attempt an inverted pair by hand-labelling if the recorder exists; otherwise expect refusal stem.
if ./crucible run it mk --falsifier restored -- true >/dev/null 2>&1 \
   && ./crucible run it mk --falsifier removed -- false >/dev/null 2>&1; then
  pass_two
  # Force both directions present but wrong polarity by swapping marker discipline:
  # restored recorded with true (exit 0) and removed with false is already inverted vs the rule
  # (removed must be nonzero). Check must name inversion.
  out=$(./crucible check it 2>&1) && st=0 || st=$?
  case $out in
    *'falsifier run pair is inverted'*) ok "an inverted falsifier pair refuses" ;;
    *'falsifier run pair did not disagree'*) ok "a non-discriminating falsifier pair refuses" ;;
    *'falsifier run pair'*) ok "a bad-polarity falsifier pair refuses under the pair stem" ;;
    *CLOSEABLE*) bad "a bad-polarity falsifier pair was accepted" ;;
    *) bad "a bad-polarity pair refused for an unexpected reason" ; say "$out" ;;
  esac
else
  bad "labelled polarity fixtures cannot run — recorder missing --falsifier"
fi
cd "$HERE"

printf '\n\n%s passed, %s failed\n' "$PASS" "$FAIL"
[ -n "$FAILED" ] && printf '%s\n' "$FAILED"
[ "$FAIL" -eq 0 ] || exit 1
printf 'every documented refusal was asserted and held.\n'
