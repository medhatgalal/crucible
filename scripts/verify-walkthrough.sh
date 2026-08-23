#!/bin/sh
# The documented walkthrough must EXECUTE. This suite runs the path the travelling documents
# print and fails when a document and the engine disagree.
#
# Why it exists. Three consecutive releases tried to fix "an agent cannot get from install to
# done on the documentation alone" by rewriting prose, and each time an audit of the published
# tarball found the new walkthrough was still not executable. 1.6.4's INVESTIGATE sequence
# shipped a `PANEL.ASSIGN.tsv` template whose own casting could not satisfy the admit bar four
# documents stated, omitted `plan-audit` entirely, and dead-ended at the first `maker result`
# because no document established the `ai/*` work branch. None of that is visible to a prose
# reviewer; all of it is visible to a run. So "the documented walkthrough works" is a CHECK here
# and not a sentence anywhere.
#
# Fixture-only. Every agent is an `echo` command, nothing reaches the network, and no real agent
# CLI is invoked. One managed+guided program carries W1, W2, W3 and W5, because standing a
# program up costs about ten seconds and this suite has to stay runnable on every push. W4 gets a
# second repository and program, and the reasons are documented engine behaviour rather than
# convenience — they are spelled out at the W4 section. Two adoptions total, not one per
# assertion.
#
# ---------------------------------------------------------------------------------------------
# WHAT IS EXTRACTED FROM THE DOCUMENTS, AND WHAT IS NOT
#
# This is what decides whether the suite is worth having, so it is stated rather than left to be
# inferred. Anything hand-written here is a place this check cannot catch drift.
#
# EXTRACTED and executed as printed:
#   * the `PANEL.ASSIGN.tsv` template in START.md. Read out of its fenced block, installed as the
#     panel, and cast against. Hand-writing an "equivalent" panel is the exact substitution that
#     let 1.6.4 ship a template that could not admit, so it is the one thing this suite must
#     never restate.
#   * every `sh` fenced block of START.md's "The exact INVESTIGATE sequence" and "The exact
#     EXECUTE sequence". Blocks are located by a verb signature (`doc_block`), never by ordinal
#     position, then evaluated line by line in THIS shell — so the variables the document's own
#     plumbing assigns (`D`, `A`, `DS`, `AS`) carry across lines exactly as they would for a
#     reader pasting the block, and a doc that changes a verb, a flag, an argument order, or a
#     step's shell plumbing changes what runs here.
#   * the refusal texts START.md prints in its ```text blocks. Those are compared against the
#     refusal the engine actually emits, whitespace-squeezed, with the attempt id and the program
#     path normalised back to the `<id>` / `<program>` the document writes.
#
# FILLED BY THE FIXTURE, because the document deliberately writes a hole. Exactly three shapes
# are filled and everything outside them runs as printed:
#   1. `<placeholder>` metavariables — `<slug>`, `<program>`, `<base>`, `<maker>`, `<auditor>`,
#      `<contract-auditor>`, `<bounded-check>`, `<evidence-basename>`, `<observed-pid>`,
#      `<search command>`, `<item-slug>`;
#   2. ALL-CAPS metavariables — `"CLAIM"`, `"EXACT SOURCE SENTENCE"`;
#   3. `[ALTERNATION]` choices — `[ABSENT|EXISTS|DEFECT]`.
#   The table is `doc_fill` below.
#
# HAND-WRITTEN, and why:
#   * `agents.tsv`. Machine-local by design, gitignored, and no document prints a runnable
#     registry. The agent NAMES are chosen to match the names START.md's own examples use
#     (`a1`, `a2`, `sc1`, `ca1`, `mk1`, `j1`), which is what lets the extracted blocks run
#     without renaming anything inside them.
#   * `PANEL.md` prose. Only its six headings are machine-checked; the body is not a sequence.
#   * the `ITEM.md` body. `claim admit` seeds a template and `ready` refuses an unfinished one;
#     no document prints a filled-in example, so the fixture writes one. Drift in the `ready`
#     contract surfaces here as a refusal naming the missing section.
#   * the maker's commit. START.md prints it as the prose comment
#     `# ... maker changes only the item's Owned files, then commits ...`, which is not a
#     command. The fixture commits where that comment sits.
#   * the second auditor's agent name. START.md hard-codes `a1` and then instructs "run steps 2
#     to 5 once per claim-auditor row, with a different agent each time", so the ` a1` token is
#     rewritten to ` a2` on the replay. That substitution is the document's own instruction.
#   * the three panel mutations W2 needs (one claim-auditor row; three rows). They are edits OF
#     the extracted template, not replacements for it.
#   * every documentation assertion (the `doc_says` calls). Those ask whether a document SAYS
#     something, so the pattern has to be written here. Each failure names the file and what it
#     must say, so a future author can act without re-deriving the defect.
#
# ---------------------------------------------------------------------------------------------
# THE PROPERTIES
#   W1  the panel template START.md prints can actually admit a claim. If it cannot, this fails.
#       That is the assertion that would have caught the 1.6.4 defect.
#   W2  the admit bar the engine enforces is the bar the documents state — in both places that
#       enforce it, with the floor the panel count does not show.
#   W3  every gate the documented EXECUTE path hits is documented, and the path reaches a maker
#       `result` writing `OUTCOME: PASS`.
#   W4  the NOBRANCH dead end is reproducible, and the way out is written down.
#   W5  the stranded-dispatch recovery the engine names in its own refusal works, and the
#       walkthrough prints one runnable dispatch form per step.

set -eu

HERE=$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)
C="$HERE/crucible"
# A real tab. `\t` inside a grep BRE is a literal `t` under GNU grep, so every tab-anchored
# assertion and every generated TSV row interpolates this instead.
tab=$(printf '\t')
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); printf '.\n'; }
bad() { FAIL=$((FAIL + 1)); printf 'FAIL %s\n' "$1"; }
# The summary must be printed on every path, including the fail-fast one. A documented step that
# refuses invalidates every step after it, so this suite stops there rather than reporting a
# cascade of consequences as if they were separate defects — but a CI reader still has to see the
# count, so the exit handler prints it. The signal handlers suppress it: an interrupted run has no
# meaningful count and a printed one would read like a completed pass.
SUMMARISED=
summary() {
  [ -n "$SUMMARISED" ] && return
  SUMMARISED=1
  printf '%s passed, %s failed\n' "$PASS" "$FAIL"
}
expect() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) || { bad "$label: command refused: $out"; return; }
  printf '%s\n' "$out" | grep -q "$pattern" && ok || bad "$label: wanted $pattern, got $out"
}
refuses() {
  label=$1; pattern=$2; shift 2
  out=$("$@" 2>&1) && { bad "$label: command accepted"; return; }
  printf '%s\n' "$out" | grep -q "$pattern" && ok || bad "$label: wanted $pattern, got $out"
}

# The travelling set: what `adopt` copies into a target repository, which is the only
# documentation an agent installed by BOOTSTRAP.md can read. A rule stated only in README.md or
# CHANGELOG.md is not reachable from an installed program, so it does not count here.
TRAVELLING='START.md CONFIGURE.md BOOTSTRAP.md LOOP.md RULES.md docs/install.md docs/managed-lifecycle.md docs/drive.md'
# The five documents this release rewrote and that a reader is handed. Each must carry the admit
# bar itself; a pointer to another file is not enough when the reader is standing in this one.
WALKTHROUGH_DOCS='START.md CONFIGURE.md BOOTSTRAP.md docs/install.md docs/managed-lifecycle.md'

# Some travelling doc must say this. `want` is a fixed string, not a pattern: these are sentences
# a reader has to be able to find, and a regex would let a near-miss pass.
doc_says() {
  label=$1; want=$2; must=$3
  for f in $TRAVELLING; do
    [ -f "$HERE/$f" ] || continue
    if grep -qF -- "$want" "$HERE/$f"; then ok; return; fi
  done
  bad "$label: no travelling document contains \"$want\" — $must (checked, under $HERE: $TRAVELLING)"
}

# Every one of the five walkthrough documents must say this, in its own words being no defence.
doc_each_says() {
  label=$1; want=$2; must=$3
  missing=
  for f in $WALKTHROUGH_DOCS; do
    [ -f "$HERE/$f" ] || { missing="$missing $f(absent)"; continue; }
    grep -qF -- "$want" "$HERE/$f" || missing="$missing $f"
  done
  [ -z "$missing" ] && ok \
    || bad "$label: these documents omit \"$want\":$missing — $must"
}

# --- reading the documents -------------------------------------------------------------------

# Print the first ```sh fenced block in FILE containing a line that matches PATTERN.
doc_block() {
  awk -v want="$2" '
    /^```sh$/ { inb = 1; n = 0; hit = 0; next }
    /^```/ {
      if (inb && hit) { for (i = 1; i <= n; i++) print body[i]; exit }
      inb = 0; next
    }
    inb { body[++n] = $0; if ($0 ~ want) hit = 1 }
  ' "$1"
}

# Print the first ```text fenced block in FILE containing a line that matches PATTERN. These are
# the refusal transcripts the document promises the reader will see.
doc_text() {
  awk -v want="$2" '
    /^```text$/ { inb = 1; n = 0; hit = 0; next }
    /^```/ {
      if (inb && hit) { for (i = 1; i <= n; i++) print body[i]; exit }
      inb = 0; next
    }
    inb { body[++n] = $0; if ($0 ~ want) hit = 1 }
  ' "$1"
}

# Print the fenced block whose first line is the PANEL.ASSIGN.tsv header: the template itself.
doc_panel_template() {
  awk -v hdr="role${tab}agent${tab}required${tab}notes" '
    $0 == "```text" { inb = 1; n = 0; hit = 0; next }
    /^```/ {
      if (inb && hit) { for (i = 1; i <= n; i++) print body[i]; exit }
      inb = 0; next
    }
    inb { body[++n] = $0; if (n == 1 && $0 == hdr) hit = 1 }
  ' "$1"
}

# Cast the extracted template. Only the `…` placeholder in the agent column is replaced; the
# roles, their order, the repeated rows, and the `required` column are the document's.
panel_cast() {
  awk -F "$tab" -v OFS="$tab" '
    NR == 1 { print; next }
    $1 == "" { next }
    {
      role = $1
      if (role == "claim-auditor") { ca++; who = (ca == 1 ? "a1" : (ca == 2 ? "a2" : "a3")) }
      else if (role == "coordinator")      who = "c0"
      else if (role == "scout")            who = "sc1"
      else if (role == "maker")            who = "mk1"
      else if (role == "reviewer")         who = "j1"
      else if (role == "contract-auditor") who = "ca1"
      else                                 who = "c0"
      if ($2 == "…" || $2 == "") $2 = who
      print
    }
  '
}

# Normalise a measured refusal onto the spelling a document can print: the program's own path
# back to `.crucible/<program>/crucible`, and whitespace squeezed so a doc's line wrap does not
# count as a difference. The attempt id is normalised by the caller, which is the only part that
# varies per run.
norm() {
  sed -e "s|\\.crucible/$PROGRAM/|.crucible/<program>/|g" \
    | tr '\n' ' ' | tr -s ' '
}

# --- executing the documents -----------------------------------------------------------------

# The declared substitution table. Filling happens at the point of use, so `$EV` and the pid are
# real values by the time a line runs rather than deferred expansions.
doc_fill() {
  sed \
    -e "s|<program>|$PROGRAM|g" \
    -e "s|<item-slug>|$SLUG|g" \
    -e "s|<slug>|$SLUG|g" \
    -e "s|<base>|$BASE|g" \
    -e "s|<auditor>|j1|g" \
    -e "s|<contract-auditor>|ca1|g" \
    -e "s|<maker>|mk1|g" \
    -e "s|<observed-pid>|$OBSPID|g" \
    -e "s|<bounded-check>|sh -c 'echo bounded check ran'|g" \
    -e "s|<evidence-basename>|${EV:-UNSET}|g" \
    -e "s|<search command>|grep -rn -- --json .|g" \
    -e "s|\"CLAIM\"|\"$CLAIM_TITLE\"|g" \
    -e "s|\"EXACT SOURCE SENTENCE\"|\"$CLAIM_SOURCE\"|g" \
    -e 's#\[ABSENT|EXISTS|DEFECT\]#ABSENT#g'
}

# Evaluate a filled block in THIS shell, one line at a time. Blank and whole-line comment lines
# are skipped so a failure message points at a real command. DOC_OUT keeps the last command's
# output, which is how the evidence basename the next block needs is obtained.
DOC_OUT=
doc_run() {
  label=$1; block=$2
  if [ -z "$block" ]; then
    bad "$label: START.md prints no such block — the documented sequence cannot be extracted"
    return 1
  fi
  # A file, not a pipe: a pipeline body is a subshell, and every variable the block assigns
  # would be discarded when it ended.
  printf '%s\n' "$block" > "$tmp/block.sh"
  doc_failed=
  while IFS= read -r doc_line; do
    # A whole-line comment or a blank line is skipped so a failure message points at a real
    # command. The one comment START.md prints inside a sequence block stands in for the maker's
    # own commit, so a caller may hook it. Written as an `if` and not `[ … ] && …`: an AND-OR
    # list whose final status is non-zero aborts the suite under `set -e`, silently, which is how
    # a doc adding one more comment line could turn this suite off.
    case $doc_line in
      ''|'#'*)
        if [ -n "${DOC_ON_COMMENT:-}" ]; then "$DOC_ON_COMMENT"; fi
        continue ;;
    esac
    # Output goes through a file, not a command substitution. `DOC_OUT=$(eval …)` would run the
    # line in a subshell, and every variable the document's plumbing assigns — `D`, `A`, `DS`,
    # `AS` — would be discarded before the next line could use it.
    set +e
    eval "$doc_line" > "$tmp/doc.out" 2>&1
    doc_rc=$?
    set -e
    DOC_OUT=$(cat "$tmp/doc.out")
    if [ "$doc_rc" -ne 0 ]; then
      doc_failed="$doc_line
    -> exit $doc_rc: $DOC_OUT"
      break
    fi
  done < "$tmp/block.sh"
  rm -f "$tmp/block.sh"
  if [ -n "$doc_failed" ]; then
    bad "$label: a command START.md prints refused:
    $doc_failed"
    printf 'stopping: every later step of the documented sequence depends on this one\n'
    return 1
  fi
  ok
}

# --- fixture ---------------------------------------------------------------------------------

PROGRAM=work
SLUG=walkthrough
BASE=main
OBSPID=$$
EV=
CLAIM_TITLE='the documented walkthrough is not covered by a check'
CLAIM_SOURCE='The documented walkthrough is not covered by a check.'

write_agents() {
  prog=$1
  {
    printf 'c0%skindA%sm%shigh%secho {BRIEF}\n'  "$tab" "$tab" "$tab" "$tab"
    printf 'a1%skindA%sm%shigh%secho {BRIEF}\n'  "$tab" "$tab" "$tab" "$tab"
    printf 'a2%skindB%sm%shigh%secho {BRIEF}\n'  "$tab" "$tab" "$tab" "$tab"
    printf 'a3%skindB%sm%shigh%secho {BRIEF}\n'  "$tab" "$tab" "$tab" "$tab"
    printf 'sc1%skindB%sm%shigh%secho {BRIEF}\n' "$tab" "$tab" "$tab" "$tab"
    printf 'ca1%skindB%sm%shigh%secho {BRIEF}\n' "$tab" "$tab" "$tab" "$tab"
    printf 'mk1%skindA%sm%shigh%secho {BRIEF}\n' "$tab" "$tab" "$tab" "$tab"
    printf 'j1%skindB%sm%shigh%secho {BRIEF}\n'  "$tab" "$tab" "$tab" "$tab"
  } > "$prog/agents.tsv"
}

write_panel_md() {
  cat > "$1/PANEL.md" <<'EOF'
# Panel

## Agents

- c0, a1, a2, a3, sc1, ca1, mk1, j1 (echo fixtures; mixed kinds)

## Roles

Cast authoritatively in PANEL.ASSIGN.tsv, which is the template START.md prints.

## Risk posture

LOW for fixture walkthrough verification.

## Isolation transport

Prefer multi-agent. ACP before subagent on single-product hosts.

## Independence ladder

1. multi-agent
2. acp
3. subagent after ACP probe failure
4. stop if none invocable

## Waivers

NONE for this fixture.
EOF
}

# Install the extracted template, optionally mutated, and re-approve. Every panel edit
# content-rebinds the approval, so re-approving is part of installing one.
install_panel() {
  mutation=${1:-none}
  case $mutation in
    none) doc_panel_template "$HERE/START.md" | panel_cast > "$Q/PANEL.ASSIGN.tsv" ;;
    one-auditor)
      doc_panel_template "$HERE/START.md" | panel_cast \
        | awk -F "$tab" '$1 == "claim-auditor" { n++; if (n == 2) next } { print }' \
        > "$Q/PANEL.ASSIGN.tsv" ;;
    three-auditors)
      { doc_panel_template "$HERE/START.md" | panel_cast
        printf 'claim-auditor%sa3%syes\n' "$tab" "$tab"; } > "$Q/PANEL.ASSIGN.tsv" ;;
  esac
  "$Q/crucible" cycle approve-panel >/dev/null \
    || { bad "the panel could not be approved with mutation '$mutation' of START.md's template — the casting the document prints is not valid casting"; return 1; }
}

tmp=$(mktemp -d "${TMPDIR:-/tmp}/crucible-walkthrough.XXXXXX")
tmp=$(cd "$tmp" && pwd)
# Cleanup must never mask the exit status. The `0` handler removes and returns, so a normal run
# still reports its own result. Each signal handler removes and then exits 128+signal, because a
# handler that only removes lets the shell resume and reach a `0` exit — an interrupted or
# timed-out run would then be recorded as a pass.
trap 'summary; rm -rf "$tmp"' 0
trap 'SUMMARISED=1; rm -rf "$tmp"; exit 129' 1
trap 'SUMMARISED=1; rm -rf "$tmp"; exit 130' 2
trap 'SUMMARISED=1; rm -rf "$tmp"; exit 143' 15

repo="$tmp/target"
mkdir -p "$repo"
(
  cd "$repo"
  git init -q -b "$BASE"
  git config user.name test
  git config user.email test@example.invalid
  git config commit.gpgsign false
  printf 'baseline\n' > tracked.txt
  git add tracked.txt
  git commit -qm baseline
  "$C" adopt "$PROGRAM" --managed >/dev/null
)
Q="$repo/.crucible/$PROGRAM"
# Everything below runs with cwd = the target repository root and `CP` spelled the way START.md
# spells it, because that is the reader's position. The engine is never invoked from this
# checkout's own directory.
cd "$repo"
CP=".crucible/$PROGRAM/crucible"

# START.md defines CP itself. Assert that definition rather than trusting the line above.
cp_block=$(doc_block "$HERE/START.md" '^CP=')
printf '%s\n' "$cp_block" | grep -qF "CP=.crucible/<program>/crucible" \
  && ok || bad "START.md must define CP as .crucible/<program>/crucible, got: $cp_block"

write_agents "$Q"
write_panel_md "$Q"

panel_template=$(doc_panel_template "$HERE/START.md")
[ -n "$panel_template" ] \
  && ok || bad "START.md has no PANEL.ASSIGN.tsv template block (header row: role<TAB>agent<TAB>required<TAB>notes) — W1 cannot extract the template it exists to test"
printf '%s\n' "$panel_template" | grep -qc "^scout${tab}" >/dev/null \
  && ok || bad 'START.md: the PANEL.ASSIGN.tsv template must cast scout — no claim is admittable without a scout report'

printf '%s\n' "$CLAIM_SOURCE" > "$repo/report.md"

# =============================================================================================
# W2, first leg. The floor `cycle` and `triage` apply is 2 whatever the panel says, so the
# template mutated down to ONE claim-auditor row plus one TRUE must not get out of INVESTIGATE.
# =============================================================================================
install_panel one-auditor
"$Q/crucible" cycle problem "$repo/report.md" >/dev/null

# The claim itself, and the first auditor, straight out of START.md's INVESTIGATE sequence.
add_block=$(doc_block "$HERE/START.md" 'claim add')
dispatch_block=$(doc_block "$HERE/START.md" 'dispatch C1 claim-auditor')
seal_block=$(doc_block "$HERE/START.md" 'attempt transport')
evidence_block=$(doc_block "$HERE/START.md" 'run-claim C1 a1')
verdict_block=$(doc_block "$HERE/START.md" 'claim verdict C1 a1')
scout_block=$(doc_block "$HERE/START.md" 'claim scout C1')
triage_block=$(doc_block "$HERE/START.md" 'CP triage')
admit_block=$(doc_block "$HERE/START.md" 'claim admit C1')

doc_run 'W1: START.md claim add creates a claim' "$(printf '%s\n' "$add_block" | doc_fill)"
doc_run 'W1: START.md dispatches a claim-auditor' "$(printf '%s\n' "$dispatch_block" | doc_fill)"
doc_run 'W1: START.md seals the attempt before the worker runs' "$(printf '%s\n' "$seal_block" | doc_fill)"
doc_run 'W1: START.md records the evidence behind the verdict' "$(printf '%s\n' "$evidence_block" | doc_fill)"
doc_run 'W1: START.md records the verdict' "$(printf '%s\n' "$verdict_block" | doc_fill)"
doc_run 'W1: START.md scouts the claim' "$(printf '%s\n' "$scout_block" | doc_fill)"

one_triage=$("$Q/crucible" triage 2>&1) || true
printf '%s\n' "$one_triage" | grep -q '^     -> MORE AUDIT' \
  && ok || bad "W2: one claim-auditor row and one TRUE must not reach ADMIT, got: $one_triage"
expect 'W2: one claim-auditor row cannot leave INVESTIGATE' '^NEXT INVESTIGATE' "$Q/crucible" cycle

# The floor is measurable, so compare the measured sentence against the documents rather than
# grepping for a keyword near it.
more_audit=$(printf '%s\n' "$one_triage" | sed -n 's/^     -> //p' | head -1)
doc_says 'W2: the MORE AUDIT floor message is documented' "$more_audit" \
  'START.md "The admit bar" must print the message cycle/triage actually emit for a one-row panel'
doc_each_says 'W2: every walkthrough document states the admit-bar rule' 'max(2,' \
  'state the bar as max(2, required=yes claim-auditor rows) so a reader casting one auditor knows it cannot admit'
for v in CRUCIBLE_MIN_AUDITORS CRUCIBLE_MIN_JUDGES CRUCIBLE_MIN_KINDS; do
  doc_says "W2: $v is documented" "$v" \
    'name the variable that overrides the bar, and what it overrides'
done

# W1, stated as one number against another and with both sides measured: the floor comes out of
# the message the engine just printed, the row count out of the template START.md prints. This is
# the 1.6.4 defect reduced to an inequality.
floor=$(printf '%s\n' "$more_audit" | sed -n 's/.*need \([0-9][0-9]*\) across.*/\1/p' | head -1)
rows=$(printf '%s\n' "$panel_template" \
  | awk -F "$tab" '$1 == "claim-auditor" && ($3 == "yes" || $3 == "required") { n++ } END { print n+0 }')
{ [ -n "$floor" ] && [ "$rows" -ge "$floor" ]; } && ok \
  || bad "W1: START.md's PANEL.ASSIGN.tsv template casts $rows required=yes claim-auditor row(s) and cycle/triage demand $floor — a reader who copies that template cannot leave INVESTIGATE. Add rows to the template at $HERE/START.md."

# =============================================================================================
# Restore the template as printed, then run the second auditor. START.md instructs steps 2 to 5
# once per claim-auditor row with a different agent each time, so the replay rewrites ` a1`.
# =============================================================================================
install_panel none
second() { printf '%s\n' "$1" | sed -e 's| a1\([^0-9A-Za-z]\)| a2\1|g' -e 's| a1$| a2|'; }
doc_run 'W1: the second claim-auditor row the template casts is dispatchable' "$(second "$dispatch_block" | doc_fill)"
doc_run 'W1: the second attempt seals' "$(printf '%s\n' "$seal_block" | doc_fill)"
doc_run 'W1: the second auditor records evidence' "$(second "$evidence_block" | doc_fill)"
doc_run 'W1: the second auditor records a verdict' "$(second "$verdict_block" | doc_fill)"

doc_run 'W1: START.md triage recommends a disposition' "$(printf '%s\n' "$triage_block" | doc_fill)"
printf '%s\n' "$DOC_OUT" | grep -q '^     -> ADMIT' \
  && ok || bad "W1: the template's claim-auditor rows must reach ADMIT in triage, got: $DOC_OUT
    the template at $HERE/START.md casts too few claim-auditor rows for its own admit bar"

cat > "$Q/PROPOSAL.md" <<'EOF'
# Proposal

## Verified problem

The documented walkthrough is not covered by a check, so documentation and engine can disagree.

## Proposed outcome

One executable suite that runs the documented path.

## Non-goals

No engine change and no documentation rewrite in this slice.

## Backlog

One bounded item.

## Verification

Two sealed TRUE verdicts and a scout ABSENT report.
EOF
expect 'a complete proposal waits for the human' '^WAIT APPROVAL' "$Q/crucible" cycle
"$Q/crucible" cycle approve >/dev/null

# =============================================================================================
# W2, second leg. `claim admit` reads the panel count, not the floor: more required rows than
# recorded TRUE verdicts must refuse there.
# =============================================================================================
install_panel three-auditors
refused_admit=$("$Q/crucible" claim admit C1 "$SLUG" 2>&1) && bad 'W2: three claim-auditor rows and two TRUE verdicts were admitted' || :
printf '%s\n' "$refused_admit" | grep -q 'has 2 TRUE verdicts, need 3' \
  && ok || bad "W2: claim admit must enforce the panel row count, got: $refused_admit"
doc_says 'W2: the panel-count refusal is documented' \
  "$(printf '%s\n' "$refused_admit" | sed 's/^crucible: //')" \
  'START.md "The admit bar" must print the refusal claim admit actually emits when the panel names more auditors than have recorded TRUE'
install_panel none

# =============================================================================================
# W1. The template START.md prints, cast as printed, must admit. This is the assertion that
# would have caught the 1.6.4 defect.
# =============================================================================================
doc_run 'W1: the panel template START.md prints can admit a claim' \
  "$(printf '%s\n' "$admit_block" | doc_fill)"
printf '%s\n' "$DOC_OUT" | grep -q "^admitted C1 as item $SLUG$" \
  && ok || bad "W1: claim admit under START.md's own panel template did not admit: $DOC_OUT
    the template at $HERE/START.md must cast enough claim-auditor rows to satisfy its own admit bar"

# =============================================================================================
# W3/W4/W5 run on admitted items. `ready` needs a finished contract and no document prints a
# filled-in one, so the fixture writes it.
# =============================================================================================
write_item() {
  cat > "$Q/items/$1/ITEM.md" <<EOF
# $1 — the documented path is executed by a check

## Goal

Run the documented walkthrough and fail when documentation and engine disagree.

## Non-goals

No engine change in this slice.

## Risk

LOW

## Owned files

- tracked.txt

## Acceptance criteria

- [ ] A1: the documented EXECUTE path reaches a maker PASS.

## Focused falsifier

scripts/verify-walkthrough.sh

## Expensive evidence

NONE

## Stop conditions

Stop if the documented sequence cannot be extracted.
EOF
}
write_item "$SLUG"

ready_block=$(doc_block "$HERE/START.md" 'plan-audit <slug>')
branch_block=$(doc_block "$HERE/START.md" 'git branch ai/<slug>')
maker_block=$(doc_block "$HERE/START.md" 'dispatch <slug> maker')
evidence_exec=$(doc_block "$HERE/START.md" 'git checkout ai/<slug>')
result_block=$(doc_block "$HERE/START.md" 'CP result ')

doc_says 'W3: plan-audit is documented' 'plan-audit' \
  'a travelling document must name plan-audit, or maker dispatch refuses with a gate nothing told the reader about'
doc_says 'W3: the plan-audit refusal is documented' 'refused: maker dispatch requires plan-audit PASS' \
  'a travelling document must print the refusal an unaudited plan produces'
doc_says 'W3: the work branch a reader must create is documented' 'git branch ai/<slug>' \
  'START.md must print the command that creates the work branch before the maker dispatch'
doc_run 'W3: ready, plan-audit and phase BUILD run as printed' \
  "$(printf '%s\n' "$ready_block" | doc_fill)"
grep -q '^VERDICT: PASS' "$Q/items/$SLUG/plan-audit.md" \
  && ok || bad 'W3: plan-audit PASS was not recorded, so the maker dispatch gate is unproven'

# --- W5: one dispatch form, and the recovery the refusal names --------------------------------
# A repeated dispatch strands an attempt. Assert the engine refuses the second one, that the
# phase transition refuses in the same terms START.md prints, that `attempt reclaim` redirects
# rather than helping, and that the recovery the refusal names lets the flow proceed.
# Only the dispatch lines of step 3 are replayed here. The seal and the start are precisely what
# a stranded attempt never gets — that is the state the two documented refusals describe.
dispatch_only=$(printf '%s\n' "$maker_block" | sed -n '/^[DA]=/p')
doc_run 'W5: a maker dispatch strands an attempt at DISPATCHED' \
  "$(printf '%s\n' "$dispatch_only" | doc_fill)"
strand=$A
refuses 'W5: a repeated dispatch refuses instead of stranding a second attempt' \
  "refused: item already has in-flight attempt $strand" \
  "$Q/crucible" dispatch "$SLUG" maker mk1 A1 FOCUSED
doc_says 'W5: the repeated-dispatch refusal is documented' \
  'refused: item already has in-flight attempt <id>' \
  'START.md must print the refusal a second item dispatch emits'

phase_refusal=$("$Q/crucible" phase "$SLUG" REVIEW 2>&1) && bad 'W5: the phase transition ignored a stranded attempt' || :
doc_phase=$(doc_text "$HERE/START.md" 'is DISPATCHED and still in flight' \
  | sed "s|<id>|$strand|g" | norm)
got_phase=$(printf '%s\n' "$phase_refusal" | norm)
case $got_phase in
  *"$doc_phase"*) ok ;;
  *) bad "W5: the phase refusal does not match the transcript START.md prints.
    START.md: $doc_phase
    engine:   $got_phase" ;;
esac

reclaim_refusal=$("$Q/crucible" attempt reclaim "$strand" 2>&1) && bad 'W5: attempt reclaim accepted an attempt that never started' || :
doc_reclaim=$(doc_text "$HERE/START.md" 'attempt reclaim requires RUNNING or OVERDUE' \
  | sed "s|<id>|$strand|g" | norm)
got_reclaim=$(printf '%s\n' "$reclaim_refusal" | norm)
case $got_reclaim in
  *"$doc_reclaim"*) ok ;;
  *) bad "W5: the attempt reclaim redirect does not match the transcript START.md prints.
    START.md: $doc_reclaim
    engine:   $got_reclaim" ;;
esac

# Run the recovery exactly as the refusal spells it, read back out of the message rather than
# retyped: a command the reader cannot copy out is not an actionable message.
recovery=$(printf '%s\n' "$phase_refusal" | tr '\n' ' ' \
  | sed -n 's|.*end it with: \([^ ]*crucible attempt finish [^ ]* ABANDONED\).*|\1|p' | head -1)
[ -n "$recovery" ] && ok || bad "W5: the phase refusal named no recovery command: $phase_refusal"
released=$(sh -c "$recovery \"fixture observed nothing started\"" 2>&1) || :
printf '%s\n' "$released" | grep -q "in-flight pointer released for $SLUG" \
  && ok || bad "W5: the documented recovery did not release the in-flight pointer: $released"
redispatch_out=$("$Q/crucible" dispatch "$SLUG" maker mk1 A1 FOCUSED 2>/dev/null) || redispatch_out=
case $redispatch_out in
  */contract.md) ok ;;
  *) bad "W5: a redispatch must proceed after the documented recovery, got: $redispatch_out" ;;
esac
if [ -n "$redispatch_out" ]; then
  redispatched=$(basename "$(dirname "$redispatch_out")")
  "$Q/crucible" attempt finish "$redispatched" ABANDONED 'fixture clears the W5 redispatch' >/dev/null
fi

# One runnable dispatch form per step. A second printed form is how the reader ends up running
# the one that strands an attempt.
for step in 'dispatch C1 claim-auditor' 'dispatch C1 scout' 'dispatch <slug> maker'; do
  n=$(awk '/^```sh$/ { inb = 1; next } /^```/ { inb = 0; next } inb' "$HERE/START.md" \
    | grep -cF -- "$step" || true)
  [ "$n" -eq 1 ] && ok \
    || bad "W5: START.md prints $n runnable forms of \"$step\" in sh code blocks; exactly one is safe, because dispatch is not idempotent and a second form strands an attempt"
done

# --- W3 continued: the branch, the maker, and a PASS ------------------------------------------
doc_run 'W3: the documented work branch is created' "$(printf '%s\n' "$branch_block" | doc_fill)"
wid=$("$Q/crucible" workid "$SLUG")
{ [ "$wid" != NOBRANCH ] && [ "$wid" != EMPTY ]; } \
  && ok || bad "W3: workid must print a commit after the documented branch command, got: $wid"

doc_run 'W3: the maker is dispatched, sealed and started' "$(printf '%s\n' "$maker_block" | doc_fill)"
maker_attempt=$A
# Where START.md prints `# ... maker changes only the item's Owned files, then commits ...`, the
# fixture is the maker. Owned files is `tracked.txt` and nothing else.
maker_commits() {
  printf 'implemented by the fixture maker\n' >> "$repo/tracked.txt"
  git -C "$repo" add tracked.txt
  git -C "$repo" commit -qm 'maker implements A1'
}
DOC_ON_COMMENT=maker_commits
doc_run 'W3: the maker checks out the work branch, commits, and records evidence' \
  "$(printf '%s\n' "$evidence_exec" | doc_fill)"
DOC_ON_COMMENT=
EV=$(basename "$(printf '%s\n' "$DOC_OUT" | awk '{print $1}')")
doc_run 'W3: the maker attempt finishes and records a result' \
  "$(printf '%s\n' "$result_block" | doc_fill)"
grep -q '^OUTCOME: PASS$' "$Q/attempts/$maker_attempt/result.md" \
  && ok || bad "W3: the maker result did not write OUTCOME: PASS into $Q/attempts/$maker_attempt/result.md"

# The whole point of the EXECUTE path is that it ends somewhere legal. REVIEW is the next gate
# and it refuses without a current-work maker PASS, so reaching it proves the result landed.
expect 'W3: a current-work maker PASS releases REVIEW' 'is now in REVIEW' \
  "$Q/crucible" phase "$SLUG" REVIEW

# =============================================================================================
# W4: the NOBRANCH dead end.
#
# This runs in a second repository with its own adopted program, and all three reasons are
# documented engine behaviour rather than convenience:
#   * a NOBRANCH maker attempt is spent. START.md says so, and it means it: the `result` refusal
#     leaves the item's in-flight pointer on a RETURNED attempt, `attempt finish` will not end a
#     RETURNED attempt, and no verb releases it. So W4 cannot share the item W3 carries to a PASS.
#   * one program holds one current item — `refused: another item is current` — so W4 cannot be a
#     second item in the first program either.
#   * `claim scout … ABSENT` refuses while ANY unmerged `ai/*` branch exists in the repository,
#     and W3 leaves one. So W4 cannot share the repository.
# Two adoptions total, not one per assertion.
# =============================================================================================
PROGRAM=nobranch
SLUG=walkthrough-nobranch
CLAIM_TITLE='the NOBRANCH dead end is not covered by a check'
CLAIM_SOURCE='The NOBRANCH dead end is not covered by a check.'
repo2="$tmp/target-nobranch"
mkdir -p "$repo2"
(
  cd "$repo2"
  git init -q -b "$BASE"
  git config user.name test
  git config user.email test@example.invalid
  git config commit.gpgsign false
  printf 'baseline\n' > tracked.txt
  git add tracked.txt
  git commit -qm baseline
  "$C" adopt "$PROGRAM" --managed >/dev/null
)
Q="$repo2/.crucible/$PROGRAM"
cd "$repo2"
CP=".crucible/$PROGRAM/crucible"
write_agents "$Q"
write_panel_md "$Q"
install_panel none
printf '%s\n' "$CLAIM_SOURCE" > "$repo2/report.md"
"$Q/crucible" cycle problem "$repo2/report.md" >/dev/null

doc_run 'W4: the claim is created' "$(printf '%s\n' "$add_block" | doc_fill)"
for who in a1 a2; do
  if [ "$who" = a2 ]; then
    w_dispatch=$(second "$dispatch_block"); w_ev=$(second "$evidence_block")
    w_verdict=$(second "$verdict_block")
  else
    w_dispatch=$dispatch_block; w_ev=$evidence_block; w_verdict=$verdict_block
  fi
  doc_run "W4: claim-auditor $who is dispatched" "$(printf '%s\n' "$w_dispatch" | doc_fill)"
  doc_run "W4: the attempt for $who seals" "$(printf '%s\n' "$seal_block" | doc_fill)"
  doc_run "W4: $who records evidence" "$(printf '%s\n' "$w_ev" | doc_fill)"
  doc_run "W4: $who records a verdict" "$(printf '%s\n' "$w_verdict" | doc_fill)"
done
doc_run 'W4: the claim is scouted' "$(printf '%s\n' "$scout_block" | doc_fill)"
cat > "$Q/PROPOSAL.md" <<'EOF'
# Proposal

## Verified problem

The NOBRANCH dead end is not covered by a check.

## Proposed outcome

Reproduce it, and require a document to name the way out.

## Non-goals

No engine change in this slice.

## Backlog

One bounded item.

## Verification

The refusal is measured and matched against the travelling documents.
EOF
"$Q/crucible" cycle approve >/dev/null
doc_run 'W4: the claim is admitted as an item' "$(printf '%s\n' "$admit_block" | doc_fill)"
write_item "$SLUG"
doc_run 'W4: ready, plan-audit and phase BUILD run as printed' \
  "$(printf '%s\n' "$ready_block" | doc_fill)"

[ "$("$Q/crucible" workid "$SLUG")" = NOBRANCH ] \
  && ok || bad "W4: the fixture cannot reproduce NOBRANCH — $SLUG already has a work branch"
doc_run 'W4: a maker is dispatched and sealed before the work branch exists' \
  "$(printf '%s\n' "$maker_block" | doc_fill)"
nobranch_attempt=$A
# Only the evidence line of step 4 is replayed. The checkout and the commit are exactly what W4
# withholds, because their absence is the defect under test.
run_line=$(printf '%s\n' "$evidence_exec" | grep '^\$CP run ')
doc_run 'W4: evidence records against a missing work branch' "$(printf '%s\n' "$run_line" | doc_fill)"
printf '%s\n' "$DOC_OUT" | grep -q 'NOBRANCH\.txt' \
  && ok || bad "W4: evidence recorded with no work branch must be named …NOBRANCH.txt, got: $DOC_OUT"
EV=$(basename "$(printf '%s\n' "$DOC_OUT" | awk '{print $1}')")
"$Q/crucible" attempt finish "$nobranch_attempt" RETURNED 'launcher observed exit 0' >/dev/null
nobranch_refusal=$("$Q/crucible" result "$nobranch_attempt" PASS "$EV" CLOSE - 2>&1) \
  && bad 'W4: result accepted evidence recorded with no work branch' || :
printf '%s\n' "$nobranch_refusal" | grep -q 'maker result requires current work' \
  && ok || bad "W4: result must refuse without current work, got: $nobranch_refusal"
doc_says 'W4: the NOBRANCH refusal is documented' 'maker result requires current work' \
  'a travelling document must print this refusal, or a reader dead-ends at the first maker result with nothing to read'
doc_says 'W4: NOBRANCH is named as the symptom' 'NOBRANCH' \
  'a travelling document must name the literal work id a missing work branch produces'
doc_says 'W4: the work branch is named where the reader is left' 'ai/<slug>' \
  'a travelling document must name the ai/<slug> work branch, or a reader stuck at NOBRANCH has nothing to create'

# --- this suite cleans up after itself --------------------------------------------------------
cd "$HERE"
rm -rf "$tmp"
trap - 0 1 2 15
[ ! -e "$tmp" ] && ok || bad "this suite left its own scratch directory behind: $tmp"

summary
[ "$FAIL" -eq 0 ]
