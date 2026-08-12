# Auto-Research: cold-start independence (promotion packet)

## Target
Crucible guided-cycle cold-start multi-agent independence (comment-complete Rev 2).

## Baseline
`main` @ pre-change 1.1.0: anti-interview BOOTSTRAP, no CONFIGURE gate, placeholder agents, solo theatre legal.

## Candidate (Rev 1 + Rev 2 + Rev 3 role casting)

### Rev 1
- Panel gate, placeholders, docs SSOT, transport enum, subagent after ACP failure

### Rev 2 (comment-driven)
- `contract-audit ATTEMPT AUDITOR PASS|FIX|STOP` — auditor ≠ attempt agent; makers cannot audit reviews
- Structural contract checks (bound attempt, follow-it-exactly, role sections)
- Multi-agent preferred when ≥2 kinds unless `LADDER_WAIVER: single-product` in PANEL
- Guided claim TRUE requires claim-auditor/scout dispatch contract
- `probe-acp ok|failed|unavailable`
- Package script expects coldstart verifier + contract-auditor role (after commit)

### Rev 3 (operator casting restore)
- Compact configure asks **agents inventory + persona→agent casting**
- `PANEL.ASSIGN.tsv` required; content-bound with panel approval
- Guided dispatch / claim-auditor / contract-audit must match casting
- Maker≠reviewer and coordinator≠maker/reviewer unless waived

## Scorecard (local WIP, uncommitted)
| Metric | Result |
| --- | --- |
| verify-coldstart-independence | pass |
| verify-agent-cycle | pass |
| verify-attempt-ledger | pass |
| verify-managed-lifecycle | pass |
| verify-task-dag | pass |
| selftest --fast | see latest run |

## Promotion decision
**promote** for fixture hard gates of Rev 2 (WP1–WP3), with honesty:

- Raises cost of independence theatre; does **not** prove multi-agent under one OS user.
- Live multi-CLI / real ACP launch is optional soft smoke, not CI default.
- Package verify of new scripts requires commit (git archive).

## Residual
- Transport remains process discipline (label + PID start), not cryptographic launch proof.
- Item-file / non-guided managed programs skip guided independence gates.
- Full semantic contract R/W remains persona RULE; shell enforces structural minimums.
