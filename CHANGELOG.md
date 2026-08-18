# Changelog

All notable changes to this project are documented here. This project follows
[Keep a Changelog](https://keepachangelog.com/) and [Semantic Versioning](https://semver.org/).

## Unreleased

## [1.5.2] - 2026-08-18

### Problem bind, abandon, and REVIEW FIX re-entry

- `cycle problem FILE` and `--next` refuse a one-line “X is not a CLI verb”
  (that is claim polarity, not a PROBLEM) and leftover/remainder catalogs that
  name no falsifiable outcome. Leftover `pr-status` titles still refuse.
- `cycle problem --abandon REASON` archives INVESTIGATE under `history/` with
  `ABANDON.md`. No PASS. No new PROBLEM. Human gate; drive must be stopped.
- Drive does not invoke the coordinator while a sealed worker exists. After a
  judge `NEXT:FIX`, the parent runs `phase ITEM BUILD` (including when the
  judge attempt is still `RETURNED` inflight) instead of stalling in REVIEW.
- Drive no longer treats every `RETURNED` inflight as a successful tick when
  `result` is missing or `NEXT:FIX`.

## [1.5.1] - 2026-08-18

### Cleanup is a first-class DONE step

- `DONE` next verb is `cycle clean --dry-run` (or human `--next`). Drive stops
  and never `--apply`, never `--next`, never deletes Jira.
- `cycle` prints a CLEANUP card: closed items, stale evidence, dead pids,
  stale panel title, CLAIMS still NEW. Not husks, not `.validation/`, not Jira.
- `close` refuses leftover `evidence/*` work-ids (run `evidence archive`) and
  work-id ≠ branch HEAD unless `--successor SHA`.
- `--next` refuses leftover `pr-status` titles and refreshes PANEL title/risk
  (cast unchanged; PANEL.APPROVAL rewritten to the new panel-id).

## [1.5.0] - 2026-08-18

### Claim polarity, verdicts, close, and FIX re-entry

- `claim add` is one predicate with polarity `ABSENT|EXISTS|DEFECT`. Bundled
  verbs (`and` / `+` / `;`) and present-tense desired-behavior-as-gap refuse.
- `claim verdict` never clobbers a writeup (prior file goes to `verdicts/history/`)
  and records `CITATION:`.
- `CLAIMS.md` `status` is written: `AUDITED_FALSE`, `ADMITTED`, `CLOSED`.
- `close` refuses unless work-id equals the item branch HEAD and the last
  judge/maker PASS. `LESSONS.md` cannot stay `NONE` after a REJECT fingerprint.
- Scout result `IN-FLIGHT`; `ABSENT` refuses when an unmerged `ai/*` branch
  already has commits. Admit refuses `EXISTS` and `IN-FLIGHT`.
- `phase REVIEW BUILD` is legal after a judge `NEXT:FIX`.
- Maker `result` may only change `## Owned files` paths when that section exists.
- `drive tick` exits 0 if the inflight attempt is already `RETURNED`.
- `--next` writes `PANEL.CONTEXT.md` (title/risk) without recasting.

## [1.4.0] - 2026-08-17

### Long-horizon loop

- `drive tick` no longer continues after the first worker. `cmd_attempt` was
  overwriting `sub`; drive now uses `drive_mode`.
- `attempt reclaim ATTEMPT` records `STOPPED` when RUNNING/OVERDUE and the pid
  is dead. Claim attempts do not BLOCK the item. `--next` reclaims dead pids
  instead of trapping the operator.
- `evidence archive SLUG` moves item evidence whose work-id is not current into
  `evidence/history/` so `check` can close.
- Guided admit/close bars follow **required** `claim-auditor` / `reviewer` rows
  unless `CRUCIBLE_MIN_AUDITORS` / `CRUCIBLE_MIN_JUDGES` is set.
- `STATUS.md` has `worth: BUILD|DOCS|NO-BUILD|UNKNOWN`. After a complete
  investigation with no ABSENT scout, `cycle` asks for NO-BUILD / DOCS-ONLY /
  live-observation, not a silent build.
- `adopt --refresh` on a directory without `PROGRAM` refuses as a **husk**.
- [docs/install.md](docs/install.md) is first-install vs upgrade from 1.3.x.

## [1.3.7] - 2026-08-17

### `dispatch ITEM judge` is usable on a guided panel

- `agent_cast_for_role` assigned `role=$(assign_role_normalize …)`, and POSIX functions
  share the caller’s variables. `cmd_dispatch_managed` then saw `reviewer` and died:
  `managed lifecycle dispatch role must be maker, judge, or adversary`.
- Normalize only for `PANEL.ASSIGN` lookup. The dispatch role stays `judge`.

P1 (drive starts sealed workers), P3 (admit attach), P4 (`FAILURES: none`), and P5
(`PASS --like`) already shipped in 1.3.6 and stay.

## [1.3.6] - 2026-08-17

### Drive starts sealed workers

- `drive` executed the coordinator and **printed** the `agents.tsv` line. Nobody started
  grok/kiro ACP, so the DAG stopped after contract-audit PASS. Drive now runs the exact
  registered command for one sealed DISPATCHED attempt per tick, records `attempt start`
  with the observed pid, waits, and records `RETURNED|TIMEOUT|STOPPED`. It still does
  not write verdicts or implement product code.
- `contract-audit PASS --like ATTEMPT2…` copies a real auditor PASS onto isomorphic
  DISPATCHED contracts (same role; claim id normalized). Drive applies that copy so
  seven scout briefs do not need seven contract-auditor sessions. The coordinator
  cannot stamp PASS.
- `claim admit CN SLUG` attaches a second claim to the current ACTIVE item of that
  slug. It no longer calls `add` and dies with `another item is current`.
- PASS writes `FAILURES: none` and `REQUIRED_FIX: none`. Checklist prose in the note
  is `NOTE:`, not a fake failure list. The file remains immutable.
- Scout `contract.md` omits the “Independence seal (before verdict)” footer (P6).
  Transport + audit still happen on the attempt ledger.

### Claim-scout contracts are role-faithful (kept from the 1.3.6 review branch)

- Generated scout contracts no longer embed `claim verdict` and they name
  `claims/CN/evidence/` as the scout report path.
- `crucible dispatch` with missing args prints usage instead of crashing under `set -u`.
- `attempt transport acp` is legal on a multi-kind panel. The label describes this hop.
- `claim scout` / `claim verdict` bind the latest **sealed** dispatch, not the first
  leftover unsealed file.

## [1.3.5] - 2026-08-14

### The DAG can start the next PROBLEM without recasting the panel

- `DONE` meant the program was finished forever. An approved follow-up (for example live-verify
  after the verbs already shipped) had no admittable claim, so `cycle` said DONE and `drive`
  printed cleanup. The operator had to `adopt` a new program and recast the panel.
- `cycle problem FILE --next` archives the closed investigation under `history/` (problem, claims,
  proposal, items, attempts) and binds a new PROBLEM. `PANEL*` and `agents.tsv` stay. Drive still
  does not invent the next problem. Leftover `DISPATCHED` attempts that never started do not
  block `--next`; `RUNNING` / `OVERDUE` still do.
- `DONE` now says there is no admittable claim, and leftover `items/` directories are not work.

### Additive engine refresh

- Refreshing by copying the program directory deleted machine-local adapters (`scripts/acp-brief.py`)
  and often left a pre-`drive` engine in the target repo. `adopt PROGRAM --refresh` overwrites
  engine files only and keeps extra scripts.
- `adopt` ships `VERSION`. `STATUS.md` records `engine:` so a stale install is visible.
- Onboarding SSOT: [docs/install.md](docs/install.md) (install, confirm engine, refresh, `--next`,
  `drive`). BOOTSTRAP, START, README, LOOP, CONFIGURE, RULE 21, drive, and orchestrator point at it.

## [1.3.4] - 2026-08-13

### STALE/FALSE resolves a lone TRUE for investigation

- A sealed STALE or FALSE from a **distinct** auditor means the claim is not current work.
  `cycle` no longer stays INVESTIGATE demanding a second TRUE (admit still needs 2 TRUEs).
- TRUE + scout `FULLY-EXISTS` also does not demand a second TRUE (nothing to admit).

## [1.3.3] - 2026-08-13

### Cycle/admit bar and drive invoke

- `cycle` no longer reports PLAN/COMPLETE for a TRUE claim that `claim admit` would refuse.
  Investigation stays `NEEDS_AUDIT` until `CRUCIBLE_MIN_AUDITORS` sealed TRUEs (and `MIN_KINDS`).
- `drive` refuses when the coordinator invoke exits non-zero (missing ACP adapter, etc.)
  instead of treating a failed launch as no-progress STOP.
- Refresh note: do not clobber machine-local scripts such as `acp-brief.py`.

## [1.3.2] - 2026-08-13

### Drive review P1s

- Product `HEAD` movement (`git commit` / completed merge) refuses; `HEAD` is reset to the
  pre-tick snapshot. In-progress `MERGE_HEAD` still refuses.
- Already-dirty product files are content-hashed; same porcelain line with new bytes refuses.
- Task worktree writes under `worktrees/` refuse (no longer hidden as `.crucible/*`).
- Verdict CHECK hashes `items/*/verdicts` and `claims/*/verdicts` (new files and overwrites).
- WAIT inflight compares live attempt **ids**, not a global count.
- `cycle: guided` flip during a tick refuses and restores `PROGRAM`.
- Progress token includes seal files, CLAIMS/PROPOSAL hashes, and live attempt ids so a
  seal-only WAIT tick is not a false STOP.
- `verify-drive.sh` covers commit, dirty-hash, worktree, verdicts, PROGRAM flip, WAIT second
  attempt, and seal-as-progress.

## [1.3.1] - 2026-08-13

### Drive honesty (adversarial review)

- Public SSOT: BOOTSTRAP, README, and START now say when to run `cycle` vs `drive`, what
  `STATUS.md` is, and that humans approve panel/proposal. README “Go deeper” lists BOOTSTRAP
  and `docs/drive.md`.
- RULE 21 scoped to what the parent actually CHECKs (cycle first, human-gate exit, uncommitted
  product porcelain, in-progress merge, new item-verdict paths, INVESTIGATE fallback dispatch).
  Duplicate RULE 21 numbering fixed.
- `docs/drive.md` no longer labels the full legal-action table as CHECKs. Drive does not invoke
  makers/auditors.
- `cycle approve-panel` / `cycle approve` refuse while `.drive.lock` exists.
- Drive re-checks `cycle: guided` and managed lifecycle every tick (not only at entry).

## [1.3.0] - 2026-08-13

### Drive outer loop

- `crucible drive` / `drive tick` for guided+managed cycles: always `cycle` first, rewrite
  `STATUS.md`, invoke the cast coordinator in a new process, then `cycle` again.
- Human gates (`WAIT PANEL`, `WAIT APPROVAL`, `ESCALATE`, `DONE`) print the exact action and
  exit; never auto-approve.
- CHECK: coordinator edits to owned product paths, verdict writes, and merges are refused
  (product tree restored). INVESTIGATE ticks dispatch the next unaudited/unscouted claim only.
  Inflight WAIT cannot start a second attempt.
- Maker contracts include the inner falsifier loop (this item only; no admit; no merge).
- `docs/drive.md`; START.md lead sentence; seeded LESSONS.md line; `verify-drive.sh`.

## [1.2.2] - 2026-08-12

### Panel bind is operational (not display-only)

- Guided `dispatch`, `attempt transport`, `contract-audit`, `attempt start`, `result`,
  `claim verdict`, and `claim scout` refuse when the approved panel hash is stale
  (PANEL.md / PANEL.ASSIGN.tsv / agents.tsv drift).
- `require_attempt_independence` re-runs the transport ladder against the *current*
  approved panel so temporary kind-collapse or waiver cannot leave a dishonest seal.

### Investigation on the independence ledger

- Every guided claim verdict (TRUE/FALSE/STALE/UNVERIFIABLE) and scout result requires
  a matching dispatch plus sealed transport + contract-audit PASS.
- Claim attempt stamps must match **item + agent + role** (stolen attempt-ids refuse).
- Consume-time re-check: `claim admit`, managed `check` judge results, and
  `cycle_investigation_state` ignore or refuse unsealed verdicts/results (covers temporary
  `cycle: guided` toggle while writing files).
- Successful claim verdict/scout terminalizes the claim attempt (`SUPERSEDED`) so session
  cleanup is not blocked by sealed-but-never-started DISPATCHED rows.

### Cast exclusions and ladder honesty

- Coordinator may not also be claim-auditor or contract-auditor; contract-auditor may not
  be a maker (approve-panel refuses).
- Waiver tokens match only lines `WAIVER:` / `LADDER_WAIVER:` (prose cannot grant them).
- Prior `probe-acp ok` blocks subagent even if PANEL says `ACP: unavailable`.
- `contract-audit FIX` SUPERSEDES the attempt (redispatch with a revised contract); re-PASS
  on the same attempt is not a path.
- Re-approving a prior panel content hash after intermediate drift re-points PANEL.APPROVAL.
- Generated contracts put transport + contract-audit **before** attempt start.
- Guided negative tests extended in `verify-coldstart-independence.sh` and agent-cycle no-work path.

## [1.2.1] - 2026-08-12

### Independence enforcement (Codex review P1/P2)

- Transport and contract-audit may only be recorded while an attempt is **DISPATCHED**;
  `attempt start` requires a sealed transport + contract-audit PASS on guided cycles
  (closes post-RETURNED independence theatre).
- contract-audit **FIX** is recoverable: prior FIX is archived under
  `contract-audit.history/`, re-audit is allowed, and item in-flight is cleared so
  redispatch can proceed.
- Panel approval content-binds `agents.tsv` (registry/invocations); mutating the
  registry invalidates the panel id.
- Claim-auditor/scout dispatches create attempt ledger rows; guided claim TRUE
  requires sealed independence on that claim attempt.
- ACP probes are append-only under `acp-probes/`; a prior `ok` cannot be unbound-
  downgraded to `failed` to unlock subagent transport.
- Missing-audit refusal prints a runnable command including the auditor argument.
- Trailing-tab hygiene in TSV examples; focused negative tests for the above.

## [1.2.0] - 2026-08-12

### Cold-start independence and role casting

- Guided cycles require a content-bound agent panel (`PANEL.md` + `cycle approve-panel`) and
  non-placeholder `agents.tsv` before problem intake or investigation.
- Compact configure asks for **agent inventory and persona→agent casting**. Authoritative
  `PANEL.ASSIGN.tsv` is required and content-bound with panel approval. Guided dispatch,
  claim-auditor dispatch, and contract-audit must match the casting table. Maker and reviewer
  must be different agents unless waived; the coordinator must not be cast as maker/reviewer
  unless waived.
- Independence ladder: prefer multi-agent products/CLIs; on single-product hosts prefer ACP;
  allow host subagents only after a recorded ACP probe failure (`probe-acp`); escalate
  `INDEPENDENCE_UNAVAILABLE` instead of continuing as the panel. When ≥2 model kinds are
  registered, guided transport must be `multi-agent` unless PANEL has
  `LADDER_WAIVER: single-product`.
- Managed attempts record transport and
  `contract-audit ATTEMPT AUDITOR PASS|FIX|STOP` before guided-cycle results are accepted.
  Auditor must be a distinct registered agent cast as `contract-auditor`; makers cannot audit
  reviews; structural contract checks apply. New `roles/contract-auditor.md`.
- Guided claim TRUE requires a prior claim-auditor or scout dispatch contract.
- Public docs SSOT rewrite: `BOOTSTRAP.md`, `START.md`, `CONFIGURE.md`, `LOOP.md`, `RULES.md`,
  README, managed-lifecycle guide, coordinator role (removed anti-interview lock-in).
- Verifiers: `scripts/verify-coldstart-independence.sh`; agent-cycle and package checks updated.
- Security honesty: process discipline raises the cost of independence theatre; under one OS user
  independence remains unproven cryptographically.

### Documentation

- Clarified bootstrap from public raw `BOOTSTRAP.md` URL or local checkout/package path.
- README cold-start success criteria: configure (agents + casting) → independent agents → evidence.

## [1.1.0] - 2026-08-11

- Simplified public usage to one fresh-agent prompt and one installed-cycle resume prompt. The
  installed `START.md` is now self-contained and keeps low-level commands behind the agent-facing
  protocol instead of making the operator learn them.
- Added deterministic `crucible-<version>.tar.gz` packaging, SHA-256 checksums, archive-content and
  cold-start verification, and Linux/BSD release-package CI gates.
- Fixed zero-argument command parsing under POSIX `dash`. The public `cycle` resume command and other
  missing-argument paths could previously exit silently on Linux because a failed `shift` terminates
  a non-interactive shell before `|| true` can recover.

- Replaced command-driven onboarding with one fresh-agent entrypoint and one durable `cycle` resume
  behavior. The coordinator now owns protocol mechanics, proposes a minimal agent/persona panel,
  interrogates the problem, obtains approval of a refined proposal, and iterates work through review
  findings until current evidence supports closure or a bounded escalation stops the run.
- Added content-bound proposal approval. A managed claim cannot become work before the operator has
  approved the current `PROPOSAL.md`, and any proposal edit invalidates that approval.
- Added behavior-named managed lifecycle selection through `crucible lifecycle`, authoritative
  `STATE.tsv`, generated `STATE.md`, a focused verifier, and an internal protocol guide. New cycles
  can select managed behavior atomically with `adopt --managed`; existing programs retain item-file
  behavior unless explicitly enabled before their first item.
- Added managed attempt records with immutable dispatch metadata, observed lifecycle events,
  evidence-bound write-once results, typed outcome/next-action pairs, explicit maker and reviewer
  deadlines, one infrastructure retry, repeated-finding stops, and unchanged-work reuse of
  canonical `FULL_SUITE` and `EXTERNAL` PASS evidence. Maker results distinguish input work from
  post-change work; review entry requires a current-work maker PASS, and managed closure rejects
  compatibility verdicts not backed by an attempt result.
- Added optional frozen `TASKS.tsv` validation and generated task views. Managed `ready` refuses
  unknown or cyclic dependencies, duplicate task IDs, unsafe or overlapping literal ownership,
  and missing or non-executable task verifiers. Dependency-ready tasks can run in isolated Git
  worktrees with a three-maker default cap, literal path ownership, frozen task verification, one
  bounded retry, and stable integration that blocks on ancestry or cherry-pick conflict. All task
  makers remain in a durable maker set, cannot review the integrated item, and reviewer results label
  same-family, cross-family, or mixed-family correlation.

## [1.0.0] - 2026-08-07

Initial release.

### What it is

A file-only refusal gate for agentic software work. One agent orchestrates, others make and judge,
and nothing closes until the recorded evidence says it may. POSIX shell and markdown; nothing to
install beyond `git` and a shell.

### The loop

An outer loop turns a problem document into a backlog: one claim per finding quoting its source,
audited by two or more agents of different kinds, scouted for work that already exists, then
triaged with the operator before anything is admitted. An inner loop walks each admitted item
through SPEC, DESIGN, TASKS, BUILD, VERIFY, ADVERSARY and GRADUATE, with a contract generated for
every dispatch and a lesson recorded at close that binds every later maker.

### What is enforced

Each of these refuses, and each is asserted in `scripts/selftest.sh` by a test that fails when the
mechanism is removed:

- Evidence is recorded by the tool, never written by an agent, and carries the work id in its
  filename and inside the file, so renaming stale evidence is refused.
- A PASS must name an evidence file its own author recorded.
- Only agents registered in `agents.tsv` may judge, and the maker may not judge its own work.
- A judge's brief is built from a whitelist and contains no maker report, transcript or rationale.
- A commit after a verdict voids that verdict; the work id is the branch tree.
- Absence fails: no work, no evidence, no falsifier, no verdict, no scout report — all refuse.
- A claim cannot become an item without enough TRUE verdicts across enough distinct kinds.
- Phases refuse without their artifacts, even when the phase is hand-edited.
- Closing twice refuses, and work changing between check and close refuses.
- Concurrent recording and closing are safe; an unknown verb refuses rather than succeeding.

### What is not enforced

The gate cannot prove who wrote a file. Under one user with a shell, one actor can author every
verdict under different registered names, and this has happened in practice during development.
Independence is a property of how you dispatch, not something a file-only design can establish.
`RULES.md` labels every line CHECK or RULE so it is always clear which one you are relying on, and
`SECURITY.md` states the boundary plainly.

### Verification status

The suite runs on Linux for every push and on BSD for tags, and reports its own assertion count.
Two things are not yet demonstrated and are named rather than implied: no agent has followed the
documented bootstrap prompt end to end in a clean environment without help, and the loop has not
yet been used to build a feature — it was validated on this repository's own release check.
