# CONFIGURE — agent and persona policy

Configuration is a hard gate on every fresh-agent guided cycle. The coordinator discovers what is
available, drafts `PANEL.md`, and waits for explicit operator approval (`cycle approve-panel`) before
investigation. Placeholder `agents.tsv` rows refuse progress.

## Two kinds of configuration

Repository policy is durable:

- role prompts define responsibilities, read/write boundaries, return shape, and verification;
- `RULES.md` defines safeguards and which are enforced versus instructional;
- the approved proposal defines risk and therefore the required review posture;
- the approved panel defines agents, kinds, isolation transport, and waivers.

Machine/session configuration is disposable:

- `agents.tsv` names invocable external agents and model families on this machine;
- ACP endpoints and host subagent ids are registered the same way (command may be empty for built-ins);
- live processes, contexts, and task worktrees are not project knowledge.

Do not write machine invocations or project lessons into global agent memory.

## Compact configure block (required once)

Ask in one message (not drip questions; do not self-answer material fields). **Both** agent
inventory and role casting are required.

### A) Agents inventory

1. Available agents/products and invocation commands
2. Kinds (model families), models, effort

### B) Role casting (personas → independent agents)

3. Active roles this cycle. Required: coordinator, claim-auditor, **scout**, maker, reviewer,
   contract-auditor. Genuinely optional: adversary, specifier, architect, planner, integrator
4. **Which registered agent plays each role**
5. Confirm coordinator is not cast as maker or reviewer

### C) Risk + isolation

6. Risk posture for this work
7. Isolation preference and ACP availability
8. Waivers (same-family, `LADDER_WAIVER: single-product`, `WAIVER: maker-reviewer-same-agent`)

Then write `PANEL.md` with:

```text
## Agents
## Roles
## Risk posture
## Isolation transport
## Independence ladder
## Waivers
```

And write `PANEL.ASSIGN.tsv` (authoritative casting; machine-checked):

```text
role	agent	required	notes
coordinator	lead	yes	this session only; never maker/judge
claim-auditor	a1	yes
claim-auditor	a2	yes
scout	sc1	yes	required on a guided cycle
contract-auditor	j2	yes
maker	mk1	yes
reviewer	j1	yes	≠ maker
adversary	adv	no	required when risk is HIGH
```

Cast `scout` in this first block. It is structurally required on a guided cycle, not optional:
`claim admit` refuses a claim with no scout report, `claim scout` refuses without a scout dispatch
(`refused: guided scout requires a scout dispatch for a1 — run: … dispatch C1 scout a1`), and that
dispatch refuses an agent that is not cast as `scout`
(`refused: agent a1 is not cast as scout in PANEL.ASSIGN.tsv`). Those two refusals are a closed
loop; the only way out is to cast a scout.

Every agent named in `PANEL.ASSIGN.tsv` needs a row in `agents.tsv`, **including the coordinator**,
even though the coordinator is this session and is never launched as a worker. Casting a coordinator
with no registry row makes `cycle approve-panel` refuse with
`PANEL.md / PANEL.ASSIGN.tsv incomplete`.

The counts in this file are the bars. `claim admit` requires one TRUE verdict per `required=yes`
`claim-auditor` row — two rows means two auditors, three means three
(`refused: C2 has 1 TRUE verdicts, need 3`) — and `close` requires one PASS per `required=yes`
`reviewer` row. `CRUCIBLE_MIN_AUDITORS` and `CRUCIBLE_MIN_JUDGES` override them.

Show inventory + casting table and wait for `cycle approve-panel`. Approval content-binds
`PANEL.md`, `PANEL.ASSIGN.tsv`, and `agents.tsv` (panel id hash). Guided dispatch, transport,
contract-audit, start, result, and claim verdict/scout refuse when that hash is stale — not only
`cycle` status.

`adopt --refresh` and `cycle problem FILE --next` keep the approved panel. **That is what "do not
recast" means:** do not swap agents when refreshing the engine or starting the next PROBLEM on the
same panel, because the recorded evidence belongs to that casting.

Correcting the casting mid-cycle is a different thing and is allowed — a role nobody cast, an agent
that turns out not to be invocable, a maker and reviewer that collapsed onto one agent. Edit
`PANEL.ASSIGN.tsv` (and `agents.tsv` if the fix is there) and run `cycle approve-panel` again. Until
you do, every guided command refuses under the stale hash:
`refused: agent panel is not current — update PANEL.md / PANEL.ASSIGN.tsv / agents.tsv and run: … cycle approve-panel`.
Tell the operator what you changed and why; approval is theirs, not yours.

### Example coordinator message

```text
I need one configure confirmation before investigation.

A) Agents — name, product, model, effort, invoke command for each.
B) Role casting — who plays coordinator, claim-auditor(s), scout, maker, reviewer(s),
   contract-auditor, optional adversary?
C) Risk + isolation — posture, multi-agent/ACP/subagent, waivers?

I will write agents.tsv + PANEL.md + PANEL.ASSIGN.tsv and stop for approve-panel.
I will not invent agents or cast myself as maker/reviewer.
```

## Independence ladder

| Priority | Transport | When | Label |
| --- | --- | --- | --- |
| 1 | multi-agent (≥2 products/CLIs) | Whenever available | strongest practical independence |
| 2 | ACP isolated sessions | Single product (e.g. Kiro only) | `ACP-ISOLATED` |
| 3 | Host subagents | Only after recorded ACP probe failure | `SUBAGENT-ISOLATED` (weaker) |
| STOP | none | Required role cannot launch | `INDEPENDENCE_UNAVAILABLE` — stop and warn |

Silent same-thread multi-hat work is never a valid transport.

Record transport per attempt (`$CP` is `.crucible/<program>/crucible`, defined at the top of
[docs/managed-lifecycle.md](docs/managed-lifecycle.md)):

```sh
$CP attempt transport <ATTEMPT> multi-agent|acp|subagent
```

Record ACP probe results with `probe-acp` (`ok|failed|unavailable`). Subagent transport needs a
recorded failure/unavailable probe, or an explicit panel line `ACP: unavailable` **only when no
prior `probe-acp ok` exists**. A successful probe cannot be overridden by panel prose.

## Contract-auditor

Every managed dispatch produces `attempts/<id>/contract.md`. The **contract-auditor** persona checks
it and records:

```sh
$CP contract-audit <ATTEMPT> <auditor-name> PASS|FIX|STOP
```

The auditor must be cast as `contract-auditor` in `PANEL.ASSIGN.tsv` and distinct from the attempt
agent. PASS is required before `result` on guided cycles. FIX → rewrite once. STOP → escalate; the
coordinator must not perform the role.

## Minimal role set

| Responsibility | Boundary |
| --- | --- |
| Coordinator | schedules, persists decisions, synthesizes; never implements or judges |
| Contract-auditor | checks dispatch contracts and independence honesty |
| Investigator/scout | establishes current facts and existing behavior |
| Maker | changes only assigned files for one approved task |
| Reviewer | independently tests acceptance criteria without maker rationale |
| Adversary | used only for medium/high risk or disputed findings |

Coordinator, claim-auditor, scout, maker, reviewer, and contract-auditor are all required on a guided
cycle. Specifier, architect, planner, and integrator are optional personas when the work actually
contains that decision boundary.

## The ACP adapter (`scripts/acp-brief.py`)

Crucible ships no ACP adapter. `scripts/acp-brief.py` is the conventional path for one the operator
writes on a single-product host; the engine preserves any file there across `adopt --refresh`
(printing `kept local adapter: scripts/acp-brief.py`) and never creates it. A fresh install has no
such file, and no Crucible check requires it.

If you use the ACP path, the adapter is the `command` column of an `agents.tsv` row. The interface it
must satisfy is that column's contract and nothing more:

- It is invoked as a shell command with `{BRIEF}` replaced by the absolute path of the dispatch
  contract, and `{MODEL}` / `{EFFORT}` replaced from the same row if the command names them.
- It must open a **fresh, isolated** agent session — that is what earns the `ACP-ISOLATED` label —
  make it read the brief at that path and follow it, and return when the session ends.
- Exit 0 when the session ran, non-zero when it could not be launched. `drive` treats a non-zero
  coordinator command as a refuse, not as an independence STOP, so a crashed adapter stops the tick
  rather than silently downgrading isolation.
- It writes nothing itself. Verdicts, evidence, and results are recorded by the session through
  `run-claim` / `result`, not by the launcher.

Nothing else in Crucible knows the file's name. Any executable satisfying that contract works, under
any path, as long as `agents.tsv` names it. `drive` runs the `agents.tsv` line itself: do not launch
the adapter by hand while drive is running.

If you have no such adapter and only one product, record the probe honestly
(`probe-acp unavailable "no adapter"`) and use the subagent rung with its weaker label — do not
describe a file that does not exist as the transport.

## Same model or different models

Role separation is context and responsibility separation. The same model class is acceptable for
multiple roles only in separate contexts/transports and must be labelled `same-family review`.

Require or strongly prefer a different model family for:

- behavioral changes with meaningful user impact;
- security, authorization, privacy, data, or migrations;
- irreversible or difficult-to-rollback operations;
- repeatedly disputed findings or two same-family review misses.

## External agent registry

When external CLIs are used, `agents.tsv` is tab-separated:

```text
name	kind	model	effort	command
maker	codex	MODEL	high	AGENT_CLI ... "read {BRIEF} and follow it exactly"
reviewer	other-family	MODEL	high	OTHER_CLI ... "read {BRIEF} and follow it exactly"
```

Replace `MODEL` / `AGENT_CLI` placeholders with real values before panel approval. `kind` means model
family and is what cross-family gates count. `{BRIEF}`, `{MODEL}`, and `{EFFORT}` are substituted by
the protocol.

## Risk posture

| Work | Minimum review posture |
| --- | --- |
| Docs, comments, reversible local config | one fresh reviewer; same-family OK if labelled; ACP preferred on single-product hosts |
| Behavioral work | independent context plus a different-family reviewer when available |
| Security, data, migration, deletion, hot path | adversary and at least one different family |
| Repeated dispute | fresh isolated adjudicator from a different family or human escalation |

The coordinator records the chosen posture in the proposal or item. If the required isolation or model
family is unavailable, it escalates or obtains an explicit risk decision; it does not rename one agent
to pretend diversity.
