# Security

## What this is not

crucible is **not a security boundary.** It runs as you, on your machine, with your shell. Under one
user, one actor can author every verdict under different registered names, read any file the gate
believes is private, and replace the gate itself. It raises the cost of faking a review panel; it does
not close it.

If you need agents that genuinely cannot forge each other's verdicts, you need separate OS identities
or containers, an evidence store owned by another principal, and a verdict service holding a write
capability. None of that is here, and `README.md` says so in the same words.

Transport labels (`multi-agent`, `acp`, `subagent`), contract audits, and attempt ledgers prove
**process discipline**, not cryptographic multi-agent identity. ACP isolation is stronger than
same-thread multi-hat work and weaker than separate OS principals.

## What it does protect against

Ordinary, non-adversarial mistakes: stale evidence, a verdict for a superseded commit, a self-review
in judge's clothing, closure with no falsifier, investigation before panel approval, guided-cycle
results without transport/contract-audit, subagent transport without a recorded ACP probe failure,
and a check that reported success while its input was missing. Those are the failures that actually
happen.

## Reporting

Open an issue. If you find a way for a non-adversarial workflow to close an item that should have
been refused, that is the most valuable report possible — include the shortest reproduction and, if
you can, the assertion that would have caught it.
