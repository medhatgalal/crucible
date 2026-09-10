# Working-mode (opt-in)

Guided 1.6.6 stays the default. Working-mode is a second runner (`wm.sh`) plus four
swap-out batteries. Install with `crucible adopt PROGRAM --managed --working-mode`.
Runtime is the target repo plus one harness CLI. Skills live in the repo
(`.crucible/skills/` and harness views). There is no `$HOME` skill runtime.

This is the coordinator travelling card for **map cadence**. Brick CHECKs
(maker ≠ judge, observed-red, NO-BUILD, live fence) live in `wm.sh` and
`scripts/verify-working-mode.sh`.

## Map cadence (6b)

Architecture names Plane A (`architecture/modules.md`) and cuts Plane B slices
that fit those module roots (`MAP.md`). Critique attacks the map. The kernel
materializes `slices.tsv` and will not start a maker until the map is accepted.

```
id	module	owned_paths	depends_on	risk	status
s1	widget	src/widget/api.py	-	LOW	READY
```

Risk is `LOW` or `HIGH`. Status is `PENDING` (after `wm map-ready`), `READY`
(after `MAP-ACCEPT`), `REVISE`, or `STOP-ASK`.

| Step | Who | Command / word |
| --- | --- | --- |
| Inventory + slices | architecture battery (mapper) | write `MAP.md`; `wm record-mapper --from MAP.md`; `wm map-ready` |
| Attack the map | critique battery (map-judge) | invert + adversarial + simple; return `MAP-ACCEPT` \| `MAP-REVISE` \| `MAP-STOP-ASK` |
| Ingest | kernel | `wm map-verdict RETURNFILE` (calls `wm check-map-word` and, on ACCEPT, `wm check-module-fit`) |
| Human sign | operator | `MAP-HUMAN` when the map is HIGH or live (8c) |
| First slice | kernel | `wm next` → `NEXT SLICE <id>` for the first READY row |
| Brick | maker / reviewer | existing small loop (`wm run maker-falsify` … `wm close`) |

`MAP-ACCEPT` is not `CLOSED PASS`. Map closer ≠ brick closer. Mapper id cannot
be the maker. Architecture author id cannot be the critique author.

`wm loop` stays a foreground walker (no `&`, no daemon). Live / destroy /
push-main remain STOP-ASK (2c).

## Human sign (8c)

After `MAP-ACCEPT`, if any slice is `HIGH` or its module `live_write` is `yes`,
the operator signs the map (`MAP-HUMAN` at repo root) before the first
`wm run maker-falsify`. LOW local maps do not need it. This is a human gate
after the map-judge (8c), not a replacement for critique and not a dummy file.

`wm` parses the same `key: value` shape as return files. `SIGNED:` must be a
non-empty human id — not mapper, maker, reviewer, parent, coordinator, or
loop. `MAP:` must name an existing map file (`MAP.md`). A token such as `x`
is not a sign.

```
SIGNED: operator
MAP: MAP.md
```

Without a valid `MAP-HUMAN` on HIGH/live, `wm run maker-falsify` refuses and
`wm next` is `STOP-ASK MAP-HUMAN`.

## Risk-triggered reviewer (3d)

3d is **label + ROUTING**, not a second engine. HIGH slices require the
reviewer’s panel `kind` to differ from the maker’s `kind` when two harnesses
are cast. If only one harness is present, `wm next` / `wm run maker-falsify`
are `STOP-ASK` rather than fake CROSS-FAMILY. LOW slices may use the same kind.

## Operator commands

```sh
wm map-ready                 # fit + slices.tsv PENDING
wm map-verdict RETURNFILE    # MAP-ACCEPT | MAP-REVISE | MAP-STOP-ASK
wm next                      # NEXT MAP | NEXT SLICE <id> | STOP-ASK | brick card
wm run maker-falsify         # first brick step; HIGH/live need MAP-HUMAN
```

## Harness adapters (13b)

`adopt --working-mode` copies `adapters/grok.md`, `adapters/claude.md`, and
`adapters/codex.md` into `.crucible/<program>/adapters/` when the source tree
has them. Each file says how to invoke that CLI and how to point ignored
`agents.tsv` at it. They are not batteries and they carry no credentials.

`wm` does not spawn a harness by name. Cast a worker whose command is the CLI:

```text
name	kind	model	effort	command
alice	grok	grok-4	high	grok -p --prompt-file {BRIEF}
bob	claude	sonnet	high	claude -p --output-format text "read {BRIEF} and follow it exactly"
carol	codex	gpt	high	codex exec -- "read {BRIEF} and follow it exactly"
dave	grok	grok-4	high	grok -p --prompt-file {BRIEF}
```

`{BRIEF}` is the absolute brief path. Skills resolve from repo-root
`.grok/skills/`, `.claude/skills/`, `.agents/skills/` — not `$HOME`. Mapper,
critique, and maker must be distinct agents. Reviewer re-runs the named
falsifier.
