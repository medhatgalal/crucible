# Demand gate (1.6.3 → 1.7.x) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Releases are gated in the order **R0 → R1b → R1 → R2**; do not start a later release's task because an earlier one is inconvenient.

**Goal:** Stop Crucible admitting work that is real but unwanted. Claim polarity (a claim stated as an absence inverts the burden of proof, because failing to find a thing is not a refutation) is what makes the engine work — and it is also the bloat engine: pointed at a capability catalog instead of a user job, every row is a true absence, so every row becomes admittable work. Close that with an operator-signed job register that no agent can author, without adding an agent, a hop, or a dependency.

**Architecture:** File-only POSIX engine, unchanged in shape. Demand is not a claim field and not a verdict: it is a `## Jobs` register inside `PROBLEM.md`, sealed by `cycle/JOBS.sha`, signed by `cycle sign-jobs`, and cited by claims. The gate lives at the two doors into `items/` — `claim admit` and `cmd_add` — never at `worth:`. R0 adds no engine change at all: it makes the existing suites honest, records a RED contract at the real gate, and measures the baseline the later releases must beat.

**Tech Stack:** POSIX `sh` (`crucible`, 5,355 lines), `scripts/verify-demand.sh` (new), `scripts/verify-attempt-ledger.sh`, `scripts/selftest.sh`, `scripts/verify-package.sh`, `scripts/verify-coldstart-independence.sh`, `.github/workflows/selftest.yml`. Baseline `VERSION` 1.6.2, `main` at `64108c0`.

## Global Constraints

- POSIX shell only. No new dependency, no daemon, no database, no network in tests.
- **R0 changes ZERO engine behavior. Do not edit the file `crucible` in R0.** If an R0 task appears to require an engine edit, STOP and report — that is a finding, not a licence.
- Own only your assigned file. Paths are disjoint by design; do not edit another worker's file.
- Do not commit, stage, tag, push, or create branches. Leave the working tree dirty for operator review.
- Never weaken, skip, or delete an existing assertion to make a suite pass.
- Do not lower `CRUCIBLE_MAX_NEW_CLAIMS`. Do not re-litigate the 8+ FILE refusal.
- Fixtures stay fixture-only: `echo`-based fake agents, no real ACP.
- Clean up every `mktemp -d`; leave no scratch directory or process behind.
- Report exact commands and exact output. Exit 0 is not by itself evidence that the intent was met.
- `worth:` is advisory display and never becomes a gate (see the vocabulary lock).

## Verified facts this plan rests on

Re-read these before trusting any task below; each was read out of the tree, not assumed.

- `cycle_worth()` (`crucible:2320`) prints `worth: BUILD` when ANY claim has one TRUE verdict AND ANY scout line says ABSENT. Both predicates read the codebase; neither reads the user.
- `cycle_worth` has **exactly three sites**: the definition (`2320`), the `printf 'worth: %s\n'` in `write_cycle_status` (`2821`), and a message-selection `case` in `cmd_next` (`3701`). **Zero** references in `cmd_add` or `cmd_claim`. It therefore gates nothing. An earlier draft of this plan gated `cycle_worth` and would have shipped a placebo whose own mutation test passed.
- The real gates are `claim admit` and `cmd_add` — two doors into `items/`.
- `cycle_refuse_non_problem` (`crucible:3991`) counts the literal string `is not a CLI verb` at `>= 8` and regexes titles for `leftover|remainder|what's left`. Rephrase the catalog and it binds.
- `write_cycle_status` rewrites `STATUS.md` every tick, and `cycle_archive_investigation` moves it into `history/`. A counter persisted there is zeroed by the event it measures — hence D2 derives, never stores.
- `claim_copy_verdict_like` (`4585`) copies `STALE|FALSE|UNVERIFIABLE`, and `FALSE` is the demand-**permitting** verdict; `drive_apply_isomorphic_verdicts` (`3196`) spreads verdicts sideways. Demand must live nowhere in `claims/` or `CLAIMS.md` or these two functions launder it.
- Evidence records `--- exit N ---` at `1264` and it is never read back for claims today.
- `adopt_install_engine` (`crucible:4360`) copies only: `crucible`, `BOOTSTRAP.md`, `START.md`, `RULES.md`, `LOOP.md`, `CONFIGURE.md`, `VERSION`, `roles/*.md`, `scripts/*.sh` minus `package-release.sh` and `verify-package.sh`, and top-level `docs/*.md`. `README.md` is NOT copied. `docs/problems/` and `docs/superpowers/` are NOT copied. The adopt-written `PROBLEM.md` is literally `# Problem\n\nTEMPLATE-PROBLEM-NEEDS-INPUT` — no Jobs block.
- The coordinator can today write `PROBLEM.md` AND score it as progress via the hash at `2866`; `PROBLEM.md` is absent from `drive_restore_all`'s revert set and from RULE 21's enumeration.
- CI (`.github/workflows/selftest.yml`) runs only `scripts/selftest.sh` and `scripts/verify-package.sh` (macOS job is tag/dispatch only). Six verify scripts are ungated: `verify-agent-cycle.sh`, `verify-attempt-ledger.sh`, `verify-drive.sh`, `verify-managed-lifecycle.sh`, `verify-quickstart.sh`, `verify-task-dag.sh`. `verify-coldstart-independence.sh` already runs inside `verify-package.sh`.
- Measured at plan time: `./scripts/verify-attempt-ledger.sh` exited **2** on clean `main`, failing at its line 151 assertion `result requires existing evidence` — wanted `missing regular evidence`, got `crucible: refused: admitting this item does not authorize tracked.txt (not in Owned files)`. Root cause is in the **fixture**, not the engine: the maker-result Owned-files authorization guard (`crucible:879-895`) fires before the evidence-existence check, and the fixture's `ITEM.md ## Owned files` omitted `tracked.txt` while the fixture commits to it. Adding `- tracked.txt` to that list is a fixture-only fix and keeps R0 engine-clean.

## File map

- Modify: `scripts/verify-attempt-ledger.sh` (D0 fixture-only fix)
- Modify: `.github/workflows/selftest.yml` (D0: gate the six ungated verify scripts)
- Create: `scripts/verify-demand.sh` (D1 RED contract at the real gate; grows into R1's gate suite and mutation manifest)
- Create: this plan (D2 design of record; D2 implementation is deferred)
- Later (R1b): `crucible` (`adopt_install_engine`, adopt `PROBLEM.md` template, `--refresh`), `START.md`, `BOOTSTRAP.md`, `RULES.md`, `LOOP.md`, `docs/demand.md` (new, top level so adopt copies it), `scripts/selftest.sh` (doc-on-surface sweep), `scripts/verify-coldstart-independence.sh`
- Later (R1): `crucible` (`cycle sign-jobs`, `claim_demand_cited`, `problem_demand_paired`, `cycle_worth` reader, `drive_restore_all`, `cycle_refuse_leftover_title`), `RULES.md`, `CHANGELOG.md`, `VERSION`
- Later (R2): `crucible` (`TASKS.tsv` requirement, `DESIGN.md` check, single risk writer), `RULES.md`, separate PROBLEM and tag

---

### Task D0: green the attempt ledger and gate the six ungated suites — R0

**Status: IN PROGRESS this session.** Checkboxes stay `[ ]` until the owning worker reports exact command and exact output; see Recovery notes for what was observed in the tree.

Mechanism: `scripts/verify-attempt-ledger.sh:151` (`refuses 'result requires existing evidence'`) never reaches its own assertion because the maker branch of `cmd_result` (`crucible:879-895`) parses `## Owned files` out of `items/alpha/ITEM.md` and refuses any changed path outside it before evidence is examined. The fixture at `scripts/verify-attempt-ledger.sh:143-146` commits `tracked.txt`, which the item did not own. Fix is one list line in the fixture heredoc; the engine is untouched. Then add all six ungated scripts (`verify-agent-cycle.sh`, `verify-attempt-ledger.sh`, `verify-drive.sh`, `verify-managed-lifecycle.sh`, `verify-quickstart.sh`, `verify-task-dag.sh`) as steps in the `refusals` job of `.github/workflows/selftest.yml`, so an unrun suite can no longer rot silently. `verify-coldstart-independence.sh` needs no step: `verify-package.sh` already runs it and is already in CI.

- [ ] Add `- tracked.txt` to `## Owned files` in the `fresh()` heredoc of `scripts/verify-attempt-ledger.sh`
- [ ] `./scripts/verify-attempt-ledger.sh` exits 0 with no `FAIL` line; record the pass/fail counts
- [ ] Add one CI step per ungated verify script; keep the existing steps and their order
- [ ] Confirm no engine edit: `git diff --name-only` must not list `crucible`
- Fixture label: red-then-green on the existing label `result requires existing evidence` — red is the recorded `wanted missing regular evidence, got ... not in Owned files`, green is the `missing regular evidence` refusal actually being asserted.
- Mutation: remove `- tracked.txt` again → the suite must return to exit 2 with the same recorded message. Second mutation: delete one added CI step → the doc/CI drift is invisible, which is the condition D0 exists to end.
- Stop condition: if greening any of the six requires editing `crucible`, STOP and report — that suite is describing an engine defect and belongs in R1's scope, not R0's.

### Task D1: RED contract at the real gate — R0

**Status: IN PROGRESS this session.** Checkboxes stay `[ ]` until the owning worker reports exact command and exact output; see Recovery notes for what was observed in the tree.

Mechanism: new `scripts/verify-demand.sh` asserts, on today's engine, that work with no user-visible job on file is admissible. It targets `claim admit` and `cmd_add`, not `cycle_worth`, and its header records why: `cycle_worth` is display-only at `2320`/`2821`/`3701` with zero references in the admission path. A1 builds a demandless investigation to COMPLETE and shows `claim admit` producing an item. A2 rephrases the leftover catalog so `cycle_refuse_non_problem` (`3991`) — which counts the literal `is not a CLI verb` at `>= 8` — no longer fires and the catalog binds. A3 is the positive control that must be read before A1 is believed: `cycle_worth` returns `UNKNOWN` unless `cycle_investigation_state` is COMPLETE, so a half-built fixture would make A1's pass an artifact of broken setup rather than a defect.

- [ ] Create `scripts/verify-demand.sh`, fixture-only, `echo` agents, `mktemp -d` cleaned on exit
- [ ] A3 first: `cycle` reaches `NEXT PROPOSE — write a refined, evidence-grounded PROPOSAL.md` and `STATUS.md` carries `worth: BUILD`
- [ ] A1: `expect 'demandless claim is admissible' '^admitted C1 as item demand-subcommand$'`
- [ ] A2: `expect 'rephrased catalog binds' '/PROBLEM\.md$'`
- [ ] Header states in-file that A1 and A2 invert to `refuses` in R1 and that the RED header comes out then
- [ ] Add the script to CI alongside D0's steps; it must pass green as a RED contract
- Fixture label: `demandless claim is admissible` (A1) and `rephrased catalog binds` (A2) are RED-as-passing today; in R1/D4 the same two labels flip to reason-specific `refuses` assertions. That flip is the plan's green.
- Mutation: assert the reason, never "any refusal". A deliberately absent `refuses` helper in this file is part of the contract — grepping for a generic failure is the sloppiness that would let the RED file lie. Mutation check: point A1's `expect` at a pattern that any error would satisfy and confirm review rejects it.
- Stop condition: if A1 cannot be made to pass without a real user-visible job in `PROBLEM.md`, the hole does not exist as described — STOP, and R1 does not ship.

### Task D2: derive the pre-gate baseline (planned here, NOT implemented this session) — R0

**Status: NOT STARTED. Design of record only.** D2 and every task after it are unchecked deliberately.

Mechanism: derive five counts from `history/` alone — admitted, landed, abandoned, refused-for-demand, duplicate-job-citation — with **no persisted counter**. A counter cannot live in `STATUS.md`: `write_cycle_status` (`2821`) rewrites it every tick and `cycle_archive_investigation` moves it into `history/`, so the counter is zeroed by the very event it measures. The derivation is a read-only pass over archived investigation directories and item state, committed as a pre-gate baseline so R1 can be judged against a number recorded before the gate existed.

- [ ] Specify the derivation source of truth (archived `history/` investigation dirs + item terminal state), in writing, before any code
- [ ] Confirm no new file in `cycle/` or `STATUS.md` holds a count
- [ ] Emit the baseline as a plain-text artifact for the operator; do not gate on it
- [ ] Do NOT implement in this session — the design of record is this section
- Fixture label: `baseline derives without a stored counter` — red when the derivation reads any file that `cycle_archive_investigation` rewrites or moves, green when it reads only `history/` and item state.
- Mutation: introduce a persisted counter and run one archive cycle; the count must visibly reset, proving why derivation is the only correct shape.
- Stop condition: **plan-level flip.** If the baseline shows admitted-vs-landed was already healthy, the demand gate is unproven ceremony and R1 does not ship.

---

### Task D10: adopt template carries a `## Jobs` skeleton — R1b (GATE on R1)

R1b is a **gate on R1, not a follow-on**. A demand gate that only works in this repository is not shipped.

Mechanism: `adopt_install_engine` (`crucible:4360`) writes `PROBLEM.md` as `# Problem\n\nTEMPLATE-PROBLEM-NEEDS-INPUT`, with no Jobs block — so a freshly adopted program would meet the R1 gate with nothing to cite and no hint what to write. Change the written template to carry a `## Jobs` skeleton whose entries are `TEMPLATE-` placeholders (so the existing template refusals still fire on an unedited file). `adopt NAME --refresh` must add the skeleton **additively** to an existing `PROBLEM.md` and must not clobber operator prose.

- [ ] Template `PROBLEM.md` includes `## Jobs` with `TEMPLATE-` placeholder entries
- [ ] `--refresh` on an authored `PROBLEM.md` appends the skeleton only if `## Jobs` is absent; byte-identical otherwise
- Fixture label: `adopt writes a jobs skeleton` / `refresh does not clobber an authored problem`.
- Mutation: make `--refresh` rewrite the whole file → the clobber fixture must fail.
- Stop condition: if additive refresh cannot be done without parsing operator prose, ship the skeleton on first adopt only and record refresh as a documented manual step.

### Task D11: register docs on the travelling surface — R1b

Mechanism: `adopt_install_engine` copies `START.md`, `BOOTSTRAP.md`, `RULES.md`, `LOOP.md`, `CONFIGURE.md`, `roles/*.md`, most `scripts/*.sh`, and **top-level** `docs/*.md`. It does not copy `README.md`, `docs/problems/`, or `docs/superpowers/`. So the register must be documented in `START.md`, `BOOTSTRAP.md`, `RULES.md`, `LOOP.md`, and a new **top-level** `docs/demand.md`. Add a `git ls-files`-style sweep to `scripts/selftest.sh` asserting every tracked file that names `sign-jobs` is inside `adopt_install_engine`'s copy list — the same shape as the existing arity sweep — so doc drift becomes a CHECK instead of a surprise at cold start.

- [ ] Create top-level `docs/demand.md` (not under `docs/problems/` or `docs/superpowers/`)
- [ ] Register documented in `START.md`, `BOOTSTRAP.md`, `RULES.md`, `LOOP.md`
- [ ] Sweep in `scripts/selftest.sh`: every file naming `sign-jobs` is on the adopt copy list
- Fixture label: `every file that teaches sign-jobs travels with adopt`.
- Mutation: document `sign-jobs` in `README.md` (never copied) → the sweep must fail.
- Stop condition: if the sweep cannot distinguish a doc that teaches the verb from a doc that merely mentions it, narrow to an explicit allowlist rather than deleting the CHECK.

### Task D12: self-teaching refusals — R1b

Mechanism: a refusal that does not name its remedy converts into a workaround. Every refusal added by this plan must name `cycle sign-jobs` and the exact file to edit (`PROBLEM.md`, `## Jobs`).

- [ ] Each new refusal message names the verb and the file
- Fixture label: `every demand refusal names its remedy` — assert the message text, not the exit status.
- Mutation: strip the verb from one message → the fixture must fail.
- Stop condition: none; this is a message-text CHECK and cannot exceed its budget.

### Task D13: cold-start contract covers demand — R1b

Mechanism: `scripts/verify-coldstart-independence.sh` already runs inside `verify-package.sh`, which is already in CI. Extend it to adopt from the **extracted package** (not the working tree) into a scratch repo, sign a register there, and admit one claim. That proves the register is usable by a program that has only the travelling surface.

- [ ] Adopt from extracted package into `mktemp -d` scratch repo; clean up
- [ ] Sign a register and admit exactly one claim in the scratch program
- Fixture label: `a cold program can sign a register and admit one claim`.
- Mutation: remove `docs/demand.md` from the copy list → cold start must fail for a stated reason.
- Stop condition: if the cold-start path needs a file that adopt does not copy, fix the copy list (D11), never the test.

---

### Task D3: operator job register, sealed — R1

Mechanism: `## Jobs` in `PROBLEM.md`, sealed by `cycle/JOBS.sha`. `cycle sign-jobs` computes the seal and **refuses while `.drive.lock` exists**, so the signature cannot be produced inside an agent-driven tick. Add `PROBLEM.md` to `drive_restore_all`'s revert set and to RULE 21's enumeration: today the coordinator can write `PROBLEM.md` and then score it as progress through the hash at `2866`, which is exactly the loop the register must not close on itself.

- [ ] `cycle sign-jobs` writes `cycle/JOBS.sha`; refuses when `.drive.lock` is present
- [ ] `PROBLEM.md` added to `drive_restore_all` revert set and RULE 21 enumeration
- [ ] Seal mismatch refuses with the remedy named (D12)
- Fixture label: `signing refuses inside a drive tick` / `a broken seal refuses admission`.
- Mutation: drop the `.drive.lock` check → the first fixture must fail.
- Stop condition: if the seal cannot be made stable across CRLF, trailing whitespace, and BOM, normalize before hashing and add encoding-variant fixtures (gap 3) rather than loosening the seal.

### Task D4: the gate itself — R1

Mechanism: `claim_demand_cited()` at **both** `claim admit` and `cmd_add` — two doors into `items/`, and a gate on one is not a gate. A **distinct-job rule** collapses N claims citing one job to ONE item, which is what actually stops catalog bloat. `problem_demand_paired()` at bind. Delete `cycle_refuse_leftover_title`'s `*pr-status*` hardcode: a named-example guard is a rephrasing target, and the register replaces it. The 8+ FILE refusal and `MAX_NEW_CLAIMS` stay untouched.

- [ ] `claim_demand_cited()` enforced at `claim admit`
- [ ] `claim_demand_cited()` enforced at `cmd_add`
- [ ] Distinct-job rule: second claim citing an already-cited job refuses
- [ ] `problem_demand_paired()` at bind
- [ ] Remove the `*pr-status*` hardcode from `cycle_refuse_leftover_title`
- Fixture labels: flip D1's `demandless claim is admissible` and `rephrased catalog binds` to reason-specific refusals, plus `two claims one job collapse to one item` and `bind requires a paired job`. Four predicates, four fixtures.
- Mutation: one per predicate — disable it and confirm exactly its own fixture fails and no other.
- Stop condition: if closing the `cmd_add` door requires a second register format or a new file under `items/`, stop and re-cut the design; one register, one seal.

### Task D5: `cycle_worth` becomes a pure reader — R1

Mechanism: `cycle_worth` (`2320`) stays advisory display and loses any pretence of gating. `WAIT-DEMAND` ships **only** alongside all four of `drive_state_name`, `drive_human_gate`, `drive_is_human_gate`, `drive_perform_legal`; without all four, an unsigned register prints `BUILD` (demand unsigned: C7). A state name that the drive loop cannot legally reach is worse than no state name.

- [ ] `cycle_worth` reads state; it never refuses
- [ ] `WAIT-DEMAND` present in all four drive functions or in none
- [ ] Unsigned register prints `BUILD` with the C7 marker
- Fixture label: `worth never refuses` / `WAIT-DEMAND is reachable or absent`.
- Mutation: add `WAIT-DEMAND` to `drive_state_name` only → the reachability fixture must fail.
- Stop condition: if `WAIT-DEMAND` cannot be made a legal drive state in this release, ship `BUILD` + C7 and defer the state.

### Task D6: anti-laundering — R1

Mechanism: demand must live **nowhere** in `claims/` or `CLAIMS.md`. `claim_copy_verdict_like` (`4585`) copies `STALE|FALSE|UNVERIFIABLE` and `FALSE` is the demand-**permitting** verdict; `drive_apply_isomorphic_verdicts` (`3196`) spreads verdicts across sibling claims. If demand were a claim field or a verdict, either function would manufacture it. Register evidence must additionally declare `expect-exit:` matched against the recorded `--- exit N ---` written at `1264` and never read back for claims today.

- [ ] Assert no demand token appears under `claims/` or in `CLAIMS.md`
- [ ] `claim_copy_verdict_like` and `drive_apply_isomorphic_verdicts` cannot propagate demand
- [ ] `expect-exit:` in register evidence compared to the recorded `--- exit N ---`
- Fixture label: `demand cannot be copied sideways` / `declared exit must match the recorded exit`.
- Mutation: add a demand field to a claim record → the no-token assertion must fail.
- Stop condition: if `expect-exit:` requires changing how evidence is written at `1264`, read-only comparison only; do not alter the evidence format in this release.

### Task D7: ship R1 — R1

Mechanism: docs, a `RULES.md` CHECK that names the RULE it converted, `CHANGELOG.md`, a grace path for in-flight investigations (a signed-register requirement must not strand work already admitted), a **mutation manifest as a CHECK** — `verify-demand.sh` asserts every `demand_*` / `sign_jobs*` function in `crucible` appears in its predicate→fixture table — and a task-ID traceability check.

- [ ] `RULES.md` CHECK names the converted RULE
- [ ] `CHANGELOG.md` entry; date matches `date +%F`; `VERSION` bumped
- [ ] Grace path for in-flight investigations, with a fixture
- [ ] Mutation manifest CHECK: no `demand_*`/`sign_jobs*` function is untested
- [ ] Task-ID traceability check (D-number → fixture → engine site)
- Fixture label: `every demand function has a fixture` — red the moment a predicate is added without one.
- Mutation: add an untested `demand_stub()` → the manifest CHECK must fail.
- Stop condition: **if R1 exceeds 120 added engine lines, stop and re-cut.** A gate that costs more than its bloat is not a fix.

---

### Task D8: TASKS.tsv when ownership spans disjoint directories — R2

R2 gets its **own PROBLEM and its own tag**; it is not smuggled into R1. Operator chose direction (a). Correction of record: this does **not** reverse the 2026-08-10 redesign — lines 84-85 of that document specify these exact triggers, and the engine implemented "conditional" as "unreachable".

Mechanism: require `TASKS.tsv` when `## Owned files` spans more than one disjoint top-level directory. That is computable at READY. The existing "more than one maker" trigger is false at first dispatch and true only after the artifact was already skippable — which is why it never fired.

- [ ] Compute disjoint top-level directory span from `## Owned files` at READY
- [ ] Require `TASKS.tsv` when the span exceeds one
- Fixture label: `a two-directory item requires a task file`.
- Mutation: revert the trigger to "more than one maker" → the fixture must pass vacuously, exposing the unreachable condition.
- Stop condition: if span computation needs anything beyond the owned list, drop to an explicit operator flag.

### Task D9: DESIGN.md and a single risk writer — R2

Mechanism: `DESIGN.md` with a content check in the shape of RULES #6, and exactly **one** risk writer — `ITEM.md ## Risk`. Delete or derive the `PROBLEM.md` grep at `4025` that sets HIGH on the phrase "HIGH confidence": two writers for one field means the field means nothing.

- [ ] `DESIGN.md` content check in RULES #6 shape
- [ ] `ITEM.md ## Risk` is the only risk writer; the `4025` grep deleted or derived
- Fixture label: `risk has exactly one writer`.
- Mutation: restore the `PROBLEM.md` phrase grep → the single-writer fixture must fail.
- Stop condition: **if the content check exceeds 40 lines, D9 does not ship** and the recorded alternative fires instead — refuse casting `specifier|architect|planner` in `assign_role_known`.

---

## Vocabulary lock

| Term | Locked meaning |
| --- | --- |
| `worth:` | Advisory **display** only. Never a gate. Making it stricter changes printed advice and admits the work anyway. |
| The gate | `claim admit` **plus** `cmd_add`. Two doors; both or neither. |
| Demand | A `## Jobs` register in `PROBLEM.md` sealed by `cycle/JOBS.sha`. **Not** a claim field. **Not** a verdict. |
| `DEMAND-ABSENT` | **Deleted from the design.** Do not reintroduce. |
| `demand-evidence:` | **Deleted from the design.** Register evidence declares `expect-exit:`. |
| `judge` | The role name. Never `reviewer`. |
| Risk | One writer: `ITEM.md ## Risk`. |
| `WAIT-DEMAND` | Ships only with all four drive functions, else `BUILD` + `demand unsigned: C7`. |

## Plan-level stop and flip conditions

- If R0's baseline (D2) shows admitted-vs-landed was already healthy → **R1 is unproven ceremony and does not ship.**
- If two cycles after R1 show every claim signed and `WAIT-DEMAND` never terminal → the gate is **certifying rather than refusing**. Delete it. Do not tune it.
- If the register needs an agent auditor per claim → **stop**; that reintroduces the 1.6.1/1.6.2 cost problem the register exists to avoid.
- If R1 exceeds **120 added engine lines** → stop and re-cut (D7).
- If D9's content check exceeds **40 lines** → D9 does not ship; the `assign_role_known` refusal fires instead.

## Three remaining gaps

1. **A signature is a gesture, not a verification.** Nothing proves the operator read what they signed. Bounded, not solved, by four things: the `.drive.lock` refusal in `cycle sign-jobs`, `PROBLEM.md` in `drive_restore_all`'s revert set, declared-exit evidence (`expect-exit:` vs `--- exit N ---` at `1264`), and a rubber-stamp detector (time-to-sign, jobs-signed vs jobs-cited). **Detected, not prevented** — and undecidable inside file-only single-user constraints. Do not pretend otherwise in docs.
2. **Nothing ranks two real jobs.** Out of scope. Made visible rather than hidden: jobs are printed in `STATUS.md` in operator-authored order, so a bad ordering is at least legible.
3. **BSD `grep -E` divergence and seal brittleness** across CRLF, trailing whitespace, and BOM. Closed by running `scripts/verify-demand.sh` on macOS at PR time (the `refusals-bsd` job is currently tag/dispatch only) plus encoding-variant fixtures on the seal.

## Recovery notes (read this first after a crash or compaction)

- Baseline: `VERSION` 1.6.2, `main` at `64108c0`, `crucible` 5,355 lines of POSIX sh.
- Session scope is **R0 only**: D0, D1 in progress; D2 designed here and deliberately **not** implemented.
- Nothing is committed by design. `git status` plus `git diff` shows exactly how far R0 got; the working tree is the progress record.
- Observed R0 progress at plan time: `scripts/verify-attempt-ledger.sh` modified with the `- tracked.txt` owned-files line (D0 fixture fix) and now exits 0; `scripts/verify-demand.sh` created (261 lines, A1 `demandless claim is admissible`, A2 `rephrased catalog binds`, A3 positive control); `.github/workflows/selftest.yml` modified (+35/-1) with steps for `verify-managed-lifecycle.sh`, `verify-coldstart-independence.sh`, `verify-attempt-ledger.sh`, `verify-task-dag.sh`, `verify-agent-cycle.sh`, `verify-drive.sh`, plus a `verify-demand.sh` step guarded by a presence test. Re-verify all three files before claiming D0 or D1 complete; `verify-quickstart.sh` was not observed in the CI list.
- `crucible` must be unmodified in R0. `git diff --name-only | grep -x crucible` returning anything is a defect in the work, not progress.
