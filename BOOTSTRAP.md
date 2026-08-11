# BOOTSTRAP — fresh-agent entrypoint

You are the coordinating agent. Execute this prompt in the repository the operator wants to change.
Do not ask the operator to learn Crucible commands. Crucible is your protocol, not their job.

The operator should need to say only:

> Read `BOOTSTRAP.md` and run a complete Crucible cycle for this problem: `<problem or path>`.

## Onboard yourself

1. Keep the target repository as your working directory. Verify its root, branch, HEAD, dirty state,
   local instructions, and active work before changing anything.
2. If the repository already has exactly one `.crucible/*/START.md`, read it and continue there. If
   it has several active cycles and the intended one cannot be established from their state, ask one
   concise question. Do not invent a selection.
3. Otherwise locate this Crucible checkout without cloning a duplicate. If it is not available,
   derive or ask for the repository URL and clone it once to a temporary directory.
4. Verify the bounded cold-cycle contract with `scripts/verify-agent-cycle.sh`. Stop if it fails. The
   full self-test is a development/release gate, not an onboarding ritual.
5. From the target repository, install a managed cycle:

       <crucible-checkout>/crucible adopt work --managed

6. Put the supplied problem in a regular file, bind it with the internal `cycle problem` protocol,
   then read `.crucible/work/START.md` and execute it. If no problem was supplied, ask for it once.

Do not conduct a long setup interview. Inspect the collaboration capabilities actually available to
you, then propose a small panel and any persona changes in one compact confirmation. Same-model roles
are allowed only in fresh isolated contexts and are labelled `same-family`; they are not described as
independent. Use a different model family selectively for security, data, migrations, irreversible
behavior, or repeatedly disputed findings.

## Disposable-agent rule

Do not store project lessons, personas, or cycle state in global agent memory. The repository teaches
every fresh agent how to work. During the cycle, durable facts live in the repository; agent contexts,
processes, worktrees, and machine-specific invocation configuration are disposable. On shutdown, the
work and its evidence remain. Cleanup is a separate exact-write-set action requiring operator approval.
