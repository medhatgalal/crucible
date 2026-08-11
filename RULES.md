# RULES

Each rule is labelled. **CHECK** means `./crucible` refuses when it is violated — you cannot
proceed. **RULE** means it is only words. Words here have a bad record: every written rule in this
program's history was broken by its own author, usually within hours. When you find a way to convert
a RULE into a CHECK, do it and say so in the lesson.

## Evidence

1. **RULE — Record evidence with the tool, never by hand.**
   `.crucible/<program>/crucible run <item> <you> -- <cmd>` captures the command, its exit status and its output, and
   stamps the work id and your name inside the file. Relabelled from CHECK after a review proved it: a hand-written file carrying the same
   header is accepted. The header makes accidental hand-writing obvious; it is not a signature,
   and under one user nothing in files can make it one.
2. **CHECK — Evidence is bound to the work it describes.** The work id is in the filename *and*
   inside the file. Renaming stale evidence to look current is refused.
3. **CHECK — Absence fails.** No work, no evidence, no falsifier, no verdict: refusal, never a pass.
   A check that is most confident when it knows least is inverted.
4. **CHECK — A PASS must name evidence its author recorded.** A verdict that cites nothing it ran
   itself is refused.
5. **RULE — Claims carry their source inline.** Not "the tests pass" but "the tests pass:
   `evidence/j1.2.<wid>.txt`, exit 0". If a claim has no artifact, label it ASSUMPTION.
6. **RULE — Closure needs a falsifier.** "It works" is worthless. "It works, and when I undo the
   change this named check fails, and here is that output" is proof. Every item's `ITEM.md` names
   its falsifier before any build starts; the gate refuses the template placeholder.

## Independence

7. **CHECK — Only registered agents may judge.** A verdict from a name not in `agents.tsv` is not a
   verdict.
8. **CHECK — The maker may not judge its own work.**
9. **CHECK — Two verdicts that are byte-identical are one verdict.**
10. **CHECK — A judge's brief contains the goal, the work and the recorded evidence. It contains no
    maker report, transcript, or rationale**, because the brief is generated from a whitelist and
    those files are not on it.
11. **RULE — Never tell a judge what not to flag, and never pre-rate a finding's severity.** If you
    believe a finding will be a false positive, let it be raised and adjudicate it. A dispatch
    containing "don't worry about", "at most minor", or "the plan chose this" is you sparing
    yourself a review loop.
12. **RULE — Prefer different kinds of agent for judging over more of the same kind.** The only
    measured result in this program's history: one cross-kind judge found a false file path that
    three same-kind reviewers had each repeated. Set `CRUCIBLE_MIN_KINDS=2` when it matters.

## Roles and dispatch

13. **RULE — Generate the dispatch before the call.** Relabelled from CHECK: an item can
    close with no dispatch on record, so nothing enforces that a verdict was preceded by one. `.crucible/<program>/crucible dispatch` writes it.
14. **RULE — One instruction per call: "read this file and follow it exactly."** Everything else
    belongs in the contract. If you find yourself explaining in the conversation, the contract is
    incomplete — fix the file.
15. **RULE — A role may only read what its role file grants and write what it names.** The
    orchestrator dispatches and records; it does not implement, judge, or merge.
16. **RULE — Dispatch is not completion.** A task handed to an agent has not happened until its
    output file exists. "In flight" is not a status. Quote the reply or report it lost.

## Memory

17. **RULE — One writer per fact.** Each fact has exactly one path. Relabelled from CHECK: the gate
    refuses a verdict from an unregistered name and evidence whose internal agent disagrees with its
    filename, but a correctly formatted foreign write by a registered name is accepted.
18. **RULE — Durable before spoken.** Write the decision to its file before you say it in a
    conversation. The single worst failure in this program's history was a priority communicated
    verbally while the written record said otherwise; an automated reviewer read the record,
    correctly concluded the work was off-plan, and stopped it two minutes before it would have
    succeeded.
19. **RULE — recorded state is the resume point.** In a managed program, `STATE.tsv` is authoritative
    and `STATE.md` is generated. In an item-file program, `STATE.md` remains the operator-maintained
    resume summary. After a restart or compaction, re-read the recorded state and `git log`; never
    re-dispatch work it marks complete. Nothing can force you to consult it, so this remains a rule.
20. **CHECK — Every closed item appends exactly one lesson**, and `LESSONS.md` is concatenated into
    every later maker brief. A lesson nobody reads is theatre; this makes reading it structural.

## Work quality

21. **RULE — Search before building.** Before any design, a scout must answer: does code already do
    this, fully or partly? Report what you searched and how, so the claim can be checked. Search by
    behaviour, not by name — the thing you would build rarely contains the words you would call it.
22. **RULE — Fit the architecture you are in, or argue to change it explicitly.** Do not silently
    introduce a second pattern for a solved problem. If the existing architecture is wrong for this
    work, say so, propose the change, and let it be judged as a change.
23. **RULE — Build only what the item asks for.** No speculative abstraction, no unrequested
    configurability. A bug fix does not need its neighbourhood refactored.
24. **RULE — Tests assert behaviour, not mocks.** A test that cannot fail is a lie with a green tick.
25. **RULE — Name things for behaviour.** No invented identifier schemes, no version numbers in
    names. Identifiers leak from private notes into code, tests, and user-visible strings.
26. **RULE — Security and performance are acceptance criteria, not afterthoughts.** If the item
    touches auth, data, or a hot path, its criteria say so before the build starts, or the judge has
    nothing to check them against.

## Stopping

27. **RULE — Bound your iteration.** The same finding twice, or an unchanged work id resubmitted,
    should stop and escalate. Relabelled from CHECK: nothing counts findings or resubmissions, so
    this is discipline, not a mechanism. Occupancy is not output; something running for hours
    while nothing lands is a stall wearing a costume.
28. **RULE — A blocked agent stops and says so.** `BLOCKED` and `NEEDS_CONTEXT` are legitimate
    terminal results with a recorded reason. Guessing under a no-questions rule is a defect
    generator. Bad work is worse than no work and you will not be penalised for escalating.
