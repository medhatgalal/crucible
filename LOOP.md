# LOOP

Crucible is one outer-to-inner loop, not a menu of commands.

```text
INTAKE → INVESTIGATE ⇄ challenge → PROPOSE → APPROVAL
                                          ↓
              DONE ← REVIEW ⇄ FIX ← EXECUTE ← validated PLAN
                       │
                       └────────────→ ESCALATE when bounded recovery ends
```

`crucible cycle` derives the current position from repository artifacts. The coordinating agent uses
the lower-level protocol; the operator does not advance phases.

## INTAKE

Preserve the supplied problem in `PROBLEM.md`. Establish repository, runtime, instructions, dirty state,
history, and active-work truth. Propose a small panel based on available coordination mechanisms.

Exit: the problem and configuration are explicit enough to investigate without guessing.

## INVESTIGATE

Split the report into atomic claims. Fact-check current behavior from source, tests, commands, history,
and existing artifacts. Independently search for behavior that already exists. False, stale, duplicate,
or already-solved claims remain recorded but do not become work.

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
not maker rationale. A rejection is a finding, not a phase failure: fix it, produce new current-work
evidence, and review again. Selective adversarial/cross-family review applies when risk or dispute warrants.

The loop stops for a typed escalation after one infrastructure retry, repeated finding, unchanged-work
resubmission, ownership conflict, or required human decision. A stop is cheaper and more truthful than
unbounded agent churn.

Exit: every approved criterion and finding is resolved against the current integrated work id.

## DONE

The refusal gate accepts closure, relevant integration/CI evidence is current, the approved backlog is
accounted for, and the final report distinguishes shipped work from remaining uncertainty. Agent contexts
can disappear without losing truth because the repository holds the evidence.
