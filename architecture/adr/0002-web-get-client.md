# Web UI is a GET camera

**Date:** 2026-09-23  
**Status:** Accepted  
**Supersedes:** the "not in the v1 workspace" timing in ADR 0001 D17. D17's rule stands: the page is not a second kernel.

The operator asked for the web UI after the room cameras. `crucible web` serves a loopback page and proxies GET `/walk`, `/stats`, and `/health` from `crucible serve`. The page does not write FLOOR, TRACE, or EVENTS. `POST /go` stays 405. See the addendum for backlog, chat, and `POST /act/go`.

Room cameras and this page are clients of the same JSON. Herdr stays an external process. No Herdr, Grok, or EngOS crate in `kernel` or `contract`.

## Addendum (2026-09-23)

The page may append `BACKLOG.tsv` and `.wm/CHAT.md` only, and may `POST /act/go`, which only spawns `go` as a process group. `crucible serve` still never writes. The page is still not a second kernel.
