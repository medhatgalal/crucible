# Working-mode experiment log (2026-09-16)

Operator extra-proof against **1.14.2**. Fixture CLIs for A/B/blank-home.
Live grok/kiro-cli/codex on the host PATH.
Empty `HOME` for fixture walks. Live kiro probe/exec inherit host HOME
(keychain). No `$HOME` skill install.

| ID | Shape (docs/working-mode.md) | Result | Evidence |
| --- | --- | --- | --- |
| E0 | `adopt work --managed` (no flag) | no `wm.sh` | blank-home + CI adopt |
| A | Example A LOW hello `loop` | `CLOSED PASS`, `hello` | `verify-working-mode-quickstart.sh` **46/0** |
| B | Example B HIGH unsigned | `STOP-ASK MAP-HUMAN`, no maker | same script **46/0** |
| B-sign | HIGH + fixture `MAP-HUMAN` | maker-falsify starts; never fake CROSS-FAMILY | map CI (`t-high-one-kind` / `t-high-two-kind`) |
| C | Vague IDEA + specifier/scout fixtures | SPEC/MAP then brick | kernel `verify-working-mode.sh` **425/0** (PR #32) |
| NO-BUILD | Product already matches falsifier | `CLOSED NO-BUILD`; PASS after no-build red refused | kernel |
| E5 | Several modules, `depends_on`, one loop | 3 CLOSED slices | `verify-working-mode-blank-home.sh` **102/0** |
| Live | grok + kiro-cli + codex, health-check IDEA | `CLOSED PASS`, four distinct PIDs, `CROSS-FAMILY` | `verify-working-mode-live.sh` **53/0** (1.14 was **52/0** with kiro dropped) |
| T | Clone `sharkdp/tinytag`, branch, `adopt --working-mode`, LOW `TINYTAG_DIR` | `CLOSED PASS`, `CROSS-FAMILY`, `python3 tests/test_tinytag.py` rc 0, no `~/.tinytag` | local branch `feat/tinytag-dir-env` |
| QS-home | empty HOME stays skill-free | pass | quickstart + blank-home + kernel |

Live 1.14.1: kiro host-HOME `chat --no-interactive` probe succeeds (the extra `ok` vs 1.14). With three kinds on PATH, specifier/maker are grok and scout/reviewer are kiro. `kiro-cli acp` is not the wm argv.

Still not claimed: default-on working-mode (1c), unattended HIGH/live (8c), parallel in-slice TASKS, working-mode clearing guided `WAIT APPROVAL`.

## Historical (2026-09-11)

Fixture-only runs against `feat/self-contained-working-mode` (pre-live).
Kept so the first extra-proof table is not rewritten.

| ID | Shape | Result |
| --- | --- | --- |
| E0 | `adopt` without `--working-mode` | no `wm.sh` |
| E1 | Vague IDEA, no specifier CLI | `STOP-ASK NEXT SPEC` |
| E2 | Vague IDEA + four fixture workers | `CLOSED PASS` hello |
| E3 | Already-green product | `CLOSED NO-BUILD` |
| E4 | HIGH unsigned | `STOP-ASK MAP-HUMAN` |
| E5 | Three-slice shop | 3 CLOSED |
| QS | Example A | 46/0 |
| K | Kernel | 305/0 |
