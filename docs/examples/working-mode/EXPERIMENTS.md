# Working-mode experiment log (2026-09-11)

Operator-unattended runs against `feat/self-contained-working-mode`.
Fixture CLIs implement the specs. Not a live Grok/Claude/Codex walk.

Engine: `/tmp/crucible-sdd-wm` (commits `cc03300` + NO-BUILD PASS refuse).
Empty `HOME`. No `$HOME` `SKILL.md`.

| ID | Shape | Result | Evidence |
| --- | --- | --- | --- |
| E0 | `adopt work --managed` (no flag) | no `wm.sh` | default layout |
| E1 | Vague IDEA only, no specifier CLI | `STOP-ASK NEXT SPEC` rc=1 | does not invent SPEC |
| E2 | Vague IDEA + specifier+scout+maker+reviewer | `CLOSED PASS`, `product/hello.txt` = `hello` | `/tmp/wm-e2.wldtF7` |
| E3 | Product already satisfies falsifier | `CLOSED NO-BUILD` | honest; PASS after no-build red is refused |
| E4 | HIGH map, unsigned, two kinds | `STOP-ASK MAP-HUMAN` | maker not started |
| E5 | Three-slice shop, one `wm loop` | 3 CLOSED rows, `total(["apple","pear"])==3` | `/tmp/wm-val-big.7PNMLq` |
| QS | Example A extra-proof | 46/0 | `scripts/verify-working-mode-quickstart.sh` |
| K | Kernel CHECKs | 305/0 | `scripts/verify-working-mode.sh` |

Drivers (scratch, not shipped): `/tmp/wm-experiments.sh`, `/tmp/wm-validate-projects.sh`.

Still not claimed: live four-CLI independence, CROSS-FAMILY, 1c default, guided `drive` stall, 1.7.1 tag.
