# 1.12 Worktree per bet Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Each in-flight slice (bet) gets a git worktree. Maker and reviewer `wm run` execute in that tree; product is synced back to the main checkout; the worktree is removed when the brick resets/closes.

**Architecture:** Reuse git worktree, not a new engine. When `.wm/slice-in-flight` exists, `ensure_bet_worktree` adds `.wm/worktrees/<id>` on branch `wm/<id>` from HEAD, and symlinks `<wt>/.wm` → the main `.wm` so FALSIFIER/verdicts stay in one place. `cmd_run` for `maker-falsify`, `maker-build`, and `reviewer` `cd`s there for `sh -c`, then copies owned product + `reviews/` back to `$PWD`. `reset_brick` force-removes the worktree. Specifier/scout stay in the main checkout. No worktree if no slice is in flight (existing maker-falsify tests keep working).

**Tech Stack:** POSIX `sh`, `git worktree`. Fixture verify scripts.

**Spec:** Factory 1.12. Worktree `/tmp/crucible-1.12` branch `feat/bet-worktree` from `origin/main` `b2dd6d0` (1.11.0). Merge when CI green; clean up after.

## Global Constraints

- POSIX `sh`. Maker ≠ judge. No CROSS-FAMILY when kinds match. MAP-HUMAN for HIGH/live.
- No Claude Code. 11c: no literal `skills/architecture|critique|review|loop-design` in `wm.sh`.
- TDD: fail first. Work only in `/tmp/crucible-1.12`.
- Preserve 1.10 isolation, 1.10b Send-back, 1.11 QUESTIONS gate.
- Sequential map → kernel → go.
- One slice in flight already. One bet worktree at a time.
- `git worktree remove --force` is allowed **only** for kernel-owned `.wm/worktrees/<id>` after sync.
- First line of `.wm/CLOSED` stays `CLOSED PASS` / `CLOSED NO-BUILD`.

## File map

- Modify: `wm.sh` — `ensure_bet_worktree`, `sync_bet_to_main`, `remove_bet_worktree`; hook `cmd_run`, `reset_brick`, loop NEXT SLICE
- Modify: `scripts/verify-working-mode-map.sh` — bet worktree tests
- Modify: `VERSION` 1.12.0, changelog, docs, blank-home pins

---

### Task 1: Bet worktree kernel + tests

**Files:** `wm.sh`, `scripts/verify-working-mode-map.sh`

**Interfaces:**
- Consumes: `slice-in-flight` id, `owned_paths`, `in_flight_owned_paths`, `cmd_run`, `reset_brick`
- Produces:
  - `ensure_bet_worktree` — if slice id set, `git worktree add -B wm/<id> .wm/worktrees/<id> HEAD`, `ln -sfn "$PWD/.wm" "<wt>/.wm"`, write `.wm/slice-worktree` with `path:` absolute and `slice:`
  - `sync_bet_to_main` — copy existing owned paths + `reviews/*` from worktree to `$PWD`
  - `remove_bet_worktree` — `git worktree remove --force` that path, delete `wm/<id>` branch, rm `slice-worktree`
  - `cmd_run` maker-falsify|maker-build|reviewer: ensure + `cd` worktree for `sh -c` + sync after (even on nonzero rc, then proceed with existing CHECKs)
  - `reset_brick` calls `remove_bet_worktree` first
  - Loop NEXT SLICE after writing `slice-in-flight` calls `ensure_bet_worktree`

Sanitize slice id: only `[A-Za-z0-9._-]`; else die.

- [ ] **Step 1: Failing tests** in `scripts/verify-working-mode-map.sh`:

```sh
# In-flight slice: maker-falsify runs in a git worktree.
setup_map_repo t-bet-worktree
write_architecture_fixture alice LOW no
impl_ok 'record-mapper bet wt' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready bet wt' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict bet wt' "$WM" map-verdict .wm/return/bob.md || true
write_spec_fit
"$WM" cast maker carol grok './tools/true-falsify.sh {BRIEF}' >/dev/null 2>"$ERR" || true
# true-falsify may not exist; reuse write_map_loop_brick maker-falsify path or a tiny stub:
mkdir -p tools
cat > tools/bet-falsify.sh <<'EOF'
#!/bin/sh
set -eu
mkdir -p .wm src/widget
printf 'test -f src/widget/api.py\n' > .wm/FALSIFIER
if command -v sha256sum >/dev/null 2>&1; then
  h=$(sha256sum .wm/FALSIFIER | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
  h=$(shasum -a 256 .wm/FALSIFIER | awk '{print $1}')
else
  h=$(openssl dgst -sha256 .wm/FALSIFIER | awk '{print $NF}')
fi
wid=$(git rev-parse --short=12 HEAD)
printf 'agent: carol\nwork-id: %s\nsha256: %s\n' "$wid" "$h" > .wm/FALSIFIER.meta
printf '\nBET-WT\n' >> src/widget/api.py
EOF
chmod +x tools/bet-falsify.sh
"$WM" cast maker carol grok './tools/bet-falsify.sh {BRIEF}' >/dev/null
printf 'id: s1\n' > .wm/slice-in-flight
impl_ok 'maker-falsify in bet worktree' "$WM" run maker-falsify || true
[ -f .wm/slice-worktree ] && grep -q '^path: ' .wm/slice-worktree \
  && ok || bad "slice-worktree missing after maker-falsify, $(cat .wm/slice-worktree 2>/dev/null || echo ABSENT)"
wt=$(awk -F ': ' '$1=="path"{print $2; exit}' .wm/slice-worktree)
[ -n "$wt" ] && [ -e "$wt/.git" ] || [ -f "$wt/.git" ] \
  && ok || bad "bet worktree path missing: $wt"
git worktree list | grep -q "worktrees/s1" \
  && ok || bad "git worktree list must contain worktrees/s1: $(git worktree list)"
grep -q BET-WT src/widget/api.py \
  && ok || bad 'product write must sync back to main checkout'
[ -f .wm/FALSIFIER ] && ok || bad 'FALSIFIER must land in main .wm (symlink)'

# Specifier does not create a bet worktree.
setup_map_repo t-bet-no-wt-specifier
write_architecture_fixture alice LOW no
"$WM" cast specifier spec0 grok 'true' >/dev/null
"$WM" run specifier >/dev/null 2>"$ERR" || true
if [ -f .wm/slice-worktree ] || ls .wm/worktrees/* >/dev/null 2>&1; then
  bad 'specifier run must not create a bet worktree'
else
  ok
fi

# Full loop still CLOSED PASS and worktree is gone after.
setup_map_repo t-bet-loop-cleanup
write_architecture_fixture alice LOW no
impl_ok 'record-mapper bet loop' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready bet loop' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict bet loop' "$WM" map-verdict .wm/return/bob.md || true
write_map_human operator MAP.md
write_spec_fit
write_map_loop_brick
"$WM" cast maker carol grok './tools/map-loop-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer dave grok './tools/map-loop-reviewer.sh {BRIEF}' >/dev/null
git add -A && git commit -qm bet-loop >/dev/null
run_map_loop
assert_map_loop_foreground 't-bet-loop-cleanup'
if grep -q 'CLOSED PASS' "$OUT" && [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  ok
else
  bad "bet loop wanted CLOSED PASS, got out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT)"
fi
if git worktree list | grep -q worktrees; then
  bad "bet worktree must be removed after CLOSE, list=$(git worktree list)"
else
  ok
fi
[ ! -f .wm/slice-worktree ] && ok || bad 'slice-worktree must be gone after CLOSE'
```

If `write_map_loop_brick` already casts maker/reviewer, don't double-cast. Reuse the same pattern as `t-loop-slice-pass`. `true-falsify` in the first test: if `cast` twice, last wins — only cast once with bet-falsify.sh.

`t-loop-slice-pass` must remain green (worktree created and torn down inside the loop).

- [ ] **Step 2:** `sh scripts/verify-working-mode-map.sh` — expect RED: no slice-worktree.

- [ ] **Step 3: Implement** helpers near `reset_brick`. Slice id from `kv_get "$WM/slice-in-flight" id`. Path `$WM/worktrees/<id>`. `cmd_run` wrap:

```sh
_ru_wt=
if [ -f "$WM/slice-in-flight" ]; then
  case $_ru_role in
    maker-falsify|maker-build|reviewer)
      ensure_bet_worktree
      _ru_wt=$(kv_get "$WM/slice-worktree" path)
      ;;
  esac
fi
set +e
if [ -n "$_ru_wt" ] && [ -d "$_ru_wt" ]; then
  ( cd "$_ru_wt" && sh -c "$_ru_expanded" ) > "$WM/worker.out" 2>"$WM/worker.err"
  _ru_rc=$?
  sync_bet_to_main "$_ru_wt"
else
  sh -c "$_ru_expanded" > "$WM/worker.out" 2>"$WM/worker.err"
  _ru_rc=$?
fi
set -e
```

`sync_bet_to_main` even when rc ≠ 0 so mutating-reviewer wall still sees the write.

`owned_product_changed` after sync (main tree).

`ensure_bet_worktree` is idempotent if path already a worktree.

Loop NEXT SLICE: after writing slice-in-flight, `ensure_bet_worktree`.

- [ ] **Step 4:** Sequential map, kernel, go. `t-loop-slice-pass` and `t-loop-high-one-kind` stay green.

- [ ] **Step 5: Commit** `feat(wm): run each in-flight slice in a git worktree`

---

### Task 2: Version 1.12.0 + docs

`VERSION` 1.12.0. Changelog:

```
## [1.12.0] - 2026-09-15

### Bet worktree
- An in-flight slice gets `.wm/worktrees/<id>` (git worktree). Maker and
  reviewer `wm run` execute there. Product/`reviews/` sync back. CLOSE
  removes the worktree. Specifier/scout stay in the main checkout.
```

Grep `1.11.0` pins. Sequential full verify. Commit `chore: working-mode 1.12.0 bet worktree`.

---

## Out of scope

- Per-role worktrees (maker vs reviewer separate trees).
- Parallel slices.
- 1.13 taste, 1.14 live walk, tags.

## Execution

SDD in `/tmp/crucible-1.12`. PR; merge when CI green; remove worktree.
