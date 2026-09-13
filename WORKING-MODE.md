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
name is missing, stops on a non-empty `QUESTIONS.md`, discovers
`grok` / `claude` / `codex` when the panel is empty, and runs `loop`.

Zero CLIs is `INDEPENDENCE_UNAVAILABLE`. Specifier `spec0` is not maker
`make0`. Two CLIs: maker kind is not reviewer kind.

## STOP-ASK

| Card | What you do |
| --- | --- |
| QUESTIONS | Answer or clear `QUESTIONS.md`, then `go` again |
| NEXT SPEC | Cast specifier or write `SPEC.md` |
| NEXT MAP | Cast scout or ingest `MAP-ACCEPT` |
| MAP-REVISE | Specifier revises `MAP.md` |
| MAP-HUMAN | HIGH or live: sign `SIGNED`, `MAP`, `SHA256` of current `MAP.md` |
| ARCH | Human / architecture; do not start the next maker |
| ESCALATE LOOP_BOUND | Inspect `.wm/`; bound is 40 |
| live / destroy / push-main | Stop. Not unattended |

HIGH needs `MAP-HUMAN`. LOW local maps do not.

Stop on `CLOSED PASS`, `CLOSED NO-BUILD`, `STOP-ASK`, or `ESCALATE`.
