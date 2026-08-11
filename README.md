# Crucible

Crucible teaches a fresh coordinating agent how to take one problem from allegation to verified,
reviewed, done work. The human describes the problem and approves the refined proposal. The agent
handles the protocol, coordination, evidence, review loops, and resumability.

There is no service, database, permanent agent, or hidden memory. Crucible is POSIX shell, role prompts,
and repository files. When an agent session ends, the work and evidence remain; the next fresh agent
relearns the process from the repository.

## Start

Stand in the repository you want to change and tell a fresh agent:

> Read Crucible's `BOOTSTRAP.md` and run a complete cycle for this problem: `<problem or path>`.

That is the operator interface. You do not need to learn the internal commands.

The agent has one durable resume behavior, `crucible cycle`; all other commands are protocol details.

If Crucible is already installed in the repository, point the agent directly at:

> Read `.crucible/<program>/START.md` and continue this cycle.

## The loop

```text
onboard/configure
       ↓
problem ⇄ investigate ⇄ falsify claims ⇄ scout existing behavior
       ↓
refined proposal ───────────────→ human approval
                                      ↓
validated breakdown → make → verify → independent review
                         ↑                  │
                         └──── fix issues ──┘
                                      ↓
                              integrate → done
```

The loop is deliberately asymmetric:

- Before approval, agents may inspect, challenge, narrow, and propose. They may not build.
- After approval, only approved bounded work enters execution.
- A review rejection goes back into the work loop with a concrete finding.
- Repeated findings, exhausted retries, unresolved product decisions, or ownership conflicts escalate
  instead of consuming unbounded tokens.
- `DONE` requires current evidence for the approved outcome and no unresolved approved work.

## Agents and personas

Crucible supports one or many agents. Roles describe responsibilities, not products:

- A coordinator schedules and persists state.
- Investigators establish facts and search for existing behavior.
- Makers change narrowly owned files.
- Reviewers independently attack the acceptance contract.
- Adversaries are reserved for higher-risk or disputed work.

The same agent class or model may fill multiple roles in fresh isolated contexts. Those reviews are
labelled **same-family**, because isolation removes conversational anchoring but not correlated model
blind spots. Different model families are used selectively for behavioral, security, data, migration,
irreversible, or repeatedly disputed work—not as ceremony on every edit.

## What is durable

The repository stores the problem, claim evidence, approved proposal, bounded items, changes, reviews,
and closure evidence. `agents.tsv`, running processes, isolated worktrees, and live agent contexts are
machine/session state. They can be cleaned after a completed cycle with an exact preview and explicit
approval. Crucible does not write project knowledge into global agent memory.

## What the gate proves

The shell refuses missing or stale work evidence, stale work ids, maker self-review, unregistered
reviewers, mutable managed results, unsupported lifecycle transitions, unapproved proposal admission,
unsafe task ownership, duplicate expensive checks on unchanged work, and unsupported closure.

It cannot cryptographically prove which human or model wrote a file under one operating-system user,
nor can it prove a test is meaningful merely because the command exited zero. Independent contexts,
discriminating falsifiers, and selective cross-family review remain operational responsibilities and
are stated honestly as such.

## Agent protocol and development

Fresh coordinating agents read [START.md](START.md). The behavioral loop is in [LOOP.md](LOOP.md),
rules and enforcement boundaries are in [RULES.md](RULES.md), and panel guidance is in
[CONFIGURE.md](CONFIGURE.md). Low-level managed-state primitives are documented in
[docs/managed-lifecycle.md](docs/managed-lifecycle.md); they are an agent protocol, not the human UX.

Developers can verify the engine with:

```sh
./scripts/selftest.sh --fast
./scripts/verify-agent-cycle.sh
```

The full suite is intentionally separate because external agent/terminal fixtures may be slow.
