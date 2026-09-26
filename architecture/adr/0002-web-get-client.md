# Web UI is a GET camera

**Date:** 2026-09-23  
**Status:** Accepted  
**Supersedes:** the "not in the v1 workspace" timing in ADR 0001 D17. D17's rule stands: the page is not a second kernel.

The operator asked for the web UI after the room cameras. `crucible web` serves a loopback page and proxies GET `/walk`, `/stats`, and `/health` from `crucible serve`. The web process does not itself write FLOOR, TRACE, or EVENTS. `POST /go` stays 405. See the addendum for backlog, chat, and `POST /act/go`.

Room cameras and this page are clients of the same JSON. Herdr stays an external process. No Herdr, Grok, or EngOS crate in `kernel` or `contract`.

## Addendum (2026-09-23)

The page may append `BACKLOG.tsv` and `.wm/CHAT.md` only, and may `POST /act/go`, which only spawns `go` as a process group. `crucible serve` still never writes. The page is still not a second kernel.

## Addendum (2026-09-25)

The page may also `POST /act/<verb>` for `crucible_web::WEB_READ_ONLY`
(`agents`, `debrief`, `next`, `panes`, `stats`, `workid`) and `POST /act/status`
only when the JSON args are exactly `["--json"]`. The web process spawns
`current_exe` with that verb and those args, waits, and returns the child's
stdout bytes as the HTTP body, including length 0. It does not substitute
stderr, it does not lossy-decode, it does not interpret the stdout, and it does
not choose the next step. Stderr stays on the camera log. Those spawns are
not a process group. Verbs whose functions create, truncate, or append a file
are not in the allowlist (`state`, `target`, `brief`, and `lifecycle`).
`POST /act/go` is unchanged. `POST /go` on `crucible serve` stays
405. `crucible serve` still never writes. The page is still not a second kernel.

## Addendum (2026-09-25)

The page may `POST /act/close`, `POST /act/drive`, `POST /act/adopt`, and `POST /act/status` with args `[]`, from `crucible_web::WEB_WRITERS`. `status --json` stays the read-only allow. `drive` and `adopt` are detached process groups and return pid JSON. `close` and bare `status` wait up to 5 seconds and return stdout. The web process still does not write FLOOR, BACKLOG, or CHAT except through the backlog and chat handlers. The children may write. `POST /go` on `crucible serve` stays 405. `POST /act/go` is unchanged. The page does not choose the next step. `state`, `target`, `brief`, and `lifecycle` stay off the page.

## Addendum (2026-09-26)

The page may `POST /act/state`, `POST /act/target`, `POST /act/brief`, and `POST /act/lifecycle`.
