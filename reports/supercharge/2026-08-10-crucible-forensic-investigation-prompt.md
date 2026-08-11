# Crucible forensic investigation prompt

```text
You are conducting a read-only forensic investigation of this repository. Do not edit source, dispatch agents, start long-running work, alter branches, kill processes, push, or clean up. You may create exactly one report:
reports/supercharge/2026-08-10-crucible-forensic-investigation.md

Goal: establish what Crucible actually is, what it was intended to do, what it currently does, why the recent multi-agent effort consumed roughly four days without closing its central outcome, and the smallest credible redesign that lets agents process a problem report through evidence-grounded triage, backlog creation, implementation, review, and closure.

Ground every material statement in fresh evidence. Separate:
- FACT: directly observed source, command output, git history, or recorded artifact.
- INFERENCE: a conclusion drawn from facts.
- RECOMMENDATION: a proposed change, never presented as current behavior.

Investigate in this order:

1. Establish repository truth
   - Record cwd, git root, branch, HEAD, dirty state, remotes, tracked versus ignored state, active runs, and all local instructions.
   - Inventory the CLI, roles, docs, self-tests, workflow, `.crucible` program state, item artifacts, and git history.
   - Build a concise map of the intended outer loop (report -> claims -> audit -> scout -> triage -> backlog) and inner loop (spec -> design -> tasks -> build -> verify -> adversary -> graduate).

2. Verify behavior, not prose
   - Trace every lifecycle transition and refusal in the shell implementation.
   - Compare each claimed guarantee with the enforcing code and a relevant test. Classify it as HARD CHECK, PROSE RULE, PARTIALLY ENFORCED, or UNPROVEN.
   - Identify proxy tests, regex/string assertions, parser assumptions, stale-evidence rules, and any claim that exceeds what the code checks.
   - Run only focused, bounded checks needed to establish a finding. Measure their duration. Do not rerun the full suite merely to duplicate existing evidence.

3. Reconstruct the four-day effort
   - Build a dated timeline from commits, item revisions, dispatches, run states, evidence, verdicts, adversary reports, and work-id changes.
   - Quantify output: commits, closed items, open items, returned/timeout/hung/undecided runs, repeated role cycles, and duplicated checks.
   - For every central failure, give: symptom, direct evidence, causal mechanism, impact, and whether it is confirmed or inferred.
   - Explain specifically why Opus struggled. Do not attribute system failures to model quality without evidence. Test these hypotheses:
     a. contradictory or over-constrained role instructions;
     b. passive prose rules mistaken for enforced workflow;
     c. every commit invalidating prior expensive evidence;
     d. one enormous item combining provider activation, documentation truthfulness, parser coverage, and test mechanics;
     e. reviewers repeatedly running slow, overlapping end-to-end falsifiers;
     f. no budget, cancellation, deduplication, or escalation protocol for long checks;
     g. high ceremony before a narrow behavioral decision;
     h. same-family reviews reinforcing framing rather than independently testing it.

4. State what went right and wrong
   - What went right: identify controls that exposed a false premise, prevented a false PASS, preserved durable evidence, or narrowed an overclaim.
   - What went wrong: distinguish a working safeguard that is too expensive from a safeguard that is merely theatre or a false proxy.
   - State the actual delivery position plainly: what has demonstrably shipped, what remains open, and what cannot yet be claimed.

5. Produce the replacement operating model
   - Design the simplest file-backed flow that preserves the valuable safeguards:
     report intake -> atomic claims -> independent fact check -> existing-work scout -> operator-approved backlog -> one narrowly bounded item -> implementation -> focused verification -> independent review -> close or escalate.
   - Define one authoritative state file, one owner for each artifact, explicit entry/exit criteria, and a minimal machine-readable result format.
   - Use role separation based on risk, not ritual:
     * coordinator: schedules and synthesizes, never silently implements or judges;
     * investigator/scout: establishes current facts;
     * maker: changes only its assigned files;
     * reviewer: independently tests acceptance criteria and does not read maker rationale;
     * adversary: used only for medium/high-risk or disputed work.
   - The same model may perform different roles only in fresh, isolated contexts and must be labelled “same-family review,” not independent. Require a different model family only for behavioral, security, data, migration, or repeatedly disputed work.
   - Introduce explicit budgets: one canonical check per work-id, evidence reuse when the tree and criterion are unchanged, bounded focused tests before full-suite tests, one retry for infrastructure failure, then BLOCKED/ESCALATE.
   - Require a falsifier for each acceptance criterion, but reject weak falsifiers that only test wording or a proxy rather than the intended behavior.
   - Keep the redesign to the minimum viable workflow; do not propose services, databases, or extra agents unless evidence shows files and shell are insufficient.

6. Deliver a prioritized remediation backlog
   For each item include: problem statement, evidence, risk, scope boundary, owner role, dependencies, smallest acceptance criterion, discriminating falsifier, verification command, estimated review cost, and stop condition.
   Prioritize by leverage: first unblock the core report-to-backlog flow, then make evidence/review economical, then address parser or documentation precision.

Required report sections:
1. Executive verdict
2. What Crucible is and its intended contract
3. Current implementation and enforcement map
4. Four-day timeline and outcome accounting
5. Why the workflow stalled, including the Opus analysis
6. What went right
7. What went wrong
8. Replacement operating model
9. Prioritized remediation backlog
10. Evidence appendix: commands, paths, SHAs, work IDs, and unresolved uncertainty

Finish with exactly one decision: PROCEED WITH REDESIGN PLAN, HOLD FOR HUMAN DECISION, or INSUFFICIENT EVIDENCE. Do not claim completion based on a clean diff or an agent’s self-report.
```
