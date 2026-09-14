# Battery: research

## In

`IDEA.md` and the target tree (packages, `src/`, existing docs). Not a maker brief. Not a falsifier command. Not `SPEC.md` / `MAP.md` as outputs of this pass.

## Out (must-write paths / words)

- `RESEARCH.md` — stack survey, constraints, non-goals, competitors if known.
- Stop. Do not continue into SPEC/MAP on this invocation.

## Must-not

- Write `SPEC.md` or `MAP.md` on the research pass.
- Write `MAP-ACCEPT` (or any map verdict).
- Stamp `CLOSED PASS` / `CLOSED NO-BUILD`.
- Author `.wm/FALSIFIER` or implement product files.
- Use live tokens, passwords, or `--api-key` (public `curl`/`wget` without those flags is allowed on this pass only).

## Send-back

None. Missing RESEARCH.md keeps NEXT RESEARCH.

## Andon

None beyond kernel STOP-ASK.

## Swap

Replacing this directory must not require editing wm.sh.
