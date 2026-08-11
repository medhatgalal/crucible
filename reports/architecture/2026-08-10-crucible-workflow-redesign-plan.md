# Architecture Recommendation

## Problem Framing

Crucible has two valuable properties: it fact-checks a problem report before admitting work, and it refuses closure without fresh evidence and independent verdicts. Its recent self-hosted run also exposed the failure of its current operating model: one item grew to a 1,368-line contract, accumulated seven revisions, invoked 57 agents across ten roles, timed out repeatedly, and remained open after 16 commits.

The failure is architectural rather than cosmetic. The executable owns evidence and closure checks, while orchestration discipline—duplicate dispatch prevention, retry limits, role budgets, repeated-finding escalation, and authoritative resume state—remains in prose or out-of-band run files. Expensive provider and full-suite checks are repeated by reviewers even when independent review only requires validation of their provenance plus a focused falsifier.

The prerequisite `ci-on-push` work is closed at `a2097294669b`, squash-merged to `main` as
`ccbcff0`, and green in hosted CI on that exact merged SHA. The redesign is isolated on
`ai/crucible-workflow-redesign`; no active agent run owns overlapping source.

## Objective

Deliver Crucible managed lifecycle as a file-only, POSIX-shell workflow that:

1. Preserves audited claim admission and strict work-ID freshness.
2. Gives the CLI one authoritative machine-readable state record.
3. Prevents duplicate or unbounded attempts before they launch.
4. Makes review cost proportional to risk.
5. Separates product failure, verification failure, infrastructure failure, and unresolved human decisions.
6. Lets the same model fill different roles in fresh contexts without mislabelling same-family review as independent diversity.
7. Lets several makers work concurrently only when a validated task DAG proves their dependencies and file ownership do not overlap.

Success means an operator can resume from files, obtain exactly one next action, and either close or explicitly block a narrow item without revising its proof contract during review.

## In Scope

- The report-to-backlog outer loop and item lifecycle after admission.
- Authoritative state, immutable attempt records, dispatch refusal, outcomes, retry accounting, and resume behavior.
- Risk-based role and model-kind requirements.
- Evidence classes and rules for canonical expensive evidence.
- A bounded task DAG, isolated task worktrees, parallel maker dispatch, and an explicit integration barrier.
- Migration of existing item-file lifecycle programs through an opt-in compatibility path.
- Focused tests, full-suite compatibility, rollback, and operational diagnostics.

## Out of Scope

- A database, daemon, service, queue, web UI, or remote coordinator.
- Cryptographic proof of which human/model authored a file.
- Automatic killing of external agent processes; Crucible does not own their process groups.
- Cross-work-ID evidence reuse in managed lifecycle.
- Automatic migration of active programs.
- Fixing or closing the current `ci-on-push` item as part of this redesign.
- Replacing Git, POSIX shell, Markdown contracts, or the existing evidence capture format.

## Assumptions & Constraints

- POSIX shell, Git, Markdown, `awk`, `sed`, and standard userland remain the only dependencies.
- Existing adopted item-file lifecycle programs must continue to run unchanged.
- Whole-work-ID evidence invalidation remains the closure boundary. A new commit invalidates every closure verdict.
- The CLI can refuse dispatches and record deadlines, but cannot safely infer that an external process died merely because a deadline passed.
- One program advances one current item at a time. Within that item's BUILD state, dependency-ready tasks may run concurrently in isolated worktrees when ownership sets are disjoint.
- The implementation baseline is merged `main` at `ccbcff0`; planning artifacts begin at `44d9db3` on the isolated redesign branch.
- Default budgets are metadata and refusal thresholds, not permission to kill processes: low-risk reviewer 10 minutes; medium/high reviewer or adversary 30 minutes; maker 45 minutes; planner/specifier 15 minutes; canonical external check 15 minutes.

## Architecture Recommendation

### Chosen design: slim managed lifecycle state machine with immutable attempts

Retain the outer loop:

```text
report -> atomic claim -> audit -> scout -> operator triage -> admit
```

Replace the seven middle phases with four item states:

```text
DRAFT -> READY -> BUILD -> REVIEW -> CLOSED
                           |          |
                           +--------> BLOCKED
```

- `DRAFT`: the item contract is incomplete or not operator-approved.
- `READY`: goal, non-goals, risk, owned files, acceptance criteria, focused falsifier, and evidence classes are frozen.
- `BUILD`: one maker attempt may own each dependency-ready task; makers never write the integration branch directly.
- `REVIEW`: focused review is active; expensive evidence has one canonical capture for the current work ID.
- `CLOSED`: current-work evidence and the risk-required panel pass.
- `BLOCKED`: a terminal-for-now state with a typed reason and an explicit unblock condition.

`DESIGN.md`, `TASKS.tsv`, generated `TASKS.md`, and `ADVERSARY.md` become risk-triggered artifacts, not mandatory lifecycle phases:

- Require `DESIGN.md` only when the item changes a public interface, architecture boundary, persistence, security boundary, or rollback behavior.
- Require `TASKS.tsv` only when more than one maker or more than one disjoint file-ownership set is necessary. `TASKS.md` is generated from it and is never authoritative.
- Require adversary review only for high-risk work or a disputed behavioral finding.

### Multi-agent task DAG

Parallelism is a property of an approved task graph, not a permission granted to agents ad hoc.

```text
                         +-> task T2 / maker A / worktree A -+
READY -> BUILD -> task T1+                                      +-> INTEGRATE -> REVIEW
                         +-> task T3 / maker B / worktree B -+
```

- `ready` validates the entire DAG before BUILD: unique task IDs, existing dependencies, no cycles, literal owned paths, no ownership overlap, and at least one verification script per task.
- A task is dispatchable only when every dependency has a current PASS result at its task commit.
- Each maker receives one task, one immutable base commit, and one isolated worktree. The task branch is `ai/<program>-<item>-<task>-<attempt>`; no task branch shares a ref prefix that conflicts with the item branch.
- Makers may not dispatch peers, edit another task's worktree, or write the item integration branch. Coordination occurs through frozen contracts and immutable results, not direct chat.
- `CRUCIBLE_MAX_PARALLEL_MAKERS` defaults to `3`. A fourth live maker dispatch refuses even when another task is ready.
- An integrator is the only role that advances the item branch. It applies passing task commits in stable topological order and records the source task commit for every integration step.
- Any textual or semantic conflict sets `INTEGRATION_CONFLICT`; Crucible never auto-resolves it. The affected task returns to READY with a new base and all prior task review results remain historical, not closure evidence.
- Task evidence proves the task commit only. After integration changes the item work ID, the final review panel runs the item's focused falsifier against the integrated work ID. Only final-work results may close the item.
- Independent tasks may finish after a sibling blocks, but integration cannot start until every required task is PASS or the operator revises the frozen DAG by returning the item to DRAFT.
- Task worktrees and branches are retained while an item is open or blocked. Cleanup is explicit after integration and final closure; no failure path deletes them automatically.

### Existing backlog disposition

The self-hosting backlog is input to managed lifecycle, not an automatically inherited implementation queue:

| Existing item | Disposition | managed lifecycle destination / reason |
| --- | --- | --- |
| `ci-on-push` | CLOSED | Merged as `ccbcff0`; delivery baseline only. |
| `feature-through-the-loop` | CLOSE WITHOUT NEW CODE | The completed CI item exercised maker, judges, adversary, integration, rejection, and closure. |
| `atomic-publish-assert` | RETAIN P0 | State/evidence atomicity mutation test. |
| `close-write-window` | RETAIN P0 | Atomic close and state-lock test. |
| `falsifier-must-discriminate` | RETAIN P0 | READY must prove the focused falsifier distinguishes changed from restored behavior. |
| `dispatch-contradicts-role` | RETAIN P0 | Generated contracts must obey role read/write boundaries. |
| `dispatch-lifecycle` | MERGE | Immutable attempt ledger and deterministic `next`. |
| `evidence-retire-verb` | MERGE | Attempt/result history and explicit archival after closure. |
| `finding-identity` | MERGE | Repeated finding fingerprint and `REPEATED_FINDING` stop. |
| `judge-isolation-worktree` | MERGE | Task/reviewer worktree contract and process adapter. |
| `orchestration-harness` | MERGE | State, attempts, bounded launch metadata, and task DAG. |
| `coldstart-gaps` | MERGE | One managed lifecycle cold-start/intake item. |
| `intake-any-source` | MERGE | Same cold-start item; add DOCUMENT, OPERATOR, and AGENT-PROPOSED provenance plus independent fidelity review. |
| `scout-arg-in-docs` | SUPERSEDE BRANCH, RETAIN REQUIREMENT | Keep the required AGENT example, but do not merge the 241-line regex sweep; validate executable examples through focused fixtures. |
| `transport-and-kinds` | SPLIT | Keep risk/kind policy in managed lifecycle; defer experimental ACP/app-server transports. |
| `byte-identical-guard` | SUPERSEDE | Immutable attempt IDs prevent verdict overwrite; retain a focused duplicate-result refusal test. |
| `lesson-injection-label` | RETAIN P1 | Either enforce lesson injection or relabel the documentation claim. |
| `isolate-or-withdraw` | RETAIN P1 | Release assertion/mutation discipline. |
| `inline-workflow-blocks-move-into-scripts` | DEFER P2 | Independent workflow cleanup after the state/attempt kernel. |
| `workflow-form-completeness` | DOCUMENTED NON-GOAL FOR MANAGED LIFECYCLE | No dependency-free shell regex can parse every Actions/YAML form honestly. |
| `a-transcription-inside-scripts-is-a-second-writer` | DOCUMENTED NON-GOAL FOR MANAGED LIFECYCLE | Treat `scripts/` as a trusted executable boundary; do not claim semantic duplicate detection. |
| B10: empty `LESSONS.md` after `add` | RETAIN P0 | Seed the file with a header; mutation proves a fresh `add` no longer creates a false refusal. |

`ai/scout-arg-in-docs` remains preserved and unmerged until the focused managed lifecycle cold-start item replaces its valid documentation delta. No commit from that branch is silently cherry-picked.

### Evidence policy

Every criterion declares one evidence class:

| Class | Producer | Reviewer obligation | Duplicate policy |
| --- | --- | --- | --- |
| `FOCUSED` | Maker and each reviewer | Run the discriminating check independently | Allowed once per required reviewer |
| `FULL_SUITE` | One canonical maker/integrator capture per work ID | Validate binding/output; run only focused falsifier independently | Second canonical capture refused |
| `EXTERNAL` | One canonical integrator/provider capture per work ID | Validate provider identity, head binding, conclusion, and completeness from the artifact | Second canonical capture refused |
| `MANUAL` | Named operator | Reviewer verifies the signed-off artifact exists; cannot promote high-risk work alone | One current-work sign-off |

A PASS still requires evidence recorded by its author. For `FULL_SUITE` and `EXTERNAL`, the reviewer’s evidence is the command that validates the canonical artifact plus its independently run focused falsifier; the reviewer does not repeat the expensive operation.

Cross-work-ID reuse is rejected for managed lifecycle. Splitting oversized items is the safe way to stop unrelated documentation edits from invalidating provider proof. Criterion fingerprints may be reconsidered only after measured managed lifecycle cost data shows that splitting is insufficient.

### Risk and panel policy

| Risk | Examples | Required panel | Adversary |
| --- | --- | --- | --- |
| `LOW` | Docs, comments, reversible config | One fresh-context reviewer; same family allowed and labelled | No |
| `MEDIUM` | Behavioral code, CI, CLI semantics | Two reviewers across at least two model kinds | On dispute |
| `HIGH` | Auth, data, migration, deletion, hot path | Three reviewers across at least two kinds | Required |

The maker and a reviewer may use the same model only in separate contexts. That result is recorded as `INDEPENDENT-CONTEXT: yes` and `KIND-DIVERSITY: no`; it does not satisfy a multi-kind requirement.

### Attempt and stopping policy

- A dispatch creates an immutable attempt before printing the launch command.
- The key is `(item, work-id, role, criterion, evidence-class, agent)`.
- A second nonterminal attempt with the same item/role/criterion is refused.
- A second canonical `FULL_SUITE` or `EXTERNAL` attempt at the same work ID is refused if a usable artifact already exists.
- A deadline crossing changes the visible condition to `OVERDUE`, not `TIMEOUT`; Crucible cannot claim process death without an observed exit.
- The launcher or operator records `TIMEOUT`, `STOPPED`, or `ABANDONED` with the exact process observation and reason.
- One infrastructure retry is permitted. A second infrastructure failure sets the item to `BLOCKED`.
- The same normalized finding fingerprint twice sets the item to `BLOCKED`; a fresh decider or human must supply the unblock decision.
- A current-work PASS by an agent makes a duplicate dispatch to that agent/role/criterion refuse before launch.

## Component or Module Boundaries

| Component | Responsibility | Owns | Must not own |
| --- | --- | --- | --- |
| Claim admission | Audit, scout, triage, admission | `CLAIMS.md`, claim evidence/verdicts | Item implementation |
| State kernel | One authoritative lifecycle and in-flight view | `STATE.tsv`; generated `STATE.md` | Agent process execution |
| Item contract | Frozen goal and proof boundary | `ITEM.md`, optional `DESIGN.md` | Runtime status |
| Task DAG kernel | Validate dependencies/ownership and expose ready tasks | `TASKS.tsv`, generated `TASKS.md`, `tasks/*.paths`, task verification scripts | Launching agents or resolving conflicts |
| Attempt ledger | Immutable dispatch and outcome facts | `attempts/<id>/meta.tsv`, `contract.md`, `result.md` | Mutable item requirements |
| Evidence recorder | Atomic command capture bound to work/agent/class | Existing `evidence/*.txt` headers plus evidence class | Deciding PASS |
| Gate evaluator | Risk panel, evidence freshness, duplicate and retry rules | `check`, `next`, dispatch refusal | Repairing work |
| Process adapter | External launcher records PID/exit/deadline observations | Optional launcher-side data in attempt metadata | Inferring outcomes without observation |
| Integrator | Apply passing task commits in stable topological order | Item integration branch and integration evidence | Editing task branches or auto-resolving conflicts |

`STATE.tsv` is the machine source of truth. `STATE.md` is generated from it and is never hand-edited. `ITEM.md` no longer owns `PHASE` or `STATUS` in managed lifecycle; those item-file headers are mirrored during compatibility mode only.

## Interface Contracts

### Program format marker

Each program gains:

```text
lifecycle: managed
```

Absence means item-file lifecycle. Existing programs remain item-file lifecycle until an explicit migration.

### `STATE.tsv`

Header and row format:

```text
item\tstatus\tstage\twork_id\trisk\tinflight_attempt\tblock_code\tupdated_epoch
ci-on-push\tACTIVE\tREVIEW\ta2097294669b\tMEDIUM\t-\t-\t1786399200
```

Allowed status values: `ACTIVE`, `BLOCKED`, `CLOSED`, `DROPPED`.

Allowed stage values: `DRAFT`, `READY`, `BUILD`, `REVIEW`.

State mutations acquire an atomic `mkdir` lock, rewrite through a same-directory temporary file, validate exactly one row per item, atomically rename, and then regenerate `STATE.md`.

### Attempt record

`dispatch` allocates `attempts/<attempt-id>/` using an atomic directory claim. `meta.tsv` contains:

```text
attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of
A1786399200.12345\titem\tT1\ta2097294669b\tjudge\tj1\tkiro\tA1\tFOCUSED\tDISPATCHED\t1786399200\t1786401000\t-
```

Attempt states: `DISPATCHED`, `RUNNING`, `OVERDUE`, `RETURNED`, `TIMEOUT`, `STOPPED`, `ABANDONED`, `SUPERSEDED`.

Terminal attempts are immutable. Corrections create a new attempt referencing `retry_of`.

### Result record

`result.md` begins with exact parseable headers:

```text
OUTCOME: PASS|REJECT|BLOCKED|NEEDS_CONTEXT|SCOPE_CONFLICT
ITEM: <slug>
WORK-ID: <id>
ATTEMPT-ID: <id>
ROLE: <role>
AGENT: <name>
KIND: <kind>
CRITERION: <id>
TASK-ID: <id or ->
EVIDENCE-CLASS: FOCUSED|FULL_SUITE|EXTERNAL|MANUAL
EVIDENCE: <relative path>
FINDING-FINGERPRINT: <12-char digest or ->
NEXT: CLOSE|FIX|DECIDE|ESCALATE
```

The body contains concise findings and file/line locations. It must not contain hidden reasoning or maker rationale.

### Item contract

Every managed lifecycle `ITEM.md` contains these sections in order:

```text
## Goal
## Non-goals
## Risk
## Owned files
## Acceptance criteria
## Focused falsifier
## Expensive evidence
## Stop conditions
```

Hard intake checks:

- Risk is exactly `LOW`, `MEDIUM`, or `HIGH`.
- One to three atomic acceptance criteria; more requires a split or explicit operator override recorded in state.
- Exactly one focused falsifier command/script, expected to finish within ten minutes.
- At most one `FULL_SUITE` or `EXTERNAL` canonical check per item.
- Owned files are declared before BUILD.
- Requirements freeze at READY. A material requirement change returns to DRAFT and archives current attempts; a wording-only clarification must not change the property or falsifier.

### Task DAG contract

When an item needs multiple makers, `TASKS.tsv` is frozen at READY:

```text
task_id\tdepends_on\tpaths_file\tverify_script
T1\t-\ttasks/T1.paths\ttasks/T1.verify.sh
T2\tT1\ttasks/T2.paths\ttasks/T2.verify.sh
T3\tT1\ttasks/T3.paths\ttasks/T3.verify.sh
```

- `depends_on` is `-` or a comma-separated set of earlier task IDs.
- `paths_file` contains one literal repository-relative path per line. Globs, absolute paths, `..`, tabs, newlines in Git paths, and the same path in two tasks refuse.
- `verify_script` is a repository-relative executable POSIX shell file. It receives the task commit as `$1` and exits non-zero on failure.
- `TASKS.md` and a ready-task view are generated from this contract; hand edits are overwritten.
- The DAG is limited to 32 tasks and 128 dependency edges in managed lifecycle. Larger work must split into items.

### CLI surface

Existing outer-loop and evidence verbs remain. Add or tighten:

```text
crucible state                         render STATE.md from STATE.tsv
crucible lifecycle status             report item-file or managed behavior
crucible lifecycle enable --dry-run   report the exact fresh-program write set
crucible lifecycle enable --apply     enable managed behavior before the first item
crucible ready ITEM                    validate/freeze the item contract
crucible task list ITEM                render DAG and derived task states
crucible task ready ITEM               print dependency-ready tasks
crucible dispatch ITEM ROLE AGENT [CRITERION]
crucible attempt start ATTEMPT PID     record observed launch and deadline
crucible attempt overdue ATTEMPT       mark overdue only if deadline passed
crucible attempt finish ATTEMPT STATE  record observed terminal process state
crucible result ATTEMPT OUTCOME ...    validate and write immutable result.md
crucible integrate ITEM                apply passing task commits in stable topological order
crucible block ITEM CODE "reason"     terminal-for-now stop with unblock condition
```

`next` remains the operator entry point but becomes read-only. It returns exactly one of:

```text
NEXT <item> <action> <exact command>
WAIT <attempt> <deadline>
BLOCKED <item> <code> <unblock condition>
DONE
```

### Block codes

`REPEATED_FINDING`, `RETRY_EXHAUSTED`, `OVERDUE_PROCESS`, `SCOPE_CONFLICT`, `MISSING_CONTEXT`, `OPERATOR_DECISION`, `EXTERNAL_UNAVAILABLE`, `DEPENDENCY_CYCLE`, `OWNERSHIP_OVERLAP`, `TASK_BLOCKED`, `INTEGRATION_CONFLICT`.

## Trade-offs

| Decision | Benefit | Cost | Mitigation |
| --- | --- | --- | --- |
| Preserve whole-work-ID freshness | Retains the strongest anti-staleness gate | Unrelated commits still invalidate evidence | Enforce narrow items and frozen contracts |
| One canonical expensive check | Removes duplicated provider/full-suite cost | Reviewers rely on another agent’s capture | Each reviewer independently validates binding/completeness and runs a focused falsifier |
| `STATE.tsv` plus generated Markdown | Deterministic resume and parsing | Adds a migration surface | Behavior-named opt-in and compatibility readers |
| Immutable attempt records | Prevents overwritten PASSes and reconstructs retries | More small files | Per-attempt directory is bounded and archiveable after closure |
| Risk-based roles | Cuts ceremony for low-risk work | Requires correct risk classification | Operator confirms risk at READY; reviewers may escalate but not downgrade |
| No automatic process killing | Avoids false timeout labels and orphaned children | Overdue work needs operator/launcher action | Refuse new duplicate work and expose exact PID/deadline condition |
| Parallel makers only through a frozen DAG | Gains safe concurrency without shared mutable work | Adds task and integration contracts | Literal ownership, dependency validation, isolated worktrees, one integrator |

## Rejected Alternatives

### Harden the existing seven-phase loop only

Rejected because it leaves status duplicated across `ITEM.md`, prose `STATE.md`, run directories, and agent narration. More prose or more phase checks would not prevent duplicate launches, overwritten verdicts, or repeated expensive proof.

### Criterion fingerprints that reuse evidence across work IDs

Rejected for managed lifecycle because relevant-file manifests and normalized criterion hashing introduce a second freshness model beside the current commit-boundary invariant. This is precisely the kind of cleverness likely to create another proof-parser dispute. Reconsider only from measured managed lifecycle evidence.

### Database/service orchestrator

Rejected because current scale does not justify operational state, migrations, credentials, or availability dependencies. Atomic filesystem claims are sufficient for one-program/one-current-item coordination.

### Always require different model families

Rejected because it makes low-risk work needlessly expensive and can block environments with one provider. Different kinds remain mandatory for behavioral/high-risk work; same-family review is explicit rather than presented as equivalent diversity.

### Let `next` reap or kill overdue attempts

Rejected because a deadline is not evidence that a process died. The current run already recorded judges as timed out before they delivered valid PASSes. `next` must remain read-only and report what is known.

### Free-form peer-to-peer agent collaboration

Rejected because direct agent messaging creates unrecorded authority, hidden dependencies, and mutable scope. Makers collaborate through the frozen DAG and immutable result artifacts; only the coordinator schedules and only the integrator changes the item branch.

### Multiple makers on the integration branch

Rejected because file ownership alone does not prevent index, generated-file, or semantic conflicts in one working tree. Each maker receives an isolated task worktree and the integration barrier is explicit.

## Migration / Rollback Plan

### Preconditions

1. `ci-on-push` is closed under item-file lifecycle; local and remote `main` agree at `ccbcff0`, and hosted CI is green.
2. `ai/crucible-workflow-redesign` is isolated from that exact baseline; planning artifacts are preserved at `44d9db3` and are not delivery evidence.
3. `ai/scout-arg-in-docs` remains preserved and must not be merged as-is; its requirement is assigned to the managed lifecycle cold-start item.
4. Before source edits, write the managed lifecycle implementation backlog from the disposition table and assign non-overlapping file ownership.

### Expand

1. Add managed lifecycle parsers, state/attempt helpers, and focused fixtures without changing item-file lifecycle default behavior.
2. Add the `lifecycle: managed` PROGRAM field, `STATE.tsv`, generated `STATE.md`, immutable attempt/result contracts, and new CLI help.
3. Make a managed lifecycle fixture program exercise READY through CLOSED while all existing item-file lifecycle tests remain unchanged.
4. Dual-write item-file `PHASE`/`STATUS` and verdict files in managed lifecycle compatibility mode.

### Migrate

1. `lifecycle enable --dry-run` prints the exact fresh-program write set and changes nothing.
2. Enable only a fresh fixture first. Active item-file conversion remains refused until a separately tested conversion behavior exists.
3. Compare `next`, `check`, current work ID, panel requirements, and close refusal before/after.
4. Opt in new `adopt` programs to managed lifecycle only after the fixture and copied-program gates pass.
5. Existing programs remain item-file lifecycle until separately approved.

### Contract

Do not remove item-file readers in the first managed lifecycle release. Removal is a later compatibility decision based on adoption evidence.

### Rollback triggers

- An item-file lifecycle program changes behavior without opting in.
- Migration changes a work ID or loses a verdict/evidence reference.
- `next` returns more than one action or an action inconsistent with state.
- Duplicate dispatch is accepted in a focused concurrency fixture.
- A current-work PASS becomes unusable after rollback.

### Rollback order

1. Stop new managed lifecycle dispatches; do not delete artifacts.
2. Remove `lifecycle: managed` from the copied fixture's `PROGRAM` file.
3. Verify item-file `ITEM.md`, evidence, and verdicts still drive `next`/`check`.
4. Revert the redesign branch normally if source rollback is needed; preserve managed lifecycle state/attempt files as ignored audit artifacts.
5. Do not migrate active programs until the defect is corrected and the full migration matrix passes again.

## Risks & Validation

### Primary risks

- TSV escaping or malformed rows could corrupt state. Reject tabs/newlines in identifiers and validate every rewrite before rename.
- A launcher could report a false PID/outcome. Crucible can make the record auditable but cannot authenticate the actor; retain the documented limitation.
- Risk may be understated to reduce panel cost. Only the operator can approve risk at READY; reviewers can escalate risk but cannot downgrade it.
- Dual writes can drift. Compatibility tests must mutate either representation and prove inconsistency refuses.
- Attempt directories can grow. Archive them only after closure; never delete live or blocking evidence automatically.

### Focused test matrix

1. item-file lifecycle program behavior and all existing refusal fixtures remain byte-for-byte compatible.
2. `ready` refuses missing risk, more than three criteria, missing focused falsifier, two expensive evidence classes, or overlapping maker ownership.
3. State rewrite is atomic and concurrent writers cannot lose a row.
4. `dispatch` refuses a second live attempt for the same item/role/criterion.
5. `dispatch` refuses a current-work duplicate where that agent already has PASS.
6. Exactly one canonical `FULL_SUITE`/`EXTERNAL` capture is accepted per work ID.
7. A reviewer can PASS by recording validation of canonical expensive evidence plus an independent focused falsifier.
8. A new commit makes all prior managed lifecycle closure results stale, preserving item-file lifecycle semantics.
9. An expired live attempt becomes `OVERDUE`, never `TIMEOUT`, without an observed terminal state.
10. One infrastructure retry is accepted; the second transitions to `RETRY_EXHAUSTED`.
11. A repeated finding fingerprint transitions to `REPEATED_FINDING`.
12. LOW/MEDIUM/HIGH panel and kind thresholds enforce the table above.
13. Same-family fresh-context review is labelled and does not satisfy kind diversity.
14. `next` returns exactly one deterministic action and does not mutate files.
15. Migration dry-run writes nothing; apply creates only the printed write set.
16. Rolling an opted-in copied program back to item-file lifecycle retains usable evidence and verdicts.
17. `ready` rejects missing dependencies, cycles, unknown task IDs, duplicate task IDs, path overlap, globs, and non-executable verification scripts.
18. Dependency-ready tasks may dispatch concurrently up to the configured cap; a fourth maker refuses at the default cap of three.
19. A dependent task cannot dispatch before every dependency has a PASS at its recorded task commit.
20. Makers cannot write the integration branch; each attempt is bound to its own task worktree and commit.
21. Integration applies task commits in stable topological order and blocks on any conflict without modifying the unresolved item branch.
22. Task evidence cannot close the item after integration changes the work ID; final-work review evidence is required.
23. A blocked task prevents integration while independent tasks may finish and remain preserved.

### Acceptance gates

- Add a dedicated focused verifier for managed lifecycle state/attempt behavior that finishes in under 15 seconds on the development machine.
- Run the existing quickstart and full self-test once at final work ID; do not repeat them per reviewer.
- Mutation proof: remove duplicate-dispatch refusal and observe the focused verifier fail; restore and pass.
- Mutation proof: change `OVERDUE` to `TIMEOUT` inference and observe the focused verifier fail.
- Migration proof on a copied inactive program, including rollback.
- Independent medium-risk review across two kinds, each running focused falsifiers; one canonical full-suite/CI capture at final work ID.
- Parallel collaboration pilot with at least three tasks, two concurrently runnable makers, one dependency barrier, and one deliberate ownership-overlap refusal.
- CI green on the exact merged SHA before release.

## Decision Log

| Decision | Recommendation | Why | Risk | Mitigation |
| --- | --- | --- | --- | --- |
| Lifecycle | Four item states; optional design/tasks/adversary artifacts | Removes ceremony not tied to risk | Under-specification | READY contract checks and risk escalation |
| State | `STATE.tsv` SSOT, generated `STATE.md` | Deterministic resume | Migration/drift | Opt-in managed lifecycle and dual writes |
| Freshness | Preserve whole-work-ID validity | Proven, understandable invariant | Rework after unrelated commits | Split items and freeze requirements |
| Expensive evidence | One canonical capture per work ID | Removes dominant duplicated cost | Trusting capture | Independent artifact validation plus focused falsifier |
| Attempts | Immutable per-dispatch records | Stops overwrites and reconstructs history | File growth | Archive only after closure |
| Deadline | Report OVERDUE; never infer TIMEOUT | Prevents false terminal labels | Manual intervention | Refuse duplicates and expose unblock action |
| Models | Risk-based kind diversity | Matches review cost to downside | Correlated blind spots at LOW | Explicit same-family label and escalation |
| Collaboration | Frozen task DAG, isolated worktrees, one integrator | Enables bounded multi-agent implementation | Integration and task-state complexity | Literal ownership, stable order, final-work review |
| Rollout | Additive opt-in managed lifecycle | Protects active programs | Temporary dual complexity | No item-file behavior removal in first release |

## Confidence

Confidence: **91/100**.

High confidence in the failure diagnosis, strict-freshness boundary, backlog disposition, and bounded task-DAG contract. Moderate uncertainty remains around the external launcher contract and measured integration cost because neither has been implemented or piloted; the design deliberately records observed process facts rather than claiming process authority.

## Architecture Quality Scorecard

- Output Completeness: 2
- Scope Discipline: 2
- Technical Specificity: 2
- Evidence Quality: 2
- Failure-Aware Decisions: 2
- Migration Clarity: 2
- Benchmark Fit: 2
- Overall Score: 14
- Pass: true
- Rationale: The recommendation preserves current constraints, defines parseable state/attempt/task contracts, rejects weaker freshness and free-form collaboration models, and includes opt-in migration, rollback, concurrency, and focused proof gates.
