# Loop router (D8 / D15)

ADR: architecture/adr/0001-rust-operating-layer.md
ADR-HASH: 8e62d40ad5812be972e5e111513a2f295301dbae67e1eab7f77577dbd112821e

This file is the engine fixture. The tracked copy is `<repo>/.grok/rules/loop-router.md` (a regular file, identical bytes, not a symlink). Grok loads that tracked file. Do not copy it to `$HOME` unless the operator runs `crucible doctor --home`.
Keep-current is `crucible doctor` (warn when `<cwd>/.grok/rules/loop-router.md` is missing or stale versus ADR-HASH; it does not write).
Do not invent `/loop`.
Engine CI hashes this file against the ADR (D8 + D15 + Signal). Never `$HOME`.

## Signal

If `/crucible` is live, or a walk is live (FLOOR / `go`): follow Crucible.
Do **not** force `/execute-plan` inside `/crucible`. Workers are harness CLIs
Crucible starts. Grok slash is not inside a walk unless the user interrupts.

If the user named another framework (EngOS, herdr-init, Superpowers, …):
follow that porcelain.

Otherwise: Grok-native. Phrase map + `NEXT:` + Enter-to-send.

Interrupt wins until `/crucible` or `go` again.

| Mode | Signal | Next-step hints |
| --- | --- | --- |
| Crucible | `/crucible` or FLOOR/`go` live | FLOOR, STOP-ASK, `wm.sh status` |
| Other | User named EngOS / herdr-init / … | That porcelain |
| Grok-native | Neither | Phrase map + `NEXT:` + Enter-to-send |
