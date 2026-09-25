# Grok adapter (working-mode)

How this machine launches Grok. Not a battery. Not the engine. No secrets.

Working-mode does **not** install skills under `$HOME/.grok/skills`. After
`crucible adopt PROGRAM --managed --working-mode`, Grok discovers repo-root
`.grok/skills/` (views of canonical `.crucible/skills/`) before `$HOME`.
`$HOME` is not the source of truth. Nested `.crucible/.grok/skills/` is a
projection copy; Grok does not scan it unless you point `--agent` at a file there.

## Invoke

`.crucible/<program>/wm.sh` does not spawn `grok` by name. Put the CLI on a
machine-local `agents.tsv` row (gitignored) and
`.crucible/<program>/wm.sh cast` that agent. `{BRIEF}` is replaced with the
absolute brief path. `{SESSION}` is a fresh UUID minted per `wm run`. The
engine quotes both replacements (POSIX double quotes; any `"` in the value
is escaped). Do not wrap `{BRIEF}` or `{SESSION}` in quotes in the command
— that would double-quote.

```text
name	kind	model	effort	command
alice	grok	grok-4	high	grok --session-id {SESSION} --always-approve --no-subagents --disable-web-search --prompt-file {BRIEF}
```

Guided-cycle `agents.tsv` uses the same columns. Auth stays in the operator
environment (never in this file, never in skills, never committed).

Non-interactive one-shot: `grok --session-id {SESSION} --always-approve --no-subagents --disable-web-search --prompt-file {BRIEF}`.
Do not pass `-p` (`--single` requires a prompt string and steals `--prompt-file`).
(engine quotes the path and session id). Interactive: `grok --cwd .` then
open `{BRIEF}`. Discover (`wm go`) uses that one-shot line so each station
gets a distinct Grok session rather than one chat wearing four hats.
One kind is never labelled CROSS-FAMILY; FLOOR/CLOSED record
`SUBAGENT-ISOLATED` when maker and reviewer share a kind (distinct agent
ids). Two kinds is `CROSS-FAMILY`. MAP-HUMAN still required for HIGH/live.

## Skills

Commit `.grok/skills/` in the target. Do not install batteries under `$HOME`.
Swap Grok for Claude by editing `agents.tsv` `kind`/`command` and using
`adapters/claude.md` — do not edit `wm.sh`.

## Loop router

The tracked repo file `.grok/rules/loop-router.md` is the contract path. It is not `$HOME/.grok/rules/loop-router.md`. `crucible doctor --home` is the only writer of the home file. The skill paragraphs above do not describe how this file is loaded.
