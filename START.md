# START — execute one problem-to-done cycle

If `drive` is running, read `STATUS.md` after `cycle` and perform only the single next legal
orchestrator action. Conversational “keep looping” is not a waiver to implement.

You are the coordinating agent only. This file is self-contained. `RULES.md`, `LOOP.md`, role files,
and the managed-lifecycle guide are references to consult when their gate or role becomes relevant;
do not make the operator read them.

## How to use an installed cycle

The program is `.crucible/<program>/crucible` (after `adopt work --managed`, `<program>` is `work`).
Cwd is the **target repository root**, not the program directory. Install, refresh, and
`--next` copy-paste: [docs/install.md](docs/install.md).

| Who | Command | Why |
| --- | --- | --- |
| Anyone / resume | `.crucible/<program>/crucible cycle` | One durable next state; rewrites `STATUS.md` |
| Operator / babysit | `.crucible/<program>/crucible drive` | Outer loop: cycle → one coordinator tick → cycle. `drive tick` is one iteration |
| Operator | `<newer-source>/crucible adopt <program> --refresh` | Additive engine update. Does not delete local adapters such as `scripts/acp-brief.py`. Cwd is the target repository. |
| Human only | `cycle approve-panel`, `cycle approve` | Panel and proposal. Drive never auto-approves |
| Human only | `cycle problem FILE --next` | After this investigation should end: same panel, archive under `history/`, bind a new PROBLEM. Drive never invents the next problem. |
| Human only | act on `ESCALATE` / cleanup | Independence stop, overdue, or `cycle clean --dry-run` after you are finished with the program |

`STATUS.md` is the next-action card (`state`, `engine`, active item, inflight attempt, last evidence,
human gate). Read it after every `cycle`. `WAIT PANEL`, `WAIT APPROVAL`, `ESCALATE`, and `DONE` are
human gates — stop and show the operator the exact line. On `DONE`, the operator binds the next
PROBLEM with `cycle problem FILE --next` or previews cleanup. Do not recast the panel.

Resume after every restart, compaction, agent return, review, or repository change with:

    .crucible/<program>/crucible cycle

It reports one state: `CONFIGURE`, `WAIT PANEL`, `INTAKE`, `INVESTIGATE`, `PROPOSE`, `APPROVAL`,
`PLAN`, `EXECUTE`, `REVIEW`, `ESCALATE`, or `DONE`. Use `help protocol` yourself when a low-level
transition is required. If the operator started `drive`, do not start a second conversational
implementation loop.

## Onboard (configure before investigate)

Re-establish repository truth: root, branch, HEAD, dirty state, instructions, active work, architecture,
and test entrypoints. Trust repository state over conversational memory.

You are the **coordinator**. You do **not** implement work and you do **not** author review verdicts.
If you implement and judge, the panel is void.

Ask **one compact configure block** (not drip questions; not self-answered material config).
It has two required halves — agents **and** persona casting:

**A) Agents inventory**

1. Which agents/products can I use, and how is each invoked?
2. What kinds/models and effort defaults?

**B) Role casting (who plays which persona on independent agents)**

3. Which roles are active this cycle?
4. For each role, which registered agent plays it?
5. Confirm coordinator (this session) is not maker or reviewer.

**C) Risk + isolation**

6. How reversible / what risk posture is this work?
7. Isolation preference: multi-agent, ACP, or (only if ACP fails) subagents?
8. Any waivers (same-family, single-product ladder, maker-reviewer-same-agent)?

Write real invocations into ignored `agents.tsv`. Write `PANEL.md` covering:

- `## Agents`
- `## Roles` (human summary of casting)
- `## Risk posture`
- `## Isolation transport`
- `## Independence ladder`
- `## Waivers`

And write authoritative casting in `PANEL.ASSIGN.tsv`:

```text
role	agent	required	notes
coordinator	…	yes	this session; not maker/reviewer
claim-auditor	…	yes
maker	…	yes
reviewer	…	yes	≠ maker
contract-auditor	…	yes
```

Show inventory + casting and stop until the operator approves (`cycle approve-panel`). Placeholder
`agents.tsv` rows (`MODEL`, `AGENT_CLI`, `OTHER_CLI`) refuse progress. Missing or invalid casting
refuses progress. Do not invent agents or cast yourself as maker/reviewer.

### Independence ladder

1. **Multi-agent** (≥2 products/CLIs) preferred whenever available.
2. **ACP** isolated sessions when only one product (for example Kiro) is present.
3. **Host subagents** only after a recorded ACP probe failure; label weaker isolation.
4. **STOP** with `INDEPENDENCE_UNAVAILABLE` if none can be invoked — never silent solo theatre.

Record machine invocations in ignored `agents.tsv`; role standards belong in role files.

## Investigate and propose

Treat the report as allegations. Split it into atomic, source-traceable claims. Dispatch independent
auditors (multi-agent or ACP-isolated) to inspect current code, tests, behavior, history, and evidence;
then search for behavior that already exists. Classify each claim as confirmed, false, stale, already
present, partly present, or unverifiable.

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

Every role dispatch:

1. Generate a file contract (`dispatch`) under a **current** approved panel.
2. While DISPATCHED, record transport (`attempt transport … multi-agent|acp|subagent`).
3. Run **contract-auditor** → `contract-audit ATTEMPT AUDITOR PASS|FIX|STOP` (still DISPATCHED).
4. On FIX the attempt is SUPERSEDED — rewrite the contract via **redispatch**; on STOP escalate
   `INDEPENDENCE_UNAVAILABLE` — do not do the role yourself.
5. Only after PASS: if `drive` is running, **do not** start the worker — drive executes the
   exact `agents.tsv` command, records `attempt start` with the pid, waits, and records
   finish. If drive is not running, invoke that command yourself, then `attempt start|finish`.
6. Record observed outcomes (`result`) after the worker returns. Do not write verdicts for
   another role.

Dispatch makers into isolated contexts/worktrees. Bind attempts and evidence to the current work id.
Give reviewers the approved contract, current work, and evidence—not maker rationale. A rejection
returns concrete findings to the maker. Repeat the make → verify → review → fix loop until every
approved criterion and finding is resolved against the current integrated work.

Reuse unchanged expensive evidence. Allow one infrastructure retry. Repeated findings, unchanged-work
resubmission, ownership conflict, exhausted retry, missing independence, or a required human decision
is `ESCALATE`, not permission for unbounded agent churn.

## Finish

`DONE` means **this PROBLEM has no admittable claim**. Report what changed, what proves it, remaining
uncertainty, and exact repository state. When attempts exist, `cycle` writes an `INDEPENDENCE.md`
receipt (process summary, not a close gate).

Do not persist personal memory. Do not recast the panel. Do not bind the next PROBLEM yourself.

- Another problem on the same panel: stop and tell the operator to run
  `cycle problem FILE --next` (see [docs/install.md](docs/install.md)).
- Program finished: run `cycle clean --dry-run` and show the exact preview. Run
  `cycle clean --apply` only after explicit approval.
