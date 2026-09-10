---
name: architecture
description: Name the product's modules and cut slices that fit them (RULE 26).
---

Job: discover Plane A (the architecture that exists) and cut delivery units whose owned paths sit inside it. You are the mapper. You do not accept the map. You do not implement.

## Plane A inventory

Write `architecture/modules.md` as a tab-separated table (header required):

```
module_id	root_path	public_contracts	test_entrypoint	pattern_instance	live_write
```

- `root_path` is a directory that already exists (a package, `src/<name>/`, `cmd/<name>/`). Not a metaphor.
- `pattern_instance` is a path to a real file that shows the pattern this module already uses (RULE 26).
- `live_write` is `yes` or `no` (credentials, destroy, push-main). Recording it is not permission to do it.
- Every path under `src/`, `packages/`, `cmd/` that this work might touch is either under a `root_path` or listed `UNOWNED`. Two plausible packagings → ask once, do not pick in secret.

Do not invent fairy-tale rooms. A room that is not a directory in the tree is a defect.

## RULE 26

Fit the architecture you are in, or argue to change it explicitly. Name the existing pattern with a path to an instance. If the existing architecture is wrong for this work, write `CHANGES-ARCHITECTURE` on the map (and in DESIGN.md if you write one) and STOP. Do not quietly add a second pattern. `wm check-module-fit` treats `CHANGES-ARCHITECTURE` as STOP.

## Slices (Plane B)

Write `MAP.md`:

```
MAPPER: <your-agent-id>

id	module	owned_paths	depends_on	risk
<slug>	<module_id>	<path>[,<path>…]	-|<id>	LOW|HIGH
```

Every `owned_paths` entry must be under that row's module `root_path`. After writing, record identity and fit:

- `wm record-mapper --from MAP.md` (reads `MAPPER:`)
- `wm check-module-fit` (refuses owned paths outside named modules)

The mapper id is durable in `.wm/mapper`. Later `wm cast maker` with that same agent is refused for these slices. Mapper ≠ maker.

## Must-not

- `MAP-ACCEPT` / `MAP-REVISE` / `MAP-STOP-ASK` — critique writes those, via `wm check-map-word`.
- `CLOSED PASS` — brick closer, not a map word.
- Implement product, author the falsifier, spawn makers, or refresh skills.
- Edit `wm.sh`. Replacing this directory must not require that.

Stop: missing inventory, path outside modules, fairy-tale root that does not exist, `CHANGES-ARCHITECTURE`, two packagings with no human pick.
