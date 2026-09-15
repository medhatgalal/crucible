# 1.10b Executed Send-back Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Kernel executes battery `## Send-back` from CONTRACT (card + cap + andon), so swapping a judging battery does not require editing `wm.sh`.

**Architecture:** Keep POSIX `wm.sh`. Add a TSV under `## Send-back` (`word`, `card`, `cap`, `andon`). `sendback_lookup` resolves the judging battery via ROUTING (ATTACK-MAP → critique for map words; BRICK → review for brick words) and `skill_dir` (cwd overlay, then ENGINE). `cmd_next` and `cmd_loop` read cap/andon/card from that row instead of hardcoding `ge 2` / `NEXT MAP` / `ESCALATE REVIEW_FAIL`. Missing row is refuse, not a silent fallback.

**Tech Stack:** POSIX `sh`, fixture verify scripts. No harness CLIs.

**Spec:** Factory 1.10b. Worktree `/tmp/crucible-1.10b` branch `feat/send-back-kernel` from `origin/main` `c441b83` (1.10.0). Owner standing rule: merge when CI green; clean up worktree after.

## Global Constraints

- POSIX `sh` only.
- Maker ≠ judge: distinct agent ids; distinct kinds when ≥2 CLIs exist.
- Never stamp CROSS-FAMILY when kinds match. MAP-HUMAN still required for HIGH/live.
- No Claude Code. kiro one-shot is `kiro-cli chat --no-interactive`.
- `wm.sh` must not contain literal `skills/architecture`, `skills/critique`, `skills/review`, or `skills/loop-design` (11c). Batteries from ROUTING column 3 via `$bat`.
- Skills in-repo / ENGINE sibling, never `$HOME`.
- TDD: fail first.
- Work only in `/tmp/crucible-1.10b`. Do not implement on main.
- Preserve 1.10.0 isolation (session, packs, owned-path wall, HIGH one-kind).
- Default review CONTRACT remains cap `2` andon `ESCALATE REVIEW_FAIL` (existing two-FAIL tests stay green).
- Default critique CONTRACT remains card `NEXT MAP` for `MAP-REVISE` (existing MAP-REVISE tests stay green).
- First line of `.wm/CLOSED` stays `CLOSED PASS` / `CLOSED NO-BUILD`.
- Sequential verify (do not parallel map+kernel: leftover `wm loop` pgrep false-fails).

## File map

- Modify: `skills/critique/CONTRACT.md`, `skills/review/CONTRACT.md` (and architecture/research if they already have ## Send-back) — TSV after prose.
- Modify: `wm.sh` — `sendback_battery_for_word`, `sendback_lookup`; `cmd_next`; `cmd_loop` FAIL path.
- Modify: `scripts/verify-working-mode.sh` — overlay cap=1 FAIL escalates without retry.
- Modify: `scripts/verify-working-mode-map.sh` — overlay MAP-REVISE card; require TSV header.
- Modify: `.github/workflows/selftest.yml` — run `verify-working-mode-map.sh` (closes 1.10a CI gap).
- Modify: `VERSION` 1.10.1, `CHANGELOG.md`, `docs/whats-new.md`, `WORKING-MODE.md` if needed.

---

### Task 1: Parse and execute Send-back from CONTRACT

**Files:**
- Modify: `skills/critique/CONTRACT.md`, `skills/review/CONTRACT.md`
- Modify: `skills/architecture/CONTRACT.md`, `skills/research/CONTRACT.md` (TSV or explicit none-row)
- Modify: `wm.sh`
- Modify: `scripts/verify-working-mode.sh` (FAIL overlay)
- Modify: `scripts/verify-working-mode-map.sh` (MAP-REVISE overlay + TSV fgrep)

**Interfaces:**
- Consumes: `routing_file`, `skill_dir`, `review_fail_count`, `green_reviewer_fail`, `map_word_recorded`
- Produces:
  - `sendback_battery_for_word WORD` prints ROUTING column 3 for ATTACK-MAP (map words) or BRICK (brick words).
  - `sendback_lookup BATTERY WORD` prints `card<TAB>cap<TAB>andon` or returns 1.
  - Default review row: `FAIL	NEXT RUN maker-build	2	ESCALATE REVIEW_FAIL`
  - Default critique row: `MAP-REVISE	NEXT MAP	1	-`

CONTRACT TSV (after existing prose, still under `## Send-back`):

```
word	card	cap	andon
MAP-REVISE	NEXT MAP	1	-
```

```
word	card	cap	andon
FAIL	NEXT RUN maker-build	2	ESCALATE REVIEW_FAIL
```

- [ ] **Step 1: Write failing tests**

In `scripts/verify-working-mode-map.sh`, after existing Send-back heading checks (or near critique CONTRACT fgreps), require the TSV header on critique and review:

```sh
require_fgrep "$HERE/skills/critique/CONTRACT.md" 'word	card	cap	andon' \
  'critique CONTRACT Send-back must include word/card/cap/andon TSV'
require_fgrep "$HERE/skills/review/CONTRACT.md" 'word	card	cap	andon' \
  'review CONTRACT Send-back must include word/card/cap/andon TSV'
```

Overlay: MAP-REVISE card comes from cwd `.crucible/skills/critique` not hardcoded NEXT MAP.

Place after existing `t-cadence-revise` / MAP-REVISE next test (the one that expects NEXT MAP). New fixture:

```sh
setup_map_repo t-sendback-revise-overlay
write_architecture_fixture alice LOW no
impl_ok 'record-mapper sendback overlay' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready sendback overlay' "$WM" map-ready || true
write_map_return bob MAP-REVISE
impl_ok 'map-verdict sendback overlay' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
mkdir -p .crucible/skills/critique
cp "$HERE/skills/critique/SKILL.md" .crucible/skills/critique/SKILL.md
cp "$HERE/skills/critique/CONTRACT.md" .crucible/skills/critique/CONTRACT.md
# Rewrite only the TSV data row; keep header.
awk 'BEGIN{FS=OFS="\t"}
  $1=="MAP-REVISE" { $2="STOP-ASK"; print; next }
  { print }
' .crucible/skills/critique/CONTRACT.md > .crucible/skills/critique/CONTRACT.md.tmp
mv .crucible/skills/critique/CONTRACT.md.tmp .crucible/skills/critique/CONTRACT.md
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q '^STOP-ASK' \
    && ok || bad "overlay critique Send-back MAP-REVISE must emit STOP-ASK, got $card"
  printf '%s\n' "$card" | grep -q 'NEXT MAP' \
    && bad "overlay must not keep hardcoded NEXT MAP, got $card" || ok
else
  # STOP-ASK may be printed then exit 0 from next; if next dies, still ok if err matches
  err=$(cat "$ERR")
  printf '%s\n' "$err" | grep -q 'STOP-ASK' \
    && ok || bad "overlay MAP-REVISE next refused without STOP-ASK: $err"
fi
```

Note: `cmd_next` currently `say`s and returns 0 for STOP-ASK MAP-HUMAN etc. STOP-ASK from Send-back should `say` and return 0 like other cards so `next` is inspectable.

In `scripts/verify-working-mode.sh`, after `t-e4-two-fail-escalate` (keep that test; default cap stays 2), add overlay cap=1. **Cap is enforced in `cmd_loop` when consuming `NEXT RUN maker-build`, not in `cmd_next`.** `next` must not increment `review-fail-count` (existing E4). With cap=1, first FAIL still cards `NEXT RUN maker-build`; loop increments 0→1, `1 >= 1`, halts andon without running maker-build.

```sh
# Send-back overlay: review CONTRACT cap=1 → loop ESCALATE on first FAIL consume.
setup_repo t-sendback-fail-cap1
write_loop_maker_pass
write_loop_reviewer_fail
"$WM" cast maker alice grok './tools/loop-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer bob grok './tools/loop-reviewer-fail.sh {BRIEF}' >/dev/null
mkdir -p .crucible/skills/review
cp "$HERE/skills/review/SKILL.md" .crucible/skills/review/SKILL.md
cp "$HERE/skills/review/CONTRACT.md" .crucible/skills/review/CONTRACT.md
awk 'BEGIN{FS=OFS="\t"}
  $1=="FAIL" { $3=1; print; next }
  { print }
' .crucible/skills/review/CONTRACT.md > .crucible/skills/review/CONTRACT.md.tmp
mv .crucible/skills/review/CONTRACT.md.tmp .crucible/skills/review/CONTRACT.md
commit_msg 'sendback cap1 overlay'
run_wm_loop
assert_loop_foreground 't-sendback-fail-cap1'
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'ESCALATE REVIEW_FAIL' \
  && ok || bad "cap=1 overlay wanted ESCALATE REVIEW_FAIL, got out=$(cat "$OUT") err=$(cat "$ERR")"
[ "$LOOP_RC" -ne 0 ] && ok || bad 'cap=1 overlay loop must not exit 0'
rfc=$(cat .wm/review-fail-count 2>/dev/null || echo ABSENT)
[ "$rfc" = 1 ] && ok || bad "cap=1 overlay review-fail-count wanted 1, got $rfc"
```

`write_loop_maker_pass` / `write_loop_reviewer_fail` / `run_wm_loop` already exist. Reuse them. Default two-FAIL test still wants count 2.

Also assert `wm.sh` has `sendback_lookup` and does not write a literal `skills/critique` or `skills/review` path (11c already greps those).

- [ ] **Step 2: Run tests — expect RED**

```sh
cd /tmp/crucible-1.10b
sh scripts/verify-working-mode-map.sh
sh scripts/verify-working-mode.sh
```

Expected: missing TSV header; overlay MAP-REVISE still NEXT MAP; overlay cap=1 still NEXT RUN maker-build. Not syntax errors.

- [ ] **Step 3: Implement**

CONTRACT files: keep prose, then the TSV header + one data row as specified.

`wm.sh` (near `skill_dir`):

```sh
sendback_battery_for_word() {
  _sbw=$1
  _sbw_rf=$(routing_file) || return 1
  _sbw_ph=
  case $_sbw in
    MAP-ACCEPT|MAP-REVISE|MAP-STOP-ASK) _sbw_ph=ATTACK-MAP ;;
    PASS|FAIL|BLOCKED|NO-BUILD) _sbw_ph=BRICK ;;
    *) return 1 ;;
  esac
  awk -F '\t' -v p="$_sbw_ph" 'NR > 1 && $1 == p { print $3; exit }' "$_sbw_rf"
}

sendback_lookup() {
  _sbl_bat=$1
  _sbl_word=$2
  [ -n "$_sbl_bat" ] && [ -n "$_sbl_word" ] || return 1
  _sbl_dir=$(skill_dir "$_sbl_bat") || return 1
  _sbl_c="${_sbl_dir}/CONTRACT.md"
  [ -f "$_sbl_c" ] || return 1
  awk -v word="$_sbl_word" '
    $0 == "## Send-back" { p=1; next }
    p && /^## / { exit }
    p && $0 == "word\tcard\tcap\tandon" { hdr=1; next }
    hdr && NF {
      split($0, a, "\t")
      if (a[1] == word) {
        print a[2] "\t" a[3] "\t" a[4]
        exit
      }
    }
  ' "$_sbl_c"
}
```

`cmd_next` MAP-REVISE branch: replace hardcoded `say "NEXT MAP"` with lookup:

```sh
if [ "$_nx_mw" = MAP-REVISE ]; then
  _nx_bat=$(sendback_battery_for_word MAP-REVISE) || die "Send-back battery missing for MAP-REVISE"
  _nx_row=$(sendback_lookup "$_nx_bat" MAP-REVISE) || die "Send-back missing MAP-REVISE"
  _nx_card=$(printf '%s\n' "$_nx_row" | awk -F '\t' '{ print $1 }')
  [ -n "$_nx_card" ] && [ "$_nx_card" != - ] || die "Send-back MAP-REVISE card empty"
  say "$_nx_card"
  return 0
fi
```

`cmd_next` green_reviewer_fail branch: card from Send-back TSV. **Do not apply cap in `next`** (count is unchanged here). If `review_fail_count` is already `>= cap` (loop wrote it last tick), `say` andon; else `say` card.

```sh
if green_reviewer_fail; then
  _nx_bat=$(sendback_battery_for_word FAIL) || die "Send-back battery missing for FAIL"
  _nx_row=$(sendback_lookup "$_nx_bat" FAIL) || die "Send-back missing FAIL"
  _nx_card=$(printf '%s\n' "$_nx_row" | awk -F '\t' '{ print $1 }')
  _nx_cap=$(printf '%s\n' "$_nx_row" | awk -F '\t' '{ print $2 }')
  _nx_andon=$(printf '%s\n' "$_nx_row" | awk -F '\t' '{ print $3 }')
  case $_nx_cap in ''|*[!0-9]*) die "Send-back FAIL cap must be a number" ;; esac
  _nx_rfc=$(review_fail_count)
  if [ "$_nx_rfc" -ge "$_nx_cap" ]; then
    [ -n "$_nx_andon" ] && [ "$_nx_andon" != - ] || die "Send-back FAIL andon empty"
    say "$_nx_andon"
    return 0
  fi
  [ -n "$_nx_card" ] || die "Send-back FAIL card empty"
  say "$_nx_card"
  return 0
fi
```

With default cap 2: first FAIL `rfc=0` → card `NEXT RUN maker-build` (existing E4). After loop writes 2, next is andon.

`cmd_loop` `"NEXT RUN maker-build"` FAIL block: increment count as today, lookup FAIL cap/andon, halt when `_lp_rfc >= cap`. Do not hardcode 2 or `ESCALATE REVIEW_FAIL`. Cap=1: first consume increments to 1 and halts before `run maker-build`.

Do not hardcode battery directory literals.

- [ ] **Step 4: Re-run map then kernel sequentially. Existing MAP-REVISE NEXT MAP, two-FAIL ESCALATE, FAIL-then-PASS CLOSED stay green. Overlay tests green.**

```sh
sh scripts/verify-working-mode-map.sh
sh scripts/verify-working-mode.sh
sh scripts/verify-working-mode-go.sh
```

- [ ] **Step 5: Commit** `feat(wm): execute CONTRACT Send-back card/cap/andon`

---

### Task 2: CI map suite + 1.10.1 docs

**Files:**
- Modify: `.github/workflows/selftest.yml` (after working-mode go CHECKs, add map CHECKs)
- Modify: `VERSION` → `1.10.1`
- Modify: `CHANGELOG.md`, `docs/whats-new.md`
- Modify: `scripts/verify-working-mode-blank-home.sh` if it pins `1.10.0`

This closes the 1.10a deviation: GitHub Actions ran kernel+go but not `verify-working-mode-map.sh` (the HIGH one-kind / pack / wall suite).

- [ ] **Step 1:** Grep `1.10.0` in verify scripts; update pins that would fail. Add changelog:

```
## [1.10.1] - 2026-09-15

### Send-back kernel
- `## Send-back` is a TSV (`word`, `card`, `cap`, `andon`) the kernel executes.
- Overlay a judging battery under `.crucible/skills/<bat>/` to change FAIL cap or MAP-REVISE card without editing `wm.sh`.
- CI runs `scripts/verify-working-mode-map.sh`.
```

- [ ] **Step 2:** Sequential full verify:

```sh
sh -n wm.sh && sh -n wm-go.sh
sh scripts/verify-working-mode-map.sh
sh scripts/verify-working-mode.sh
sh scripts/verify-working-mode-go.sh
```

- [ ] **Step 3: Commit** `chore: working-mode 1.10.1 send-back kernel`

---

## Deviations already incurred (1.10a) and how 1.10b gets back

| Deviation / mistake | Workaround used | Get back |
| --- | --- | --- |
| Task 3 packs only on package `wm.sh`; `.wm/bin/wm` had no ROUTING sibling | ENGINE lookup added in fix round 1 | Keep ENGINE lookup; 1.10b Send-back uses same `skill_dir` / `routing_file` |
| Session-restart lost the Task 3 fixer; ledger marked complete early | Re-dispatched fixer | Ledger must stay open until re-review PASSes |
| Docs promised sessions/packs/wall in Task 1 before they existed | Brief-mandated copy; later tasks filled | 1.10b docs ship with the kernel, not ahead |
| CI omitted `verify-working-mode-map.sh` | Local map 381/0 | Task 2 adds the map suite to selftest.yml |
| Parallel map+kernel pgrep leftover-loop false-fail | Sequential re-run | Always sequential in this plan |
| `high_kinds_ok` name still says kinds, checks agents | Left as-is (churn) | Do not rename in 1.10b |
| Codex read-only sandbox skipped (evidence writes) | Kernel owned-path wall | Out of 1.10b |
| Deleted owned files not hashed | Plan snippet `[ -f ]` | Out of 1.10b |

## Out of scope

- Executing `## Andon` as a second table (MAP-STOP-ASK already STOP-ASK).
- RESEARCH/REPO Send-back (None).
- Shape battery (1.11), worktrees (1.12), live walk (1.14).
- Renaming `high_kinds_ok`.

## Execution

User: go ahead until done with validation and evidence. Execute SDD in `/tmp/crucible-1.10b`. After Task 2 green + whole-branch review, push PR and merge when CI green (owner standing rule), then remove this worktree.
