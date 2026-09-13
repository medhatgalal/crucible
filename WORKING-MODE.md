# WORKING-MODE

Cwd is the **target repository root**, not the program directory.

Forgot the command? Run `.crucible/work/wm.sh` with no args (help), then
`.crucible/work/wm.sh go [IDEA.md]`.

Do not mix guided `drive` or `cycle` with this runner.

## Start

1. Read this file.
2. Run `.crucible/work/wm.sh` (no args), then `go` with an optional idea file.
3. Stay in the foreground. Do not background the walker.

`go` inits if needed, copies a readable idea file onto `IDEA.md` when that
name is missing, stops on a non-empty `QUESTIONS.md` without `ANSWERS.md`,
discovers `grok` / `claude` / `codex` when the panel is empty, and runs `loop`.

Zero CLIs is `INDEPENDENCE_UNAVAILABLE`. Specifier `spec0` is not maker
`make0`. Two CLIs: maker kind is not reviewer kind.

## STOP-ASK

| Card | What you do |
| --- | --- |
| QUESTIONS | Write `ANSWERS.md` or clear `QUESTIONS.md`, then `go` again |
| NEXT INTAKE | Write `IDEA.md`, then `go` again |
| NEXT CAST | Cast the panel, then `go` again |
| NEXT RESEARCH | Research battery then `go` again |
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
