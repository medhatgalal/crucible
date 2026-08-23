# Managed lifecycle protocol

This is the coordinating agent's low-level protocol reference. Operators use the conversational cycle
described in `START.md`; they do not advance attempts, tasks, or phases themselves.

## `$CP` in the examples below

Every command on this page is written with `$CP`. It is the installed program's engine, not a
Crucible verb. Define it once per shell, from the **target repository root**:

```sh
CP=.crucible/<program>/crucible     # after `adopt work --managed`: CP=.crucible/work/crucible
```

Then `$CP cycle` is `.crucible/work/crucible cycle`. Nothing exports this for you; a shell
without it runs nothing.

Managed lifecycle makes program state machine-readable and gives the agent one deterministic resume
behavior. It is selected by behavior in `PROGRAM`:

```text
lifecycle: managed
```

There is no product version name. Programs without that field keep the item-file lifecycle so an
existing program does not change underneath active work.

## Install it for a new cycle

New cycles select the behavior during adoption (install/refresh/use: [install.md](install.md)):

```sh
<engine>/crucible adopt <program> --managed
# later, from a newer source, same program name:
<engine>/crucible adopt <program> --refresh
# leftover DONE occupying SRC, start real work without --next:
<engine>/crucible adopt <name> --managed --panel-from <program>
```

That creates `STATE.tsv` and its generated `STATE.md` atomically with the installed program, and
marks `cycle: guided`. Guided cycles require panel configure/approval before investigation.

### Guided panel gate

```sh
# Replace placeholder agents.tsv rows, write PANEL.md + PANEL.ASSIGN.tsv, then:
$CP cycle approve-panel
$CP cycle problem /path/to/report.md
# After this PROBLEM is finished — same panel, new investigation:
$CP cycle problem /path/to/next-report.md --next
# Junk INVESTIGATE (not a problem): archive without PASS or a new PROBLEM
$CP cycle problem --abandon "leftover FILE --next of a non-problem"
```

`PANEL.md` must include: Agents, Roles, Risk posture, Isolation transport, Independence ladder,
Waivers. `PANEL.ASSIGN.tsv` is authoritative role→agent casting (coordinator, claim-auditor, maker,
reviewer, contract-auditor required; scout required in practice — see [CONFIGURE.md](../CONFIGURE.md)).
Every agent named there must also have a row in `agents.tsv`, coordinator included; a coordinator with
no registry row makes `approve-panel` refuse. Placeholder `MODEL` / `AGENT_CLI` / `OTHER_CLI` rows refuse
approval. Guided dispatches must use agents cast for that role.

The admit bar is derived from the casting, with a floor the casting does not show: `claim admit`
requires one TRUE verdict per `required=yes` `claim-auditor` row (`refused: C1 has 2 TRUE verdicts,
need 3` with three such rows), while `cycle` and `triage` require **two** whatever the panel says
(`MORE AUDIT — 1 TRUE across 1 kind(s); need 2 across 1.`). The effective bar is therefore
`max(2, required=yes claim-auditor rows)` sealed TRUE verdicts from distinct agents, across at least
`CRUCIBLE_MIN_KINDS` model families (default 1). A panel with one `claim-auditor` row cannot leave
INVESTIGATE; cast two.

`close` reads `required=yes` `reviewer` rows with no floor — one row closes on one PASS.
`CRUCIBLE_MIN_AUDITORS` (no default; overrides both admit gates) and `CRUCIBLE_MIN_JUDGES`
(default 2, but the reviewer row count wins on a guided cycle) override the derived numbers. Full
table: [CONFIGURE.md](../CONFIGURE.md).

### Attempt transport and contract audit

```sh
# Before a maker dispatch: an independent plan-audit PASS and an existing work branch.
$CP plan-audit fix-report-flow <reviewer-name> PASS
git branch ai/fix-report-flow main
$CP dispatch fix-report-flow maker <maker-name> A1 FOCUSED
# contract path is printed
# While DISPATCHED only — before start:
$CP attempt transport <ATTEMPT> multi-agent   # or acp | subagent
# Auditor must be cast as contract-auditor and distinct from the attempt agent:
$CP contract-audit <ATTEMPT> <auditor-name> PASS   # or FIX | STOP
$CP attempt start <ATTEMPT> <pid>             # guided: refuses without seal above
# Optional ACP ladder probe (single-product hosts); append-only history:
# $CP probe-acp failed "acp not available"
# ... agent work: checkout ai/fix-report-flow, change owned files, commit, then run ...
$CP attempt finish <ATTEMPT> RETURNED "observed exit 0"
$CP result <ATTEMPT> PASS <evidence-file> CLOSE -
```

Full ordering, with what each of those two gates refuses: [Work an item](#work-an-item).

Independence ladder: multi-agent preferred; ACP for single-product isolation; subagent only after a
recorded ACP probe failure (`ACP-PROBE.md` with `status: failed` or PANEL notes). `STOP` blocks the
item as `INDEPENDENCE_UNAVAILABLE` — do not continue as solo theatre.

### Investigate: claims before items

INVESTIGATE has its own primitives, and none of them are item verbs. `claim add` creates the claim
object; hand-written CLAIMS.md headings create nothing the engine can audit:

```sh
$CP claim add "CLAIM" "EXACT SOURCE SENTENCE" [ABSENT|EXISTS|DEFECT]
$CP claim list
$CP dispatch CN ROLE AGENT
$CP run-claim CN NAME -- CMD...
$CP claim verdict N AGENT TRUE|FALSE|STALE|UNVERIFIABLE [CITE] [--like C2 C3]
$CP claim scout CN ABSENT|PARTLY-EXISTS|FULLY-EXISTS|IN-FLIGHT AGENT
$CP triage
$CP claim admit CN SLUG
```

The ordered walkthrough, with the seal steps between dispatch and verdict, is in
[START.md](../START.md). `triage` prints one ADMIT/DROP recommendation per claim derived from
recorded verdicts and exits non-zero while any claim has none.

The `lifecycle enable` primitive remains only for converting an empty older installation before its first
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

Then advance only when the named work is actually complete. Two gates sit between `ready` and a
maker dispatch:

```sh
$CP ready fix-report-flow
$CP plan-audit fix-report-flow <reviewer-name> PASS   # PASS | FIX | STOP
$CP phase fix-report-flow BUILD
$CP agents
# The item's Git target and its work branch — see below. The branch must exist first.
cat .crucible/<program>/items/fix-report-flow/TARGET
git branch ai/fix-report-flow main
$CP workid fix-report-flow                            # a commit, never NOBRANCH
$CP dispatch fix-report-flow maker <maker-name> A1 FOCUSED
```

`plan-audit SLUG AUDITOR PASS|FIX|STOP` records an audit of `ITEM.md` before any maker is launched.
On a guided cycle `dispatch … maker` refuses without it:
`refused: maker dispatch requires plan-audit PASS`. The auditor needs a row in `agents.tsv`; role
casting is not checked, and the independence check
(`refused: mk1 is a maker of json-flag — plan-audit must be independent`) reads the item's
`MAKERS.tsv`, which the first maker dispatch writes — so at plan-audit time it is empty and the
maker's own name is accepted. Name the reviewer deliberately. The verdict is write-once; a second
identical verdict is a no-op and a different one refuses with
`refused: plan-audit.md is immutable (existing PASS)`.

### The item's Git target and its work branch

`claim admit CN SLUG` — and `add SLUG` — bind the item to a Git target themselves, writing
`items/<slug>/TARGET` from `PROGRAM`'s `repo:` and `base:` with `branch: ai/<slug>`. Neither creates
that branch. `target` overrides the binding when the work belongs on a different repository, branch,
or base:

```text
target SLUG REPO BRANCH BASE
```

It refuses a path that is not a Git repository (`not a git repo: <repo>`) and a base that does not
resolve (`no such base ref: <base>`).

Create the work branch **before** the maker dispatch. `workid` in Git mode is the branch's commit;
while the branch is missing it is the literal string `NOBRANCH`, and the item cannot reach a result:
`run` names the evidence file `…NOBRANCH.txt`, and `result` refuses with
`maker result requires current work`. Creating the branch afterwards does not recover the attempt —
`result` then refuses `evidence work id does not match attempt`, and re-recording the evidence
refuses with `managed evidence requires a RUNNING or OVERDUE attempt` because the attempt has
already RETURNED.

`dispatch` writes a complete attempt contract before the agent is launched. Its arguments are:

```text
dispatch ITEM ROLE AGENT [CRITERION] [EVIDENCE_CLASS] [RETRY_OF]
```

`ROLE` is `maker`, `judge`, or `adversary`. `EVIDENCE_CLASS` is `FOCUSED`, `FULL_SUITE`,
`EXTERNAL`, or `MANUAL`. The command prints both the contract path and the registered invocation
from `agents.tsv`; Crucible records the work but does not launch the process.

Seal independence while DISPATCHED, then record the observed process lifecycle. The attempt id is
the directory holding the printed contract, so `A=$(basename "$(dirname "$D")")` where `$D` is
`dispatch`'s stdout:

```sh
ATTEMPT=A<epoch>.<pid>.<sequence>
$CP attempt transport "$ATTEMPT" multi-agent   # or acp|subagent per ladder
$CP contract-audit "$ATTEMPT" <contract-auditor> PASS
$CP attempt start "$ATTEMPT" <observed-pid>

# The agent checks out the work branch, commits, and only then records a focused check
# through the command in its contract. Evidence recorded before the commit carries the
# pre-change work id and `result` refuses it with
# `evidence work id does not match attempt`:
# git checkout ai/fix-report-flow && ... && git commit
# $CP run fix-report-flow <maker-name> -- <bounded-command>

$CP attempt finish "$ATTEMPT" RETURNED "launcher observed exit 0"
$CP result "$ATTEMPT" PASS <evidence-filename> CLOSE -
```

An attempt that is dispatched and then abandoned blocks the item: a second `dispatch` refuses with
`refused: item already has in-flight attempt <id>` and `phase` refuses while naming the recovery —
`refused: attempt <id> is DISPATCHED and still in flight — it never started, so end it with:
… attempt finish <id> ABANDONED "<what you observed>"`. Run the command the refusal prints; it
answers `<id> ABANDONED; never started, in-flight pointer released for <slug>` and a redispatch
proceeds. `contract-audit <id> <contract-auditor> FIX` releases the same pointer by superseding the
contract, which is the right verb when the contract was wrong rather than duplicated. Both need the
attempt to still be DISPATCHED; after `attempt start`, `contract-audit` refuses with
`refused: contract-audit may only be recorded while DISPATCHED (before attempt start)`.

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
`cycle clean --dry-run`. See [Session cleanup](#session-cleanup) for what it keeps, what it removes,
and how to clear a worktree it refuses to remove.

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

## Session cleanup

Cleanup runs in the **target repository**, from its root, on the installed program:

```sh
CP=.crucible/<program>/crucible
$CP cycle clean --dry-run
$CP cycle clean --apply
```

`--dry-run` changes nothing and prints the exact write set. Run it first and show it to the operator;
`--apply` only after explicit approval. Drive never runs `--apply`.

Preconditions, each a refusal rather than a warning:

- The cycle must report `DONE`. Anything else refuses with
  `cleanup requires DONE; current cycle says: <line>`.
- No attempt may be live. A `RUNNING` or `OVERDUE` attempt whose pid is still alive refuses with
  `cleanup refuses while attempt <id> is RUNNING (live pid)`. Dead pids are reclaimed as `STOPPED`
  and cleanup continues.
- A leftover lock from a driver that did not exit cleanly is released by `$CP drive stop`, which also
  reclaims dead pids and names any attempt still alive. The lock is
  `.crucible/<program>/.drive.lock` and it is a **directory**, not a file: `drive` takes it with
  `mkdir` and `drive stop` releases it with `rmdir`, printing
  `released <program-dir>/.drive.lock`. A regular file at that path is not a lock — `drive stop`
  prints `no .drive.lock`, leaves the file in place, and `drive` starts anyway.

What the preview prints, line by line:

| Line | Meaning |
| --- | --- |
| `KEEP …/agents.tsv` | Panel identity, not leftover evidence. Cleanup does not delete it |
| `KEEP …/PANEL.ASSIGN.tsv` | Same: the casting that produced the evidence stays |
| `REMOVE_WORKTREE <path>` | A registered worktree under `.crucible/<program>/worktrees/` — task or integration |
| `PRESERVE_BRANCH <repo> <branch>` | That worktree's branch survives; only the checkout is removed |
| `PRESERVE <program dir>` | Problem, proposal, claims, items, attempts, reviews, and evidence |
| `NO_SESSION_ARTIFACTS` | Nothing to remove; the panel and evidence lines still apply |

`agents.tsv` is machine-local and stays out of Git through the generated `.crucible/.gitignore`, but
cleanup keeps the file on disk. Destroying the panel after a `NO-BUILD` would destroy the identity
behind the verdicts.

### When a worktree refuses to be removed

`--apply` removes each listed worktree with `git worktree remove`, and git refuses a worktree with an
in-progress cherry-pick — which is how an integration worktree is left when
`task integrate` hits a conflict. Cleanup then stops:

```text
crucible: could not safely remove worktree: /repo/.crucible/work/worktrees/int
```

Nothing was destroyed. Inspect the worktree, then clear the cherry-pick and retry:

```sh
git -C .crucible/<program>/worktrees/<name> status --short
git -C .crucible/<program>/worktrees/<name> cherry-pick --abort
$CP cycle clean --apply
```

`cherry-pick --abort` returns that worktree to the commit it was on and discards only the
half-applied pick. The branch is still preserved by the retried `--apply`. If the worktree holds
uncommitted work you want, commit it on its branch first — the branch survives cleanup, the checkout
does not. Do not `rm -rf` the worktree directory instead: `--apply` skips a path that is already gone
and reports success, while `git worktree list` keeps a `prunable` registration that the preview goes on
listing as `REMOVE_WORKTREE`. Clear that with `git worktree prune`.

## Refusals that matter

Managed lifecycle refuses:

- dispatch outside the role's stage, an unregistered agent, or maker self-review;
- a guided maker dispatch with no `plan-audit PASS` on the item;
- a maker result on a Git-target item whose work branch does not exist (`workid` is `NOBRANCH`);
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
