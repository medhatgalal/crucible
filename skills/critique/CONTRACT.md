# Battery: critique

## In

The named map: `MAP.md` plus `architecture/modules.md`, written by a **different** agent than you. No mapper rationale required in your brief. Not the product tree as something to rewrite.

## Out (must-write paths / words)

- `reviews/critique.md` with exactly three lenses, in this order:
  - `## Invert` — failure-first reading of the map (what the map assumes; dogs not barking; guarded forward).
  - `## Adversarial` — attack surface, contradictions/gaps, residual risk.
  - `## Simple` — what is complected; a decomplect cut; invariants to keep.
- Return file with `WORD:` one of `MAP-ACCEPT` | `MAP-REVISE` | `MAP-STOP-ASK`, `AGENT: <your-id>`, `MAP: MAP.md`.
- Prove identity: `wm check-map-word RETURNFILE` (architecture author id ≠ critique author id).

## Must-not

- Write `MAP-ACCEPT` on a map it authored (mapper id = critique id).
- Draw or rewrite `MAP.md` / `architecture/modules.md` as the primary artifact.
- Stamp `CLOSED PASS` or brick `PASS` — those are not map words.
- Invoke `/full`, `/grade`, `/ult`, or any swiss-army parent. Invert + adversarial + simple only.
- Implement product files, skip the attack, or invent defects to fill the headings.

## Swap

Replacing this directory must not require editing wm.sh.
