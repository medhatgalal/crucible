# What is new

Release history stays in `CHANGELOG.md`. This page travels with installed programs and records limits operators still need to account for.

## 1.7.1 — unattended multi-slice and live gate

One foreground `wm loop` walks remaining READY slices; HIGH unsigned is
`STOP-ASK MAP-HUMAN`. A second `wm close` on a closeable `.wm/CLOSED` refuses
and does not append another LESSONS line. Live independence fails closed when grok/claude/codex
are missing or cannot auth. `START.md` / `BOOTSTRAP.md` point at opt-in working-mode
([working-mode.md](working-mode.md)); guided adopt stays `--managed` without
the flag. Commands use `.crucible/<program>/wm.sh` from the target repo root.
`wm run` returns the worker rc. When specifier and scout are cast with a
CLI, `wm loop` writes SPEC/MAP and map-judges unattended; without those
CLIs, `NEXT SPEC` / `NEXT MAP` stay `STOP-ASK`. `LOOP_BOUND` is 40.

## 1.7.0 — opt-in working-mode

`crucible adopt PROGRAM --managed --working-mode` copies `wm.sh`, four batteries,
harness views, and `adapters/{grok,claude,codex}.md`. Guided 1.6.6 remains the
default: adopt without `--working-mode` still has no wm or skills. Operator
card: [working-mode.md](working-mode.md). Adapters: `adapters/grok.md`,
`adapters/claude.md`, `adapters/codex.md` — point ignored `agents.tsv` at the
CLI; do not put credentials in those files.

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
- Working-mode is opt-in. Default adopt does not install `wm.sh` or skills.
- Working-mode skills live in the target tree. `$HOME` skill trees are not a runtime. A dirty laptop that still has host `~/.grok/skills` is not CROSS-FAMILY isolation.
- Live grok/claude/codex independence is unavailable when those CLIs are missing from `PATH` or cannot auth under empty HOME. Fixture workers can still close. Adapters do not ship credentials.
