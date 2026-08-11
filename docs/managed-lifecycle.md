# Managed lifecycle

Managed lifecycle makes program state machine-readable and gives the operator one deterministic
resume command. It is selected by behavior in `PROGRAM`:

```text
lifecycle: managed
```

There is no product version name. Programs without that field keep the item-file lifecycle so an
existing program does not change underneath active work.

## Enable it

Enable managed lifecycle immediately after `adopt`, before adding the first item:

```sh
CP=.crucible/<program>/crucible
$CP lifecycle status
$CP lifecycle enable --dry-run
$CP lifecycle enable --apply
$CP lifecycle status
```

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

Managed lifecycle currently owns item state, bounded attempts, typed results, retry stops, and
economical reuse of unchanged expensive evidence. It does not yet provide task-DAG parallelism,
conversion of active item-file programs, automatic agent launching or cancellation, cryptographic
proof of who authored an artifact, or proof that a recorded command is a discriminating test.
Those remain separate work; do not describe them as available merely because managed lifecycle is
enabled.
