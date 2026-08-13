# Drive — Ralph-style outer loop

`crucible drive` is an outer driver for **guided + managed** cycles. It exists so a coordinator
cannot skip `cycle` or implement. Every existing independence CHECK still applies: drive only
calls the same `dispatch` / `cycle` verbs.

Conversational “keep looping” is not a waiver to implement.

## What one tick does

1. Run `cycle` first and rewrite `STATUS.md` (and print the line).
2. If the line is `WAIT PANEL`, `WAIT APPROVAL`, `ESCALATE`, or `DONE`: print the exact human
   action and exit. Drive never auto-approves.
3. Otherwise invoke the **cast coordinator** in a **new process** with the same brief every time:
   read `START.md` and `STATUS.md`, run `cycle`, do the single next legal orchestrator action,
   write nothing a maker/reviewer/auditor should write, exit.
4. After the child exits, `cycle` again. Stop on no-progress (same status and no new
   evidence/dispatch twice), an overdue attempt, or independence STOP.

```text
.crucible/<program>/crucible drive        # loop until a human gate or stop
.crucible/<program>/crucible drive tick   # one iteration (tests and babysitting)
```

## Legal coordinator actions (CHECKs)

| State | Legal action |
| --- | --- |
| INVESTIGATE | Dispatch the next unaudited or unscouted claim only |
| PROPOSE | Write or update `PROPOSAL.md` only |
| PLAN, no ACTIVE item | Admit one item from the approved proposal |
| PLAN, DRAFT/READY | Review `ITEM.md`; do not dispatch a maker |
| EXECUTE idle | Dispatch the maker (worktree when the item has a task DAG) |
| WAIT inflight | Do not start a second attempt; observe and record |
| REVIEW RETURNED | Dispatch the reviewer with contract, work, and evidence — no maker rationale |
| reviewer FIX | Redispatch the maker with findings only |

Drive refuses (and restores the product tree) if the coordinator process:

- edits an owned product path (anything outside `.crucible/`)
- writes a verdict file
- merges

Those are CHECKs, not RULES.

## STATUS.md

Rewritten on every `cycle` and every drive tick:

- `line` / `state`
- `active-item`
- `inflight-attempt`
- `last-evidence`
- `next-human-gate`

Coordinators read it after `cycle`.

## Maker inner loop

Inside **EXECUTE** for the admitted item only, the maker brief may run the falsifier until it
passes or stop after one infrastructure retry. That loop must not admit the next backlog row
and must not merge.
