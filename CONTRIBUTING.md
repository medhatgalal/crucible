# Contributing

The rule that matters: **a claim in a document that is not asserted in `scripts/selftest.sh` is an
unverified claim.** If you add a refusal, add the assertion that proves it refuses. If you add a
document that names a verb, the docs check will require the verb to exist. Docs must not claim
multi-agent independence stronger than the CHECKs in RULES.md and the guided cycle gates.

```sh
./scripts/selftest.sh --fast               # bounded development gate
./scripts/verify-agent-cycle.sh            # cold problem-to-done behavior
./scripts/verify-drive.sh                  # drive, refresh, and cycle problem --next
./scripts/verify-managed-lifecycle.sh      # managed worktrees are created and torn down
./scripts/verify-attempt-ledger.sh         # every attempt is recorded
./scripts/verify-task-dag.sh               # task graph refuses cycles and dangling edges
./scripts/verify-coldstart-independence.sh # a cold start needs nothing but the repository
./scripts/verify-quickstart.sh             # the quickstart a reader is given actually works
./scripts/verify-package.sh                # reproducible release archive
/bin/sh -n crucible                        # it must stay POSIX sh
```

Run `./scripts/selftest.sh -v` before release or whenever a refusal changes.

CI runs each of those suites as a separate step on each push and each pull request — with
`selftest.sh -v`, and the `sh -n` parse over every shell file — so a failure names the
broken invariant without anyone opening the log. Until then only `selftest.sh` and
`verify-package.sh` were invoked directly and the rest ran nowhere, or only indirectly
against the packaged tree; two of them were red on `main` for as long as nobody ran them by
hand. An unrun suite rots, and a rotted suite is indistinguishable from a suite that never
asserted anything.

`scripts/verify-demand.sh` is the exception to read carefully. It is a recorded RED
contract, not a gate. Its three assertions pass on the current engine because they document
a hole: work can be admitted with no user-visible job named, and a capability catalog that
is rephrased past the literal guard binds as a problem. Do not read a green
`verify-demand.sh` as a demand gate, and do not add an assertion to it that the engine
already enforces. It also runs in the macOS job at pull-request time, which is otherwise
tags only: its pairing predicate counts matches with `grep -E`, the construct where BSD and
GNU disagree, so a tag-only run would report the divergence only after the merge that
introduced it.

Constraints that are not negotiable, because the project is worthless without them:

- **No dependencies.** POSIX shell and markdown. No package manager, no runtime, no service. If a
  change needs more than `sh` and `git`, that is a defect in the change.
- **Nothing outside the repository.** No absolute paths, no other project's name, no machine
  specifics. CI asserts this.
- **A refusal, never a warning.** Missing, empty, stale or malformed input fails closed. If losing
  information can improve an outcome, the check is inverted.
- **Label honestly.** `RULES.md` marks every line CHECK or RULE. Do not describe a rule as a check.
  A rule wearing a check's clothes is worse than a missing feature.

When you fix a bug, say in `CHANGELOG.md` what the bug let through, not just what changed. The
changelog is the record of how the gate was wrong before, which is the most useful thing in it.

