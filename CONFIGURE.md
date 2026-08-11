# CONFIGURE — agent and persona policy

Configuration is part of fresh-agent onboarding. The coordinator inspects what is available, proposes a
small panel, and asks for one compact confirmation only when a material choice remains. The operator is
not expected to edit a command matrix before describing the problem.

## Two kinds of configuration

Repository policy is durable:

- role prompts define responsibilities, read/write boundaries, return shape, and verification;
- `RULES.md` defines safeguards and which are enforced versus instructional;
- the approved proposal defines risk and therefore the required review posture.

Machine/session configuration is disposable:

- `agents.tsv` names invocable external agents and model families on this machine;
- built-in subagent/collaboration tools are registered by name/family/model with an empty command
  rather than inventing a fake CLI; the coordinator gives their generated contract through the tool;
- live processes, contexts, and task worktrees are not project knowledge.

Do not write machine invocations or project lessons into global agent memory.

## Minimal role set

Always preserve these responsibilities, even when one model class performs several in isolated contexts:

| Responsibility | Boundary |
| --- | --- |
| Coordinator | schedules, persists decisions, synthesizes; does not silently implement or judge |
| Investigator/scout | establishes current facts and existing behavior; does not propose unsupported work |
| Maker | changes only assigned files for one approved task |
| Reviewer | independently tests acceptance criteria without maker rationale |
| Adversary | used only for medium/high risk or disputed findings |

Specifier, architect, planner, and integrator are optional personas, not mandatory ceremonies. Use them
when the work actually contains that decision boundary. A small reversible fix can combine planning into
the coordinator and use one maker plus one fresh reviewer.

## Same model or different models

Role separation is primarily context and responsibility separation. The same model class is acceptable
for multiple roles when each role starts in a fresh isolated context and the result is labelled
`same-family review`. It is not independent in the strong sense because model blind spots remain correlated.

Require or strongly prefer a different model family for:

- behavioral changes with meaningful user impact;
- security, authorization, privacy, data, or migrations;
- irreversible or difficult-to-rollback operations;
- repeatedly disputed findings or two same-family review misses.

Different-family review is a risk control, not a quota. More reviewers running the same expensive suite
on unchanged work do not create more evidence.

## External agent registry

When external CLIs are used, `agents.tsv` is tab-separated:

```text
name	kind	model	effort	command
maker	codex	MODEL	high	AGENT_CLI ... "read {BRIEF} and follow it exactly"
reviewer	other-family	MODEL	high	OTHER_CLI ... "read {BRIEF} and follow it exactly"
```

`kind` means model family and is what cross-family gates count. Names describe stable responsibilities,
not temporary versions. `{BRIEF}`, `{MODEL}`, and `{EFFORT}` are substituted by the protocol.

## Risk posture

| Work | Minimum review posture |
| --- | --- |
| Docs, comments, reversible local config | one fresh reviewer; same-family is acceptable and labelled |
| Behavioral work | independent context plus a different-family reviewer when available |
| Security, data, migration, deletion, hot path | adversary and at least one different family |
| Repeated dispute | fresh isolated adjudicator from a different family or human escalation |

The coordinator records the chosen posture in the proposal or item. If the required isolation or model
family is unavailable, it escalates or obtains an explicit risk decision; it does not rename one agent to
pretend diversity.
