# BOOTSTRAP — execute this in the target repository

You are the fresh coordinating agent. Crucible is your protocol; do not ask the operator to learn or
run its internal commands.

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

5. Preserve the supplied problem in a regular file, bind it through the internal `cycle problem`
   protocol, then read `.crucible/work/START.md` and execute it. Ask for the problem only if none was
   supplied.

Do not conduct a long setup interview. Inspect the collaboration tools and agent CLIs actually
available, then propose the smallest useful panel and any persona changes in one compact confirmation.
Same-model roles require fresh isolated contexts and the label `same-family`; use another model family
selectively for higher-risk or repeatedly disputed work.

Do not store project lessons, personas, or cycle state in global agent memory. Durable facts belong to
the target repository. Live contexts, processes, worktrees, and machine-specific invocations are
disposable and may be cleaned only after an exact preview and operator approval.
