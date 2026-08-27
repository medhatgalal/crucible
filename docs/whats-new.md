# What is new

Release history stays in `CHANGELOG.md`. This page travels with installed programs and records limits operators still need to account for.

## Falsifier run pair

Closure refuses unless the item's falsifier was recorded by `crucible run` at the current work id
in both directions — once failing with the mechanism removed, once passing with it restored — and
the gate reads those two files rather than running the falsifier itself.

## Known limits

- `brief` is dispatched by the CLI but is absent from `help`.
- The doc-verb extractor can false-positive on prose inside fences and does not see indented fences; its enclosing README-dependent block does not run in adopted trees.
- The doc-verb `doc_set` omits `roles/*.md`.
- `guided_min_judges` deliberately has no floor once a guided panel has at least one required reviewer row; one required reviewer closes on one PASS.
- A stray claim attempt can leave a TRUE verdict permanently uncounted; there is no undo verb. Separately, `probe-acp ok` permanently invalidates subagent-sealed attempts; this ACP-probe invalidation has no restoration path.
- `scripts/verify-quickstart.sh` is a five-line exec shim to `verify-agent-cycle.sh`, causing a duplicate CI run.
- `adopt` does not copy `CHANGELOG.md`, so it does not travel into adopted trees.
- Rollback after a bad `--refresh`: the refreshed old engine is gone. Recovery is refreshing the same program from an older known-good tag; its evidence and approved panel remain untouched.
- Single-user authorship is unprovable: files written under one operating-system user cannot establish independent identity.
- The falsifier pair proves that two recorded runs of the named falsifier disagreed at the current work id. It does not prove that removing the mechanism is what made them disagree, and under a single user nothing in files can prove that.
