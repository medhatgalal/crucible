# Crucible forensic investigation

## 1. Executive verdict

**FACT.** Crucible is a dependency-free, file-backed gate for agentic software work. It turns a problem document into independently audited claims, an operator-approved backlog, and staged items that require evidence and verdicts to close ([README.md](../../README.md):1-12, 43-61; [LOOP.md](../../LOOP.md):15-93).

**FACT.** The recent effort produced 16 commits from 7–10 August, but no item is closed. There are 21 open items; `ci-on-push` remains in `VERIFY`, `scout-arg-in-docs` remains in `ADVERSARY`, and the rest are in `SPEC`. The current run ledger contains 27 `RETURNED`, 14 `UNDECIDED`, seven `TIMEOUT`, one `HUNG`, four `ABANDONED`, two `STOPPED`, one `SUPERSEDED`, and one `RUNNING` run.

**INFERENCE.** The central failure was not that the repository lacks a lifecycle. It is that a narrow documentation/parser defect was repeatedly treated as a whole-system proof obligation. Every commit invalidated expensive evidence, multiple agents reran overlapping long checks, and the item accumulated provider, parser, documentation, and CI activation requirements. The result was high coordination cost with no closed outcome.

**RECOMMENDATION.** Retain the admission and closure safeguards, but replace the middle of the lifecycle with small, criterion-scoped evidence and explicit cost/stop rules. Do not attempt another broad repair until the operator accepts the remediation backlog in section 9.

## 2. What Crucible is and its intended contract

### Intended flow

```text
problem report
  -> atomic claims with source text
  -> independent audit and scout
  -> operator triage and backlog admission
  -> one item: spec -> design -> tasks -> build -> verify -> adversary -> close
```

**FACT.** Admission requires auditor evidence and a scout result; `claim admit` rejects a claim without those conditions ([crucible](../../crucible):673-769). `check` verifies a written falsifier, maker record, required phase artifacts, current evidence, registered verdict authors, a judge's own evidence, and minimum judge/kind thresholds ([crucible](../../crucible):885-1005).

**FACT.** The repository correctly distinguishes hard closure checks from advisory middle phases: phase transitions require named files, but verdicts do not gate every transition ([LOOP.md](../../LOOP.md):6-11; [crucible](../../crucible):521-543).

## 3. Current implementation and enforcement map

The executable is 1,053 lines of POSIX shell, supported by a 993-line self-test and ten role files. The central item alone has grown to a 1,368-line `ITEM.md` plus a 421-line `TASKS.md`, which is larger than the gate implementation itself.

### Enforcement map

| Contract | Classification | Evidence |
| --- | --- | --- |
| Work-id-bound evidence and verdicts | HARD CHECK | `cmd_check` rejects stale filenames and body IDs ([crucible](../../crucible):941-955, 982-993). |
| Registered author, no maker self-review, distinct verdict content | HARD CHECK | ([crucible](../../crucible):962-1002). |
| Claim audit and scout before admission | HARD CHECK | ([crucible](../../crucible):695-769). |
| Dispatch must precede verdict; task file ownership; bounded iterations | PROSE RULE | Explicitly labelled RULE, not CHECK ([RULES.md](../../RULES.md):46-54, 90-98). |
| Independent human/model authorship of evidence | UNPROVEN by design | The README says one actor can author every verdict and a header is not a signature ([README.md](../../README.md):96-119). |
| Workflow parser sees all valid YAML/GitHub Actions forms | PARTIALLY ENFORCED | The AWK recognises only selected block and single-line `run:` shapes ([scripts/selftest.sh](../../scripts/selftest.sh):492-506). |

### Current delivery position

**FACT.** At work ID `a2097294669b`, `j1`, `j2`, and `adv` have live PASS verdicts, while `cx` has a live REJECT. The rejection is concrete: F4's evidence parser treats unrelated digit sequences on a line containing the head SHA as provider run IDs and rejects the genuine evidence set. A specifier run is currently constructing a tenth F4 harness to revise or disambiguate that test.

**FACT.** Two duplicate judge launches were marked `STOPPED` seconds after launch because PASS verdicts from those agents already existed and the new calls would have overwritten them. This preserved the artifacts, but only after spending another dispatch cycle.

**INFERENCE.** The product tree is not currently closeable. The active dispute is no longer about the shipped two-file change; it is about how its proof harness parses and credits provider evidence. That is precisely the verification-system work that should be separated from the original delivery item.

## 4. Four-day timeline and outcome accounting

| Date | FACT | Outcome |
| --- | --- | --- |
| Aug 7 | Six commits strengthened document sweeps and the scout arity check. | `scout-arg-in-docs` later reached adversary review but remains open. |
| Aug 8 | Five commits added an inline-workflow pin and removed a duplicated smoke-test step. | `ci-on-push` shifted from a CI premise to parser/document-boundary work. |
| Aug 9 | Three commits narrowed README and test comments after review found overclaims. | The item gained more criteria and revalidation obligations. |
| Aug 10 | Two further comment/README changes changed the work ID again. Later reviews split 3 PASS to 1 REJECT over the F4 proof parser. | Prior evidence became stale; the item returned to specification work while still labelled `VERIFY`. |

**FACT.** Any commit on the item's branch changes its work ID, causing old evidence and verdicts to be rejected ([README.md](../../README.md):75-86; [crucible](../../crucible):941-955, 982-984). This is correct for code changes, but it also applies to wording-only changes on the same item.

**FACT.** The open item mandates a broad F1 matrix and says a fast suite costs roughly 2.75 minutes per row, with approximately 18 rows—about 50 minutes per agent—before the complete provider and full-suite checks. This is recorded in `.crucible/self/items/ci-on-push/TASKS.md` under T3. A judge run on 10 August explicitly timed out after 5,409 seconds before producing its required stamped falsifier evidence.

**FACT.** The run history contains 17 judge, 14 specifier, 12 maker, five adversary, four planner, four integrator, and one scout invocation. Most effort therefore occurred after reality-checking, in repeated specification, making, and judging of one item.

**FACT.** At report refresh time, only the `ci-on-push` specifier run is marked `RUNNING`. It was not interrupted for this investigation.

## 5. Why the workflow stalled, including the Opus analysis

### Confirmed causes

1. **One item carries four distinct concerns.** `ci-on-push` combines provider proof, inline-workflow parsing, a documentation correction, and test-suite activation. Its `ITEM.md` was repeatedly revised to narrow false statements rather than completing a stable small requirement. This makes a comment correction invalidate proof of unrelated provider behavior.
2. **Evidence invalidation has no criterion-level reuse.** Work-id freshness is applied wholesale, not by the files or acceptance criteria changed. A good anti-staleness control became an expensive loop because every commit required re-running unrelated proof.
3. **The cost of the canonical test was not a gate.** The task recorded a 50-minute matrix, but no enforced budget, deduplication rule, or escalation mechanism stopped multiple agents from undertaking overlapping executions.
4. **The workflow parser is deliberately partial.** It opens selected `run:` spellings, while the adversary documented multiple silent variants. The current README now accurately limits the guarantee to forms “the check opens” ([README.md](../../README.md):113-116); broad prose assertions had repeatedly outpaced the parser.
5. **The middle lifecycle is guided by prose.** Rules say that repeated findings and unchanged resubmissions should stop, but no code counts or refuses those conditions ([RULES.md](../../RULES.md):90-98).
6. **Review findings can redefine the proof instead of returning a bounded fix.** The current F4 disagreement triggered another specifier and another harness even though the source work ID did not change. The workflow has no enforced distinction between a product defect, a bad falsifier, and a disagreement about evidence interpretation.

### Opus-specific conclusion

**FACT.** Opus is configured as the coordinator, several specialist roles, and one judge in the local panel; this is the same model family across those roles. The panel also has Grok and Codex reviewers in separate families.

**INFERENCE.** The observed difficulty is primarily a system-design failure, not evidence that Opus cannot create the desired flow. Opus was asked to operate inside an instruction-heavy lifecycle where it had to preserve durable proofs, avoid implementation as orchestrator, satisfy changing revisions, and repeatedly demonstrate broad end-to-end behavior. Fresh contexts and model diversity can reduce correlated mistakes, but they cannot compensate for a unit of work that is too large or a test budget with no stopping rule.

**INFERENCE.** Same-family agents likely reinforced the evolving framing. This is a correlation risk, not proof of an individual bad decision; the cross-family adversary did uncover several overclaims, which validates selective diversity rather than universal multi-agent ceremony.

## 6. What went right

- **FACT:** Claim auditing invalidated the original assertion that CI had never run on push; the item records that prior premise as false rather than building against it.
- **FACT:** Work-id binding, judge-owned evidence, and non-PASS outcomes prevented a false closure even while the source tree looked increasingly polished.
- **FACT:** Adversarial review exposed overclaims in documentation and in the workflow assertion. The shipped README now states the limitation instead of claiming complete workflow-form coverage.
- **FACT:** The durable file ledger makes the investigation possible: commits, item revisions, dispatch records, evidence, verdicts, and run outcomes can be correlated after context loss.

## 7. What went wrong

- A verifier was asked to prove a broad semantic property with a hand-rolled text classifier. Pinning selected extracted blocks can detect drift, but it is not a YAML/GitHub Actions parser.
- Repeated full-suite checks measured persistence, not new information. A review must independently test the changed acceptance criterion; it need not duplicate an unchanged long matrix at the same work ID.
- Documentation repair and behavior verification shared one work ID. This made accurate narrowing of claims operationally costly.
- The system correctly labels many requirements RULE rather than CHECK, but dispatches can still make those rules sound mandatory while the executable cannot detect violation.
- No delivery-level metric existed: agents could remain active, return lengthy artifacts, or time out without a limit on retries or a requirement to close, split, or escalate the item.
- Duplicate dispatch prevention is reactive. The orchestrator correctly stopped two redundant judge calls, but the system did not refuse them before launch.

## 8. Replacement operating model

### Minimal flow

1. **Intake:** Split a report into atomic claims; record each source sentence.
2. **Reality check:** One investigator validates each claim and one scout looks for existing behavior. Use a second model family only if a claim is high-risk or disputed.
3. **Operator decision:** Admit, drop, merge, or defer claims in a compact evidence table. No work starts without this decision.
4. **One bounded item:** Each item names one behavior, one owner, touched files, a discriminating falsifier, and a time budget. Split documentation truthfulness from parser completeness and provider activation.
5. **Maker:** Makes the smallest change, runs focused tests, and records a machine-readable result.
6. **Reviewer:** In a fresh context, independently executes the item’s falsifier. Label same-family review as such. Require another model family for behavior, security, data, migration, or a repeated finding.
7. **Close or escalate:** Reuse evidence only when the criterion and relevant tree fingerprint are unchanged. One infrastructure retry is permitted; a second is `BLOCKED`, not another silent dispatch.

### State and result contract

Use one `STATE.md` as the authoritative program queue, plus one immutable result per role:

```json
{
  "item": "slug",
  "role": "maker|reviewer|adversary",
  "tree_fingerprint": "commit plus relevant-file hashes",
  "criterion": "A1",
  "outcome": "PASS|REJECT|BLOCKED|NEEDS_CONTEXT",
  "evidence": ["path"],
  "cost_minutes": 0,
  "next": "close|fix|escalate"
}
```

The coordinator only schedules and synthesizes this state. It does not implement or self-judge. The adversary is a risk-triggered role, not a mandatory phase for typo/document-only work.

## 9. Prioritized remediation backlog

| Priority | Item | Acceptance and falsifier | Cost/stop rule |
| --- | --- | --- | --- |
| P0 | Split `ci-on-push` into provider activation, opened-form pin behavior, and documentation accuracy. | Each child has one changed surface and one focused falsifier; a comment edit cannot invalidate provider proof. | Stop after triage; do not modify the current open item without operator approval. |
| P0 | Add a criterion-scoped evidence index. | Reuse is allowed only when the criterion’s declared relevant-file hash and command version match. Modify a relevant file and prove reuse is rejected. | One short fixture test; no provider call. |
| P1 | Enforce retry/timeout escalation. | The second timeout for a criterion records `BLOCKED` and prevents another identical dispatch until the operator changes budget or method. | Use a synthetic command that exceeds a tiny configured budget. |
| P1 | Add lifecycle accounting. | `next` reports attempts, unique criteria, elapsed budget, and explicit escalation status. | A fixture with repeated results changes status from retryable to blocked. |
| P1 | Replace hand-rolled workflow completeness claims with a narrow declared grammar or a real parser. | Either every supported syntax is parsed by fixtures, or unsupported syntax fails closed/appears as an explicit scope boundary. | Decide after a spike; do not silently widen regexes. |
| P2 | Convert only high-value RULEs to CHECKs. | Start with dispatch-before-verdict and repeated-finding escalation; prove mutations fail. | Reject changes that add ceremony without an executable refusal. |

## 10. Evidence appendix

- Repository identity at start: `/Users/medhat.galal/Desktop/crucible`, branch `ai/ci-on-push`, HEAD `a2097294669b3c88cb51619131ff49caefc46733`.
- Source: `README.md`, `RULES.md`, `LOOP.md`, `crucible`, `scripts/selftest.sh`, and `.github/workflows/selftest.yml` cited above.
- Historical period inspected: 7–10 August 2026, 16 commits.
- Run ledger: `.crucible/self/run/*/state`; current counts and live runs are stated in sections 1, 3, and 4.
- Item evidence: `.crucible/self/items/ci-on-push/` and `.crucible/self/items/scout-arg-in-docs/`.
- Live verdicts inspected: `adv.md`, `j1.md`, and `j2.md` PASS; `cx.md` REJECT, all at `a2097294669b`.
- No new full self-test was started because the prompt forbids long duplicate checks and an item run remained active. Existing recorded evidence establishes prior clean suites and a green provider run at this work ID, but the live F4 rejection means the item is not closeable.

PROCEED WITH REDESIGN PLAN
