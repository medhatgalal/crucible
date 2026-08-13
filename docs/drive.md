# Drive — Ralph-style outer loop

`crucible drive` is an outer driver for **guided + managed** cycles. It exists so a coordinator
cannot skip `cycle` or implement. Every existing independence CHECK still applies: drive only
calls the same `dispatch` / `cycle` verbs.

Conversational “keep looping” is not a waiver to implement.

A cold reader of this repository: start at [BOOTSTRAP.md](../BOOTSTRAP.md) to install, then
[START.md](../START.md) for the installed prompt. This page is only the outer driver.

## Who runs what

| Actor | Runs | Does not run |
| --- | --- | --- |
| Operator | `adopt`, `drive` / `drive tick`, `cycle approve-panel`, `cycle approve`, `cycle clean` | protocol verbs (`dispatch`, `attempt`, `result`) |
| Coordinator process | `cycle`, then one legal orchestrator action from `STATUS.md` | product edits, verdicts, merges, auto-approve |
| Maker / reviewer / auditor | only their dispatched contract | admit the next backlog row, merge, wear another hat |

## What one tick does

1. Run `cycle` first and rewrite `STATUS.md` (and print the line).
2. If the line is `WAIT PANEL`, `WAIT APPROVAL`, `ESCALATE`, or `DONE`: print the exact human
   action and exit. Drive never auto-approves.
3. Otherwise invoke the **cast coordinator** in a **new process** with the same brief every time:
   read `START.md` and `STATUS.md`, run `cycle`, do the single next legal orchestrator action,
   write nothing a maker/reviewer/auditor should write, exit.
4. After the child exits, `cycle` again. Stop on no-progress (same status and no new
   evidence/dispatch twice), an overdue attempt, or independence STOP. A coordinator
   command that exits non-zero (missing adapter, ACP crash) is a **refuse**, not STOP.

```text
.crucible/<program>/crucible drive        # loop until a human gate or stop
.crucible/<program>/crucible drive tick   # one iteration (tests and babysitting)
```

## Legal coordinator actions

Drive does **not** invoke makers, reviewers, or auditors. It babysits the coordinator: one
orchestrator action per tick, then `cycle`. After a dispatch, the operator or a later tick
must actually launch the worker named in the contract.

| State | What drive does | Label |
| --- | --- | --- |
| INVESTIGATE | If the child did not add a dispatch file, parent dispatches the next missing claim-auditor/scout contract | CHECK |
| PROPOSE | Invokes coordinator; they may write `PROPOSAL.md`. Parent does not write the proposal | RULE |
| PLAN, no ACTIVE item | Invokes coordinator; they may admit one item. Parent does not invent a slug | RULE |
| PLAN, DRAFT/READY | Invokes coordinator; do not dispatch a maker. Parent does not dispatch a reviewer here (judge dispatch requires REVIEW) | RULE |
| EXECUTE idle | If BUILD and no inflight, parent may `dispatch` the maker (prints the invocation; does not start the process) | fallback |
| WAIT inflight | Refuses a new live attempt id (DISPATCHED/RUNNING/OVERDUE) that was not in the pre-tick set | CHECK |
| REVIEW RETURNED | If the inflight maker attempt is RETURNED and the item is REVIEW, parent may `dispatch` the judge | fallback |
| reviewer FIX | Not implemented as a parent fallback | RULE |

### What the implement CHECK covers

Refuse + restore when the coordinator process, during a tick:

- moves product `HEAD` (`git commit` or a completed merge) or leaves `MERGE_HEAD`
- adds product porcelain outside `.crucible/`, or **changes the contents** of an already-dirty
  product file (same porcelain line is not enough to hide it)
- writes under `worktrees/` (task worktrees are product work even though they sit in `.crucible/`)
- creates or **overwrites** `items/*/verdicts/*.md` or `claims/*/verdicts/*.md`
- deletes `cycle: guided` from `PROGRAM`
- introduces a **new** live attempt id while the cycle line is WAIT inflight

Drive is still a **dispatch babysitter**, not a make/review runner: it does not invoke makers.
A legitimate maker `TARGET` write during a coordinator tick is refused for the same reason.

## STATUS.md

Rewritten on every `cycle` and every drive tick:

- `line` / `state`
- `active-item`
- `inflight-attempt`
- `last-evidence`
- `next-human-gate`

Coordinators read it after `cycle`.

## Maker inner loop

Inside **EXECUTE** for the admitted item only, the **maker contract** (not the drive parent) may
say: run the falsifier until it passes, or stop after one infrastructure retry. That loop must
not admit the next backlog row and must not merge. Drive does not run that loop.
