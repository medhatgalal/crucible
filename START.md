# START — execute one problem-to-done cycle

You are the coordinating agent. This file is self-contained. `RULES.md`, `LOOP.md`, role files, and the
managed-lifecycle guide are references to consult when their gate or role becomes relevant; do not make
the operator read them.

Resume after every restart, compaction, agent return, review, or repository change with:

    .crucible/<program>/crucible cycle

It reports one state: `INTAKE`, `INVESTIGATE`, `PROPOSE`, `APPROVAL`, `PLAN`, `EXECUTE`, `REVIEW`,
`ESCALATE`, or `DONE`. Use `help protocol` yourself when a low-level transition is required.

## Onboard

Re-establish repository truth: root, branch, HEAD, dirty state, instructions, active work, architecture,
and test entrypoints. Trust repository state over conversational memory.

Inspect the available collaboration mechanisms and propose the smallest useful panel in one message:
coordinator, investigator/scout, maker, reviewer, and—only for higher-risk or disputed work—adversary.
The same model may fill several roles in fresh isolated contexts, labelled `same-family review`.
Different-family review is preferred for behavioral, security, data, migration, irreversible, or
repeatedly disputed work. Do not fake independence with different filenames.

Record machine invocations in ignored `agents.tsv`; role standards belong in role files. Ask only about
material choices you cannot discover. Do not interview the operator one question at a time.

## Investigate and propose

Treat the report as allegations. Split it into atomic, source-traceable claims. Independently inspect
current code, tests, behavior, history, and evidence; then search for behavior that already exists.
Classify each claim as confirmed, false, stale, already present, partly present, or unverifiable.

Write one `PROPOSAL.md` containing exactly:

- `## Verified problem`
- `## Proposed outcome`
- `## Non-goals`
- `## Backlog`
- `## Verification`

Separate facts, inferences, and recommendations. Remove false/stale work and narrow partial work to the
actual gap. Show the proposal and evidence to the operator, then stop. Do not plan or build until the
operator explicitly approves it. Record approval through the cycle protocol; any proposal edit
invalidates the approval.

## Plan, make, review, fix

Admit one narrow approved item. Define literal owned paths, one to three acceptance criteria,
discriminating falsifiers, bounded checks, risk, dependencies, and stop conditions. Use a task graph
only for genuinely disjoint ownership. A fresh reviewer validates the breakdown before build.

Dispatch makers into isolated contexts/worktrees. Bind attempts and evidence to the current work id.
Give reviewers the approved contract, current work, and evidence—not maker rationale. A rejection
returns concrete findings to the maker. Repeat the make → verify → review → fix loop until every
approved criterion and finding is resolved against the current integrated work.

Reuse unchanged expensive evidence. Allow one infrastructure retry. Repeated findings, unchanged-work
resubmission, ownership conflict, exhausted retry, or a required human decision is `ESCALATE`, not
permission for unbounded agent churn.

## Finish

`DONE` requires current evidence, resolved findings, accounted approved scope, integration/CI tied to
the reviewed work id, and an accepting refusal gate. Report what changed, what proves it, remaining
uncertainty, and exact repository state.

Do not persist personal memory. After `DONE`, run `cycle clean --dry-run` and show the exact preview.
Run `cycle clean --apply` only after explicit approval; it removes machine-only configuration and safely
unregisters isolated worktrees while preserving branches, work, reviews, and evidence.
