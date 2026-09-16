# 1.13 test_entrypoint run, mutation extra-proof, INTENT headings, taste after CLOSE

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Four kernel CHECKs: (1) `INTENT.md` has `## User` `## Job` `## Non-goals` before `map-ready`; (2) named `test_entrypoint` exists; (3) `wm green` extra-proof: hiding `test_entrypoint` must make the FALSIFIER fail; (4) after CLOSE, write `reviews/taste.md` with the lesson line.

**Architecture:** Extend existing hooks. Do not add a new battery. Extra-proof runs only when `check_falsifier_test_entrypoint` has a real path (slice-in-flight + modules). `reach_green` without modules stays unchanged. Do not forbid `close NONE`. Do not require `## Taste` in `reviews/review.md` (that would churn every PASS fixture).

**Worktree:** `/tmp/crucible-1.13` `feat/entrypoint-taste` from `origin/main` `948708b` (1.12.0). Merge when CI green.

## Global Constraints

- POSIX `sh`. 11c. No Claude. Sequential map → kernel → go.
- Preserve 1.10–1.12 isolation, Send-back, QUESTIONS, bet worktree.
- Extra-proof must **restore** the test_entrypoint even if the command fails (`set +e` around the run).
- Do not hide paths under `.wm/`.
- TDD fail first. Work only in `/tmp/crucible-1.13`.

## File map

- `wm.sh` — `intent_ok`, `test_entrypoint` exists, `falsifier_extra_proof`, `cmd_map_ready`, `cmd_green`, `cmd_close`
- `scripts/verify-working-mode-map.sh` — INTENT headings, extra-proof, taste receipt
- `scripts/verify-working-mode.sh` — extra-proof / missing te path if easier in kernel fixtures
- `skills/review/SKILL.md` or architecture — one line that extra-proof exists
- `VERSION` 1.13.0, changelog, docs, blank-home pins

---

### Task 1: Kernel CHECKs + tests

**INTENT**

```sh
intent_ok() {
  [ -f INTENT.md ] || return 1
  grep -q '^## User$' INTENT.md || grep -q '^## User' INTENT.md || return 1
  grep -q '^## Job' INTENT.md || return 1
  grep -q '^## Non-goals' INTENT.md || return 1
}
```

`cmd_map_ready`: `[ -f INTENT.md ] || die`; `intent_ok || die "INTENT.md missing ## User / ## Job / ## Non-goals"`.

**test_entrypoint exists** in `check_falsifier_test_entrypoint` after resolving path:

```
[ -e "$_te_path" ] || die "test_entrypoint missing"
```

**extra-proof** `falsifier_extra_proof CMD` after green FALSIFIER success, same guards as te check. `mv` te aside into `$WM/.te-mut.$$`, run CMD, restore, require nonzero. Die `falsifier does not discriminate`.

**taste** in `cmd_close` after writing CLOSED: `mkdir -p reviews`; `printf '## Taste\n%s\n' "$_cl_lesson" > reviews/taste.md` for both PASS and NO-BUILD.

- [ ] **Step 1: Tests**

Map suite after INTENT missing test:

```sh
setup_map_repo t-intent-headings
write_architecture_fixture alice LOW no
impl_ok 'record-mapper intent headings' "$WM" record-mapper --from MAP.md || true
printf 'no headings\n' > INTENT.md
refuses 'map-ready INTENT without headings' 'INTENT.md' "$WM" map-ready
```

Extra-proof (map or kernel): slice-in-flight, modules te `tests/widget`, FALSIFIER cites the path but does not depend on it, e.g. `grep -q tests/widget /dev/null || true` — wait `|| true` always passes. Use `printf 'grep -F tests/widget /dev/null\n'` which cites the path, succeeds with te present (grep /dev/null fails actually!).

`grep -F tests/widget /dev/null` exits 1 always. Bad for green.

Use: `test -d tests/widget -o -f /` which is always true if we have `-o -f /`... that's tautological in effect.

Better discriminating fixture:
- Honest FALSIFIER: `test -d tests/widget` — extra-proof hide dir, must fail. GREEN after product exists.
- Cheat FALSIFIER: `test -d /` plus the path as a no-op: `test -d / # tests/widget` — grep -F cites tests/widget, extra-proof hide tests/widget, `test -d /` still passes → refuse discriminate.

Need to pass tautology (not true/: /exit 0). `test -d / # tests/widget` works.

Kernel-style fixture: setup_map_repo, architecture with tests/widget, slice-in-flight, write FALSIFIER `test -d / # tests/widget` with meta, record-pre-falsify, red (this FALSIFIER passes so red is NO-BUILD not RED!). Problem: red expects fail for RED path.

Extra-proof is on **green**, which requires FALSIFIER to pass on current tree first, then fail when te hidden.

Honest path: FALSIFIER `test -d tests/widget`. Red: if tests/widget exists, red is NO-BUILD not extra-proof. Extra-proof still runs on green after NO-BUILD? cmd_green still runs for no-build path? Loop NO-BUILD skips green.

For GREEN path: FALSIFIER fails on red (missing marker), passes after build. Extra-proof: hide tests/widget, `grep marker && test -d tests/widget` fails. Map-loop already has that FALSIFIER. Add assertion after t-loop-slice-pass or a dedicated green:

Simplest extra-proof test: after `t-loop-slice-pass` equivalent, or dedicated:

```sh
setup_map_repo t-extra-proof-cheat
# modules te tests/widget (exists via fixture)
# slice-in-flight s1, slices.tsv
# FALSIFIER: test -f src/widget/api.py # tests/widget
# This cites te, passes while te hidden too if api.py remains
# extra-proof hides tests/widget, command still passes → refuse on green
```

Need red to fail first: `test -f src/widget/MISSING # tests/widget` then after build create MISSING... messy.

Dedicated helper that plants green.status path:

Call extra-proof from `cmd_green` only. Fixture:

1. setup_map_repo + write_architecture_fixture (has tests/widget)
2. map-ready path + spec + slice-in-flight
3. FALSIFIER `test -f src/widget/api.py # tests/widget` (fails red if we delete api.py first)

Easier: don't go through red. Unit-test extra-proof via `wm green` after planting green preconditions... green requires FALSIFIER.sha256 from red.

Full walk:
- delete api.py content so `test -f src/widget/api.py` still true (file exists)
- For cheat: FALSIFIER `test -f src/widget/api.py # tests/widget`
- red: file exists → NO-BUILD (status 0). Extra-proof not on red.

Hmm extra-proof on green only. For NO-BUILD we skip extra-proof. Need a RED then GREEN walk.

Cheat FALSIFIER that **fails** on empty product then **passes** after file exists, and does **not** use te:
`test -s src/widget/api.py # tests/widget`
- Start: truncate api.py to empty, red fails (good RED)
- Build: write content, green passes
- Extra-proof hide tests/widget: `test -s src/widget/api.py` still passes → die discriminate

Honest: `test -s src/widget/api.py && test -d tests/widget` — extra-proof hide dir, fails, green ok.

Two tests: cheat refuses; honest loop still CLOSED PASS (map-loop-maker already uses tests/widget).

Taste test: after CLOSED PASS, `[ -f reviews/taste.md ]` and grep the lesson.

- [ ] **Step 2:** RED map suite
- [ ] **Step 3:** implement
- [ ] **Step 4:** sequential map, kernel, go
- [ ] **Step 5:** commit `feat(wm): INTENT headings, te extra-proof, taste after CLOSE`

---

### Task 2: Version 1.13.0 + docs

VERSION 1.13.0. Changelog Shape-style bullets. Grep 1.12.0 pins. Sequential verify. Commit `chore: working-mode 1.13.0 entrypoint taste`.

---

## Out of scope

- Forbidding `close NONE`
- `## Taste` in review.md
- 1.14 live walk, tags

## Execution

SDD in `/tmp/crucible-1.13`. PR; merge when CI green; remove worktree.
