---
name: critique
description: Attack a named map with invert, adversarial, and simple lenses only.
---

Kernel: `.crucible/<program>/wm.sh` from the target repository root. `wm` in examples below means that script.

Job: independently attack `MAP.md`. You are the map-judge, not the mapper and not a brick reviewer. Mapper ≠ map-judge ≠ later maker.

## Lenses (only these three)

Write `reviews/critique.md`:

### Invert

Read the map as a claim that will fail. What does it assume is true? What absence would the map not notice (dogs not barking)? What is the smallest guarded forward path if the inversion holds? This is critique of the **named map**, not a new map.

### Adversarial

Attack surface of the decomposition (owned-path leaks, missing seams, HIGH/live unmarked). Contradictions and gaps. Residual risk that ACCEPT would swallow. Do not invent defects to fill the template; if the map holds, say so and name the strongest attack that failed.

### Simple

What is complected (delivery unit vs product module vs engine CHECK vs this battery)? Name one decomplect cut. Do not delete honesty CHECKs to make the loop “easy.”

Do **not** run `/full`, `/grade`, `/ult`, or any other parent toolkit. Those are out of this battery.

## Map words (not CLOSED PASS)

Write `.wm/return/<you>.md`:

```
WORD: MAP-ACCEPT|MAP-REVISE|MAP-STOP-ASK
AGENT: <you>
MAP: MAP.md
```

Then: `wm check-map-word .wm/return/<you>.md`.

- `MAP-ACCEPT` — map is fit to run as slices; you did not author it.
- `MAP-REVISE` — named defects; mapper must edit; you still do not become the mapper by rewriting it yourself unless asked, and you still must not ACCEPT your own rewrite.
- `MAP-STOP-ASK` — cannot judge (independence, live fence, `CHANGES-ARCHITECTURE` already STOP).

`CLOSED PASS` and brick `PASS` are refused here. Map closer ≠ brick closer.

## Identity

Architecture author id ≠ critique author id. `wm check-map-word` refuses when `AGENT` equals `.wm/mapper` or `MAPPER:` on the map. You must not write `MAP-ACCEPT` on a map it authored.

## Must-not

- Author `MAP.md` / `architecture/modules.md`.
- Implement product or stamp brick verdicts.
- Edit `wm.sh`. Replacing this directory must not require that.
