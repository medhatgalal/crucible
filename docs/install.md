# Install, refresh, and use

Human and AI front door for getting a cycle onto a target repository and keeping the
engine current. Protocol details stay in [START.md](../START.md) and
[drive.md](drive.md).

Cwd for every command below is the **target repository root**, not this documentation
tree and not `.crucible/<program>/`.

## Fresh install

Need a local Crucible source (clone or extracted `crucible-<version>.tar.gz`). Verify
it once:

```sh
<path-to-crucible>/scripts/verify-agent-cycle.sh
```

Then install the default guided program named `work`:

```sh
<path-to-crucible>/crucible adopt work --managed
```

That copies the engine into `.crucible/work/` (including `VERSION`) and seeds
`PROGRAM`, `STATE.tsv`, `CLAIMS.md`, and a template `PROBLEM.md`. It does **not**
approve a panel or bind a problem.

A second program name is a second cycle (`adopt b3 --managed`). Do not reuse a name;
that refuses. To update an existing program, use `--refresh`.

## Confirm the installed engine

```sh
.crucible/work/crucible cycle
```

That rewrites `.crucible/work/STATUS.md`. After 1.3.5 the file includes `engine:`
from `.crucible/work/VERSION`.

| You see | Meaning |
| --- | --- |
| `engine: 1.3.5` (or current) | This program is running that engine |
| `engine: unknown` or no `STATUS.md` | Pre-1.3.5 install; refresh |
| `crucible: unknown verb: drive` | Pre-1.3.0 engine; refresh |

The binary that must gain new verbs is `.crucible/work/crucible`, not a GitHub
release the agent has not installed.

## Refresh (additive)

From the **newer** Crucible source, cwd still the target repo:

```sh
<path-to-newer-crucible>/crucible adopt work --refresh
```

Refresh every program you actually run (`work`, `b3`, …). `--managed` is
install-only; do not pass it with `--refresh`.

**Overwrites:** `crucible`, `VERSION`, `START.md`, `BOOTSTRAP.md`, `RULES.md`,
`LOOP.md`, `CONFIGURE.md`, `roles/*.md`, `scripts/*.sh` (except release packagers),
`docs/*.md`.

**Keeps:** `PROGRAM`, `PANEL*`, `agents.tsv`, `PROBLEM.md`, `CLAIMS.md`,
`PROPOSAL.md`, `APPROVAL`, `STATE*`, `items/`, `claims/`, `attempts/`,
`history/`, `LESSONS.md`, and extra local scripts such as `scripts/acp-brief.py`.

Do not copy the program directory by hand. That deletes adapters and can leave a
pre-`drive` binary.

After refresh: `.crucible/work/crucible cycle` and read `engine:`.

## Bind a problem and run

First problem on a new program (panel must already be approved):

```sh
.crucible/work/crucible cycle approve-panel
.crucible/work/crucible cycle problem /abs/path/to/report.md
```

This PROBLEM is finished or should be abandoned, same panel, new investigation:

```sh
.crucible/work/crucible cycle problem /abs/path/to/next-report.md --next
```

`--next` archives the current investigation under `.crucible/work/history/` and
keeps the panel. It refuses while an item is `ACTIVE`/`BLOCKED` or an attempt is
`RUNNING`/`OVERDUE`. Leftover `DISPATCHED` (never started) does not block.
Drive never invents the next problem. Do not recast the panel.

Babysit the coordinator:

```sh
.crucible/work/crucible drive        # until a human gate or no-progress stop
.crucible/work/crucible drive tick   # one iteration
```

| Who | Runs |
| --- | --- |
| Operator | `adopt` / `--refresh`, `drive`, `cycle approve-panel`, `cycle approve`, `cycle problem FILE [--next]`, `cycle clean` |
| Coordinator | `cycle`, then one legal action from `STATUS.md` |
| Maker / reviewer / auditor | only their dispatched contract |

`WAIT PANEL`, `WAIT APPROVAL`, `ESCALATE`, and `DONE` are human gates. Drive
never auto-approves. Conversational “keep looping” is not implement and not merge.

After `DONE`, either bind the next PROBLEM (`--next`) or preview cleanup
(`cycle clean --dry-run`). Apply cleanup only with explicit approval.

## Cold start vs already installed

| Situation | Read |
| --- | --- |
| No `.crucible/*/START.md` | [BOOTSTRAP.md](../BOOTSTRAP.md) |
| Program exists | [START.md](../START.md) and `STATUS.md` |
| Engine looks stale | This page, Refresh |
| Outer loop behavior | [drive.md](drive.md) |
