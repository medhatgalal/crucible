# Battery: repo-scout

## In

The target tree's tracked files (packages, `src/`, tests, CI). Not a maker brief. Not a falsifier command. Not `SPEC.md` / `MAP.md` / `INTENT.md` as outputs of this pass. Skip when the tree is greenfield (README / adopt only).

## Out (must-write paths / words)

- `REPO.md` — layout, test command, CI, modules, hotspots.
- Stop. Do not continue into SPEC/MAP/INTENT on this invocation.

## Must-not

- Write `SPEC.md`, `MAP.md`, or `INTENT.md` on the repo-scout pass.
- Write `MAP-ACCEPT` (or any map verdict).
- Stamp `CLOSED PASS` / `CLOSED NO-BUILD` / `PASS`.
- Author `.wm/FALSIFIER` or implement product files.
- Use live tokens, passwords, or `--api-key`.

## Swap

Replacing this directory must not require editing wm.sh.
