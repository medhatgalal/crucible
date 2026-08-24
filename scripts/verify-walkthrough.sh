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
#   * the two `INDEPENDENCE INCOMPLETE` transcripts START.md prints — the stray-attempt one and
#     the stale-panel one. Same treatment: extracted, normalised, compared against the disposition
#     `triage` actually emits in each of those two states.
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
#   * the three unreadable-panel states W6/C needs (absent; malformed; every claim-auditor row
#     `required=no`). No document prints a broken panel, and the property under test is what the
#     ENGINE does when it cannot read one.
#   * the two states W6/E and W6/F need — a stray unsealed EARLIEST claim dispatch, and a panel
#     recast after approval. START.md describes both in prose and prints the `triage` transcript
#     for each, but neither is a documented SEQUENCE: step 2 prints exactly one runnable dispatch
#     form and W5 asserts that it prints only one. So the states are built with fixture commands.
#     Running the extracted block a third time would add no documentation coverage — W1 already
#     executes every line of it twice — and would report fixture plumbing as though it were
#     checked prose.
#   * the state W6/G needs — a TRUE verdict whose author has no usable evidence on disk. `claim
#     verdict` refuses without evidence, so the only order that reaches this state is the one the
#     reviewer used: record the verdict normally, then remove the evidence file. No document prints
#     a sequence for it and the property under test is what the ENGINE does when it can no longer
#     see the check behind a verdict it already accepted.
#   * the third claim-auditor's agent name, `a3`. Same reason as the second: START.md hard-codes
#     `a1` and instructs "with a different agent each time". Where a measured line is compared
#     against START.md's transcript, the auditor is normalised to `<auditor>` on BOTH sides rather
#     than one side being rewritten to match the other.
#   * the claim's polarity during a W6 measurement. `cycle` only names the bar aloud when some
#     claim is not polarity ABSENT, and the fixture's one claim is inferred ABSENT from its own
#     wording, so a measurement that needs cycle's number flips it to DEFECT and back. Reasons at
#     `claim_polarity_set`.
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
#   W6  the three gates that enforce the admit bar apply ONE number — the engine has one writer of
#       it and three readers, and they are measured against each other at one, two and three
#       required=yes claim-auditor rows rather than pinned as three message strings. With the
#       invariant CHANGELOG 1.3.3 named (`cycle` never reports readiness for a claim `claim admit`
#       would refuse), the floor RULES.md 3 demands when the panel cannot be read, and the
#       CRUCIBLE_MIN_AUDITORS override that must still beat both. These are engine-behaviour
#       properties, so they are measured off the engine; the floor they are compared against is
#       extracted from START.md.
#       The bar is a THRESHOLD over a NUMERATOR, and 1.6.6 unified only the threshold. W6/A–D all
#       stand where the raw count of TRUE verdict files is SHORT of the bar, which is the half
#       every numerator agrees on. W6/E, W6/F and W6/G stand in the other half — raw count MEETS
#       the bar, independently counted set does not — which is where the divergence actually lived.
#       One block per per-verdict check `claim admit` applies and a reporting surface must agree
#       with: the attempt's transport (E), the panel's currency (F), and the evidence behind the
#       verdict (G). E and F diverge by a message, because those checks refuse; G diverges only by
#       a number, because that check subtracts.

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

# --- W6: measuring the admit bar off the gates that enforce it --------------------------------
#
# The bar has ONE writer in the engine and THREE readers: `cycle`'s INVESTIGATE line and state,
# `triage`'s recommendation, and `claim admit`'s refusal. When each reader carried its own copy of
# the number, a panel casting three claim-auditors let `cycle` report PLAN and `triage` recommend
# ADMIT for a claim `claim admit` then refused with `need 3`. Nothing in this suite reddened when
# that copy was reintroduced, and the reasons are worth writing down because they are the shape of
# the next gap:
#   * W2's first leg stands at ONE required row, where max(2,1) is 2 and a bare floor of 2 is also
#     2, so its messages are byte-identical whichever number the reader used;
#   * W2's second leg installs THREE rows — the row that actually moved — but only ever asks
#     `claim admit`, which was the one reader that already consulted the panel;
#   * W1's inequality compares the floor parsed at one row against the template's own row count,
#     and 2 >= 2 holds either way.
# So the number is MEASURED off each of the three gates and the three are compared. Pinning three
# message strings is what let the divergence live in the documents as if it were the rule.

# required=yes claim-auditor rows in the panel that is installed right now. This measures the
# fixture's own input, so an assertion can refuse to run rather than silently cover the wrong
# panel size if a mutation of START.md's template stops producing the size it was written for.
panel_ca_rows() {
  [ -f "$Q/PANEL.ASSIGN.tsv" ] || { printf '0'; return; }
  awk -F "$tab" '
    $1 ~ /^#/ || $1 == "role" { next }
    $1 == "claim-auditor" && (tolower($3) == "yes" || tolower($3) == "required") { n++ }
    END { print n+0 }
  ' "$Q/PANEL.ASSIGN.tsv"
}

# `cycle` names the bar out loud only when some claim is not polarity ABSENT; an all-ABSENT
# problem gets a different sentence with no number in it. The fixture's single claim is INFERRED
# ABSENT from its own wording, so a measurement that needs cycle's number opens a window in which
# the claim reads DEFECT and closes it again. This is sound rather than convenient: DEFECT with
# `scout: ABSENT` is a combination `claim admit`'s own polarity gate accepts, and nothing that
# computes the bar reads polarity at all — so the number measured inside the window is the number
# the gate applies outside it. Only whether cycle says it aloud changes.
claim_polarity_set() {
  sed "s/^    polarity: .*/    polarity: $1/" "$Q/CLAIMS.md" > "$tmp/claims.md"
  cat "$tmp/claims.md" > "$Q/CLAIMS.md"
}

# The bar each gate applies, read out of the gate itself. Numbers land in GB_CYCLE / GB_TRIAGE /
# GB_ADMIT and the raw output in GB_*_OUT, so a later assertion can reason about the SAME run
# rather than provoking a second one. Deliberately not a command substitution: the globals would
# be discarded with the subshell.
#
# All three gates name a number only while REFUSING, so every call site stands at a panel size the
# recorded TRUE verdicts do not satisfy. A gate that names no number there has decided the bar is
# already met while another gate refuses — that is the divergence itself, not a missing
# measurement — so it reads NONE and fails the comparison instead of being skipped.
#
# The `claim admit` probe runs under CRUCIBLE_MIN_KINDS=9 so that asking it what its bar is can
# never admit the claim as a side effect. The kinds gate sits immediately after the TRUE-count
# gate: a short count still prints the count refusal, and a satisfied count hits kinds instead of
# creating an item.
#
# The probe's exit status is recorded in GB_ADMIT_RC and not discarded with `|| true`. W6/G asserts
# that `claim admit` REFUSED, and a surface's refusal is its exit status; deriving it from the text
# would make the assertion depend on how the refusal is worded, which is the one thing that block
# must not do.
GB_CYCLE=NONE; GB_TRIAGE=NONE; GB_ADMIT=NONE
GB_CYCLE_OUT=; GB_TRIAGE_OUT=; GB_ADMIT_OUT=; GB_ADMIT_RC=0
gate_bars() {
  gb_ov=${1:-}
  gb_pol=$(sed -n 's/^    polarity: //p' "$Q/CLAIMS.md" | head -1)
  claim_polarity_set DEFECT
  GB_CYCLE_OUT=$(CRUCIBLE_MIN_AUDITORS="$gb_ov" "$Q/crucible" cycle 2>&1) || true
  GB_TRIAGE_OUT=$(CRUCIBLE_MIN_AUDITORS="$gb_ov" "$Q/crucible" triage 2>&1) || true
  set +e
  GB_ADMIT_OUT=$(CRUCIBLE_MIN_AUDITORS="$gb_ov" CRUCIBLE_MIN_KINDS=9 \
    "$Q/crucible" claim admit C1 bar-probe 2>&1)
  GB_ADMIT_RC=$?
  set -e
  claim_polarity_set "$gb_pol"
  gb_c=$(printf '%s\n' "$GB_CYCLE_OUT"  | sed -n 's/.*admit needs \([0-9][0-9]*\) sealed TRUE.*/\1/p' | head -1)
  gb_t=$(printf '%s\n' "$GB_TRIAGE_OUT" | sed -n 's/.*MORE AUDIT.*need \([0-9][0-9]*\) across.*/\1/p' | head -1)
  gb_a=$(printf '%s\n' "$GB_ADMIT_OUT"  | sed -n 's/.*TRUE verdicts, need \([0-9][0-9]*\).*/\1/p' | head -1)
  GB_CYCLE=${gb_c:-NONE}; GB_TRIAGE=${gb_t:-NONE}; GB_ADMIT=${gb_a:-NONE}
}

# The recommendation line triage printed on the last measured run, with its indent stripped.
gate_rec() { printf '%s\n' "$GB_TRIAGE_OUT" | sed -n 's/^ *-> //p' | head -1; }

# Three gates, one number. LABEL, the number every gate must apply, and an optional
# CRUCIBLE_MIN_AUDITORS value to measure under.
gates_apply() {
  ga_label=$1; ga_want=$2; ga_ov=${3:-}
  gate_bars "$ga_ov"
  if [ "$GB_CYCLE" = "$ga_want" ] && [ "$GB_TRIAGE" = "$ga_want" ] && [ "$GB_ADMIT" = "$ga_want" ]
  then
    ok
  else
    bad "$ga_label: every gate must apply $ga_want, but cycle applies $GB_CYCLE, triage applies $GB_TRIAGE and claim admit applies $GB_ADMIT.
    NONE means that gate named no bar at all while another gate refuses: it has decided the bar is
    met and the others have not, which is exactly the divergence CHANGELOG 1.3.3 named.
    cycle:  $(printf '%s\n' "$GB_CYCLE_OUT" | head -1)
    triage: $(gate_rec)
    admit:  $GB_ADMIT_OUT"
  fi
}

# The same, with the expected number derived from the panel that is installed and the floor that
# was extracted from START.md — max(floor, rows) — rather than hand-picked per call site.
gates_agree() {
  gg_label=$1; gg_rows=$2
  gg_got=$(panel_ca_rows)
  if [ "$gg_got" -ne "$gg_rows" ]; then
    bad "$gg_label: the installed panel casts $gg_got required=yes claim-auditor row(s), not the $gg_rows this assertion covers — the fixture's panel mutation no longer produces that size, so the panel size this assertion exists for is no longer being checked at all"
    return
  fi
  gg_want=$gg_rows
  if [ "$gg_want" -lt "$doc_floor" ]; then gg_want=$doc_floor; fi
  gates_apply "$gg_label [max($doc_floor,$gg_rows)=$gg_want]" "$gg_want"
}

# --- W6/E and W6/F helpers: the OTHER half of the numerator ------------------------------------
#
# The bar is a threshold over a numerator. Everything above measures the THRESHOLD, and every call
# site above stands where the raw count of TRUE verdict files is short of it — the half where the
# numerator cannot matter, because the arithmetic is short whichever set is counted. The two blocks
# at the end of W6 stand where the raw count MEETS the bar and the independently counted set does
# not, so the fixture has to be able to measure the RAW side itself to prove it is in that half.

# TRUE verdict FILES on disk for a claim, counted the way the defect counted them: every file whose
# verdict line reads TRUE, with no independence check at all. This exists to prove the fixture is
# in the half the assertions below are about — a state short on BOTH numerators is the state W6/A–D
# already cover and would prove nothing here. It is never compared against a gate.
raw_true_files() {
  rtf=0
  for rtf_f in "$Q/claims/$1/verdicts"/*.md; do
    [ -f "$rtf_f" ] || continue
    [ "$(sed -n 's/^CLAIM-VERDICT: //p' "$rtf_f" | head -1)" = TRUE ] && rtf=$((rtf + 1))
  done
  printf '%s' "$rtf"
}

# max(floor extracted from START.md, required=yes claim-auditor rows in the panel installed now).
# Same composition the engine states, from the same two measured inputs the rest of W6 uses.
admit_bar_now() {
  abn=$(panel_ca_rows)
  [ "$abn" -lt "$doc_floor" ] && abn=$doc_floor
  printf '%s' "$abn"
}

# Usable evidence files for one agent on one claim, counted the way the engine counts them:
# non-empty, and headed by the run marker `crucible run-claim` writes. W6/G stands in the state
# where this is ZERO for an agent whose TRUE verdict is on disk.
#
# The marker is read out of the ADOPTED engine's own `MARK=` assignment rather than retyped here.
# This suite does not own the engine, and a fixture that hard-coded the string would keep reporting
# "no usable evidence" after the engine started writing a different header — it would be measuring
# its own constant. An unreadable marker makes this return 0 files found for every agent, which the
# assertion below catches as a fixture failure rather than a passing state.
usable_evidence() {
  ue_mark=$(awk -F "'" '/^MARK=/ { print $2; exit }' "$Q/crucible")
  ue=0
  [ -n "$ue_mark" ] || { printf '0'; return; }
  for ue_f in "$Q/claims/$1/evidence/$2".*.txt; do
    [ -s "$ue_f" ] || continue
    [ "$(head -1 "$ue_f")" = "$ue_mark" ] || continue
    ue=$((ue + 1))
  done
  printf '%s' "$ue"
}

# Does TEXT name AGENT as a whole word? Every non-alphanumeric byte, newlines included, becomes a
# space, so the test is one space-delimited token match over the whole text and cannot be fooled by
# `a3` sitting inside a longer name or an attempt id. Deliberately no BRE alternation: `\|` is a GNU
# extension and this suite runs under /bin/sh on hosts whose grep does not have it.
names_agent() {
  printf ' %s\n' "$1" | tr -c '0-9A-Za-z' ' ' | grep -q " $2 "
}

# Two gates, one state: `triage` and `claim admit`. W6/F uses this instead of `gate_bars` because
# in a stale-panel state `cycle` is stopped by the panel gate itself and reports WAIT PANEL, so it
# is not a party to the disagreement there and measuring it would cost a second full cycle walk to
# assert something the panel gate already guarantees. The `claim admit` probe keeps
# CRUCIBLE_MIN_KINDS=9 for the same reason `gate_bars` does: interrogating it must not admit.
FP_TRIAGE_OUT=; FP_ADMIT_OUT=; FP_ADMIT_RC=0
gates_two() {
  FP_TRIAGE_OUT=$("$Q/crucible" triage 2>&1) || true
  set +e
  FP_ADMIT_OUT=$(CRUCIBLE_MIN_KINDS=9 "$Q/crucible" claim admit C1 bar-probe 2>&1)
  FP_ADMIT_RC=$?
  set -e
}
fp_rec() { printf '%s\n' "$FP_TRIAGE_OUT" | sed -n 's/^ *-> //p' | head -1; }

# The auditor named inside a disposition, normalised to the metavariable START.md's transcripts
# write around it. Applied to BOTH sides of a transcript comparison, and deliberately blind to the
# name: which agent the document picks for its example is fixture-local, and the suite does not own
# START.md. The token is located by its position — the one that precedes `resolves to attempt` —
# so renaming the example auditor on either side changes nothing here.
norm_auditor() { sed -e 's|\([.] \)[^ ][^ ]* resolves to attempt|\1<auditor> resolves to attempt|g'; }

# A transcript comparison for the `triage` dispositions START.md prints. `norm` squeezes whitespace
# and rewrites the program path; the attempt id is substituted into the document's `<id>` by the
# caller. The trailing sentence period is stripped from both sides: the engine ends a disposition
# with one and the document's fenced transcript does not, which is a difference in punctuation and
# not in what the operator is told.
trim_dot() { sed -e 's/[. ]*$//'; }

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
# W2 measures the one-row/one-TRUE floor. Step 6 now optionally records a scout TRUE, so omit
# only that optional verdict here; W4 executes the full extracted scout block below.
w2_scout_block=$(printf '%s\n' "$scout_block" \
  | sed '/^\$CP claim verdict C1 sc1 TRUE[[:space:]]*\(#.*\)\{0,1\}$/d')
doc_run 'W1: START.md scouts the claim' "$(printf '%s\n' "$w2_scout_block" | doc_fill)"

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
# W6/A. The three gates agree, at every panel size that matters.
#
# The floor is a documentation property as much as an engine one, so it is read OUT of START.md
# rather than retyped here. Every walkthrough document is already required above to state the rule
# as `max(2, required=yes claim-auditor rows)`; this lifts the constant out of that same sentence.
# An engine that lowers the floor therefore fails against the document, and a release that lowers
# it in code and in prose together fails the `max(2,` assertion above instead of quietly agreeing
# with itself here.
# =============================================================================================
doc_floor=$(sed -n 's/.*max(\([0-9][0-9]*\),.*/\1/p' "$HERE/START.md" | head -1)
case $doc_floor in
  ''|*[!0-9]*)
    bad "W6/A: START.md must state the admit bar as max(<floor>, required=yes claim-auditor rows) — every bar measured below is compared against the floor extracted from that sentence and not against a number typed into this suite; falling back to 2 to keep going"
    doc_floor=2 ;;
  *) ok ;;
esac

# One required row, one TRUE. The floor is what applies and all three gates must say so. This is
# the size that reddens the moment the panel count is allowed to win BELOW the floor.
gates_agree 'W6/A: cycle, triage and claim admit apply one admit bar at 1 required=yes claim-auditor row' 1

# =============================================================================================
# Restore the template as printed, then run the second auditor. START.md instructs steps 2 to 5
# once per claim-auditor row with a different agent each time, so the replay rewrites ` a1`.
# =============================================================================================
install_panel none

# Two required rows, still one TRUE. Here max(2,2) and a bare floor of 2 agree, so no mutation of
# the floor test alone can separate them — this size is in the set because A is the claim that the
# gates agree at every panel size that matters, and it is the one that still catches a gate which
# stopped reading the bar from the shared writer altogether rather than merely reading it wrong.
gates_agree 'W6/A: cycle, triage and claim admit apply one admit bar at 2 required=yes claim-auditor rows' 2

# =============================================================================================
# W6/C. RULES.md 3, "Absence fails". `panel_required_count` answers 0 for a panel that is missing,
# unparseable, or casts no required claim-auditor at all, and a bar that followed that 0 downwards
# would admit work on ONE uncorroborated TRUE verdict — a check most confident when it knows least
# is inverted. So the floor holds in all three states.
#
# `triage` is the gate that still answers the question in these states, so the number is measured
# there. `cycle` refuses to route at all and `claim admit` refuses on panel currency before it ever
# reaches its count; both refusals are stronger than the floor rather than weaker, and they are
# asserted as themselves below so that a release which turned one of them into a warning is caught
# here too. Measured behaviour, not restated documentation: the floor these are compared against is
# the one extracted from START.md.
# =============================================================================================
absent_cycle_ok=1
absent_admit_ok=1
absent_detail=
for state in absent malformed zero-rows; do
  case $state in
    absent) rm -f "$Q/PANEL.ASSIGN.tsv" ;;
    malformed)
      printf 'this is not a panel\nrole agent required notes\nclaim-auditor a1 yes\n' \
        > "$Q/PANEL.ASSIGN.tsv" ;;
    zero-rows)
      { printf 'role%sagent%srequired%snotes\n' "$tab" "$tab" "$tab"
        printf 'claim-auditor%sa1%sno%soptional\n' "$tab" "$tab" "$tab"
        printf 'claim-auditor%sa2%sno%soptional\n' "$tab" "$tab" "$tab"; } > "$Q/PANEL.ASSIGN.tsv" ;;
  esac
  absent_cycle_out=$("$Q/crucible" cycle 2>&1) || true
  absent_triage_out=$("$Q/crucible" triage 2>&1) || true
  set +e
  absent_admit_out=$(CRUCIBLE_MIN_KINDS=9 "$Q/crucible" claim admit C1 bar-probe 2>&1)
  absent_admit_rc=$?
  set -e
  absent_bar=$(printf '%s\n' "$absent_triage_out" \
    | sed -n 's/.*MORE AUDIT.*need \([0-9][0-9]*\) across.*/\1/p' | head -1)
  if [ "${absent_bar:-NONE}" = "$doc_floor" ]; then
    ok
  else
    bad "W6/C: RULES.md 3, absence fails — with PANEL.ASSIGN.tsv $state the bar must stay at the floor $doc_floor, but the gate applied ${absent_bar:-none: it named no bar at all}. A bar that follows an unreadable panel down admits work on one uncorroborated TRUE verdict.
    triage: $(printf '%s\n' "$absent_triage_out" | sed -n 's/^ *-> //p' | head -1)"
  fi
  printf '%s\n' "$absent_cycle_out" | grep -q '^NEXT CONFIGURE' || {
    absent_cycle_ok=
    absent_detail="$absent_detail
    $state: cycle said \"$(printf '%s\n' "$absent_cycle_out" | head -1)\" instead of NEXT CONFIGURE"; }
  [ "$absent_admit_rc" -ne 0 ] || {
    absent_admit_ok=
    absent_detail="$absent_detail
    $state: claim admit did not refuse — it answered \"$absent_admit_out\""; }
done
if [ -n "$absent_cycle_ok" ] && [ -n "$absent_admit_ok" ]; then
  ok
else
  bad "W6/C: RULES.md 3, absence fails — an absent, malformed or zero-row PANEL.ASSIGN.tsv must stop the cycle rather than lower the bar: cycle must send the reader to NEXT CONFIGURE and claim admit must refuse outright.$absent_detail"
fi
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

# =============================================================================================
# THREE required rows and TWO TRUE verdicts. This is the row that moved, and until now the only
# gate this suite asked here was `claim admit` — the one gate that already read the panel. The
# next three blocks all stand on this one configuration.
# =============================================================================================
gates_agree 'W6/A: cycle, triage and claim admit apply one admit bar at 3 required=yes claim-auditor rows' 3

# =============================================================================================
# W6/B. The invariant CHANGELOG 1.3.3 recorded, named here so a future reader knows what they
# broke: `cycle` no longer reports PLAN/COMPLETE for a TRUE claim that `claim admit` would refuse.
#
# Asserted as the implication rather than as cycle's sentence, because the sentence is not the
# invariant — the relationship between two gates is. Both halves read the run `gates_agree` just
# made, so the three gates are being compared on one state and not on three.
# =============================================================================================
case $GB_ADMIT_OUT in
  *'TRUE verdicts, need'*)
    case $GB_CYCLE_OUT in
      'NEXT INVESTIGATE'*) ok ;;
      *) bad "W6/B: invariant CHANGELOG 1.3.3 broken — cycle reports readiness for a claim claim admit refuses. cycle must still be in INVESTIGATE while the bar is unmet.
    claim admit: $GB_ADMIT_OUT
    cycle:       $(printf '%s\n' "$GB_CYCLE_OUT" | head -1)" ;;
    esac ;;
  *) bad "W6/B: invariant CHANGELOG 1.3.3 cannot be tested — at 3 required=yes claim-auditor rows with 2 TRUE verdicts claim admit must refuse for want of TRUE verdicts, so that cycle has something to disagree with. It said: $GB_ADMIT_OUT" ;;
esac
case $(gate_rec) in
  'MORE AUDIT'*) ok ;;
  *) bad "W6/B: invariant CHANGELOG 1.3.3 broken — triage recommends a disposition claim admit refuses. With the bar unmet the only sound recommendation is MORE AUDIT.
    claim admit: $GB_ADMIT_OUT
    triage:      $(gate_rec)" ;;
esac

# =============================================================================================
# W6/D. The documented escape hatch still wins. CRUCIBLE_MIN_AUDITORS is the auditable way to run
# a different bar — auditable because it sits in the invocation instead of in a quietly edited
# panel row. If it stopped reaching all three gates, the only way left to run a smaller bar would
# be to edit the panel, which is the change nobody reviews.
# =============================================================================================
gates_apply 'W6/D: CRUCIBLE_MIN_AUDITORS reaches all three gates and overrides the panel count' 9 9

# Lowering is the case that matters, because it has to beat the panel count (3) AND the floor (2).
# One measurement, three gates asserted separately so a failure names which one stopped honouring
# the override.
gate_bars 1
case $GB_CYCLE_OUT in
  'NEXT INVESTIGATE'*)
    bad "W6/D: CRUCIBLE_MIN_AUDITORS=1 did not lower cycle's bar below the panel count and the floor — with 2 TRUE verdicts and the bar at 1, cycle must be past INVESTIGATE. It said: $(printf '%s\n' "$GB_CYCLE_OUT" | head -1)" ;;
  *) ok ;;
esac
case $(gate_rec) in
  'ADMIT'*) ok ;;
  *) bad "W6/D: CRUCIBLE_MIN_AUDITORS=1 did not lower triage's bar below the panel count and the floor — with 2 TRUE verdicts and the bar at 1, triage must recommend ADMIT. It said: $(gate_rec)" ;;
esac
# `claim admit` under the override must get PAST its TRUE-count gate. The probe keeps
# CRUCIBLE_MIN_KINDS=9 set, so passing that gate surfaces as the NEXT refusal — kinds — rather
# than as an admission, which is what makes this assertion safe to run mid-flow.
case $GB_ADMIT_OUT in
  *'TRUE verdicts, need'*)
    bad "W6/D: CRUCIBLE_MIN_AUDITORS=1 did not lower claim admit's TRUE-verdict bar below the panel count and the floor — it still refused on the count: $GB_ADMIT_OUT" ;;
  *'kind(s), need 9'*) ok ;;
  *) bad "W6/D: the claim admit override probe refused for an unexpected reason, so it proves nothing about the bar: $GB_ADMIT_OUT" ;;
esac

# The probe interrogates `claim admit`; it must never have answered by admitting. The kinds gate is
# what makes that safe, so it is checked rather than assumed — a probe that admitted C1 would
# quietly invalidate every W1 assertion after this point instead of failing here.
probe_item=$(sed -n '/^### C1 /,/^### C[0-9]/{s/^    item: //p;}' "$Q/CLAIMS.md" | head -1)
if [ ! -e "$Q/items/bar-probe" ] && [ -z "$probe_item" ]; then
  ok
else
  bad "W6: the bar probe admitted the claim it was only supposed to interrogate — C1 now reads item \"$probe_item\" and $Q/items/bar-probe $([ -e "$Q/items/bar-probe" ] && printf exists || printf 'does not exist'). Every W1 assertion below this point is measuring a state the probe created."
fi

# =============================================================================================
# W6/E. The half W6/B cannot see.
#
# W6/B above stands at three required rows with TWO TRUE verdicts: the RAW count of TRUE verdict
# files is short of the bar, so every numerator agrees there and the arithmetic is short whichever
# set is counted. That is why a release could add an assertion named after the CHANGELOG 1.3.3
# invariant and still leave the invariant broken — the defect lived where the raw count MEETS the
# bar and the independently counted set does not, and nothing above can stand there.
#
# The state is built the way the defect was built. `dispatch` is not idempotent; `claim verdict`
# binds an agent's LAST sealed dispatch and the admit bar resolves that agent's EARLIEST one. So a
# stray earliest dispatch left unsealed keeps a recorded TRUE verdict off the counted set while
# leaving the verdict file on disk: three required rows, three TRUE verdict files, two counted.
#
# The fixture's own arithmetic is asserted before anything is concluded from it. A state that is
# short on BOTH numerators is the state W6/B already covers, and an assertion standing in it would
# report a pass for the half it was written to leave.
#
# What is NOT asserted here, because the engine does not promise it: that a `triage` ADMIT implies
# `claim admit` succeeds. Two per-verdict checks stay outside the counter — the transport ladder
# re-checked against the current panel, and the ACP-probe failure `subagent` transport requires —
# because each refuses rather than subtracting, and a read-only reporting surface cannot refuse
# mid-report. The evidence check was the third until it was folded into `claim_true_counts`: it
# subtracted rather than refusing, which is why W6/G below can stand in its state at all.
# ADMIT narrows admittability without proving it. That residual limit is a documented property
# and is asserted as one below.
# =============================================================================================
stray_who=a3
stray_dispatch=$("$Q/crucible" dispatch C1 claim-auditor "$stray_who" 2>/dev/null)
stray_attempt=$(sed -n 's/^attempt-id: //p' "$stray_dispatch" | head -1)
sealed_dispatch=$("$Q/crucible" dispatch C1 claim-auditor "$stray_who" 2>/dev/null)
sealed_attempt=$(sed -n 's/^attempt-id: //p' "$sealed_dispatch" | head -1)
"$Q/crucible" attempt transport "$sealed_attempt" multi-agent >/dev/null
"$Q/crucible" contract-audit "$sealed_attempt" ca1 PASS >/dev/null
"$Q/crucible" run-claim C1 "$stray_who" -- grep -rn -- --json . >/dev/null 2>&1 || true
"$Q/crucible" claim verdict C1 "$stray_who" TRUE >/dev/null

# The fixture is in the half these assertions are about, or it proves nothing. Both numbers are
# measured: the raw file count off the verdict directory, the bar off the installed panel and the
# floor extracted from START.md.
stray_bar=$(admit_bar_now)
stray_raw=$(raw_true_files C1)
{ [ "$stray_raw" -ge "$stray_bar" ] && [ "$stray_bar" -eq 3 ]; } && ok \
  || bad "W6/E: the fixture is not in the half this block exists for — $stray_raw raw TRUE verdict file(s) against a bar of $stray_bar. The raw count must MEET the bar (and the bar must be the 3-row one), otherwise this is the both-numerators-short state W6/B already covers and every assertion below reports a pass for the half it was written to leave."

# And it is a stray-EARLIEST state specifically, not some other way of being uncounted: the first
# dispatch this fixture made carries no transport, the second does, and the verdict was recorded
# off the second. Checked on the ledger rather than inferred from a message.
{ [ -n "$stray_attempt" ] && [ -n "$sealed_attempt" ] \
    && [ "$stray_attempt" != "$sealed_attempt" ] \
    && [ ! -e "$Q/attempts/$stray_attempt/transport" ] \
    && [ -e "$Q/attempts/$sealed_attempt/transport" ]; } && ok \
  || bad "W6/E: the fixture did not reach a stray-EARLIEST state — earliest attempt $stray_attempt (transport $([ -e "$Q/attempts/$stray_attempt/transport" ] && printf present || printf absent)), later attempt $sealed_attempt (transport $([ -e "$Q/attempts/$sealed_attempt/transport" ] && printf present || printf absent)). The uncounted TRUE verdict has to be uncounted for the documented reason, or W6/E is measuring a different defect."

# One state, three gates, one measurement — the same helper every W6 block above uses.
gate_bars

# `triage` is a recommendation surface: it must not recommend a disposition `claim admit` refuses.
# ADMIT is named as the failure explicitly, so a future disposition that is neither ADMIT nor this
# one fails as unrecognised rather than passing by not being ADMIT.
case $(gate_rec) in
  'INDEPENDENCE INCOMPLETE'*) ok ;;
  ADMIT*) bad "W6/E: invariant CHANGELOG 1.3.3 broken in the half W6/B cannot see — the raw count of TRUE verdict files meets the bar, the independently counted set does not, and triage recommends a disposition claim admit refuses. triage must count the set the gate counts.
    claim admit: $GB_ADMIT_OUT
    triage:      $(gate_rec)" ;;
  *) bad "W6/E: triage reported neither ADMIT nor INDEPENDENCE INCOMPLETE with $stray_raw raw TRUE verdict file(s) against a bar of $stray_bar and one of them uncounted. An unrecognised disposition is not a pass: it means this block no longer knows what it is measuring.
    triage: $(gate_rec)" ;;
esac

# The other side of the implication has to be real, or there is nothing to disagree with.
case $GB_ADMIT_OUT in
  *'has no transport'*) ok ;;
  *) bad "W6/E: invariant CHANGELOG 1.3.3 cannot be tested in this half — with a stray unsealed earliest dispatch, claim admit must refuse on that attempt's missing transport, so that the reporting surfaces have something to disagree with. It said: $GB_ADMIT_OUT" ;;
esac

# `cycle`'s half of the same invariant, on the same measured state. START.md promises cycle names
# nothing here and keeps printing its INVESTIGATE line; that promise is asserted as behaviour.
case $GB_CYCLE_OUT in
  'NEXT INVESTIGATE'*) ok ;;
  *) bad "W6/E: invariant CHANGELOG 1.3.3 broken in the half W6/B cannot see — cycle reports readiness for a claim claim admit refuses. With one TRUE verdict uncounted the bar is unmet and cycle must still be in INVESTIGATE.
    claim admit: $GB_ADMIT_OUT
    cycle:       $(printf '%s\n' "$GB_CYCLE_OUT" | head -1)" ;;
esac

# The correspondence that is the operator-facing value of the new disposition, and the reason it
# exists rather than a bare MORE AUDIT: the attempt `triage` names is the attempt `claim admit`
# refuses on, and it is the earliest dispatch. A diagnostic naming a different attempt would send
# the operator to seal something that was never in the way, and nothing else here would notice.
triage_id=$(gate_rec | sed -n 's/.*resolves to attempt \([^ ,]*\),.*/\1/p' | head -1)
admit_id=$(printf '%s\n' "$GB_ADMIT_OUT" \
  | sed -n 's/.*refused: attempt \([^ ]*\) has no transport.*/\1/p' | head -1)
{ [ -n "$triage_id" ] && [ "$triage_id" = "$admit_id" ] && [ "$triage_id" = "$stray_attempt" ]; } \
  && ok || bad "W6/E: triage and claim admit do not name the same attempt. triage names \"${triage_id:-none}\", claim admit refuses on \"${admit_id:-none}\", and the stray earliest dispatch is $stray_attempt. An operator who follows triage must land on the attempt the gate is actually blocked by.
    triage:      $(gate_rec)
    claim admit: $GB_ADMIT_OUT"

# The disposition is a transcript START.md prints, so it is compared against the document rather
# than pinned here. Both sides are normalised the same way: program path, attempt id, and the
# example auditor the document hard-codes.
doc_ii=$(doc_text "$HERE/START.md" 'INDEPENDENCE INCOMPLETE' \
  | norm_auditor | sed "s|<id>|$stray_attempt|g" | norm | trim_dot)
got_ii=$(printf '%s\n' "$(gate_rec)" | norm_auditor | norm | trim_dot)
if [ -n "$doc_ii" ] && [ "$doc_ii" = "$got_ii" ]; then
  ok
else
  bad "W6/E: the disposition does not match the transcript START.md prints.
    START.md: ${doc_ii:-(no INDEPENDENCE INCOMPLETE text block found)}
    engine:   $got_ii"
fi

# START.md promises `claim admit` refuses on the same attempt, and prints the refusal. Compared as
# a fixed string against the travelling set, so a reader standing in the document finds it.
doc_says 'W6/E: the claim admit refusal on the same attempt is documented' \
  "$(printf '%s\n' "$GB_ADMIT_OUT" | sed 's/^crucible: //' \
      | sed "s|$stray_attempt|<id>|g" | norm | sed 's/ *$//')" \
  'START.md must print the refusal claim admit emits for the attempt triage names, or an operator who read triage has nothing to match the gate against'
doc_says 'W6/E: the earliest-dispatch resolution rule is documented' \
  "resolves that agent's **earliest** claim dispatch" \
  'a travelling document must state that the admit bar resolves an agent EARLIEST claim dispatch — it is the whole reason a recorded TRUE verdict can sit on disk uncounted, and a reader who does not know it cannot diagnose this state'
doc_says 'W6/E: cycle staying in INVESTIGATE here is documented' \
  'it keeps printing its `NEXT INVESTIGATE` line' \
  'START.md must tell the reader cycle names nothing in this state, or the reader takes cycle silence for agreement'
# The fixed rule this fixture measures, kept where a reader meets the corresponding state. The
# fixed string is the shortest one that distinguishes the corrected disposition from ADMIT without
# pinning the full transcript.
doc_says 'W6/E: TRUE files that do not qualify are documented as non-ADMIT' \
  'instead of recommending `ADMIT`' \
  'START.md must state that when raw TRUE files clear the bar but the counted set does not, triage reports the shortfall instead of recommending ADMIT'

# The recovery `triage` printed is the recovery that works, and sealing the attempt it named is what
# changes the disposition — so the disposition above was caused by the stray seal and not by
# something incidental to the fixture. Deliberately insensitive to the numerator: it is the control,
# not the gap-closer, and it stays green under a reverted numerator.
"$Q/crucible" attempt transport "$stray_attempt" multi-agent >/dev/null
"$Q/crucible" contract-audit "$stray_attempt" ca1 PASS >/dev/null
sealed_triage=$("$Q/crucible" triage 2>&1) || true
case $(printf '%s\n' "$sealed_triage" | sed -n 's/^ *-> //p' | head -1) in
  ADMIT*) ok ;;
  *) bad "W6/E: sealing the attempt triage named did not change the disposition, so the INDEPENDENCE INCOMPLETE above was not caused by the stray earliest dispatch and this block is measuring something else. After transport + contract-audit PASS on $stray_attempt, triage said: $(printf '%s\n' "$sealed_triage" | sed -n 's/^ *-> //p' | head -1)" ;;
esac

# =============================================================================================
# W6/F. The second reachable state of the same shape. Three required rows, three properly sealed
# TRUE verdicts — the state W6/E just finished creating — and the panel recast after approval, so
# PANEL.APPROVAL names a stale panel id. Every counted verdict resolves through the current panel,
# so the counted set drops to zero while all three verdict files stay on disk: raw count still
# meets the bar. `claim admit` refuses on panel currency, and `triage` must not recommend ADMIT.
#
# The recast adds a `required=no` row, so `panel_required_count` is unchanged and the bar stays at
# 3. A recast that moved the bar would be testing the threshold again instead of the numerator.
# =============================================================================================
printf 'claim-auditor%sa1%sno%srecast after approval, never re-approved\n' \
  "$tab" "$tab" "$tab" >> "$Q/PANEL.ASSIGN.tsv"

panel_bar=$(admit_bar_now)
panel_raw=$(raw_true_files C1)
{ [ "$panel_raw" -ge "$panel_bar" ] && [ "$panel_bar" -eq 3 ]; } && ok \
  || bad "W6/F: the fixture is not in the half this block exists for — $panel_raw raw TRUE verdict file(s) against a bar of $panel_bar. The recast must leave the raw count meeting the bar and the bar unmoved, or this measures the threshold rather than the numerator."

gates_two
case $(fp_rec) in
  'INDEPENDENCE INCOMPLETE'*) ok ;;
  ADMIT*) bad "W6/F: invariant CHANGELOG 1.3.3 broken with a stale panel — three TRUE verdict files still meet the bar, none of them counts against a panel that is no longer current, and triage recommends a disposition claim admit refuses.
    claim admit: $FP_ADMIT_OUT
    triage:      $(fp_rec)" ;;
  *) bad "W6/F: triage reported neither ADMIT nor INDEPENDENCE INCOMPLETE with $panel_raw raw TRUE verdict file(s), a bar of $panel_bar and a stale panel. An unrecognised disposition is not a pass.
    triage: $(fp_rec)" ;;
esac
case $FP_ADMIT_OUT in
  *'agent panel is not current'*) [ "$FP_ADMIT_RC" -ne 0 ] && ok \
    || bad "W6/F: claim admit named the stale panel and still accepted: $FP_ADMIT_OUT" ;;
  *) bad "W6/F: invariant CHANGELOG 1.3.3 cannot be tested in this state — with the panel recast after approval claim admit must refuse on panel currency, so that triage has something to disagree with. It said: $FP_ADMIT_OUT" ;;
esac

# Both surfaces must send the operator to the same recovery, which is this state's version of the
# attempt-id correspondence W6/E asserts.
{ printf '%s\n' "$(fp_rec)" | grep -q 'cycle approve-panel' \
    && printf '%s\n' "$FP_ADMIT_OUT" | grep -q 'cycle approve-panel'; } && ok \
  || bad "W6/F: triage and claim admit do not name the same recovery for a stale panel. Both must send the operator to cycle approve-panel.
    triage:      $(fp_rec)
    claim admit: $FP_ADMIT_OUT"

# START.md prints this disposition too, and there is no attempt id in it to normalise.
doc_panel_ii=$(doc_text "$HERE/START.md" 'agent panel is not current' | norm | trim_dot)
got_panel_ii=$(printf '%s\n' "$(fp_rec)" | norm | trim_dot)
if [ -n "$doc_panel_ii" ] && [ "$doc_panel_ii" = "$got_panel_ii" ]; then
  ok
else
  bad "W6/F: the stale-panel disposition does not match the transcript START.md prints.
    START.md: ${doc_panel_ii:-(no such text block found)}
    engine:   $got_panel_ii"
fi

# W6/E and W6/F ran three more `claim admit` probes. Each was interrogated under
# CRUCIBLE_MIN_KINDS=9 so that clearing the count surfaces as the kinds refusal instead of as an
# admission, and that has to be checked rather than assumed: one of these states does clear the
# count, and a probe that admitted C1 would invalidate every W1 assertion after this point.
probe_item=$(sed -n '/^### C1 /,/^### C[0-9]/{s/^    item: //p;}' "$Q/CLAIMS.md" | head -1)
if [ ! -e "$Q/items/bar-probe" ] && [ -z "$probe_item" ]; then
  ok
else
  bad "W6: a W6/E or W6/F probe admitted the claim it was only supposed to interrogate — C1 now reads item \"$probe_item\" and $Q/items/bar-probe $([ -e "$Q/items/bar-probe" ] && printf exists || printf 'does not exist'). Every W1 assertion below this point is measuring a state the probe created."
fi

install_panel none

# =============================================================================================
# W6/G. The third reachable state of the same shape, and the one 1.6.6 said it had pinned.
#
# The 1.6.6 CHANGELOG claimed this suite pinned the residual limit "so no future change can assert
# that a `triage` ADMIT proves admittability". What it shipped for that is `doc_says` — a
# fixed-string search of the travelling documents for the sentence `does not prove the claim
# admittable`. That asserts a document SAYS something, and no engine change can redden it. No
# assertion stood in a state where the divergence was real, and neither W6/E nor W6/F can be that
# assertion: each stands on one specific per-verdict check, and this is a different one.
#
# The check here is the third of the three: at least one run-marked evidence file per TRUE auditor.
# It is the odd one out, and that is why it stayed uncovered. The other two REFUSE — `claim admit`
# dies naming the attempt, so `triage` and the gate diverge by a MESSAGE that a transcript
# comparison can catch. This one SUBTRACTS: the verdict is skipped, the count comes up short, and
# the count gate refuses. The surfaces diverge by a NUMBER while every message stays well-formed,
# so no transcript comparison anywhere above can see it.
#
# The state is built the way the reviewer reproduced it. The verdict is recorded normally, with its
# evidence in place — `claim verdict` refuses without it, so there is no other order — and the
# evidence file is removed afterwards. An operator reaches it through a `git clean`, a pruned
# scratch directory, or evidence written somewhere that later moved. What is left is a sealed,
# independent, registered TRUE verdict file that `claim admit` will not count: three required rows,
# three TRUE verdict files, one of them unevidenced.
#
# `claim admit` has to REACH that check or this measures nothing. It sits inside the per-verdict
# walk, ahead of the TRUE-count gate and well ahead of kinds, so the CRUCIBLE_MIN_KINDS=9 probe
# cannot mask it; and the panel is installed and re-approved immediately below, so panel currency —
# the blocker W6/F stands on — cannot refuse first. Both are asserted rather than assumed: the
# refusal `claim admit` gives is measured, and its arithmetic is compared against the raw count.
#
# What is asserted is the PROPERTY and not either surface's wording: `triage` must not recommend
# ADMIT while `claim admit` refuses, and `cycle` must not report readiness either. The disposition
# `triage` reports instead is MEASURED and printed on failure rather than pinned, because the engine
# change that folds this check into the shared counter landed alongside this block and a suite that
# pinned its sentence would be testing that sentence instead of the property. On the tree where this
# was written the three surfaces read: `cycle` NEXT INVESTIGATE, `triage` INDEPENDENCE INCOMPLETE
# naming a3 and the `run-claim` recovery, `claim admit` exit 2 on `has 2 TRUE verdicts, need 3` after
# declining to count a3. Nothing here asserts the converse — that a `triage` ADMIT proves
# admittability — because the engine does not promise it: the transport-ladder re-check against the
# current panel and the ACP-probe failure `subagent` transport requires are still refusals outside
# the counter, so the residual limit the assertion above finds documented is still real.
# =============================================================================================
install_panel three-auditors
noev_who=a3
noev_evidence_before=$(usable_evidence C1 "$noev_who")
mkdir -p "$tmp/noev"
mv "$Q/claims/C1/evidence/$noev_who".*.txt "$tmp/noev/" 2>/dev/null || :

# The fixture is in the half this block exists for, or it proves nothing — same discipline as W6/E
# and W6/F, and the same two measured numbers. The raw count of TRUE verdict files must still MEET
# the bar (a state short on both numerators is what W6/A–D already cover), the bar must be the
# 3-row one, and the state must be UNEVIDENCED for exactly the documented reason: a TRUE verdict on
# disk for an agent that had usable evidence when the verdict was written and has none now.
noev_bar=$(admit_bar_now)
noev_raw=$(raw_true_files C1)
noev_verdict=$(sed -n 's/^CLAIM-VERDICT: //p' "$Q/claims/C1/verdicts/$noev_who.md" 2>/dev/null | head -1)
noev_evidence_now=$(usable_evidence C1 "$noev_who")
{ [ "$noev_raw" -ge "$noev_bar" ] && [ "$noev_bar" -eq 3 ] \
    && [ "$noev_verdict" = TRUE ] \
    && [ "$noev_evidence_before" -gt 0 ] && [ "$noev_evidence_now" -eq 0 ]; } && ok \
  || bad "W6/G: the fixture is not in the half this block exists for — $noev_raw raw TRUE verdict file(s) against a bar of $noev_bar, $noev_who's verdict reads \"${noev_verdict:-none}\", and $noev_who had $noev_evidence_before usable evidence file(s) before the removal and $noev_evidence_now after. The raw count must MEET the 3-row bar and the TRUE verdict must be on disk with its evidence gone, or this is not the unevidenced-verdict state and every assertion below reports a pass for the half it was written to leave."

# One state, three surfaces, one measurement — the same helper every W6 block above uses.
gate_bars
noev_rec=$(gate_rec)

# The property. `triage` is a recommendation surface: it must not recommend a disposition
# `claim admit` refuses. Refusal is read off the exit STATUS, so this half of the implication does
# not depend on how the refusal is worded; the disposition is matched only against ADMIT, so a
# future disposition this suite has never seen passes on the property rather than reddening on its
# spelling — what it reports instead is asserted, measured, immediately below.
if [ "$GB_ADMIT_RC" -eq 0 ]; then
  bad "W6/G: the invariant cannot be tested — with one of three TRUE verdicts unevidenced, claim admit must refuse, so that the reporting surfaces have something to disagree with. It exited 0 and said: $GB_ADMIT_OUT"
else
  case $noev_rec in
    ADMIT*) bad "W6/G: invariant CHANGELOG 1.3.3 broken on the evidence check — the raw count of TRUE verdict files meets the bar, one of those verdicts has no usable evidence so claim admit does not count it, and triage recommends a disposition claim admit refuses. triage must count the set the gate counts.
    claim admit: $GB_ADMIT_OUT
    triage:      $noev_rec" ;;
    '') bad "W6/G: triage printed no disposition at all for C1, so this block no longer knows what it is measuring.
    triage: $GB_TRIAGE_OUT" ;;
    *) ok ;;
  esac
fi

# What `claim admit` actually reports here, measured as arithmetic rather than pinned as a string:
# it refuses on the TRUE count, and the count it names is the raw count MINUS the one unevidenced
# verdict, against the bar the installed panel sets. That is what makes this a numerator defect and
# not a threshold one — the threshold both surfaces read is the same, and the sets are not.
noev_counted=$(printf '%s\n' "$GB_ADMIT_OUT" \
  | sed -n 's/.*has \([0-9][0-9]*\) TRUE verdicts, need.*/\1/p' | head -1)
noev_need=$(printf '%s\n' "$GB_ADMIT_OUT" \
  | sed -n 's/.*TRUE verdicts, need \([0-9][0-9]*\).*/\1/p' | head -1)
{ [ -n "$noev_counted" ] && [ "$noev_counted" -eq $((noev_raw - 1)) ] \
    && [ "$noev_need" = "$noev_bar" ]; } && ok \
  || bad "W6/G: claim admit did not refuse on the count with the unevidenced verdict subtracted — with $noev_raw TRUE verdict file(s), one unevidenced, and a bar of $noev_bar it must count $((noev_raw - 1)) and demand $noev_bar. It named ${noev_counted:-no count} against ${noev_need:-no bar}: $GB_ADMIT_OUT"

# `cycle`'s half of the same invariant, on the same measured state. Unlike W6/F, cycle IS a party
# here: the panel is current, so nothing stops it before it reads the claim, and with one TRUE
# verdict uncounted the bar is unmet and it must still be routing the reader to INVESTIGATE.
case $GB_CYCLE_OUT in
  'NEXT INVESTIGATE'*) ok ;;
  *) bad "W6/G: invariant CHANGELOG 1.3.3 broken on the evidence check — cycle reports readiness for a claim claim admit refuses. With one of three TRUE verdicts unevidenced the bar is unmet and cycle must still be in INVESTIGATE.
    claim admit: $GB_ADMIT_OUT
    cycle:       $(printf '%s\n' "$GB_CYCLE_OUT" | head -1)" ;;
esac

# The correspondence that makes the disposition useful, and the equivalent of the attempt-id
# correspondence W6/E asserts and the shared-recovery one W6/F asserts: whatever `triage` names as
# the blocker is the verdict `claim admit` refuses on. The bound token is the AGENT, not a phrase —
# an identifier both surfaces already have to produce, so this holds across any rewording of either
# message. An operator sent to a different auditor would re-run a check that was never in the way,
# and nothing else in this suite would notice.
{ names_agent "$noev_rec" "$noev_who" && names_agent "$GB_ADMIT_OUT" "$noev_who"; } && ok \
  || bad "W6/G: triage and claim admit do not name the same uncounted auditor. The verdict with no usable evidence is $noev_who's, and both surfaces must name it — claim admit because that is the verdict it declined to count, triage because that is where it must send the operator.
    triage:      $noev_rec
    claim admit: $GB_ADMIT_OUT"

# The probe interrogated `claim admit` in a state whose raw count meets the bar. It must not have
# answered by admitting: an admitted C1 would invalidate every W1 assertion below instead of
# failing here. Same guard as W6/A–F, repeated because this is a new state, not a new probe style.
probe_item=$(sed -n '/^### C1 /,/^### C[0-9]/{s/^    item: //p;}' "$Q/CLAIMS.md" | head -1)
if [ ! -e "$Q/items/bar-probe" ] && [ -z "$probe_item" ]; then
  ok
else
  bad "W6/G: the bar probe admitted the claim it was only supposed to interrogate — C1 now reads item \"$probe_item\" and $Q/items/bar-probe $([ -e "$Q/items/bar-probe" ] && printf exists || printf 'does not exist'). Every W1 assertion below this point is measuring a state the probe created."
fi

# Put the evidence back and restore the template as printed. W1 below admits C1 under that
# template, so a restore that silently failed reddens there rather than passing quietly here.
mv "$tmp/noev/$noev_who".*.txt "$Q/claims/C1/evidence/" 2>/dev/null || :
rmdir "$tmp/noev" 2>/dev/null || :
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

# =============================================================================================
# B1. A valid extra auditor must not hide an earlier TRUE auditor that claim admit refuses.
#
# The panel is written once, approved exactly once, and never recast. It requires a1+a2 and casts
# a3 as an optional extra. a1 and a3 are independently sealed; a2 records TRUE from a later sealed
# dispatch while its earliest dispatch remains unsealed. The pre-fix counter reaches the bar with
# a1+a3, so triage says ADMIT and cycle says NEXT PROPOSE even though claim admit refuses on a2.
# =============================================================================================
PROGRAM=b1-one-panel
b1_repo="$tmp/target-b1"
mkdir -p "$b1_repo"
(
  cd "$b1_repo"
  git init -q -b "$BASE"
  git config user.name test
  git config user.email test@example.invalid
  git config commit.gpgsign false
  printf 'baseline\n' > tracked.txt
  git add tracked.txt
  git commit -qm baseline
  "$C" adopt "$PROGRAM" --managed >/dev/null
)
Q="$b1_repo/.crucible/$PROGRAM"
cd "$b1_repo"
write_agents "$Q"
write_panel_md "$Q"
cat > "$Q/PANEL.ASSIGN.tsv" <<EOF
role${tab}agent${tab}required${tab}notes
coordinator${tab}c0${tab}yes${tab}this session
claim-auditor${tab}a1${tab}yes${tab}required auditor
claim-auditor${tab}a2${tab}yes${tab}required auditor
claim-auditor${tab}a3${tab}no${tab}optional extra auditor
scout${tab}sc1${tab}yes${tab}search existing behavior
maker${tab}mk1${tab}yes${tab}implementation
reviewer${tab}j1${tab}yes${tab}independent review
contract-auditor${tab}ca1${tab}yes${tab}contract seal
EOF
b1_approvals=0
"$Q/crucible" cycle approve-panel >/dev/null
b1_approvals=$((b1_approvals + 1))
b1_panel_before=$(cksum "$Q/agents.tsv" "$Q/PANEL.md" "$Q/PANEL.ASSIGN.tsv" "$Q/PANEL.APPROVAL")
printf 'An uncounted required auditor must block readiness.\n' > "$b1_repo/report.md"
"$Q/crucible" cycle problem "$b1_repo/report.md" >/dev/null
b1_cn=$("$Q/crucible" claim add 'an uncounted required auditor does not block readiness' \
  'An uncounted required auditor must block readiness.' DEFECT)

b1_seal() {
  b1_id=$1
  "$Q/crucible" attempt transport "$b1_id" multi-agent >/dev/null
  "$Q/crucible" contract-audit "$b1_id" ca1 PASS >/dev/null
}

for b1_who in a1 a2 a3; do
  "$Q/crucible" run-claim "$b1_cn" "$b1_who" -- sh -c 'echo reproduced' >/dev/null
  "$Q/crucible" dispatch "$b1_cn" claim-auditor "$b1_who" >/dev/null
  b1_attempt=$(sed -n 's/^attempt-id: //p' \
    "$Q/claims/$b1_cn/dispatches/1-claim-auditor-$b1_who.md" | head -1)
  if [ "$b1_who" = a2 ]; then
    b1_refused_attempt=$b1_attempt
    "$Q/crucible" dispatch "$b1_cn" claim-auditor "$b1_who" >/dev/null
    b1_attempt=$(sed -n 's/^attempt-id: //p' \
      "$Q/claims/$b1_cn/dispatches/2-claim-auditor-$b1_who.md" | head -1)
    b1_later_attempt=$b1_attempt
  fi
  b1_seal "$b1_attempt"
  "$Q/crucible" claim verdict "$b1_cn" "$b1_who" TRUE >/dev/null
done
"$Q/crucible" run-claim "$b1_cn" sc1 -- sh -c 'echo searched' >/dev/null
"$Q/crucible" dispatch "$b1_cn" scout sc1 >/dev/null
b1_scout_attempt=$(sed -n 's/^attempt-id: //p' \
  "$Q/claims/$b1_cn/dispatches/1-scout-sc1.md" | head -1)
b1_seal "$b1_scout_attempt"
"$Q/crucible" claim scout "$b1_cn" PARTLY-EXISTS sc1 >/dev/null
b1_panel_after=$(cksum "$Q/agents.tsv" "$Q/PANEL.md" "$Q/PANEL.ASSIGN.tsv" "$Q/PANEL.APPROVAL")
b1_raw=$(raw_true_files "$b1_cn")
b1_bar=$(admit_bar_now)
if [ "$b1_approvals" -eq 1 ] && [ "$b1_panel_before" = "$b1_panel_after" ] \
  && [ "$b1_raw" -eq 3 ] && [ "$b1_bar" -eq 2 ] \
  && [ ! -e "$Q/attempts/$b1_refused_attempt/transport" ] \
  && [ -e "$Q/attempts/$b1_later_attempt/transport" ]; then
  ok
else
  bad "B1 fixture: expected exactly one panel approval with no recast, three TRUE files against bar 2, and a2 earliest unsealed/later sealed — approvals=$b1_approvals raw=$b1_raw bar=$b1_bar earliest=$b1_refused_attempt later=$b1_later_attempt"
fi
b1_triage=$("$Q/crucible" triage 2>&1) || true
b1_rec=$(printf '%s\n' "$b1_triage" | sed -n 's/^ *-> //p' | head -1)
b1_cycle=$("$Q/crucible" cycle 2>&1) || true
set +e
b1_admit=$("$Q/crucible" claim admit "$b1_cn" b1-probe 2>&1)
b1_admit_rc=$?
set -e
case $b1_rec in
  "INDEPENDENCE INCOMPLETE — 3 TRUE on file, 2 counted across 2 kind(s); need 2 across 1. a2 resolves to attempt $b1_refused_attempt, which has no transport"*) b1_triage_safe=1 ;;
  *) b1_triage_safe= ;;
esac
b1_cycle_want='NEXT INVESTIGATE — independently fact-check every unresolved claim (FALSE/STALE closes a claim; admit needs 2 sealed TRUE to create work)'
if [ -n "$b1_triage_safe" ] && [ "$b1_cycle" = "$b1_cycle_want" ] \
  && [ "$b1_admit_rc" -ne 0 ] \
  && printf '%s\n' "$b1_admit" | grep -q "refused: attempt $b1_refused_attempt has no transport"; then
  ok
else
  bad "B1: one panel approval with no recast must report the exact earliest-attempt blocker on triage, cycle, and claim admit — triage: ${b1_rec:-none}; cycle: $(printf '%s\n' "$b1_cycle" | head -1); claim admit: $b1_admit"
fi

# B1 grammar control and mutation. Once the earliest a2 attempt is sealed, all three canonical
# line-1 TRUE files must be gate-visible. Then remove the optional third verdict and put leading
# prose ahead of a2's otherwise valid TRUE header. Physical line 2 is not a verdict: triage,
# cycle, and admission must all report the same one-TRUE shortfall rather than advance.
b1_seal "$b1_refused_attempt"
b1_control_triage=$("$Q/crucible" triage 2>&1) || true
b1_control_rec=$(printf '%s\n' "$b1_control_triage" | sed -n 's/^ *-> //p' | head -1)
b1_control_cycle=$("$Q/crucible" cycle 2>&1) || true
set +e
b1_control_admit=$(CRUCIBLE_MIN_AUDITORS=4 \
  "$Q/crucible" claim admit "$b1_cn" b1-grammar-control 2>&1)
b1_control_admit_rc=$?
set -e
if [ "$b1_control_rec" = 'ADMIT, NARROWED — only the gap the scout could not find.' ] \
  && [ "$b1_control_cycle" = 'NEXT PROPOSE — no ABSENT capability; propose DOCS-ONLY or live-observation' ] \
  && [ "$b1_control_admit_rc" -ne 0 ] \
  && printf '%s\n' "$b1_control_admit" | grep -q "refused: $b1_cn has 3 TRUE verdicts, need 4"; then
  ok
else
  bad "B1 grammar control: canonical line-1 TRUE must remain eligible on every surface — triage: ${b1_control_rec:-none}; cycle: $(printf '%s\n' "$b1_control_cycle" | head -1); admit probe: $b1_control_admit"
fi

mv "$Q/claims/$b1_cn/verdicts/a3.md" "$tmp/b1-a3.md"
{
  printf 'leading prose is not a verdict header\n'
  cat "$Q/claims/$b1_cn/verdicts/a2.md"
} > "$tmp/b1-a2.md"
mv "$tmp/b1-a2.md" "$Q/claims/$b1_cn/verdicts/a2.md"
b1_malformed_triage=$("$Q/crucible" triage 2>&1) || true
b1_malformed_rec=$(printf '%s\n' "$b1_malformed_triage" | sed -n 's/^ *-> //p' | head -1)
b1_malformed_cycle=$("$Q/crucible" cycle 2>&1) || true
set +e
b1_malformed_admit=$("$Q/crucible" claim admit "$b1_cn" b1-grammar-probe 2>&1)
b1_malformed_admit_rc=$?
set -e
if [ "$(sed -n '1p' "$Q/claims/$b1_cn/verdicts/a2.md")" = 'leading prose is not a verdict header' ] \
  && [ "$(sed -n '2p' "$Q/claims/$b1_cn/verdicts/a2.md")" = 'CLAIM-VERDICT: TRUE' ] \
  && [ "$b1_malformed_rec" = 'MORE AUDIT — 1 TRUE across 1 kind(s); need 2 across 1.' ] \
  && [ "$b1_malformed_cycle" = "$b1_cycle_want" ] \
  && [ "$b1_malformed_admit_rc" -ne 0 ] \
  && printf '%s\n' "$b1_malformed_admit" | grep -q "refused: $b1_cn has 1 TRUE verdicts, need 2"; then
  ok
else
  bad "B1 grammar: leading prose plus TRUE on line 2 must be non-verdict on triage, cycle, and claim admit — triage: ${b1_malformed_rec:-none}; cycle: $(printf '%s\n' "$b1_malformed_cycle" | head -1); claim admit: $b1_malformed_admit"
fi

# --- this suite cleans up after itself --------------------------------------------------------
cd "$HERE"
rm -rf "$tmp"
trap - 0 1 2 15
[ ! -e "$tmp" ] && ok || bad "this suite left its own scratch directory behind: $tmp"

summary
[ "$FAIL" -eq 0 ]
