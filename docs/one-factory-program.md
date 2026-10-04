# One-factory program

The board for the eleven notes. Chat is not the record. Update the status row in the same change that moves a slice. Slices run one at a time. Do not start the next slice while the current pull request is open.

Home is this checkout. Do not install this into another repository. `VERSION` stays `1.28.0` until a release is requested. Do not edit `~/.config/herdr/config.toml`. Do not create a Herdr workspace. Do not displace herdr-init.

## Decisions recorded 2026-10-04

Medhat confirmed the home above. He made an exception to the pause-before-main rule: a slice is squash-merged only after review, fixes, its proof, and green `refusals` and `refusals-bsd`. One verified slice merges, then the next slice starts. Branches are not deleted. He authorized joining the Herdr workspace that already exists.

`message queue` prints `idle` or the four sections `queue`, `graph`, `paused`, and `escalated`. `orchestrate run` does not edit product files. Maker shells still write the files they own. A key sender, when that slice starts, types only into panes in a workspace already owned on this machine.

## What is already on main

`35fbd55` shows `.wm/FLOOR.md` in the Floor area and adds `Publish, delete, or leave this machine.` only when a queue line is already `paused` or `escalated`. Pull request 111.

## Loop for every remaining slice

1. Write this slice's scope, non-goals, and proof in the row below before editing product code.
2. Add the failing test named in the proof.
3. Implement only that slice.
4. Run the proof. Fix until it passes.
5. Open a pull request containing the product change, the doc update, and this row's new status.
6. Review the diff. Fix on the same branch.
7. Wait until `refusals` and `refusals-bsd` pass.
8. Squash-merge. Fast-forward local `main`. Leave the remote branch.
9. Mark the row `done` with the merge commit. Start the next row.

## Slices

| Id | Status | Work | Done when |
| --- | --- | --- | --- |
| S0 | done `91f9400` | This board | This file is on `main` and names every slice below |
| S1 | done `2ab3b34` | Queue text matches the printer | `source_legal_id_creates_an_orders_row_and_queue_prints_waiting` expects the four sections. The page test that treats `door waiting` as not a pause stays |
| S2 | done `3b1bd6f` | Send can answer a pause | The page can post kind `answer` and the order id. A `source` send still does not clear a pause |
| S3 | open | One record per agent: new, in progress, finished | A test shows those three states from the factory's own records. No OS process list yet |

## S3 design

`agent_records` reads `attempts/<id>/meta.tsv` and that attempt's `events.tsv`. It does not spawn a process, read a process list, or write a file. One line per agent name. The row kept for an agent is the attempt whose last event epoch is greater. Names sort. The text is the heading `agents` and then `name state`.

`DISPATCHED` is `new`. `RUNNING` and `OVERDUE` are `in progress`. `RETURNED`, `TIMEOUT`, `STOPPED`, and `ABANDONED` are `finished`. Any other state is skipped. An agent with no attempt is absent.
| S4 | waiting | Process watcher | A test reports progress, state, and stop for an agent process the factory started. It does not scan unrelated processes |
| S5 | waiting | Dashboard panels | The page shows agents, repo, reviews, next work, blocked work, git, and intake from records that exist. No panel invents a second queue |
| S6 | waiting | Stop, resume, restart, correct | Each verb has one command and one test. None is a second `orchestrate run` |
| S7 | waiting | Options essay | A paused line shows choices and tradeoffs beside the sentence already on the page |
| S8 | waiting | Keys into an owned pane | A test types into a pane in a workspace this checkout already joined. No key is sent to any other machine |
| S9 | waiting | Git handoff | One command records a git handoff of an order. `orchestrate run` still does not edit product files |
| S10 | waiting | Crucible room beside Herdr | `crucible room` joins the existing workspace. herdr-init and its config are untouched |

S3 through S10 each get a short design note in this file before their tests. S1 and S2 are fixes to contracts the code already states.

## Stop

Stop before editing herdr-init, creating a workspace, deleting a branch, installing into another repository, bumping `VERSION`, or sending work to another factory. A red required check is fixed on the same branch. A check that cannot be fixed stops the slice.
