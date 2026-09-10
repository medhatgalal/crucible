# Working-mode unattended envelope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One foreground `wm loop` walks every READY slice to a verified close (or honest STOP-ASK) without a test harness resetting brick state, and a live throwaway-repo gate proves distinct harness processes when CLIs exist.

**Architecture:** 1.7.0 kit stays. Slice-close is not work-close: after a closeable brick, mark that `slices.tsv` row CLOSED, clear brick receipts, pick the next READY whose `depends_on` are CLOSED, continue. Work-level `.wm/CLOSED` + exit 0 only when no READY slice remains. Unattended is legal only for LOW + not live + MAP-ACCEPT. HIGH/live/ARCH/2c/one-kind stay STOP-ASK. Do not auto-write `MAP-HUMAN`.

**Tech Stack:** POSIX `sh` (`wm.sh`, `scripts/verify-*.sh`), fixture echo/python workers, optional real grok/claude/codex in a throwaway repo with empty `HOME`.

**Spec:** Operator ballot unchanged (`1b, 2c, 3d, 4a, 5c, 6b, 7c, 8b, 8c, 9c, 10c, 11c, 12e, 13b, 14a, 15b, 16a`). Parent plan: `docs/superpowers/plans/2026-09-10-self-contained-working-mode.md`. This plan closes the mission holes that 1.7.0 left: one-brick loop, harness-reset 3-slice, skipped live arm, START/BOOTSTRAP omit working-mode, `cmd_run` always returns 0.

## Global Constraints

- Do not change guided 1.6.6 default behavior. `scripts/verify-agent-cycle.sh`, `verify-drive.sh`, `verify-package.sh` that pass on 1.7.0 must still pass.
- Do not modify `~/Desktop/workgraph-jira`. No origin push. No live Jira.
- No `$HOME` skill trees as a runtime dependency.
- Maker ≠ judge. Controller does not implement product or stamp PASS.
- No background processes. Foreground only. Live/destroy/push-main STOP-ASK (2c).
- Do not weaken 5c: CLOSED PASS still requires reviewer exec after last maker-* of **that brick**.
- Do not auto-write `MAP-HUMAN`. Do not skip 8c/3d/ARCH.
- Unattended legal set: `risk=LOW`, `live_write≠yes`, `MAP-ACCEPT` recorded. Unattended illegal set (STOP-ASK): HIGH, live fence, `^ARCH:`, one-kind HIGH, 2c, ESCALATE.
- Version this work **1.7.1**. Changelog date = `date +%F` on the release commit (Task C).
- `/tmp/crucible-protocol.IjzbAa/engine/` is evidence, not the blessed tree.
- Do not copy 1.7.0 `scripts/verify-working-mode-blank-home.sh` `mark_slice_closed` / `reset_brick` into the kernel as a test-only workaround; the kernel must own those steps.

## Independent verification protocol (every task)

1. Falsifier first: write/extend `scripts/verify-*.sh`, run RED before the product change.
2. Implementer ≠ reviewer. A different subagent re-runs the verify script and named cheats. The implementer may not ACCEPT their own work.
3. Every ACCEPT names command + exit code + a path.
4. After touching `crucible` or `adopt`, run `scripts/verify-agent-cycle.sh`.
5. Working-mode verifies use `HOME` set to an empty temp dir.

## File map

Modify:

- `wm.sh` — `first_ready_slice` honors CLOSED deps; `cmd_loop` slice-close then continue; brick reset after slice-close; `cmd_run` returns worker rc
- `scripts/verify-working-mode.sh` — one-invocation 3-slice walk; planted CLOSED still refused mid-map; HIGH unsigned after LOW STOP-ASK
- `scripts/verify-working-mode-map.sh` — depends_on CLOSED; no harness reset
- `scripts/verify-working-mode-blank-home.sh` — **one** `wm loop`; delete `mark_slice_closed` / `reset_brick` from the walk
- `START.md`, `BOOTSTRAP.md` — one-line opt-in pointer; guided examples stay `--managed` without the flag
- `docs/working-mode.md` — invoke `.crucible/<program>/wm.sh` from target root
- `CHANGELOG.md`, `VERSION` → `1.7.1`, `docs/whats-new.md`

Create:

- `scripts/verify-working-mode-live.sh` — live independence gate

Do not modify: guided cycle verbs; workgraph-jira; host `~/.grok`.

---

### Task 1: Multi-slice walker (one `wm loop`)

**Ballot:** 4a, 5c, 6b. Mission: unattended LOW map without harness reset.

**Files:**

- Modify: `wm.sh` (`first_ready_slice`, `cmd_loop` `NEXT CLOSE` / top-of-loop `closed_is_closeable`, new `reset_brick` / `mark_slice_status`)
- Modify: `scripts/verify-working-mode.sh`
- Modify: `scripts/verify-working-mode-map.sh`
- Modify: `scripts/verify-working-mode-blank-home.sh` (walk must be one `wm loop`; no `mark_slice_closed` / `reset_brick` in the script)

**Interfaces:**

- Consumes: 1.7.0 `slices.tsv` header `id module owned_paths depends_on risk status`; `cmd_loop` currently `exit 0` on first `NEXT CLOSE`; `first_ready_slice` only prints READY rows whose `depends_on` is `-` or empty (it never waits on CLOSED parents).
- Produces:
  - `first_ready_slice` prints the first READY id whose every `depends_on` token is `-`/empty **or** a slice id whose status is CLOSED. Tokens split on comma.
  - After a closeable brick on slice S: append lesson as today; set S status CLOSED in `slices.tsv`; if another READY slice exists, **clear brick receipts** (same set as today’s blank-home `reset_brick`: `.wm/CLOSED`, FALSIFIER*, red/built/green, last-maker-run, reviewer-ran, slice-in-flight, dispatch, worker.*, verdicts/return/invoke/spawn/briefs/evidence dirs recreated empty), clear loop `ran_reviewer` / in-flight id, **continue**. If none remain: keep work-level `.wm/CLOSED`, print it, `exit 0`.
  - One slice in flight: a second `NEXT SLICE` with a different id while in-flight is unset-after-reset is OK; while in-flight is set to S, a different id is STOP-ASK.
  - 5c: each brick still requires reviewer exec after that brick’s last maker-*. Planted `.wm/CLOSED` before any closeable brick still refuses. After slice-close reset, a planted CLOSED must not skip the next brick’s reviewer.
  - HIGH unsigned next slice: after LOW s1 closes, if s2 is HIGH and no valid `MAP-HUMAN`, loop prints `STOP-ASK MAP-HUMAN` (or `STOP-ASK`), exit 1, s2 maker not exec’d.

- [ ] **Step 1: Write failing assertions** in `scripts/verify-working-mode.sh` (and map/blank-home as needed)

```sh
# (A1) three LOW slices, s2 depends_on=s1, s3 depends_on=s2
#     ONE `wm loop` invocation; script must not rewrite slices.tsv or rm .wm/CLOSED
#     expect: three CLOSED PASS in the one stdout-or three slice rows status=CLOSED,
#     product of s1 then s2 then s3 landed, work-level CLOSED PASS, loop exit 0
# (A2) planted `printf 'CLOSED PASS\n' > .wm/CLOSED` before loop still must not
#     print CLOSED PASS / exit 0 without reviewer exec
# (A3) after honest s1, if test plants CLOSED during s2 brick, s2 must not CLOSED PASS
#     without reviewer exec
# (A4) s1 LOW then s2 HIGH unsigned: loop STOP-ASK after s1; s2 FALSIFIER absent
# (A5) blank-home 3-slice: grep the verify script itself — walk must not define
#     mark_slice_closed or reset_brick; a single `wm loop` must produce 3 CLOSED rows
```

- [ ] **Step 2: Run RED** — current kernel exits after first CLOSE; blank-home still has harness reset; expect non-zero
- [ ] **Step 3: Implement** until GREEN. Keep Task 1–7 kernel CHECKs (183) green. `LOOP_BOUND` may rise only if 3× brick steps exceed 40 (document the new integer).
- [ ] **Step 4: Independent reviewer** re-runs kernel + map + blank-home with empty HOME; confirms **one** `wm loop` in blank-home; tries A2/A4 cheats in a throwaway repo.
- [ ] **Step 5: Commit** `feat: wm loop walks remaining READY slices without harness reset`

**Stop:** Do not bump VERSION. Do not invoke grok/claude/codex. Do not auto-write MAP-HUMAN.

---

### Task 2: Live independence gate

**Ballot:** 13b live arm. Mission: throwaway repo + real CLIs, distinct processes.

**Files:**

- Create: `scripts/verify-working-mode-live.sh`
- Modify: `scripts/package-release.sh` / `scripts/verify-package.sh` only if the live script must be in the tarball (prefer **not** packaging the live proof script if it needs host CLIs; keep it in the source tree and run from the worktree against an extracted tarball target)

**Interfaces:**

- Consumes: Task 1 one-loop walker; `scripts/package-release.sh`; adapters
- Produces: live gate script that:
  1. `HOME=$(mktemp -d)`
  2. Builds/uses `crucible-$VERSION.tar.gz` of HEAD; extract **outside** the worktree; `adopt work --managed --working-mode` into a throwaway git repo
  3. Tiny LOW idea (e.g. `product.txt` / one-module Python), MAP-ACCEPT already recorded by **fixture** architecture/critique **or** by real CLIs if you can keep it LOW and bounded
  4. If `command -v grok`, `claude`, and `codex` are not all present: print `INDEPENDENCE_UNAVAILABLE` and **exit 1** for this script (fixture suites remain green). Do not print `LIVE_CLIS_PRESENT` and exit 0.
  5. If CLIs exist: cast mapper/critique/maker/reviewer so **four PIDs differ** (or at least mapper ≠ critique ≠ maker ≠ reviewer as the brief requires). Reviewer command must re-run the named falsifier. `wm loop` (one invocation) CLOSED PASS or honest `CLOSED NO-BUILD`. Cite invoke.log `writer: wm-run` and a product path.
  6. `find "$HOME"` has no `SKILL.md` / `.grok/skills` / `.claude/skills` / `.agents/skills`
  7. Do not push origin; do not write under `$HOME` skill trees

- [ ] **Step 1: RED** — script missing or live arm still `LIVE_CLIS_PRESENT` + fixtures + exit 0
- [ ] **Step 2: Implement**
- [ ] **Step 3: Independent reviewer runs `scripts/verify-working-mode-live.sh` with empty HOME from the worktree against a tarball extract.** ACCEPT only with command+exit+path. If CLIs exist and the walk fails, REVISE (do not waive). If CLIs absent, ACCEPT the **unavailable** fail (exit 1 + `INDEPENDENCE_UNAVAILABLE`) as the honest live gate, and record that the mission live bar is blocked by environment — still not a fixture PASS.
- [ ] **Step 4: Commit** `feat: live independence gate fails closed when CLIs missing or walk fails`

**Stop:** Do not bump VERSION. Do not refresh workgraph-jira. Guided adopt without `--working-mode` still has no wm.

---

### Task 3: Discoverability + `cmd_run` rc + 1.7.1

**Files:**

- Modify: `START.md`, `BOOTSTRAP.md` — **one** opt-in sentence + pointer to `docs/working-mode.md`. Guided install line stays `adopt work --managed` (no `--working-mode`).
- Modify: `docs/working-mode.md` — commands use `.crucible/<program>/wm.sh` from the target repo root
- Modify: `wm.sh` `cmd_run` — after existing judge/WORD checks, `return "$_ru_rc"` (not `return 0`). Maker-falsify/build that `die` still die. Reviewer missing WORD still die.
- Modify: `scripts/verify-working-mode.sh` — `wm run maker-build` of a command `false` exits non-zero; does not CLOSED PASS
- Modify: `CHANGELOG.md`, `VERSION` → `1.7.1`, `docs/whats-new.md`
- Modify: `scripts/verify-package.sh` if VERSION pin requires it

**Interfaces:**

- Consumes: Task 1 walker; Task 2 live script (leave it passing as it did)
- Produces: 1.7.1 pin; operator can find working-mode from START/BOOTSTRAP without making it default

- [ ] **Step 1: RED** — START/BOOTSTRAP have no working-mode; `wm run` of `false` returns 0
- [ ] **Step 2: Implement**
- [ ] **Step 3: Independent reviewer** greps START/BOOTSTRAP (guided default still no flag); `wm run` rc; `scripts/verify-agent-cycle.sh`; `scripts/verify-package.sh` `PACKAGE-OK 1.7.1`; kernel/map/blank-home still green
- [ ] **Step 4: Commit** `release: 1.7.1 unattended multi-slice and live gate`

---

## Verification matrix (mission ACCEPT)

| Mission bar | Task | Proof |
| --- | --- | --- |
| Kit 1.7.0 unchanged as default | 1–3 | guided cycle/drive/package green; default adopt has no wm |
| One `wm loop` walks the map | 1 | 3 CLOSED rows, no harness reset, depends_on honored |
| 5c per brick | 1 | planted CLOSED mid-map refuses |
| Unattended envelope | 1 | LOW walks; HIGH unsigned STOP-ASK |
| Live independence | 2 | distinct PIDs + falsifier re-run **or** honest `INDEPENDENCE_UNAVAILABLE` exit 1 |
| Discoverable opt-in | 3 | START/BOOTSTRAP pointer; examples still guided |
| Worker rc | 3 | `false` maker-build non-zero |

## Out of scope

- 1c default flip
- Auto-`MAP-HUMAN`
- MAP-HUMAN hash-bind, grok `{BRIEF}` quotes, scoped `pgrep`, double-close LESSONS (minors)
- CROSS-FAMILY / second OS user
- Parallel in-slice TASKS (6d)
- Background waiters
- Guided `drive` stall
