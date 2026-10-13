# Scorecard

The repo holds this table. A row is `2` only when the same proof has a receipt on this engine checkout and on a fresh host repo. A model summary is not a receipt. Chat is not this file.

Side E is this checkout, run with `./crucible`. Side T is a new git repository that is not this tree, installed with `adopt` from this source. `0` means that side has no receipt. `1` means one side has a receipt on the current commit. `2` means both receipts are linked from the pull request that landed the row. A `2` falls back to `1` when `main` moves and the receipt was not re-run.

Autopilot and a second work order stay blocked until SC-5 through SC-13 are all `2`.

## Architect note — role text

Dispatch crosses a boundary. The program directory is `.crucible/<program>/`. The engine tree is the repo that contains `roles/`. A copied install has `roles/` and a `crucible` binary in the program directory. A state-only install has neither, and the role text stays at `<repo>/roles/<role>.md`.

The rule: a file in the program wins. If the program has no `roles/<role>.md` and no copied `crucible` binary, read `<repo>/roles/<role>.md` from the `repo:` field. Otherwise refuse `no such role`. There is no second lookup.

## Rows

| Id | Sentence | E | T | Blocked by |
| --- | --- | --- | --- | --- |
| SC-1 | Proof is a run of the real binary, not a checklist or a model saying it worked | 0 | 0 | No `./crucible` receipt on this commit |
| SC-2 | An architect check happens before code that crosses a boundary | 0 | 0 | The note is in the same change as the code. No `./crucible` receipt |
| SC-3 | Boundaries are folders, and a gate rejects illegal imports and banned slop before review | 0 | 0 | Crates exist. No import gate |
| SC-4 | One paved path for each job | 0 | 0 | No `./crucible` receipt on this commit. `IDEA.md` remains the unported kernel walk |
| SC-5 | A repeated mistake becomes a test, lint, CI check, or rule | 0 | 0 | The discriminator is a library test. No `./crucible` receipt on this commit |
| SC-6 | A feature map is generated, kept in the repo, and pull requests link their evidence | 0 | 0 | No generated map |
| SC-7 | A gardener pass proposes a new rule or a deletion | 0 | 0 | No gardener |
| SC-8 | Nothing merges without evidence | 0 | 0 | A green selftest is not a receipt that a merge without evidence is refused |
| SC-9 | The repo holds status, skills, and allowlists | 0 | 0 | This file is not a `./crucible` receipt |
| SC-10 | Autopilot and parallel runs stay blocked until SC-5 through SC-13 are all 2 | 0 | 0 | Stays blocked |
| SC-11 | Parallel work has join rules before merge | 0 | 0 | Stays blocked with SC-10 |
| SC-12 | Work resumes after a crash | 0 | 0 | Restart and dead-pid resume are on main and were not re-run on this commit |
| SC-13 | Each work order is isolated in its own branch and folder | 0 | 0 | Stays blocked with SC-10 |

## Not in SC-5 through SC-13

| Row | Sentence | Score |
| --- | --- | --- |
| Drop-in | A fresh host runs `TRY.md` through `cycle problem` onto `PROBLEM.md`, then `drive`, until the human gate or done | 0 |
| Board | A stage board is generated from git and can show several work orders | 0 |
| Plugs | Grok, Codex, Kiro, and Claude stay adapters | Not re-proved on a host |
| Auth | Real authentication for actors | Open. One OS user cannot prove identity. Do not record this as closed |
| Docs | Standing docs match the live policy | The role-text sentence is in `docs/install.md` on this branch |
| Report title | The seven-block report prints the problem title | Not this branch. Not a bet |

`TRY.md` is the host file the operator writes. It is not a third intake verb. The kernel walk on `IDEA.md` stays refused until a later bet.

B9, B10, and B11 stay named. This file does not mark them done.
