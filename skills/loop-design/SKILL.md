---
name: loop-design
description: Craft, audit, and debrief the small loop. Not the delivery walker.
---

Kernel: `.crucible/<program>/wm.sh` from the target repository root. `wm` in examples below means that script.

Job: design or doctor the **small loop** (observe → choose → act → verify → record → stop). You do not run product slices. `wm loop` is the kernel walker; this battery is not that walker.

## Craft

Write `loop-design/NOTE.md` `## Craft`. For the loop under review, name:

- what is observed (receipts, falsifier, verdicts)
- what chooses the next verb (`wm next` cards, `ROUTING.tsv`)
- what acts (one foreground child, no `&`)
- what verifies (re-run falsifier; maker ≠ judge)
- what is recorded (evidence via `wm evidence`, not hand-waved)
- what stops (`CLOSED PASS` / `CLOSED NO-BUILD` / STOP-ASK / exhausted)

If there is no feedback cycle, it is not a loop — say so. Do not turn a one-shot into theatre.

## Audit

`## Audit` ends with exactly one word:

- `Ready` — the CHECKs still form a loop.
- `Repair needed` — name the missing CHECK or complected job.
- `Not actually a loop` — one-shot; do not pretend otherwise.

Independent verification when self-approval is the risk: the auditor of this note is not the author of the loop under repair.

## Debrief

`## Debrief` may propose the smallest justified patch to a battery or CHECK, as files under `proposals/` only. Applying a battery patch is a human / refresh KEEP decision. One run is not a pattern. Do not rewrite `wm.sh` from debrief.

## Must-not

- Run product, edit `src/`, `wm run` makers, or `wm loop` as this skill's job (not the delivery walker).
- Publish or fetch a Loop Library; no catalog find; no schedule.
- Stamp `CLOSED PASS` or `MAP-ACCEPT`.
- Edit `wm.sh`. Replacing this directory must not require that.
