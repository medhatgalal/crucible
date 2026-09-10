# Battery: review

## In

A frozen delivery unit: `SPEC.md` (owned files, MAKER-WRITES falsifier slot), the maker's product diff, `.wm/FALSIFIER` (one command), observed-red / green receipts if present. You are the cast reviewer, not the maker.

## Out (must-write paths / words)

- `reviews/review.md` with two lenses:
  - `## Code` — scope, simplicity, correctness of the **actual diff**; no mutation.
  - `## Testing` — edge cases, coverage gaps, whether the named falsifier can fail.
- Brick return file: `WORD: PASS|FAIL|BLOCKED|NO-BUILD` plus `EVIDENCE:` from `wm evidence` after you **re-run** the named falsifier yourself.
- Kernel ingest: `wm verdict RETURNFILE` (or `wm run reviewer`). `PASS` requires that evidence.

## Must-not

- Be the maker, or accept a maker-authored verdict.
- Skip the named falsifier. A PASS that did not re-run it is refused.
- Treat map words (`MAP-ACCEPT` / `MAP-REVISE` / `MAP-STOP-ASK`) as `CLOSED PASS`. Map closer ≠ brick closer.
- Apply product fixes, author `.wm/FALSIFIER`, or stamp `CLOSED PASS` yourself (`wm close` is the kernel).

## Swap

Replacing this directory must not require editing wm.sh.
