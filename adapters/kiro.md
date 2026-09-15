# Kiro adapter (working-mode)

How this machine launches Kiro CLI. Not a battery. Not the engine. No secrets.

Working-mode does **not** read `$HOME/.kiro/skills`. After
`crucible adopt PROGRAM --managed --working-mode`, repo skill views are
`.grok/skills/`, `.claude/skills/`, and `.agents/skills/` (views of canonical
`.crucible/skills/`). Nested `.crucible/.agents/skills/` is a projection copy.

ACP (`kiro-cli acp`) is a JSON-RPC **server** for the guided-cycle hop.
Do not pass `kiro-cli acp` as a `wm run` command. Working-mode uses headless
chat.

## Invoke

`.crucible/<program>/wm.sh` does not spawn `kiro-cli` by name. Put the CLI on a
machine-local `agents.tsv` row (gitignored) and
`.crucible/<program>/wm.sh cast` that agent. `{BRIEF}` is replaced with the
absolute brief path. Kind is `kiro` (binary is `kiro-cli`).

```text
name	kind	model	effort	command
bob	kiro	default	high	kiro-cli chat --no-interactive --trust-all-tools 'read {BRIEF} and follow it exactly'
```

Guided-cycle `agents.tsv` uses the same columns. Auth stays in the operator
environment (never in this file, never in skills, never committed).

Non-interactive: `kiro-cli chat --no-interactive --trust-all-tools 'read {BRIEF} and follow it exactly'`.
Do not pass tokens on the command line. Do not use `kiro-cli acp`.

## Skills

Commit repo-root skill views in the target. Do not install batteries under `$HOME`.
Swap Kiro for Grok or Codex by editing `agents.tsv` `kind`/`command` — do not
edit `wm.sh`.
