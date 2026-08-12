# Changelog

All notable changes to this project are documented here. This project follows
[Keep a Changelog](https://keepachangelog.com/) and [Semantic Versioning](https://semver.org/).

## Unreleased

- **Cold-start independence contract.** Guided cycles now require a content-bound agent panel
  (`PANEL.md` + `cycle approve-panel`) and non-placeholder `agents.tsv` before problem intake or
  investigation. Success is structure followed with independent agents—not solo theatre.
- **Independence ladder.** Prefer multi-agent products/CLIs; on single-product hosts prefer ACP
  isolation; allow host subagents only after a recorded ACP probe failure; escalate
  `INDEPENDENCE_UNAVAILABLE` instead of continuing as the panel.
- **Contract-auditor.** Managed attempts record transport and a file-based
  `contract-audit ATTEMPT AUDITOR PASS|FIX|STOP` before guided-cycle results are accepted.
  Auditor must be a distinct registered agent; makers cannot audit reviews; structural contract
  checks apply. New `roles/contract-auditor.md`.
- **Independence ladder teeth.** When ≥2 model kinds are registered, guided transport must be
  `multi-agent` unless PANEL has `LADDER_WAIVER: single-product`. `probe-acp` records ACP
  availability. Guided claim TRUE requires a prior claim-auditor/scout dispatch contract.
- **Role casting configure.** Compact configure asks for agents **and** persona→independent-agent
  casting. `PANEL.ASSIGN.tsv` is required and content-bound with panel approval; guided dispatch and
  contract-audit must match the casting table.
- Rewrote public `BOOTSTRAP.md`, `START.md`, `CONFIGURE.md`, `LOOP.md`, coordinator role, and
  related docs so configure → independent dispatch is the SSOT (removed anti-interview lock-in).
- Clarified that a fresh agent may bootstrap Crucible from either the public raw `BOOTSTRAP.md` URL or
  an absolute local checkout/package path. Reworked the README around product value, the verified loop,
  multi-agent role boundaries, durable evidence, and an original lightweight project mark.

## [1.1.0] - 2026-08-11

- Simplified public usage to one fresh-agent prompt and one installed-cycle resume prompt. The
  installed `START.md` is now self-contained and keeps low-level commands behind the agent-facing
  protocol instead of making the operator learn them.
- Added deterministic `crucible-<version>.tar.gz` packaging, SHA-256 checksums, archive-content and
  cold-start verification, and Linux/BSD release-package CI gates.
- Fixed zero-argument command parsing under POSIX `dash`. The public `cycle` resume command and other
  missing-argument paths could previously exit silently on Linux because a failed `shift` terminates
  a non-interactive shell before `|| true` can recover.

- Replaced command-driven onboarding with one fresh-agent entrypoint and one durable `cycle` resume
  behavior. The coordinator now owns protocol mechanics, proposes a minimal agent/persona panel,
  interrogates the problem, obtains approval of a refined proposal, and iterates work through review
  findings until current evidence supports closure or a bounded escalation stops the run.
- Added content-bound proposal approval. A managed claim cannot become work before the operator has
  approved the current `PROPOSAL.md`, and any proposal edit invalidates that approval.
- Added behavior-named managed lifecycle selection through `crucible lifecycle`, authoritative
  `STATE.tsv`, generated `STATE.md`, a focused verifier, and an internal protocol guide. New cycles
  can select managed behavior atomically with `adopt --managed`; existing programs retain item-file
  behavior unless explicitly enabled before their first item.
- Added managed attempt records with immutable dispatch metadata, observed lifecycle events,
  evidence-bound write-once results, typed outcome/next-action pairs, explicit maker and reviewer
  deadlines, one infrastructure retry, repeated-finding stops, and unchanged-work reuse of
  canonical `FULL_SUITE` and `EXTERNAL` PASS evidence. Maker results distinguish input work from
  post-change work; review entry requires a current-work maker PASS, and managed closure rejects
  compatibility verdicts not backed by an attempt result.
- Added optional frozen `TASKS.tsv` validation and generated task views. Managed `ready` refuses
  unknown or cyclic dependencies, duplicate task IDs, unsafe or overlapping literal ownership,
  and missing or non-executable task verifiers. Dependency-ready tasks can run in isolated Git
  worktrees with a three-maker default cap, literal path ownership, frozen task verification, one
  bounded retry, and stable integration that blocks on ancestry or cherry-pick conflict. All task
  makers remain in a durable maker set, cannot review the integrated item, and reviewer results label
  same-family, cross-family, or mixed-family correlation.

## [1.0.0] - 2026-08-07

Initial release.

### What it is

A file-only refusal gate for agentic software work. One agent orchestrates, others make and judge,
and nothing closes until the recorded evidence says it may. POSIX shell and markdown; nothing to
install beyond `git` and a shell.

### The loop

An outer loop turns a problem document into a backlog: one claim per finding quoting its source,
audited by two or more agents of different kinds, scouted for work that already exists, then
triaged with the operator before anything is admitted. An inner loop walks each admitted item
through SPEC, DESIGN, TASKS, BUILD, VERIFY, ADVERSARY and GRADUATE, with a contract generated for
every dispatch and a lesson recorded at close that binds every later maker.

### What is enforced

Each of these refuses, and each is asserted in `scripts/selftest.sh` by a test that fails when the
mechanism is removed:

- Evidence is recorded by the tool, never written by an agent, and carries the work id in its
  filename and inside the file, so renaming stale evidence is refused.
- A PASS must name an evidence file its own author recorded.
- Only agents registered in `agents.tsv` may judge, and the maker may not judge its own work.
- A judge's brief is built from a whitelist and contains no maker report, transcript or rationale.
- A commit after a verdict voids that verdict; the work id is the branch tree.
- Absence fails: no work, no evidence, no falsifier, no verdict, no scout report — all refuse.
- A claim cannot become an item without enough TRUE verdicts across enough distinct kinds.
- Phases refuse without their artifacts, even when the phase is hand-edited.
- Closing twice refuses, and work changing between check and close refuses.
- Concurrent recording and closing are safe; an unknown verb refuses rather than succeeding.

### What is not enforced

The gate cannot prove who wrote a file. Under one user with a shell, one actor can author every
verdict under different registered names, and this has happened in practice during development.
Independence is a property of how you dispatch, not something a file-only design can establish.
`RULES.md` labels every line CHECK or RULE so it is always clear which one you are relying on, and
`SECURITY.md` states the boundary plainly.

### Verification status

The suite runs on Linux for every push and on BSD for tags, and reports its own assertion count.
Two things are not yet demonstrated and are named rather than implied: no agent has followed the
documented bootstrap prompt end to end in a clean environment without help, and the loop has not
yet been used to build a feature — it was validated on this repository's own release check.
