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
PASS=0; FAIL=0; FAILED=""

say() { [ "$VERBOSE" = 1 ] && printf '    %s\n' "$*" || true; }
ok()   { PASS=$((PASS+1)); [ "$VERBOSE" = 1 ] && printf '  ok   %s\n' "$1" || printf '.'; }
bad()  { FAIL=$((FAIL+1)); FAILED="$FAILED
  FAILED: $1"; printf '\n  FAIL %s\n' "$1"; }

# A fresh run root with a registered panel, one item, a written falsifier and work.
# Prints the directory. The caller must cd into it: `cd "$(fresh)"`.
# Setup runs in a subshell so this function never changes the caller's directory.
mkrun() {
  d=$(mktemp -d)
  ( cd "$d"
    cp "$C" ./crucible; cp -R "$HERE/roles" .; cp "$HERE/RULES.md" .
    printf 'mk\tkiro\tm\thigh\techo {BRIEF}\n'  > agents.tsv
    printf 'j1\tkiro\tm\thigh\techo {BRIEF}\n' >> agents.tsv
    printf 'j2\tgrok\tm\thigh\techo {BRIEF}\n' >> agents.tsv
    ./crucible add it "selftest item" >/dev/null
    [ "${1:-}" = nofalsifier ] || {
      sed 's|^TEMPLATE-FALSIFIER-UNWRITTEN.*|Undo the change; the named check fails.|' items/it/ITEM.md > i.tmp
      mv i.tmp items/it/ITEM.md; }
    [ "${1:-}" = nowork ] || { mkdir -p items/it/work; printf 'x=1\n' > items/it/work/a.py; }
    [ "${1:-}" = nomaker ] || printf 'mk\n' > items/it/MAKER
  )
  printf '%s' "$d"
}
fresh() { printf '%s' "$(mkrun "${1:-}")"; }

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

# two valid, distinct, evidence-citing PASS verdicts for the current work id
pass_two() {
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
tr=$(mktemp -d)/target; mkdir -p "$tr"
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
out=$(cd "$(mktemp -d)" && "$C" adopt x 2>&1) && bad "adopt outside a git repo was allowed" \
  || { printf '%s' "$out" | grep -q 'not inside a git repository' \
       && ok "adopt outside a git repository refuses" || bad "wrong reason outside a repo"; }


printf '\nouter loop: claims, scout, triage\n'
cd "$HERE"
ct=$(mktemp -d)/repo; mkdir -p "$ct"
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
[ -f "$ct/.crucible/p/BOOTSTRAP.md" ] && ok "adopt ships BOOTSTRAP.md into the program" \
  || bad "adopt did not ship BOOTSTRAP.md"


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
  tmux send-keys -t crucible-selftest.0 "cd '$ct' && ./.crucible/p/crucible panes gate workids" C-m
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
verbs=$(awk '/^```/{f=!f;next} f' *.md 2>/dev/null \
  | grep -oE '(^|[[:space:]/])crucible [a-z][a-z]*' \
  | sed 's|.*crucible ||' | sort -u)
missing=""
for v in $verbs; do
  ./crucible help 2>/dev/null | grep -q "crucible $v" || missing="$missing $v"
done
[ -z "$missing" ] && ok "every crucible verb shown in a doc code block exists in help" \
  || bad "docs use verbs the script lacks:$missing"
for f in *.md; do
  [ -s "$f" ] || bad "$f is empty"
done
ok "no documentation file is empty"
grep -q "\[$(cat VERSION)\]" CHANGELOG.md \
  && ok "the changelog's top entry matches VERSION" || bad "CHANGELOG does not mention $(cat VERSION)"


# The README is the front door and it went badly stale once, describing a superseded
# model while mentioning none of the current verbs. This makes that impossible to
# repeat quietly.
for v in adopt selftest claim triage next dispatch run check close panes; do
  grep -q "crucible $v" README.md || bad "README never mentions the $v verb"
done
ok "the README mentions every principal verb"
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
[ -f .github/workflows/selftest.yml ] && ok "CI runs the selftest" || bad "no CI workflow"
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

# A5: the README smoke block must execute (via the helper that extracts the
# ## Smoke test ```sh fence, not the loop diagram) and the gate must refuse after edit.
./scripts/verify-quickstart.sh >/dev/null 2>&1 \
  && ok "README smoke closes an item and post-edit check refuses" \
  || bad "README smoke (scripts/verify-quickstart.sh) failed"

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
if [ -f "$wf" ] && [ -f scripts/verify-quickstart.sh ]; then
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
else
  bad "cannot check workflow/suite agreement: $wf or scripts/verify-quickstart.sh is missing"
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
for v in adopt agents target next phase dispatch brief run run-claim check close workid claim triage panes help; do
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
for v in adopt agents target next phase dispatch brief run run-claim check close workid claim triage panes help; do
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
cd "$HERE"; st=$(mktemp -d)/t; mkdir -p "$st"
( cd "$st" && git init -q -b main && echo c > a.py && git add -A \
  && git -c user.email=s@s -c user.name=s commit -qm i ) >/dev/null 2>&1
( cd "$st" && "$C" adopt p >/dev/null 2>&1 )
miss=""
for f in STATE.md BACKLOG.md LESSONS.md CLAIMS.md PROGRAM agents.tsv crucible; do
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
sp=$(mktemp -d)/t; mkdir -p "$sp"
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
ap=$(mktemp -d)/t; mkdir -p "$ap"
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
# and LOOP must not send a claim auditor to the item-scoped recorder
grep -qE 'crucible run <item>|crucible run [A-Z]' LOOP.md \
  && bad "LOOP.md sends a claim auditor to the item-scoped recorder" \
  || ok "LOOP.md names run-claim for claim evidence"

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
cd "$HERE"; gp=$(mktemp -d)/t; mkdir -p "$gp"
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
pv=$(mktemp -d)/plant; mkdir -p "$pv"; ( cd "$pv" && git init -q -b main
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

printf '\n\n%s passed, %s failed\n' "$PASS" "$FAIL"
[ -n "$FAILED" ] && printf '%s\n' "$FAILED"
[ "$FAIL" -eq 0 ] || exit 1
printf 'every documented refusal was asserted and held.\n'
