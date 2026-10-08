# Grok stand-in (temporary)

This rule applies only while Crucible cannot run the slice itself.

Crucible is operable when a guided checkout other than this engine ledger can carry one order through `drive`, an independent `reviews/review.md` written by a reviewer who is not the maker, and a land step that refuses to merge without that record plus green `refusals` and `refusals-bsd`. Until that is true, engine work in this checkout follows this file.

When `/crucible` is live, or a walk is live (FLOOR / `go`), the loop router wins. Do not force `/execute-plan` inside that walk. This stand-in covers engine slices built while no walk is live.

## Required sequence

A behavior change runs this sequence. The author of the diff does not count as the reviewer.

1. Framing and the bet stay in the shaping workshop. A bet stops. It does not open design, tickets, or code.
2. After the bet, write one waiting order on `docs/one-factory-program.md` whose done-when is the bet sentence, then one seam note: the module that already owns the behavior, what it reads, what it writes, and what it must not become.
3. Architecture of that seam uses `engos-design-architecture`.
4. Design uses the bundled `design` skill (writer and reviewer until consensus, including its pull-request plan). The sealed page design does not get another doubt cycle.
5. Implementation uses the bundled `execute-plan` skill. Fixes land in a worktree, not as unreviewed edits on the main workspace. The named proof is a failing test first.
6. Before commit, `engos-quality-code-review` reads the diff and returns ready, blocked, or split. A blocked or split result stops the commit.
7. Before merge, `requesting-code-review` dispatches a fresh subagent with the diff, the done-when sentence, and no session history. Required findings are fixed on the same branch or rejected on the pull request with a technical reason.
8. A behavior change also runs `engos-quality-testing-review` on the named proof: the test must fail if that behavior is removed. A documentation change also runs `engos-quality-docs-review`. A runtime, security, or boundary change also runs `code-review-and-quality`.
9. Open the pull request. Watch it with `/pr-babysit add --ship <number>`. Persist checks the way that skill specifies. Do not add a `watch_pr` shell script.
10. `engos-quality-gitops-review` confirms the hosted `pull_request` runs of `refusals` and `refusals-bsd`. A skipped macOS job on the push event is acceptable only when the pull-request job passed. A failed log is read with `gh run view <id> --log-failed` before any rerun.
11. Human review threads and bot review threads, including Codex, are answered before merge. A green `gh pr checks` exit code is not a review.
12. Squash with `gh pr merge <n> --squash --match-head-commit <sha>` only after steps 6–11. Then fast-forward local `main`. Leave the remote branch. No `--admin`, no `--auto`, no branch delete, no force-push, no `VERSION` bump, no tag.

A documentation correction of a contract already on the board may skip steps 3–5. Steps 6–12 still apply.

## Retirement

Delete this file in the same change that makes Crucible operable, as defined above. Until that change is on `main`, this file stays.
