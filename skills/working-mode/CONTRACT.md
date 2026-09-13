# Battery: working-mode

Discovery skill for the working-mode runner. Not a ROUTING battery.

## In

A target git repository. Optional `IDEA.md` (or another idea file passed to
`go`). Harness CLIs on PATH (`grok`, `claude`, `codex`) when the panel is empty.

## Out (must-write paths / words)

- No-args `wm.sh` prints help (`run:`, `go`, `harness:`).
- `wm.sh go [IDEA.md]` inits, copies a missing `IDEA.md`, discovers CLIs,
  casts distinct `spec0` / `scout0` / `make0` / `rev0`, then `loop`.

## Must-not

- Mix guided `drive` with this runner.
- Background the walker.
- Auto-write `MAP-HUMAN`.
- Invent SPEC/MAP when specifier/scout CLIs are absent (`STOP-ASK`).

## Swap

Replacing this directory must not require editing wm.sh.
