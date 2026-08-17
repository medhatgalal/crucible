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
8. **CHECK — No recorded maker may judge the item it helped build.** Managed parallel work keeps a
   durable maker set, so an earlier task maker cannot review the integrated item merely because a
   different maker ran later.
9. **CHECK — Two verdicts that are byte-identical are one verdict.**
10. **CHECK — A judge's brief contains the goal, the work and the recorded evidence. It contains no
    maker report, transcript, or rationale**, because the brief is generated from a whitelist and
    those files are not on it.
11. **CHECK in guided cycle — Panel and role casting are approved before investigation and
    execution.** Placeholder `agents.tsv`, incomplete `PANEL.md`, missing/invalid
    `PANEL.ASSIGN.tsv` (role→independent agent), or unapproved/stale panel refuse problem
    binding, claim intake, proposal approval, **dispatch, transport, contract-audit, attempt
    start, result, claim verdict, and claim scout**. Casting is checked against the live
    approved files; a stale panel hash refuses those commands (not only `cycle` status).
    Coordinator may not also be claim-auditor or contract-auditor; contract-auditor may not
    be a maker; maker ≠ reviewer unless an explicit `WAIVER:` / `LADDER_WAIVER:` line says so.
12. **CHECK in guided cycle — Managed results and claim verdicts require transport +
    contract-audit PASS.** Each attempt records `multi-agent|acp|subagent` while DISPATCHED.
    Subagent requires a recorded ACP probe failure (or an explicit `ACP: unavailable` line when
    no prior `probe-acp ok` exists). Missing independence is refused, not silently converted
    into solo theatre. Every guided claim verdict and scout result needs a sealed claim attempt.
13. **RULE — Never tell a judge what not to flag, and never pre-rate a finding's severity.** If you
    believe a finding will be a false positive, let it be raised and adjudicate it. A dispatch
    containing "don't worry about", "at most minor", or "the plan chose this" is you sparing
    yourself a review loop.
14. **RULE — Prefer different kinds of agent for judging over more of the same kind.** The only
    measured result in this program's history: one cross-kind judge found a false file path that
    three same-kind reviewers had each repeated. Set `CRUCIBLE_MIN_KINDS=2` when it matters.
    Prefer multi-agent products; on single-product hosts prefer ACP isolation before host subagents.

## Roles and dispatch

15. **CHECK in managed lifecycle; RULE in item-file lifecycle — Generate the dispatch before the
    call.** A managed result is bound to a recorded attempt and refuses without it. An item-file
    item can still close with no dispatch on record, so `.crucible/<program>/crucible dispatch`
    remains operator discipline there.
16. **RULE — One instruction per call: "read this file and follow it exactly."** Everything else
    belongs in the contract. If you find yourself explaining in the conversation, the contract is
    incomplete — fix the file.
17. **RULE — A role may only read what its role file grants and write what it names.** The
    orchestrator dispatches and records; it does not implement, judge, or merge. The
    contract-auditor checks file contracts and may STOP independence-unavailable attempts.
18. **CHECK in managed lifecycle; RULE in item-file lifecycle — Dispatch is not completion.** A
    managed dispatch records an in-flight attempt and `next` waits for its observed outcome; it does
    not advance the item. In item-file lifecycle, a task handed to an agent has not happened until
    its output file exists. Quote the reply or report it lost.

## Durable state, not agent memory

19. **RULE — One writer per fact.** Each fact has exactly one path. Relabelled from CHECK: the gate
    refuses a verdict from an unregistered name and evidence whose internal agent disagrees with its
    filename, but a correctly formatted foreign write by a registered name is accepted.
20. **RULE — Durable before spoken.** Write the decision to its file before you say it in a
    conversation. The single worst failure in this program's history was a priority communicated
    verbally while the written record said otherwise; an automated reviewer read the record,
    correctly concluded the work was off-plan, and stopped it two minutes before it would have
    succeeded.
21. **CHECK in guided+managed drive — Parent always cycles; human gates do not auto-approve;
    coordinator implement paths refuse.** `drive` runs `cycle` (rewrites `STATUS.md`) before and
    after each coordinator process and re-checks `cycle: guided` every tick. `WAIT PANEL`,
    `WAIT APPROVAL`, `ESCALATE`, and `DONE` exit without invoking the coordinator.
    Drive never binds the next PROBLEM (`cycle problem FILE --next` is a human gate).
    Drive starts at most one sealed DISPATCHED worker per tick (`agents.tsv` command,
    `attempt start` with the observed pid, wait, `attempt finish` RETURNED|TIMEOUT|STOPPED).
    Drive does not write verdicts. `cycle approve-panel` and `cycle approve` refuse while
    `.drive.lock` exists. After the child,
    these refuse (and restore): product `HEAD` movement (commit or completed merge), `MERGE_HEAD`,
    new or content-changed product porcelain (including already-dirty files), task-worktree
    writes, new or overwritten `items/*/verdicts` and `claims/*/verdicts` files, removing
    `cycle: guided`, and a new live attempt id while the cycle is WAIT inflight. INVESTIGATE
    fallback dispatches one missing claim-auditor/scout contract. Conversational “keep looping”
    is a RULE, not this CHECK.
22. **RULE — recorded state is the resume point.** In a managed program, `STATE.tsv` is authoritative
    and `STATE.md` is generated. In an item-file program, `STATE.md` remains the operator-maintained
    resume summary. After a restart or compaction, re-read the recorded state and `git log`; never
    re-dispatch work it marks complete. Nothing can force you to consult it, so this remains a rule.
23. **CHECK — Every closed item appends exactly one cycle lesson or `NONE`**, and `LESSONS.md` is
    concatenated into later maker briefs. This is repository evidence, not global model memory, and may
    be included in an operator-approved cleanup after the cycle.
24. **RULE — Never persist project lessons or personas in global agent memory.** A fresh agent relearns
    the workflow from repository files. Agent contexts and machine configuration are disposable.

## Work quality

25. **RULE — Search before building.** Before any design, a scout must answer: does code already do
    this, fully or partly? Report what you searched and how, so the claim can be checked. Search by
    behaviour, not by name — the thing you would build rarely contains the words you would call it.
26. **RULE — Fit the architecture you are in, or argue to change it explicitly.** Do not silently
    introduce a second pattern for a solved problem. If the existing architecture is wrong for this
    work, say so, propose the change, and let it be judged as a change.
27. **RULE — Build only what the item asks for.** No speculative abstraction, no unrequested
    configurability. A bug fix does not need its neighbourhood refactored.
28. **RULE — Tests assert behaviour, not mocks.** A test that cannot fail is a lie with a green tick.
29. **RULE — Name things for behaviour.** No invented identifier schemes, no version numbers in
    names. Identifiers leak from private notes into code, tests, and user-visible strings.
30. **RULE — Security and performance are acceptance criteria, not afterthoughts.** If the item
    touches auth, data, or a hot path, its criteria say so before the build starts, or the judge has
    nothing to check them against.

## Stopping

31. **CHECK in managed lifecycle; RULE in item-file lifecycle — Bound your iteration.** Managed
    lifecycle blocks a repeated finding fingerprint, refuses duplicate current-work PASSes and a
    second canonical expensive PASS, and allows one retry after an observed timeout before
    `RETRY_EXHAUSTED`. Item-file lifecycle does not count findings or resubmissions. Occupancy is
    not output; something running for hours while nothing lands is a stall wearing a costume.
32. **RULE — A blocked agent stops and says so.** `BLOCKED`, `NEEDS_CONTEXT`, and
    `INDEPENDENCE_UNAVAILABLE` are legitimate terminal results with a recorded reason. Guessing under
    a no-questions rule is a defect generator. Bad work is worse than no work and you will not be
    penalised for escalating.
