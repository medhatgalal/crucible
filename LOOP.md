# LOOP

Two loops. The outer one converts a report into a backlog. The inner one walks one backlog item to
merge. Both are driven by `.crucible/<program>/crucible next`, which reads authoritative program
artifacts and tells you the one thing to do. With [managed lifecycle](docs/managed-lifecycle.md), that
source is `STATE.tsv`; `STATE.md` is generated for people.

What is gated, precisely — because an earlier version of this page overclaimed and a judge caught it.
`crucible phase` refuses to enter a phase whose named artifact does not exist, and `crucible check`
refuses closure without work, evidence bound to the current work, a written falsifier, and enough
passing verdicts across enough distinct kinds. Moving between the middle phases is **not** gated on
verdicts: the orchestrator advances them, guided by `crucible next`. So the gate is hard at the ends
and advisory in the middle, and you should rely on it accordingly.

---

## Outer loop: report → backlog

The point is that **most claims in a report are wrong, stale, or already done**, and building them is
worse than ignoring them. Nothing enters the backlog until it survives audit.

```
report
  │
  ├─ INTAKE      orchestrator splits the report into numbered CLAIMS, one per finding,
  │              each quoting its source sentence verbatim.       → CLAIMS.md
  │
  ├─ AUDIT       for each claim, dispatch 2+ claim-auditors of different kinds. Each
  │              independently answers: is this claim TRUE of the code as it exists?
  │              They record their checks with `crucible run-claim` and write verdicts.
  │              A claim with no surviving TRUE verdict is DROPPED with its reason.
  │              → claims/<n>/verdicts/*.md
  │
  ├─ SCOUT       for each surviving claim, dispatch a scout: does code already do this,
  │              fully or partly? Search by behaviour, not by name; report what you
  │              searched. The scout records its searches with `run-claim` and its
  │              result with `claim scout CN RESULT AGENT`; there is no separate report file. FULLY-EXISTS is dropped and recorded.   → claim scout CN RESULT AGENT
  │
  ├─ TRIAGE      `crucible triage` builds the decision table from the verdicts. It refuses
  │              to recommend anything for an unaudited claim. Take it to the operator and
  │              agree the backlog together: drops, merges, splits, narrowings.
  │
  └─ ADMIT       `crucible claim admit CN <slug>` — refuses a claim without enough TRUE
                 verdicts across enough kinds.                     → items/<slug>/
```

Verbs for this loop:

```sh
crucible claim add "<claim>" "<exact source sentence>"      # one per finding
crucible dispatch C1 claim-auditor a1                        # two, of different kinds
crucible run-claim C1 a1 -- <cmd>                            # the auditor records its checks
crucible claim verdict C1 a1 TRUE|FALSE|STALE|UNVERIFIABLE
crucible claim scout   C1 ABSENT|PARTLY-EXISTS|FULLY-EXISTS AGENT
crucible triage                                             # the table for the operator
crucible claim admit   C1 <slug>                            # refuses if unaudited
```

Gate to admit, enforced by `crucible claim admit`: enough TRUE verdicts (`CRUCIBLE_MIN_AUDITORS`,
default 2) across enough distinct kinds (`CRUCIBLE_MIN_KINDS`), **and** a recorded scout report that
is not FULLY-EXISTS. A missing scout report refuses, because a claim reaching the backlog with
nobody having looked for an existing implementation is the most repeated failure this loop exists
to prevent. Everything dropped stays in `CLAIMS.md` with the verdict that killed it, because a
dropped claim that leaves no trace comes back next quarter.

---

## Inner loop: one item → merged

The seven phases below are the item-file compatibility workflow. Managed programs use the smaller
behavior documented in [Managed lifecycle](docs/managed-lifecycle.md).

```
      ┌──────────────────────────────────────────────────────────────┐
      │                                                              │
   SPEC ──► DESIGN ──► TASKS ──► BUILD ──► VERIFY ──► ADVERSARY ──► GRADUATE
      ▲                            │          │           │
      └──── REJECT sends it back ──┴──────────┴───────────-┘
```

| Phase | Who does it | Artifact it must produce | Gate to leave the phase |
|---|---|---|---|
| **SPEC** | specifier | `ITEM.md`: the ask, acceptance criteria, non-goals, **and the falsifier** | falsifier written (not the template marker); 1+ judge PASS that the criteria are testable and the falsifier would actually fail |
| **DESIGN** | architect | `DESIGN.md`: how it fits the existing architecture, alternatives considered, what it deliberately does not do, rollback | 1+ judge PASS; if it proposes changing the architecture, that is stated as a change and judged as one |
| **TASKS** | planner | `TASKS.md`: independently testable steps, file ownership per task, order | 1+ judge PASS that each task is testable alone and no two tasks own the same file |
| **BUILD** | maker (one per task) | code under `work/`, plus recorded evidence per task | every task checked; no task left claiming completion without evidence |
| **VERIFY** | maker records, judges check | recorded runs for tests, lint, security, and any performance criterion the spec named | evidence exists, is bound to the current work id, and is non-empty for each named criterion |
| **ADVERSARY** | adversary | `ADVERSARY.md`: attempts to break it, not to praise it | its findings are either fixed or answered with evidence; unanswered Critical/Important blocks |
| **GRADUATE** | integrator | commit, branch, CI result, merge | `.crucible/<program>/crucible check` CLOSEABLE with `CRUCIBLE_MIN_JUDGES` PASSes; CI result recorded as evidence; then `close` |

### Iteration contract

A REJECT returns **the findings only**, never the judge's opinion of the maker. The maker fixes, or
disputes with evidence. A dispute goes to a **fresh** judge that never saw the original verdict, with
the sides unlabelled — a judge asked to reconsider its own verdict defends it.

Bounded: two rejects on the same finding, or the same work id submitted twice, is a terminal stop.
Escalate to the human with the reason. That is a legitimate outcome, not a failure of nerve.

### What survives a restart

`STATE.md` holds: the current item, its phase, the dispatch in flight, and the work id at the last
gate pass. On restart, read it and `git log`; do not re-dispatch a completed phase. `LESSONS.md`
holds what was learned, and is concatenated into every later maker brief, so learning is structural
rather than remembered.

### Where the human is

Exactly three places: they hand over the report; they answer an escalation; they say whether a
proposed architecture change is acceptable. Everywhere else the loop runs on its own, and if it
needs a nudge, the nudge is the finding.
