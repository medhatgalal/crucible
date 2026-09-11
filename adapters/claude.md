# Claude adapter (working-mode)

How this machine launches Claude Code. Not a battery. Not the engine. No secrets.

Working-mode does **not** read `$HOME/.claude/skills`. After
`crucible adopt PROGRAM --managed --working-mode`, Claude discovers repo-root
`.claude/skills/` (views of canonical `.crucible/skills/`). Nested
`.crucible/.claude/skills/` is a projection copy.

## Invoke

`.crucible/<program>/wm.sh` does not spawn `claude` by name. Put the CLI on a
machine-local `agents.tsv` row (gitignored) and
`.crucible/<program>/wm.sh cast` that agent. `{BRIEF}` is replaced with the
absolute brief path.

```text
name	kind	model	effort	command
bob	claude	sonnet	high	claude -p --output-format text "read {BRIEF} and follow it exactly"
```

Guided-cycle `agents.tsv` uses the same columns. Auth stays in the operator
environment (never in this file, never in skills, never committed).

Non-interactive: `claude -p --output-format text "read {BRIEF} and follow it exactly"`.
Do not pass API keys on the command line.

## Skills

Commit `.claude/skills/` in the target. Do not install batteries under `$HOME`.
Swap Claude for Grok by editing `agents.tsv` `kind`/`command` and using
`adapters/grok.md` — do not edit `wm.sh`.
