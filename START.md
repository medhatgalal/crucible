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
| Operator / babysit | `.crucible/<program>/crucible drive` | Outer loop. `drive tick` is one sealed worker. INVESTIGATE parent-dispatches engine claim-auditor templates without coordinator ACP. After isomorphic STALE/FALSE copy, drive does not start a sibling worker. `drive stop` releases a leftover `.crucible/<program>/.drive.lock` |
| Operator | `<newer-source>/crucible adopt <program> --refresh` | Additive engine update. Stop `drive` first. Does not delete an operator-written adapter at `.crucible/<program>/scripts/acp-brief.py`. Cwd is the target repository. |
| Operator | `<source>/crucible adopt NAME --managed --panel-from SRC` | Sibling cycle with SRC's approved panel. Leftover DONE/PROBLEM stays on SRC. Drive never `--next`s. |
| Human only | `cycle approve-panel`, `cycle approve` | Panel and proposal. Drive never auto-approves |
| Human only | `cycle problem FILE --next` | After this investigation should end: same panel, archive under `history/`, bind a new PROBLEM. Drive never invents the next problem. |
| Human only | `cycle problem --abandon REASON` | Archive junk INVESTIGATE with no PASS and no new PROBLEM. Same panel. |
| Human only | act on `ESCALATE` / cleanup | Independence stop, overdue, or `cycle clean --dry-run` after you are finished with the program |

For release-specific changes and current operator-visible limits, see
[docs/whats-new.md](docs/whats-new.md).

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
claim-auditor	…	yes	dedicated claim verifier
claim-auditor	…	yes
scout	…	yes	required on a guided cycle
maker	…	yes
reviewer	…	yes	≠ maker
contract-auditor	…	yes
```

The template uses two dedicated claim-auditors, but one required claim-auditor row can progress when
the cast scout independently records the second sealed TRUE. See [the admit bar](#the-admit-bar).

Cast `scout` here, in the first configure block. It is structurally required, not optional: no claim
can be admitted without a scout report, `claim scout` refuses without a scout dispatch, and
`dispatch CN scout AGENT` refuses any agent that is not cast as `scout`. Leaving it out means
recasting mid-cycle to get out of that pair of refusals.

### The admit bar

A claim needs **`max(2, required=yes claim-auditor rows)` sealed TRUE verdicts from distinct
registered agents**, across at least `CRUCIBLE_MIN_KINDS` model families (default 1). A TRUE from a
sealed claim-auditor or scout attempt is eligible; the required claim-auditor row count sets the
threshold, not the eligible role. With one required claim-auditor row, one claim-auditor TRUE plus one
scout TRUE satisfies the default floor.

One row and only one eligible TRUE remains below the bar: `triage` prints
`MORE AUDIT — 1 TRUE across 1 kind(s); need 2 across 1.` and `claim admit` refuses with
`refused: C1 has 1 TRUE verdicts, need 2`. Three required claim-auditor rows raise the bar to three,
and on two eligible TRUE verdicts `triage` prints
`MORE AUDIT — 2 TRUE across 2 kind(s); need 3 across 1.` and `claim admit` refuses with
`refused: C1 has 2 TRUE verdicts, need 3`; `cycle approve` refuses with
`refused: investigation is incomplete`.

`cycle` prints the number only when at least one claim is not polarity `ABSENT`. The
[INVESTIGATE sequence](#the-exact-investigate-sequence) below infers polarity `ABSENT` from its own
claim wording, so on that path `cycle` prints no number at all:

```text
NEXT INVESTIGATE — independently fact-check every unresolved ABSENT claim (NO-BUILD if all FALSE/STALE)
```

The numbered sentence is reachable, and it is what `cycle` prints as soon as one recorded claim has
polarity `DEFECT` or `EXISTS`:

```text
NEXT INVESTIGATE — independently fact-check every unresolved claim (FALSE/STALE closes a claim; admit needs 2 sealed TRUE to create work)
```

At three `required=yes` `claim-auditor` rows the same sentence reads
`admit needs 3 sealed TRUE to create work`.

A missing or malformed `PANEL.ASSIGN.tsv` does not lower the bar to zero. It stays at 2, and
`cycle` separately refuses to proceed with `NEXT CONFIGURE` until the casting is fixed.

A `triage` `ADMIT` does not guarantee that `claim admit` will accept the claim. Keep each TRUE
agent's `run-claim` evidence, keep the panel current, and use transport valid under the current
independence ladder. `subagent` transport requires a recorded ACP-probe failure. Recovery examples
are in [step 7](#the-exact-investigate-sequence).

The close bar has no floor: `close` requires one PASS per `required=yes` `reviewer` row, and one
row closes on one PASS.

| Variable | Default | What it overrides |
| --- | --- | --- |
| `CRUCIBLE_MIN_AUDITORS` | unset — the rule above applies | The admit bar at every gate; `=1` admits on a single TRUE |
| `CRUCIBLE_MIN_JUDGES` | 2, but the `required=yes` `reviewer` row count wins on a guided cycle | The close bar |
| `CRUCIBLE_MIN_KINDS` | 1 | The model-family spread; `=1` means two same-family TRUEs admit |

Setting `CRUCIBLE_MIN_AUDITORS=1` makes a one-auditor panel admit, and it is a real weakening of the
gate rather than a workaround. Cast the second auditor instead.

Every agent named in `PANEL.ASSIGN.tsv` also needs a row in `agents.tsv` — **including the
coordinator**, which never runs as a worker. A coordinator with no registry row makes
`cycle approve-panel` refuse with `PANEL.md / PANEL.ASSIGN.tsv incomplete`.

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
`PANEL.ASSIGN.tsv` or the dispatch refuses. Run this form and no other — stdout is the dispatch
contract path, the attempt id and the exact `agents.tsv` command go to stderr, so capture the path
and read the attempt id out of the file:

```sh
D=$($CP dispatch C1 claim-auditor a1)
A=$(sed -n 's/^attempt-id: //p' "$D" | head -1)
```

`dispatch` is not idempotent. Running it twice for the same claim and agent writes a second
dispatch file (`2-claim-auditor-a1.md`) and a second attempt, and only the one you sealed is sealed.
`claim verdict` accepts the last **sealed** dispatch, so the verdict records normally; the admit bar
resolves that agent's **earliest** claim dispatch, so an unsealed stray sitting in front of the
sealed one keeps that agent's TRUE verdict off the count. `triage` reports that and names the attempt
in the way. On a three-row panel with three TRUE verdicts on file and `a3`'s earliest dispatch
unsealed:

```text
INDEPENDENCE INCOMPLETE — 3 TRUE on file, 2 counted across 2 kind(s); need 3 across 1. a3 resolves to attempt <id>, which has no transport — run: .crucible/<program>/crucible attempt transport <id> <multi-agent|acp|subagent> while DISPATCHED
```

`cycle` names nothing here; it keeps printing its `NEXT INVESTIGATE` line. `claim admit` refuses on
the same attempt with
`refused: attempt <id> has no transport (multi-agent|acp|subagent) — run: .crucible/<program>/crucible attempt transport <id> <transport> while DISPATCHED`.
`triage` is the surface that names the agent and the attempt, so read the id out of it.

An attempt that was never started is ended with `attempt finish <id> ABANDONED "<what you
observed>"`, and the engine names that recovery inside the refusal you hit — run the command it
prints. `attempt reclaim` looks like the verb for this and is not; it refuses and redirects:

```text
attempt reclaim requires RUNNING or OVERDUE: <id> never started, so there is no pid to
reclaim — end it with: .crucible/<program>/crucible attempt finish <id> ABANDONED "<what you observed>"
```

At **item** level the ABANDONED finish clears the block. A stray item dispatch refuses both a second
`dispatch` (`refused: item already has in-flight attempt <id>`) and the phase transition, and the
phase refusal prints the same recovery:

```text
refused: attempt <id> is DISPATCHED and still in flight — it never started, so end it with:
.crucible/<program>/crucible attempt finish <id> ABANDONED "<what you observed>"
```

Running it answers `<id> ABANDONED; never started, in-flight pointer released for <slug>`, and a
redispatch proceeds. `contract-audit <id> <contract-auditor> FIX` releases the same pointer by
superseding the contract, and is the right verb when the contract itself was wrong rather than
duplicated. Both work only while the attempt is DISPATCHED; after `attempt start`, `contract-audit`
refuses with `refused: contract-audit may only be recorded while DISPATCHED (before attempt start)`.

At **claim** level it does not restore the verdict. `attempt finish <stray> ABANDONED "<reason>"`
answers `<id> ABANDONED; claim attempt has no item state` — the abandonment is recorded, and the
admit bar goes on ignoring that agent's TRUE because the earliest dispatch file still points at the
abandoned attempt. There is no verb that undoes that today, and abandonment makes the recovery
`triage` prints stale rather than wrong: `triage` still reports `INDEPENDENCE INCOMPLETE` naming that
same attempt and the `attempt transport` command, and that command now refuses with
`refused: transport may only be recorded while DISPATCHED (before attempt start)`. Do not seal an
attempt nobody launched to get past it: a recorded transport and contract-audit PASS for an unrun
attempt is the exact dishonesty the seal exists to prevent. Record the abandonment, tell the operator
which claim and agent are affected, and dispatch a different cast auditor to make up the count.

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
verdict onto isomorphic claims. Repeat steps 2 to 5 until the claim has enough distinct eligible
TRUE verdicts. A sealed scout attempt can also supply a TRUE; required claim-auditor rows set the
threshold but do not restrict eligible TRUE verdicts to that role. Full rule and the three overriding
variables: [The admit bar](#the-admit-bar).

**6. Scout the claim.** A TRUE verdict says the report is accurate; the scout says whether the work
already exists. This is not optional — no claim is admittable without a scout report.

```sh
DS=$($CP dispatch C1 scout sc1)
AS=$(sed -n 's/^attempt-id: //p' "$DS" | head -1)
$CP attempt transport "$AS" multi-agent
$CP contract-audit "$AS" ca1 PASS
$CP run-claim C1 sc1 -- <search command>
$CP claim verdict C1 sc1 TRUE       # when the scout independently verifies the claim too
$CP claim scout C1 ABSENT sc1      # ABSENT | PARTLY-EXISTS | FULLY-EXISTS | IN-FLIGHT
```

The scout's TRUE is eligible for the admit floor and its scout result remains separately required.
Without transport **and** a contract-audit PASS, `claim scout` refuses with
`refused: guided scout requires a matching scout attempt (item+agent+role) on the independence
ledger for sc1`, and `claim verdict` refuses the same way for an auditor
(`refused: guided claim verdict requires a matching claim attempt (item+agent+role) on the
independence ledger for a1`).

`FULLY-EXISTS` and `IN-FLIGHT` block admission. `ABSENT` refuses when an unmerged `ai/*` branch
already implements the work — record `IN-FLIGHT` instead. Polarity must agree with the scout:
`ABSENT` claims need scout `ABSENT`; `DEFECT` claims need `PARTLY-EXISTS` or `ABSENT`.

**7. Triage before you write the proposal.**

```sh
$CP triage
```

`triage` prints one recommendation per claim — `ADMIT`, `ADMIT, NARROWED`, `DROP`, `MORE AUDIT`,
`INDEPENDENCE INCOMPLETE`, `SCOUT FIRST`, `AUDITORS DISAGREE`, or `ASK THE OPERATOR` — each derived
from recorded verdicts and the scout report, never from opinion. It refuses to recommend anything for
a claim nobody audited and exits non-zero while any claim has no verdicts. Take its output to the
operator as the input to `PROPOSAL.md`. Merging overlapping claims and splitting oversized ones is
not visible from verdicts; that judgement is yours and the operator's.

`INDEPENDENCE INCOMPLETE` is the disposition for TRUE verdict files that do not count. `triage`
counts the set `claim admit` counts — sealed TRUE verdicts from registered agents that still resolve
to an independent attempt — so when the raw count of TRUE files clears the bar and the counted set
does not, it reports the shortfall with the blocker and the command instead of recommending `ADMIT`:

```text
INDEPENDENCE INCOMPLETE — 3 TRUE on file, 2 counted across 2 kind(s); need 3 across 1. a3 resolves to attempt <id>, which has no transport — run: .crucible/<program>/crucible attempt transport <id> <multi-agent|acp|subagent> while DISPATCHED
```

It fires for a panel that is no longer current too, and there it names the panel instead:

```text
INDEPENDENCE INCOMPLETE — 3 TRUE on file, 0 counted across 0 kind(s); need 3 across 1. the agent panel is not current — run: .crucible/<program>/crucible cycle approve-panel
```

A `triage` `ADMIT` is a recommendation, not an admission. Before `claim admit`, keep the panel
current and satisfy these requirements for each eligible TRUE:

- The transport ladder, re-checked against the current panel.
- The recorded ACP-probe failure that `subagent` transport requires.

Each eligible TRUE needs at least one evidence file written by `run-claim`; the engine recognises it
by its `crucible-run/1` header. If that file is removed, `cycle`, `triage`, and `claim admit` no longer
accept that TRUE. On a two-row panel with `C1` audited TRUE by `a1` and `a2` and `a2`'s evidence file
removed, `triage` reports the shortfall and names the command that would close it:

```text
INDEPENDENCE INCOMPLETE — 2 TRUE on file, 1 counted across 1 kind(s); need 2 across 1. a2 recorded no usable evidence for C1 — run: .crucible/<program>/crucible run-claim C1 a2 -- <command>.
```

`cycle` names nothing in that state either; it stays on its `NEXT INVESTIGATE` line. `claim admit`
notes the verdict it declined to count and then refuses on the count:

```text
ignoring a2: no usable evidence for C1
refused: C1 has 1 TRUE verdicts, need 2
```

Both lines go to stderr and stdout stays empty, so read stderr rather than only the exit status.
Recording the missing check puts the verdict back on the count and returns `triage` to `ADMIT`.

**A `subagent` seal the probe has overtaken is terminal for the claim.** One state is reachable
through engine verbs alone. An attempt sealed on `subagent` while the ACP probe read `failed` was
honest when it was sealed. When ACP comes back and the operator records `probe-acp ok`, the ladder
stops accepting that seal, the TRUE verdict behind it stops counting, and `triage` says so:

```text
INDEPENDENCE INCOMPLETE — 2 TRUE on file, 1 counted across 1 kind(s); need 2 across 1. a2 resolves to attempt <id>, sealed on subagent while the ACP probe read failed; the probe now reads ok, so the ladder no longer accepts that seal. Terminal — the probe refuses a downgrade, transport is only recordable while DISPATCHED, and a redispatch resolves to the same earliest attempt. claim admit refuses on this verdict too, so a further auditor does not unblock it: C1 cannot be admitted while a2 reads TRUE. Re-file the finding as a new claim, or abandon the investigation.
```

`claim admit` refuses there on the ladder, in the same terms `attempt transport <id> subagent` uses
when no probe failure is on record:

```text
refused: subagent requires a recorded ACP probe failure (ACP-PROBE.md status: failed, or PANEL notes ACP unavailable)
```

Each exit that disposition rules out was measured on that state:

- The probe is one-way. After an `ok`, `probe-acp failed` refuses with
  `refused: ACP probe already ok; cannot downgrade to failed to unlock weaker isolation (record PANEL ACP-unavailable if needed)`,
  and `probe-acp unavailable` refuses in the same terms.
- The seal cannot be rewritten. Recording the verdict terminalised the attempt, and
  `attempt transport <id> acp` on it refuses with
  `refused: transport may only be recorded while DISPATCHED (before attempt start)`.
Re-file the finding as a new claim and audit it on honest seals, or abandon the investigation; either
way tell the operator which claim and which agent are affected.

Admission itself happens after the operator approves the proposal:

```sh
$CP claim admit C1 <item-slug>
```

### The proposal

Write `.crucible/<program>/PROPOSAL.md` — the engine reads that exact path — containing exactly:

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

### The exact EXECUTE sequence

Two gates sit between `ready` and a maker `result` and neither is discoverable from the earlier
steps: an independent `plan-audit PASS`, and a work branch that already exists. Run these in this
order, from the target repository root.

**1. Freeze the contract and get the plan audited.**

```sh
$CP ready <slug>
$CP plan-audit <slug> <auditor> PASS      # PASS | FIX | STOP
$CP phase <slug> BUILD
```

`plan-audit SLUG AUDITOR PASS|FIX|STOP` is the reviewer's check on `ITEM.md` before any maker is
launched. Without it, `dispatch <slug> maker …` refuses with
`refused: maker dispatch requires plan-audit PASS`. The auditor needs a row in `agents.tsv` and
nothing more — role casting is not checked here. Independence is checked, and how far it reaches
depends on what the item has on record. `plan-audit` refuses an agent listed in the item's
`MAKERS.tsv`, which the first maker dispatch writes; while that file is absent it refuses the agent
named in `items/<slug>/MAKER`, and `crucible brief <slug> maker <agent>` writes that file with no
gate of its own. Measured on two identical items: with neither file on record the first `plan-audit`
by `mk1` was accepted, and after `crucible brief json-flag maker mk1` — no maker dispatched, no
`MAKERS.tsv`, no maker attempt on the ledger — the first `plan-audit` by `mk1` refused:

```text
refused: mk1 is a maker of json-flag — plan-audit must be independent
```

Nothing was written: no `plan-audit.md` appeared, and the same command with an independent auditor
wrote it. So the refusal reaches an agent you briefed as maker, and it does not reach an agent you
have neither briefed nor dispatched. Name the reviewer — this is the "fresh reviewer validates the
breakdown" step, and for an agent with nothing on record the engine will not enforce it for you. The
verdict is write-once: a different one refuses with
`refused: plan-audit.md is immutable (existing PASS)`.

**2. Make sure the work branch exists.** `claim admit` binds the item to a Git target itself,
writing `items/<slug>/TARGET` from `PROGRAM`'s `repo:` and `base:` with `branch: ai/<slug>`. It does
not create that branch. `crucible target SLUG REPO BRANCH BASE` overrides the binding when the work
belongs on another repository, branch, or base; it refuses a base ref that does not exist
(`no such base ref: <base>`).

```sh
cat .crucible/<program>/items/<slug>/TARGET   # repo / branch / base
git branch ai/<slug> <base>                   # only if it does not exist yet
$CP workid <slug>                             # must print a commit, not NOBRANCH
```

Create the branch **before** the maker dispatch. `workid` is the branch's commit; while the branch
is missing it is the literal string `NOBRANCH`, and every later step degrades from there: `run`
names the evidence file `…NOBRANCH.txt`, and `result` refuses with
`maker result requires current work`. Creating the branch after the fact does not rescue it —
`result` then refuses `evidence work id does not match attempt`, and re-recording the evidence
refuses with `managed evidence requires a RUNNING or OVERDUE attempt` because the attempt has
already RETURNED. That attempt is spent.

**3. Dispatch and seal the maker.** The contract path is printed on stdout; the attempt id is its
parent directory.

```sh
D=$($CP dispatch <slug> maker <maker> A1 FOCUSED)
A=$(basename "$(dirname "$D")")
$CP attempt transport "$A" multi-agent      # or acp | subagent
$CP contract-audit "$A" <contract-auditor> PASS
$CP attempt start "$A" <observed-pid>
```

**4. The maker commits on the work branch, then records evidence.** Order matters: `run` stamps the
evidence with the work id as it is at that moment, and `result` requires that stamp to equal the
post-change work id. Evidence recorded before the commit carries the pre-change work id and `result`
refuses it with `evidence work id does not match attempt`.

```sh
git checkout ai/<slug>
# ... maker changes only the item's Owned files, then commits ...
$CP run <slug> <maker> -- <bounded-check>
```

**5. Finish and record the result.**

```sh
$CP attempt finish "$A" RETURNED "launcher observed exit 0"
$CP result "$A" PASS <evidence-basename> CLOSE -
```

`result` diffs the change against the dispatch work id and refuses a path outside `## Owned files`
with `refused: admitting this item does not authorize <path> (not in Owned files)`.

**6. Review, then close.** `phase <slug> REVIEW` refuses without a current-work maker PASS
(`refused: REVIEW requires a current-work maker PASS`). The reviewer runs the same
dispatch → transport → contract-audit → start → run → finish → result sequence, then:

```sh
$CP check <slug>
$CP close <slug> "one durable lesson, or NONE"
```

### Every role dispatch

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
  (`cleanup refuses while attempt <id> is RUNNING (live pid)`); a leftover
  `.crucible/<program>/.drive.lock` is released by `drive stop`. It KEEPs `agents.tsv` and `PANEL.ASSIGN.tsv` (panel identity, not
  leftover evidence), PRESERVEs the whole program directory — problem, proposal, claims, items,
  attempts, reviews, evidence — and removes the task and integration worktrees under
  `.crucible/<program>/worktrees/` while preserving their branches. If `--apply` stops with
  `could not safely remove worktree`, an integration worktree was left mid-cherry-pick: inspect it,
  then `git -C <worktree> cherry-pick --abort` and retry. Full story:
  [docs/managed-lifecycle.md](docs/managed-lifecycle.md#session-cleanup).

## Falsifier pair

Closure refuses unless the item's falsifier was recorded by `crucible run` at the current work id
in both directions — once failing with the mechanism removed, once passing with it restored — and
the gate reads those two files rather than running the falsifier itself.

