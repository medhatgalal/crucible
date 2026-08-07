# Crucible

A file-only refusal gate for agentic software work. One agent orchestrates, others make and judge,
and nothing closes until the recorded evidence says it may.

**No dependencies.** POSIX shell and markdown. Nothing to install, no service, no database, no
tracker. State is files in your repository.

The problem it exists for: an agent working alone grades its own homework. It picks the evidence,
decides what the evidence means, and rules on whether it succeeded. When those three roles sit in one
context the result is confident and wrong often enough to matter. Separating them helps only if the
separation is enforced, so this enforces what it can and says plainly what it cannot.

---

## How you start it

Stand in the repository you want to work on. Give an agent one line:

> read `https://raw.githubusercontent.com/medhatgalal/crucible/main/BOOTSTRAP.md` and execute it.
> The repository is `https://github.com/medhatgalal/crucible.git`

Give both, because they are different things: the first is the file to read, the second is what to
clone if the agent finds no local checkout. A raw file URL is not cloneable, and an agent handed
only the first will fail on `git clone`.

`BOOTSTRAP.md` is a prompt, not documentation. The agent verifies the gate works, then interviews you
— one question at a time, waiting for each answer — for a program name, which agents you have and how
each is invoked, how reversible the work is, which roles you want, and your problem document. Then it
installs itself into *your* repo and asks permission before doing anything.

**The first thing it does is not planning. It fact-checks your document against your code.** Two
auditors of different kinds per claim, then a scout for work that already exists, then a triage table
it brings back to you so you agree the backlog together. Claims that turn out untrue, already fixed,
or already implemented are dropped with the evidence that killed them — work you do not have to do is
the most valuable output of that step.

Because it installs into your repo, your working directory stays the repo, and claims, items,
evidence, verdicts and lessons are versioned alongside the code they belong to. `agents.tsv` is
gitignored because it names the agents on your machine. A second effort against the same repo is a
second program with its own panel and no interaction.

## The loop

```
problem document
   │
   ├─ claims          one per finding, quoting its source sentence verbatim
   ├─ audit           2+ auditors of different kinds: TRUE / FALSE / STALE / UNVERIFIABLE
   ├─ scout           does this already exist, fully or partly? searched by behaviour, not name
   ├─ triage          the decision table — refuses to recommend on an unaudited claim
   └─ admit           survivors become items, each on its own branch
                          │
   SPEC ─ DESIGN ─ TASKS ─ BUILD ─ VERIFY ─ ADVERSARY ─ GRADUATE
      ▲                      │        │         │
      └──── a REJECT sends it back ───┴─────────┘
```

Phases refuse without their artifacts, and closure refuses without evidence and verdicts.
`crucible check` refuses; it never warns. Not every step is gated — the intake, triage and
admission decisions are yours, informed by verdicts the gate does enforce.

## What it refuses

A missing or wrong file stops the run. Each of these is asserted in `scripts/selftest.sh`.

One caveat, because an earlier version of this page overclaimed and a review proved it: **the
assertions overlap.** Several mechanisms guard the same property, so deleting one mechanism does not
always fail the suite — the next guard catches it. Individually load-bearing, verified by mutation,
are: the work id being the branch commit, evidence binding to that id, a PASS naming its own
author's evidence, the registered-agent requirement, a substantive falsifier, a recorded maker, and
claim admission needing recorded checks. The rest are asserted but not individually isolated, and
`RELEASE.md` names the mutations to run by hand before a release.

- Evidence must exist, be non-empty, name a registered agent, and carry the current work id in both
  its filename and its body — so renaming stale evidence to look current is refused.
- A PASS must name an evidence file recorded under its own author's name.
- Only agents registered in `agents.tsv` may judge, and a recorded maker may not judge its own work.
  A missing maker is itself a refusal, because otherwise the self-review check is vacuous.
- Any commit on the item's branch changes the work id and voids every verdict given before it.
- Absence refuses: no work, no evidence, no falsifier section, an empty falsifier, no verdict, no
  scout result, too few verdicts, too few distinct kinds.
- A claim cannot become an item unless enough registered auditors of enough distinct kinds recorded
  their own checks and returned TRUE, and a scout recorded a result that is not FULLY-EXISTS.
- Phases refuse without their artifacts, even when the phase is hand-edited.
- Closing twice refuses. Work changing between check and close refuses.
- Concurrent recording and closing are safe: names are unique by construction, evidence is published
  by atomic rename, and a close is claimed with `mkdir`.
- An unknown verb refuses instead of printing help and succeeding.

## What it does not do

Withdrawn claims, listed because an earlier version of this page asserted them and an independent
review proved each one false:

**It cannot tell tool-recorded evidence from a convincing forgery.** `crucible run` writes a header,
and a hand-written file carrying that header is accepted. The header is a **convention that makes
accidental hand-written evidence obvious**, not a signature. Under one user with a shell there is no
way to close this in files alone.

**It cannot prove who wrote a verdict.** One actor can author every verdict under different
registered names. This has happened in practice during development, and the gate reported the item
closeable. Independence is a property of how you dispatch, not something this can establish.

**It does not verify that a recorded command was a meaningful test**, that a judge understood what it
read, that a lesson was applied rather than merely present, or that your acceptance criteria are
complete. Nothing here challenges the criteria you wrote; a requirement you failed to write is
invisible to every review it performs.

**It does not enforce that a dispatch preceded a verdict**, that a finding is not resubmitted, or
that file ownership in `TASKS.md` is respected. Those are in `RULES.md` and labelled RULE, not CHECK.

So the honest description is narrow: **it keeps an auditable record and refuses careless omissions.**
It raises the cost of a fabricated review; it does not make one impossible. `RULES.md` labels every
line CHECK or RULE so you always know which you are relying on.

## Judging: kinds beat counts

`kind` is the model family, and it is what `CRUCIBLE_MIN_KINDS` counts. Same-kind agents have
independent context but correlated blind spots and will confidently repeat each other's mistakes. Two
judges of different kinds are worth more than three of one kind. Set panel size by reversibility: one
judge for docs, two of different kinds for anything behavioural, three for auth, data, migrations,
deletions and hot paths. See [CONFIGURE.md](CONFIGURE.md).

## Verbs

```
crucible adopt [PROGRAM]               install into the repo you are standing in
crucible selftest                      prove the gate refuses what it claims to
crucible claim add|verdict|scout|admit the outer loop: audit a problem document
crucible triage                        evidence-grounded decision table for the operator
crucible next                          what to do now, read from files not memory
crucible dispatch ITEM ROLE AGENT      write the contract for a call, print its path
crucible run ITEM AGENT -- CMD...      run CMD and record the result as evidence
crucible check ITEM                    the gate. non-zero unless closeable
crucible close ITEM "LESSON"           refuses unless check passes
crucible panes [VIEW...]               overlay live views onto your tmux window
```

`crucible help` lists them all. `crucible dispatch` prints the exact command to invoke the agent,
model and effort substituted from `agents.tsv`.

## Watching a run

`crucible panes` overlays live views onto the tmux window you are already in. It creates no session
and replaces no session manager: your pane is left alone and titled `agent`, the rest become views of
the gate, the verdicts, the work ids and the newest evidence.

**If a view and an agent disagree about whether something is done, the view is right.** That is the
entire reason the views exist.

## Verifying it yourself

```sh
./scripts/selftest.sh          # -v to name each assertion as it runs
```

Every documented refusal is asserted there. A claim in this README that is not asserted in the
selftest is an unverified claim, and CI runs it on every push.

## Smoke test

Not how you start a program — this is a two-minute check that the gate mechanics work in a scratch
directory. The real entry point is `BOOTSTRAP.md` above.

```sh
d=$(mktemp -d) && cp crucible "$d/" && cp -r roles "$d/" && cp RULES.md "$d/" && cd "$d"
printf 'mk\tkiro\tm\thigh\techo {BRIEF}\nj1\tkiro\tm\thigh\techo {BRIEF}\nj2\tgrok\tm\thigh\techo {BRIEF}\n' > agents.tsv
./crucible add demo "Close one file-only item"
./crucible brief demo maker mk >/dev/null      # records who made it; a missing maker refuses
sed -i.bak 's|^TEMPLATE-FALSIFIER-UNWRITTEN.*|Undo the change; the named check fails.|' items/demo/ITEM.md
mkdir -p items/demo/work && echo 'x = 1' > items/demo/work/a.py
./crucible run demo j1 -- sh -c 'echo j1 ran the falsifier'
./crucible run demo j2 -- sh -c 'echo j2 re-derived independently'
w=$(./crucible workid demo)
printf 'VERDICT: PASS\nWORK-ID: %s\nfalsifier run, see %s\n' "$w" "$(ls items/demo/evidence | grep ^j1)" > items/demo/verdicts/j1.md
printf 'VERDICT: PASS\nWORK-ID: %s\nindependent, see %s\n'   "$w" "$(ls items/demo/evidence | grep ^j2)" > items/demo/verdicts/j2.md
CRUCIBLE_MIN_KINDS=2 ./crucible check demo
CRUCIBLE_MIN_KINDS=2 ./crucible close demo "Kinds beat counts."
```

Then confirm it really is a gate: `echo 'x = 2' >> items/demo/work/a.py && ./crucible check demo`
now refuses, because the edit voided both verdicts.

## Documents

| file | what it is |
|---|---|
| [BOOTSTRAP.md](BOOTSTRAP.md) | the entry prompt. Point an agent at this. |
| [START.md](START.md) | what an installed program's orchestrator reads |
| [RULES.md](RULES.md) | the constraints, each labelled CHECK or RULE |
| [LOOP.md](LOOP.md) | the lifecycle and what each transition requires |
| [CONFIGURE.md](CONFIGURE.md) | panels, models, effort, roles, how many agents |
| [roles/](roles/) | one file per role. **Editing these is how you encode your standards.** |
| [RELEASE.md](RELEASE.md) | the release procedure |
| [CHANGELOG.md](CHANGELOG.md) | what changed, and which bug caused it |

## License

MIT. See [LICENSE](LICENSE).
