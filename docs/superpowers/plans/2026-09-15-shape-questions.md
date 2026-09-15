# 1.11 Shape: no new top-level package without QUESTIONS

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `check-module-fit` / `map-ready` refuse to create a **new** `src/<name>`, `packages/<name>`, or `cmd/<name>` in a tree that already has a product package, unless `QUESTIONS.md` and `ANSWERS.md` are both non-empty. Greenfield (no existing product package dir) still mkdir as today.

**Architecture:** No new battery directory. Shape is architecture Plane A plus this kernel CHECK. Detect existing product packages as directories matching `src/*`, `packages/*`, `cmd/*`. Before mkdir of a missing module `root_path`, if another product package already exists, require QUESTIONS+ANSWERS. Die strings: `new top-level package requires QUESTIONS.md` / `QUESTIONS.md without ANSWERS.md`. Do not invent a `skills/shape` tree (11c + unexpected-battery test).

**Tech Stack:** POSIX `sh`, fixture verify scripts.

**Spec:** Factory 1.11. Worktree `/tmp/crucible-1.11` branch `feat/shape-questions` from `origin/main` `bb92d0f` (1.10.1). Merge when CI green; clean up after.

## Global Constraints

- POSIX `sh`.
- Maker ≠ judge; no CROSS-FAMILY when kinds match; MAP-HUMAN for HIGH/live.
- No Claude Code. 11c: no literal `skills/architecture|critique|review|loop-design` in `wm.sh`.
- No new unexpected package battery under `skills/`.
- TDD: fail first.
- Work only in `/tmp/crucible-1.11`.
- Preserve 1.10.x isolation and Send-back.
- Greenfield `t-greenfield-mkdir` / `t-greenfield-map-ready` stay green (mkdir without QUESTIONS).
- Existing brownfield fixtures that only name already-created `src/widget` stay green.
- Sequential map then kernel then go (never parallel).
- First line of `.wm/CLOSED` stays `CLOSED PASS` / `CLOSED NO-BUILD`.

## File map

- Modify: `wm.sh` (`cmd_check_module_fit` before mkdir)
- Modify: `scripts/verify-working-mode-map.sh` (brownfield new-package tests)
- Modify: `skills/architecture/SKILL.md` + `CONTRACT.md` (kernel CHECK, not only prose)
- Modify: `VERSION` 1.11.0, `CHANGELOG.md`, `docs/whats-new.md`, `WORKING-MODE.md`, blank-home pins

---

### Task 1: Kernel CHECK + map tests

**Files:**
- Modify: `wm.sh` around `cmd_check_module_fit` mkdir (`wm.sh` ~1727)
- Modify: `scripts/verify-working-mode-map.sh` after greenfield tests (~853–889)
- Modify: `skills/architecture/SKILL.md`, `skills/architecture/CONTRACT.md`

**Interfaces:**
- Consumes: `list_module_roots`, `questions_need_ask` (do not call it for the mkdir gate — require both files explicitly)
- Produces:
  - `existing_product_package` — true if any directory `src/*`, `packages/*`, or `cmd/*` exists
  - `is_product_package_root PATH` — true if PATH is `src/<name>`, `packages/<name>`, or `cmd/<name>` (no `..`, not absolute)
  - Before mkdir of missing `_cm_r`: if `existing_product_package` and `is_product_package_root _cm_r` and `_cm_r` does not exist: require `-s QUESTIONS.md` else die `new top-level package requires QUESTIONS.md`; require `-s ANSWERS.md` else die `QUESTIONS.md without ANSWERS.md`

- [ ] **Step 1: Failing tests** in `scripts/verify-working-mode-map.sh` after `t-greenfield-map-ready`:

```sh
# Brownfield: existing src/widget, modules invent src/gadget → refuse without QUESTIONS.
setup_map_repo t-shape-new-pkg-no-q
write_architecture_fixture alice LOW no
printf 'gadget\tsrc/gadget\tsrc/gadget/api.py\ttests/gadget\tsrc/gadget/api.py\tno\n' \
  >> architecture/modules.md
[ -d src/widget ] && ok || bad 'brownfield fixture must already have src/widget'
[ ! -d src/gadget ] && ok || bad 'brownfield fixture must start without src/gadget'
refuses 'new top-level package without QUESTIONS.md' 'QUESTIONS.md' \
  "$WM" check-module-fit
if [ -d src/gadget ]; then
  bad 'must not mkdir src/gadget without QUESTIONS.md'
else
  ok
fi

# QUESTIONS.md without ANSWERS.md still refuses; no mkdir.
setup_map_repo t-shape-new-pkg-q-no-a
write_architecture_fixture alice LOW no
printf 'gadget\tsrc/gadget\tsrc/gadget/api.py\ttests/gadget\tsrc/gadget/api.py\tno\n' \
  >> architecture/modules.md
printf 'Should gadget be a new package or live under widget?\n' > QUESTIONS.md
refuses 'new package QUESTIONS without ANSWERS' 'ANSWERS.md' \
  "$WM" check-module-fit
if [ -d src/gadget ]; then
  bad 'must not mkdir src/gadget without ANSWERS.md'
else
  ok
fi

# QUESTIONS + ANSWERS: mkdir allowed (human chose the packaging).
setup_map_repo t-shape-new-pkg-answered
write_architecture_fixture alice LOW no
printf 'gadget\tsrc/gadget\tsrc/gadget/api.py\ttests/gadget\tsrc/gadget/api.py\tno\n' \
  >> architecture/modules.md
printf 'Should gadget be a new package or live under widget?\n' > QUESTIONS.md
printf 'New package src/gadget.\n' > ANSWERS.md
impl_ok 'new package with QUESTIONS+ANSWERS fits' "$WM" check-module-fit || true
[ -d src/gadget ] && ok || bad 'answered QUESTIONS must allow mkdir src/gadget'
[ -f src/gadget/.gitkeep ] && ok || bad 'answered QUESTIONS mkdir must touch .gitkeep'

# Existing module only (src/widget already there): no QUESTIONS required.
setup_map_repo t-shape-existing-pkg
write_architecture_fixture alice LOW no
impl_ok 'existing package fit without QUESTIONS' "$WM" check-module-fit || true
```

Do not weaken `t-greenfield-mkdir` / `t-greenfield-map-ready`.

- [ ] **Step 2: Run** `sh scripts/verify-working-mode-map.sh`

Expected RED: brownfield mkdir of `src/gadget` currently succeeds (today’s greenfield mkdir path). Not a syntax error.

- [ ] **Step 3: Implement** in `cmd_check_module_fit` before `mkdir -p "$_cm_r"`:

```sh
existing_product_package() {
  _epp=
  for _epp in src packages cmd; do
    [ -d "$_epp" ] || continue
    for _epp_p in "$_epp"/*; do
      [ -d "$_epp_p" ] || continue
      return 0
    done
  done
  return 1
}

is_product_package_root() {
  _ipr=$1
  [ -n "$_ipr" ] || return 1
  printf '%s\n' "$_ipr" | awk -F '/' '
    $0 ~ /\.\./ { exit 1 }
    $0 ~ /^\// { exit 1 }
    NF == 2 && ($1 == "src" || $1 == "packages" || $1 == "cmd") && $2 != "" && $2 != "." {
      exit 0
    }
    { exit 1 }
  '
}
```

Wait: `src/widget` is `src/*` with one slash — `*/*/*` would reject `src/widget` because `src/widget` has one slash... In glob: `src/widget` matches `src/*` and does NOT match `*/*/*` (three components). `src/foo/bar` matches `*/*/*`. Good.

`packages/foo` OK. `src` alone is not `src/*`. `src/foo/bar` is not a top-level package for this CHECK (would still mkdir if we don't classify it — keep invalid? Module roots are supposed to be package dirs. If it's `src/foo/bar` and missing, existing mkdir still happens unless we only gate `is_product_package_root`. That's OK: the factory ask is top-level packages.)

Gate:

```
if [ ! -d "$_cm_r" ]; then
  if is_product_package_root "$_cm_r" && existing_product_package; then
    [ -s QUESTIONS.md ] || die "new top-level package requires QUESTIONS.md"
    [ -s ANSWERS.md ] || die "QUESTIONS.md without ANSWERS.md"
  fi
  mkdir -p "$_cm_r"
  touch "$_cm_r/.gitkeep"
fi
```

Architecture SKILL: replace “Greenfield may create empty packages (kernel mkdir)” with: greenfield mkdir remains; **brownfield new `src/`/`packages/`/`cmd/` package requires QUESTIONS.md + ANSWERS.md; kernel refuses silent mkdir**.

CONTRACT Must-not / Andon: silent new top-level package without QUESTIONS.

- [ ] **Step 4:** Sequential `sh scripts/verify-working-mode-map.sh` then `sh scripts/verify-working-mode.sh` then `sh scripts/verify-working-mode-go.sh`. Greenfield tests still pass.

- [ ] **Step 5: Commit** `feat(wm): refuse new top-level package without QUESTIONS`

---

### Task 2: Version 1.11.0 + docs + pins

**Files:** `VERSION`, `CHANGELOG.md`, `docs/whats-new.md`, `WORKING-MODE.md`, `docs/working-mode.md`, `scripts/verify-working-mode-blank-home.sh` if it pins 1.10.1

Changelog:

```
## [1.11.0] - 2026-09-15

### Shape
- Brownfield `check-module-fit` / `map-ready` will not mkdir a new
  `src/<name>`, `packages/<name>`, or `cmd/<name>` unless `QUESTIONS.md`
  and `ANSWERS.md` are both non-empty. Greenfield still mkdir.
```

- [ ] Grep `1.10.1` pins; update those that would fail.
- [ ] Sequential full verify (`sh -n`, map, kernel, go).
- [ ] Commit `chore: working-mode 1.11.0 shape QUESTIONS gate`

---

## Out of scope

- New `skills/shape` battery / ROUTING row.
- Executing architecture Andon table.
- 1.12 worktrees, 1.13 taste, 1.14 live walk, tags.

## Execution

User: proceed until done with validation. SDD in `/tmp/crucible-1.11`. Push PR; merge when CI green; remove worktree.
