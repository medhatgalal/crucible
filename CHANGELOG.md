# Changelog

All notable changes to this project are documented here. This project follows
[Keep a Changelog](https://keepachangelog.com/) and [Semantic Versioning](https://semver.org/).

## Unreleased

## [1.6.5] - 2026-08-23

### Untracked panel copy, cleanup exit status, and an executable walkthrough

- `cycle approve-panel` copies `agents.tsv` to `PANEL.AGENTS.tsv`, and the
  generated `.crucible/.gitignore` did not exclude it. 1.6.4's own docs told
  operators to run `git add .crucible`, so that commit tracked a byte-identical
  copy of the machine's agent command lines and absolute paths while four
  documents promised machine-local invocations stayed out. The pattern is now
  generated, `adopt --refresh` appends any missing pattern additively without
  clobbering operator edits, and a refresh reports a copy that is already
  tracked with the `git rm --cached` needed to untrack it — an ignore rule
  cannot untrack what is already in the index.
- The cleanup traps added in 1.6.4 removed the scratch directory on a signal and
  then let the shell resume to a `0` exit, so an interrupted or timed-out suite
  was recorded as a pass. 1.6.4's notes claimed cleanup happened "without
  masking exit status"; that was wrong. Ten scripts now exit 128+signal after
  cleaning up, and a normal pass or failure still returns its own status.
- An independent cold-start audit ran the published 1.6.4 tarball from the
  documents only. The blockers below are its findings.
- `START.md`'s own `PANEL.ASSIGN.tsv` template carried one `claim-auditor` row,
  and four documents stated the admit bar as one TRUE verdict per
  `required=yes` row. The real rule is `max(2, required rows)`, enforced in two
  places that disagree: `cycle` and `triage` apply a floor of 2 while
  `claim admit` applies the panel count. A reader following the template could
  never leave INVESTIGATE. The template now carries two rows and every document
  states the rule, with `CRUCIBLE_MIN_AUDITORS`, `CRUCIBLE_MIN_JUDGES` and
  `CRUCIBLE_MIN_KINDS` and their defaults.
- The first maker `result` was unreachable: no document established the
  `ai/<slug>` work branch, so `workid` returned `NOBRANCH` and `result` refused
  `maker result requires current work`, with no recovery. `claim admit` writes
  `TARGET` for you; the branch is not created. Both are now documented, along
  with `crucible target SLUG REPO BRANCH BASE`.
- `plan-audit SLUG AUDITOR PASS` is required before maker dispatch and appeared
  in no walkthrough. It also does not check maker independence at that point,
  because the check reads `MAKERS.tsv`, which the first maker dispatch writes;
  the documentation says so rather than promising a check that does not fire.
- The walkthrough printed two runnable `dispatch` forms for one step. Run as
  printed, both created attempts and only one was sealed, which made the
  agent's TRUE verdict invisible to `cycle` while `triage` still reported
  ADMIT. One form now, and the recovery is documented.
- `phase` refused with an empty attempt id when `STATE.tsv` had no row for the
  item, and a stray `DISPATCHED` attempt could not be cleared by
  `attempt reclaim` or `attempt finish`, leaving the item permanently stuck.
  Refusals now name what is actually in flight, and
  `attempt finish <id> ABANDONED "<observation>"` ends a never-started attempt
  and releases the in-flight pointer. The guard against proceeding while real
  work is in flight is unchanged.
- The CLEANUP card named `.validation/` and husk programs (`work/`, `b3/`) from
  the machine it was written on. On another repo those may not exist, and
  `work/` was a live panel-approved program there, so an agent relaying the line
  told the operator to delete a live cycle. Husks are now read off the tree and
  nothing is named that has not been seen — the same hard-coding class as the
  1.6.1 `engine: 1.4.0` defect.
- `selftest.sh` reported two failures and exited 1 from any tree without
  `.github/`, which is every release tarball and every adopted program, since
  `adopt` copies `scripts/*.sh`. The CI-workflow assertions now skip with a
  stated reason when the workflow is absent, and remain exactly as strict,
  including the inline-step digest, when it is present.
- The dirty-worktree cleanup refusal ended with
  `retry: crucible cycle clean --apply`, and a bare `crucible` is not on PATH in
  a target repository. It now prints the program's own path.
- `.drive.lock` had no documented location and is a directory at
  `.crucible/<program>/.drive.lock`; a hand-made file there is silently ignored
  by `drive stop`. `PROPOSAL.md`'s path, the `acp-brief.py` adapter's location,
  and `scripts/verify-cleanup.sh`'s absence from README's verification list are
  also fixed.
- Documented behavior found while proving the walkthrough: `drive` dispatches an
  INVESTIGATE claim to the first cast claim-auditor only, and its isomorphic
  copy never carries TRUE, so drive alone halts at one TRUE verdict.
- New `scripts/verify-walkthrough.sh` executes the documented install-to-done
  path, extracting the panel template and commands from the documents rather
  than restating them, and fails when documentation and engine disagree. Three
  consecutive releases tried to fix "an agent cannot act from the documents
  alone" by writing better documents; each time an audit found the new
  walkthrough was not executable. This makes that a test.
- `## Jobs`, `cycle sign-jobs`, and `worth: WAIT-DEMAND` remain planned only, in
  `docs/superpowers/plans/2026-08-22-demand-gate.md`. `scripts/verify-demand.sh`
  remains a recorded RED contract whose three assertions pass today — **not** a
  gate. The demand gate does not exist yet.

## [1.6.4] - 2026-08-22

### Cold-start docs, temp-dir cleanup, and CHECKs that hold the prose

- An adversarial cold-start audit ran the published 1.6.3 tarball in throwaway
  repositories from the documents alone, never opening the Crucible checkout —
  once as a fresh install, once upgrading a lived-in 1.6.1 installation. Both
  completed, but the fresh install only got through because the engine
  self-documents at runtime via `help protocol`, usage strings, and generated
  dispatch contracts. The docs alone were not sufficient.
- The `claim` verbs and `triage` appeared in no document that `adopt` copies, so
  INVESTIGATE — the first phase where an agent must act — had no command
  sequence anywhere. `START.md` now carries the ordered walkthrough:
  `claim add`, `dispatch`, `attempt transport`, `contract-audit`, `run-claim`,
  `claim verdict`, `claim scout`, `triage`, `claim admit`.
- `$CP` was used 34 times in `docs/managed-lifecycle.md` and twice in
  `CONFIGURE.md` and defined nowhere, so every command example there was
  unrunnable as printed. Defined once.
- The upgrade instructions said to confirm `engine:` in `STATUS.md` right after
  `--refresh`, but `adopt` never rewrites `STATUS.md`, so the check reported the
  old version and a careful reader concluded the upgrade had failed. Run `cycle`
  first, then confirm. The docs no longer hard-code a version number, which is
  the defect that left 1.6.1 telling its users to confirm `engine: 1.4.0`.
- Scout was listed as an optional role but is required on a guided cycle: with
  no scout cast, `claim scout` refused and pointed at a `dispatch` that refused
  in turn, while three documents said not to recast the panel. Scout is now
  required in the initial casting, and the no-recast rule is scoped to
  `--refresh` and `--next`.
- Upgrades now say to stop `drive` before `--refresh` replaces the engine
  binary.
- `scripts/acp-brief.py` was named in five places as a preserved local adapter,
  ships in no package, and was defined nowhere. `CONFIGURE.md` now states what
  it must do and that the operator writes it.
- The admit and close bars, the legal `LOW|MEDIUM|HIGH` risk tokens, and the
  coordinator's required `agents.tsv` row were all facts an agent had to guess.
  Documented.
- `README` linked two files absent from the release tarball. Documents now say
  to commit the program directory, without which the durability claim does not
  hold.
- Six suites leaked a `mktemp -d` per run; 1,642 directories totalling 1.2 GB
  had accumulated on one machine, the oldest five days old. Because `adopt`
  copies `scripts/*.sh` into every target repository, adopters inherited the
  leak. All now clean up on exit and on interrupt, without masking exit status.
- `cycle clean --apply` refused a dirty worktree with only a path, which is the
  right refusal and useless guidance. It now names the state — in-progress
  cherry-pick or uncommitted changes — and the exact command that clears it, and
  previews the same in `--dry-run`, so the operator learns before applying
  rather than after. The refusal still stands and nothing is force-removed.
- New `scripts/verify-cleanup.sh` proves cleanup in a target repository: panel
  identity and evidence preserved, worktrees removed and their branches kept,
  refusals when an attempt is live or a worktree is dirty, the documented
  recovery working, `drive stop` clearing a stale lock, and no suite leaking.
- Three new CHECKs, because prose has a poor record here: every verb
  `help protocol` prints must appear in a document that `adopt` copies; every
  relative markdown link in a travelling document must resolve to a file the
  package ships; every verify script that makes a temp directory must set a trap
  that removes it.
- `## Jobs`, `cycle sign-jobs`, and `worth: WAIT-DEMAND` are planned only, in
  `docs/superpowers/plans/2026-08-22-demand-gate.md`. `scripts/verify-demand.sh`
  remains a recorded RED contract whose three assertions pass today — **not** a
  gate. The demand gate does not exist yet.

## [1.6.3] - 2026-08-22

### Maker result on NOBRANCH, and CI that actually runs the suites

- `cmd_result` passed `dispatch_wid` to `git diff` without validating it as a
  rev. `dispatch_wid` is legitimately `NOBRANCH` when a maker is dispatched
  before a work branch exists (only judge and adversary refuse that state), so
  the first maker result on a git-target item aborted at exit 128 with no
  message — git's fatal output was swallowed by a `2>/dev/null` with no
  matching `|| true`. `result` now validates the rev and falls back to the item
  base, so it reaches the Owned-files scope gate and refuses with a
  diagnosable message instead of crashing. The scope gate is still enforced on
  the first commit.
- Every verify suite runs on every push — `verify-agent-cycle`, `verify-drive`,
  `verify-task-dag`, `verify-managed-lifecycle`, `verify-attempt-ledger`,
  `verify-coldstart-independence`, `verify-quickstart`, `verify-package` —
  alongside `selftest`. Seven of them previously ran nowhere, which let two stay
  red on main unnoticed. The macOS job also runs on pull requests for one fast
  script, where BSD and GNU `grep` diverge.
- `verify-attempt-ledger` and `verify-managed-lifecycle` declared stale
  owned-file scopes and one stale expected refusal message.
  `verify-managed-lifecycle` also leaked a temp directory per run and now
  cleans up.
- `scripts/verify-demand.sh` records three assertions that pass today: work can
  be admitted with no user-visible job named, and a rephrased capability
  catalog binds. It is a recorded RED contract for a later release, **not** a
  gate — a passing suite here is not enforcement. The demand gate itself is
  planned in `docs/superpowers/plans/2026-08-22-demand-gate.md` and does not
  exist yet.

## [1.6.2] - 2026-08-20

### Cost-per-claim close and sibling cycle for leftover PROBLEM

- After isomorphic `STALE`/`FALSE` copy, drive does not start a sibling
  claim-auditor worker (the copied verdict already closed the claim). Parent
  INVESTIGATE tick treats those copies as progress and does not fall through
  to a coordinator ACP hop.
- `adopt NAME --managed --panel-from SRC` installs a sibling cycle with the
  same approved panel (`agents.tsv`, `PANEL.md`, `PANEL.ASSIGN.tsv`,
  `PANEL.APPROVAL`). Leftover DONE on `SRC` stays put. Drive never `--next`s.
- `cycle problem FILE` while an investigation is bound refuses with `--next`
  (same panel, archive) and `--panel-from` (parallel cycle). Empty-claim
  INVESTIGATE still uses the coordinator to split `PROBLEM.md`. Product-path
  discipline still runs when a claim was already dispatched and is unsealed.

## [1.6.1] - 2026-08-20

### Fast isomorphic INVESTIGATE and panel-safe CLEANUP

- Drive INVESTIGATE parent-dispatches engine claim-auditor templates (all
  sibling claims, first auditor) without a coordinator ACP hop. One worker
  per tick. Engine-template contract-audit PASS is recorded by the parent
  on the first newly dispatched claim; isomorphic `--like` copies the rest.
- After one contract-audit PASS, `--like` copies onto isomorphic C2/C3 in
  the same/next tick. `claim verdict CN AGENT STALE|FALSE --like C2 C3`
  copies non-TRUE verdicts onto sealed siblings. Status STALE stays STALE.
- STATUS for ABSENT-only claims no longer says “admit needs 2 TRUE”;
  NO-BUILD when every verdict is FALSE/STALE even if scout said ABSENT.
- `cycle clean --dry-run` KEEP agents.tsv and PANEL.ASSIGN.tsv. `--apply`
  does not delete the panel. CLEANUP card says do not destroy the panel.
- `cycle` syncs PANEL.CONTEXT.md title from PROBLEM.md (cast unchanged).
  `--next` keeps the original title case (a POSIX helper no longer clobbers
  `title`). CLEANUP compares titles with the same helper, so DONE does not
  report panel-title-stale after a refresh.

## [1.6.0] - 2026-08-19

### Investigate fan-out cap, plan-audit, drive stop

- FILE refuses a laundry list of 8+ “X is not a CLI verb” even with a trailing
  falsifier paragraph (leftover catalog, not a PROBLEM).
- `claim add` refuses a 4th `NEW` claim while 3 remain (`CRUCIBLE_MAX_NEW_CLAIMS`,
  default 3). Audit or drop before adding more. Admit bar stays 2 TRUE.
- `crucible drive stop` releases `.drive.lock` and reclaims dead RUNNING pids.
  It does not kill a live worker and never `--next`s or `--apply`.
- Guided maker dispatch requires `plan-audit SLUG AUDITOR PASS` (not the maker).
- `result` while `.drive.lock` exists must come from a RUNNING worker, not the
  coordinator.

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
