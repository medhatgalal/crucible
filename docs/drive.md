# Drive — Ralph-style outer loop

`crucible drive` is an outer driver for **guided + managed** cycles. It exists so a coordinator
cannot skip `cycle` or implement. Every existing independence CHECK still applies: drive only
calls the same `dispatch` / `cycle` verbs.

Conversational “keep looping” is not a waiver to implement.

A cold reader of this repository: start at [BOOTSTRAP.md](../BOOTSTRAP.md) or
[install.md](install.md) to install or refresh, then [START.md](../START.md) for the
installed prompt. This page is only the outer driver.

## Who runs what

| Actor | Runs | Does not run |
| --- | --- | --- |
| Operator | `adopt` / `adopt --refresh`, `drive` / `drive tick`, `cycle approve-panel`, `cycle approve`, `cycle problem FILE --next`, `cycle problem --abandon REASON`, `cycle clean` | protocol verbs (`dispatch`, `attempt`, `result`) |
| Coordinator process | `cycle`, then one legal orchestrator action from `STATUS.md` (dispatch + seal) | product edits, verdicts, merges, auto-approve, starting ACP when drive is running |
| Drive parent | After seal: run the exact `agents.tsv` line, `attempt start` pid, wait, `attempt finish` | write verdicts, mint approvals |
| Maker / reviewer / auditor | only their dispatched contract | admit the next backlog row, merge, wear another hat |

## What one tick does

1. Run `cycle` first and rewrite `STATUS.md` (and print the line).
2. If the line is `WAIT PANEL`, `WAIT APPROVAL`, `ESCALATE`, or `DONE`: print the exact human
   action and exit. Drive never auto-approves and never binds the next problem.
3. If a DISPATCHED attempt is already sealed, start that worker (`agents.tsv`), record pid,
   wait, finish. One worker per tick. Then cycle.
4. Otherwise invoke the **cast coordinator** to dispatch/seal. After the child, copy
   isomorphic PASS audits (`--like`) and start one newly sealed worker if any.
5. Stop on no-progress, overdue, or independence STOP. A coordinator command that exits
   non-zero (missing adapter, ACP crash) is a **refuse**, not STOP.

```text
.crucible/<program>/crucible drive        # loop until a human gate or stop
.crucible/<program>/crucible drive tick   # one sealed worker or one coordinator action
.crucible/<program>/crucible drive stop   # release leftover .drive.lock; reclaim dead pids
```

## Legal coordinator actions

Drive babysits the coordinator (dispatch + seal) and **starts one sealed worker per tick**.
The coordinator does not implement and does not write verdicts. After PASS, do not paste
`acp-brief.py` — the parent runs the `agents.tsv` line.

| State | What drive does | Label |
| --- | --- | --- |
| INVESTIGATE | Parent dispatches **all** unaudited claims to the first claim-auditor, records transport, seals one engine-template contract (no kiro hop), copies isomorphic PASS/`STALE`/`FALSE`, and starts **one** claim worker **only if that claim has no verdict yet**. Coordinator ACP is not invoked for this fallback. Empty-claim INVESTIGATE still uses the coordinator to split PROBLEM.md | CHECK |
| PROPOSE | Invokes coordinator; they may write `PROPOSAL.md`. Parent does not write the proposal | RULE |
| PLAN, no ACTIVE item | Invokes coordinator; they may admit one item. Parent does not invent a slug | RULE |
| PLAN, DRAFT/READY | Invokes coordinator; do not dispatch a maker. Parent does not dispatch a reviewer here (judge dispatch requires REVIEW) | RULE |
| EXECUTE idle | If BUILD and no inflight, parent may `dispatch` the maker; a later tick starts it once sealed | fallback |
| SEALED DISPATCHED | Parent runs `agents.tsv`, `attempt start` pid, wait, finish | CHECK |
| WAIT inflight | Refuses a new live attempt id (DISPATCHED/RUNNING/OVERDUE) that was not in the pre-tick set | CHECK |
| REVIEW RETURNED | If the inflight maker attempt is RETURNED and the item is REVIEW, parent may `dispatch` the judge | fallback |
| reviewer FIX | Judge `NEXT:FIX`: parent `phase ITEM BUILD` and may dispatch the maker. Coordinator does not start ACP after seal. | CHECK |

### What the implement CHECK covers

Refuse + restore when the coordinator process, during a tick:

- moves product `HEAD` (`git commit` or a completed merge) or leaves `MERGE_HEAD`
- adds product porcelain outside `.crucible/`, or **changes the contents** of an already-dirty
  product file (same porcelain line is not enough to hide it)
- writes under `worktrees/` (task worktrees are product work even though they sit in `.crucible/`)
- creates or **overwrites** `items/*/verdicts/*.md` or `claims/*/verdicts/*.md`
- deletes `cycle: guided` from `PROGRAM`
- introduces a **new** live attempt id while the cycle line is WAIT inflight

Drive **does** start a sealed worker (one per tick) after the coordinator exits. That start
is the parent, not the coordinator child, so a maker `TARGET` write during the coordinator
tick is still refused. The worker process may write its own verdicts after `attempt start`.

Isomorphic claim contracts (same role; claim id normalized) may share one contract-auditor
PASS via `contract-audit ATTEMPT AUDITOR PASS --like ATTEMPT2…`. Drive applies that copy
automatically. The auditor still issued the PASS; the coordinator does not stamp it.

## STATUS.md

Rewritten on every `cycle` and every drive tick:

- `line` / `state`
- `engine` (from the installed `VERSION`; `unknown` if the program was never refreshed)
- `worth` (`BUILD` / `DOCS` / `NO-BUILD` / `UNKNOWN`)
- `active-item`
- `inflight-attempt`
- `last-evidence`
- `next-human-gate`

Coordinators read it after `cycle`. If `engine:` is `unknown` or missing, the operator
must `--refresh` before `drive` can be trusted (see [install.md](install.md)).

## Maker inner loop

Inside **EXECUTE** for the admitted item only, the **maker contract** (not the drive parent) may
say: run the falsifier until it passes, or stop after one infrastructure retry. That loop must
not admit the next backlog row and must not merge. Drive does not run that loop.
