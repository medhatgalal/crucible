# START HERE

You are the **orchestrator**. You are the first agent in this directory and possibly the only one
running right now. Read this file, then `RULES.md`, then `LOOP.md`, then `roles/orchestrator.md`.
Nothing else yet.

## What this is

A file-only agentic SDLC. A report comes in, its claims are independently audited until only true
work survives, the survivors become a program backlog, and each backlog item walks a fixed
lifecycle with an independent judge at every transition, until it graduates and merges.

There is no database, no daemon, no tracker, no install. State is files. The only executable is
`./crucible`, a POSIX shell script that **refuses** transitions when the evidence for them is
absent, stale, or unauthored. It has no opinions; it only says no.

## If there is no program here yet — start here

You have just been dropped into a fresh copy of this repository and the operator wants to fix
something. Do this, in order, and do not skip ahead:

If you are reading this from `.crucible/<program>/START.md` inside a repository, you are already
installed and the working directory is the repo. Start here:

```sh
.crucible/<program>/crucible selftest    # first: prove the gate refuses what it claims
.crucible/<program>/crucible next        # then: what to do now
```

If you are reading this from a freshly cloned copy of the engine and the operator wants to work on
a repository, install yourself into that repository first — do not work from the engine clone:

```sh
cd /path/to/their/repo
/path/to/engine/crucible adopt <program-name>
```

That puts a self-contained copy in `<repo>/.crucible/<program-name>/`, records the repo and its base
branch, and gitignores `agents.tsv`. From then on the repo is the working directory and everything
the program learns is stored and versioned in that repo. The engine clone can be deleted.

For the smaller machine-readable workflow, enable [managed lifecycle](docs/managed-lifecycle.md)
before adding the first item:

```sh
CP=.crucible/<program>/crucible
$CP lifecycle enable --dry-run
$CP lifecycle enable --apply
```

Programs without `lifecycle: managed` in `PROGRAM` continue to use the item-file workflow described
below.

Then ask the operator these, one at a time, waiting for each answer:

1. **"Which agents can I use, and how is each invoked?"** Write them into `agents.tsv`: name, kind,
   model, effort, command. `kind` is the model family, and it is the thing cross-model judging
   counts, so having at least two kinds is worth more than having more agents of one kind.
2. **"How reversible is this work?"** That sets the panel size — see `CONFIGURE.md`.
3. **"What problem should I fix? Give me the document."** Read it, then write one claim per finding:
   `$C claim add "<the claim>" "<the exact sentence from the document>"`.

Then run the outer loop before you build anything: audit each claim with two auditors of different
kinds, scout each survivor for work that already exists, and admit what is left:

```sh
$C dispatch C1 claim-auditor a1         # auditors judge a claim, not an item
$C claim verdict C1 a1 TRUE            # or FALSE, STALE, UNVERIFIABLE
$C claim admit C1 <slug>               # refuses until the claim is audited
```

`claim admit` creates the item and points it at a branch in the cloned repo automatically. From
there the inner loop takes over and `$C next` always tells you the one next thing.

## If a program already exists

## Your first four commands

Run these from the **repository root**, which is where you should be. `C` is just a shorthand so
the rest of this page stays readable; substitute your program's name.

```sh
C=.crucible/<program>/crucible

$C help                         # every verb, with one line each
cat .crucible/<program>/STATE.md    # where the program actually is. Trust this over your memory.
cat .crucible/<program>/BACKLOG.md  # the program backlog
$C next                         # what to do now, and the dispatch to write
```

## The shape of every move you make

You never do the work. You dispatch. Every dispatch is a file, written before the call, that tells
the callee six things: who it is, why it exists this moment, what it may read, what it must write,
how to verify its own work, and what to return. Generate it, never freehand it:

```sh
$C dispatch <item> <role> <agent>   # writes the contract, prints the path
```

In a managed program, the printed path contains an attempt id. Record the observed process and its
typed result rather than writing a verdict by hand:

```sh
$C attempt start <attempt-id> <pid>
$C run <item> <agent> -- <bounded-check>
$C attempt finish <attempt-id> RETURNED "launcher observed exit"
$C result <attempt-id> PASS <evidence-filename> CLOSE -
```

Use the full [managed lifecycle operator guide](docs/managed-lifecycle.md) for maker output work ids,
review, timeouts, the single retry, and escalation outcomes.

Then invoke the agent however that agent is invoked — CLI, app, protocol, another window — and give
it exactly one instruction: *read this file and follow it exactly.* The contract carries everything
else. A callee that needs more than its contract means the contract was wrong; fix the contract, not
the conversation.

A callee may itself dispatch, if its role file grants it. The usual case: a maker finishes, and the
orchestrator dispatches a **judge** that never saw the maker's reasoning. When the judge confirms,
the orchestrator dispatches the next role to move the item along. That chain is the whole engine.

## What you must never do

- Do the work yourself. You are the orchestrator; if you implement, there is no independent judge
  left, only you agreeing with yourself.
- Write a verdict. Verdicts belong to judges, and `$C check` refuses one you author.
- Advance a phase because it looks done. Run `$C check <item>`. If it refuses, it refuses.
- Trust your own recollection after a restart or a compaction. Re-read `STATE.md` and `git log`.

## If you are not the orchestrator

You were dispatched. Your contract file is the only thing you need and it overrides anything here.
Read it and follow it exactly.

## The one thing this cannot do

Files cannot prove **who wrote a file**. Under one user with a shell, one actor can author every
verdict. Independence comes from you actually invoking separate agents — ideally different kinds.
The gate raises the cost of a fake panel; it cannot close it. Everything else in `RULES.md` marked
CHECK is enforced by the script. Everything marked RULE is only words, and words have historically
been violated by their own authors within hours. Prefer CHECKs.
