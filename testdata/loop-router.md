# Loop router (D8 / D15)

ADR: architecture/adr/0001-rust-operating-layer.md
ADR-HASH: ef61196fb93aad0ff3c782506a2e49a7cd05a112d19c4aafe44ed272c17a295d

This file is the engine fixture. Copy it to `~/.grok/rules/loop-router.md`.
Keep-current is `crucible doctor` (warn when the home copy is missing or stale versus ADR-HASH).
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
