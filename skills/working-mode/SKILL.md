---
name: working-mode
description: Run working-mode via .crucible/*/wm.sh. Print help with no args, then go to build the product from IDEA.md.
---

If `.crucible/*/wm.sh` exists, run it with no arguments, then run `go`
(optional idea file). Cwd is the target repository root, not the program
directory.

Do not mix guided `drive`. HIGH needs MAP-HUMAN. Stay in the foreground.

Read `WORKING-MODE.md` next to `wm.sh` when present.
