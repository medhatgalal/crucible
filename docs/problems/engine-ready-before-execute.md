PROBLEM: Guided+managed Crucible must keep problem-bind, abandon, and
REVIEW→BUILD working after 1.5.2. Do not invent leftover RFC rows as the
next PROBLEM. Coordinator must not start ACP after seal.

Shipped in 1.5.2 — do not re-admit:

1. FILE / `--next` refuse a one-line “X is not a CLI verb” and leftover
   remainder catalogs that name no falsifiable outcome.
2. `cycle problem --abandon REASON` archives INVESTIGATE with `ABANDON.md`.
   No PASS. No new PROBLEM. Human gate.
3. Drive does not invoke the coordinator while a sealed worker exists.
4. Drive + `phase` return REVIEW→BUILD after judge `NEXT:FIX`, including
   when the judge attempt is still RETURNED inflight. A RETURNED tick is
   not success when `result` is missing or `NEXT:FIX`.
5. Drive never `--next`s. Humans file the next PROBLEM. Leftover RFC
   catalogs refuse unless they state one falsifiable outcome.

Falsifier: FILE `workgraph nosuchverb is not a CLI verb.` → refuse.
`--abandon` with no new PROBLEM → INTAKE. `phase ITEM BUILD` after judge
FIX while inflight is that RETURNED judge → BUILD. Coordinator ACP must
not start when a sealed worker exists.

Never list:
- Do not treat leftover RFC rows as the next PROBLEM unless a human files
  a real, single-outcome report.
- Do not fake PASS to leave INVESTIGATE.
- Do not start ACP from the coordinator process after seal.
- Do not implement WorkGraph remainder inside this engine item.

Later (not this item): plan-audit before maker dispatch; RED-before-PASS;
BASE..HEAD review range; hosted-job close. Do not rename roles.
