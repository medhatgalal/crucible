# Codex adapter (working-mode)

How this machine launches Codex. Not a battery. Not the engine. No secrets.

Working-mode does **not** read `$HOME/.codex` skill trees or `$HOME/.agents/skills`.
After `crucible adopt PROGRAM --managed --working-mode`, Codex/agents discovery
uses repo-root `.agents/skills/` (views of canonical `.crucible/skills/`). Nested
`.crucible/.agents/skills/` is a projection copy.

## Invoke

`.crucible/<program>/wm.sh` does not spawn `codex` by name. Put the CLI on a
machine-local `agents.tsv` row (gitignored) and
`.crucible/<program>/wm.sh cast` that agent. `{BRIEF}` is replaced with the
absolute brief path.

```text
name	kind	model	effort	command
carol	codex	gpt	high	codex exec -- 'read {BRIEF} and follow it exactly'
```

Guided-cycle `agents.tsv` uses the same columns. Auth stays in the operator
environment (never in this file, never in skills, never committed).

Non-interactive: `codex exec -- 'read {BRIEF} and follow it exactly'`.
Do not pass tokens on the command line.

## Skills

Commit `.agents/skills/` in the target. Do not install batteries under `$HOME`.
Swap Codex for Grok or Claude by editing `agents.tsv` `kind`/`command` — do not
edit `wm.sh`.
