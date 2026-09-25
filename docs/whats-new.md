# What is new

Release history stays in `CHANGELOG.md`. This page travels with installed programs and records limits operators still need to account for.

## Unreleased

`crucible room` reads `<cwd>/.crucible/herdr/`. A bad layout exits 2 before herdr and before listen. It attaches to one existing Herdr workspace label and does not create a workspace. It adds a role tab only when that label is absent, and pane-runs `go`, `reap`, and `camera` only on a tab this call created. It probes `GET /health` on `127.0.0.1:1734` and spawns this binary's `serve --bind 127.0.0.1:1734` only on connection refused. Missing herdr does not listen.

`crucible serve --bind 127.0.0.1:PORT` (default `127.0.0.1:1734`) GET `/walk`,
`/stats?since=`, `/health` on loopback only. Same JSON as `status --json` /
`stats --json`. Missing `.wm` is `available: false`. No `POST /go`.

## 1.20.1 — debrief refusal on the page

`debrief` with no `.wm/FLOOR.md` prints `no FLOOR.md (run go or status)` on
stdout and exits 1. No `.wm/TRACE.tsv` prints `no TRACE.tsv` the same way.
The page shows that stdout. It still does not show stderr.

## 1.20.0 — shaping menu

`crucible help` is unchanged unless `.crucible/<program>/shaping` is `grok`.
That value inserts one row naming `modules/shaping/SKILL.md`. It creates no
file. The page does not change. Default is off when the value file is absent.

## 1.19.0 — page calls read-only verbs

`crucible web` on `127.0.0.1:1735` can POST `/act/<verb>` for `agents`,
`debrief`, `next`, `panes`, `stats`, `workid`, and `status --json` only.
The page shows the child's stdout (empty when the child wrote none) and,
when the exit code is not 0, that code. It does not show stderr. It does
not choose the next step. Bare `status`,
`close`, `drive`, `adopt`, `state`, `target`, `brief`, and `lifecycle` are
not on the page. `POST /go` stays unavailable. `POST /act/go` is unchanged.

## 1.17.0 — Rust working-mode kernel

`wm.sh` is an exec wrapper around the `crucible` binary. `go`, `status`,
`debrief`, and `stats` run in Rust. Inner loop remains `.crucible/work/wm.sh go`
(the wrapper execs rust). `crucible --version` prints `1.17.0`. Adopt still
copies the binary plus wrapper as `.crucible/work/crucible` and
`.crucible/work/wm.sh`. Product machines need no rustc. `/execute-plan` is not
the product walker. Rollback is the previous tarball.

## 1.16.4 — continue does not skip brick

`go` clears closeable PASS/NO-BUILD receipts only when CLOSED is missing
(so continue cannot skip brick). STOP-ASK resume keeps in-flight receipts.
`go` resets TRACE; duplicate consecutive FLOOR cards are skipped.

## 1.16.3 — honest FLOOR clock, MAP-REVISE reason, empty te

`go` resets `t=+`. Empty MAP-REVISE is invalid. Empty test files cannot NO-BUILD.

## 1.16.2 — harness question tools

`/crucible` intake uses the harness question tool: Grok `ask_user_question`,
Codex `request_user_input`, Kiro CLI numbered chat (no built-in ask-user tool).

## 1.16.1 — FLOOR stream and debrief

`go` prints elapsed FLOOR lines. `debrief` is the post-run improvement view
(TRACE deltas). Outer `/crucible` must surface FLOOR instead of silent wait.

## 1.16.0 — /crucible outer loop

`/crucible` in Grok: short intake, then `wm.sh go` until CLOSE or a brake.
User-facing: install/update Crucible and kick off. Inner loop unchanged.

## 1.15.2 — shape-commit planted te

`commit_shape` adds planted `test_entrypoint` files so red is not dirty porcelain.

## 1.15.1 — te exists at map-ready

`map-ready` plants a missing `test_entrypoint` (empty file or directory).
`wm init` gitignores `.wm/`.

## 1.15.0 — smooth go

`go` commits shape files before pre-falsify. MAP-REVISE cap runs in the
loop (default 2, then `ESCALATE MAP_REVISE`). Kernel `.wm` is absolute.

## 1.14.2 — grok prompt-file and ./ owned paths

`wm go` grok discover uses `--always-approve --prompt-file` (not `-p`).
Maker-build owned-path CHECK matches `./foo` to `foo`.

## 1.14.1 — kiro host auth

Live kiro probe and exec inherit host `HOME` so keychain OIDC applies.
Empty HOME plus copied `cli.json` hangs; that is not logout.
`kiro-cli acp` remains a JSON-RPC server, not a `wm run` argv.

## 1.14.0 — live one-kind

Live walk proceeds with one of grok/kiro-cli/codex as
`SUBAGENT-ISOLATED` (distinct agent ids). Two kinds still split
maker/reviewer (`CROSS-FAMILY`). Zero of those CLIs is still
`INDEPENDENCE_UNAVAILABLE`. One kind is never labelled CROSS-FAMILY.

## 1.13.0 — entrypoint taste

`map-ready` requires `INTENT.md` headings `## User` / `## Job` /
`## Non-goals`. Named `test_entrypoint` must exist. After green
FALSIFIER success, extra-proof hides that path and the command
must fail. CLOSE writes `reviews/taste.md` (`## Taste` plus the
lesson) for PASS and NO-BUILD.

## 1.12.0 — bet worktree

An in-flight slice gets `.wm/worktrees/<id>`. Maker and reviewer `wm run`
execute there; product/`reviews/` sync back; CLOSE removes the worktree.
Specifier/scout stay in the main checkout.

## 1.11.0 — shape QUESTIONS gate

Brownfield `check-module-fit` / `map-ready` will not mkdir a new
`src/<name>`, `packages/<name>`, or `cmd/<name>` unless `QUESTIONS.md`
and `ANSWERS.md` are both non-empty. Greenfield still mkdir.

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
- Working-mode skills live in the target tree (repo-root `.grok` / `.claude` / `.agents` / `.kiro` `/skills`). That view is the source of truth, not `$HOME`. A dirty laptop that still has host `~/.grok/skills` is not CROSS-FAMILY isolation. Live kiro inherits host HOME (keychain). Workspace `.kiro/skills/<name>` wins over `~/.kiro/skills/<name>`; if the workspace view is missing, kiro may still read the home copy.
- Live grok/kiro-cli/codex independence is unavailable when none of those CLIs are on `PATH` or cannot auth. grok/codex probe under empty HOME with copied auth files. kiro probe/exec inherit host HOME (keychain OIDC); empty HOME hang is not logout. One kind is `SUBAGENT-ISOLATED`; two kinds `CROSS-FAMILY`. Claude Code is not required. Fixture workers can still close. Adapters do not ship credentials.
- `/crucible` inner loop is `.crucible/work/wm.sh go` (wrapper execs rust). `/execute-plan` is not the product walker for adopted repos. Product machines need no rustc.
- Loopback GET serve (`/walk` `/stats` `/health`) does not start a walk; there is no POST `/go`. Non-loopback bind is refused.
