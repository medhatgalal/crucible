# Codex adapter (working-mode)

How this machine launches Codex. Not a battery. Not the engine. No secrets.

Working-mode does **not** install skills under `$HOME/.codex` or `$HOME/.agents/skills`.
After `crucible adopt PROGRAM --managed --working-mode`, Codex discovery uses
repo-root `.agents/skills/` (views of canonical `.crucible/skills/`). Nested
`.crucible/.agents/skills/` is a projection copy. There is no `.codex/skills`
view: a second tree would duplicate skill names. `$HOME` is not the source of truth.

## Invoke

`.crucible/<program>/wm.sh` does not spawn `codex` by name. Put the CLI on a
machine-local `agents.tsv` row (gitignored) and
`.crucible/<program>/wm.sh cast` that agent. `{BRIEF}` is replaced with the
absolute brief path.
Each `wm run` mints a fresh UUID `session:` in the brief (`{SESSION}` if the
command uses it). One kind is never labelled CROSS-FAMILY; FLOOR/CLOSED
record `SUBAGENT-ISOLATED` when maker and reviewer share a kind (distinct
agent ids). Two kinds is `CROSS-FAMILY`. MAP-HUMAN still required for HIGH/live.

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
