# 1.14 Live walk: two-kind or one-kind isolated

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development.

**Goal:** `scripts/verify-working-mode-live.sh` proceeds with **one** of grok/kiro-cli/codex as `SUBAGENT-ISOLATED` (distinct agent ids). Two or more kinds still split maker/reviewer (`CROSS-FAMILY`). Zero CLIs still `INDEPENDENCE_UNAVAILABLE`. Never fake CROSS-FAMILY. Worker card writes `INTENT.md` headings and plants `tests/test_health.py` so 1.13 CHECKs pass.

**Architecture:** Same live script. `LIVE_N -lt 1` (not `-lt 2`) is unavailable. If `_live_k2` empty, set it to `_live_k1`. After CLOSE, assert independence label. PATH-stripped / zero CLI still exit 1. Not a CI gate. Claude Code still unused.

**Worktree:** `/tmp/crucible-1.14` `feat/live-one-kind` from `cfd5c19`. Merge when CI green (kernel/map/go). Live walk is extra proof, run once from this host (all three CLIs present).

## Global Constraints

- POSIX sh. No Claude. kiro is `chat --no-interactive`, not acp.
- Distinct agent ids always. MAP-HUMAN still required for HIGH/live; this IDEA is LOW.
- 1.13: INTENT headings; te exists at falsify; extra-proof on green; taste.md on CLOSE.
- Sequential map → kernel → go. Live walk last (long, empty HOME + copied auth).
- TDD: go/map greps of the live script fail first.

## File map

- `scripts/verify-working-mode-live.sh`
- `scripts/verify-working-mode-go.sh` (script-contract greps)
- `VERSION` 1.14.0, changelog, docs, blank-home pins

---

### Task 1: Live script one-kind + 1.13 worker card + greps

**LIVE_N**

```sh
if [ "$LIVE_N" -lt 1 ]; then
  die_unavail "live grok/kiro-cli/codex CLI missing (need >=1; ...)"
fi
# after assigning k1/k2:
[ -n "$_live_k2" ] || _live_k2=$_live_k1
```

Header comments: one-kind proceeds SUBAGENT-ISOLATED; two kinds CROSS-FAMILY; zero still unavailable.

**After CLOSED**, in the walk:

```sh
iso=$(awk -F ': ' '$1=="independence"{print $2; exit}' .wm/CLOSED)
if [ "$LIVE_N" -ge 2 ]; then
  [ "$iso" = CROSS-FAMILY ] && ok || bad "two-kind live CLOSED wanted CROSS-FAMILY, got $iso"
else
  [ "$iso" = SUBAGENT-ISOLATED ] && ok || bad "one-kind live CLOSED wanted SUBAGENT-ISOLATED, got $iso"
  grep -q CROSS-FAMILY .wm/CLOSED && bad 'one-kind must not fake CROSS-FAMILY' || ok
fi
```

**WORKER.md specifier:** write `INTENT.md` with `## User` `## Job` `## Non-goals`. Plant `tests/test_health.py` as `import sys; sys.exit(1)` (exists at falsify). Keep te path `tests/test_health.py`. Maker-build overwrites the test.

**Go suite** (source contract, no live CLIs):

```sh
require_fgrep live.sh 'need >=1' ...
grep -q 'LIVE_N" -lt 1' or equivalent
grep SUBAGENT-ISOLATED
grep CROSS-FAMILY
# must not still require -lt 2 as the unavailable gate
if grep -q 'need >=2' "$HERE/scripts/verify-working-mode-live.sh"; then bad; fi
```

Use the same require helpers as that file, or simple grep.

- [ ] Step 1: greps first → RED (`need >=2` still there)
- [ ] Step 2: implement
- [ ] Step 3: sequential map, kernel, go (not the live script yet)
- [ ] Step 4: commit `feat(wm): live walk one-kind SUBAGENT-ISOLATED`

---

### Task 2: Version 1.14.0 + docs

VERSION 1.14.0. Changelog: live one-kind; two-kind still CROSS-FAMILY; zero still unavailable. Pins. Sequential verify. Commit `chore: working-mode 1.14.0 live one-kind`.

---

### Task 3 (controller): run live walk

From `/tmp/crucible-1.14` with host PATH (grok+kiro-cli+codex present):

```sh
HOME="$HOME" sh scripts/verify-working-mode-live.sh
```

(The script copies auth into empty HOME itself.) Expect CLOSED PASS/NO-BUILD, CROSS-FAMILY (three CLIs → two kinds), four PIDs. Record pass counts. Do not print secrets.

If the walk fails, REVISE (do not waive). Fail-closed auth is INDEPENDENCE_UNAVAILABLE exit 1 — that is not a fixture PASS.

---

## Out of scope

- Making live a required CI job
- Claude Code
- Tags

## Execution

SDD Tasks 1–2. Controller runs Task 3 live walk. PR; merge when GitHub CI green.
