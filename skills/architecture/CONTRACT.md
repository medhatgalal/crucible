# Battery: architecture

## In

IDEA.md, the target tree (packages, `src/`, tests, existing DESIGN.md / ADRs), and any already-written `architecture/modules.md`. Not a maker brief. Not a falsifier command.

## Out (must-write paths / words)

- `architecture/modules.md` — Plane A inventory, one row per module: `module_id`, `root_path`, `public_contracts`, `test_entrypoint`, `pattern_instance`, `live_write`.
- `MAP.md` — slices whose owned paths sit under those module roots. First line field: `MAPPER: <agent-id>`.
- Record that mapper id with `wm record-mapper --from MAP.md`.
- Fit: `wm check-module-fit` (owned paths ⊆ named `root_path` values).
- If the work needs a second pattern for a solved problem: write `CHANGES-ARCHITECTURE` and STOP. Do not continue the map.

## Must-not

- Write `MAP-ACCEPT` (or any map verdict). Architecture authors the map; it does not accept it.
- Stamp `CLOSED PASS` / `CLOSED NO-BUILD` (those are brick-close words).
- Implement product files, author `.wm/FALSIFIER`, dispatch makers, or cast itself as maker of these slices.
- Invent fairy-tale rooms, floors, or castles that are not packages / directories in the tree.
- Silently introduce a second pattern. `CHANGES-ARCHITECTURE` is STOP, not a footnote.

## Send-back

MAP-REVISE → MAP (specifier rewrites MAP.md, then critique again).

## Andon

CHANGES-ARCHITECTURE → STOP-ASK ARCH. Two packagings → QUESTIONS.

## Swap

Replacing this directory must not require editing wm.sh.
