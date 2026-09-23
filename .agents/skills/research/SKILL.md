---
name: research
description: Survey stack, constraints, non-goals, and competitors from IDEA.md into RESEARCH.md before SPEC.
---

Kernel: `.crucible/<program>/wm.sh` from the target repository root. `wm` in examples below means that script.

Job: read `IDEA.md` and write `RESEARCH.md` (stack survey, constraints, non-goals, competitors if known). You are the specifier on a research pass. You do not write `SPEC.md` or `MAP.md`. You do not implement.

## RESEARCH.md

Consume `IDEA.md`. Write a short survey the later SPEC pass can use:

- stack / existing tree (languages, packages, services already in-repo)
- constraints (LOW vs live, auth, data store)
- non-goals for v1
- competitors or prior art if the idea names any; otherwise say unknown

Stop after `RESEARCH.md`. A second specifier run writes SPEC/MAP.

Public `curl`/`wget` without `--token` / `--password` / `--api-key` may be used on this pass only. Do not send live tokens or passwords.

## Must-not

- `SPEC.md` / `MAP.md` / product files on this pass.
- `MAP-ACCEPT` / `MAP-REVISE` / `MAP-STOP-ASK`.
- `CLOSED PASS`.
- `.wm/FALSIFIER` or brick verdicts.
- Live tokens, passwords, `--api-key`.
- Edit `wm.sh`. Replacing this directory must not require that.
