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
| S3 | done `391c369` | One record per agent: new, in progress, finished | A test shows those three states from the factory's own records. No OS process list yet |
| S4 | done `5bd2bbb` | Process watcher | A test reports progress, state, and stop for an agent process the factory started. It does not scan unrelated processes |
| S5 | done `608947c` | Dashboard panels | The page shows agents, repo, reviews, next work, blocked work, git, and intake from records that exist. No panel invents a second queue |
| S6 | done `b7beabb` | Stop, resume, restart, correct | Each verb has one command and one test. None is a second `orchestrate run` |
| S7 | open | Options essay | A paused line shows choices and tradeoffs beside the sentence already on the page |
| S8 | waiting | Keys into an owned pane | A test types into a pane in a workspace this checkout already joined. No key is sent to any other machine |
| S9 | waiting | Git handoff | One command records a git handoff of an order. `orchestrate run` still does not edit product files |
| S10 | waiting | Crucible room beside Herdr | `crucible room` joins the existing workspace. herdr-init and its config are untouched |

## S3 design

`agent_records` reads `attempts/<id>/meta.tsv` and that attempt's `events.tsv`. It does not spawn a process, read a process list, or write a file. One line per agent name. The row kept for an agent is the attempt whose last event epoch is greater. Names sort. The text is the heading `agents` and then `name state`.

`DISPATCHED` is `new`. `RUNNING` and `OVERDUE` are `in progress`. `RETURNED`, `TIMEOUT`, `STOPPED`, and `ABANDONED` are `finished`. Any other state is skipped. An agent with no attempt is absent.

## S4 design

`agent_process` reads one attempt id. It uses that attempt's agent name and the S3 state word, and the last numeric pid of at least 2 in that attempt's `events.tsv`. It runs `kill -0` on that pid only. The line is `name state running` or `name state stopped`. A pid below 2, a `-`, or a missing pid is `stopped`, and `kill` is not called. A live process the attempt did not record does not appear and does not change the line. This slice does not signal the process, scan a process list, or add a command.

Proof: `agent_process_reports_the_recorded_pid_only`. A dash pid stays `stopped` while another process is alive. The recorded pid is `running` until that process exits, then `stopped`, even while the other process is still alive.

## S5 design

`dashboard` reads the served directory. It does not write, spawn, or print the message queue. When that directory has no `PROGRAM` file, one `.crucible` child that is `cycle: guided`, `lifecycle: managed`, and whose `repo:` is that directory supplies the factory records. Zero matches or several matches leave the factory records on the served directory. A second match is not chosen.

`agents` is `agent_records` for that factory directory. `repo` is its `repo:` line. `next` is each `STATE.tsv` row whose status is `ACTIVE`, written `item stage`. `blocked` is each row whose status is `BLOCKED`, written `item block`. Any other status is absent. `reviews` is the regular file `reviews/review.md` in the repo directory. `git` is `branch NAME` when `.git` is a real directory and `HEAD` says `ref: refs/heads/NAME`, plus `slug branch NAME off BASE` from each `items/<slug>/TARGET`. A `.git` file is not followed. A target's repo path is not opened and is not printed. `intake` is the first non-empty `IDEA.md` line, at most 200 characters, and each READY id in `BACKLOG.tsv`. A missing record leaves the heading with no line under it.

The page heading Dashboard loads `GET /api/dashboard`. The `dashboard` verb prints the same text for the program root. Neither is a second queue.

Proof: `dashboard_shows_records_that_exist_and_not_the_queue` and `dashboard_uses_one_guided_program_beside_the_served_directory`.

## S6 design

`attempt stop`, `attempt resume`, `attempt restart`, and `attempt correct` are subcommands of the existing `attempt` verb. None calls `orchestrate run`. None spawns a worker. None is `drive stop`.

`attempt stop ATTEMPT` requires `RUNNING` or `OVERDUE`. It reads the recorded pid. A pid of 2 or more is signaled with `kill -TERM` on that pid only, not its process group. A pid of `-` or empty is not signaled. A pid below 2 is refused with `refusing pid N`, and `STOPPED` is not written. A failed signal does not write `STOPPED`. On success it records `STOPPED` with reason `operator-stop` and blocks the item the same way `attempt finish` does.

`attempt resume ATTEMPT` requires `STOPPED`. The recorded pid must still be alive. A dead pid is refused with `resume requires the recorded pid of ATTEMPT to be alive`, nothing is spawned, and the events stay as they were. A live pid records `RUNNING` with reason `operator-resume` and sets the item `ACTIVE` with that same attempt in flight. The only exit from a terminal state is `STOPPED` to `RUNNING`, and only through this command.

`attempt restart ATTEMPT NEW` requires `STOPPED`. `NEW` must be an attempt id that does not already exist. It copies the attempt meta, sets the new id, state `DISPATCHED`, and `retry_of` to the old id. The new events row is `DISPATCHED` with pid `-` and reason `restart`. The item becomes `ACTIVE` with the new attempt in flight. The old attempt stays `STOPPED`. Nothing is spawned or signaled.

`attempt correct ATTEMPT TEXT` records one manager message of kind `correction`. It does not print the messages path, does not add an orders row, and does not clear a pause.

Proof: `stop_signals_the_recorded_pid_and_refuses_pid_one`, `resume_requires_the_recorded_pid_to_be_alive`, `restart_records_a_new_dispatched_attempt_without_spawning`, and `correct_records_a_manager_correction_and_leaves_the_pause`.

## S7 design

The question text stays empty unless a queue line is `paused` or `escalated`. The sentence `Publish, delete, or leave this machine.` stays the line after `1 ID`. The same question text then lists the three choices in that sentence:

1. Publish. The work leaves this machine.
2. Delete. The copy on this machine is removed.
3. Leave. The work stays on this machine.

`Recommend 3. Leave keeps the work here.` A waiting line, `door waiting`, and an idle queue show neither the sentence nor the choices. The essay is not a second queue, not a new route, and not a message kind. It does not fetch. The send button does not contain it.

Proof: `paused_line_shows_the_options_beside_the_pause_sentence`.

S3 through S10 each get a short design note in this file before their tests. S1 and S2 are fixes to contracts the code already states.

## Stop

Stop before editing herdr-init, creating a workspace, deleting a branch, installing into another repository, bumping `VERSION`, or sending work to another factory. A red required check is fixed on the same branch. A check that cannot be fixed stops the slice.
