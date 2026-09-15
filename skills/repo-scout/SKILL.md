---
name: repo-scout
description: Inventory an existing repo into REPO.md (layout, test command, CI, modules, hotspots) before SPEC.
---

Kernel: `.crucible/<program>/wm.sh` from the target repository root. `wm` in examples below means that script.

Job: when `NEXT REPO` / the brief says write `REPO.md` and that file is missing, inventory the tracked tree. You are the specifier on a repo-scout pass. You do not write `SPEC.md` or `MAP.md`. You do not implement.

## REPO.md

Consume the tracked tree (not IDEA-only greenfield). Write a short inventory the later SPEC pass can use:

- layout (packages, `src/`, `cmd/`, tests)
- test command (how this repo proves a unit)
- CI (workflow files if any; otherwise unknown)
- modules (directories that look like Plane A roots)
- hotspots (files that change often or sit on a seam)

Stop after `REPO.md`. A later specifier run writes INTENT/SPEC/MAP.

Greenfield (README / adopt noise only, no product files) skips this pass.

## Must-not

- `SPEC.md` / `MAP.md` / `INTENT.md` / product files on this pass.
- `MAP-ACCEPT` / `MAP-REVISE` / `MAP-STOP-ASK`.
- `CLOSED PASS` / `PASS`.
- `.wm/FALSIFIER` or brick verdicts.
- Live tokens, passwords, `--api-key`.
- Edit `wm.sh`. Replacing this directory must not require that.
