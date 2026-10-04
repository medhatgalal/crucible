# Factory completion

Pickup after the 2026-09-28 reboot. This file is the remaining work. The session plan is not required.

Verified on `main` at tag `v1.28.0` (`c453cac`). Orders O1–O7, corrections C1–C7, and F1–F4 are merged. `crucible orchestrate run` now delivers maker shells through `drive` and waits when it asks. Do not install into a product until the operator names a path.

## Done

Messages, grill, ticket source, stats from `MESSAGES.tsv`, one orchestrator step, `orchestrate run`, assembly script once, and a maker shell that lands through `result`. The 1.28.0 verb `speech` is now `message`. `go` calls `drive` only when `cycle: guided` is present. Kernel `POST /go` stays 405.

## Still open

The eleven-note program that starts after the floor page is tracked in `docs/one-factory-program.md`. F1–F4 below are already merged. They are not that program.

The desk is on `main` at `9f61800`. A legal source message writes one orders row. The page does not emit the 29 verb buttons. Send reloads only when the exit header is `0`. The numbered line appears only for a queue row that is already `paused` or `escalated`. `VERSION` stays `1.28.0`. This desk has no tag.

## Next

Do not install this into another repository.

The page and the CLI read one message file. `message queue` is the dashboard text: queue, graph, pauses, and escalations. `GET /api/factory` prints that same text. `message show` prints `MESSAGES.tsv`. `GET /api/chat` prints that same text. Send still posts a manager sentence to `POST /act/message`. `POST /act/chat` is not a route. Walk stays the kernel camera.

The guided room still pane-runs `orchestrate run` and `message queue` by tab label. Each command sets `CRUCIBLE_ROOT` and its working directory to that checkout. It does not pane-run the chat tab, `go`, `reap`, or `camera`. A person can type `crucible message` in that shell. The page Send is the same sentence.

After this is on `main`, the proofs are three fixtures, not a product repository: a vague idea, a full system, and a bug. Join the existing workspace labeled `crucible`. Do not create a workspace. Do not edit `~/.config/herdr/config.toml`. Command cruft stays until a newer command covers it and the verb-table test is updated in that same change.

## Orders

F1, then F2, then F3, then F4. They share `result.rs`, `orchestrate.rs`, and `crates/room`. Do not run them side by side.

### F1 — Landed message comes from result

In `crates/guided/src/result.rs`, `PASS` with `CLOSE` appends `machine landed <slug>` through `messages::append`. `ESCALATE` appends `machine escalated <slug>`. No other outcome writes a message. The slug is the order id.

Proof: shipped `crucible result` on a returned maker attempt writes that one line. A second result does not write another.

### F2 — One real inner loop

Depends on F1. A fixture `agents.tsv` maker is a shell that writes the owned file and exits 0. Shipped `drive` starts it. The test does not plant `landed`. After `crucible result`, `MESSAGES.tsv` contains `landed` for that order. `guided_go_is_drive_and_reaches_done` stays the pre-seeded client test.

### F3 — The orchestrator keeps going

Depends on F2. `crucible orchestrate run` is the existing command a guided room pane-runs. It delivers each dispatched order through `drive` and waits in that process when it asks. It does not write product files. That tick rolls the product tree back when the coordinator commits, merges, or edits an owned product path. Maker shells still write the files they own. `orchestrate step` stays one pass. The dashboard still runs `message queue` once.

Proof: one `crucible orchestrate run` starts the maker shells. Idle requires `drive worker exit 0` and a `PASS`/`CLOSE` result for each order. A `need-a-fact` keeps that same process waiting until the manager answers. A planted `landed` line is not idle.

### F4 — Tag the factory

Depends on F3. Bump `VERSION` to `1.28.0`. Update `scripts/verify-working-mode.sh`, `scripts/verify-working-mode-go.sh`, `scripts/verify-working-mode-blank-home.sh`, the CLI and room version assertions, and `docs/working-mode.md`. Merge only after Linux `refusals` and macOS `refusals-bsd`. Tag `v1.28.0` on that commit.

## Not in this file

A second dispatch in one step. A second message verb. Jira or GitLab. A ticket help row. Replacing `GET /api/walk`. Porting working-mode `go --next`. Installing into a repository the operator has not named.

## How to verify

Each order is its own pull request. The proof is a test that runs the shipped `crucible` binary. `scripts/selftest.sh` must still see one verb table. Room tests must still show no `config.toml` write and no workspace create. Do not edit `~/.config/herdr/config.toml`. Do not create a Herdr workspace. Do not pop `stash@{0}`. Do not delete `reports/`.

F1–F4 are done and tagged `v1.28.0`. The desk commit is `9f61800`. The room label fix is `fc15034`. The next proofs are three fixtures, not a repository path.
