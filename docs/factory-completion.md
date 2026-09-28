# Factory completion

Pickup after the 2026-09-28 reboot. This file is the remaining work. The session plan is not required.

Verified on `main` at tag `v1.28.0` (`c453cac`). Orders O1–O7, corrections C1–C7, and F1–F4 are merged. `crucible orchestrate run` now delivers maker shells through `drive` and waits when it asks. Do not install into a product until the operator names a path.

## Done

Speech, grill, ticket source, stats from `SPEECH.tsv`, one orchestrator step, `orchestrate run`, assembly script once, and a maker shell that lands through `result`. `go` calls `drive` only when `cycle: guided` is present. Kernel `POST /go` stays 405.

## Still open

F1–F4 are tagged at `v1.28.0` (`c453cac`). This change makes `crucible orchestrate run` start the maker shells and wait for an answer. No new command. No product install.

## Orders

F1, then F2, then F3, then F4. They share `result.rs`, `orchestrate.rs`, and `crates/room`. Do not run them side by side.

### F1 — Landed speech comes from result

In `crates/guided/src/result.rs`, `PASS` with `CLOSE` appends `machine landed <slug>` through `speech::speech`. `ESCALATE` appends `machine escalated <slug>`. No other outcome writes speech. The slug is the order id.

Proof: shipped `crucible result` on a returned maker attempt writes that one line. A second result does not write another.

### F2 — One real inner loop

Depends on F1. A fixture `agents.tsv` maker is a shell that writes the owned file and exits 0. Shipped `drive` starts it. The test does not plant `landed`. After `crucible result`, `SPEECH.tsv` contains `landed` for that order. `guided_go_is_drive_and_reaches_done` stays the pre-seeded client test.

### F3 — The orchestrator keeps going

Depends on F2. `crucible orchestrate run` is the existing command a guided room pane-runs. It delivers each dispatched order through `drive` and waits in that process when it asks. It does not edit product files. `orchestrate step` stays one pass. The dashboard still runs `speech queue` once.

Proof: one `crucible orchestrate run` starts the maker shells. Idle requires `drive worker exit 0` and a `PASS`/`CLOSE` result for each order. A `need-a-fact` keeps that same process waiting until the manager answers. A planted `landed` line is not idle.

### F4 — Tag the factory

Depends on F3. Bump `VERSION` to `1.28.0`. Update `scripts/verify-working-mode.sh`, `scripts/verify-working-mode-go.sh`, `scripts/verify-working-mode-blank-home.sh`, the CLI and room version assertions, and `docs/working-mode.md`. Merge only after Linux `refusals` and macOS `refusals-bsd`. Tag `v1.28.0` on that commit.

## Not in this file

A second dispatch in one step. A second speech verb. Jira or GitLab. A ticket help row. Replacing `GET /api/walk`. Porting working-mode `go --next`. Installing into a repository the operator has not named.

## How to verify

Each order is its own pull request. The proof is a test that runs the shipped `crucible` binary. `scripts/selftest.sh` must still see one verb table. Room tests must still show no `config.toml` write and no workspace create. Do not edit `~/.config/herdr/config.toml`. Do not create a Herdr workspace. Do not pop `stash@{0}`. Do not delete `reports/`.

After F4, stop. The next order is a repository path from the operator.
