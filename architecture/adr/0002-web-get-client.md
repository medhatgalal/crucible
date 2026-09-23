# Web UI is a GET camera

**Date:** 2026-09-23  
**Status:** Accepted  
**Supersedes:** the "not in the v1 workspace" timing in ADR 0001 D17. D17's rule stands: the page is not a second kernel.

The operator asked for the web UI after the room cameras. `crucible web` serves a loopback page and proxies GET `/walk`, `/stats`, and `/health` from `crucible serve`. POST, including `POST /go`, is refused. The page does not write FLOOR, TRACE, or EVENTS, and it does not spawn `go`.

Room cameras and this page are clients of the same JSON. Herdr stays an external process. No Herdr, Grok, or EngOS crate in `kernel` or `contract`.
