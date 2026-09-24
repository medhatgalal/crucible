//! Claim ledger: add, list, verdict, scout, and admit.
//!
//! Replacing a verdict names `history/{who}.{ts}.md` with `ts` = [`Clock::now_unix`],
//! the shell's `date +%s` at the move. Not `SystemTime`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use crucible_contract::Clock;

use crate::cycle::{
    attempt_contract_audit_pass, attempt_dirs_a, attempt_event, attempt_state, attempt_transport,
    claim_agent_attempt, claim_agent_evidence_file, claim_agent_evidence_ok,
    claim_agent_independence_ok, claim_attempt_matches, claim_verdict_word, file_name,
    program_field, read_dir_paths, self_path, workid,
};
use crate::panel::{
    approval_current, contains_ci, guided_min_auditors, guided_min_auditors_label, is_posint,
    kind_of, min_kinds, panel_approval_current, split_tabs, transport_ladder_ok,
};
use crate::program::{uses_guided_cycle, uses_managed_lifecycle};
use crate::state::{state_add_item, state_validate_file};
use crate::{message, records, GuidedError};

/// Shared with the cycle tests that swap `CRUCIBLE_MIN_*` for one assertion.
#[allow(dead_code)]
pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());

const CLAIMS_SEED: &str = "\
# CLAIMS

One heading per finding, quoting its source sentence verbatim.
Nothing becomes an item until it survives audit.
";

/// `crucible claim`. Stdout on success. Refusals are the shell `die` strings.
pub fn claim(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    let (sub, rest) = match args.split_first() {
        Some((sub, rest)) => (*sub, rest),
        None => ("", &[][..]),
    };
    // The ledger creates itself before the subcommand is checked, including usage failures.
    let ledger = ensure_claims(root)?;
    match sub {
        "add" => claim_add(root, rest),
        "list" => claim_list(&ledger),
        "verdict" => claim_verdict(root, clock, rest),
        "scout" => claim_scout(root, clock, rest),
        "admit" => claim_admit(root, clock, rest),
        _ => Err(message(
            "usage: crucible claim add|list|verdict|admit|scout ...",
        )),
    }
}

/// Why the first TRUE file does not count. `None` when every TRUE file counts.
pub(crate) fn claim_true_uncounted(root: &Path, cn: &str) -> Result<Option<String>, GuidedError> {
    let dir = root.join("claims").join(cn).join("verdicts");
    if !dir.is_dir() {
        return Ok(None);
    }
    for path in glob_md(&dir) {
        if claim_verdict_word(&path) != Some("TRUE") {
            continue;
        }
        let agent = file_stem(&path);
        if kind_of(root, &agent)?.is_empty() {
            return Ok(Some(format!(
                "{agent} has no agents.tsv row, so its verdict carries no model kind"
            )));
        }
        if !claim_agent_evidence_ok(root, cn, &agent) {
            return Ok(Some(format!(
                "{agent} recorded no usable evidence for {cn} — run: {} run-claim {cn} {agent} -- <command>",
                self_path(root)
            )));
        }
        if !uses_guided_cycle(root)? || claim_agent_independence_ok(root, cn, &agent)? {
            continue;
        }
        if !panel_approval_current(root)? {
            return Ok(Some(format!(
                "the agent panel is not current — run: {} cycle approve-panel",
                self_path(root)
            )));
        }
        let id = claim_agent_attempt(root, cn, &agent)?;
        if id.is_empty() {
            return Ok(Some(format!(
                "{agent} has no matching claim attempt on the independence ledger — run: {} dispatch {cn} claim-auditor {agent}",
                self_path(root)
            )));
        }
        if attempt_transport(root, &id)?.is_none() {
            return Ok(Some(format!(
                "{agent} resolves to attempt {id}, which has no transport — run: {} attempt transport {id} <multi-agent|acp|subagent> while DISPATCHED",
                self_path(root)
            )));
        }
        let transport = attempt_transport(root, &id)?.unwrap_or_default();
        if transport_ladder_ok(root, &transport)? != 0 {
            return Ok(Some(format!(
                "{agent} resolves to attempt {id}, sealed on {transport} while the ACP probe read failed; the probe now reads ok, so the ladder no longer accepts that seal. Terminal — the probe refuses a downgrade, transport is only recordable while DISPATCHED, and a redispatch resolves to the same earliest attempt. claim admit refuses on this verdict too, so a further auditor does not unblock it: {cn} cannot be admitted while {agent} reads TRUE. Re-file the finding as a new claim, or abandon the investigation"
            )));
        }
        if !attempt_contract_audit_pass(root, &id)? {
            let auditor = suggest_contract_auditor(root)?;
            return Ok(Some(format!(
                "{agent} resolves to attempt {id}, which has no contract-audit PASS — run: {} contract-audit {id} {auditor} PASS while DISPATCHED",
                self_path(root)
            )));
        }
    }
    Ok(None)
}

pub(crate) fn min_kinds_label() -> Result<String, GuidedError> {
    let n = min_kinds()?;
    match std::env::var("CRUCIBLE_MIN_KINDS") {
        Ok(raw) if !raw.is_empty() => Ok(raw),
        _ => Ok(n.to_string()),
    }
}

pub(crate) fn glob_md(dir: &Path) -> Vec<PathBuf> {
    glob_ending(dir, ".md")
}

fn ensure_claims(root: &Path) -> Result<PathBuf, GuidedError> {
    let path = root.join("CLAIMS.md");
    if !path.is_file() {
        fs::write(&path, CLAIMS_SEED)?;
    }
    Ok(path)
}

fn claim_add(root: &Path, args: &[&str]) -> Result<String, GuidedError> {
    let txt = args.first().copied().unwrap_or("");
    let src = args.get(1).copied().unwrap_or("");
    let pol_arg = args.get(2).copied().unwrap_or("");
    if txt.is_empty() {
        return Err(message(
            "usage: crucible claim add \"CLAIM\" \"EXACT SOURCE SENTENCE\" [ABSENT|EXISTS|DEFECT]",
        ));
    }
    if src.is_empty() {
        return Err(message(
            "refused: a claim needs its source sentence quoted verbatim",
        ));
    }
    if uses_guided_cycle(root)? && !panel_approval_current(root)? {
        return Err(message(
            "refused: approve the agent panel before investigation (cycle approve-panel)",
        ));
    }
    let (max_n, max_label) = max_new_claims()?;
    let path = root.join("CLAIMS.md");
    let text = fs::read_to_string(&path)?;
    if count_exact(&text, "    status: NEW") as u64 >= max_n {
        return Err(message(format!(
            "refused: at most {max_label} NEW claims (CRUCIBLE_MAX_NEW_CLAIMS); audit or drop before adding more"
        )));
    }
    if claim_looks_bundled(txt) {
        return Err(message(
            "refused: claim must be one predicate — do not bundle verbs or clauses (and/+/,/;)",
        ));
    }
    let pol = if pol_arg.is_empty() {
        let inferred = claim_infer_polarity(txt);
        if inferred == "REFUSE-DESIRED" {
            return Err(message(
                "refused: present-tense desired behavior is not a gap — state ABSENT/DEFECT (e.g. \"X is not a CLI verb\")",
            ));
        }
        inferred.to_string()
    } else if matches!(pol_arg, "ABSENT" | "EXISTS" | "DEFECT") {
        pol_arg.to_string()
    } else {
        return Err(message("polarity must be ABSENT, EXISTS, or DEFECT"));
    };
    let n = count_prefix(&text, "### C") + 1;
    let extra = format!(
        "\n### C{n} — {txt}\n\n    source: \"{src}\"\n    polarity: {pol}\n    status: NEW\n    audited-by:\n    scout:\n    item:\n"
    );
    OpenOptions::new()
        .append(true)
        .open(&path)?
        .write_all(extra.as_bytes())?;
    fs::create_dir_all(root.join("claims").join(format!("C{n}")).join("verdicts"))?;
    Ok(format!("C{n}\n"))
}

fn claim_list(path: &Path) -> Result<String, GuidedError> {
    let text = fs::read_to_string(path)?;
    let lines: Vec<&str> = records(&text)
        .into_iter()
        .filter(|line| line.starts_with("### C") || line.starts_with("    status:"))
        .collect();
    Ok(paste_pairs(&lines))
}

fn claim_verdict(root: &Path, clock: &dyn Clock, args: &[&str]) -> Result<String, GuidedError> {
    let n = args.first().copied().unwrap_or("");
    let who = args.get(1).copied().unwrap_or("");
    let word = args.get(2).copied().unwrap_or("");
    if n.is_empty() || who.is_empty() || word.is_empty() {
        return Err(message(
            "usage: crucible claim verdict N AGENT TRUE|FALSE|STALE|UNVERIFIABLE [CITE] [--like C2 C3]",
        ));
    }
    if kind_of(root, who)?.is_empty() {
        return Err(message(format!("unregistered agent: {who}")));
    }
    if !matches!(word, "TRUE" | "FALSE" | "STALE" | "UNVERIFIABLE") {
        return Err(message("verdict must be TRUE FALSE STALE or UNVERIFIABLE"));
    }
    let dir = root.join("claims").join(n).join("verdicts");
    if !dir.is_dir() {
        return Err(message(format!("no such claim: {n}")));
    }
    if !claim_agent_evidence_ok(root, n, who) {
        return Err(message(format!(
            "refused: {who} recorded no usable evidence for {n} — run: crucible run-claim {n} {who} -- <command>"
        )));
    }
    if uses_guided_cycle(root)? {
        require_panel_approval(root)?;
        let auditor = format!("-claim-auditor-{who}.md");
        let scout = format!("-scout-{who}.md");
        let found = find_sealed_attempt(root, n, who, None, &[auditor.as_str(), scout.as_str()])?;
        if !found.dispatched {
            return Err(message(format!(
                "refused: guided claim verdict requires a claim dispatch for {who} — run: {} dispatch {n} claim-auditor {who}",
                self_path(root)
            )));
        }
        let Some(attempt) = found.attempt else {
            return Err(message(format!(
                "refused: guided claim verdict requires a matching claim attempt (item+agent+role) on the independence ledger for {who}"
            )));
        };
        require_attempt_independence(root, &attempt)?;
        supersede_if_dispatched(root, clock, &attempt, &format!("claim-verdict-{word}"))?;
    }
    let (mut cite, likes) = split_cite_likes(&args[3..]);
    if cite.is_empty() {
        if let Some(evidence) = claim_agent_evidence_file(root, n, who) {
            cite = rel_under_root(root, &evidence);
        }
    }
    if cite.is_empty() {
        return Err(message(
            "refused: claim verdict requires a citation (evidence path or quote)",
        ));
    }
    let verdict_path = dir.join(format!("{who}.md"));
    if nonempty_file(&verdict_path) {
        let history = dir.join("history");
        fs::create_dir_all(&history)?;
        // Shell line 5397: `ts=$(date +%s)`, then `history/$who.$ts.md`.
        let ts = clock.now_unix();
        fs::rename(&verdict_path, history.join(format!("{who}.{ts}.md")))?;
    }
    let kind = kind_of(root, who)?;
    fs::write(
        &verdict_path,
        format!("CLAIM-VERDICT: {word}\nAGENT: {who}\nKIND: {kind}\nCITATION: {cite}\n"),
    )?;
    match word {
        "STALE" => rewrite_claim_line(root, n, "status", "STALE")?,
        "FALSE" | "UNVERIFIABLE" => rewrite_claim_line(root, n, "status", "AUDITED_FALSE")?,
        _ => {}
    }
    let mut out = String::new();
    for like in likes {
        match claim_copy_verdict_like(root, n, &like, who)? {
            Some(line) => out.push_str(&line),
            None => {
                return Err(message(format!(
                    "refused: cannot copy {word} onto {like} from {n}"
                )));
            }
        }
    }
    out.push_str(&format!("{}\n", verdict_path.display()));
    Ok(out)
}

fn claim_scout(root: &Path, clock: &dyn Clock, args: &[&str]) -> Result<String, GuidedError> {
    let n = args.first().copied().unwrap_or("");
    let res = args.get(1).copied().unwrap_or("");
    let who = args.get(2).copied().unwrap_or("");
    if n.is_empty() || res.is_empty() {
        return Err(message(
            "usage: crucible claim scout CN ABSENT|PARTLY-EXISTS|FULLY-EXISTS|IN-FLIGHT AGENT",
        ));
    }
    if !matches!(
        res,
        "ABSENT" | "PARTLY-EXISTS" | "FULLY-EXISTS" | "IN-FLIGHT"
    ) {
        return Err(message(
            "scout result must be ABSENT, PARTLY-EXISTS, FULLY-EXISTS, or IN-FLIGHT",
        ));
    }
    let ledger = fs::read_to_string(root.join("CLAIMS.md"))?;
    if !heading_exists(&ledger, n) {
        return Err(message(format!("no such claim: {n}")));
    }
    if who.is_empty() {
        return Err(message(
            "usage: crucible claim scout CN RESULT AGENT — name the scout that searched",
        ));
    }
    if kind_of(root, who)?.is_empty() {
        return Err(message(format!("unregistered agent: {who}")));
    }
    if !claim_agent_evidence_ok(root, n, who) {
        return Err(message(format!(
            "refused: {who} recorded no usable searches for {n} — run: crucible run-claim {n} {who} -- <search>"
        )));
    }
    if res == "ABSENT" {
        refuse_absent_if_inflight(root)?;
    }
    if uses_guided_cycle(root)? {
        require_panel_approval(root)?;
        let suffix = format!("-scout-{who}.md");
        let found = find_sealed_attempt(root, n, who, Some("scout"), &[suffix.as_str()])?;
        if !found.dispatched {
            return Err(message(format!(
                "refused: guided scout requires a scout dispatch for {who} — run: {} dispatch {n} scout {who}",
                self_path(root)
            )));
        }
        let Some(attempt) = found.attempt else {
            return Err(message(format!(
                "refused: guided scout requires a matching scout attempt (item+agent+role) on the independence ledger for {who}"
            )));
        };
        require_attempt_independence(root, &attempt)?;
        supersede_if_dispatched(root, clock, &attempt, &format!("claim-scout-{res}"))?;
    }
    rewrite_claim_line(root, n, "scout", res)?;
    Ok(format!("{n} scout: {res} (by {who})\n"))
}

fn claim_admit(root: &Path, clock: &dyn Clock, args: &[&str]) -> Result<String, GuidedError> {
    let n = args.first().copied().unwrap_or("");
    let slug = args.get(1).copied().unwrap_or("");
    if n.is_empty() || slug.is_empty() {
        return Err(message("usage: crucible claim admit CN SLUG"));
    }
    let dir = root.join("claims").join(n).join("verdicts");
    if !dir.is_dir() {
        return Err(message(format!("no such claim: {n}")));
    }
    let mut auditors = 0u64;
    let mut kinds = Vec::new();
    for path in glob_md(&dir) {
        let vn = file_stem(&path);
        if kind_of(root, &vn)?.is_empty() {
            eprintln!("ignoring unregistered verdict file: {vn}");
            continue;
        }
        if !claim_agent_evidence_ok(root, n, &vn) {
            eprintln!("ignoring {vn}: no usable evidence for {n}");
            continue;
        }
        if claim_verdict_word(&path) != Some("TRUE") {
            continue;
        }
        let kind = kind_of(root, &vn)?;
        if !kinds.iter().any(|seen| seen == &kind) {
            kinds.push(kind);
        }
        auditors += 1;
        if uses_guided_cycle(root)? {
            let ca = claim_agent_attempt(root, n, &vn)?;
            if ca.is_empty() {
                return Err(message(format!(
                    "refused: admit requires a sealed claim attempt for TRUE auditor {vn}"
                )));
            }
            require_attempt_independence(root, &ca)?;
        }
    }
    let need = guided_min_auditors(root)?;
    let need_label = guided_min_auditors_label(root)?;
    if auditors < need {
        return Err(message(format!(
            "refused: {n} has {auditors} TRUE verdicts, need {need_label}"
        )));
    }
    let need_kinds = min_kinds()?;
    let kinds_label = min_kinds_label()?;
    if (kinds.len() as u64) < need_kinds {
        return Err(message(format!(
            "refused: {n} audited by {} kind(s), need {kinds_label}",
            kinds.len()
        )));
    }
    let ledger = fs::read_to_string(root.join("CLAIMS.md"))?;
    let scout = claim_field_named(&ledger, n, "scout").unwrap_or_default();
    let mut pol = claim_field_named(&ledger, n, "polarity").unwrap_or_default();
    if pol.is_empty() {
        pol = "ABSENT".to_string();
    }
    match scout.as_str() {
        "FULLY-EXISTS" => {
            return Err(message(format!(
                "refused: the scout found {n} already implemented. Drop it, or record a narrowed claim for the gap."
            )));
        }
        "IN-FLIGHT" => {
            return Err(message(format!(
                "refused: {n} is IN-FLIGHT on an unmerged branch — merge or drop, do not admit"
            )));
        }
        "ABSENT" | "PARTLY-EXISTS" => {}
        _ => {
            return Err(message(format!(
                "refused: {n} has no scout report. Dispatch a scout, then: crucible claim scout {n} ABSENT|PARTLY-EXISTS|FULLY-EXISTS|IN-FLIGHT AGENT"
            )));
        }
    }
    match pol.as_str() {
        "EXISTS" => {
            return Err(message(
                "refused: polarity EXISTS is not a gap — drop it or record ABSENT/DEFECT",
            ));
        }
        "ABSENT" if scout != "ABSENT" => {
            return Err(message(format!(
                "refused: polarity ABSENT requires scout ABSENT (got {scout})"
            )));
        }
        "DEFECT" if scout != "PARTLY-EXISTS" && scout != "ABSENT" => {
            return Err(message(format!(
                "refused: polarity DEFECT requires scout PARTLY-EXISTS or ABSENT (got {scout})"
            )));
        }
        _ => {}
    }
    if uses_managed_lifecycle(root)? {
        if uses_guided_cycle(root)? {
            require_panel_approval(root)?;
        }
        if !approval_current(root)? {
            return Err(message(
                "refused: the current PROPOSAL.md is not operator-approved",
            ));
        }
    }
    let mut out = String::new();
    if root.join("items").join(slug).is_dir() {
        if uses_managed_lifecycle(root)? && first_active_slug(root).as_deref() != Some(slug) {
            return Err(message(format!(
                "refused: item {slug} exists but is not the current ACTIVE item"
            )));
        }
    } else {
        out.push_str(&add_item(root, clock, slug, &claim_title(&ledger, n))?);
    }
    rewrite_claim_line(root, n, "item", slug)?;
    if root.join("PROGRAM").is_file() {
        if let Some(repo) = program_field(root, "repo") {
            let base = program_field(root, "base").unwrap_or_else(|| "main".to_string());
            let _ = cmd_target(root, slug, &repo, &format!("ai/{slug}"), &base);
        }
    }
    rewrite_claim_line(root, n, "status", "ADMITTED")?;
    out.push_str(&format!("admitted {n} as item {slug}\n"));
    Ok(out)
}

fn add_item(
    root: &Path,
    clock: &dyn Clock,
    slug: &str,
    title: &str,
) -> Result<String, GuidedError> {
    if slug.is_empty() {
        return Err(message("usage: crucible add SLUG \"TITLE\""));
    }
    if !valid_slug(slug) {
        return Err(message("slug may only contain A-Za-z0-9._-"));
    }
    let managed = uses_managed_lifecycle(root)?;
    if managed {
        state_validate_file(&root.join("STATE.tsv"))?;
        if count_current_items(root)? != 0 {
            return Err(message("refused: another item is current"));
        }
        if uses_guided_cycle(root)? {
            if !panel_approval_current(root)? {
                return Err(message("refused: the agent panel is not operator-approved"));
            }
            if !approval_current(root)? {
                return Err(message(
                    "refused: the current PROPOSAL.md is not operator-approved",
                ));
            }
        }
    }
    fs::create_dir_all(root.join("items"))?;
    let lessons = root.join("LESSONS.md");
    if !lessons.is_file() {
        fs::write(&lessons, "")?;
    }
    let mut out = String::new();
    if let Some(note) = seed_agents(root)? {
        out.push_str(&note);
    }
    let dir = root.join("items").join(slug);
    if dir.is_dir() {
        return Err(message(format!("item exists: {slug}")));
    }
    fs::create_dir_all(dir.join("work"))?;
    fs::create_dir_all(dir.join("evidence"))?;
    fs::create_dir_all(dir.join("verdicts"))?;
    fs::create_dir_all(dir.join("briefs"))?;
    fs::write(dir.join("ITEM.md"), item_markdown(slug, title, managed))?;
    if let Some(repo) = program_field(root, "repo") {
        let base = program_field(root, "base").unwrap_or_else(|| "main".to_string());
        let _ = cmd_target(root, slug, &repo, &format!("ai/{slug}"), &base);
    }
    if managed {
        let wid = workid(root, slug)?;
        state_add_item(root, clock, slug, &wid, "LOW")?;
    }
    out.push_str(&format!("{}/ITEM.md\n", dir.display()));
    if managed {
        out.push_str(&format!(
            "next: complete the frozen contract, then\n      {} ready {slug}\n",
            self_path(root)
        ));
    } else {
        out.push_str(&format!(
            "next: write the ask, criteria and falsifier into that file, then\n      crucible brief {slug} maker <name>\n"
        ));
    }
    Ok(out)
}

fn item_markdown(slug: &str, title: &str, managed: bool) -> String {
    let shown = if title.is_empty() { "untitled" } else { title };
    let goal = if title.is_empty() {
        "Describe the work, and quote the report sentence that caused this item."
    } else {
        title
    };
    if managed {
        format!(
            "\
# {slug} — {shown}

## Goal

{goal}

## Non-goals

State what this item will not change.

## Risk

LOW

## Owned files

- List literal repository-relative paths before READY.

## Acceptance criteria

- [ ] A1

## Focused falsifier

TEMPLATE-FALSIFIER-UNWRITTEN. Replace this line with one bounded command or script.

## Expensive evidence

NONE

## Stop conditions

Stop and escalate if the frozen contract must change.
"
        )
    } else {
        format!(
            "\
# {slug} — {shown}

PHASE: SPEC
STATUS: OPEN

## The ask

{goal}

## Acceptance criteria

- [ ] A1

## Falsifier

TEMPLATE-FALSIFIER-UNWRITTEN. Replace this line: name the change to undo and the check
that must then fail. The gate refuses closure while this marker is present.
"
        )
    }
}

fn seed_agents(root: &Path) -> Result<Option<String>, GuidedError> {
    let path = root.join("agents.tsv");
    if path.is_file() {
        return Ok(None);
    }
    let mut body = String::from(
        "\
# name\tkind\tmodel\teffort\tcommand   ({BRIEF} {MODEL} {EFFORT} are substituted)
# Only names listed here may author a verdict. Repeat a kind for a same-kind panel.
",
    );
    if have("kiro-cli") {
        body.push_str(
            "\
lead\tkiro\tclaude-opus-5\tmax\tkiro-cli chat --no-interactive --trust-all-tools --model {MODEL} --effort {EFFORT} \"read {BRIEF} and follow it exactly\"
mk1\tkiro\tclaude-sonnet-5\thigh\tkiro-cli chat --no-interactive --trust-all-tools --model {MODEL} --effort {EFFORT} \"read {BRIEF} and follow it exactly\"
j1\tkiro\tclaude-opus-5\thigh\tkiro-cli chat --no-interactive --trust-all-tools --model {MODEL} --effort {EFFORT} \"read {BRIEF} and follow it exactly\"
",
        );
    }
    if have("grok") {
        body.push_str(
            "j2\tgrok\tgrok-4.5\thigh\tgrok -p \"read {BRIEF} and follow it exactly\" --model {MODEL}\n",
        );
    }
    if have("codex") {
        body.push_str(
            "j3\tcodex\tgpt-5-codex\thigh\tcodex exec --skip-git-repo-check -m {MODEL} \"read {BRIEF} and follow it exactly\"\n",
        );
    }
    fs::write(&path, body)?;
    Ok(Some(format!(
        "wrote {} — edit it to match the agents you will actually use\n",
        path.display()
    )))
}

fn cmd_target(
    root: &Path,
    slug: &str,
    repo: &str,
    branch: &str,
    base: &str,
) -> Result<(), GuidedError> {
    let dir = root.join("items").join(slug);
    if slug.is_empty() {
        return Err(message("need a slug"));
    }
    if !dir.is_dir() {
        return Err(message(format!("no such item: {slug}")));
    }
    if repo.is_empty() || branch.is_empty() || base.is_empty() {
        return Err(message("usage: crucible target SLUG REPO BRANCH BASE"));
    }
    let git = Path::new(repo).join(".git");
    if !git.is_dir() && !git.is_file() {
        return Err(message(format!("not a git repo: {repo}")));
    }
    let status = Command::new("git")
        .args(["-C", repo, "rev-parse", "--verify", "--quiet", base])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(message(format!("no such base ref: {base}")));
    }
    fs::write(
        dir.join("TARGET"),
        format!("repo: {repo}\nbranch: {branch}\nbase: {base}\n"),
    )?;
    Ok(())
}

fn claim_looks_bundled(text: &str) -> bool {
    text.split('\n').any(|line| {
        contains_ci(line, " and ")
            || contains_ci(line, "; ")
            || contains_ci(line, " + ")
            || contains_ci(line, " / ")
            || contains_ci(line, " as well as ")
            || contains_ci(line, " along with ")
            || comma_and(line)
    })
}

fn comma_and(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find(", ") {
        let after = from + at + 2;
        if lower[after..].contains(" and ") {
            return true;
        }
        from = from + at + 1;
        if from >= lower.len() {
            break;
        }
    }
    false
}

fn claim_present_desired(text: &str) -> bool {
    let desired = text.split('\n').any(|line| {
        contains_ci(line, " writes ")
            || contains_ci(line, " should ")
            || contains_ci(line, " must ")
            || contains_ci(line, " will ")
            || contains_ci(line, " needs to ")
            || contains_ci(line, " ought to ")
    });
    let negated = text.split('\n').any(|line| {
        contains_ci(line, " not ")
            || contains_ci(line, " never ")
            || contains_ci(line, " no ")
            || contains_isnt(line)
            || contains_ci(line, " does not ")
            || contains_ci(line, " do not ")
            || contains_ci(line, " cannot ")
    });
    desired && !negated
}

fn contains_isnt(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    if bytes.len() < 7 {
        return false;
    }
    (0..=bytes.len() - 7).any(|i| {
        bytes[i] == b' '
            && bytes[i + 1] == b'i'
            && bytes[i + 2] == b's'
            && bytes[i + 3] == b'n'
            && bytes[i + 5] == b't'
            && bytes[i + 6] == b' '
    })
}

fn claim_infer_polarity(text: &str) -> &'static str {
    let lower = text.to_ascii_lowercase();
    if lower.contains(" is not ")
        || lower.contains(" does not exist")
        || lower.contains(" has no ")
        || lower.contains(" missing ")
        || lower.contains(" not a ")
    {
        return "ABSENT";
    }
    if lower.contains(" already ") || lower.contains(" exists ") || lower.contains(" ships ") {
        return "EXISTS";
    }
    if lower.contains(" wrong ")
        || lower.contains(" broken ")
        || lower.contains(" defect")
        || lower.contains(" incorrect ")
    {
        return "DEFECT";
    }
    if claim_present_desired(text) {
        return "REFUSE-DESIRED";
    }
    "ABSENT"
}

fn max_new_claims() -> Result<(u64, String), GuidedError> {
    let raw = std::env::var("CRUCIBLE_MAX_NEW_CLAIMS").unwrap_or_default();
    let label = if raw.is_empty() { "3".to_string() } else { raw };
    if !is_posint(&label) {
        return Err(message(
            "CRUCIBLE_MAX_NEW_CLAIMS must be a positive integer",
        ));
    }
    let n = label
        .parse::<u64>()
        .map_err(|_| message("CRUCIBLE_MAX_NEW_CLAIMS must be a positive integer"))?;
    Ok((n, label))
}

/// `paste - -`: pairs joined by a tab. A short final row still emits the delimiter.
fn paste_pairs(lines: &[&str]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        out.push_str(lines[i]);
        out.push('\t');
        if i + 1 < lines.len() {
            out.push_str(lines[i + 1]);
            i += 2;
        } else {
            i += 1;
        }
        out.push('\n');
    }
    out
}

fn rewrite_claim_line(root: &Path, n: &str, field: &str, value: &str) -> Result<(), GuidedError> {
    let path = root.join("CLAIMS.md");
    if !path.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&path)?;
    let heading = format!("### {n} ");
    let field_prefix = format!("    {field}:");
    let mut inc = false;
    let mut out = String::new();
    for line in records(&text) {
        if line.starts_with(&heading) {
            inc = true;
        }
        if inc && line.starts_with(&field_prefix) {
            out.push_str(&format!("    {field}: {value}\n"));
            inc = false;
            continue;
        }
        if line.starts_with("### C") && !line.starts_with(&heading) {
            inc = false;
        }
        out.push_str(line);
        out.push('\n');
    }
    let tmp = root.join(format!("c.{}.tmp", std::process::id()));
    fs::write(&tmp, out)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn claim_field_named(text: &str, n: &str, field: &str) -> Option<String> {
    let start = format!("### {n} ");
    let prefix = format!("    {field}: ");
    let mut inside = false;
    for line in records(text) {
        if !inside {
            if line.starts_with(&start) {
                inside = true;
            }
            continue;
        }
        if line.starts_with("### C") && line.as_bytes().get(5).is_some_and(|b| b.is_ascii_digit()) {
            break;
        }
        if let Some(value) = line.strip_prefix(&prefix) {
            return Some(value.to_string());
        }
    }
    None
}

fn claim_title(text: &str, n: &str) -> String {
    let prefix = format!("### {n} — ");
    records(text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string()
}

fn heading_exists(text: &str, n: &str) -> bool {
    let prefix = format!("### {n} ");
    records(text).iter().any(|line| line.starts_with(&prefix))
}

struct SealFind {
    dispatched: bool,
    attempt: Option<String>,
}

fn find_sealed_attempt(
    root: &Path,
    cn: &str,
    who: &str,
    role: Option<&str>,
    suffixes: &[&str],
) -> Result<SealFind, GuidedError> {
    let dir = root.join("claims").join(cn).join("dispatches");
    let mut dispatched = false;
    let mut attempt = None;
    if dir.is_dir() {
        for suffix in suffixes {
            for path in glob_ending(&dir, suffix) {
                dispatched = true;
                let text = fs::read_to_string(&path)?;
                let stamped = records(&text)
                    .into_iter()
                    .find_map(|line| line.strip_prefix("attempt-id: "))
                    .unwrap_or("");
                if stamped.is_empty() {
                    continue;
                }
                if claim_attempt_matches(root, stamped, cn, who, role)?
                    && claim_attempt_is_sealed(root, stamped)
                {
                    attempt = Some(stamped.to_string());
                }
            }
        }
    }
    if dispatched && attempt.is_none() {
        attempt = last_ledger_attempt(root, cn, who, role)?;
    }
    Ok(SealFind {
        dispatched,
        attempt,
    })
}

fn last_ledger_attempt(
    root: &Path,
    cn: &str,
    who: &str,
    role: Option<&str>,
) -> Result<Option<String>, GuidedError> {
    let mut last = None;
    for path in attempt_dirs_a(root) {
        let id = file_name(&path);
        if claim_attempt_matches(root, &id, cn, who, role)? && claim_attempt_is_sealed(root, &id) {
            last = Some(id);
        }
    }
    Ok(last)
}

fn claim_attempt_is_sealed(root: &Path, id: &str) -> bool {
    let dir = root.join("attempts").join(id);
    if !dir.join("transport").is_file() || !dir.join("contract-audit.md").is_file() {
        return false;
    }
    let Ok(text) = fs::read_to_string(dir.join("contract-audit.md")) else {
        return false;
    };
    records(&text)
        .iter()
        .any(|line| line.starts_with("VERDICT: PASS"))
}

pub(crate) fn require_panel_approval(root: &Path) -> Result<(), GuidedError> {
    if !uses_guided_cycle(root)? || panel_approval_current(root)? {
        return Ok(());
    }
    Err(message(format!(
        "refused: agent panel is not current — update PANEL.md / PANEL.ASSIGN.tsv / agents.tsv and run: {} cycle approve-panel",
        self_path(root)
    )))
}

pub(crate) fn require_attempt_independence(root: &Path, id: &str) -> Result<(), GuidedError> {
    if !uses_guided_cycle(root)? {
        return Ok(());
    }
    require_panel_approval(root)?;
    let Some(transport) = attempt_transport(root, id)? else {
        return Err(message(format!(
            "refused: attempt {id} has no transport (multi-agent|acp|subagent) — run: {} attempt transport {id} <transport> while DISPATCHED",
            self_path(root)
        )));
    };
    match transport_ladder_ok(root, &transport)? {
        0 => {}
        1 => {
            return Err(message(
                "refused: subagent requires a recorded ACP probe failure (ACP-PROBE.md status: failed, or PANEL notes ACP unavailable)",
            ));
        }
        _ => {
            return Err(message(
                "refused: transport must be multi-agent|acp|subagent",
            ));
        }
    }
    if attempt_contract_audit_pass(root, id)? {
        return Ok(());
    }
    let auditor = suggest_contract_auditor(root)?;
    Err(message(format!(
        "refused: attempt {id} lacks contract-audit PASS — run: {} contract-audit {id} {auditor} PASS while DISPATCHED",
        self_path(root)
    )))
}

fn suggest_contract_auditor(root: &Path) -> Result<String, GuidedError> {
    let path = root.join("PANEL.ASSIGN.tsv");
    if !path.is_file() {
        return Ok("<auditor-name>".to_string());
    }
    let text = fs::read_to_string(path)?;
    for line in records(&text) {
        let fields = split_tabs(line);
        if fields.first().copied() == Some("contract-auditor") {
            let who = fields.get(1).copied().unwrap_or("");
            if !who.is_empty() {
                return Ok(who.to_string());
            }
        }
    }
    Ok(String::new())
}

fn supersede_if_dispatched(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    reason: &str,
) -> Result<(), GuidedError> {
    // `$(attempt_state)` failing does not abort under `set -e`; only DISPATCHED is terminalized.
    if attempt_state(root, id).ok().as_deref() == Some("DISPATCHED") {
        attempt_event(root, clock, id, "SUPERSEDED", "-", reason)?;
    }
    Ok(())
}

fn claim_copy_verdict_like(
    root: &Path,
    from: &str,
    to: &str,
    who: &str,
) -> Result<Option<String>, GuidedError> {
    let src = root
        .join("claims")
        .join(from)
        .join("verdicts")
        .join(format!("{who}.md"));
    if !src.is_file() {
        return Ok(None);
    }
    let Some(word) = claim_verdict_word(&src) else {
        return Ok(None);
    };
    if !matches!(word, "STALE" | "FALSE" | "UNVERIFIABLE") {
        return Ok(None);
    }
    let dest_dir = root.join("claims").join(to).join("verdicts");
    fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(format!("{who}.md"));
    if dest.exists() {
        return Ok(None);
    }
    if uses_guided_cycle(root)? && !ledger_has_sealed(root, to, who)? {
        return Ok(None);
    }
    fs::copy(&src, &dest)?;
    let status = if word == "STALE" {
        "STALE"
    } else {
        "AUDITED_FALSE"
    };
    rewrite_claim_line(root, to, "status", status)?;
    Ok(Some(format!("{}\n", dest.display())))
}

fn ledger_has_sealed(root: &Path, cn: &str, who: &str) -> Result<bool, GuidedError> {
    for path in attempt_dirs_a(root) {
        let id = file_name(&path);
        if claim_attempt_matches(root, &id, cn, who, None)? && claim_attempt_is_sealed(root, &id) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn split_cite_likes(args: &[&str]) -> (String, Vec<String>) {
    let mut cite = String::new();
    let mut likes = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--like" {
            for extra in &args[i + 1..] {
                likes.extend(extra.split_whitespace().map(str::to_string));
            }
            break;
        }
        cite = args[i].to_string();
        i += 1;
    }
    (cite, likes)
}

fn rel_under_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|rel| rel.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn nonempty_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false)
}

fn refuse_absent_if_inflight(root: &Path) -> Result<(), GuidedError> {
    if !root.join("PROGRAM").is_file() {
        return Ok(());
    }
    let repo = program_field(root, "repo").unwrap_or_default();
    let base = program_field(root, "base").unwrap_or_else(|| "main".to_string());
    if !repo_has_git(&repo) {
        return Ok(());
    }
    if let Some(branch) = first_unmerged_ai(&repo, &base) {
        return Err(message(format!(
            "refused: unmerged branch {branch} implements work — record IN-FLIGHT, not ABSENT"
        )));
    }
    Ok(())
}

fn repo_has_git(repo: &str) -> bool {
    let marker = if repo.is_empty() {
        PathBuf::from("/.git")
    } else {
        Path::new(repo).join(".git")
    };
    (!repo.is_empty() && marker.is_dir()) || marker.is_file()
}

fn first_unmerged_ai(repo: &str, base: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", repo, "branch", "--list", "ai/*"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in records(&text) {
        let branch = line.trim_start_matches(['*', ' ']);
        if branch.is_empty() {
            continue;
        }
        if rev_list_count(repo, &format!("{base}..{branch}")) > 0 {
            return Some(branch.to_string());
        }
    }
    None
}

fn rev_list_count(repo: &str, range: &str) -> u64 {
    let Ok(output) = Command::new("git")
        .args(["-C", repo, "rev-list", "--count", range])
        .stderr(Stdio::null())
        .output()
    else {
        return 0;
    };
    if !output.status.success() {
        return 0;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn count_current_items(root: &Path) -> Result<usize, GuidedError> {
    let text = fs::read_to_string(root.join("STATE.tsv"))?;
    let mut n = 0usize;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let status = split_tabs(rec).get(1).copied().unwrap_or("");
        if status == "ACTIVE" || status == "BLOCKED" {
            n += 1;
        }
    }
    Ok(n)
}

fn first_active_slug(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join("STATE.tsv")).ok()?;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields = split_tabs(rec);
        if fields.get(1).copied() == Some("ACTIVE") {
            return Some(fields.first().copied().unwrap_or("").to_string());
        }
    }
    None
}

fn have(bin: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        fs::metadata(dir.join(bin))
            .map(|meta| meta.is_file())
            .unwrap_or(false)
    })
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn count_exact(text: &str, line: &str) -> usize {
    records(text).iter().filter(|row| **row == line).count()
}

fn count_prefix(text: &str, prefix: &str) -> usize {
    records(text)
        .iter()
        .filter(|line| line.starts_with(prefix))
        .count()
}

fn glob_ending(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut paths = read_dir_paths(dir);
    paths.retain(|path| {
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            return false;
        };
        !name.starts_with('.') && name.ends_with(suffix) && path.is_file()
    });
    paths.sort();
    paths
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::{panel_id, proposal_id};
    use crate::state::STATE_HEADER;
    use crucible_contract::FixedClock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::MutexGuard;

    const EPOCH: i64 = 1_700_000_000;

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-claims-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    struct Restore(&'static str, Option<std::ffi::OsString>);

    impl Drop for Restore {
        fn drop(&mut self) {
            // SAFETY: the test holds ENV_LOCK until this guard drops.
            unsafe {
                match self.1.take() {
                    Some(prev) => std::env::set_var(self.0, prev),
                    None => std::env::remove_var(self.0),
                }
            }
        }
    }

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn set_env(key: &'static str, value: &str) -> Restore {
        let prev = std::env::var_os(key);
        // SAFETY: caller holds ENV_LOCK and Restore puts the previous value back.
        unsafe { std::env::set_var(key, value) };
        Restore(key, prev)
    }

    fn unset_env(key: &'static str) -> Restore {
        let prev = std::env::var_os(key);
        // SAFETY: same lock as set_env.
        unsafe { std::env::remove_var(key) };
        Restore(key, prev)
    }

    fn clock() -> FixedClock {
        FixedClock::new(EPOCH)
    }

    fn agents(root: &Path) {
        fs::write(
            root.join("agents.tsv"),
            "a1\tkindA\tm\thigh\ttrue\na2\tkindA\tm\thigh\ttrue\n",
        )
        .unwrap();
    }

    fn evidence(root: &Path, cn: &str, agent: &str) {
        let dir = root.join("claims").join(cn).join("evidence");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("{agent}.manual.txt")),
            "crucible-run/1\nchecked\n",
        )
        .unwrap();
    }

    fn approve_panel(root: &Path) {
        fs::write(root.join("PANEL.md"), "panel\n").unwrap();
        fs::write(root.join("PANEL.ASSIGN.tsv"), "role\tagent\n").unwrap();
        let id = panel_id(root).unwrap().unwrap();
        fs::write(root.join("PANEL.APPROVAL"), format!("panel-id: {id}\n")).unwrap();
    }

    #[test]
    fn add_lists_polarity_and_refuses_bundles() {
        let _guard = env_lock();
        let _max = set_env("CRUCIBLE_MAX_NEW_CLAIMS", "3");
        let tmp = Tmp::new();
        let root = tmp.root.as_path();
        let clock = clock();
        assert_eq!(
            claim(root, &[], &clock).unwrap_err().to_string(),
            "usage: crucible claim add|list|verdict|admit|scout ..."
        );
        assert!(root.join("CLAIMS.md").is_file());

        assert_eq!(
            claim(root, &["add", "the widget is not present"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: a claim needs its source sentence quoted verbatim"
        );
        assert_eq!(
            claim(
                root,
                &["add", "reads and writes the file", "source sentence"],
                &clock,
            )
            .unwrap_err()
            .to_string(),
            "refused: claim must be one predicate — do not bundle verbs or clauses (and/+/,/;)"
        );
        assert_eq!(
            claim(
                root,
                &["add", "it should write a file", "source sentence"],
                &clock,
            )
            .unwrap_err()
            .to_string(),
            "refused: present-tense desired behavior is not a gap — state ABSENT/DEFECT (e.g. \"X is not a CLI verb\")"
        );
        assert_eq!(
            claim(
                root,
                &[
                    "add",
                    "the widget is not present",
                    "source sentence",
                    "MAYBE"
                ],
                &clock,
            )
            .unwrap_err()
            .to_string(),
            "polarity must be ABSENT, EXISTS, or DEFECT"
        );

        assert_eq!(
            claim(
                root,
                &["add", "the widget is not present", "No widget is present."],
                &clock,
            )
            .unwrap(),
            "C1\n"
        );
        assert_eq!(
            claim(
                root,
                &[
                    "add",
                    "parser already ships the flag",
                    "The parser already ships the flag.",
                ],
                &clock,
            )
            .unwrap(),
            "C2\n"
        );
        assert_eq!(
            claim(root, &["add", "anything", "quoted", "DEFECT"], &clock,).unwrap(),
            "C3\n"
        );
        let ledger = fs::read_to_string(root.join("CLAIMS.md")).unwrap();
        assert!(ledger.contains("    polarity: ABSENT\n"));
        assert!(ledger.contains("    polarity: EXISTS\n"));
        assert!(ledger.contains("    polarity: DEFECT\n"));
        assert!(root.join("claims/C1/verdicts").is_dir());
        assert_eq!(
            claim(root, &["list"], &clock).unwrap(),
            "\
### C1 — the widget is not present\t    status: NEW
### C2 — parser already ships the flag\t    status: NEW
### C3 — anything\t    status: NEW
"
        );

        fs::write(root.join("PROGRAM"), "cycle: guided\n").unwrap();
        assert_eq!(
            claim(root, &["add", "another gap is not here", "src"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: approve the agent panel before investigation (cycle approve-panel)"
        );
    }

    #[test]
    fn max_new_claims_uses_the_raw_limit() {
        let _guard = env_lock();
        let _max = set_env("CRUCIBLE_MAX_NEW_CLAIMS", "1");
        let tmp = Tmp::new();
        let root = tmp.root.as_path();
        let clock = clock();
        claim(root, &["add", "the widget is not present", "src"], &clock).unwrap();
        assert_eq!(
            claim(root, &["add", "the other widget is not present", "src"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: at most 1 NEW claims (CRUCIBLE_MAX_NEW_CLAIMS); audit or drop before adding more"
        );
        drop(_max);
        let _bad = set_env("CRUCIBLE_MAX_NEW_CLAIMS", "0");
        assert_eq!(
            claim(root, &["add", "still missing somewhere", "src"], &clock)
                .unwrap_err()
                .to_string(),
            "CRUCIBLE_MAX_NEW_CLAIMS must be a positive integer"
        );
    }

    #[test]
    fn verdict_history_name_is_fixed_clock_unix() {
        let _guard = env_lock();
        let _mins = unset_env("CRUCIBLE_MIN_AUDITORS");
        let tmp = Tmp::new();
        let root = tmp.root.as_path();
        let clock = clock();
        agents(root);
        claim(root, &["add", "the widget is not present", "src"], &clock).unwrap();
        evidence(root, "C1", "a1");
        let first = claim(root, &["verdict", "C1", "a1", "TRUE", "one"], &clock).unwrap();
        assert!(first.ends_with("claims/C1/verdicts/a1.md\n"));
        claim(root, &["verdict", "C1", "a1", "FALSE", "two"], &clock).unwrap();
        let history = root.join("claims/C1/verdicts/history/a1.1700000000.md");
        assert!(history.is_file(), "missing {}", history.display());
        let old = fs::read_to_string(&history).unwrap();
        assert!(old.starts_with("CLAIM-VERDICT: TRUE\n"));
        assert!(old.contains("CITATION: one\n"));
        let cur = fs::read_to_string(root.join("claims/C1/verdicts/a1.md")).unwrap();
        assert_eq!(
            cur,
            "CLAIM-VERDICT: FALSE\nAGENT: a1\nKIND: kindA\nCITATION: two\n"
        );
        assert!(fs::read_to_string(root.join("CLAIMS.md"))
            .unwrap()
            .contains("    status: AUDITED_FALSE\n"));
    }

    #[test]
    fn like_copies_a_false_verdict() {
        let _guard = env_lock();
        let tmp = Tmp::new();
        let root = tmp.root.as_path();
        let clock = clock();
        agents(root);
        claim(root, &["add", "the widget is not present", "src"], &clock).unwrap();
        claim(root, &["add", "the icon is not present", "src"], &clock).unwrap();
        evidence(root, "C1", "a1");
        let out = claim(
            root,
            &["verdict", "C1", "a1", "FALSE", "quote", "--like", "C2"],
            &clock,
        )
        .unwrap();
        let copy = root.join("claims/C2/verdicts/a1.md");
        assert_eq!(
            out,
            format!(
                "{}\n{}\n",
                copy.display(),
                root.join("claims/C1/verdicts/a1.md").display()
            )
        );
        assert_eq!(
            fs::read_to_string(copy).unwrap(),
            fs::read_to_string(root.join("claims/C1/verdicts/a1.md")).unwrap()
        );
        let ledger = fs::read_to_string(root.join("CLAIMS.md")).unwrap();
        assert_eq!(ledger.matches("    status: AUDITED_FALSE\n").count(), 2);
    }

    #[test]
    fn guided_verdict_requires_a_sealed_attempt_then_supersedes_it() {
        let _guard = env_lock();
        let tmp = Tmp::new();
        let root = tmp.root.as_path();
        let clock = clock();
        agents(root);
        fs::write(root.join("PROGRAM"), "cycle: guided\n").unwrap();
        approve_panel(root);
        claim(root, &["add", "the widget is not present", "src"], &clock).unwrap();
        evidence(root, "C1", "a1");
        assert_eq!(
            claim(root, &["verdict", "C1", "a1", "TRUE", "q"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: guided claim verdict requires a claim dispatch for a1 — run: ./crucible dispatch C1 claim-auditor a1"
        );

        let id = "A1700000000.9.1";
        let attempt = root.join("attempts").join(id);
        fs::create_dir_all(&attempt).unwrap();
        fs::write(
            attempt.join("meta.tsv"),
            format!(
                "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\n{id}\tC1\t-\tCLAIM\tclaim-auditor\ta1\tkindA\t-\tFOCUSED\tDISPATCHED\t1\t1801\t-\n"
            ),
        )
        .unwrap();
        fs::write(
            attempt.join("events.tsv"),
            "state\tepoch\tpid\treason\nDISPATCHED\t1\t-\tdispatch-recorded\n",
        )
        .unwrap();
        let dispatch = root.join("claims/C1/dispatches");
        fs::create_dir_all(&dispatch).unwrap();
        fs::write(
            dispatch.join("1-claim-auditor-a1.md"),
            format!("attempt-id: {id}\n"),
        )
        .unwrap();
        assert_eq!(
            claim(root, &["verdict", "C1", "a1", "TRUE", "q"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: guided claim verdict requires a matching claim attempt (item+agent+role) on the independence ledger for a1"
        );

        fs::write(attempt.join("transport"), "subagent\n").unwrap();
        fs::write(attempt.join("contract-audit.md"), "VERDICT: PASS\n").unwrap();
        assert_eq!(
            claim(root, &["verdict", "C1", "a1", "TRUE", "q"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: subagent requires a recorded ACP probe failure (ACP-PROBE.md status: failed, or PANEL notes ACP unavailable)"
        );

        fs::write(attempt.join("transport"), "multi-agent\n").unwrap();
        claim(root, &["verdict", "C1", "a1", "TRUE", "q"], &clock).unwrap();
        let events = fs::read_to_string(attempt.join("events.tsv")).unwrap();
        assert!(
            events.contains("SUPERSEDED\t1700000000\t-\tclaim-verdict-TRUE\n"),
            "{events}"
        );
        assert!(root.join("claims/C1/verdicts/a1.md").is_file());
    }

    #[test]
    fn scout_refuses_unmerged_ai_branch() {
        let _guard = env_lock();
        let tmp = Tmp::new();
        let root = tmp.root.as_path();
        let clock = clock();
        agents(root);
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "{args:?}");
        };
        git(&["init"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        git(&["config", "commit.gpgsign", "false"]);
        git(&["config", "color.ui", "never"]);
        git(&["commit", "--allow-empty", "-m", "init"]);
        git(&["branch", "-M", "main"]);
        git(&["checkout", "-b", "ai/gap"]);
        git(&["commit", "--allow-empty", "-m", "gap"]);
        git(&["checkout", "main"]);
        fs::write(
            root.join("PROGRAM"),
            format!("repo: {}\nbase: main\n", repo.display()),
        )
        .unwrap();
        claim(root, &["add", "the widget is not present", "src"], &clock).unwrap();
        evidence(root, "C1", "a1");
        assert_eq!(
            claim(root, &["scout", "C1", "ABSENT", "a1"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: unmerged branch ai/gap implements work — record IN-FLIGHT, not ABSENT"
        );
        assert_eq!(
            claim(root, &["scout", "C1", "IN-FLIGHT", "a1"], &clock).unwrap(),
            "C1 scout: IN-FLIGHT (by a1)\n"
        );
    }

    #[test]
    fn admit_creates_the_item_from_the_claim_title() {
        let _guard = env_lock();
        let _aud = unset_env("CRUCIBLE_MIN_AUDITORS");
        let _kinds = unset_env("CRUCIBLE_MIN_KINDS");
        let tmp = Tmp::new();
        let root = tmp.root.as_path();
        let clock = clock();
        agents(root);
        claim(root, &["add", "the widget is not present", "src"], &clock).unwrap();
        evidence(root, "C1", "a1");
        evidence(root, "C1", "a2");
        claim(root, &["verdict", "C1", "a1", "TRUE", "q"], &clock).unwrap();
        claim(root, &["verdict", "C1", "a2", "TRUE", "q"], &clock).unwrap();
        claim(root, &["scout", "C1", "ABSENT", "a1"], &clock).unwrap();
        assert_eq!(
            claim(root, &["admit", "C1", "bad slug"], &clock)
                .unwrap_err()
                .to_string(),
            "slug may only contain A-Za-z0-9._-"
        );
        let out = claim(root, &["admit", "C1", "gap"], &clock).unwrap();
        let item = root.join("items/gap/ITEM.md");
        assert_eq!(
            out,
            format!(
                "{}\nnext: write the ask, criteria and falsifier into that file, then\n      crucible brief gap maker <name>\nadmitted C1 as item gap\n",
                item.display()
            )
        );
        assert_eq!(
            fs::read_to_string(&item).unwrap(),
            "\
# gap — the widget is not present

PHASE: SPEC
STATUS: OPEN

## The ask

the widget is not present

## Acceptance criteria

- [ ] A1

## Falsifier

TEMPLATE-FALSIFIER-UNWRITTEN. Replace this line: name the change to undo and the check
that must then fail. The gate refuses closure while this marker is present.
"
        );
        assert!(fs::read_to_string(root.join("CLAIMS.md"))
            .unwrap()
            .contains("    status: ADMITTED\n"));
        assert!(root.join("LESSONS.md").is_file());
    }

    #[test]
    fn managed_admit_stamps_state_from_the_clock() {
        let _guard = env_lock();
        let _aud = unset_env("CRUCIBLE_MIN_AUDITORS");
        let _kinds = unset_env("CRUCIBLE_MIN_KINDS");
        let tmp = Tmp::new();
        let root = tmp.root.as_path();
        let clock = clock();
        agents(root);
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(root.join("PROPOSAL.md"), "proposal\n").unwrap();
        let id = proposal_id(root).unwrap().unwrap();
        fs::write(root.join("APPROVAL"), format!("proposal-id: {id}\n")).unwrap();
        fs::write(root.join("STATE.tsv"), format!("{STATE_HEADER}\n")).unwrap();
        claim(root, &["add", "the widget is not present", "src"], &clock).unwrap();
        evidence(root, "C1", "a1");
        evidence(root, "C1", "a2");
        claim(root, &["verdict", "C1", "a1", "TRUE", "q"], &clock).unwrap();
        claim(root, &["verdict", "C1", "a2", "TRUE", "q"], &clock).unwrap();
        claim(root, &["scout", "C1", "ABSENT", "a1"], &clock).unwrap();
        let out = claim(root, &["admit", "C1", "gap"], &clock).unwrap();
        assert!(out.contains("      ./crucible ready gap\n"));
        assert!(out.ends_with("admitted C1 as item gap\n"));
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(
            state.contains("gap\tACTIVE\tDRAFT\tEMPTY\tLOW\t-\t-\t1700000000\n"),
            "{state}"
        );
        assert!(fs::read_to_string(root.join("items/gap/ITEM.md"))
            .unwrap()
            .contains("## Focused falsifier\n"));
    }
}
