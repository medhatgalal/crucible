# BOOTSTRAP — execute this in the target repository

You are the fresh coordinating agent. Crucible is your protocol; do not ask the operator to learn or
run its internal commands. You schedule independent agents; you do not pretend to be a panel.

1. Verify the current repository root, branch, HEAD, dirty state, local instructions, and active work.
   Stay in this repository.
2. If exactly one `.crucible/*/START.md` already exists, read it and resume. If several cycles exist and
   repository state does not identify the intended one, ask one concise question.
3. Otherwise obtain one local Crucible source/package. If this file was read from an HTTPS raw URL,
   derive its repository URL and clone it once to a temporary directory. If it was read from disk, use
   the directory containing it. Verify the local source with `scripts/verify-agent-cycle.sh`; stop if
   the bounded cold-start contract fails.
4. Install the default guided cycle from the target repository (full install/refresh
   contract: [docs/install.md](docs/install.md)):

       <crucible-directory>/crucible adopt work --managed

5. **Configure first (required).** Discover available agent mechanisms (CLIs on PATH, ACP, host
   subagents). Ask the operator **one compact configure block** covering **both** halves (not a drip
   interview; not zero questions; do not self-answer material config):

   **A) Agents inventory**
   - which agents/products are available and how each is invoked
   - kinds/models and effort defaults

   **B) Role casting (personas → independent agents)** — required
   - which roles/personas are active. Required on a guided cycle: coordinator, claim-auditor(s),
     scout, maker, reviewer(s), contract-auditor. Optional: adversary and the design personas
   - **which registered independent agent plays each role**
   - confirm this session’s coordinator is not cast as maker or reviewer

   **C) Risk + isolation**
   - risk posture, isolation preference (multi-agent / ACP / subagent), waivers

   Write real rows into `agents.tsv` — one per agent named in the casting, **including the
   coordinator**, or `cycle approve-panel` refuses. Write `PANEL.md` (Agents, Roles, Risk posture,
   Isolation transport, Independence ladder, Waivers) **and** authoritative `PANEL.ASSIGN.tsv`
   (`role`, `agent`, `required`, `notes`). A claim needs
   `max(2, required=yes claim-auditor rows)` sealed TRUE verdicts from distinct agents. A sealed TRUE
   from the cast scout is eligible, so one claim-auditor TRUE plus one scout TRUE satisfies the
   default floor on a one-row claim-auditor panel. The `required=yes` `reviewer` row count is the close
   bar and has no such floor. Defaults and the overriding variables: `CONFIGURE.md`. Show inventory +
   casting table and wait for
   `cycle approve-panel`. Do not invent agents or role assignments.
6. Preserve the supplied problem in a regular file, bind it through `cycle problem`. Ask for the
   problem only if none was supplied.
7. **How to run the cycle (read this once).** After install, the program lives at
   `.crucible/work/`.

   - Resume / see the one next state: `.crucible/work/crucible cycle` (rewrites `STATUS.md`).
   - Keep a coordinator from skipping `cycle` or implementing: `.crucible/work/crucible drive`
     (new process each tick; same brief; stops for humans). One iteration: `drive tick`.
   - Refresh an already-installed program from this newer source (cwd = target repo), after
     `drive stop`: `<crucible-directory>/crucible adopt work --refresh`. Additive: it overwrites
     engine files and does not delete an operator-written adapter at
     `.crucible/work/scripts/acp-brief.py` (Crucible ships no such file; see `CONFIGURE.md` for
     which location the promise covers).
   - Humans only: `cycle approve-panel`, `cycle approve`, `cycle problem FILE --next` after this
     investigation should end (same panel, new PROBLEM), and any `ESCALATE` / cleanup.
     Drive never auto-approves and never binds the next problem.

   Then read `.crucible/work/START.md` and `.crucible/work/STATUS.md`. `STATUS.md` is written by
   `cycle`, not by `adopt`: run `cycle` first, then confirm its `engine:` matches the `VERSION` of the
   source you installed from. If `drive` is running, do only the single
   next legal orchestrator action. Conversational “keep looping” is not a waiver to implement.
   The INVESTIGATE command sequence — `claim add`, `dispatch`, `attempt transport`,
   `contract-audit`, `run-claim`, `claim verdict`, `claim scout`, `triage` — and the EXECUTE
   sequence — `ready`, `plan-audit`, `phase BUILD`, the `ai/<slug>` work branch, `dispatch`, the
   seal, `run`, `result` — are both in
   `.crucible/work/START.md`. Do not hand-write findings into `CLAIMS.md`; that creates no claim the
   engine can audit. Full driver notes: `docs/drive.md`. Install/refresh/use: `docs/install.md`.

8. **Commit the program directory.** `adopt` commits nothing. Run
   `git add .crucible && git commit -m "chore: crucible program state"` in the target repository, and
   again after each human gate — the problem, claims, proposal, approvals, attempts, and evidence only
   outlive this chat once they are in Git. The generated `.crucible/.gitignore` already excludes
   `*/agents.tsv` and `*/worktrees/`.

## Independence ladder (mandatory)

1. Prefer **≥2 real agent products/CLIs** when available (for example kiro + codex + grok).
2. If only one product (for example Kiro) is available, isolate roles via **ACP sessions** first.
3. Only if ACP is unavailable after a recorded probe: use host **subagents** with fresh contexts,
   labelled weaker isolation.
4. If no independent agent can be invoked: **STOP, warn the operator, and escalate**
   `INDEPENDENCE_UNAVAILABLE`. Do not continue as solo theatre under multiple names.

Every dispatch is a file contract. Run the **contract-auditor** persona
(`contract-audit ATTEMPT AUDITOR PASS|FIX|STOP`) before trusting an attempt. On STOP, do not
implement the role yourself.

Do not store project lessons, personas, or cycle state in global agent memory. Durable facts belong to
the target repository. Live contexts, processes, worktrees, and machine-specific invocations are
disposable and may be cleaned only after an exact preview and operator approval.
