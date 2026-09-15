# 1.10a One-Kind Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** HIGH slices may proceed when only one of grok / kiro-cli / codex is present, if maker and reviewer are distinct agent ids isolated by fresh sessions, per-station skill packs, and a kernel owned-path wall — labelled `SUBAGENT-ISOLATED`, never fake `CROSS-FAMILY`.

**Architecture:** Keep POSIX `wm.sh` as the kernel. Isolation is a CHECK pack, not a second engine: `high_kinds_ok` becomes agent-distinct (kinds may match); `honest_isolation` / FLOOR / CLOSED stamp `CROSS-FAMILY` only when maker.kind ≠ reviewer.kind; `cmd_run` mints a UUID session, expands `{SESSION}`, and embeds the ROUTING battery for that station into the brief; after reviewer/scout exec, owned product paths must be unchanged. Two kinds on PATH still split maker/reviewer (stronger bar). MAP-HUMAN remains required for HIGH/live.

**Tech Stack:** POSIX `sh` (`wm.sh`, `wm-go.sh`), fixture verify scripts (`scripts/verify-working-mode-map.sh`, `scripts/verify-working-mode.sh`, `scripts/verify-working-mode-go.sh`). No harness CLIs in tests. No `$HOME` skills.

**Spec:** User amendment 2026-09-15 superseding ballot 3d STOP-ASK on one-kind HIGH. Worktree `/tmp/crucible-1.10a` branch `feat/one-kind-isolation` from `origin/main` `8614796` (1.9.0). Do not merge. Do not edit Desktop/crucible (stale 1.6.6).

## Global Constraints

- POSIX `sh` only in `wm.sh` / `wm-go.sh` / verify scripts (no bashisms).
- Maker ≠ judge: distinct **agent ids** always; distinct **kinds** when ≥2 of grok/kiro-cli/codex exist.
- Never stamp `CROSS-FAMILY` when maker.kind equals reviewer.kind.
- `next` cards must not print `CROSS-FAMILY` (label lives on FLOOR / CLOSED / invoke log).
- MAP-HUMAN still required for HIGH/live; do not auto-write it.
- No Claude Code on discover. kiro one-shot is `kiro-cli chat --no-interactive`, not `kiro-cli acp`.
- `wm.sh` must not contain the literal paths `skills/architecture`, `skills/critique`, `skills/review`, or `skills/loop-design` (11c swap). Resolve batteries from `ROUTING.tsv` column 3 via a `$bat` variable.
- Skills live in the repo / engine sibling, never `$HOME`.
- TDD: write/change the failing test, watch it fail for the right reason, then implement. Do not implement first.
- Work only in `/tmp/crucible-1.10a`. Do not merge to main. Do not tag.
- First line of `.wm/CLOSED` stays `CLOSED PASS` or `CLOSED NO-BUILD`. Extra `independence:` line is allowed after that.
- Fixture `./tools/*.sh` commands must keep working (session/CLI flag injection only when the expanded command is grok / kiro-cli / codex).

## File map

- Modify: `wm.sh` — `high_kinds_ok`, `honest_isolation`, `write_invoke_log`, `write_brief`, `cmd_run`, `floor_write`, `cmd_close`, `guard_map_before_maker`; add `session_uuid`, `station_battery`, `skill_dir`, `append_station_pack`, owned-path snapshot/compare.
- Modify: `wm-go.sh` — grok `_go_cli_cmd` includes `--session-id {SESSION} --no-subagents`.
- Modify: `scripts/verify-working-mode-map.sh` — invert 3d HIGH one-kind tests; add FLOOR/CLOSED label, session, skill-pack, owned-path wall tests.
- Modify: `scripts/verify-working-mode.sh` — session + brief pack assertions as needed.
- Modify: `scripts/verify-working-mode-go.sh` — grok template contains `{SESSION}`; one-kind panel still spec0≠make0.
- Modify: `WORKING-MODE.md`, `docs/working-mode.md`, `CHANGELOG.md`, `docs/whats-new.md`, `VERSION`, `adapters/{grok,kiro,codex}.md`.

---

### Task 1: HIGH one-kind proceeds + honest independence label

**Files:**
- Modify: `scripts/verify-working-mode-map.sh` (docs fgrep ~947–953, `t-high-one-kind` ~1162–1190, `t-high-two-kind` ~1192–1212, `t-loop-high-one-kind` ~1444–1459)
- Modify: `wm.sh` (`honest_isolation` ~288, `write_invoke_log` ~307, `high_kinds_ok` ~1212–1218, `guard_map_before_maker` ~1456, `cmd_next` ~2016, `floor_write` ~2130, `cmd_close` ~1911–1923)
- Modify: `docs/working-mode.md` (Risk-triggered reviewer section ~296–302)
- Modify: `WORKING-MODE.md` (after “Two CLIs: maker kind is not reviewer kind.”)

**Interfaces:**
- Consumes: existing `panel_kind`, `panel_agent`, `floor_write`, `cmd_close`, map fixtures `write_architecture_fixture`, `cast_brick_panel`, `write_map_human`, `write_spec_fit`, `write_map_loop_brick`
- Produces: `high_kinds_ok` true iff maker and reviewer both have non-dash kind **and** distinct agent ids (kinds may match). `honest_isolation` prints `CROSS-FAMILY` iff maker.kind ≠ reviewer.kind, else `SUBAGENT-ISOLATED`. FLOOR and CLOSED record `independence: <label>`.

- [ ] **Step 1: Write the failing tests**

In `scripts/verify-working-mode-map.sh`, replace the docs fgrep that claims STOP-ASK for HIGH + one kind:

```sh
require_fgrep "$HERE/docs/working-mode.md" 'SUBAGENT-ISOLATED' \
  'docs/working-mode.md must name SUBAGENT-ISOLATED for one-kind HIGH'
require_fgrep "$HERE/docs/working-mode.md" 'STOP-ASK' \
  'docs/working-mode.md must name STOP-ASK'
if [ -f "$HERE/docs/working-mode.md" ] && grep -q 'fake CROSS-FAMILY' "$HERE/docs/working-mode.md"; then
  ok
else
  bad 'docs/working-mode.md must refuse fake CROSS-FAMILY (3d is label+ROUTING)'
fi
```

Replace `t-high-one-kind` so HIGH + same kind + MAP-HUMAN + distinct agents emits NEXT SLICE, never CROSS-FAMILY on the card, FLOOR says SUBAGENT-ISOLATED, and maker-falsify may start:

```sh
# 3d superseded 2026-09-15: HIGH + one kind + distinct agents + MAP-HUMAN proceeds.
# Label SUBAGENT-ISOLATED. Never fake CROSS-FAMILY.
setup_map_repo t-high-one-kind
write_architecture_fixture alice HIGH no
impl_ok 'record-mapper HIGH one-kind' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready HIGH one-kind' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict HIGH one-kind' "$WM" map-verdict .wm/return/bob.md || true
cast_brick_panel carol dave grok grok
write_map_human operator MAP.md
if card=$("$WM" next 2>"$ERR"); then
  printf '%s\n' "$card" | grep -E -q 'NEXT SLICE s1' \
    && ok || bad "HIGH + one kind with MAP-HUMAN next must emit NEXT SLICE, got $card"
  printf '%s\n' "$card" | grep -q 'STOP-ASK' \
    && bad "HIGH + one kind must not STOP-ASK when agents differ (got $card)" || ok
  printf '%s\n' "$card" | grep -q 'CROSS-FAMILY' \
    && bad "HIGH + one kind must not fake CROSS-FAMILY (got $card)" || ok
else
  bad "HIGH + one kind next refused: $(cat "$ERR")"
fi
"$WM" status >/dev/null 2>"$ERR" || true
if [ -f .wm/FLOOR.md ] && grep -q '^independence: SUBAGENT-ISOLATED$' .wm/FLOOR.md; then
  ok
else
  bad "HIGH one-kind FLOOR must say independence: SUBAGENT-ISOLATED, got $(cat .wm/FLOOR.md 2>/dev/null || echo ABSENT)"
fi
if grep -q 'CROSS-FAMILY' .wm/FLOOR.md 2>/dev/null; then
  bad 'HIGH one-kind FLOOR must not say CROSS-FAMILY'
else
  ok
fi
rm -f .wm/FALSIFIER
impl_ok 'HIGH + one kind with MAP-HUMAN can start maker' "$WM" run maker-falsify || true
[ -f .wm/FALSIFIER ] && ok || bad 'HIGH one-kind maker-falsify must write FALSIFIER'
```

On `t-high-two-kind`, after successful `next`, assert FLOOR `independence: CROSS-FAMILY` while the **card** still must not print CROSS-FAMILY:

```sh
"$WM" status >/dev/null 2>"$ERR" || true
if [ -f .wm/FLOOR.md ] && grep -q '^independence: CROSS-FAMILY$' .wm/FLOOR.md; then
  ok
else
  bad "HIGH two-kind FLOOR must say independence: CROSS-FAMILY, got $(cat .wm/FLOOR.md 2>/dev/null || echo ABSENT)"
fi
```

Keep the existing `printf '%s\n' "$card" | grep -q 'CROSS-FAMILY' && bad ...` on the next card.

Replace `t-loop-high-one-kind` with a full HIGH brick walk (same shape as `t-loop-slice-pass`, one slice, HIGH, MAP-HUMAN, same kinds):

```sh
# HIGH + one kind + MAP-HUMAN + brick workers → CLOSED PASS, SUBAGENT-ISOLATED.
setup_map_repo t-loop-high-one-kind
write_architecture_fixture alice HIGH no
impl_ok 'record-mapper loop HIGH one-kind' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready loop HIGH one-kind' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict loop HIGH one-kind' "$WM" map-verdict .wm/return/bob.md || true
write_map_human operator MAP.md
write_spec_fit
write_map_loop_brick
"$WM" cast maker carol grok './tools/map-loop-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer dave grok './tools/map-loop-reviewer.sh {BRIEF}' >/dev/null
git add -A
git commit -qm 'HIGH one-kind loop workers' >/dev/null
run_map_loop
assert_map_loop_foreground 't-loop-high-one-kind'
if grep -q 'CLOSED PASS' "$OUT" && [ -f .wm/CLOSED ] && grep -q 'CLOSED PASS' .wm/CLOSED; then
  ok
else
  bad "HIGH one-kind loop wanted CLOSED PASS, got out=$(cat "$OUT") closed=$(cat .wm/CLOSED 2>/dev/null || echo ABSENT) err=$(cat "$ERR")"
fi
[ "$LOOP_RC" -eq 0 ] && ok || bad "HIGH one-kind loop exit $LOOP_RC err=$(cat "$ERR")"
printf '%s\n%s\n' "$(cat "$OUT")" "$(cat "$ERR")" | grep -q 'CROSS-FAMILY' \
  && bad 'HIGH one-kind loop must not fake CROSS-FAMILY' || ok
if grep -q '^independence: SUBAGENT-ISOLATED$' .wm/CLOSED \
  && grep -q '^independence: SUBAGENT-ISOLATED$' .wm/FLOOR.md; then
  ok
else
  bad "HIGH one-kind CLOSED/FLOOR must record SUBAGENT-ISOLATED, closed=$(cat .wm/CLOSED) floor=$(cat .wm/FLOOR.md 2>/dev/null || echo ABSENT)"
fi
[ -f .wm/reviewer-ran ] && ok || bad 'HIGH one-kind loop did not exec the reviewer CLI'
```

Do not weaken `t-loop-map-human` (HIGH unsigned still STOP-ASK MAP-HUMAN). Do not weaken maker≠reviewer agent-id cast refusals.

- [ ] **Step 2: Run tests to verify they fail**

Run from `/tmp/crucible-1.10a`:

```sh
sh scripts/verify-working-mode-map.sh
```

Expected RED (not syntax error): HIGH + one kind still STOP-ASK; FLOOR missing `independence:`; docs missing `SUBAGENT-ISOLATED`. Do not implement until this RED is observed.

- [ ] **Step 3: Minimal kernel + docs**

Replace `honest_isolation` and use it from invoke log + FLOOR + CLOSED:

```sh
honest_isolation() {
  _hi_mk=$(panel_kind maker)
  _hi_rk=$(panel_kind reviewer)
  if [ -n "$_hi_mk" ] && [ "$_hi_mk" != - ] \
    && [ -n "$_hi_rk" ] && [ "$_hi_rk" != - ] \
    && [ "$_hi_mk" != "$_hi_rk" ]; then
    printf 'CROSS-FAMILY\n'
    return 0
  fi
  printf 'SUBAGENT-ISOLATED\n'
}
```

`write_invoke_log` must call `honest_isolation` instead of hardcoding `SUBAGENT-ISOLATED`.

`high_kinds_ok`:

```sh
high_kinds_ok() {
  _hk_mk=$(panel_kind maker)
  _hk_rk=$(panel_kind reviewer)
  _hk_ma=$(panel_agent maker)
  _hk_ra=$(panel_agent reviewer)
  [ -n "$_hk_mk" ] && [ "$_hk_mk" != - ] || return 1
  [ -n "$_hk_rk" ] && [ "$_hk_rk" != - ] || return 1
  [ -n "$_hk_ma" ] && [ "$_hk_ma" != - ] || return 1
  [ -n "$_hk_ra" ] && [ "$_hk_ra" != - ] || return 1
  [ "$_hk_ma" != "$_hk_ra" ]
}
```

`guard_map_before_maker` die string: `STOP-ASK: HIGH requires distinct maker and reviewer agents` (not “kinds”).

`floor_write`: after `andon:` print `independence: $(honest_isolation)`.

`cmd_close`: when writing CLOSED PASS / CLOSED NO-BUILD, first line remains the CLOSED word; second line `independence: <honest_isolation>`.

`docs/working-mode.md` section **Risk-triggered reviewer (3d)** becomes:

```
3d is **label + ROUTING**, not a second engine. HIGH slices require
distinct maker and reviewer **agent ids**. When two harnesses are
cast, maker `kind` ≠ reviewer `kind` and FLOOR/CLOSED record
`independence: CROSS-FAMILY`. If only one harness is present, HIGH
still proceeds after `MAP-HUMAN` with isolated sessions and station
packs; FLOOR/CLOSED record `independence: SUBAGENT-ISOLATED`. Never
fake CROSS-FAMILY. LOW slices may use the same kind.
```

`WORKING-MODE.md` after the two-CLI sentence:

```
One CLI: HIGH still runs after MAP-HUMAN. Isolation is
SUBAGENT-ISOLATED (fresh session, station pack, owned-path wall).
Never labelled CROSS-FAMILY.
```

- [ ] **Step 4: Re-run map suite**

```sh
sh scripts/verify-working-mode-map.sh
```

Expected: 0 failed. Also run `sh scripts/verify-working-mode.sh` and `sh scripts/verify-working-mode-go.sh` to catch CLOSED first-line regressions.

- [ ] **Step 5: Commit**

```bash
git add scripts/verify-working-mode-map.sh wm.sh docs/working-mode.md WORKING-MODE.md
git commit -m "$(cat <<'EOF'
feat(wm): HIGH one-kind proceeds as SUBAGENT-ISOLATED

Supersede 3d STOP-ASK: HIGH with one CLI product runs when maker and
reviewer agent ids differ. Stamp CROSS-FAMILY only when kinds differ.
MAP-HUMAN unchanged.
EOF
)"
```

---

### Task 2: Fresh session id per `wm run`

**Files:**
- Modify: `wm.sh` (`session_uuid`, `write_brief`, `cmd_run` `{SESSION}` expand, `write_invoke_log`)
- Modify: `wm-go.sh` (`_go_cli_cmd` grok line)
- Modify: `scripts/verify-working-mode-map.sh` (session tests)
- Modify: `scripts/verify-working-mode-go.sh` (grok command contains `{SESSION}`)
- Modify: `adapters/grok.md` (document `--session-id`)

**Interfaces:**
- Consumes: Task 1 `write_brief` / `cmd_run` / `write_invoke_log`
- Produces: `session_uuid` prints a lowercase UUID. `write_brief ROLE AGENT [SESSION]` writes `session: <id>`. `cmd_run` expands `{SESSION}` the same way as `{BRIEF}` (quoted). Grok discover command is `grok --session-id {SESSION} --no-subagents -p --prompt-file {BRIEF}`. Fixture `./tools/*.sh` without `{SESSION}` still run.

- [ ] **Step 1: Write the failing tests**

Append to `scripts/verify-working-mode-map.sh` before the home-leak check:

```sh
# Session: each wm run mints a distinct session line in the brief.
setup_map_repo t-session-ids
write_architecture_fixture alice LOW no
impl_ok 'record-mapper session' "$WM" record-mapper --from MAP.md || true
"$WM" cast specifier spec0 grok 'true' >/dev/null
"$WM" run specifier >/dev/null 2>"$ERR" || true
b1=$(ls .wm/briefs/specifier.* 2>/dev/null | head -1)
s1=
[ -n "$b1" ] && s1=$(awk -F ': ' '$1=="session"{print $2; exit}' "$b1")
"$WM" run specifier >/dev/null 2>"$ERR" || true
b2=$(ls -t .wm/briefs/specifier.* 2>/dev/null | head -1)
s2=
[ -n "$b2" ] && s2=$(awk -F ': ' '$1=="session"{print $2; exit}' "$b2")
if [ -n "$s1" ] && [ -n "$s2" ] && [ "$s1" != "$s2" ]; then
  ok
else
  bad "wm run must mint distinct session ids, got s1=$s1 s2=$s2"
fi
printf '%s\n' "$s1" | grep -E -q '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' \
  && ok || bad "session id must be a UUID, got $s1"
```

In `scripts/verify-working-mode-go.sh`, after POSIX parse of `wm-go.sh`, add:

```sh
if grep -q '{SESSION}' "$HERE/wm-go.sh" \
  && grep -q -- '--session-id' "$HERE/wm-go.sh"; then
  ok
else
  bad 'wm-go.sh grok command must pass --session-id {SESSION}'
fi
```

- [ ] **Step 2: Run the new tests — expect RED** (missing `session:` in brief; no `{SESSION}` in wm-go.sh)

```sh
sh scripts/verify-working-mode-map.sh
sh scripts/verify-working-mode-go.sh
```

- [ ] **Step 3: Implement**

```sh
session_uuid() {
  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen | tr 'A-Z' 'a-z'
    return 0
  fi
  od -An -N16 -tx1 /dev/urandom 2>/dev/null | tr -d ' \n' | awk '{
    if (length($0) < 32) exit 1
    printf "%s-%s-4%s-a%s-%s\n", substr($0,1,8), substr($0,9,4), substr($0,14,3), substr($0,18,3), substr($0,21,12)
  }'
}
```

`write_brief`: third arg `_wb_session`; print `session: %s` after `agent:` when non-empty.

`cmd_run`: `_ru_session=$(session_uuid)` before `write_brief`; pass it; export `WM_SESSION`; awk-expand `{SESSION}` like `{BRIEF}` (quoted). Write `session:` into invoke log.

`_go_cli_cmd` grok:

```sh
grok) printf '%s\n' 'grok --session-id {SESSION} --no-subagents -p --prompt-file {BRIEF}' ;;
```

kiro/codex stay one-shot (no `--resume`). Do not inject grok flags into `./tools/*.sh`.

- [ ] **Step 4: Re-run map + go + kernel suites. Expected 0 failed.**

- [ ] **Step 5: Commit**

```bash
git add wm.sh wm-go.sh scripts/verify-working-mode-map.sh scripts/verify-working-mode-go.sh adapters/grok.md
git commit -m "feat(wm): mint a fresh UUID session per wm run"
```

---

### Task 3: Per-station skill packs in briefs

**Files:**
- Modify: `wm.sh` (`station_battery`, `skill_dir`, `append_station_pack` called from `write_brief`)
- Modify: `scripts/verify-working-mode-map.sh`

**Interfaces:**
- Consumes: `routing_file`, `prebrick_next_card`, `write_brief`, `.wm/ENGINE` / `WM_ENGINE`
- Produces: specifier SPEC/MAP brief contains architecture SKILL (`RULE 26`) and not review (`You verify; you do not improve`). scout brief contains critique (`MAP-ACCEPT|MAP-REVISE`). reviewer brief contains review (`re-run`) and not `You are the mapper`. maker brief contains neither architecture mapper job nor review judge job. Batteries resolved from ROUTING column 3; no literal `skills/architecture` etc. in `wm.sh`.

- [ ] **Step 1: Failing tests** in `scripts/verify-working-mode-map.sh`:

```sh
setup_map_repo t-pack-specifier
write_architecture_fixture alice LOW no
"$WM" cast specifier spec0 grok 'true' >/dev/null
"$WM" run specifier >/dev/null 2>"$ERR" || true
sb=$(ls -t .wm/briefs/specifier.* 2>/dev/null | head -1)
[ -n "$sb" ] && grep -q 'RULE 26' "$sb" \
  && ok || bad "specifier brief must embed architecture SKILL (RULE 26), brief=$(cat $sb 2>/dev/null || echo ABSENT)"
[ -n "$sb" ] && grep -q 'You verify; you do not improve' "$sb" \
  && bad 'specifier brief must not embed review SKILL' || ok

setup_map_repo t-pack-scout
write_architecture_fixture alice LOW no
impl_ok 'record-mapper pack scout' "$WM" record-mapper --from MAP.md || true
mkdir -p tools
printf '#!/bin/sh\nset -eu\nagent=bob\nmkdir -p .wm/return\nprintf "WORD: MAP-ACCEPT\\nAGENT: %s\\nMAP: MAP.md\\n" "$agent" > .wm/return/bob.md\n' > tools/scout-ok.sh
chmod +x tools/scout-ok.sh
"$WM" cast scout bob grok './tools/scout-ok.sh {BRIEF}' >/dev/null
"$WM" run scout >/dev/null 2>"$ERR" || true
scb=$(ls -t .wm/briefs/scout.* 2>/dev/null | head -1)
[ -n "$scb" ] && grep -q 'MAP-ACCEPT' "$scb" && grep -qi invert "$scb" \
  && ok || bad "scout brief must embed critique SKILL, brief=$(cat $scb 2>/dev/null || echo ABSENT)"
[ -n "$scb" ] && grep -q 'You are the mapper' "$scb" \
  && bad 'scout brief must not embed architecture mapper job' || ok

setup_map_repo t-pack-reviewer
write_architecture_fixture alice LOW no
impl_ok 'record-mapper pack rev' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready pack rev' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict pack rev' "$WM" map-verdict .wm/return/bob.md || true
write_spec_fit
write_map_loop_brick
"$WM" cast maker carol grok './tools/map-loop-maker.sh {BRIEF}' >/dev/null
"$WM" cast reviewer dave grok './tools/map-loop-reviewer.sh {BRIEF}' >/dev/null
git add -A && git commit -qm pack-rev >/dev/null
"$WM" run maker-falsify >/dev/null 2>"$ERR" || true
# red/build path may not finish; still write a reviewer brief via a direct run if FALSIFIER exists
if [ -f .wm/FALSIFIER ]; then
  "$WM" run reviewer >/dev/null 2>"$ERR" || true
fi
rb=$(ls -t .wm/briefs/reviewer.* 2>/dev/null | head -1)
if [ -n "$rb" ] && grep -q 're-run' "$rb" && grep -q 'You verify; you do not improve' "$rb"; then
  ok
else
  bad "reviewer brief must embed review SKILL, brief=$(cat $rb 2>/dev/null || echo ABSENT)"
fi
[ -n "$rb" ] && grep -q 'You are the mapper' "$rb" \
  && bad 'reviewer brief must not embed architecture mapper job' || ok

# 11c still holds
if grep -E -q 'skills/(architecture|critique|review|loop-design)' "$WM"; then
  bad 'wm.sh hardcodes battery paths; replacing a directory would require editing wm.sh'
else
  ok
fi
```

Keep the existing 11c check; the new block is extra behavioral proof. Prefer a smaller reviewer-pack fixture: cast reviewer with `sh -c 'mkdir -p .wm/return reviews; printf "WORD: FAIL\nEVIDENCE: x\n" > .wm/return/dave.md'` after a maker-falsify so `cmd_run reviewer` writes the brief even if verdict later refuses. The brief is written **before** exec, so a command `false` still leaves `.wm/briefs/reviewer.*`. Use:

```sh
"$WM" cast reviewer dave grok 'false'
"$WM" run reviewer >/dev/null 2>"$ERR" || true
rb=$(ls -t .wm/briefs/reviewer.* | head -1)
```

`cmd_run reviewer` requires distinct maker, a CLI command, and (after exec) a WORD file — but `write_brief` happens before `sh -c`. `false` still produces the brief. Guard: if `cmd_run` dies before write_brief because map guard only applies to maker, reviewer should reach write_brief.

- [ ] **Step 2: Run map suite — expect RED** (briefs lack RULE 26 / invert / re-run)

- [ ] **Step 3: Implement without hardcoded battery paths**

```sh
station_battery() {
  _stb_role=$1
  _stb_rf=$(routing_file) || return 0
  _stb_ph=
  case $_stb_role in
    specifier)
      case $(prebrick_next_card) in
        "NEXT RESEARCH") _stb_ph=RESEARCH ;;
        "NEXT REPO") _stb_ph=REPO ;;
        *) _stb_ph=MAP ;;
      esac
      ;;
    scout) _stb_ph=ATTACK-MAP ;;
    reviewer) _stb_ph=BRICK ;;
    *) return 0 ;;
  esac
  awk -F '\t' -v p="$_stb_ph" 'NR > 1 && $1 == p { print $3; exit }' "$_stb_rf"
}

skill_dir() {
  _sd_bat=$1
  [ -n "$_sd_bat" ] && [ "$_sd_bat" != - ] || return 1
  if [ -d ".crucible/skills/${_sd_bat}" ]; then
    printf '%s\n' ".crucible/skills/${_sd_bat}"
    return 0
  fi
  if [ -d "skills/${_sd_bat}" ]; then
    printf '%s\n' "skills/${_sd_bat}"
    return 0
  fi
  _sd_src=
  if [ -f "$WM/ENGINE" ]; then
    _sd_src=$(kv_get "$WM/ENGINE" engine)
  fi
  [ -n "$_sd_src" ] || _sd_src=${WM_ENGINE:-$0}
  _sd_root=$(CDPATH= cd "$(dirname "$_sd_src")" && pwd)
  if [ -d "${_sd_root}/skills/${_sd_bat}" ]; then
    printf '%s\n' "${_sd_root}/skills/${_sd_bat}"
    return 0
  fi
  return 1
}

append_station_pack() {
  _ap_role=$1
  _ap_bat=$(station_battery "$_ap_role") || return 0
  [ -n "$_ap_bat" ] || return 0
  _ap_dir=$(skill_dir "$_ap_bat") || return 0
  printf '\n## Station pack (%s)\n' "$_ap_bat"
  [ -f "$_ap_dir/SKILL.md" ] && cat "$_ap_dir/SKILL.md"
  printf '\n'
  [ -f "$_ap_dir/CONTRACT.md" ] && cat "$_ap_dir/CONTRACT.md"
}
```

Call `append_station_pack` from `write_brief` using the **panel** role (`specifier` / `scout` / `reviewer`), not `maker-falsify`. Maker gets no pack.

`routing_file` in a fixture repo: cwd has no ROUTING.tsv — fall back to engine sibling (already implemented). Copied `.wm/bin/wm` has ENGINE pointing at package `wm.sh`, so `dirname` is the package root. Good.

- [ ] **Step 4: Re-run map + kernel. Confirm 11c still ok.**

- [ ] **Step 5: Commit** `feat(wm): embed ROUTING station pack in each brief`

---

### Task 4: Reviewer/scout owned-path wall

**Files:**
- Modify: `wm.sh` (`cmd_run` reviewer/scout)
- Modify: `scripts/verify-working-mode-map.sh`
- Modify: `skills/review/SKILL.md` Must-not (one line: do not write owned product paths)

**Interfaces:**
- Consumes: `owned_paths`, `in_flight_owned_paths`, `path_in_list`, `file_sha256`
- Produces: after `wm run reviewer` or `scout`, if an owned product path’s content hash changed during the child, `die "reviewer wrote owned paths"` (scout: `scout wrote owned paths`). Meta `.wm/**` and `reviews/**` may change.

- [ ] **Step 1: Failing test**

```sh
setup_map_repo t-rev-owned-wall
write_architecture_fixture alice LOW no
impl_ok 'record-mapper wall' "$WM" record-mapper --from MAP.md || true
impl_ok 'map-ready wall' "$WM" map-ready || true
write_map_return bob MAP-ACCEPT
impl_ok 'map-verdict wall' "$WM" map-verdict .wm/return/bob.md || true
write_spec_fit
mkdir -p tools
cat > tools/reviewer-mutates.sh <<'EOF'
#!/bin/sh
set -eu
printf '\nMUTATED\n' >> src/widget/api.py
mkdir -p .wm/return reviews
printf '## Code\nx\n## Testing\ny\n' > reviews/review.md
printf 'WORD: FAIL\nEVIDENCE: none\n' > .wm/return/dave.md
EOF
chmod +x tools/reviewer-mutates.sh
"$WM" cast maker carol grok 'true' >/dev/null
"$WM" cast reviewer dave grok './tools/reviewer-mutates.sh {BRIEF}' >/dev/null
git add -A && git commit -qm wall >/dev/null
# Need a last-maker-run? reviewer may run without FALSIFIER for this CHECK — cmd_run reviewer does not require FALSIFIER.
refuses 'reviewer that writes owned path is refused' 'owned path' \
  "$WM" run reviewer
grep -q MUTATED src/widget/api.py && ok || bad 'fixture must have attempted the write (CHECK is after exec)'
```

If `cmd_run reviewer` currently accepts this (no CHECK), the new `refuses` is RED. The file may stay mutated (CHECK is fail-closed, not a rollback). That is acceptable; do not add rollback.

- [ ] **Step 2: Watch RED** — reviewer currently accepted.

- [ ] **Step 3: Implement**

Before `sh -c` for reviewer/scout, snapshot hashes of existing owned paths (SPEC owned + in-flight slice). After child returns (any rc), if any of those paths changed hash or a newly dirty owned path appears, `die`. Do not treat `.wm/`, `reviews/`, `reviews/review.md`, `.wm/return/` as owned product.

Reuse `owned_paths` + `in_flight_owned_paths`. Skip paths matching `.wm/*` or `reviews/*`.

```sh
snapshot_owned_product() {
  _so_out=$1
  : > "$_so_out"
  _so_list=$(printf '%s\n%s\n' "$(owned_paths)" "$(in_flight_owned_paths)")
  while IFS= read -r _so_p || [ -n "$_so_p" ]; do
    [ -n "$_so_p" ] || continue
    case $_so_p in .wm|.wm/*|reviews|reviews/*) continue ;; esac
    [ -f "$_so_p" ] || continue
    printf '%s %s\n' "$_so_p" "$(file_sha256 "$_so_p")" >> "$_so_out"
  done <<EOF
$_so_list
EOF
}

owned_product_changed() {
  _oc_snap=$1
  _oc_list=$(printf '%s\n%s\n' "$(owned_paths)" "$(in_flight_owned_paths)")
  while IFS= read -r _oc_p || [ -n "$_oc_p" ]; do
    [ -n "$_oc_p" ] || continue
    case $_oc_p in .wm|.wm/*|reviews|reviews/*) continue ;; esac
    [ -f "$_oc_p" ] || continue
    _oc_new=$(file_sha256 "$_oc_p")
    _oc_old=
    [ -f "$_oc_snap" ] && _oc_old=$(awk -v p="$_oc_p" '$1==p { print $2; exit }' "$_oc_snap")
    if [ -z "$_oc_old" ] || [ "$_oc_old" != "$_oc_new" ]; then
      return 0
    fi
  done <<EOF
$_oc_list
EOF
  return 1
}
```

Call only for reviewer and scout. Honest LOW reviewer fixtures (`map-loop-reviewer.sh`) do not touch `src/widget/api.py` — they must still PASS.

Do **not** add Codex `--sandbox read-only`: `wm evidence` writes `.wm/evidence` and would break PASS. Kernel CHECK is the wall.

- [ ] **Step 4: Re-run map + kernel. t-loop-slice-pass and t-loop-high-one-kind stay green.**

- [ ] **Step 5: Commit** `feat(wm): refuse reviewer/scout writes to owned product paths`

---

### Task 5: Version 1.10.0, changelog, adapters

**Files:**
- Modify: `VERSION` (`1.9.0` → `1.10.0`)
- Modify: `CHANGELOG.md` (promote Unreleased three-verb notes stay; add `## [1.10.0]` one-kind isolation)
- Modify: `docs/whats-new.md`
- Modify: `adapters/grok.md`, `adapters/kiro.md`, `adapters/codex.md` (fresh session; never CROSS-FAMILY on one kind)
- Modify: `WORKING-MODE.md` / `docs/working-mode.md` if Task 1 left gaps
- Test: `scripts/verify-working-mode.sh` already greps VERSION? If a test pins `1.9.0`, update it.

- [ ] **Step 1:** `grep -n '1.9.0' scripts/verify-working-mode*.sh VERSION docs/working-mode.md WORKING-MODE.md docs/whats-new.md` and fix any pin that would fail. Add changelog bullets:

```
## [1.10.0] - 2026-09-15

### One-kind isolation
- HIGH proceeds with one of grok/kiro-cli/codex when maker and reviewer
  agent ids differ. FLOOR/CLOSED `independence: SUBAGENT-ISOLATED`.
- Two kinds still split maker/reviewer; that case is `CROSS-FAMILY`.
  One kind is never labelled CROSS-FAMILY.
- Each `wm run` mints a UUID `session:` (Grok `--session-id`).
- Brief embeds only that station’s ROUTING battery (SKILL + CONTRACT).
- Reviewer/scout that mutate owned product paths are refused.
- MAP-HUMAN still required for HIGH/live.
```

- [ ] **Step 2:** Run full working-mode verifies:

```sh
sh -n wm.sh && sh -n wm-go.sh
sh scripts/verify-working-mode.sh
sh scripts/verify-working-mode-map.sh
sh scripts/verify-working-mode-go.sh
```

Expected: 0 failed. Do not claim green without this output.

- [ ] **Step 3: Commit** `chore: working-mode 1.10.0 one-kind isolation`

---

## Self-review

- Spec coverage: HIGH one-kind proceeds; honest labels; sessions; station packs; owned-path wall; two-kind stronger bar; MAP-HUMAN; no fake CROSS-FAMILY; 11c swap; no Claude; POSIX; tests first.
- Placeholder scan: none.
- Type consistency: `honest_isolation`, `session_uuid`, `station_battery`, `skill_dir`, `append_station_pack`, `{SESSION}` used with the same names in later tasks.
- Out of scope: executed CONTRACT Send-back (1.10b), worktrees (1.12), live 3-CLI walk (1.14), merge, tag v1.9.0.

## Execution

User asked to plan **and deliver with verification**. Execute via subagent-driven-development in `/tmp/crucible-1.10a`. Do not merge. Push a PR only after Task 5 is green and an independent whole-branch review PASSes.
