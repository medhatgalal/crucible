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
$CP dispatch fix-report-flow maker <maker-name>

# The maker commits first, then records focused evidence with the command in its dispatch.

$CP phase fix-report-flow REVIEW
$CP dispatch fix-report-flow judge <reviewer-name>
$CP check fix-report-flow
$CP close fix-report-flow "one durable lesson, or NONE"
```

At any point:

```sh
$CP next
cat .crucible/<program>/STATE.md
```

`STATE.tsv` is authoritative. `STATE.md` is generated for people and must not be edited. `next` is
read-only.

## What this behavior does not yet provide

Managed lifecycle currently owns item state and transition refusal. Dispatch still uses the existing
brief, evidence, and verdict files. Immutable attempt records, retry and cost budgets, task-DAG
parallelism, active-program conversion, and automatic agent launching remain separate unfinished
work. Do not describe those capabilities as available merely because managed lifecycle is enabled.
