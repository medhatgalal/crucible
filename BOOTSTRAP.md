# BOOTSTRAP — read this file and execute it

You are an agent. Someone pointed you at this file, from a URL or from a local checkout, and said
run it. This file is a prompt, not documentation. Execute it.

Your working directory is the repository the operator wants to work on. **Keep it that way.** If you
are currently inside the crucible checkout instead, ask the operator which repository they mean and
`cd` there first. Everything you produce belongs to their repo, not to crucible's.

You do not know the answers to what follows. **Ask, one question at a time, and wait.** Do not batch
the questions, do not guess a default to keep things moving, and do not proceed past an unanswered
one. Getting this wrong quietly is worse than being slow.

---

## Step 1 — locate the engine

The crucible checkout is already on disk somewhere. Find it without cloning anything:

```sh
# if the operator told you where it is, use that. otherwise look in the usual places:
for d in ./crucible ../crucible ~/crucible ~/src/crucible ~/code/crucible; do
  [ -x "$d/crucible" ] && echo "found: $d"
done
```

If that finds nothing, ask: **"Where is your crucible checkout?"**

If the operator does not have one — you were pointed at this file by URL and there is no local
copy — then clone the **repository** once, into a temporary directory.

Take care here: the URL you were given may be a link to *this file*, not to the repository. A raw
file URL is not cloneable, and `git clone` on one fails with 404. Derive the repository URL, or
ask for it:

```sh
# a raw file URL looks like
#   https://raw.githubusercontent.com/<owner>/<repo>/<branch>/BOOTSTRAP.md
# the repository it belongs to is
#   https://github.com/<owner>/<repo>.git
tmp=$(mktemp -d)
git clone --depth 1 "$REPO_URL" "$tmp/crucible"    # REPO_URL, not the file URL
```

If you cannot tell which repository a URL belongs to, **ask the operator for the repository URL**
rather than guessing. Then verify you got the engine and not something else:

```sh
test -x "$tmp/crucible/crucible" && "$tmp/crucible/scripts/selftest.sh"
```

That is the only case where cloning is right. Do not clone when a checkout already exists: a
second copy drifts, and you will end up running a version that does not match its own
documentation. Either way, the next step installs a self-contained copy into the operator's
repository, so the temporary clone can be deleted afterwards.

Verify it works before you trust it:

```sh
<engine>/scripts/selftest.sh
```

If that does not end in `0 failed`, stop and tell the operator. A gate that cannot prove it refuses
is not a gate, and nothing after this point means anything.

## Step 2 — interview the operator

Ask these in order. Each answer changes what you do next.

**Q1. "What should I call this program?"** One short name, letters, digits, dash. It becomes
`.crucible/<name>/` in their repo. If they will run several efforts against this repo, the name
should say which effort this is, not what the repo is.

**Q2. "Which agents can I use, and how is each one invoked?"** For each: a short name, its *kind*
(the model family), the model, the effort or reasoning level if it has one, and the exact command
line that hands it a file. You will write these into `agents.tsv`.

Tell them why kind matters, because it changes their answer: **kind is what cross-model judging
counts.** Two judges of different kinds catch more than three of the same kind — same-kind agents
have independent context but correlated blind spots, and will confidently repeat each other's
mistakes. If they only have one kind available, say plainly that judging will be weaker.

**Q3. "How reversible is this work?"** This sets the panel size. Docs and reversible config: one
judge. Anything behavioural: two, of different kinds. Auth, data, migrations, deletions, hot paths:
three, at least two kinds. Offer the recommendation, let them override, record what they chose.

**Q4. "Which roles do you want?"** List what is in `<engine>/roles/`. Orchestrator, maker and judge
are the mechanism and stay. Adversary is worth keeping for anything they would hate to ship wrong.
The rest — claim-auditor, scout, specifier, architect, planner, integrator — are staging they can
drop if they would rather write the artifacts themselves. Deleting a role file makes that role
undispatchable, which is the intended way to configure. Ask whether they want to edit any role file
now: **a role file is where their standards live**, and it binds every agent that ever takes that
role.

**Q5. "What problem am I solving? Give me the document."** A report, an audit, a critique, a list of
complaints, a design doc — whatever they have. If they have nothing written, ask them to describe it
and write it down yourself, then read it back and get their confirmation that it is right.

## Step 3 — install

```sh
cd <their repo root>
<engine>/crucible adopt <program-name>
```

If you want authoritative machine-readable state, enable [managed lifecycle](docs/managed-lifecycle.md)
now, before admitting the first item:

```sh
CP=.crucible/<program-name>/crucible
$CP lifecycle enable --dry-run
$CP lifecycle enable --apply
```

Then write their answers into `.crucible/<program-name>/agents.tsv`, delete any role files they did
not want, and check `.crucible/<program-name>/crucible agents` shows what they described.

Confirm before going further: **"Here is the panel, the roles, and the judging thresholds I recorded.
Shall I proceed?"** Show it to them. Do not assume you understood.

Now make the run visible. If you are inside a tmux session, this is the moment:

```sh
.crucible/<program>/crucible panes
```

It creates no session and replaces nothing — whatever made the window keeps owning it. It leaves
your pane alone, titles it `agent`, and turns the other panes into live views of the gate, the
verdicts, the work ids and the newest evidence. Re-run it any time to reset them.

If the operator is not in tmux, say what they would gain and offer it: from here on you will make
claims about state constantly, and those panes are how they check you without asking. If they
decline, continue — it is a window, not a dependency.

## Step 4 — turn the document into claims

Read their document. For every distinct finding, write one claim, quoting the source sentence
verbatim so the trace back is a string match and not your memory:

```sh
.crucible/<program>/crucible claim add "<the claim, one line>" "<the exact sentence from the document>"
```

Split compound findings. One claim should be one thing that could be independently true or false. If
a sentence contains three complaints, that is three claims.

## Step 5 — fact-check the report before you believe a word of it

**This is the step that earns everything else, and it comes before any planning.** A report is
someone's belief about a codebase at some past moment. Much of it is wrong, some is already fixed,
and some describes work that already exists somewhere they did not look.

For each claim, dispatch **two auditors of different kinds**:

```sh
.crucible/<program>/crucible dispatch <slug-or-claim> claim-auditor a1
```

An auditor answers one question — is this claim true of the code as it exists *now*? — and answers
it from the code, with `file:line`, recording every command it runs. It returns TRUE, FALSE, STALE
(was true, since fixed), or UNVERIFIABLE (cannot be established from the code, and here is what is
missing). It may not guess in either direction.

Then, for each survivor, dispatch a **scout**: does the repo already do this, fully or partly? The
scout searches by behaviour, not by name, because the thing you would build rarely contains the words
you would call it — and it reports what it searched so the claim of absence can itself be checked.

Record each result:

```sh
.crucible/<program>/crucible claim verdict C1 a1 TRUE
```

## Step 6 — brainstorm the backlog with the operator, from the evidence

Now, and only now, produce the triage table:

```sh
.crucible/<program>/crucible triage
```

It refuses to emit a disposition for any claim that has not been audited, so this table is grounded
in verdicts rather than in your impressions. Take it to the operator and work through it together.
Say what the evidence says, and what you would do, and let them decide:

- **False positives** — claims the auditors found untrue. Name them and the evidence. These are the
  most valuable output of the whole step: work not done.
- **Already fixed** — STALE claims, with the change that fixed them.
- **Already exists** — what the scout found, fully or partly. Partly is the interesting case: the
  item shrinks to the gap.
- **Redundant or overlapping** — claims that are the same work wearing different words. Propose the
  merge.
- **Too big** — claims that are really several items. Propose the split.
- **Unverifiable** — say so rather than resolving it in either direction, and ask them what they know
  that the code does not show.
- **What is missing** — anything you found while auditing that the report did not mention. Report it;
  do not silently add it.

Argue with them where the evidence supports it, and change your mind when theirs is better. Keep
going until there is a backlog they will actually commit to. Every dropped claim keeps its heading
and the verdict that killed it, so nothing quietly returns next quarter.

## Step 7 — admit and ask permission to run

```sh
.crucible/<program>/crucible claim admit C1 <slug>
```

`admit` refuses a claim without enough TRUE verdicts from enough distinct kinds, so an unaudited
claim cannot become work. Each admitted item is created and pointed at a branch in their repo
automatically.

Then stop and ask: **"Here is the executable backlog, in this order. Shall I start?"** Show it.
Do not begin the inner loop on your own initiative.

## Step 8 — run the loop

Read `.crucible/<program>/LOOP.md` and `.crucible/<program>/RULES.md`, then:

```sh
.crucible/<program>/crucible next
```

`next` reads the files, not your memory, and tells you the one next thing. Use it after every
interruption, restart, and context compaction, and trust it over anything you think you remember.

---

## The standing obligation

This applies to every step above, to the inner loop, and to everything you say to the operator.

**Nothing is accepted because it sounds right.** Not a claim, not an answer, not a plan, not a design,
not a fix, and not your own reasoning. Every one of them is a proposal until something independent has
tried to break it and failed.

In practice:

- **Every claim gets audited before it becomes work.** Enforced: `claim admit` refuses otherwise.
- **Every artifact gets judged by an agent that did not produce it**, and the judge never receives the
  maker's reasoning. Enforced: judge briefs are built from a whitelist, and the maker is barred from
  judging its own work by name.
- **Every PASS cites evidence its own author recorded.** Enforced.
- **Anything that reaches graduation gets an adversary** whose job is to break it, not to balance a
  report with praise.
- **Every claim you make to the operator carries its source inline** — the command, the file:line, the
  evidence path. If you have no artifact, label it ASSUMPTION out loud. You will be wrong sometimes;
  the labelling is what makes that recoverable.
- **Losing information never improves an outcome.** Missing, empty or stale evidence refuses. If you
  find a check that is most confident when it knows least, that check is inverted — say so.
- **When you cannot establish something, say INSUFFICIENT_EVIDENCE and name what is missing.** Never
  resolve doubt in favour of proceeding.

The gate enforces what can be enforced and `RULES.md` labels the rest honestly as words. Words have a
poor record here — in this project's own history, written rules were broken by their own authors
within hours. So prefer a check, and when you convert a rule into a check, record it as a lesson so
the next program starts ahead of this one.
