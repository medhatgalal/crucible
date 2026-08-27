# Install, refresh, and use

**Read this first.** Humans and agents: cwd is always the **target repository root**,
not this documentation tree and not `.crucible/<program>/`.

| Situation | Section |
| --- | --- |
| No `.crucible/<program>/PROGRAM` | [First install](#first-install) |
| Program exists; `engine:` older than this tree | [Upgrade](#upgrade-an-installed-program) |
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
`PANEL.ASSIGN.tsv`. Every agent named in `PANEL.ASSIGN.tsv` needs a row in
`agents.tsv` — **including the coordinator**. Without a coordinator row,
`approve-panel` refuses with `PANEL.md / PANEL.ASSIGN.tsv incomplete`.

The admit bar is `max(2, required=yes claim-auditor rows)` sealed TRUE verdicts from distinct
registered agents, across at least `CRUCIBLE_MIN_KINDS` model families (default 1). A TRUE from a
sealed claim-auditor or scout attempt is eligible. With one required claim-auditor row, one
claim-auditor TRUE plus one scout TRUE satisfies the default floor. Three required claim-auditor
rows raise the floor to three; two eligible TRUEs then make `claim admit` refuse with
`refused: C1 has 2 TRUE verdicts, need 3`.

A `triage` `ADMIT` does not guarantee that `claim admit` will accept the claim. Keep each TRUE
agent's `run-claim` evidence, keep the panel current, and use transport valid under the current
independence ladder. `subagent` transport requires a recorded ACP-probe failure. When TRUE verdicts
are on file but are not eligible, `triage` reports `INDEPENDENCE INCOMPLETE` and names the recovery.
The close bar reads `required=yes` `reviewer` rows with no floor: one row closes on one PASS.
`CRUCIBLE_MIN_AUDITORS` (no default of its own) and `CRUCIBLE_MIN_JUDGES` (default 2, overridden by
the reviewer row count on a guided cycle) override those numbers. Scout is required on a guided
cycle — cast it in the initial block ([CONFIGURE.md](../CONFIGURE.md)).
Human:

```sh
.crucible/work/crucible cycle approve-panel
.crucible/work/crucible cycle problem /abs/path/to/report.md
.crucible/work/crucible drive
```

Human gates after that: `WAIT APPROVAL`, `ESCALATE`, `DONE`, live write envelopes.
`drive` starts sealed `agents.tsv` workers. Do not launch an ACP adapter such as
`.crucible/<program>/scripts/acp-brief.py` yourself while drive is running — the drive
parent runs the `agents.tsv` line. Crucible does not ship that adapter; see
[CONFIGURE.md](../CONFIGURE.md) for what it must do if you use the ACP path.

## Commit the program directory

`adopt` writes files and commits nothing. Evidence only outlives the chat that
produced it if it is in Git, so commit `.crucible/` in the target repository:

```sh
git add .crucible && git commit -m "chore: record program state"
```

`adopt` generates `.crucible/.gitignore` with `*/agents.tsv` and `*/worktrees/`, so
machine-local agent invocations and isolated worktrees stay out of the commit. Everything
else under `.crucible/<program>/` — `PROBLEM.md`, `CLAIMS.md`, `PROPOSAL.md`, `APPROVAL`,
`PANEL*`, `claims/`, `items/`, `attempts/`, `history/` — is the durable record. Commit
again after each human gate; `cycle clean` preserves these files but nothing restores them
if they were never committed.

## Upgrade an installed program

This is the only upgrade section and it applies to any earlier version, not only 1.3.x.
Cwd = target repo.

**Stop the driver first.** `--refresh` replaces the engine binary in place, and nothing in
the engine refuses a refresh under a running loop:

```sh
.crucible/work/crucible drive stop
```

`drive stop` releases a leftover `.crucible/<program>/.drive.lock`, reclaims attempts whose
pid is dead, and
prints `live-attempt: <id> RUNNING pid <pid> (not reclaimed)` for one that is still alive.
If it names a live attempt, let that worker finish before refreshing. That lock is a
directory: `drive stop` releases it with `rmdir` and prints `released …/.drive.lock`, and a
regular file at the same path is not a lock — `drive stop` prints `no .drive.lock` and
leaves it alone.

Then run from the **newer** source (this checkout or a newer tarball):

```sh
<path-to-newer-crucible>/crucible adopt work --refresh
<path-to-newer-crucible>/crucible adopt prkey --refresh   # every live program
.crucible/work/crucible cycle
```

Confirm **after** `cycle`, not before: `STATUS.md` `engine:` must equal the `VERSION` of
the source you refreshed from. `drive` must be a verb. `--managed` is install-only; do not
pass it with `--refresh`.

Seeing the old `engine:` immediately after `--refresh` is expected and is not a failed
upgrade. `adopt` never writes `STATUS.md`, so the card keeps whatever the last `cycle`
wrote until the next `cycle` rewrites it. What proves the refresh landed is `adopt`'s own
`refreshed engine <old> -> <new>` line and `.crucible/<program>/VERSION`. Confirming
`engine:` before running `cycle` reads a stale card.

**Overwrites:** `crucible`, `VERSION`, `START.md`, `BOOTSTRAP.md`, `RULES.md`,
`LOOP.md`, `CONFIGURE.md`, `roles/*.md`, `scripts/*.sh` (except release packagers),
top-level `docs/*.md`.

**Keeps:** `PROGRAM`, `PANEL*`, `agents.tsv`, `PROBLEM.md`, `CLAIMS.md`,
`PROPOSAL.md`, `APPROVAL`, `STATE*`, `items/`, `claims/`, `attempts/`,
`history/`, `LESSONS.md`, and any operator-written adapter at
`.crucible/<program>/scripts/acp-brief.py` — inside the program directory, which is
where `--refresh` looks and what it reports as `kept local adapter: scripts/acp-brief.py`.
An adapter at the repository's own `scripts/acp-brief.py` is outside the program directory
and is not covered by that promise (see [CONFIGURE.md](../CONFIGURE.md) — Crucible does not
ship one and `--refresh` never creates it).

If a refreshed engine is bad, stop `drive` and run the same `adopt <program> --refresh` command from
an older known-good tag or extracted release, then run the installed program's `cycle`. Refreshing
back replaces the engine-owned files in **Overwrites** and leaves the evidence and approved panel in
**Keeps** untouched.

The copied scripts include `verify-demand.sh`. It passes, and it is not a gate: it records
assertions about a known hole in admission, so a green run proves nothing about your cycle.
Do not treat it as one of the install checks.

Do not copy the program directory by hand.

| You see | Do |
| --- | --- |
| `engine:` equal to the source `VERSION`, after a `cycle` | Already current |
| `engine:` below the source `VERSION`, after a `cycle` | `--refresh` from that source |
| `engine:` below the source `VERSION`, before any `cycle` | Expected after `--refresh`; run `cycle`, then read it again |
| `engine: unknown` / no `STATUS.md` | Pre-1.3.5, or never `cycle`d since install; `--refresh` then `cycle` |
| `unknown verb: drive` | Pre-1.3.0; `--refresh` |
| `is a husk (no PROGRAM)` | Not a program. Do not `--refresh`. Keep, trash, or adopt another name |
| `unknown verb: reclaim` / `evidence` | Engine older than 1.4.0; `--refresh` |
| `crucible result` exits 128 printing nothing, on the first maker result of a git-target item | Engine older than 1.6.3; `--refresh` |
| `refused: maker dispatch requires plan-audit PASS` | Not an install problem. Run `plan-audit SLUG AUDITOR PASS` first — [START.md](../START.md) |
| `maker result requires current work`, or evidence named `…NOBRANCH.txt` | The item's `ai/<slug>` work branch does not exist. Create it before the maker dispatch — [START.md](../START.md) |

From 1.3.6, drive starts sealed workers. From 1.3.7, `dispatch ITEM judge` stays
`judge`. From 1.4.0: `attempt reclaim`, `evidence archive`, panel-bound judge/auditor
minima, `worth:` on STATUS, `--next` reclaims dead RUNNING pids. From 1.5.0:
one-predicate claims with polarity; verdicts append (history); `IN-FLIGHT` scout;
`phase REVIEW BUILD` after judge FIX; close work-id must be HEAD/last PASS;
`--next` writes `PANEL.CONTEXT.md`. From 1.6.0: at most 3 NEW claims;
FILE refuses 8+ CLI-verb catalogs; `drive stop`; guided `plan-audit PASS`
before maker dispatch. From 1.6.2: isomorphic STALE/FALSE copy does not start
a sibling worker; `adopt NAME --managed --panel-from SRC` copies an approved
panel onto a sibling cycle. From 1.6.3: the first maker `result` on a
git-target item checks owned paths against the target base and either passes or
refuses with a reason. Before that the dispatch work id was `NOBRANCH` until a
work branch existed, it reached `git diff` unvalidated, and the engine exited
128 with no message printed at all.

## Next problem (same panel)

```sh
.crucible/work/crucible cycle problem /abs/path/to/next-report.md --next
```

Archives under `history/`. Keeps the panel. Refuses `ACTIVE`/`BLOCKED` items and
**live** `RUNNING`/`OVERDUE` pids. Dead RUNNING pids are reclaimed (`STOPPED`)
then `--next` continues. Leftover `DISPATCHED` does not block. Drive never binds
the next problem. Do not swap agents here — that is the whole of "do not recast"
([CONFIGURE.md](../CONFIGURE.md)); correcting a casting mistake mid-cycle is allowed and
needs `cycle approve-panel` again.

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
stops and never `--apply`. Then `--next` or `--apply` (human only). The card's last line
names what cleanup is *not* for; read those names off the card the engine prints rather
than from this page. Full cleanup story, including the
mid-cherry-pick worktree recovery: [managed-lifecycle.md](managed-lifecycle.md#session-cleanup).

`STATUS.md` `worth:` is `BUILD` (scout ABSENT), `DOCS` (only PARTLY-EXISTS),
`NO-BUILD` (no ABSENT/PARTLY), or `UNKNOWN` (investigation incomplete).

## Falsifier pair

Closure refuses unless the item's falsifier was recorded by `crucible run` at the current work id
in both directions — once failing with the mechanism removed, once passing with it restored — and
the gate reads those two files rather than running the falsifier itself.

