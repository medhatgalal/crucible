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
4. Install the default guided cycle from the target repository:

       <crucible-directory>/crucible adopt work --managed

5. **Configure first (required).** Discover available agent mechanisms (CLIs on PATH, ACP, host
   subagents). Ask the operator **one compact configure block** covering **both** halves (not a drip
   interview; not zero questions; do not self-answer material config):

   **A) Agents inventory**
   - which agents/products are available and how each is invoked
   - kinds/models and effort defaults

   **B) Role casting (personas → independent agents)** — required
   - which roles/personas are active (coordinator, claim-auditor(s), maker, reviewer(s),
     contract-auditor, optional scout/adversary/…)
   - **which registered independent agent plays each role**
   - confirm this session’s coordinator is not cast as maker or reviewer

   **C) Risk + isolation**
   - risk posture, isolation preference (multi-agent / ACP / subagent), waivers

   Write real rows into `agents.tsv`. Write `PANEL.md` (Agents, Roles, Risk posture, Isolation
   transport, Independence ladder, Waivers) **and** authoritative `PANEL.ASSIGN.tsv`
   (`role`, `agent`, `required`, `notes`). Show inventory + casting table and wait for
   `cycle approve-panel`. Do not invent agents or role assignments.
6. Preserve the supplied problem in a regular file, bind it through `cycle problem`, then read
   `.crucible/work/START.md` and execute it. Ask for the problem only if none was supplied.

## Independence ladder (mandatory)

1. Prefer **≥2 real agent products/CLIs** when available (for example kiro + codex + grok).
2. If only one product (for example Kiro) is available, isolate roles via **ACP sessions** first.
3. Only if ACP is unavailable after a recorded probe: use host **subagents** with fresh contexts,
   labelled weaker isolation.
4. If no independent agent can be invoked: **STOP, warn the operator, and escalate**
   `INDEPENDENCE_UNAVAILABLE`. Do not continue as solo theatre under multiple names.

Every dispatch is a file contract. Run the **contract-auditor** persona (`contract-audit PASS|FIX|STOP`)
before trusting an attempt. On STOP, do not implement the role yourself.

Do not store project lessons, personas, or cycle state in global agent memory. Durable facts belong to
the target repository. Live contexts, processes, worktrees, and machine-specific invocations are
disposable and may be cleaned only after an exact preview and operator approval.
