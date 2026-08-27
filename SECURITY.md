# Security

## What this is not

crucible is **not a security boundary.** Do not use its registered names, transport labels, contract
audits, or attempt ledger as access controls. The canonical boundary is listed in
[Known limits](docs/whats-new.md#known-limits).

If your workflow requires principals that cannot alter one another's records, provide that isolation
outside Crucible with separate OS identities or containers and separately controlled evidence and
verdict storage. Panel approval binds the current panel files to guided execution; it grants no file
permissions.

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

## Falsifier pair

Closure refuses unless the item's falsifier was recorded by `crucible run` at the current work id
in both directions — once failing with the mechanism removed, once passing with it restored — and
the gate reads those two files rather than running the falsifier itself.

