# Install, refresh, and use

**Read this first.** Humans and agents: cwd is always the **target repository root**,
not this documentation tree and not `.crucible/<program>/`.

| Situation | Section |
| --- | --- |
| No `.crucible/<program>/PROGRAM` | [First install](#first-install) |
| Program exists; `engine:` older than this tree | [Upgrade](#upgrade-from-1-3-x) |
| Directory exists but no `PROGRAM` | Husk — keep, trash, or `adopt` a **different** name. Do not `--refresh`. |
| Cycle already bound | [START.md](../START.md) and `STATUS.md` |

Protocol: [START.md](../START.md). Outer loop: [drive.md](drive.md).

## First install

Need a Crucible source (clone or `crucible-<version>.tar.gz` from the GitHub release).

```sh
<path-to-crucible>/scripts/verify-agent-cycle.sh
<path-to-crucible>/crucible adopt work --managed
.crucible/work/crucible cycle
```

That copies the engine into `.crucible/work/` (`VERSION`, `START.md`, roles, docs) and
seeds `PROGRAM`, `STATE.tsv`, `CLAIMS.md`, template `PROBLEM.md`. It does **not**
approve a panel or bind a problem.

A second name is a second cycle (`adopt prkey --managed`). Reusing a name refuses.
When leftover DONE still occupies `SRC` and real work must start without
`--next`ing it away: `adopt NAME --managed --panel-from SRC` copies the
approved panel and leaves `SRC`'s PROBLEM in place.

Then the coordinator (not the operator) writes `agents.tsv`, `PANEL.md`,
`PANEL.ASSIGN.tsv`. Cast **enough required claim-auditors and reviewers** for the
admit/close bars (defaults follow required rows). Human:

```sh
.crucible/work/crucible cycle approve-panel
.crucible/work/crucible cycle problem /abs/path/to/report.md
.crucible/work/crucible drive
```

Human gates after that: `WAIT APPROVAL`, `ESCALATE`, `DONE`, live write envelopes.
`drive` starts sealed `agents.tsv` workers. Do not paste `acp-brief.py` while drive
is running.

## Upgrade from 1.3.x

Cwd = target repo. Run from the **newer** source (this checkout or a newer tarball):

```sh
<path-to-newer-crucible>/crucible adopt work --refresh
<path-to-newer-crucible>/crucible adopt prkey --refresh   # every live program
.crucible/work/crucible cycle
```

`STATUS.md` must show `engine: 1.4.0` (or current). `drive` must be a verb.
`--managed` is install-only; do not pass it with `--refresh`.

**Overwrites:** `crucible`, `VERSION`, `START.md`, `BOOTSTRAP.md`, `RULES.md`,
`LOOP.md`, `CONFIGURE.md`, `roles/*.md`, `scripts/*.sh` (except release packagers),
`docs/*.md`.

**Keeps:** `PROGRAM`, `PANEL*`, `agents.tsv`, `PROBLEM.md`, `CLAIMS.md`,
`PROPOSAL.md`, `APPROVAL`, `STATE*`, `items/`, `claims/`, `attempts/`,
`history/`, `LESSONS.md`, `scripts/acp-brief.py`.

Do not copy the program directory by hand.

| You see | Do |
| --- | --- |
| `engine: 1.4.0` | Already current |
| `engine: 1.3.7` or older | `--refresh` from this tree |
| `engine: unknown` / no `STATUS.md` | Pre-1.3.5; `--refresh` |
| `unknown verb: drive` | Pre-1.3.0; `--refresh` |
| `is a husk (no PROGRAM)` | Not a program. Do not `--refresh`. Keep, trash, or adopt another name |
| `unknown verb: reclaim` / `evidence` | Engine older than 1.4.0; `--refresh` |

From 1.3.6, drive starts sealed workers. From 1.3.7, `dispatch ITEM judge` stays
`judge`. From 1.4.0: `attempt reclaim`, `evidence archive`, panel-bound judge/auditor
minima, `worth:` on STATUS, `--next` reclaims dead RUNNING pids. From 1.5.0:
one-predicate claims with polarity; verdicts append (history); `IN-FLIGHT` scout;
`phase REVIEW BUILD` after judge FIX; close work-id must be HEAD/last PASS;
`--next` writes `PANEL.CONTEXT.md`. From 1.6.0: at most 3 NEW claims;
FILE refuses 8+ CLI-verb catalogs; `drive stop`; guided `plan-audit PASS`
before maker dispatch. From 1.6.2: isomorphic STALE/FALSE copy does not start
a sibling worker; `adopt NAME --managed --panel-from SRC` copies an approved
panel onto a sibling cycle.

## Next problem (same panel)

```sh
.crucible/work/crucible cycle problem /abs/path/to/next-report.md --next
```

Archives under `history/`. Keeps the panel. Refuses `ACTIVE`/`BLOCKED` items and
**live** `RUNNING`/`OVERDUE` pids. Dead RUNNING pids are reclaimed (`STOPPED`)
then `--next` continues. Leftover `DISPATCHED` does not block. Drive never binds
the next problem. Do not recast.

To start real work **without** discarding the leftover PROBLEM, install a sibling:

```sh
<path-to-crucible>/crucible adopt live --managed --panel-from work
.crucible/live/crucible cycle problem /abs/path/to/real-report.md
```

`--panel-from` copies `agents.tsv`, `PANEL.md`, `PANEL.ASSIGN.tsv`, and
`PANEL.APPROVAL`. It does not copy `PROBLEM.md`. Drive never `--next`s.

Stale item evidence (work-id ≠ current): `crucible evidence archive SLUG` then `check`.

## Who runs what

| Who | Runs |
| --- | --- |
| Operator | `adopt` / `--refresh`, `drive`, `cycle approve-panel`, `cycle approve`, `cycle problem FILE [--next]`, `cycle problem --abandon REASON`, `cycle clean`, `evidence archive` |
| Coordinator | `cycle`, dispatch, transport, `contract-audit` — never start ACP after seal |
| Drive parent | Sealed worker `agents.tsv` command, `attempt start` / finish. One worker per `drive tick`. Does not invoke the coordinator while a sealed worker exists. |
| Maker / reviewer / auditor | only their contract |

`WAIT PANEL`, `WAIT APPROVAL`, `ESCALATE`, and `DONE` stop drive. Conversational
“keep looping” is not implement.

After `DONE`: **`cycle clean --dry-run` is the next verb** (CLEANUP card). Drive
stops and never `--apply`. Then `--next` or `--apply` (human only). Jira leftovers
and ignored `.validation/` are not cycle clean.

`STATUS.md` `worth:` is `BUILD` (scout ABSENT), `DOCS` (only PARTLY-EXISTS),
`NO-BUILD` (no ABSENT/PARTLY), or `UNKNOWN` (investigation incomplete).
