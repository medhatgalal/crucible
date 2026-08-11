# Managed lifecycle protocol

This is the coordinating agent's low-level protocol reference. Operators use the conversational cycle
described in `START.md`; they do not advance attempts, tasks, or phases themselves.

Managed lifecycle makes program state machine-readable and gives the agent one deterministic resume
behavior. It is selected by behavior in `PROGRAM`:

```text
lifecycle: managed
```

There is no product version name. Programs without that field keep the item-file lifecycle so an
existing program does not change underneath active work.

## Install it for a new cycle

New cycles select the behavior during adoption:

```sh
<engine>/crucible adopt <program> --managed
```

That creates `STATE.tsv` and its generated `STATE.md` atomically with the installed program. The
`lifecycle enable` primitive remains only for converting an empty older installation before its first
item; it is not an onboarding step.

The dry-run changes nothing and prints the exact write set. Apply creates authoritative `STATE.tsv`,
generates `STATE.md`, and records `lifecycle: managed` in `PROGRAM` last. Enabling refuses after the
first item because conversion of active item-file programs is not implemented yet.

## Work an item

```sh
$CP add fix-report-flow "Make report triage produce an approved backlog"
```

Edit the printed `ITEM.md`. Keep its sections in order and replace every placeholder:

1. Goal
2. Non-goals
3. Risk (`LOW`, `MEDIUM`, or `HIGH`)
4. Owned files
5. One to three acceptance criteria
6. One bounded focused falsifier
7. Expensive evidence, or `NONE`
8. Stop conditions

Then advance only when the named work is actually complete:

```sh
$CP ready fix-report-flow
$CP phase fix-report-flow BUILD
$CP agents
$CP dispatch fix-report-flow maker <maker-name> A1 FOCUSED
```

`dispatch` writes a complete attempt contract before the agent is launched. Its arguments are:

```text
dispatch ITEM ROLE AGENT [CRITERION] [EVIDENCE_CLASS] [RETRY_OF]
```

`ROLE` is `maker`, `judge`, or `adversary`. `EVIDENCE_CLASS` is `FOCUSED`, `FULL_SUITE`,
`EXTERNAL`, or `MANUAL`. The command prints both the contract path and the registered invocation
from `agents.tsv`; Crucible records the work but does not launch the process.

Record the observed process lifecycle using the attempt id in the contract path:

```sh
ATTEMPT=A<epoch>.<pid>.<sequence>
$CP attempt start "$ATTEMPT" <observed-pid>

# The agent commits first, then records a focused check through the command in its contract:
# $CP run fix-report-flow <maker-name> -- <bounded-command>

$CP attempt finish "$ATTEMPT" RETURNED "launcher observed exit 0"
$CP result "$ATTEMPT" PASS <evidence-filename> CLOSE -
```

Results have a small, typed outcome contract:

| Outcome | Allowed next action | Finding fingerprint |
| --- | --- | --- |
| `PASS` | `CLOSE` | `-` |
| `REJECT` | `FIX` | required 12-character identifier |
| `BLOCKED` | `DECIDE` or `ESCALATE` | optional |
| `NEEDS_CONTEXT` | `DECIDE` or `ESCALATE` | optional |
| `SCOPE_CONFLICT` | `DECIDE` or `ESCALATE` | optional |

The evidence argument is one basename from the item's `evidence/` directory. It must have been
recorded by `crucible run` for the same agent, work id, and attempt id. A result is write-once. A
maker attempt records the work id it started from and its result records the new post-change work
id; a reviewer attempt refuses if the work changes while review is in progress.

Move to review only after the maker attempt has returned and its current-work PASS is recorded.
The transition refuses without that result:

```sh
$CP phase fix-report-flow REVIEW
$CP dispatch fix-report-flow judge <reviewer-name> A1 FOCUSED
```

The reviewer contract contains the item, work, and recorded evidence, but no maker rationale. Use
the same `attempt start`, `attempt finish`, and `result` sequence for the review. A judge result also
publishes the compatibility verdict used by `check`:

```sh
$CP check fix-report-flow
$CP close fix-report-flow "one durable lesson, or NONE"
```

Managed maker dispatch records every maker in `MAKERS.tsv`. Reviewer contracts and results label their
relation to those maker families as `SAME-FAMILY`, `CROSS-FAMILY`, or `MIXED-FAMILY`; every recorded maker
is barred from reviewing the integrated item.

## Deadlines, retries, and evidence reuse

Maker attempts default to 45 minutes and reviewer/adversary attempts to 30 minutes. Override the
recorded deadline for a dispatch with `CRUCIBLE_MAKER_SECONDS` or `CRUCIBLE_REVIEW_SECONDS`.

A passed deadline is not proof that a process timed out:

```sh
$CP attempt overdue "$ATTEMPT"       # only after the recorded deadline
$CP attempt finish "$ATTEMPT" TIMEOUT "launcher observed timeout"
```

`overdue` records `OVERDUE`; only an observed launcher outcome records `TIMEOUT`, `STOPPED`, or
`ABANDONED`. One matching retry is allowed after the first `TIMEOUT`:

```sh
$CP dispatch fix-report-flow maker <maker-name> A1 FOCUSED "$ATTEMPT"
```

A second timeout persists `RETRY_EXHAUSTED` and stops the item for operator action. A repeated
12-character REJECT fingerprint persists `REPEATED_FINDING`. For an unchanged work id, Crucible
also refuses a duplicate PASS by the same agent/role/criterion and refuses a second canonical
`FULL_SUITE` or `EXTERNAL` PASS. This lets later reviewers reuse the recorded expensive evidence
instead of rerunning it. After the first timeout, `next` prints the exact matching retry command;
it does not silently advance the lifecycle.

## Resume and inspect

At any point:

```sh
$CP next
cat .crucible/<program>/STATE.md
```

`next` reports `WAIT`, asks for a returned attempt's result, prints an available retry, exposes a
typed block, or prints the next lifecycle command. `STATE.tsv` is authoritative. `STATE.md` is
generated for people and must not be edited.

Each dispatch is preserved under `attempts/<attempt-id>/`:

- `meta.tsv` binds the immutable dispatch identity, criterion, input work id, evidence class,
  deadline, and retry source.
- `events.tsv` records observed lifecycle transitions.
- `contract.md` is the exact file given to the agent.
- `result.md` is the write-once, evidence-bound outcome.

After the guided cycle reports `DONE`, the coordinator can preview session cleanup with
`cycle clean --dry-run`. `--apply` is allowed only after DONE and refuses while any attempt is live.
It safely removes registered isolated worktrees and the machine-only `agents.tsv`, while preserving task
branches and all durable problem, proposal, work, review, and evidence artifacts.

## Optional task graph

When one item genuinely needs disjoint maker ownership, add `TASKS.tsv` and its task files before
`ready`:

```text
task_id	depends_on	paths_file	verify_script
T1	-	tasks/T1.paths	tasks/T1.verify.sh
T2	T1	tasks/T2.paths	tasks/T2.verify.sh
T3	T1	tasks/T3.paths	tasks/T3.verify.sh
```

Each `.paths` file contains one literal repository-relative owned path per line. Each verifier is
an executable POSIX shell script beginning with `#!/bin/sh`; it will receive the task commit as
`$1` when task execution is enabled. `ready` refuses malformed rows, duplicate or unknown task IDs,
dependency cycles, more than 32 tasks or 128 edges, unsafe/glob paths, prefix ownership overlap,
task paths outside the item's declared Owned files, missing files, symlinks, and non-executable
verifiers. It then freezes the graph as `TASKS.id` and generates `TASKS.md`.

Inspect the frozen graph without editing it. The coordinator may then dispatch dependency-ready tasks
to isolated Git worktrees and integrate their passing commits in stable graph order:

```sh
$CP task list fix-report-flow
$CP task ready fix-report-flow
$CP task dispatch fix-report-flow T1 <maker-name> A1 FOCUSED
$CP task integrate fix-report-flow
```

`task ready` exposes only tasks whose dependencies have PASS results. Any edit to the TSV, path
sets, verifier contents, or verifier mode after READY refuses. The current implementation
refuses item-wide maker dispatch for a task-graph item. Task dispatch creates an isolated worktree,
binds its base and owned paths into the attempt, caps live makers at three by default, runs the frozen
task verifier against the returned commit, and refuses changes outside task ownership. Integration
requires every task PASS, applies commits in stable dependency order, records `INTEGRATION.tsv`, and
blocks on ancestry or cherry-pick conflict. These are agent protocol primitives, not operator steps.

## Refusals that matter

Managed lifecycle refuses:

- dispatch outside the role's stage, an unregistered agent, or maker self-review;
- a second in-flight attempt for the current item or criterion;
- finishing before an observed start, inferring timeout solely from a deadline, or mutating a
  recorded result;
- a result with stale, missing, foreign-agent, or different-attempt evidence;
- incompatible outcome/next-action pairs, REJECT without a fingerprint, repeated findings, and
  retries beyond the single infrastructure retry;
- duplicate current-work PASSes and duplicate canonical expensive checks.
- managed closure when a compatibility verdict is not backed by its matching attempt result.

## What this behavior does not yet provide

Managed lifecycle currently owns item state, bounded attempts, typed results, retry stops,
economical reuse of unchanged expensive evidence, frozen task graphs, isolated task worktrees, owned
path enforcement, and stable integration. It does not provide conversion of active item-file programs,
automatic agent launching or cancellation, cryptographic proof of who authored an artifact, or proof
that a recorded command is a discriminating test. Task dispatch currently refuses a task with more than
one direct dependency; broader fan-in remains a separate integration behavior. The coordinating agent
supplies orchestration and human interaction; do not describe those as shell-enforced merely because
managed lifecycle is enabled.
