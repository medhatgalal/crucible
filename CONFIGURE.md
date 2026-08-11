# CONFIGURE

How to set this up for a real repo, and what to actually decide. Read this once; after that
`.crucible/<program>/crucible next` tells you what to do.

Programs may select [managed lifecycle](docs/managed-lifecycle.md) before their first item. The
selection is stored as `lifecycle: managed` in `PROGRAM`; absence preserves item-file behavior.

---

## 1. Where it lives — inside the target repo

`adopt`, run from inside the repository you want to work on, installs a self-contained program at
`<repo>/.crucible/<program>/`. The repository stays your working directory, which is what agents
expect, and everything the program learns is stored and versioned in the repository it learned it in.

```
<engine checkout>/            cloned once, anywhere; only used to run `adopt`
  crucible  BOOTSTRAP.md  START.md  RULES.md  LOOP.md  CONFIGURE.md  roles/  scripts/

<your repo>/
  .crucible/
    .gitignore                generated: ignores */agents.tsv
    <program>/                one directory per effort. Committed with your code.
      crucible  BOOTSTRAP.md  START.md  RULES.md  LOOP.md  CONFIGURE.md  roles/  scripts/
      agents.tsv              YOUR panel. Machine-specific, gitignored.
      PROGRAM  STATE.md  CLAIMS.md  BACKLOG.md  LESSONS.md
      claims/<CN>/{dispatches,evidence,verdicts}
      items/<slug>/{ITEM.md,TARGET,dispatches,evidence,verdicts,work}
```

Everything runs as `.crucible/<program>/crucible <verb>` from the repo root. The engine checkout can
be deleted afterwards: each program carries its own copy, pinned at the version that installed it.

**A second effort against the same repo is a second `adopt`** — its own panel, its own backlog, its
own lessons, no interaction with the first.

## 2. Configure it

```sh
cd /path/to/your/repo
<engine>/crucible adopt security-audit      # installs and writes a starter agents.tsv
$EDITOR .crucible/security-audit/agents.tsv
.crucible/security-audit/crucible agents    # check what you declared
```

`agents.tsv` is five tab-separated columns. `{BRIEF}`, `{MODEL}` and `{EFFORT}` are substituted:

```
lead	kiro	claude-opus-5	max	kiro-cli chat --no-interactive --trust-all-tools --model {MODEL} --effort {EFFORT} "read {BRIEF} and follow it exactly"
mk1	kiro	claude-sonnet-5	high	kiro-cli chat --no-interactive --trust-all-tools --model {MODEL} --effort {EFFORT} "read {BRIEF} and follow it exactly"
j1	grok	grok-4.5	high	grok -p "read {BRIEF} and follow it exactly" --model {MODEL}
j2	codex	gpt-5-codex	high	codex exec --skip-git-repo-check -m {MODEL} "read {BRIEF} and follow it exactly"
```

`kind` is what `CRUCIBLE_MIN_KINDS` counts, and it is the only column with teeth: a verdict from a
name not in this file is refused. Name agents by role-ish function, not by model, so you can swap the
model without renaming anything.

Then `.crucible/<program>/crucible dispatch <item> <role> <agent>` prints the exact command to run, model and effort
already substituted. That is the whole interface.

## 3. What to decide, with the reasoning

**Which model leads.** The orchestrator never writes code; its entire job is sequencing and judgement
about what is done. Give it your strongest model at your highest effort. It is also the cheapest place
to spend, because it emits dispatches, not diffs.

**Which model makes.** A cheap, fast model is fine *to the extent the plan is specific*. That is the
actual trade: task specificity is what lets you downgrade the maker. Vague task, strong maker. If a
maker returns `BLOCKED` or `NEEDS_CONTEXT`, the answer is usually a better task, not a better model —
try the plan first, then upgrade.

**Which models judge.** At least one judge must be a **different kind** from the maker. The only
measured result this project has: three same-kind reviewers each repeated a false file path, and one
cross-kind judge caught it in under two minutes. **Kind diversity beats judge count.** Two judges with
`CRUCIBLE_MIN_KINDS=2` is stronger than three of the same kind.

**Effort for the adversary.** Highest you have. Its job is the failure nobody thought of, which is the
one task where reasoning depth pays most directly.

## 4. How many agents

| Work | Judges | Setting |
|---|---|---|
| Docs, comments, a typo, reversible config | 1 | `CRUCIBLE_MIN_JUDGES=1` |
| Anything that changes behaviour — the default | 2, different kinds | `CRUCIBLE_MIN_KINDS=2` |
| Auth, data, migrations, deletion, a hot path, anything hard to reverse | 3, at least 2 kinds | `CRUCIBLE_MIN_JUDGES=3 CRUCIBLE_MIN_KINDS=2` |
| Plus an adversary | on every item that reaches GRADUATE | it is a phase, not an option |

**Makers: exactly one per task, always.** Two makers on one file overwrite each other, which is why
`TASKS.md` assigns file ownership per task. Note honestly: nothing parses `TASKS.md`, so that
ownership is a RULE the planner and its judge enforce by reading, not a CHECK the gate applies. Parallelism comes
from running several *items* or several *non-overlapping tasks*, never from two makers in one file.

So a typical item uses 4–6 agent invocations: one specifier, one architect, one planner, one maker per
task, two judges, one adversary — several of which can be the same registered agent in a fresh session.

## 5. Which roles to keep

Ten are provided. A role you delete from `roles/` simply cannot be dispatched — the script refuses an
unknown role — so pruning is safe and is the intended way to configure.

Keep always: **orchestrator, maker, judge**. That trio is the mechanism; everything else is staging.
Add **adversary** for anything you would be embarrassed to ship wrong.

Drop when they cost more than they return: **claim-auditor** and **scout** if the work comes from your
own head rather than a report you distrust; **architect** for small self-contained changes;
**specifier** and **planner** if you write `ITEM.md` and `TASKS.md` yourself, which for a handful of
items is often faster; **integrator** if you prefer to merge by hand.

Editing a role file changes what every agent in that role is told, permanently and everywhere. That is
the intended place to encode your standards — the role file is the skill.

## 6. Code versus prose, and what enforces anything

**~513 lines of POSIX shell** — the only thing that enforces. Every refusal lives here: work exists,
evidence was recorded by the tool and is bound to the current tree, a PASS names evidence its own
author recorded, only registered agents may judge, the maker may not judge itself, byte-identical
verdicts count once, a commit voids prior verdicts, phases refuse without their artifact, closing twice
refuses.

**~250 lines of prose plus 10 role files** — instruction only. `RULES.md` labels every line CHECK or
RULE for exactly this reason. Rules have a bad record: in this project's history every written rule was
broken by its own author, usually within hours. Treat prose as guidance and the shell as the gate.

## 7. Honest verification status

| Aspect | Status |
|---|---|
| Every refusal in the gate | **Verified** by execution; the suite prints its own count |
| Full item SPEC→GRADUATE→close | **Verified**, sandbox and git mode |
| Git mode: a commit after a PASS voids the verdicts | **Verified** — a sneaked commit produced stale failures, reverting restored CLOSEABLE |
| `adopt` installs into the target repo and the program self-verifies | **Verified** |
| `agents.tsv` is genuinely gitignored in the target repo | **Verified** with `git check-ignore` |
| Two programs, different panels, one repo | **Verified** |
| Dispatch emits a runnable command with model and effort | **Verified** |
| Runs under `/bin/sh` with no `shasum` | **Verified**, and on Linux in CI |
| Concurrent `run` and `close` are safe | **Verified** — both were races, both are asserted now |
| Judges attacking the tool itself | **Verified** — five rounds, five REJECTs, every finding closed and asserted |
| **A real agent dispatched via a contract, doing real work** | **NOT verified.** In every end-to-end test I played the agents. This is the biggest gap. |
| The outer loop: claims → audit → scout → admit | **Implemented and asserted**: `claim add/verdict/scout/admit`, `triage`, and `dispatch CN <role>` with a claim-scoped recorder. Admission refuses without enough TRUE verdicts across enough kinds and a scout report. |
| **CI and merge gates** | **NOT verified.** The integrator role is prose; nothing checks a pipeline against a sha. |
| **A cold agent given only `START.md`** | **NOT verified.** |

The first row of NOT verified is the one to close before trusting any of this: run one real item with
real agents, and see whether the contracts are actually sufficient without you in the loop.

## 8. Watching a run

crucible ships no session manager and creates no session. Use whatever you already have — the
convention it assumes is one large pane you work in plus a few beside it. Then, once a program is
installed, overlay the views onto that window:

```sh
.crucible/<program>/crucible panes                      # gate, verdicts, workids, tail
.crucible/<program>/crucible panes gate memory git      # or pick your own
```

It leaves the pane you are in alone and titles it `agent`, replaces what the other panes are running
with live views, splits to create one only if there are not enough, and sets `pane-border-status` so
each is labelled. Re-run it any time to reset them, and it refuses outside tmux rather than guessing.
`BOOTSTRAP.md` tells the agent to run it right after install, before any dispatch.

The views are also directly runnable if you would rather place them yourself:

```sh
sh .crucible/<program>/scripts/watch.sh <view> [seconds]   # --once for a single frame
```

| view | what it answers |
|---|---|
| `gate` | what still refuses, and the one next thing to do |
| `verdicts` | every item and claim verdict, as its author wrote it, with kinds |
| `workids` | work id, phase and status per item — a commit changes the id and voids prior verdicts |
| `evidence` | every recorded command and its exit line |
| `tail` | the newest evidence file in full |
| `git` | branches, commits, working tree |
| `memory` | `LESSONS.md` and the claim ledger |

Each shows artifacts, never narration. **If a view and an agent disagree about whether something is
done, the view is right** — that is the entire reason they exist. Add a pane per dispatched agent
when you want to watch them work; `crucible dispatch` prints the command to paste. For a fully static
layout, `examples/tmuxinator-crucible.yml` is a starting point.
