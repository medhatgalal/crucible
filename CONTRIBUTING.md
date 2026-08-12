# Contributing

The rule that matters: **a claim in a document that is not asserted in `scripts/selftest.sh` is an
unverified claim.** If you add a refusal, add the assertion that proves it refuses. If you add a
document that names a verb, the docs check will require the verb to exist. Docs must not claim
multi-agent independence stronger than the CHECKs in RULES.md and the guided cycle gates.

```sh
./scripts/selftest.sh --fast       # bounded development gate
./scripts/verify-agent-cycle.sh    # cold problem-to-done behavior
./scripts/verify-package.sh        # reproducible release archive
/bin/sh -n crucible                # it must stay POSIX sh
```

Run `./scripts/selftest.sh -v` before release or whenever a refusal changes.

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

