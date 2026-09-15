---
name: review
description: Judge a delivery unit through code and testing lenses; re-run the named falsifier.
---

Kernel: `.crucible/<program>/wm.sh` from the target repository root. `wm` in examples below means that script.

Job: independently judge one frozen unit. You verify; you do not improve. Maker ≠ reviewer.

## Lenses

Write `reviews/review.md`:

### Code

Read the actual diff on owned paths. Scope (only what SPEC owns), simplicity (no second pattern for a solved problem), correctness. Do not mutate the product. File:line for every finding.

### Testing

Does the named falsifier assert the behaviour, or a proxy? Edge cases, absence, boundaries. You are not the author of `.wm/FALSIFIER`. You **re-run** the command that is already there.

## Re-run the named falsifier

`PASS` is refused unless you recorded evidence of running that exact command:

```
wm evidence <you> -- sh -c "$(sed -n '1p' .wm/FALSIFIER)"
```

(or `wm evidence <you> --` plus the argv). Cite that evidence file. Skipping the test is a defect, not a shortcut.

## Brick words (map words are not CLOSED PASS)

Return `.wm/return/<you>.md`:

```
WORD: PASS|FAIL|BLOCKED|NO-BUILD
EVIDENCE: <path recorded by wm evidence>
```

`wm verdict` / `wm run reviewer` ingest that file. `CLOSED PASS` is written only by `wm close` after a reviewer-role exec. `MAP-ACCEPT` is not a brick WORD and is refused here. Map words are not `CLOSED PASS`.

## Must-not

- Be the maker of this unit.
- Skip the falsifier, apply fixes, or rewrite SPEC to match the diff.
- Load host skill trees. This battery is the review procedure.
- Edit `wm.sh`. Replacing this directory must not require that.
