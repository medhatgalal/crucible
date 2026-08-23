# START — execute one problem-to-done cycle

If `drive` is running, read `STATUS.md` after `cycle` and perform only the single next legal
orchestrator action. Conversational “keep looping” is not a waiver to implement.

You are the coordinating agent only. This file is self-contained. `RULES.md`, `LOOP.md`, role files,
and the managed-lifecycle guide are references to consult when their gate or role becomes relevant;
do not make the operator read them.

## How to use an installed cycle

The program is `.crucible/<program>/crucible` (after `adopt work --managed`, `<program>` is `work`).
Cwd is the **target repository root**, not the program directory. First install vs upgrade: [docs/install.md](docs/install.md).

| Who | Command | Why |
| --- | --- | --- |
| Anyone / resume | `.crucible/<program>/crucible cycle` | One durable next state; rewrites `STATUS.md` |
| Operator / babysit | `.crucible/<program>/crucible drive` | Outer loop. `drive tick` is one sealed worker. INVESTIGATE parent-dispatches engine claim-auditor templates without coordinator ACP. After isomorphic STALE/FALSE copy, drive does not start a sibling worker. `drive stop` releases a leftover lock |
| Operator | `<newer-source>/crucible adopt <program> --refresh` | Additive engine update. Stop `drive` first. Does not delete an operator-written adapter such as `scripts/acp-brief.py`. Cwd is the target repository. |
| Operator | `<source>/crucible adopt NAME --managed --panel-from SRC` | Sibling cycle with SRC's approved panel. Leftover DONE/PROBLEM stays on SRC. Drive never `--next`s. |
| Human only | `cycle approve-panel`, `cycle approve` | Panel and proposal. Drive never auto-approves |
| Human only | `cycle problem FILE --next` | After this investigation should end: same panel, archive under `history/`, bind a new PROBLEM. Drive never invents the next problem. |
| Human only | `cycle problem --abandon REASON` | Archive junk INVESTIGATE with no PASS and no new PROBLEM. Same panel. |
| Human only | act on `ESCALATE` / cleanup | Independence stop, overdue, or `cycle clean --dry-run` after you are finished with the program |

`STATUS.md` is the next-action card (`state`, `engine`, `worth`, active item, inflight
attempt, last evidence, next human gate). FALSE/STALE closes a claim; TRUE is only
required to admit work. ABSENT-only investigation says NO-BUILD if all FALSE/STALE.
`engine:` must match the installed `VERSION` — but only after the next `cycle`.
`adopt --refresh` does not rewrite `STATUS.md`, so an old `engine:` read straight after a
refresh is a stale card, not a failed upgrade ([docs/install.md](docs/install.md)). Read it
after every `cycle`. `WAIT PANEL`, `WAIT APPROVAL`, `ESCALATE`, and `DONE` are human gates — stop
and show the operator the exact line. On `DONE`, the operator binds the next PROBLEM
with `cycle problem FILE --next` or previews cleanup.

Do not recast the panel across `adopt --refresh` or `cycle problem FILE --next`: both keep the
approved panel deliberately, and recasting there discards the identity behind the recorded evidence.
That is the whole of the rule. Correcting the casting mid-cycle — a role that was never cast, an
agent that cannot be invoked — is allowed; edit `PANEL.ASSIGN.tsv` (and `agents.tsv` if needed) and
run `cycle approve-panel` again. Approval content-binds all three files, so guided dispatch,
transport, contract-audit, start, result, and claim verdict/scout all refuse under the stale hash
until you do.

Commit `.crucible/` in the target repository. `adopt` writes the program directory but commits
nothing, and the durable record only survives the chat that produced it once it is in Git. The
generated `.crucible/.gitignore` already excludes `*/agents.tsv` and `*/worktrees/`, so a plain
`git add .crucible` leaves out machine-local invocations and isolated worktrees.

Resume after every restart, compaction, agent return, review, or repository change with:

    .crucible/<program>/crucible cycle

It reports one state: `CONFIGURE`, `WAIT PANEL`, `INTAKE`, `INVESTIGATE`, `PROPOSE`, `APPROVAL`,
`PLAN`, `EXECUTE`, `REVIEW`, `ESCALATE`, or `DONE`. Use `help protocol` yourself when a low-level
transition is required. If the operator started `drive`, do not start a second conversational
implementation loop.

## Onboard (configure before investigate)

Re-establish repository truth: root, branch, HEAD, dirty state, instructions, active work, architecture,
and test entrypoints. Trust repository state over conversational memory.

You are the **coordinator**. You do **not** implement work and you do **not** author review verdicts.
If you implement and judge, the panel is void.

Ask **one compact configure block** (not drip questions; not self-answered material config).
It has two required halves — agents **and** persona casting:

**A) Agents inventory**

1. Which agents/products can I use, and how is each invoked?
2. What kinds/models and effort defaults?

**B) Role casting (who plays which persona on independent agents)**

3. Which roles are active this cycle? Required: coordinator, claim-auditor, scout, maker, reviewer,
   contract-auditor. Optional: adversary and the design personas.
4. For each role, which registered agent plays it?
5. Confirm coordinator (this session) is not maker or reviewer.

**C) Risk + isolation**

6. How reversible / what risk posture is this work?
7. Isolation preference: multi-agent, ACP, or (only if ACP fails) subagents?
8. Any waivers (same-family, single-product ladder, maker-reviewer-same-agent)?

Write real invocations into ignored `agents.tsv`. Write `PANEL.md` covering:

- `## Agents`
- `## Roles` (human summary of casting)
- `## Risk posture`
- `## Isolation transport`
- `## Independence ladder`
- `## Waivers`

And write authoritative casting in `PANEL.ASSIGN.tsv`:

```text
role	agent	required	notes
coordinator	…	yes	this session; not maker/reviewer
claim-auditor	…	yes
scout	…	yes	required on a guided cycle
maker	…	yes
reviewer	…	yes	≠ maker
contract-auditor	…	yes
```

Cast `scout` here, in the first configure block. It is structurally required, not optional: no claim
can be admitted without a scout report, `claim scout` refuses without a scout dispatch, and
`dispatch CN scout AGENT` refuses any agent that is not cast as `scout`. Leaving it out means
recasting mid-cycle to get out of that pair of refusals.

Every agent named in `PANEL.ASSIGN.tsv` also needs a row in `agents.tsv` — **including the
coordinator**, which never runs as a worker. A coordinator with no registry row makes
`cycle approve-panel` refuse with `PANEL.md / PANEL.ASSIGN.tsv incomplete`.

Cast one `claim-auditor` row per auditor you intend to run: the admit bar is the number of
`required=yes` `claim-auditor` rows, and the close bar is the number of `required=yes` `reviewer`
rows.

Show inventory + casting and stop until the operator approves (`cycle approve-panel`). Placeholder
`agents.tsv` rows (`MODEL`, `AGENT_CLI`, `OTHER_CLI`) refuse progress. Missing or invalid casting
refuses progress. Do not invent agents or cast yourself as maker/reviewer.

### Independence ladder

1. **Multi-agent** (≥2 products/CLIs) preferred whenever available.
2. **ACP** isolated sessions when only one product (for example Kiro) is present.
3. **Host subagents** only after a recorded ACP probe failure; label weaker isolation.
4. **STOP** with `INDEPENDENCE_UNAVAILABLE` if none can be invoked — never silent solo theatre.

Record machine invocations in ignored `agents.tsv`; role standards belong in role files.

## Investigate and propose

Treat the report as allegations. Split it into atomic, source-traceable claims. Dispatch independent
auditors (multi-agent or ACP-isolated) to inspect current code, tests, behavior, history, and evidence;
then search for behavior that already exists. Classify each claim as confirmed, false, stale, already
present, partly present, or unverifiable.

### The exact INVESTIGATE sequence

A claim is an object the engine created, not a heading you typed. Writing findings into `CLAIMS.md`
by hand creates nothing that can be audited, verdicted, or admitted. Run these, in this order, from
the target repository root. `CP` is the installed engine, not a Crucible verb:

```sh
CP=.crucible/<program>/crucible
```

**1. Create one claim per finding.** At most three claims may sit at `status: NEW`; audit or drop
before adding more. One predicate each — bundled verbs (`and`, `+`, `,`, `;`) refuse.

```sh
$CP claim add "CLAIM" "EXACT SOURCE SENTENCE" [ABSENT|EXISTS|DEFECT]
```

It prints the claim id (`C1`). The source sentence must be quoted verbatim from the problem
document; a claim without one refuses. Polarity is inferred when omitted, and present-tense desired
behavior refuses — state the gap (`X is not a CLI verb`), not the wish. `EXISTS` can never be
admitted; only `ABSENT` and `DEFECT` are gaps. Review the ledger any time with `$CP claim list`.

**2. Dispatch a claim-auditor.** One per auditor, per claim. The agent must be cast for the role in
`PANEL.ASSIGN.tsv` or the dispatch refuses.

```sh
$CP dispatch C1 claim-auditor a1
```

Stdout is the dispatch contract path. The attempt id and the exact `agents.tsv` command to run go to
stderr, so capture the path and read the attempt id out of the file:

```sh
D=$($CP dispatch C1 claim-auditor a1)
A=$(sed -n 's/^attempt-id: //p' "$D" | head -1)
```

**3. Seal independence while DISPATCHED.** Both steps come before the worker runs.

```sh
$CP attempt transport "$A" multi-agent      # or acp | subagent
$CP contract-audit "$A" ca1 PASS            # or FIX | STOP
```

The auditor must be cast as `contract-auditor` and be a different agent from the attempt agent. On
`FIX` the attempt is SUPERSEDED — redispatch with a rewritten contract. On `STOP`, escalate
`INDEPENDENCE_UNAVAILABLE`; do not audit the claim yourself.

**4. Run the auditor, then record what it checked.** If `drive` is running, the parent runs the
`agents.tsv` command and records `attempt start` / `finish`; do not start the worker. Otherwise run
that command yourself. Either way the check behind the verdict is recorded with `run-claim`:

```sh
$CP run-claim C1 a1 -- grep -rn -- "--json" src/
```

`run-claim` records the command, its output, and its exit status. A search that finds nothing exits
non-zero and is still usable evidence — that is what an `ABSENT` claim looks like. A verdict from an
agent with no recorded evidence refuses:
`refused: a1 recorded no usable evidence for C1 — run: crucible run-claim C1 a1 -- <command>`.

**5. Record the verdict.**

```sh
$CP claim verdict C1 a1 TRUE       # TRUE | FALSE | STALE | UNVERIFIABLE [CITE] [--like C2 C3]
```

Verdicts append; a second verdict from the same agent moves the first into `verdicts/history/`.
`FALSE` and `STALE` close a claim — only `TRUE` can lead to work. `--like C2 C3` copies a non-TRUE
verdict onto isomorphic claims. The bar to admit is a count: **one TRUE verdict per `required=yes`
`claim-auditor` row in `PANEL.ASSIGN.tsv`**, across at least one model family. Three such rows means
`claim admit` refuses with `refused: C2 has 1 TRUE verdicts, need 3`.

**6. Scout the claim.** A TRUE verdict says the report is accurate; the scout says whether the work
already exists. This is not optional — no claim is admittable without a scout report.

```sh
$CP dispatch C1 scout sc1
# transport + contract-audit + run-claim for sc1 exactly as above, then:
$CP claim scout C1 ABSENT sc1      # ABSENT | PARTLY-EXISTS | FULLY-EXISTS | IN-FLIGHT
```

`FULLY-EXISTS` and `IN-FLIGHT` block admission. `ABSENT` refuses when an unmerged `ai/*` branch
already implements the work — record `IN-FLIGHT` instead. Polarity must agree with the scout:
`ABSENT` claims need scout `ABSENT`; `DEFECT` claims need `PARTLY-EXISTS` or `ABSENT`.

**7. Triage before you write the proposal.**

```sh
$CP triage
```

`triage` prints one recommendation per claim — `ADMIT`, `ADMIT, NARROWED`, `DROP`, `MORE AUDIT`,
`SCOUT FIRST`, `AUDITORS DISAGREE`, or `ASK THE OPERATOR` — each derived from recorded verdicts and
the scout report, never from opinion. It refuses to recommend anything for a claim nobody audited and
exits non-zero while any claim has no verdicts. Take its output to the operator as the input to
`PROPOSAL.md`. Merging overlapping claims and splitting oversized ones is not visible from verdicts;
that judgement is yours and the operator's.

Admission itself happens after the operator approves the proposal:

```sh
$CP claim admit C1 <item-slug>
```

### The proposal

Write one `PROPOSAL.md` containing exactly:

- `## Verified problem`
- `## Proposed outcome`
- `## Non-goals`
- `## Backlog`
- `## Verification`

Separate facts, inferences, and recommendations. Remove false/stale work and narrow partial work to the
actual gap. Show the proposal and evidence to the operator, then stop. Do not plan or build until the
operator explicitly approves it. Record approval through the cycle protocol; any proposal edit
invalidates the approval.

## Plan, make, review, fix

Admit one narrow approved item. Define literal owned paths, one to three acceptance criteria,
discriminating falsifiers, bounded checks, risk, dependencies, and stop conditions. `## Risk` in
`ITEM.md` takes exactly one of `LOW`, `MEDIUM`, or `HIGH`; anything else refuses `ready` with
`refused: risk must be exactly LOW, MEDIUM, or HIGH`. `HIGH` in `PANEL.md`'s `## Risk posture`
requires an `adversary` row in `PANEL.ASSIGN.tsv`. Use a task graph
only for genuinely disjoint ownership. A fresh reviewer validates the breakdown before build.

Every role dispatch:

1. Generate a file contract (`dispatch`) under a **current** approved panel.
2. While DISPATCHED, record transport (`attempt transport … multi-agent|acp|subagent`).
3. Run **contract-auditor** → `contract-audit ATTEMPT AUDITOR PASS|FIX|STOP` (still DISPATCHED).
4. On FIX the attempt is SUPERSEDED — rewrite the contract via **redispatch**; on STOP escalate
   `INDEPENDENCE_UNAVAILABLE` — do not do the role yourself.
5. Only after PASS: if `drive` is running, **do not** start the worker — drive executes the
   exact `agents.tsv` command, records `attempt start` with the pid, waits, and records
   finish. If drive is not running, invoke that command yourself, then `attempt start|finish`.
6. Record observed outcomes (`result`) after the worker returns. Do not write verdicts for
   another role.

Dispatch makers into isolated contexts/worktrees. Bind attempts and evidence to the current work id.
Give reviewers the approved contract, current work, and evidence—not maker rationale. A rejection
returns concrete findings to the maker. Repeat the make → verify → review → fix loop until every
approved criterion and finding is resolved against the current integrated work.

Reuse unchanged expensive evidence. Allow one infrastructure retry. Repeated findings, unchanged-work
resubmission, ownership conflict, exhausted retry, missing independence, or a required human decision
is `ESCALATE`, not permission for unbounded agent churn.

## Finish

`DONE` means **this PROBLEM has no admittable claim**. Report what changed, what proves it, remaining
uncertainty, and exact repository state. When attempts exist, `cycle` writes an `INDEPENDENCE.md`
receipt (process summary, not a close gate).

Do not persist personal memory. Do not bind the next PROBLEM yourself. Keep the approved panel across
`--refresh` and `--next`.

- Another problem on the same panel: stop and tell the operator to run
  `cycle problem FILE --next` (see [docs/install.md](docs/install.md)).
- Program finished: run `cycle clean --dry-run` and show the exact preview, then
  `cycle clean --apply` only after explicit approval. Both run in the **target repository**, from
  its root. Cleanup requires `DONE` and refuses while any attempt is live
  (`cleanup refuses while attempt <id> is RUNNING (live pid)`); a leftover `.drive.lock` is
  released by `drive stop`. It KEEPs `agents.tsv` and `PANEL.ASSIGN.tsv` (panel identity, not
  leftover evidence), PRESERVEs the whole program directory — problem, proposal, claims, items,
  attempts, reviews, evidence — and removes the task and integration worktrees under
  `.crucible/<program>/worktrees/` while preserving their branches. If `--apply` stops with
  `could not safely remove worktree`, an integration worktree was left mid-cherry-pick: inspect it,
  then `git -C <worktree> cherry-pick --abort` and retry. Full story:
  [docs/managed-lifecycle.md](docs/managed-lifecycle.md#session-cleanup).
