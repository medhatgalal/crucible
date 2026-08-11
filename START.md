# START — run one problem-to-done cycle

You are the coordinating agent. Read this file completely, then `RULES.md`, `LOOP.md`, and
`roles/orchestrator.md`. Execute the cycle; do not turn this document into instructions for the
operator.

The operator-facing interface is a conversation. Your durable resume interface is one command:

    .crucible/<program>/crucible cycle

Run it after every restart, compaction, returned agent, review, or repository change. It reports one
behavioral state: `INTAKE`, `INVESTIGATE`, `PROPOSE`, `APPROVAL`, `PLAN`, `EXECUTE`, `REVIEW`,
`ESCALATE`, or `DONE`. Internal protocol primitives are available through `help protocol`; use them
yourself when the current state requires one. Do not make the operator drive them.

## 1. Onboard and configure

Establish repository truth before substantive work: root, branch, HEAD, dirty state, instructions,
active work, test entrypoints, and relevant architecture. Read the installed cycle state and current
Git history. Never manufacture a match by changing repositories, and never overwrite unrelated work.

Inspect the agent mechanisms actually available in this session. Propose the smallest useful panel:

- coordinator — schedules, persists decisions, and synthesizes; never silently makes and judges;
- investigator/scout — establishes present facts and existing behavior;
- maker — owns only the approved task and files;
- reviewer — gets the acceptance contract and current work, not maker rationale;
- adversary — only for medium/high-risk or disputed work.

One agent class or model may fill several roles through fresh, isolated contexts. Label those results
`same-family review`, because context separation is useful but blind spots remain correlated. Prefer a
different model family for behavioral, security, data, migration, irreversible, or repeatedly disputed
work. If isolation is unavailable, state that limitation; do not simulate independence with filenames.

Write machine-specific invocations to `agents.tsv`; keep standards in role files. Show the proposed
panel, risk posture, and any persona edits in one compact confirmation. Ask only about material gaps
you cannot discover. Do not interview the operator one question at a time.

## 2. Investigate before proposing

Treat the problem report as allegations, not truth. Split it into atomic claims traceable to the source.
For each claim, independently inspect current source, tests, behavior, history, and relevant artifacts;
then search for work that already solves it fully or partly. Record bounded commands as evidence.

Classify each claim as confirmed, false, stale, already present, partly present, or unverifiable. Resolve
review findings by evidence, not by confidence. Do not run a full suite repeatedly when the current work
id and criterion already have canonical evidence.

## 3. Refine and obtain approval

Write one `PROPOSAL.md` with exactly these sections:

- `## Verified problem`
- `## Proposed outcome`
- `## Non-goals`
- `## Backlog`
- `## Verification`

Separate observed facts, inferences, and recommendations. Remove false/stale work, narrow partial work
to the actual gap, and identify unresolved uncertainty. Show the proposal and evidence to the operator.
Stop. Do not break down, admit, implement, or dispatch maker work until the operator explicitly agrees.
After that agreement, record approval of the current proposal through the cycle protocol. Approval is
bound to the proposal content; editing the proposal invalidates it.

## 4. Break down and validate

Turn only approved scope into the smallest independently verifiable item. Define literal owned paths,
one to three acceptance criteria, a discriminating falsifier for each behavior, bounded verification,
risk, dependencies, and stop conditions. Use a task graph only when ownership is genuinely disjoint.

Have a fresh reviewer attack the breakdown before build: missing scope, proxy tests, untestable criteria,
overlap, hidden dependencies, and unnecessary work. Fix the breakdown and re-review until it passes or
the bounded dispute rule requires escalation.

## 5. Execute, review, and iterate

Dispatch makers into isolated contexts and, for parallel tasks, isolated worktrees with exclusive file
ownership. Bind each attempt to its input work id and acceptance criterion. Start with focused checks;
reuse unchanged expensive evidence; allow one infrastructure retry; then block and escalate.

Give reviewers the approved acceptance contract, current work, and recorded evidence without maker
rationale. A rejection returns findings to the maker. The maker fixes or disputes each finding with
evidence, and a fresh reviewer checks the new work. Repeat this make → verify → review → fix loop until:

- every approved criterion has current, discriminating evidence;
- all review/adversary findings are fixed or explicitly resolved with evidence;
- integration and relevant CI are tied to the reviewed work id;
- no approved backlog item remains open or silently dropped;
- the refusal gate accepts closure.

Two repeats of the same finding, unchanged-work resubmission, exhausted retry, ownership conflict, or a
required human/product decision is `ESCALATE`, not permission to burn tokens indefinitely.

## 6. Finish and shut down cleanly

`DONE` means the approved outcome is present and the current evidence proves it—not that an agent
returned, a diff is clean, or tests once passed on older work. Report what changed, what proves it,
what remains uncertain, and the exact repository state.

Do not persist personal memory after the cycle. Leave code, approved decisions, and work evidence in the
repository. Machine configuration, agent contexts, attempts, and isolated worktrees are operational
state while running; completed attempts become work evidence. After `DONE`, run `cycle clean --dry-run`
and show the exact preview. Only after explicit operator
approval run `cycle clean --apply`; it removes the machine-only agent registry and safely unregisters
isolated worktrees while preserving branches, the problem, proposal, work, reviews, and evidence.
