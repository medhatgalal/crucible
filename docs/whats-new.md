# What is new

Release history stays in `CHANGELOG.md`. This page travels with installed programs and records limits operators still need to account for.

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
