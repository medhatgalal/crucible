# What is new

Release history stays in `CHANGELOG.md`. This page travels with installed programs and records limits operators still need to account for.

## Unreleased

`go` discovers grok, kiro-cli (kind kiro), and codex only (not Claude Code).
First-class verbs: no-args help, `go`, `status`. No-flag `go` drains a READY
backlog row when `IDEA.md` is missing, `STOP-ASK INTAKE` when there is no
backlog, and archives plus next after a closeable CLOSED. `go --next` remains
an alias. `.wm/FLOOR.md` and `.wm/TRACE.tsv` on `go` / `status`. Live
independence fails closed when fewer than two of grok/kiro-cli/codex are on
PATH or cannot auth. `kiro-cli acp` is a guided-cycle JSON-RPC server, not a
`wm run` argv. Adapter: `adapters/kiro.md`.

## 1.10.1 — send-back kernel

`## Send-back` is a TSV (`word`, `card`, `cap`, `andon`) the kernel
executes. Overlay a judging battery under `.crucible/skills/<bat>/`
to change FAIL cap or MAP-REVISE card without editing `wm.sh`. CI
runs `scripts/verify-working-mode-map.sh`.

## 1.10.0 — one-kind isolation

HIGH proceeds with one of grok/kiro-cli/codex when maker and reviewer
agent ids differ. FLOOR/CLOSED `independence: SUBAGENT-ISOLATED`. Two
kinds still split maker/reviewer (`CROSS-FAMILY`). One kind is never
labelled CROSS-FAMILY. Each `wm run` mints a UUID `session:` (Grok
`--session-id`). Brief embeds only that station’s ROUTING battery.
Reviewer/scout that mutate owned product paths are refused. MAP-HUMAN
still required for HIGH/live.

## 1.9.0 — factory plants

`go --next` drains `BACKLOG.tsv` one map at a time. Optional `REPO.md`
inventory before SPEC. Maker-build diff must stay in `owned_paths`.
Tautological falsifiers (`true` / `:` / `exit 0`) refused. Specifier
writes `INTENT.md` (map-ready existence CHECK, not CLOSE). `.wm/METRICS.tsv`
on close / STOP-ASK / ESCALATE. Still opt-in; no auto MAP-HUMAN.

## 1.8.1 — honesty

Spaced owned paths, live `LOOP_BOUND`, pure `next`, QUESTIONS/`ANSWERS.md`
resume, `go` POSIX. Forgot the command: `.crucible/work/wm.sh` then `go`.

## 1.8.0 — `wm go` and quality loop

Forgot the command: `.crucible/work/wm.sh` (no args) then `go [IDEA.md]`.
Specifier reads IDEA or `STOP-ASK QUESTIONS`. Optional research before SPEC.
Greenfield mkdir roots; MAP-REVISE re-runs specifier. PASS needs
`reviews/review.md`. FALSIFIER must cite `test_entrypoint`. FAIL retries
maker-build (cap 2). LOOP_BOUND scales with slices. Live walk is health-check
or honest `INDEPENDENCE_UNAVAILABLE`. Still opt-in; no auto MAP-HUMAN.

## 1.7.1 — unattended multi-slice and live gate

One foreground `wm loop` walks remaining READY slices; HIGH unsigned is
`STOP-ASK MAP-HUMAN`. A second `wm close` on a closeable `.wm/CLOSED` refuses
and does not append another LESSONS line. Live independence fails closed when grok/claude/codex
are missing or cannot auth. `START.md` / `BOOTSTRAP.md` point at opt-in working-mode
([working-mode.md](working-mode.md)); guided adopt stays `--managed` without
the flag. Commands use `.crucible/<program>/wm.sh` from the target repo root.
`wm run` returns the worker rc. When specifier and scout are cast with a
CLI, `wm loop` writes SPEC/MAP and map-judges unattended; without those
CLIs, `NEXT SPEC` / `NEXT MAP` stay `STOP-ASK`. A no-build red cannot
ingest reviewer PASS (WORD must be NO-BUILD). `LOOP_BOUND` is 40.

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
- Live grok/kiro-cli/codex independence is unavailable when fewer than two of those CLIs are on `PATH` or cannot auth under empty HOME. Claude Code is not required. Fixture workers can still close. Adapters do not ship credentials.
