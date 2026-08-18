# Long-horizon loop (1.4.0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. This session executes inline.

**Goal:** Make a guided+managed Crucible cycle survivable for multi-day agent work: dead processes do not trap `--next`, `drive tick` is one role, close/admit bars match the approved panel, stale evidence can be archived, cycle names NO-BUILD/DOCS-ONLY, husks refuse refresh clearly, and agents can install or upgrade from docs alone.

**Architecture:** Keep the file-only POSIX engine. Fix POSIX variable clobber (`drive_mode` not `sub`). Add two operator/coordinator verbs (`attempt reclaim`, `evidence archive`). Derive default auditor/judge minima from `PANEL.ASSIGN.tsv` required rows unless `CRUCIBLE_MIN_*` is set. Project `worth:` on `STATUS.md`. Do not invent `--force` or skip independence.

**Tech Stack:** POSIX `sh` (`crucible`), `scripts/verify-drive.sh`, `scripts/verify-agent-cycle.sh`, `scripts/verify-coldstart-independence.sh`.

## Global Constraints

- Do not revert 9d7b41b / 96a8515 / 17f0a4f or rewrite tagged 1.3.6 / 1.3.7.
- Version this release **1.4.0**. Changelog date must match `date +%F`.
- No WorkGraph product edits. No live Jira. No PyPI.
- Tests must stay fixture-only (echo agents, no real ACP).
- After merge: tag `v1.4.0`, GitHub release, verify checksums, leave `main` clean.

## File map

- Modify: `crucible` (drive_mode, reclaim, archive, panel mins, worth, husk)
- Modify: `scripts/verify-drive.sh`, `scripts/verify-agent-cycle.sh`
- Modify: `docs/install.md` (first-time vs upgrade), `START.md`, `BOOTSTRAP.md`, `docs/drive.md`, `LOOP.md`, `README.md`
- Create: this plan
- Modify: `VERSION`, `CHANGELOG.md`

---

### Task 1: `drive tick` uses `drive_mode` (loop CHECK)

`cmd_attempt` sets `sub=start|finish` and clobbers `cmd_drive`'s `sub`. After the first worker, `case $sub in tick)` misses and the tick **continues**.

- [x] Rename `cmd_drive` local `sub` → `drive_mode`
- [x] Test: after a sealed worker, `drive tick` exits 0 and does not start a second attempt in the same tick

### Task 2: `attempt reclaim` + `--next` on dead pid

- [x] `attempt reclaim ATTEMPT` if RUNNING/OVERDUE and `kill -0` fails (or pid is `-`)
- [x] Claim attempts: STOPPED, no item BLOCK
- [x] Item attempts: STOPPED + OPERATOR_DECISION (same as finish STOPPED)
- [x] `cycle problem --next` and `cycle_live_attempts_terminal` reclaim dead RUNNING then continue
- [x] Live pid still refuses reclaim and `--next`

### Task 3: `evidence archive SLUG`

- [x] Move `items/SLUG/evidence/*` files whose name work-id ≠ `workid SLUG` into `evidence/history/`
- [x] `check` already skips non-files (history dir)
- [x] Test: stale file fails check; archive; check no longer cites that file

### Task 4: Panel-bound admit/close bars

- [x] `guided_min_auditors` / `guided_min_judges`: env wins if `CRUCIBLE_MIN_*` set; else required `claim-auditor` / `reviewer` rows on guided panels; else 2
- [x] Tests: one required reviewer → `check` needs 1 unless env=2

### Task 5: Worth projection + husk refresh

- [x] `STATUS.md` `worth: BUILD|DOCS|NO-BUILD|UNKNOWN`
- [x] After COMPLETE with no scout ABSENT: PROPOSE line says no ABSENT capability
- [x] `adopt NAME --refresh` on a directory without PROGRAM: husk refusal

### Task 6: Docs (agent-first)

- [x] `docs/install.md`: First install vs upgrade from 1.3.x
- [x] START / BOOTSTRAP / drive / LOOP / README point at it

### Task 7: Ship

- [x] `VERSION` 1.4.0, changelog, verify suites, PR, green CI, merge, tag, release, clean
