# WORKING-MODE

Cwd is the **target repository root**, not the program directory.

Forgot the command? Run `.crucible/work/wm.sh` with no args (help), then
`.crucible/work/wm.sh go [IDEA.md]`. Use `status` to read the next card.

Do not mix guided `drive` or `cycle` with this runner.

## Start

1. Read this file.
2. Run `.crucible/work/wm.sh` (no args), then `go` with an optional idea file
   (or `status` to inspect).
3. Stay in the foreground. Do not background the walker.

`go` inits if needed. With no flags: missing `IDEA.md` plus a READY
`BACKLOG.tsv` row is the same as `go --next`; missing `IDEA.md` and no
backlog is `STOP-ASK INTAKE`; a closeable CLOSED plus another READY row
archives and takes the next idea. `go --next` remains an alias.
A readable idea file is copied onto `IDEA.md` when that name is missing.
Non-empty `QUESTIONS.md` without `ANSWERS.md` is `STOP-ASK QUESTIONS`.
`go` discovers `grok` / `kiro-cli` / `codex` when the panel is empty
and walks until `CLOSED`, `STOP-ASK`, or `ESCALATE`.
`go` and `status` write `.wm/FLOOR.md` (station, card, wip, andon,
evidence). `status` does not increment FAIL retries.

Specifier SPEC pass writes `INTENT.md` (`## User` `## Job` `## Non-goals`).
`map-ready` dies if those headings are missing. Reviewer PASS does not require it.
Named `test_entrypoint` must exist. After green FALSIFIER success,
hiding that path must make the command fail. CLOSE writes
`reviews/taste.md` (`## Taste` plus the lesson).
Brownfield `check-module-fit` / `map-ready` will not mkdir a new
`src/<name>`, `packages/<name>`, or `cmd/<name>` unless `QUESTIONS.md`
and `ANSWERS.md` are both non-empty. Greenfield still mkdir.
Existing-repo inventory may write `REPO.md` (`NEXT REPO`) before SPEC.
Greenfield (README / adopt only) skips.

Zero CLIs is `INDEPENDENCE_UNAVAILABLE`. Specifier `spec0` is not maker
`make0`. Two kinds: maker kind is not reviewer kind (`CROSS-FAMILY`).
One kind: HIGH/live still runs after MAP-HUMAN. Isolation is
SUBAGENT-ISOLATED (fresh session, station pack, owned-path wall).
Never labelled CROSS-FAMILY.
Each `wm run` mints a UUID `session:` (Grok `--session-id {SESSION}`).
The brief embeds only that station’s ROUTING battery (SKILL + CONTRACT).
Reviewer/scout that mutate owned product paths are refused.
MAP-HUMAN still required for HIGH/live.
Judging batteries ship a `## Send-back` TSV (`word`, `card`, `cap`, `andon`).
Overlay `.crucible/skills/<bat>/` to change FAIL cap or MAP-REVISE card.
An in-flight slice runs maker/reviewer in `.wm/worktrees/<id>`; CLOSE removes it.

## STOP-ASK

| Card | What you do |
| --- | --- |
| QUESTIONS | Write `ANSWERS.md` or clear `QUESTIONS.md`, then `go` again |
| NEXT INTAKE | Write `IDEA.md`, then `go` again |
| NEXT CAST | Cast the panel, then `go` again |
| NEXT RESEARCH | Research battery then `go` again |
| NEXT REPO | Specifier writes `REPO.md`, then `go` again |
| NEXT SPEC | Cast specifier or write `SPEC.md`, then `go` again |
| NEXT MAP | Cast scout or ingest `MAP-ACCEPT`, then `go` again |
| MAP-REVISE | Specifier revises `MAP.md`, then `go` again |
| MAP-HUMAN | Write `MAP-HUMAN`, then `go` again |
| ARCH | Human / architecture; do not start the next maker |
| ESCALATE LOOP_BOUND | Inspect `.wm/`; bound is max(40, min(240, 16+12×slices)) |
| live / destroy / push-main | Stop. Not unattended |

HIGH needs `MAP-HUMAN`. LOW local maps do not. Recipe for `MAP-HUMAN`:

SIGNED: operator
MAP: MAP.md
SHA256: <sha256 of current MAP.md>

Then `go` again.

Stop on `CLOSED PASS`, `CLOSED NO-BUILD`, `STOP-ASK`, or `ESCALATE`.

Debug (not the start path): `cast`, `loop`, `next`, `map-ready`.
