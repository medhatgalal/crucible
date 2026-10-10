# One-factory program

The board for the eleven notes. Chat is not the record. Update the status row in the same change that moves a slice. Slices run one at a time. Do not start the next slice while the current pull request is open.

Home is this checkout. Do not install this into another repository. `VERSION` stays `1.28.0` until a release is requested. Do not edit `~/.config/herdr/config.toml`. Do not create a Herdr workspace. Do not displace herdr-init.

## Decisions recorded 2026-10-04

Medhat confirmed the home above. He made an exception to the pause-before-main rule: a slice is squash-merged only after review, fixes, its proof, and green `refusals` and `refusals-bsd`. One verified slice merges, then the next slice starts. Branches are not deleted. He authorized joining the Herdr workspace that already exists.

`message queue` prints `idle` or the four sections `queue`, `graph`, `paused`, and `escalated`. `orchestrate run` does not write product files. The `drive tick` it calls rolls the product tree back when the coordinator commits, merges, or edits an owned product path. Maker shells still write the files they own. A key sender, when that slice starts, types only into panes in a workspace already owned on this machine.

Medhat superseded the S6 rule that `attempt restart`, and `attempt resume` of a dead pid, spawn nothing. Those two will run the maker through the existing deliver path. `attempt stop` stays a signal to the recorded pid. That behavior is B1. It is not built yet.

The build gate for a new row is a bet, then one order on this board whose done-when is the sentence that was bet, then one seam note, then the nine steps below. `MAP-ACCEPT` is not required.

## Decisions recorded 2026-10-05

Medhat confirmed three choices. B1 stays the restart and resume behavior, and it is not the first build. The dashboard choice is `2=c`: the one tab in the crucible workspace becomes the factory report, and each block has a time. That is its own later pitch. herdr-init stays unedited. The pitch choice is `3=a`: the shipped pitch stays sealed, and each remaining behavior gets its own pitch. The pitch now in shaping is that a slice does not reach main unless someone who did not write it has judged it. On 2026-10-05 he accepted that frame and started shaping. Later the same day he accepted the shaped package, then accepted the bet. The one order is visible judgment. Its done-when is: before the slice is on main, a person can see a judgment of that slice and can see that the judge is not the author. The handover is the markdown file in the shaping run. He chose that file and did not ask for HTML, a Google Doc, Word, or JSON. Shaping stopped. On 2026-10-05 he approved that spec. The behavior is on main at `181cd59` (pull request 125), in `crates/guided/src/orchestrate.rs`. The dashboard pitch is next. B1 is after that.

## Decisions recorded 2026-10-08

Medhat answered `1` to the question whose options were: speak both records, leave both stops, or bet the sealed package without those records. The verbatim reply is `1`. It selects the two records in that question. The generic read of one project-local command file is inside attaching Crucible's herdr as configuration tied to Crucible. The stop below allows that one generic edit. The edit must not put a Crucible name inside herdr-init. Herdr-init stays generic. This answer is not a bet, does not open an order, and does not start code.

## Bet recorded 2026-10-08

Medhat answered `1` to the later question whose options were: bet this dashboard package and open one order, or hold the bet. The verbatim reply is `1`. It is the bet. The done-when is the sentence on the `dashboard-tab` row. It is not a separate team-accept sentence, and it does not by itself show the tab.

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

## Stand-in

This checkout is the engine ledger. A guided room or `orchestrate run` here drives that ledger, so an engine slice is not built by the factory loop. Until a guided checkout other than this ledger can carry one order through `drive`, an independent `reviews/review.md`, and a land step that refuses without that record and both required checks, engine work follows `.grok/rules/grok-stand-in.md`.

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
| S7 | done `863f94e` | Options essay | A paused line shows choices and tradeoffs beside the sentence already on the page |
| S8 | done `1a1d481` | Keys into an owned pane | A test types into a pane in a workspace this checkout already joined. No key is sent to any other machine |
| S9 | done `f1f588b` | Git handoff | One command records a git handoff of an order. `orchestrate run` still does not edit product files |
| S10 | done `378091e` | Crucible room beside Herdr | `crucible room` joins the existing workspace. herdr-init and its config are untouched |

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

The paragraphs above are what the code does today. Medhat superseded the spawn-nothing rule for restart and for resume of a dead pid. B1 builds that. Until B1 lands, those two commands still do not run the maker.

## S7 design

The question text stays empty unless a queue line is `paused` or `escalated`. The sentence `Publish, delete, or leave this machine.` stays the line after `1 ID`. The same question text then lists the three choices in that sentence:

1. Publish. The work leaves this machine.
2. Delete. The copy on this machine is removed.
3. Leave. The work stays on this machine.

`Recommend 3. Leave keeps the work here.` A waiting line, `door waiting`, and an idle queue show neither the sentence nor the choices. The essay is not a second queue, not a new route, and not a message kind. It does not fetch. The send button does not contain it.

Proof: `paused_line_shows_the_options_beside_the_pause_sentence`.

## S8 design

`crucible keys PANE KEY...` types those key names into one pane. It reads the workspace label in this checkout's `.crucible/herdr/workspace`, lists Herdr workspaces, and keeps the one workspace whose label matches and whose cwd is this checkout. It lists panes with `--workspace` that id. The pane must be in that list. The keys go to `herdr pane send-keys` for that pane only.

A pane that is not in that list is refused, and no keys are sent. A key or pane that starts with `-` is refused, so the command cannot pass `--machine` or `--remote`. It does not create, close, or rename a workspace. It does not read Herdr's config file. It does not call `pane run`. It does not start `orchestrate run`.

Proof: `keys_type_into_the_joined_workspace_pane_only`.

## S9 design

`crucible handoff ORDER` appends one row to `HANDOFF.tsv` in the program directory: the order id, the branch, and the commit. The order must already be a row in `ORDERS.tsv`. The branch and commit come from the product directory. That directory is the `repo:` line in `PROGRAM` when it is present, otherwise the program directory. `.git` must be a real directory. `HEAD` must say `ref: refs/heads/NAME`, and `refs/heads/NAME` must be a 40-hex commit. A `.git` file is not followed.

The command does not run git, so it does not commit, push, or refresh an index. It does not write into the product directory. It does not send the order anywhere. `orchestrate run` is unchanged and still does not edit product files.

Proof: `handoff_records_the_order_commit_and_leaves_the_product`.

## S10 design

`crucible room` takes no arguments. It lists Herdr workspaces and joins the one whose label is the line in `.crucible/herdr/workspace` and whose cwd is this checkout. Missing standing role tabs are created in that workspace. Zero workspaces, two workspaces, or a cwd that is not this checkout stop with the herdr-init message, and no workspace is created.

The command does not create, close, or rename a workspace. It does not read or write a Herdr config file. It does not edit herdr-init. A checkout that is not `cycle: guided` prints `go not started` and does not pane-run.

Proof: `room_joins_the_existing_workspace_and_leaves_herdr_init`.

S3 through S10 each get a short design note in this file before their tests. S1 and S2 are fixes to contracts the code already states.

## Orders

A row here is a bet. Visible judgment is on main at `181cd59` (pull request 125).

| Id | Status | Work | Done when |
| --- | --- | --- | --- |
| visible-judgment | done | Extend the existing land decision | Before the slice is on main, a person can see a judgment of that slice and can see that the judge is not the author |
| dashboard-tab | done | The crucible workspace tab shows the existing factory report, with one time on each block | The one dashboard tab in the crucible workspace shows the existing factory report, and agents, repo, reviews, next, blocked, git, and intake each show the same readable time |

## dashboard-tab seam

One order. The proof sentence is the done-when on the row. This note is the seam. The bet is the `1` recorded above. The shaped package is the three pieces below. The 2026-10-08 attach exception allows the generic read. It does not name Crucible inside herdr-init.

### Objective

Medhat looks at the one dashboard tab in the crucible workspace and reads a time on each block of the existing factory report. The page, the `dashboard` verb, and one room pane are not that tab.

### Scope

In: a generic read of `.herdr-config/dashboard.command` in the herdr-init source the installer publishes. Missing, absent, or empty keeps `layout-tree.py`, `agents-live.sh`, `recent-activity.sh`, and `reaper-log.sh`. One non-empty line, of at most 4096 bytes, runs that line. A symlink, a file over that size, or any other text does not launch. The reader has no Crucible name, no new profile key, and no report command of its own. A new dashboard does not split. Recovery of one pane, or of those four labels, uses that read and does not unsplit, so each idle shell runs the line. A pane that is not an idle shell, and is not already running that line, is preserved. This checkout gets that file, and its one line prints `dashboard` for the directory the launcher already has. `dashboard` stamps one captured instant on all seven blocks. The body bytes under each heading stay the bytes they are today.

### Non-goals

B1. B9. B10. B11. A second dashboard, a second queue, or a second meaning of `GET /api/dashboard`. An edit of `orchestrate.rs`, of `crates/cli/tests/cli.rs`, or of `program_root`. A maximum age. The four-second camera sleep as a required poll. The layout header clock or the camera header clock as this time. A hand-edit of an installed herdr-init revision. A Crucible name inside herdr-init. `VERSION`, a tag, a branch delete, or a live `crucible room` against this ledger. A claim that the cargo test is the tab.

### Commands

Test this order's clock: `cargo test -p crucible-guided dashboard_stamps_one_instant_on_all_seven_blocks`

The herdr-init reader test lives in that source's own tree. Its fixture line is not a Crucible command. It is not this cargo test. The wider suite stays `scripts/selftest.sh`. The required checks stay `refusals` and `refusals-bsd`. Those checks are not this order's proof.

### Shape of the time

Each block is the heading, a newline, the instant, a newline, then the existing body. The same instant is captured once. That includes the agents block from `agent_records` and the failure text `agents\nunreadable\n`. The production instant is UTC `YYYY-MM-DDTHH:MM:SSZ`. Tests pass the instant in. This is the shape the research spike checked. The bet required a readable time and did not name a second shape.

### Command line

On 2026-10-08 Medhat answered `1`. `crucible dashboard` with no argument still prints `dashboard` for `program_root()`. `crucible dashboard DIR` prints `dashboard` for `DIR` and does not resolve `program_root()`. Two arguments, an empty argument, or an argument that starts with `-` are refused with `usage: crucible dashboard [DIR]`. Other verbs still resolve `program_root()`. The panes are created with the launcher's cwd. This checkout's `.herdr-config/dashboard.command` is the one line `crucible dashboard .` The generic read of that file is in the herdr-init source on its default branch at `0a3eb0d`. On 2026-10-09 the reviewed installer plan published herdr-init revision `62ec7499ae98c1a1137143ea337f269379c5ee9b70edb9589e5eeda7d7f3af63` from that commit. That revision reads the file and contains no Crucible name.

### Proof

`dashboard_stamps_one_instant_on_all_seven_blocks` is the clock proof. It is on `main` at `7e90bcd`. It fails while a block has no instant, while two blocks show different instants, or while a body byte changes to make room for the time. It passes when all seven blocks show the injected instant and the bodies are otherwise unchanged, including `agents\nunreadable\n`.

The tab proof is a look after the installer-published launcher is the one running. A green cargo test, the page, and the verb do not pass that look. On 2026-10-10 that launcher typed the checkout line into a new idle dashboard pane. The pane showed agents, repo, reviews, next, blocked, git, and intake, each at `2026-10-10T05:03:29Z`.

### Boundaries

Always: one report, one instant, one generic read, no Crucible name in herdr-init.

Ask first: a third launch behavior for a file that is neither empty nor one line. That need stops this order.

Never: depend on Jira or GitLab. Install this repo into another repository. Edit herdr-init beyond the generic read. Treat this checkout as a second product home.

### Where the order is

The clock proof and `dashboard_dir_uses_the_argument_and_leaves_the_default` are on `main` at `7e90bcd`. The checkout file is on `main`. The generic read is on the herdr-init source's default branch at `0a3eb0d`, and the 2026-10-09 publish of revision `62ec7499ae98c1a1137143ea337f269379c5ee9b70edb9589e5eeda7d7f3af63` reads the file. On 2026-10-10 the dashboard tab showed that report, and the seven blocks showed `2026-10-10T05:03:29Z`. The earlier four-pane camera tab remains, under another label, and its running camera was left in place. The row is `done`.

## Named backlog

A row here is not an order. It becomes an order only after its own pitch is bet. S0–S10 stay closed.

| Id | Status | Work | Done when |
| --- | --- | --- | --- |
| B1 | waiting | Restart and resume run the maker | `attempt restart` of a stopped attempt runs the maker through the existing deliver path. `attempt resume` of a dead pid does the same. A correction for that order is visible to the waiter. Neither command is a second `orchestrate run`. `attempt stop` stays a signal to the recorded pid |
| B9 | named | Page appearance | The same verbs and the same sections, after a visual pass. Its own pitch |
| B10 | named | Human-feedback eval | A rating record that is not `answer` or `correction`. Its own pitch |
| B11 | named | Telemetry export | A defined export. The TSV files are not that export. Its own pitch |

## Stop

Stop before editing herdr-init, creating a workspace, deleting a branch, installing into another repository, bumping `VERSION`, or sending work to another factory. The 2026-10-08 choice allows one exception to the herdr-init clause: a generic read of one project-local command file, with no Crucible name inside herdr-init. That exception is not a bet and does not open an order. A red required check is fixed on the same branch. A check that cannot be fixed stops the slice.
