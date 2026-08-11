# Crucible

Crucible teaches a fresh coordinating agent to take one reported problem through investigation,
human-approved scope, implementation, independent review, and evidence-backed closure.

It is a self-contained POSIX shell protocol: no service, database, package manager, or permanent agent
memory. The repository keeps the work and evidence; a later agent relearns the process from files.

## Start a cycle

Open the repository you want to change and tell a fresh agent:

> Read `/absolute/path/to/crucible/BOOTSTRAP.md`. You are already in the target repository. Run a
> complete Crucible cycle for this problem: `<problem or report path>`.

The path may point to a checkout or an extracted release package. The agent installs Crucible into the
target repository, configures the available agents/personas, investigates the report, and returns one
refined proposal for approval. The operator does not drive lifecycle commands.

If a cycle is already installed, say:

> Read `.crucible/<program>/START.md` and continue this cycle.

The agent resumes from repository evidence with `crucible cycle`; it does not rely on chat memory.

## The loop

```text
onboard → investigate ⇄ challenge → proposal → human approval
                                                   ↓
             done ← integrate ← review ⇄ fix ← make ← validated plan
```

- Nothing is built before the current proposal is explicitly approved.
- False, stale, duplicated, or already-implemented findings do not become backlog work.
- Review findings return to the maker until fixed, disproved with evidence, or bounded escalation.
- `DONE` requires current evidence for every approved outcome—not an agent self-report or clean diff.

The same model may perform several roles in fresh isolated contexts; those reviews are labelled
**same-family**. Different model families are used selectively for behavioral, security, data,
migration, irreversible, or repeatedly disputed work.

## What remains after shutdown

Code, approved decisions, reviews, and evidence remain in the repository. Machine-only agent
configuration and isolated worktrees can be previewed and cleaned after `DONE`. Crucible does not write
project knowledge into global agent memory.

## Reference

- [START.md](START.md) — self-contained fresh-agent operating prompt
- [LOOP.md](LOOP.md) — lifecycle behavior and exit criteria
- [RULES.md](RULES.md) — enforced checks versus instructional rules
- [CONFIGURE.md](CONFIGURE.md) — agent/model/persona policy
- [Managed lifecycle protocol](docs/managed-lifecycle.md) — low-level agent-facing commands
- [CONTRIBUTING.md](CONTRIBUTING.md) and [RELEASE.md](RELEASE.md) — maintainer gates

Developer verification:

```sh
./scripts/selftest.sh --fast
./scripts/verify-agent-cycle.sh
./scripts/verify-package.sh
```
