# LOOP

Crucible is one outer-to-inner loop, not a menu of commands.

```text
CONFIGURE → WAIT PANEL → INTAKE → INVESTIGATE ⇄ challenge → PROPOSE → APPROVAL
                                                              ↓
                DONE ← REVIEW ⇄ FIX ← EXECUTE ← validated PLAN
                 │       │
                 │       └────────────→ ESCALATE when bounded recovery ends
                 │                      (includes INDEPENDENCE_UNAVAILABLE)
                 └─ cycle problem FILE --next (human) → INTAKE   same panel
                    cycle problem --abandon REASON (human) → INTAKE  no PASS
                    adopt NAME --managed --panel-from SRC (human) → sibling INTAKE
                      leftover PROBLEM stays on SRC; drive never --nexts
```

`crucible cycle` derives the current position from repository artifacts. The coordinating agent uses
the lower-level protocol; the operator does not advance phases. `crucible drive` is the outer loop
that re-runs cycle, invokes the coordinator once per tick, and stops for human gates so a chat
cannot skip cycle or implement.

## CONFIGURE and WAIT PANEL

Discover available agents (CLI, ACP, subagent). Ask the operator for (1) agent inventory and
(2) **persona/role casting** — which independent agent plays coordinator, claim-auditor, maker,
reviewer, contract-auditor, and optional roles. Write real `agents.tsv` rows, `PANEL.md`, and
authoritative `PANEL.ASSIGN.tsv`. Show inventory + casting and stop until `cycle approve-panel`.
Placeholders and incomplete casting refuse progress. Guided dispatches must match the casting table.

Exit: content-bound approval of panel + casting; registry is non-placeholder.

## INTAKE

Preserve the supplied problem in `PROBLEM.md`. Establish repository, runtime, instructions, dirty state,
history, and active-work truth.

Exit: the problem is explicit enough to investigate without guessing.

## INVESTIGATE

Split the report into atomic claims. Fact-check current behavior from source, tests, commands, history,
and existing artifacts using independent agents (multi-agent or ACP-isolated). Independently search for
behavior that already exists. False, stale, duplicate, or already-solved claims remain recorded but do
not become work.

Exit: every material claim has evidence and an existing-work disposition, or is explicitly unverifiable.

## PROPOSE and APPROVAL

Synthesize verified facts into `PROPOSAL.md`: actual problem, intended outcome, non-goals, bounded
backlog, and verification. The operator reviews one coherent proposal. Approval is recorded against its
content hash; editing it invalidates approval.

Exit: explicit human approval of the current proposal. No maker work is legal before this point.

## PLAN

Admit one narrow item. Define owned paths, dependencies, risk, acceptance criteria, discriminating
falsifiers, bounded checks, review cost, and stop conditions. A fresh review attacks the breakdown.

Exit: the breakdown is independently judged executable and non-overlapping.

## EXECUTE ⇄ REVIEW ⇄ FIX

Makers work only in their assigned scope. Reviews see the approved contract, current work, and evidence,
not maker rationale. Each dispatch has transport + contract-audit PASS on guided cycles. A rejection is
a finding, not a phase failure: fix it, produce new current-work evidence, and review again. Selective
adversarial/cross-family review applies when risk or dispute warrants.

The loop stops for a typed escalation after one infrastructure retry, repeated finding, unchanged-work
resubmission, ownership conflict, independence unavailable, or required human decision. A stop is
cheaper and more truthful than unbounded agent churn.

Exit: every approved criterion and finding is resolved against the current integrated work id.

## DONE

The refusal gate accepts closure, relevant integration/CI evidence is current, the approved backlog is
accounted for, structure/independence receipts exist when attempts were used, and the final report
distinguishes shipped work from remaining uncertainty. Agent contexts can disappear without losing truth
because the repository holds the evidence.

`DONE` means **this PROBLEM has no admittable claim** (every claim is STALE, FALSE, FULLY-EXISTS, or
below the admit bar). Leftover `items/` directories that are not in `STATE.tsv` are not work. Drive
stops here and does not invent the next investigation.

The panel stays. The operator starts the next problem-to-done pass with
`cycle problem FILE --next` (archives the closed investigation under `history/`, keeps
`PANEL*` and `agents.tsv`). Recasting the panel is not required.

## Falsifier pair

Closure refuses unless the item's falsifier was recorded by `crucible run` at the current work id
in both directions — once failing with the mechanism removed, once passing with it restored — and
the gate reads those two files rather than running the falsifier itself.

