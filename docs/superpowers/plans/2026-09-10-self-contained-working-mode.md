# Self-contained working-mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship opt-in working-mode so a target repo plus a harness (Grok / Claude / Codex) can run module-fitting slices through a small loop with no `$HOME` skills and no Crucible clone at runtime.

**Architecture:** Guided 1.6.6 stays default. Working-mode is a second runner (`wm`) plus four swap-out batteries copied by `adopt --working-mode`. Engine CHECKs, batteries, harness adapters, product modules, and delivery units stay decomplected. Canonical skills live in the package `skills/` and in the target `.crucible/skills/`; harness views are projections.

**Tech Stack:** POSIX `sh` (`crucible`, `wm.sh`), fixture CLI workers in verify scripts, existing `scripts/verify-*.sh` pattern.

**Spec:** Operator ballot `1b, 2c, 3d, 4a, 5c, 6b, 7c, 8b, 8c, 9c, 10c, 11c, 12e, 13b, 14a, 15b, 16a`. Advisory docs: `/tmp/crucible-protocol.IjzbAa/self-contained-replan.md`, `self-contained-simple.md`. Do not copy `/tmp/.../engine/wm.sh` as-is; re-implement against this plan’s CHECKs.

**Ballot (locked):** 1b opt-in · 2c live fence · 3d risk-triggered judge · 4a terminal walker · 5c script-started judge · 6b module-fitting slices · 7c architecture battery · 8b critique battery · 8c human signs map after 8b · 9c tarball adopt · 10c core four · 11c CONTRACT+ROUTING KEEP · 12e LESSONS+ARCH · 13b three adapters + blank-home · 14a RULE 26 fit · 15b path layout · 16a one canonical tree.

## Global Constraints

- Do not change guided 1.6.6 default behavior. All existing `scripts/verify-*.sh` that pass today must still pass.
- Do not modify the sibling workgraph-jira repository.
- No `$HOME` skill trees as a runtime dependency (`~/.grok/skills`, `~/.claude/skills`, `~/.agents/skills`, `~/.analyze-context` as product memory).
- Runtime after adopt: target repo + one harness CLI. Refresh only from a versioned tarball; refuse `src == dst`.
- Maker ≠ judge is a CHECK. Controller does not implement product or stamp PASS.
- No background processes in verify or `wm loop`. Foreground only. Live/destroy/push-main STOP-ASK (2c).
- Version this work **1.7.0** (opt-in). Changelog date = `date +%F` on the release commit.
- Tests fixture-only until Task 8 (echo/python stubs). Task 8 may use real harnesses only in a throwaway temp repo with empty `HOME`.
- `/tmp/crucible-protocol.IjzbAa/engine/` is evidence of CHECKs, not the blessed tree.

## Independent verification protocol (every task)

Applies to Tasks 1–8. Skipping a gate is a plan failure.

1. **Falsifier first.** The implementer writes or extends the task’s `scripts/verify-*.sh` (or battery contract test) and runs it **RED** before product code exists.
2. **Implementer ≠ reviewer.** A fresh subagent implements. A **different** fresh subagent re-runs the verify script, tries the named cheats, and writes ACCEPT / REVISE. The implementer may not ACCEPT their own work.
3. **Cite artifacts.** Every ACCEPT names command + exit code + a path. “Looks good” is not a verdict.
4. **Guided regression.** After each task that touches `crucible` or `adopt`, run `scripts/verify-agent-cycle.sh` (and `verify-package.sh` when packaging files change).
5. **No home leak.** After Task 3, every working-mode verify runs with `HOME` set to an empty temp dir (except the harness binary’s own cache if a real CLI is invoked in Task 8 — then skills must still resolve from the target tree).

## File map

Create:

- `wm.sh` — working-mode CHECK runner (kernel)
- `skills/architecture/SKILL.md`, `CONTRACT.md`
- `skills/critique/SKILL.md`, `CONTRACT.md`
- `skills/review/SKILL.md`, `CONTRACT.md`
- `skills/loop-design/SKILL.md`, `CONTRACT.md`
- `adapters/grok.md`, `adapters/claude.md`, `adapters/codex.md`
- `scripts/verify-working-mode.sh` — CHECK selftest (fixtures)
- `scripts/verify-working-mode-adopt.sh` — adopt layout + tarball + blank HOME
- `scripts/verify-working-mode-map.sh` — map CHECKs (module-fit, MAP-ACCEPT identities)
- `scripts/project-skills.sh` — 15b/16a projections
- `docs/working-mode.md` — operator/coordinator travelling doc
- this plan

Modify:

- `crucible` — `cmd_adopt --working-mode`, copy skills/wm/adapters, refuse self-refresh, ENGINE-SOURCE pin, KEEP batteries on refresh
- `scripts/package-release.sh` / `scripts/verify-package.sh` — require `wm.sh`, `skills/*/SKILL.md`, `adapters/*.md`
- `docs/install.md`, `START.md`, `BOOTSTRAP.md`, `CHANGELOG.md`, `VERSION` (1.7.0 on release task)
- `.crucible/.gitignore` generator — still ignore `*/agents.tsv`, `*/worktrees/`; **do not** ignore `skills/`

Do not modify: guided cycle verbs except adopt/copy; workgraph-jira; host `~/.grok`.

---

### Task 1: Working-mode kernel CHECKs (`wm.sh`)

**Ballot:** 2c, 4a (runner exists), 5c, maker≠judge, MAKER-WRITES, observed-red, NO-BUILD.

**Files:**

- Create: `wm.sh`
- Create: `scripts/verify-working-mode.sh`
- Test: that verify script (fixture git repos under `mktemp`)

**Interfaces:**

- Consumes: none (new kernel)
- Produces: `wm.sh` commands: `init`, `cast ROLE AGENT KIND COMMAND`, `ready`, `record-pre-falsify`, `red`, `built`, `green`, `evidence AGENT -- CMD`, `verdict RETURNFILE`, `run ROLE`, `next`, `loop`, `close`, `status`, `workid`

**CHECKs the verify script must prove (RED then GREEN):**

- [ ] **Step 1: Write `scripts/verify-working-mode.sh` with these assertions (expect fail: no `wm.sh`)**

```sh
# (1) SPEC ## Focused falsifier must be exactly MAKER-WRITES
# (2) red: missing product → RED; falsifier already green on clean tree → NO-BUILD
# (3) product dirty vs pre-falsify on red → early-implement, not NO-BUILD
# (4) FALSIFIER containing `&` or nohup → refuse before exec
# (5) maker cannot verdict PASS
# (6) planted .wm/verdicts without reviewer run → close refuses
# (7) after maker-build, next is NEXT RUN reviewer (ignore leftover reviewer receipts)
# (8) CLOSED PASS requires wm run reviewer actually exec'd (invoke.log after last maker-*)
# (9) live/push-main/rm -rf in falsifier or evidence argv → refuse (2c)
# (10) no background: loop does not daemonize
```

- [ ] **Step 2: Run it; expect non-zero** (`wm.sh` missing or CHECKs absent)
- [ ] **Step 3: Implement `wm.sh` until the script exits 0**
- [ ] **Step 4: Independent reviewer re-runs verify-working-mode.sh and the loopfull forge from the protocol engine-review; ACCEPT only if both refuse cheats**
- [ ] **Step 5: Commit** `feat: working-mode kernel CHECKs (opt-in wm.sh)`

**Stop:** Do not wire `adopt` yet. Do not call Grok/Claude/Codex.

---

### Task 2: Skill package layout + projections (15b, 16a)

**Files:**

- Create: `skills/architecture/CONTRACT.md` (stub SKILL.md body allowed: “not yet ported”)
- Create: same for `critique`, `review`, `loop-design`
- Create: `scripts/project-skills.sh`
- Create: `ROUTING.tsv` template
- Modify: `scripts/verify-working-mode-adopt.sh` (start here; Task 3 fills adopt)

**Interfaces:**

- Consumes: canonical dir `skills/<name>/`
- Produces: `project-skills.sh SRC DST` writes:
  - `$DST/.crucible/skills/<name>/` (canonical)
  - `$DST/.crucible/.grok/skills/<name>/`
  - `$DST/.crucible/.claude/skills/<name>/`
  - `$DST/.crucible/.agents/skills/<name>/`
  - `$DST/.grok/skills/<name>/`, `$DST/.claude/skills/<name>/`, `$DST/.agents/skills/<name>/` as **views** of the same files (copy or relative symlink, committed, no `$HOME`)

- [ ] **Step 1: Failing test** — `project-skills.sh` into a temp repo; assert no files under `$HOME`; assert canonical + grok/claude/agents views exist; assert `SKILL.md` content is identical across views
- [ ] **Step 2: Implement `project-skills.sh` + four stub batteries with `CONTRACT.md` listing must-write / must-not**
- [ ] **Step 3: Independent reviewer runs the test with `HOME=/tmp/empty-home-$$`**
- [ ] **Step 4: Commit** `feat: canonical skills and harness views (no home dir)`

---

### Task 3: `adopt --working-mode` + tarball pin (9c, 11c KEEP)

**Files:**

- Modify: `crucible` `cmd_adopt` / `adopt_install_engine`
- Modify: `scripts/package-release.sh`, `scripts/verify-package.sh`
- Modify: `docs/install.md`
- Create: finish `scripts/verify-working-mode-adopt.sh`

**Interfaces:**

- Consumes: `wm.sh`, `skills/`, `adapters/` (adapters may still be stubs), `project-skills.sh`
- Produces:
  - `crucible adopt PROGRAM --managed --working-mode` copies kernel + wm + skills + projections + `ROUTING.tsv` + `ENGINE-SOURCE` (version + sha256 of the installing tree when known)
  - `crucible adopt PROGRAM --refresh` refuses if `src` resolves to `dst` (self-refresh)
  - `--refresh` KEEP: do not overwrite `skills/<name>/` if `.keep` or `KEEP` list says so unless `--overwrite-batteries`
  - Missing required battery in ROUTING → `wm ready` / adopt prints refused, does not generalist-fallback

- [ ] **Step 1: Extend verify-working-mode-adopt.sh**

```sh
# adopt --working-mode into temp git repo
# assert .crucible/PROGRAM/wm.sh and .crucible/skills/architecture/SKILL.md
# assert .grok/skills/architecture/SKILL.md exists (projection)
# HOME=empty: no writes under the empty home except maybe .git config we set
# --refresh with src==dst refuses
# tarball via package-release includes wm.sh and skills/architecture/SKILL.md
```

- [ ] **Step 2: Run RED**
- [ ] **Step 3: Implement adopt + package list**
- [ ] **Step 4: Run `scripts/verify-agent-cycle.sh` (guided still green)**
- [ ] **Step 5: Independent reviewer: adopt from the tarball, not from the git worktree path**
- [ ] **Step 6: Commit** `feat: adopt --working-mode copies wm and skills from a pin`

---

### Task 4: Core four batteries (10c, 7c, 8b, 11c, 14a) — port/rewrite, not dump

**Files:**

- Fill: `skills/architecture/{SKILL.md,CONTRACT.md}`
- Fill: `skills/critique/{SKILL.md,CONTRACT.md}`  # invert + adversarial + simple only
- Fill: `skills/review/{SKILL.md,CONTRACT.md}`    # code/testing as lenses; must re-run falsifier
- Fill: `skills/loop-design/{SKILL.md,CONTRACT.md}` # loopy craft/audit/debrief only
- Create: `scripts/verify-working-mode-map.sh` (contracts + identity CHECKs; fixture agents)

**CONTRACT.md shape (every battery):**

```markdown
## In
## Out (must-write paths / words)
## Must-not
## Swap
Replacing this directory must not require editing wm.sh.
```

**Identity CHECKs in verify-working-mode-map.sh:**

- architecture author id ≠ critique author id
- critique must not write MAP-ACCEPT on a map it authored
- MAP-ACCEPT / MAP-REVISE / MAP-STOP-ASK are the only map words (not CLOSED PASS)
- owned paths must be under named modules from `architecture/modules.md` or refuse (14a)
- CHANGES-ARCHITECTURE → STOP (no silent second pattern)
- mapper id recorded; later `cast maker` equal to mapper → refuse for those slices (7c)

- [ ] **Step 1: Contract tests RED** (empty SKILL bodies fail must-write)
- [ ] **Step 2: Port/rewrite the four SKILL.md files in Crucible terms (no `engos-` names)**
- [ ] **Step 3: Independent reviewer: try mapper=maker, critique self-ACCEPT, owned path outside modules — all must refuse**
- [ ] **Step 4: Commit** `feat: four working-mode batteries with swap contracts`

**Skip:** SuperCharge `/full` `/grade` `/ult`, Batman, Loopy run/publish, GWS, herdr-init.

---

### Task 5: Map cadence + human sign (6b, 8c, 3d labels)

**Files:**

- Modify: `wm.sh` — `wm map-ready`, `wm map-verdict RETURNFILE`, `wm next` after MAP-ACCEPT emits first READY slice
- Modify: `docs/working-mode.md`
- Extend: `scripts/verify-working-mode-map.sh`

**Interfaces:**

- Produces: `MAP.md` / `slices.tsv` (id, module, owned paths, depends-on, risk LOW|HIGH, status)
- `wm map-verdict` refuses if author == mapper
- After MAP-ACCEPT, if risk HIGH or live fence, require `MAP-HUMAN` file (8c) before first `wm run maker-falsify`
- 3d is **label + ROUTING**, not a second engine: HIGH slices require `kind` of reviewer ≠ maker’s kind when two harnesses are cast; if only one harness, STOP-ASK rather than fake CROSS-FAMILY

- [ ] **Step 1: Tests RED** — map without human sign on HIGH/live cannot start maker; LOW local can
- [ ] **Step 2: Implement**
- [ ] **Step 3: Independent reviewer**
- [ ] **Step 4: Commit** `feat: map ACCEPT/REVISE and human sign for HIGH/live`

---

### Task 6: Terminal walker + CLOSED (4a, 5c, 2c)

**Files:**

- Modify: `wm.sh` `loop` — consume `next` until CLOSE / STOP-ASK / ESCALATE; one slice in flight; no `&`
- Extend: `scripts/verify-working-mode.sh` with a **fixture** reviewer CLI that writes return WORD after exec

**Interfaces:**

- `wm loop` execs `PANEL` commands as children, waits on process exit (no mtime daemon)
- CLOSED PASS only if `invoke/reviewer.log` exists after last maker-build (5c)
- 2c: refuse commands matching live host / `git push` to main / recursive delete

- [ ] **Step 1: Fixture loop test RED**
- [ ] **Step 2: Implement loop**
- [ ] **Step 3: Independent reviewer repeats planted-receipt cheat after maker-build**
- [ ] **Step 4: Commit** `feat: wm loop walks slices; CLOSED requires reviewer exec`

---

### Task 7: Learning (12e)

**Files:**

- Modify: `wm.sh` close path — append exactly one line to `.crucible/<prog>/LESSONS.md` or `NONE`
- Modify: maker brief concatenation (working-mode analogue of RULE 23)
- Extend: verify script

**CHECKs:**

- close without a lesson line refuses
- line matching `^ARCH:` → STOP-ASK, do not start next maker
- `loop-design` debrief may write `proposals/` only; applying a battery patch is a human/refresh KEEP decision
- no writes under `$HOME`

- [ ] **Step 1–4:** RED test, implement, independent review, commit `feat: LESSONS.md and ARCH fence for working-mode`

---

### Task 8: Harness adapters + blank-home proof (13b) + 3-slice walk

**Files:**

- Fill: `adapters/grok.md`, `claude.md`, `codex.md` (how to invoke; no secrets; point `agents.tsv` at CLIs)
- Create: `scripts/verify-working-mode-blank-home.sh`
- Modify: `CHANGELOG.md`, `VERSION` → `1.7.0`, `docs/whats-new.md`

**Independent proof (throwaway temp git repo only):**

1. `HOME=$(mktemp -d)` — no skills there
2. Unpack tarball; `adopt work --managed --working-mode`
3. Fixture 3-slice map along a tiny Python package (not a monster): two module slices + one seam falsifier
4. If real harness CLIs are absent: **STOP** and record INDEPENDENCE_UNAVAILABLE for live agents; still PASS the fixture loop (echo workers)
5. If CLIs exist: mapper process ≠ critique process ≠ maker process; reviewer re-runs falsifier; `wm loop` CLOSED PASS on fixtures or live
6. Guided `verify-agent-cycle.sh` still green on a **separate** adopt without `--working-mode`

- [ ] **Step 1: blank-home script RED** (adopt not projecting skills)
- [ ] **Step 2: Implement adapters + pin docs**
- [ ] **Step 3: Independent reviewer runs blank-home + 3-slice fixture from the tarball**
- [ ] **Step 4: Independent reviewer confirms guided default adopt (no `--working-mode`) still matches 1.6.6 behavior**
- [ ] **Step 5: Commit + changelog 1.7.0** `release: 1.7.0 opt-in self-contained working-mode`

**Do not:** `--refresh` workgraph-jira; live Jira; push to origin from the proof repo.

---

## Verification matrix (what “done” means)

| Ballot | Task | Proof |
| --- | --- | --- |
| 1b | 3, 8 | default adopt has no wm; `--working-mode` has wm |
| 2c | 1, 6 | live/push-main/rm -rf refused |
| 3d | 5 | HIGH + one kind → STOP-ASK; two kinds allowed |
| 4a | 6 | `wm loop` foreground child wait |
| 5c | 1, 6 | no CLOSED PASS without reviewer exec |
| 6b | 5 | slices.tsv along modules |
| 7c | 4, 5 | architecture battery; mapper ≠ maker |
| 8b | 4, 5 | critique ≠ mapper |
| 8c | 5 | MAP-HUMAN required for HIGH/live |
| 9c | 3, 8 | tarball adopt; self-refresh refused |
| 10c | 4 | four batteries only in package required list |
| 11c | 3, 4 | CONTRACT + ROUTING KEEP |
| 12e | 7 | LESSONS + ARCH |
| 13b | 8 | three adapters; blank HOME |
| 14a | 4, 5 | module-fit + CHANGES-ARCHITECTURE |
| 15b | 2, 3 | package `skills/` → `.crucible/skills/` + views + repo-root projection |
| 16a | 2 | one canonical tree |

**Regression:** `scripts/verify-agent-cycle.sh`, `verify-package.sh`, `verify-drive.sh` remain green.

## Out of scope (explicit)

- Promoting working-mode to default (1c)
- Porting full SuperCharge, Batman, GWS, herdr-init
- Loopy run/publish
- Parallel in-slice TASKS.tsv (6d)
- Background waiters
- Auto-`cycle approve`

## Execution

Do not start Tasks 1–8 in the same context that ACCEPTs them. Per task: implementer subagent → reviewer subagent → only then next task.
