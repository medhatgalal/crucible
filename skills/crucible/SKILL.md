---
name: crucible
description: >
  Guided outer loop for Crucible working-mode. Ask what the user wants
  (new work, brownfield, bug, continue, update install, or engine), gather
  only the missing facts, adopt if needed, then run wm.sh go until CLOSED,
  STOP-ASK, or ESCALATE. Use when the user runs /crucible, says crucible,
  working-mode, FSD, or wants the chat to launch product work through go.
  Do not use for guided-cycle drive/START.md. Do not use to silently patch wm.sh.
metadata:
  short-description: "Guided intake then wm go (FSD until a brake)"
---

You are the **outer loop** (coordinator). Inner loop is `.crucible/work/wm.sh go`
(the wrapper execs the Rust `crucible` binary). You do not implement the product.
You do not judge PASS. You do not mix `drive`.
Do not use `/execute-plan` as the product walker for adopted repos.
Grok built-ins such as `/execute-plan` apply only when `/crucible` is not the loop.

# Brakes (always on)

If the user says stop, cancel, abort, hold, I'll drive, take the wheel, andon,
red, red button, or don't implement: stop launching or kill the in-flight `go`
process group, run `status`, report FLOOR, wait.

Engine work ("fix wm.sh", "change Crucible", "1.16") is a **different loop**.
Say so. Do not `go` on a product as a way to patch the engine.

# Intake (skip any answer already in the conversation)

Ask **one** missing fact at a time. Every missing fact — fork **or** free text
(path, job sentence, source) — goes through the harness question tool. Inspect
the **live** tool list. Do not guess. Do not print a numbered list in chat when
a question tool is present.

| Harness | Tool | Call |
| --- | --- | --- |
| Grok | `ask_user_question` | `questions[]` with `question` + `options` (`label`, `description`). Other is automatic. |
| Codex | `request_user_input` | one question (max 3). Each needs `id`, `header` (≤12 chars), `question`, 2–3 `options`. Recommended first, label suffix `(Recommended)`. Do not add Other; the client does. |
| Kiro CLI | none | no built-in ask-user tool. One numbered question in chat; wait. Do not invent `AskUserQuestion` or `ask_user_question`. |

If the Grok or Codex tool is missing this session, use the Kiro fallback.
Wait for the tool (or chat) answer before the next fact.

Forks use compact options. Free text still uses the same tool: offer an
inferred default as `(Recommended)` when you have one (cwd git root,
`$CRUCIBLE_SRC`) and let Other take the rest.

1. **Intent** (required if unknown)
   - New work (empty or new repo)
   - Brownfield feature (existing repo)
   - Bug fix (existing repo)
   - Continue a walk (`status` / `go` again on a **product** tree after
     engine refresh). Not how you fix wm.sh.
   - Update/refresh Crucible in this repo
   - Improve the Crucible **engine** (worktree on the Crucible source; then
     refresh the product and Continue walk)

2. **Where** — target git root. Default: current workspace if it is a git repo.
   Never adopt into the Crucible engine repo unless intent is engine (and then don't `go` a product).

3. **Job** — one sentence for `IDEA.md` unless continuing or refresh-only.
   Risk LOW unless they say HIGH or production/live. `live_write` defaults no.

4. **Source** — Crucible install to adopt from: `$CRUCIBLE_SRC`, or a tree that
   contains `./crucible` and `./wm.sh`. Ask if missing.

# Launch

Cwd **must** be the target root.

- No `.crucible/work/wm.sh`:  
  `"$SRC/crucible" adopt work --managed --working-mode`  
  (refresh: same with `--refresh`; stop any walker first).
- Write `IDEA.md` from the job sentence if missing and not continue/refresh.
- Run `.crucible/work/wm.sh` (help) once if they have never seen it, then:

```sh
.crucible/work/wm.sh go
```

Stay with the walker, but **do not lock the chat**. `go` prints `FLOOR t=+Ns
station=… card=… wip=…` on every card. After each ~20s of silence, print the
latest FLOOR line or run `status`. Never wait minutes without showing FLOOR.
Do not recast unless `go` says `NEXT CAST`. Do not `git commit` to unstick `go`.
On CLOSE run `debrief` and show TRACE deltas.

# After go starts

On **CLOSED PASS** or **CLOSED NO-BUILD**: show CLOSED, FLOOR, `git log -5 --oneline`, stop.

On **STOP-ASK**: quote the card from `WORKING-MODE.md` / FLOOR. Tell them the
file to write (`MAP-HUMAN`, `ANSWERS.md`, `IDEA.md`). Wait. Then `go` again.

On **ESCALATE** / kernel `refused:`: show the exact line and TRACE tail. Do **not**
patch `wm.sh` unless they explicitly switch to engine intent.

# Isolation

Zero CLIs → report `INDEPENDENCE_UNAVAILABLE`. One kind → never say CROSS-FAMILY.
Do not exec `kiro-cli acp`. Do not install skills under `$HOME`.
