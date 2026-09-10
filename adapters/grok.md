# Grok adapter (working-mode)

How this machine launches Grok. Not a battery. Not the engine. No secrets.

Working-mode does **not** read `$HOME/.grok/skills`. After
`crucible adopt PROGRAM --managed --working-mode`, Grok discovers repo-root
`.grok/skills/` (views of canonical `.crucible/skills/`). Nested
`.crucible/.grok/skills/` is a projection copy; Grok does not scan it unless
you point `--agent` at a file there.

## Invoke

`wm` does not spawn `grok` by name. Put the CLI on a machine-local `agents.tsv`
row (gitignored) and `wm cast` that agent. `{BRIEF}` is replaced with the
absolute brief path.

```text
name	kind	model	effort	command
alice	grok	grok-4	high	grok -p --prompt-file {BRIEF}
```

Guided-cycle `agents.tsv` uses the same columns. Auth stays in the operator
environment (never in this file, never in skills, never committed).

Non-interactive one-shot: `grok -p --prompt-file {BRIEF}`. Interactive:
`grok --cwd .` then open `{BRIEF}`.

## Skills

Commit `.grok/skills/` in the target. Do not install batteries under `$HOME`.
Swap Grok for Claude by editing `agents.tsv` `kind`/`command` and using
`adapters/claude.md` — do not edit `wm.sh`.
