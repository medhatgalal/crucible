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

3. Active roles this cycle (required: coordinator, claim-auditor, maker, reviewer, contract-auditor;
   optional: scout, adversary, specifier, architect, planner, integrator)
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
contract-auditor	j2	yes	
maker	mk1	yes	
reviewer	j1	yes	≠ maker
adversary	adv	no	required when risk is HIGH
```

Show inventory + casting table and wait for `cycle approve-panel`. Approval content-binds both
`PANEL.md` and `PANEL.ASSIGN.tsv`.

### Example coordinator message

```text
I need one configure confirmation before investigation.

A) Agents — name, product, model, effort, invoke command for each.
B) Role casting — who plays coordinator, claim-auditor(s), maker, reviewer(s),
   contract-auditor, optional scout/adversary?
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

Record transport per attempt:

```sh
$CP attempt transport <ATTEMPT> multi-agent|acp|subagent
```

Record ACP probe results in `ACP-PROBE.md` (`status: ok|failed|unavailable`) or note ACP unavailable
in `PANEL.md` before using subagent transport.

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

Specifier, architect, planner, and integrator are optional personas when the work actually contains
that decision boundary.

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
