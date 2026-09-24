//! Cycle status, problem binding, approvals, and clean.
//!
//! Epochs come from [`Clock::now_unix`]. The archive directory stamp is
//! `YYYYMMDDTHHMMSSZ` with no separators. The shell variable `$pid` in
//! `cycle_archive_investigation` is the proposal id, or `noproposal`, not `$$`.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;

use crucible_contract::Clock;

use crate::panel::{
    agents_registry_ready, approval_current, contains_ci, contains_word_ci, first_transport_token,
    guided_min_auditors, guided_min_auditors_label, hash_file, is_nonempty_regular, is_regular,
    kind_of, min_kinds, panel_approval_current, panel_id, panel_valid, proposal_id, proposal_valid,
    split_tabs, transport_ladder_ok,
};
use crate::program::{uses_guided_cycle, uses_managed_lifecycle};
use crate::project::project_cycle_line;
use crate::state::{
    state_render_file, state_update_item, state_validate_file, state_value, STATE_HEADER,
};
use crate::{message, records, GuidedError};

const MARK: &str = "crucible-run/1";

const USAGE_CYCLE: &str = "usage: crucible cycle [status]|problem FILE [--next]|problem --abandon REASON|approve-panel|approve|clean --dry-run|--apply";
const USAGE_PROBLEM: &str = "usage: crucible cycle problem FILE [--next] | --abandon REASON";
const USAGE_CLEAN: &str = "usage: crucible cycle clean --dry-run|--apply";

/// `crucible cycle`. Returns the shell's stdout. Refusals are [`GuidedError::Message`].
pub fn cycle(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    let (sub, rest) = match args.split_first() {
        Some((sub, rest)) => (*sub, rest),
        None => ("status", &[][..]),
    };
    match sub {
        "status" => {
            if !rest.is_empty() {
                return Err(message("usage: crucible cycle [status]"));
            }
            write_cycle_status(root, clock)
        }
        "problem" => cycle_problem(root, clock, rest),
        "approve-panel" => cycle_approve_panel(root, clock, rest),
        "approve" => cycle_approve(root, clock, rest),
        "clean" => {
            if rest.len() != 1 {
                return Err(message(USAGE_CLEAN));
            }
            cycle_cleanup(root, clock, rest[0])
        }
        _ => Err(message(USAGE_CYCLE)),
    }
}

pub fn write_cycle_status(root: &Path, clock: &dyn Clock) -> Result<String, GuidedError> {
    cycle_sync_panel_context(root, clock)?;
    let line = cycle_status(root, clock)?;
    let state = drive_state_name(&line);
    let item = nonempty_or_dash(drive_active_item(root)?);
    let inflight = nonempty_or_dash(drive_inflight(root, &item)?);
    let evidence = drive_last_evidence(root)?;
    let gate = drive_human_gate(state);
    let engine = engine_version(root)?;
    let worth = cycle_worth(root)?;
    let epoch = clock.now_unix();
    let mut body = format!(
        "\
# STATUS

line: {line}
state: {state}
engine: {engine}
worth: {worth}
active-item: {item}
inflight-attempt: {inflight}
last-evidence: {evidence}
next-human-gate: {gate}
updated-epoch: {epoch}

Read this after `cycle`. If drive is running, perform only the single next legal orchestrator action.
"
    );
    let report = if state == "DONE" {
        let report = cycle_cleanup_report(root)?;
        body.push('\n');
        body.push_str(&report);
        Some(report)
    } else {
        None
    };
    fs::write(root.join("STATUS.md"), body)?;
    // Guided-only programs project. `working-mode: yes` keeps the working-mode board.
    if !working_mode_yes(root) {
        if let Some(repo) = program_field(root, "repo") {
            let independence = recorded_independence(root)?;
            project_cycle_line(Path::new(&repo), &line, &independence, clock)?;
        }
    }
    let mut out = format!("{line}\n");
    if let Some(report) = report {
        out.push_str(&report);
    }
    Ok(out)
}

/// The cycle line `cmd_cycle_status` prints, without a trailing newline.
pub fn cycle_status(root: &Path, _clock: &dyn Clock) -> Result<String, GuidedError> {
    if uses_guided_cycle(root)? {
        if !panel_valid(root)? {
            if !agents_registry_ready(root)? {
                return Ok("NEXT CONFIGURE — replace placeholder agents.tsv; ask operator for agents AND role casting; write PANEL.md + PANEL.ASSIGN.tsv".to_string());
            }
            if !root.join("PANEL.ASSIGN.tsv").is_file() {
                return Ok("NEXT CONFIGURE — cast personas to independent agents in PANEL.ASSIGN.tsv (role→agent) and complete PANEL.md".to_string());
            }
            return Ok("NEXT CONFIGURE — fix PANEL.md / PANEL.ASSIGN.tsv (agents inventory, role casting, risk, isolation, ladder, waivers)".to_string());
        }
        if !panel_approval_current(root)? {
            let id = panel_id(root)?.unwrap_or_default();
            return Ok(format!("WAIT PANEL — show agent inventory and role casting {id} to the operator; do not investigate or build"));
        }
    }
    if problem_needs_intake(&root.join("PROBLEM.md")) {
        return Ok("NEXT INTAKE — capture the operator's problem in PROBLEM.md".to_string());
    }
    match cycle_investigation_state(root)?.as_str() {
        "EMPTY" => {
            return Ok("NEXT INVESTIGATE — split PROBLEM.md into atomic claims and verify them via independent agents".to_string());
        }
        "NEEDS_AUDIT" => {
            if claims_absent_only(root)? {
                return Ok("NEXT INVESTIGATE — independently fact-check every unresolved ABSENT claim (NO-BUILD if all FALSE/STALE)".to_string());
            }
            let n = guided_min_auditors_label(root)?;
            return Ok(format!("NEXT INVESTIGATE — independently fact-check every unresolved claim (FALSE/STALE closes a claim; admit needs {n} sealed TRUE to create work)"));
        }
        "NEEDS_SCOUT" => {
            return Ok(
                "NEXT INVESTIGATE — search for existing behavior before proposing work".to_string(),
            );
        }
        _ => {}
    }
    if !proposal_valid(root)? {
        return Ok(match cycle_worth(root)?.as_str() {
            "DOCS" => "NEXT PROPOSE — no ABSENT capability; propose DOCS-ONLY or live-observation"
                .to_string(),
            "NO-BUILD" => {
                "NEXT PROPOSE — no missing capability; propose NO-BUILD or stop".to_string()
            }
            _ => "NEXT PROPOSE — write a refined, evidence-grounded PROPOSAL.md".to_string(),
        });
    }
    let pid = proposal_id(root)?.unwrap_or_default();
    if !approval_current(root)? {
        return Ok(format!(
            "WAIT APPROVAL — show proposal {pid} to the operator; do not plan or build"
        ));
    }
    if uses_managed_lifecycle(root)? {
        state_validate_file(&root.join("STATE.tsv"))?;
        if let Some(blocked) = blocked_independence(root)? {
            return Ok(format!("ESCALATE INDEPENDENCE_UNAVAILABLE {blocked} — cannot invoke required independent agent; stop and warn; do not continue as solo theatre"));
        }
        if let Some(slug) = current_slug(root)? {
            let status = state_value(root, &slug, 2)?.unwrap_or_default();
            let stage = state_value(root, &slug, 3)?.unwrap_or_default();
            let inflight = state_value(root, &slug, 6)?.unwrap_or_default();
            if status == "BLOCKED" {
                let code = state_value(root, &slug, 7)?.unwrap_or_default();
                return Ok(format!("ESCALATE {slug} — {code}"));
            }
            if inflight == "TASKS" {
                return Ok(format!("NEXT EXECUTE {slug} — dispatch dependency-ready tasks or integrate passing work"));
            }
            if inflight != "-" {
                let attempt_status = attempt_state(root, &inflight)?;
                return Ok(match attempt_status.as_str() {
                    "DISPATCHED" | "RUNNING" => format!("WAIT {slug} — agent work or review is in flight ({inflight})"),
                    "OVERDUE" => format!("ESCALATE {slug} — observe and record the overdue process outcome ({inflight})"),
                    "RETURNED" => format!("NEXT REVIEW {slug} — reconcile returned attempt {inflight} against its evidence"),
                    "TIMEOUT" => format!("NEXT EXECUTE {slug} — use the one bounded retry for attempt {inflight}"),
                    other => format!("ESCALATE {slug} — inconsistent attempt state {other} for {inflight}"),
                });
            }
            return Ok(match stage.as_str() {
                "DRAFT" | "READY" => format!("NEXT PLAN {slug} — validate the bounded breakdown before execution"),
                "BUILD" => format!("NEXT EXECUTE {slug} — make, verify, review, and fix until accepted"),
                "REVIEW" => format!("NEXT REVIEW {slug} — independently falsify current work; close or return findings"),
                _ => String::new(),
            });
        }
    }
    if cycle_unadmitted_work(root)? {
        return Ok(format!(
            "NEXT PLAN — admit the next bounded item from approved proposal {pid}"
        ));
    }
    write_independence_receipt(root)?;
    Ok("DONE — no admittable claim remains. Next: cycle clean --dry-run (or cycle problem FILE --next). Drive never --apply.".to_string())
}

pub fn cycle_investigation_state(root: &Path) -> Result<String, GuidedError> {
    let path = root.join("CLAIMS.md");
    if !path.is_file() {
        return Ok("EMPTY".to_string());
    }
    let text = fs::read_to_string(&path)?;
    let total = count_claim_headings(&text);
    if total == 0 {
        return Ok("EMPTY".to_string());
    }
    let guided = uses_guided_cycle(root)?;
    for i in 1..=total {
        let cn = format!("C{i}");
        let vd = root.join("claims").join(&cn).join("verdicts");
        let mut verdicts = 0usize;
        let mut truth = 0usize;
        if vd.is_dir() {
            for path in md_files(&vd) {
                let Some(word) = claim_verdict_word(&path) else {
                    continue;
                };
                let agent = file_stem(&path);
                if guided && !claim_agent_independence_ok(root, &cn, &agent)? {
                    continue;
                }
                verdicts += 1;
                if word == "TRUE" {
                    truth += 1;
                }
            }
        }
        if verdicts == 0 {
            return Ok("NEEDS_AUDIT".to_string());
        }
        if truth > 0 {
            let scout = claim_field(&text, i, "scout").unwrap_or_default();
            if scout.is_empty() {
                return Ok("NEEDS_SCOUT".to_string());
            }
            if !claim_true_superseded(root, &cn)?
                && matches!(scout.as_str(), "ABSENT" | "PARTLY-EXISTS")
                && !claim_true_bar_met(root, &cn)?
            {
                return Ok("NEEDS_AUDIT".to_string());
            }
        }
    }
    Ok("COMPLETE".to_string())
}

pub fn cycle_worth(root: &Path) -> Result<String, GuidedError> {
    let path = root.join("CLAIMS.md");
    if !path.is_file() {
        return Ok("UNKNOWN".to_string());
    }
    if cycle_investigation_state(root)? != "COMPLETE" {
        return Ok("UNKNOWN".to_string());
    }
    let mut truec = false;
    let claims = root.join("claims");
    if claims.is_dir() {
        if let Ok(rd) = fs::read_dir(&claims) {
            for ent in rd.flatten() {
                for verdict in md_files(&ent.path().join("verdicts")) {
                    if claim_verdict_word(&verdict) == Some("TRUE") {
                        truec = true;
                    }
                }
            }
        }
    }
    if !truec {
        return Ok("NO-BUILD".to_string());
    }
    let text = fs::read_to_string(path)?;
    let absent = records(&text)
        .iter()
        .filter(|line| **line == "    scout: ABSENT")
        .count();
    let partly = records(&text)
        .iter()
        .filter(|line| **line == "    scout: PARTLY-EXISTS")
        .count();
    if absent > 0 {
        Ok("BUILD".to_string())
    } else if partly > 0 {
        Ok("DOCS".to_string())
    } else {
        Ok("NO-BUILD".to_string())
    }
}

fn cycle_problem(root: &Path, clock: &dyn Clock, args: &[&str]) -> Result<String, GuidedError> {
    let mut src: Option<&str> = None;
    let mut next = false;
    let mut abandon = false;
    let mut reason = String::new();
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--next" => next = true,
            "--abandon" => {
                abandon = true;
                reason = args[i + 1..].join(" ");
                break;
            }
            flag if flag.starts_with('-') => return Err(message(USAGE_PROBLEM)),
            other => {
                if src.is_some() {
                    return Err(message(USAGE_PROBLEM));
                }
                src = Some(other);
            }
        }
        i += 1;
    }
    if abandon {
        if next {
            return Err(message(USAGE_PROBLEM));
        }
        if reason.is_empty() {
            return Err(message("usage: crucible cycle problem --abandon REASON"));
        }
        if drive_locked(root) {
            return Err(message(
                "refused: cycle problem --abandon is a human gate — stop drive first",
            ));
        }
        if !uses_guided_cycle(root)? {
            return Err(message("cycle problem --abandon requires a guided cycle"));
        }
        if !uses_managed_lifecycle(root)? {
            return Err(message(
                "cycle problem --abandon requires managed lifecycle behavior",
            ));
        }
        if !cycle_no_current_item(root)? {
            return Err(message(
                "refused: an ACTIVE or BLOCKED item is current — close or escalate it first",
            ));
        }
        let dest = cycle_archive_investigation(root, clock)?;
        let epoch = clock.now_unix();
        fs::write(
            dest.join("ABANDON.md"),
            format!("reason: {reason}\nepoch: {epoch}\ndecision: ABANDONED\n"),
        )?;
        return Ok(format!("abandoned {}\n", dest.display()));
    }
    let Some(src) = src else {
        return Err(message(USAGE_PROBLEM));
    };
    let src_path = Path::new(src);
    if !is_nonempty_regular(src_path) {
        return Err(message(format!(
            "problem must be a non-empty regular file: {src}"
        )));
    }
    cycle_refuse_non_problem(src_path)?;
    if uses_guided_cycle(root)? && !panel_approval_current(root)? {
        return Err(message(
            "refused: approve the agent panel before binding a problem (cycle approve-panel)",
        ));
    }
    if next {
        if drive_locked(root) {
            return Err(message(
                "refused: cycle problem --next is a human gate — stop drive first",
            ));
        }
        if !uses_guided_cycle(root)? {
            return Err(message("cycle problem --next requires a guided cycle"));
        }
        if !uses_managed_lifecycle(root)? {
            return Err(message(
                "cycle problem --next requires managed lifecycle behavior",
            ));
        }
        if !cycle_no_current_item(root)? {
            return Err(message(
                "refused: an ACTIVE or BLOCKED item is current — close or escalate it first",
            ));
        }
        cycle_live_attempts_terminal(root, clock)?;
        let dest = cycle_archive_investigation(root, clock)?;
        let mut out = format!("archived {}\n", dest.display());
        out.push_str(&cycle_bind_problem(root, clock, src_path)?);
        cycle_refresh_panel_context(root, clock)?;
        return Ok(out);
    }
    if cycle_investigation_state(root)? != "EMPTY" {
        return Err(message(bound_investigation_message(root)));
    }
    if root.join("APPROVAL").is_file() {
        return Err(message("refused: a proposal is already approved"));
    }
    cycle_bind_problem(root, clock, src_path)
}

fn cycle_approve_panel(
    root: &Path,
    clock: &dyn Clock,
    args: &[&str],
) -> Result<String, GuidedError> {
    if !args.is_empty() {
        return Err(message("usage: crucible cycle approve-panel"));
    }
    if drive_locked(root) {
        return Err(message(
            "refused: cycle approve-panel is a human gate — stop drive first (drive never auto-approves)",
        ));
    }
    if !uses_guided_cycle(root)? {
        return Err(message("approve-panel requires a guided cycle"));
    }
    if !panel_valid(root)? {
        return Err(message(
            "refused: PANEL.md / PANEL.ASSIGN.tsv incomplete (need agents inventory, role→agent casting, non-placeholder agents.tsv, transport policy)",
        ));
    }
    fs::copy(root.join("agents.tsv"), root.join("PANEL.AGENTS.tsv"))?;
    let Some(pid) = panel_id(root)? else {
        return Err(message(
            "refused: PANEL.md / PANEL.ASSIGN.tsv incomplete (need agents inventory, role→agent casting, non-placeholder agents.tsv, transport policy)",
        ));
    };
    if panel_approval_current(root)? {
        return Ok(format!("panel {pid} is already approved\n"));
    }
    fs::create_dir_all(root.join("panel-approvals"))?;
    let record = root.join("panel-approvals").join(format!("{pid}.md"));
    let now = clock.now_unix();
    let agents_hash: String = hash_file(&root.join("agents.tsv"))?
        .chars()
        .take(12)
        .collect();
    if fs::symlink_metadata(&record).is_err() {
        fs::write(
            &record,
            format!(
                "panel-id: {pid}\napproved-epoch: {now}\ndecision: APPROVED\nagents-hash: {agents_hash}\n"
            ),
        )?;
    } else {
        let mut file = fs::OpenOptions::new().append(true).open(&record)?;
        writeln!(
            file,
            "panel-id: {pid}\nreapproved-epoch: {now}\ndecision: APPROVED\nagents-hash: {agents_hash}"
        )?;
    }
    fs::write(
        root.join("PANEL.APPROVAL"),
        format!("panel-id: {pid}\nrecord: panel-approvals/{pid}.md\n"),
    )?;
    Ok(format!("approved panel {pid}\n"))
}

fn cycle_approve(root: &Path, clock: &dyn Clock, args: &[&str]) -> Result<String, GuidedError> {
    if !args.is_empty() {
        return Err(message("usage: crucible cycle approve"));
    }
    if drive_locked(root) {
        return Err(message(
            "refused: cycle approve is a human gate — stop drive first (drive never auto-approves)",
        ));
    }
    if uses_guided_cycle(root)? && !panel_approval_current(root)? {
        return Err(message("refused: panel is not operator-approved"));
    }
    if cycle_investigation_state(root)? != "COMPLETE" {
        return Err(message("refused: investigation is incomplete"));
    }
    if !proposal_valid(root)? {
        return Err(message(
            "refused: PROPOSAL.md is missing required sections or contains placeholders",
        ));
    }
    let Some(pid) = proposal_id(root)? else {
        return Err(message(
            "refused: PROPOSAL.md is missing required sections or contains placeholders",
        ));
    };
    if approval_current(root)? {
        return Ok(format!("proposal {pid} is already approved\n"));
    }
    fs::create_dir_all(root.join("approvals"))?;
    let record = root.join("approvals").join(format!("{pid}.md"));
    if fs::symlink_metadata(&record).is_ok() {
        return Err(message(format!(
            "approval record already exists but is not current: {}",
            record.display()
        )));
    }
    let now = clock.now_unix();
    fs::write(
        &record,
        format!("proposal-id: {pid}\napproved-epoch: {now}\ndecision: APPROVED\n"),
    )?;
    fs::write(
        root.join("APPROVAL"),
        format!("proposal-id: {pid}\nrecord: approvals/{pid}.md\n"),
    )?;
    Ok(format!("approved proposal {pid}\n"))
}

fn cycle_cleanup(root: &Path, clock: &dyn Clock, flag: &str) -> Result<String, GuidedError> {
    match flag {
        "--dry-run" | "--apply" => {}
        _ => return Err(message(USAGE_CLEAN)),
    }
    if !uses_guided_cycle(root)? {
        return Err(message("cleanup requires a guided cycle"));
    }
    let status = cycle_status(root, clock)?;
    if !status.starts_with("DONE") {
        return Err(message(format!(
            "cleanup requires DONE; current cycle says: {status}"
        )));
    }
    for path in attempt_child_dirs(root) {
        let id = file_name(&path);
        let attempt_status = attempt_state(root, &id)?;
        match attempt_status.as_str() {
            "RUNNING" | "OVERDUE" => {
                if attempt_pid_alive(root, &id)? {
                    return Err(message(format!(
                        "cleanup refuses while attempt {id} is {attempt_status} (live pid)"
                    )));
                }
                // `cmd_attempt reclaim` dies here before any ledger write.
                if !uses_managed_lifecycle(root)? {
                    return Err(message("attempt requires managed lifecycle behavior"));
                }
                reclaim_dead_attempt(root, clock, &id)?;
            }
            "DISPATCHED" => {}
            other => {
                if !attempt_terminal(other) {
                    return Err(message(format!(
                        "cleanup refuses while attempt {id} is {attempt_status}"
                    )));
                }
            }
        }
    }
    let mut out = cycle_cleanup_report(root)?;
    if root.join("agents.tsv").is_file() {
        out.push_str(&format!(
            "KEEP {} (panel identity — not leftover evidence)\n",
            root.join("agents.tsv").display()
        ));
    }
    if root.join("PANEL.ASSIGN.tsv").is_file() {
        out.push_str(&format!(
            "KEEP {} (panel identity — not leftover evidence)\n",
            root.join("PANEL.ASSIGN.tsv").display()
        ));
    }
    let records = cycle_worktree_records(root)?;
    let mut found = false;
    for rec in &records {
        out.push_str(&format!(
            "REMOVE_WORKTREE {}\nPRESERVE_BRANCH {} {}\n",
            rec.path, rec.repo, rec.branch
        ));
        if let Some(blocker) = cycle_worktree_blocker(Path::new(&rec.path))? {
            out.push_str(&format!(
                "BLOCKED_WORKTREE {} — {blocker}, then retry: {} cycle clean --apply\n",
                rec.path,
                self_path(root)
            ));
        }
        found = true;
    }
    out.push_str(&format!(
        "PRESERVE {} (problem, proposal, work, reviews, and evidence)\n",
        root.display()
    ));
    if !found {
        out.push_str("NO_SESSION_ARTIFACTS\n");
    }
    if flag != "--apply" {
        return Ok(out);
    }
    for rec in &records {
        let worktree = Path::new(&rec.path);
        if rec.path.is_empty() || !worktree.is_dir() {
            continue;
        }
        let (success, text) = match git_combined(&[
            "-C", &rec.repo, "worktree", "remove", &rec.path,
        ]) {
            Ok(output) => output,
            Err(err) => {
                return Err(message(format!(
                        "could not safely remove worktree: {} — git refused: {err}, then retry: {} cycle clean --apply",
                        rec.path,
                        self_path(root)
                    )));
            }
        };
        if !success {
            let blocker =
                cycle_worktree_blocker(worktree)?.unwrap_or_else(|| format!("git refused: {text}"));
            return Err(message(format!(
                "could not safely remove worktree: {} — {blocker}, then retry: {} cycle clean --apply",
                rec.path,
                self_path(root)
            )));
        }
    }
    remove_empty_dirs(&root.join("worktrees"));
    out.push_str("session cleanup applied; panel identity and evidence preserved\n");
    Ok(out)
}

fn cycle_cleanup_report(root: &Path) -> Result<String, GuidedError> {
    let mut out = String::from(
        "\
CLEANUP — next verb: cycle clean --dry-run (drive never --apply). Or cycle problem FILE --next.
KEEP panel: agents.tsv and PANEL.ASSIGN.tsv (do not destroy the panel after NO-BUILD).
",
    );
    let state = root.join("STATE.tsv");
    if state.is_file() {
        let text = fs::read_to_string(&state)?;
        for (idx, rec) in records(&text).into_iter().enumerate() {
            if idx == 0 {
                continue;
            }
            let fields = split_tabs(rec);
            if fields.get(1).copied() == Some("CLOSED") {
                out.push_str(&format!(
                    "closed-item: {} work {}\n",
                    fields.first().copied().unwrap_or(""),
                    fields.get(3).copied().unwrap_or("")
                ));
            }
        }
    }
    let items = root.join("items");
    if items.is_dir() {
        let mut dirs = read_dir_paths(&items);
        dirs.retain(|path| path.is_dir());
        dirs.sort();
        for idir in dirs {
            let slug = file_name(&idir);
            if !idir.join("evidence").is_dir() {
                continue;
            }
            let wid = workid(root, &slug).unwrap_or_default();
            for rel in stale_evidence(root, &slug, &wid) {
                out.push_str(&format!("stale-evidence: {rel}\n"));
            }
        }
    }
    if root.join(".drive.lock").is_dir() {
        out.push_str("stale-lock: .drive.lock\n");
    }
    for path in attempt_dirs_a(root) {
        let id = file_name(&path);
        let Ok(ast) = attempt_state(root, &id) else {
            continue;
        };
        if ast == "RUNNING" || ast == "OVERDUE" {
            if attempt_pid_alive(root, &id)? {
                out.push_str(&format!("live-attempt: {id} {ast}\n"));
            } else {
                out.push_str(&format!("dead-pid: {id} {ast} (attempt reclaim)\n"));
            }
        }
    }
    let problem = root.join("PROBLEM.md");
    let context = root.join("PANEL.CONTEXT.md");
    if problem.is_file() && context.is_file() {
        let ptitle = cycle_problem_title(&problem);
        let text = fs::read_to_string(&context).unwrap_or_default();
        let ctitle = records(&text)
            .into_iter()
            .find_map(|line| line.strip_prefix("problem-title: "))
            .unwrap_or("");
        if ptitle != ctitle {
            out.push_str(&format!(
                "panel-title-stale: context {ctitle} vs PROBLEM {ptitle}\n"
            ));
        }
    }
    let claims = root.join("CLAIMS.md");
    if claims.is_file() {
        let text = fs::read_to_string(&claims).unwrap_or_default();
        if records(&text).contains(&"    status: NEW") {
            out.push_str("claims-status-new: CLAIMS.md still has status NEW after close/audit\n");
        }
    }
    for husk in cycle_husk_programs(root) {
        out.push_str(&format!(
            "NOT cycle clean: husk program {husk} (no PROGRAM — keep it, trash it, or adopt that name)\n"
        ));
    }
    out.push_str(
        "NOT cycle clean: Jira or any other tracker, and ignored build or validation directories.\n",
    );
    Ok(out)
}

fn cycle_archive_investigation(root: &Path, clock: &dyn Clock) -> Result<PathBuf, GuidedError> {
    let stamp = archive_stamp(clock.now_unix());
    let suffix = proposal_id(root)?.unwrap_or_else(|| "noproposal".to_string());
    let dest = root.join("history").join(format!("{stamp}-{suffix}"));
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(message(format!(
            "archive already exists: {}",
            dest.display()
        )));
    }
    fs::create_dir_all(&dest)?;
    for name in [
        "PROBLEM.md",
        "CLAIMS.md",
        "PROPOSAL.md",
        "APPROVAL",
        "STATE.tsv",
        "STATE.md",
        "INDEPENDENCE.md",
        "BACKLOG.md",
        "DRIVE.state",
        "DRIVE.BRIEF.md",
        "STATUS.md",
    ] {
        let src = root.join(name);
        if fs::symlink_metadata(&src).is_ok() {
            fs::rename(&src, dest.join(name))?;
        }
    }
    for name in ["claims", "items", "approvals", "attempts"] {
        let src = root.join(name);
        if fs::symlink_metadata(&src).is_ok() {
            fs::rename(&src, dest.join(name))?;
        }
    }
    fs::create_dir_all(root.join("items"))?;
    let prog = program_value(root, "program");
    fs::write(
        root.join("CLAIMS.md"),
        format!(
            "# CLAIMS — {prog}\n\nOne heading per finding from the problem document, quoting its source sentence\nverbatim. Nothing becomes an item until it survives audit.\n"
        ),
    )?;
    fs::write(
        root.join("BACKLOG.md"),
        "- [ ] example — replace this line with real items as claims are admitted\n",
    )?;
    if uses_managed_lifecycle(root)? {
        fs::write(root.join("STATE.tsv"), format!("{STATE_HEADER}\n"))?;
        state_render_file(root, &root.join("STATE.md"), None)?;
    }
    Ok(dest)
}

fn cycle_bind_problem(root: &Path, clock: &dyn Clock, src: &Path) -> Result<String, GuidedError> {
    if !is_nonempty_regular(src) {
        return Err(message(format!(
            "problem must be a non-empty regular file: {}",
            src.display()
        )));
    }
    fs::copy(src, root.join("PROBLEM.md"))?;
    cycle_sync_panel_context(root, clock)?;
    Ok(format!("{}/PROBLEM.md\n", root.display()))
}

fn cycle_sync_panel_context(root: &Path, clock: &dyn Clock) -> Result<(), GuidedError> {
    let problem = root.join("PROBLEM.md");
    let sized = fs::metadata(&problem).map(|m| m.len() > 0).unwrap_or(false);
    if !sized {
        return Ok(());
    }
    let title = cycle_problem_title(&problem);
    if title.is_empty() {
        return Ok(());
    }
    let ctx = root.join("PANEL.CONTEXT.md");
    if ctx.is_file() {
        let text = fs::read_to_string(&ctx)?;
        let ctitle = records(&text)
            .into_iter()
            .find_map(|line| line.strip_prefix("problem-title: "))
            .unwrap_or("");
        if title == ctitle {
            return Ok(());
        }
    }
    let body = fs::read_to_string(&problem).unwrap_or_default();
    let risk = problem_risk(&body);
    let epoch = clock.now_unix();
    fs::write(
        &ctx,
        format!(
            "problem-title: {title}\nrisk: {risk}\nrefreshed-epoch: {epoch}\n\nSynced from PROBLEM.md. Cast is unchanged.\n"
        ),
    )?;
    Ok(())
}

fn cycle_refresh_panel_context(root: &Path, clock: &dyn Clock) -> Result<(), GuidedError> {
    let problem = root.join("PROBLEM.md");
    let mut title = cycle_problem_title(&problem);
    if title.is_empty() {
        title = "(untitled)".to_string();
    }
    cycle_refuse_leftover_title(&title)?;
    let body = fs::read_to_string(&problem).unwrap_or_default();
    let risk = problem_risk(&body);
    let epoch = clock.now_unix();
    fs::write(
        root.join("PANEL.CONTEXT.md"),
        format!(
            "problem-title: {title}\nrisk: {risk}\nrefreshed-epoch: {epoch}\n\nRefreshed by cycle problem --next. Cast is unchanged.\n"
        ),
    )?;
    let pf = root.join("PANEL.md");
    if pf.is_file() {
        let text = fs::read_to_string(&pf)?;
        let mut rewritten = String::new();
        let mut replaced = false;
        for line in records(&text) {
            if !replaced && line.starts_with("# ") {
                rewritten.push_str("# ");
                rewritten.push_str(&title);
                rewritten.push('\n');
                replaced = true;
            } else {
                rewritten.push_str(line);
                rewritten.push('\n');
            }
        }
        fs::write(&pf, rewritten)?;
        if uses_guided_cycle(root)? && root.join("PANEL.APPROVAL").is_file() {
            if let Some(pid) = panel_id(root)? {
                let now = clock.now_unix();
                fs::write(
                    root.join("PANEL.APPROVAL"),
                    format!(
                        "panel-id: {pid}\napproved-epoch: {now}\ndecision: APPROVED\nrefreshed-by: cycle-problem-next\n"
                    ),
                )?;
            }
        }
    }
    Ok(())
}

fn cycle_refuse_non_problem(src: &Path) -> Result<(), GuidedError> {
    let text = fs::read_to_string(src)?;
    let title = problem_title_of(&text);
    cycle_refuse_leftover_title(&title)?;
    let nlines = records(&text).iter().filter(|line| !is_blank(line)).count();
    let ncli = records(&text)
        .iter()
        .filter(|line| contains_ci(line, "is not a CLI verb"))
        .count();
    if ncli >= 8 {
        return Err(message(
            "refused: leftover catalog of CLI-verb claims is not a PROBLEM — file one falsifiable outcome",
        ));
    }
    let folded = text.replace('\n', " ");
    if contains_ci(&format!("{title} {folded}"), "is not a CLI verb") && nlines <= 3 {
        return Err(message(
            "refused: 'is not a CLI verb' is claim polarity, not a PROBLEM",
        ));
    }
    let lower = title.to_ascii_lowercase();
    if title_is_leftover_catalog(&lower) && !names_outcome(&text) {
        return Err(message(
            "refused: leftover/remainder catalog is not a PROBLEM — file one falsifiable outcome",
        ));
    }
    Ok(())
}

fn cycle_refuse_leftover_title(title: &str) -> Result<(), GuidedError> {
    let lower = title.to_ascii_lowercase();
    if lower.contains("pr-status") || lower == "prstatus" {
        return Err(message(
            "refused: leftover pr-status title — bind the real PROBLEM, not a leftover title",
        ));
    }
    Ok(())
}

fn bound_investigation_message(root: &Path) -> String {
    let mut prog = program_value(root, "program");
    if prog.is_empty() {
        prog = root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
    }
    format!("refused: leftover PROBLEM is still bound (investigation already has claims). Next real PROBLEM on this panel: cycle problem FILE --next (human). Parallel cycle without discarding this one: adopt NAME --managed --panel-from {prog}. Drive never --nexts.")
}

fn cycle_no_current_item(root: &Path) -> Result<bool, GuidedError> {
    if !uses_managed_lifecycle(root)? || !root.join("STATE.tsv").is_file() {
        return Ok(true);
    }
    Ok(current_slug(root)?.is_none())
}

fn cycle_live_attempts_terminal(root: &Path, clock: &dyn Clock) -> Result<(), GuidedError> {
    for path in attempt_child_dirs(root) {
        let id = file_name(&path);
        let status = attempt_state(root, &id)?;
        if status == "RUNNING" || status == "OVERDUE" {
            if attempt_pid_alive(root, &id)? {
                return Err(message(format!("refused: attempt {id} is {status}")));
            }
            reclaim_dead_attempt(root, clock, &id)?;
        }
    }
    Ok(())
}

fn cycle_unadmitted_work(root: &Path) -> Result<bool, GuidedError> {
    let path = root.join("CLAIMS.md");
    if !path.is_file() {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    let total = count_claim_headings(&text);
    for i in 1..=total {
        if !claim_field(&text, i, "item").unwrap_or_default().is_empty() {
            continue;
        }
        let scout = claim_field(&text, i, "scout").unwrap_or_default();
        if matches!(scout.as_str(), "ABSENT" | "PARTLY-EXISTS")
            && claim_true_bar_met(root, &format!("C{i}"))?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn claims_absent_only(root: &Path) -> Result<bool, GuidedError> {
    let path = root.join("CLAIMS.md");
    if !path.is_file() {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    let total = count_claim_headings(&text);
    if total == 0 {
        return Ok(false);
    }
    let absent = records(&text)
        .iter()
        .filter(|line| **line == "    polarity: ABSENT")
        .count();
    Ok(absent == total)
}

fn claim_true_bar_met(root: &Path, cn: &str) -> Result<bool, GuidedError> {
    if !root.join("claims").join(cn).join("verdicts").is_dir() {
        return Ok(false);
    }
    if !claim_true_independence_ok(root, cn)? {
        return Ok(false);
    }
    let (auditors, kinds) = claim_true_counts(root, cn)?;
    Ok(auditors >= guided_min_auditors(root)? && kinds >= min_kinds()?)
}

fn claim_true_independence_ok(root: &Path, cn: &str) -> Result<bool, GuidedError> {
    if !uses_guided_cycle(root)? {
        return Ok(true);
    }
    let dir = root.join("claims").join(cn).join("verdicts");
    if !dir.is_dir() {
        return Ok(true);
    }
    for path in md_files(&dir) {
        if claim_verdict_word(&path) != Some("TRUE") {
            continue;
        }
        let agent = file_stem(&path);
        if kind_of(root, &agent)?.is_empty() || !claim_agent_evidence_ok(root, cn, &agent) {
            continue;
        }
        if !claim_agent_independence_ok(root, cn, &agent)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn claim_true_counts(root: &Path, cn: &str) -> Result<(u64, u64), GuidedError> {
    let mut auditors = 0u64;
    let mut kinds = Vec::new();
    let dir = root.join("claims").join(cn).join("verdicts");
    if dir.is_dir() {
        for path in md_files(&dir) {
            if claim_verdict_word(&path) != Some("TRUE") {
                continue;
            }
            let agent = file_stem(&path);
            let kind = kind_of(root, &agent)?;
            if kind.is_empty() || !claim_agent_evidence_ok(root, cn, &agent) {
                continue;
            }
            if uses_guided_cycle(root)? && !claim_agent_independence_ok(root, cn, &agent)? {
                continue;
            }
            auditors += 1;
            if !kinds.iter().any(|seen| seen == &kind) {
                kinds.push(kind);
            }
        }
    }
    Ok((auditors, kinds.len() as u64))
}

fn claim_true_superseded(root: &Path, cn: &str) -> Result<bool, GuidedError> {
    let dir = root.join("claims").join(cn).join("verdicts");
    if !dir.is_dir() {
        return Ok(false);
    }
    let guided = uses_guided_cycle(root)?;
    let mut true_agents = Vec::new();
    for path in md_files(&dir) {
        if claim_verdict_word(&path) != Some("TRUE") {
            continue;
        }
        let agent = file_stem(&path);
        if guided && !claim_agent_independence_ok(root, cn, &agent)? {
            continue;
        }
        true_agents.push(agent);
    }
    if true_agents.is_empty() {
        return Ok(false);
    }
    for path in md_files(&dir) {
        let word = claim_verdict_word(&path);
        if !matches!(word, Some("FALSE") | Some("STALE")) {
            continue;
        }
        let agent = file_stem(&path);
        if true_agents.iter().any(|who| who == &agent) {
            continue;
        }
        if guided && !claim_agent_independence_ok(root, cn, &agent)? {
            continue;
        }
        return Ok(true);
    }
    Ok(false)
}

fn claim_agent_independence_ok(root: &Path, cn: &str, agent: &str) -> Result<bool, GuidedError> {
    if !panel_approval_current(root)? {
        return Ok(false);
    }
    let id = claim_agent_attempt(root, cn, agent)?;
    if id.is_empty() {
        return Ok(false);
    }
    let Some(transport) = attempt_transport(root, &id)? else {
        return Ok(false);
    };
    if transport_ladder_ok(root, &transport)? != 0 {
        return Ok(false);
    }
    attempt_contract_audit_pass(root, &id)
}

fn claim_agent_attempt(root: &Path, cn: &str, agent: &str) -> Result<String, GuidedError> {
    let dir = root.join("claims").join(cn).join("dispatches");
    if dir.is_dir() {
        for role in ["claim-auditor", "scout"] {
            let mut best: Option<(u64, String)> = None;
            for path in read_dir_paths(&dir) {
                if !path.is_file() {
                    continue;
                }
                let name = file_name(&path);
                let suffix = format!("-{role}-{agent}.md");
                if !name.ends_with(&suffix) {
                    continue;
                }
                let npart = name.split('-').next().unwrap_or("");
                if npart.is_empty() || !npart.bytes().all(|b| b.is_ascii_digit()) {
                    continue;
                }
                let Ok(n) = npart.parse::<u64>() else {
                    continue;
                };
                let text = fs::read_to_string(&path)?;
                let stamped = records(&text)
                    .into_iter()
                    .find_map(|line| line.strip_prefix("attempt-id: "))
                    .unwrap_or("");
                if stamped.is_empty() || !claim_attempt_matches(root, stamped, cn, agent, None)? {
                    continue;
                }
                if best.as_ref().map(|(prev, _)| n < *prev).unwrap_or(true) {
                    best = Some((n, stamped.to_string()));
                }
            }
            if let Some((_, id)) = best {
                return Ok(id);
            }
        }
    }
    let mut last = String::new();
    for path in attempt_dirs_a(root) {
        let id = file_name(&path);
        if claim_attempt_matches(root, &id, cn, agent, None)? {
            last = id;
        }
    }
    Ok(last)
}

fn claim_attempt_matches(
    root: &Path,
    id: &str,
    cn: &str,
    agent: &str,
    want_role: Option<&str>,
) -> Result<bool, GuidedError> {
    if !root.join("attempts").join(id).join("meta.tsv").is_file() {
        return Ok(false);
    }
    if attempt_meta(root, id, 2)? != cn || attempt_meta(root, id, 6)? != agent {
        return Ok(false);
    }
    let role = attempt_meta(root, id, 5)?;
    Ok(match want_role {
        Some(want) => role == want,
        None => role == "claim-auditor" || role == "scout",
    })
}

fn claim_agent_evidence_ok(root: &Path, cn: &str, agent: &str) -> bool {
    let dir = root.join("claims").join(cn).join("evidence");
    let mut paths = read_dir_paths(&dir);
    paths.retain(|path| {
        path.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|name| evidence_name_matches(name, agent))
    });
    paths.sort();
    let mut found = false;
    for path in paths {
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if !meta.is_file() || meta.len() == 0 {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if records(&text).into_iter().next().unwrap_or("") == MARK {
            found = true;
        }
    }
    found
}

fn evidence_name_matches(name: &str, agent: &str) -> bool {
    let Some(rest) = name.strip_prefix(&format!("{agent}.")) else {
        return false;
    };
    rest.strip_suffix(".txt").is_some_and(|mid| !mid.is_empty())
}

fn claim_verdict_word(path: &Path) -> Option<&'static str> {
    let text = fs::read_to_string(path).ok()?;
    match records(&text).into_iter().next()? {
        "CLAIM-VERDICT: TRUE" => Some("TRUE"),
        "CLAIM-VERDICT: FALSE" => Some("FALSE"),
        "CLAIM-VERDICT: STALE" => Some("STALE"),
        "CLAIM-VERDICT: UNVERIFIABLE" => Some("UNVERIFIABLE"),
        _ => None,
    }
}

fn claim_field(text: &str, n: usize, field: &str) -> Option<String> {
    let start = format!("### C{n} ");
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

fn count_claim_headings(text: &str) -> usize {
    records(text)
        .iter()
        .filter(|line| line.starts_with("### C"))
        .count()
}

fn write_independence_receipt(root: &Path) -> Result<(), GuidedError> {
    // Shell returns before the attempt loop unless the cycle is guided.
    if !uses_guided_cycle(root)? {
        return Ok(());
    }
    let mut body = String::from(
        "\
# Independence receipt

Generated from attempt ledger and contract audits. Not a cryptographic proof.

| Attempt | Role | Agent | Kind | Transport | Contract audit |
| --- | --- | --- | --- | --- | --- |
",
    );
    for path in attempt_dirs_a(root) {
        if !path.join("meta.tsv").is_file() {
            continue;
        }
        let id = file_name(&path);
        let role = attempt_meta(root, &id, 5)?;
        let agent = attempt_meta(root, &id, 6)?;
        let kind = attempt_meta(root, &id, 7)?;
        let transport = attempt_transport(root, &id)?.unwrap_or_else(|| "-".to_string());
        let audit = contract_audit_word(&path.join("contract-audit.md"));
        body.push_str(&format!(
            "| {id} | {role} | {agent} | {kind} | {transport} | {audit} |\n"
        ));
    }
    body.push_str(
        "\n## Ladder\n\n1. multi-agent (preferred)\n2. acp (single-product isolation)\n3. subagent (only after ACP probe failure)\n4. stop if none invocable\n",
    );
    fs::write(root.join("INDEPENDENCE.md"), body)?;
    Ok(())
}

fn contract_audit_word(path: &Path) -> String {
    if !path.is_file() {
        return "MISSING".to_string();
    }
    let Ok(text) = fs::read_to_string(path) else {
        return "MISSING".to_string();
    };
    records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix("VERDICT: "))
        .filter(|word| !word.is_empty())
        .unwrap_or("MISSING")
        .to_string()
}

fn attempt_contract_audit_pass(root: &Path, id: &str) -> Result<bool, GuidedError> {
    let path = attempt_dir(root, id)?.join("contract-audit.md");
    if !is_regular(&path) {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    Ok(records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix("VERDICT: "))
        == Some("PASS"))
}

fn attempt_transport(root: &Path, id: &str) -> Result<Option<String>, GuidedError> {
    let path = attempt_dir(root, id)?.join("transport");
    if !is_regular(&path) {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let line = records(&text)
        .into_iter()
        .next()
        .unwrap_or("")
        .replace('\r', "");
    match line.as_str() {
        "multi-agent" | "acp" | "subagent" => Ok(Some(line)),
        _ => Ok(None),
    }
}

fn attempt_dir(root: &Path, id: &str) -> Result<PathBuf, GuidedError> {
    if !valid_attempt_id(id) {
        return Err(message(format!("invalid attempt id: {id}")));
    }
    let path = root.join("attempts").join(id);
    if !path.is_dir() {
        return Err(message(format!("no such attempt: {id}")));
    }
    Ok(path)
}

fn valid_attempt_id(id: &str) -> bool {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'.' || b == b'A')
    {
        return false;
    }
    let mut parts = id.split('.');
    let Some(head) = parts.next() else {
        return false;
    };
    let Some(mid) = parts.next() else {
        return false;
    };
    let Some(tail) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    let Some(digits) = head.strip_prefix('A') else {
        return false;
    };
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && !mid.is_empty()
        && mid.bytes().all(|b| b.is_ascii_digit())
        && !tail.is_empty()
        && tail.bytes().all(|b| b.is_ascii_digit())
}

fn attempt_meta(root: &Path, id: &str, column: usize) -> Result<String, GuidedError> {
    let text = fs::read_to_string(attempt_dir(root, id)?.join("meta.tsv"))?;
    let row = records(&text).get(1).copied().unwrap_or("");
    Ok(split_tabs(row)
        .get(column.saturating_sub(1))
        .copied()
        .unwrap_or("")
        .to_string())
}

fn attempt_state(root: &Path, id: &str) -> Result<String, GuidedError> {
    let text = fs::read_to_string(attempt_dir(root, id)?.join("events.tsv"))?;
    let mut state = String::new();
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        state = split_tabs(rec).first().copied().unwrap_or("").to_string();
    }
    Ok(state)
}

fn attempt_pid(root: &Path, id: &str) -> Result<String, GuidedError> {
    let text = fs::read_to_string(attempt_dir(root, id)?.join("events.tsv"))?;
    let mut pid = "-".to_string();
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let value = split_tabs(rec).get(2).copied().unwrap_or("");
        if !value.is_empty() && value != "-" {
            pid = value.to_string();
        }
    }
    Ok(pid)
}

fn attempt_pid_alive(root: &Path, id: &str) -> Result<bool, GuidedError> {
    let pid = attempt_pid(root, id)?;
    Ok(pid_alive(&pid))
}

fn pid_alive(pid: &str) -> bool {
    if pid.is_empty() || pid == "-" {
        return false;
    }
    Command::new("ps")
        .args(["-p", pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn attempt_terminal(state: &str) -> bool {
    matches!(
        state,
        "RETURNED" | "TIMEOUT" | "STOPPED" | "ABANDONED" | "SUPERSEDED"
    )
}

fn attempt_event(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    new: &str,
    pid: &str,
    reason: &str,
) -> Result<(), GuidedError> {
    if reason.contains('\t') || reason.contains('\n') {
        return Err(message("attempt reason must be one line without tabs"));
    }
    let ad = attempt_dir(root, id)?;
    let lock = ad.join(".event.lock");
    if fs::create_dir(&lock).is_err() {
        return Err(message(format!("attempt update already in progress: {id}")));
    }
    let result = (|| {
        let old = attempt_state(root, id)?;
        if attempt_terminal(&old) {
            return Err(message(format!("attempt {id} is terminal: {old}")));
        }
        let events = ad.join("events.tsv");
        let mut body = fs::read(&events)?;
        body.extend(format!("{new}\t{}\t{pid}\t{reason}\n", clock.now_unix()).into_bytes());
        let tmp = ad.join(format!(".events.{}.tmp", std::process::id()));
        fs::write(&tmp, &body)?;
        fs::rename(&tmp, &events)?;
        Ok(())
    })();
    let _ = fs::remove_dir(&lock);
    result
}

fn reclaim_dead_attempt(root: &Path, clock: &dyn Clock, id: &str) -> Result<(), GuidedError> {
    let current = attempt_state(root, id)?;
    match current.as_str() {
        "RUNNING" | "OVERDUE" => {}
        "DISPATCHED" => {
            return Err(message(format!(
                "attempt reclaim requires RUNNING or OVERDUE: {id} never started, so there is no pid to reclaim — end it with: {} attempt finish {id} ABANDONED \"<what you observed>\"",
                self_path(root)
            )));
        }
        _ => return Err(message("attempt reclaim requires RUNNING or OVERDUE")),
    }
    let pid = attempt_pid(root, id)?;
    if pid_alive(&pid) {
        return Err(message(format!(
            "refused: attempt {id} pid {pid} is still alive"
        )));
    }
    attempt_event(root, clock, id, "STOPPED", &pid, "dead-pid")?;
    let slug = attempt_meta(root, id, 2)?;
    if is_claim_slug(&slug) {
        return Ok(());
    }
    let task = attempt_meta(root, id, 3)?;
    let block = if task != "-" {
        "TASK_BLOCKED"
    } else {
        "OPERATOR_DECISION"
    };
    state_attempt_update(root, clock, &slug, "BLOCKED", "-", block)
}

fn state_attempt_update(
    root: &Path,
    clock: &dyn Clock,
    slug: &str,
    status: &str,
    inflight: &str,
    block: &str,
) -> Result<(), GuidedError> {
    let stage = state_value(root, slug, 3)?.unwrap_or_default();
    let wid = state_value(root, slug, 4)?.unwrap_or_default();
    let risk = state_value(root, slug, 5)?.unwrap_or_default();
    state_update_item(
        root, clock, slug, status, &stage, &wid, &risk, inflight, block,
    )
}

fn is_claim_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    bytes.len() >= 2 && bytes[0] == b'C' && bytes[1].is_ascii_digit()
}

fn drive_state_name(line: &str) -> &'static str {
    if line.starts_with("NEXT CONFIGURE") {
        "CONFIGURE"
    } else if line.starts_with("WAIT PANEL") {
        "WAIT_PANEL"
    } else if line.starts_with("NEXT INTAKE") {
        "INTAKE"
    } else if line.starts_with("NEXT INVESTIGATE") {
        "INVESTIGATE"
    } else if line.starts_with("NEXT PROPOSE") {
        "PROPOSE"
    } else if line.starts_with("WAIT APPROVAL") {
        "WAIT_APPROVAL"
    } else if line.starts_with("NEXT PLAN") {
        "PLAN"
    } else if line.starts_with("NEXT EXECUTE") {
        "EXECUTE"
    } else if line.starts_with("NEXT REVIEW") {
        "REVIEW"
    } else if line.starts_with("WAIT ") {
        "WAIT"
    } else if line.starts_with("ESCALATE") {
        "ESCALATE"
    } else if line.starts_with("DONE") {
        "DONE"
    } else {
        "UNKNOWN"
    }
}

fn drive_human_gate(state: &str) -> &'static str {
    match state {
        "WAIT_PANEL" => "approve-panel",
        "WAIT_APPROVAL" => "approve",
        "ESCALATE" => "escalate",
        "DONE" => "cycle-clean",
        _ => "none",
    }
}

fn drive_active_item(root: &Path) -> Result<String, GuidedError> {
    if !uses_managed_lifecycle(root)? || !root.join("STATE.tsv").is_file() {
        return Ok("-".to_string());
    }
    Ok(current_slug(root)?.unwrap_or_default())
}

fn drive_inflight(root: &Path, item: &str) -> Result<String, GuidedError> {
    if item.is_empty() || item == "-" {
        return Ok("-".to_string());
    }
    match state_value(root, item, 6) {
        Ok(Some(value)) if !value.is_empty() => Ok(value),
        _ => Ok("-".to_string()),
    }
}

fn current_slug(root: &Path) -> Result<Option<String>, GuidedError> {
    let text = fs::read_to_string(root.join("STATE.tsv"))?;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields = split_tabs(rec);
        let status = fields.get(1).copied().unwrap_or("");
        if status == "ACTIVE" || status == "BLOCKED" {
            return Ok(Some(fields.first().copied().unwrap_or("").to_string()));
        }
    }
    Ok(None)
}

fn blocked_independence(root: &Path) -> Result<Option<String>, GuidedError> {
    let text = fs::read_to_string(root.join("STATE.tsv"))?;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields = split_tabs(rec);
        if fields.get(1).copied() == Some("BLOCKED")
            && fields.get(6).copied() == Some("INDEPENDENCE_UNAVAILABLE")
        {
            return Ok(Some(fields.first().copied().unwrap_or("").to_string()));
        }
    }
    Ok(None)
}

fn engine_version(root: &Path) -> Result<String, GuidedError> {
    let path = root.join("VERSION");
    if !is_nonempty_regular(&path) {
        return Ok("unknown".to_string());
    }
    let text = fs::read_to_string(path)?;
    Ok(records(&text).into_iter().next().unwrap_or("").to_string())
}

/// File mtimes are the evidence files' own stamps, not an epoch this crate mints.
fn drive_last_evidence(root: &Path) -> Result<String, GuidedError> {
    let mut found = Vec::new();
    for name in ["claims", "items"] {
        collect_evidence(&root.join(name), root, &mut found);
    }
    if found.is_empty() {
        return Ok("-".to_string());
    }
    found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok(found
        .last()
        .map(|(_, _, rel)| rel.clone())
        .unwrap_or_else(|| "-".into()))
}

fn collect_evidence(dir: &Path, root: &Path, out: &mut Vec<(i64, String, String)>) {
    for path in read_dir_paths(dir) {
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            collect_evidence(&path, root, out);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let full = path.to_string_lossy().into_owned();
        if !full.contains("/evidence/") {
            continue;
        }
        let mtime = meta
            .modified()
            .ok()
            .and_then(|when| when.duration_since(UNIX_EPOCH).ok())
            .map(|dur| dur.as_secs() as i64)
            .unwrap_or(0);
        let rel = path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| full.clone());
        out.push((mtime, full, rel));
    }
}

fn recorded_independence(root: &Path) -> Result<String, GuidedError> {
    if !panel_approval_current(root)? {
        return Ok("-".to_string());
    }
    if let Some(token) = receipt_transport(root)? {
        return Ok(token);
    }
    if let Some(token) = first_attempt_transport(root)? {
        return Ok(token);
    }
    if let Ok(text) = fs::read_to_string(root.join("PANEL.md")) {
        if let Some(token) = first_transport_token(&text) {
            return Ok(token.to_string());
        }
    }
    Ok("-".to_string())
}

fn receipt_transport(root: &Path) -> Result<Option<String>, GuidedError> {
    let path = root.join("INDEPENDENCE.md");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    for line in records(&text) {
        if !line.starts_with('|') || line.starts_with("| ---") || line.starts_with("| Attempt") {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if let Some(token) = cells.get(5).copied() {
            if matches!(token, "multi-agent" | "acp" | "subagent") {
                return Ok(Some(token.to_string()));
            }
        }
    }
    Ok(None)
}

fn first_attempt_transport(root: &Path) -> Result<Option<String>, GuidedError> {
    for path in attempt_dirs_a(root) {
        let id = file_name(&path);
        if !valid_attempt_id(&id) {
            continue;
        }
        if let Some(token) = attempt_transport(root, &id)? {
            return Ok(Some(token));
        }
    }
    Ok(None)
}

fn working_mode_yes(root: &Path) -> bool {
    let Ok(text) = fs::read_to_string(root.join("PROGRAM")) else {
        return false;
    };
    records(&text).contains(&"working-mode: yes")
}

fn drive_locked(root: &Path) -> bool {
    // `[ -d ]` is false for a regular file. That file is not a lock.
    root.join(".drive.lock").is_dir()
}

fn problem_needs_intake(path: &Path) -> bool {
    let sized = fs::metadata(path)
        .map(|meta| meta.len() > 0)
        .unwrap_or(false);
    if !sized {
        return true;
    }
    fs::read_to_string(path)
        .map(|text| text.contains("TEMPLATE-PROBLEM-NEEDS-INPUT"))
        .unwrap_or(false)
}

fn cycle_problem_title(path: &Path) -> String {
    fs::read_to_string(path)
        .map(|text| problem_title_of(&text))
        .unwrap_or_default()
}

fn problem_title_of(text: &str) -> String {
    for line in records(text) {
        if is_blank(line) {
            continue;
        }
        let body = match line.strip_prefix('#') {
            Some(rest) => rest.trim_start_matches(|c: char| c.is_ascii_whitespace()),
            None => line,
        };
        return body.replace('`', "");
    }
    String::new()
}

fn problem_risk(text: &str) -> &'static str {
    if contains_word_ci(text, "HIGH") {
        "HIGH"
    } else if contains_word_ci(text, "MEDIUM") {
        "MEDIUM"
    } else {
        "LOW"
    }
}

fn title_is_leftover_catalog(lower: &str) -> bool {
    lower.contains("leftover") || lower.contains("remainder") || contains_whats_left(lower)
}

fn contains_whats_left(lower: &str) -> bool {
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if &bytes[i..i + 4] == b"what" {
            if bytes.get(i + 4..i + 10) == Some(b"s left".as_slice()) {
                return true;
            }
            if bytes.get(i + 5..i + 11) == Some(b"s left".as_slice()) {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn names_outcome(text: &str) -> bool {
    contains_ci(text, "falsif") || contains_ci(text, "## outcome") || contains_must_letter(text)
}

fn contains_must_letter(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut from = 0;
    while let Some(at) = lower[from..].find("must ") {
        let idx = from + at + 5;
        if bytes.get(idx).is_some_and(|b| b.is_ascii_lowercase()) {
            return true;
        }
        from = idx;
        if from >= lower.len() {
            break;
        }
    }
    false
}

fn program_field(root: &Path, key: &str) -> Option<String> {
    let value = program_value(root, key);
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn program_value(root: &Path, key: &str) -> String {
    let Ok(text) = fs::read_to_string(root.join("PROGRAM")) else {
        return String::new();
    };
    let prefix = format!("{key}: ");
    records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string()
}

fn self_path(root: &Path) -> String {
    if let Ok(value) = std::env::var("CRUCIBLE_SELF") {
        if !value.is_empty() {
            return value;
        }
    }
    if root.join("PROGRAM").is_file() {
        if let Some(repo) = program_field(root, "repo") {
            let rr = physical(Path::new(&repo));
            let rt = physical(root);
            if let Ok(rest) = rt.strip_prefix(&rr) {
                if !rest.as_os_str().is_empty() {
                    return format!("{}/crucible", rest.display());
                }
            }
        }
    }
    "./crucible".to_string()
}

fn physical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn nonempty_or_dash(value: String) -> String {
    if value.is_empty() {
        "-".to_string()
    } else {
        value
    }
}

fn is_blank(line: &str) -> bool {
    line.bytes().all(|b| b.is_ascii_whitespace())
}

fn archive_stamp(unix: i64) -> String {
    let days = unix.div_euclid(86400);
    let sod = unix.rem_euclid(86400) as u64;
    let (y, m, d) = civil_from_days(days);
    let hh = sod / 3600;
    let mm = (sod % 3600) / 60;
    let ss = sod % 60;
    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

fn civil_from_days(z: i64) -> (i64, u64, u64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn workid(root: &Path, slug: &str) -> Result<String, GuidedError> {
    let item = root.join("items").join(slug);
    let target = item.join("TARGET");
    if target.is_file() {
        let text = fs::read_to_string(&target)?;
        let repo = records(&text)
            .into_iter()
            .find_map(|line| line.strip_prefix("repo: "))
            .unwrap_or("");
        let branch = records(&text)
            .into_iter()
            .find_map(|line| line.strip_prefix("branch: "))
            .unwrap_or("");
        if repo.is_empty() || branch.is_empty() {
            return Ok("NOBRANCH".to_string());
        }
        let out = Command::new("git")
            .args(["-C", repo, "rev-parse", "--verify", "--quiet", branch])
            .output()?;
        if !out.status.success() {
            return Ok("NOBRANCH".to_string());
        }
        let sha = String::from_utf8_lossy(&out.stdout);
        let sha = sha.trim();
        if sha.is_empty() {
            return Ok("NOBRANCH".to_string());
        }
        return Ok(sha.chars().take(12).collect());
    }
    let files = list_files(&item.join("work"));
    if files.is_empty() {
        return Ok("EMPTY".to_string());
    }
    let mut manifest = String::new();
    for path in files {
        let rel = path.strip_prefix(&item).unwrap_or(&path);
        let mode = fs::symlink_metadata(&path)
            .map(|meta| ls_mode(meta.mode()))
            .unwrap_or_else(|_| "----------".to_string());
        manifest.push_str(&format!("{} {mode} {}\n", rel.display(), hash_file(&path)?));
    }
    Ok(crate::panel::h12(manifest.as_bytes()))
}

fn stale_evidence(root: &Path, slug: &str, wid: &str) -> Vec<String> {
    let dir = root.join("items").join(slug).join("evidence");
    let mut paths = read_dir_paths(&dir);
    paths.sort();
    let mut out = Vec::new();
    for path in paths {
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.starts_with(".partial.") {
            continue;
        }
        let Some(stem) = name.strip_suffix(".txt") else {
            continue;
        };
        let id = stem.rsplit('.').next().unwrap_or(stem);
        if id == wid {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.display().to_string());
        out.push(rel);
    }
    out
}

fn ls_mode(mode: u32) -> String {
    let mut out = String::with_capacity(10);
    out.push(match mode & 0o170000 {
        0o100000 => '-',
        0o040000 => 'd',
        0o120000 => 'l',
        0o010000 => 'p',
        0o020000 => 'c',
        0o060000 => 'b',
        0o140000 => 's',
        _ => '?',
    });
    let specs = [
        (0o400, 0o200, 0o100, 0o4000, 's', 'S'),
        (0o040, 0o020, 0o010, 0o2000, 's', 'S'),
        (0o004, 0o002, 0o001, 0o1000, 't', 'T'),
    ];
    for (r, w, x, special, yes, no) in specs {
        out.push(if mode & r != 0 { 'r' } else { '-' });
        out.push(if mode & w != 0 { 'w' } else { '-' });
        out.push(match (mode & special != 0, mode & x != 0) {
            (true, true) => yes,
            (true, false) => no,
            (false, true) => 'x',
            (false, false) => '-',
        });
    }
    out
}

fn list_files(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for path in read_dir_paths(dir) {
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                walk(&path, out);
            } else if meta.is_file() {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

struct WorktreeRec {
    repo: String,
    path: String,
    branch: String,
}

fn cycle_worktree_records(root: &Path) -> Result<Vec<WorktreeRec>, GuidedError> {
    let prefix = if root.join("worktrees").is_dir() {
        format!("{}/", physical(&root.join("worktrees")).display())
    } else {
        format!("{}/", root.join("worktrees").display())
    };
    let mut repos = Vec::new();
    if let Some(repo) = program_field(root, "repo") {
        repos.push(repo);
    }
    for path in attempt_child_dirs(root) {
        let task = path.join("task.tsv");
        if !task.is_file() {
            continue;
        }
        let text = fs::read_to_string(task)?;
        if let Some(row) = records(&text).get(1).copied() {
            let repo = split_tabs(row).first().copied().unwrap_or("");
            if !repo.is_empty() {
                repos.push(repo.to_string());
            }
        }
    }
    repos.sort();
    repos.dedup();
    let mut out = Vec::new();
    for repo in repos {
        if !Path::new(&repo).is_dir() {
            continue;
        }
        let Ok(output) = Command::new("git")
            .args(["-C", &repo, "worktree", "list", "--porcelain"])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        out.extend(parse_worktrees(&text, &repo, &prefix));
    }
    Ok(out)
}

fn parse_worktrees(text: &str, repo: &str, prefix: &str) -> Vec<WorktreeRec> {
    let mut out = Vec::new();
    let mut path = String::new();
    let mut branch = "-".to_string();
    let mut keep = false;
    for line in records(text) {
        if let Some(rest) = line.strip_prefix("worktree ") {
            path = rest.to_string();
            keep = path.starts_with(prefix);
            branch = "-".to_string();
            continue;
        }
        if keep {
            if let Some(rest) = line.strip_prefix("branch ") {
                branch = rest.strip_prefix("refs/heads/").unwrap_or(rest).to_string();
            }
            if line.is_empty() {
                out.push(WorktreeRec {
                    repo: repo.to_string(),
                    path: path.clone(),
                    branch: branch.clone(),
                });
                keep = false;
            }
        }
    }
    if keep {
        out.push(WorktreeRec {
            repo: repo.to_string(),
            path,
            branch,
        });
    }
    out
}

fn cycle_worktree_blocker(path: &Path) -> Result<Option<String>, GuidedError> {
    let output = Command::new("git")
        .args([
            "-C",
            &path.display().to_string(),
            "rev-parse",
            "--absolute-git-dir",
        ])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let gitdir = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if fs::symlink_metadata(gitdir.join("CHERRY_PICK_HEAD")).is_ok() {
        return Ok(Some(format!(
            "in-progress cherry-pick; clear it with: git -C {} cherry-pick --abort",
            path.display()
        )));
    }
    let status = Command::new("git")
        .args(["-C", &path.display().to_string(), "status", "--porcelain"])
        .output()?;
    if status.status.success() && !status.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(Some(format!(
            "uncommitted changes; commit them, or discard with: git -C {} stash --include-untracked",
            path.display()
        )));
    }
    Ok(None)
}

fn cycle_husk_programs(root: &Path) -> Vec<String> {
    let Some(parent) = root.parent() else {
        return Vec::new();
    };
    if parent.file_name().and_then(|s| s.to_str()) != Some(".crucible") {
        return Vec::new();
    }
    let mut kids = read_dir_paths(parent);
    kids.sort();
    let mut out = Vec::new();
    for cand in kids {
        if !cand.is_dir() || cand == root || cand.join("PROGRAM").is_file() {
            continue;
        }
        if let Some(name) = cand.file_name().and_then(|s| s.to_str()) {
            // `"$parent"/*` does not list dot-directories.
            if name.starts_with('.') {
                continue;
            }
            out.push(format!("{name}/"));
        }
    }
    out
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_dir())
        .unwrap_or(false)
}

/// `find -depth -type d` without `-L`: do not descend through a symlink.
fn remove_empty_dirs(dir: &Path) {
    if !is_real_dir(dir) {
        return;
    }
    for path in read_dir_paths(dir) {
        if is_real_dir(&path) {
            remove_empty_dirs(&path);
        }
    }
    let _ = fs::remove_dir(dir);
}

fn attempt_child_dirs(root: &Path) -> Vec<PathBuf> {
    let mut paths = read_dir_paths(&root.join("attempts"));
    // `"$ROOT"/attempts/*` skips dot-names.
    paths.retain(|path| path.is_dir() && !file_name(path).starts_with('.'));
    paths.sort();
    paths
}

/// `(success, merged stdout and stderr)`. Trailing newlines are stripped, as in `$(...)`.
fn git_combined(args: &[&str]) -> Result<(bool, String), String> {
    let mut child = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| err.to_string())?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "git stdout missing".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "git stderr missing".to_string())?;
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });
    let status = child.wait().map_err(|err| err.to_string())?;
    let mut buf = err_handle.join().unwrap_or_default();
    buf.extend(out_handle.join().unwrap_or_default());
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    while text.ends_with('\n') || text.ends_with('\r') {
        text.pop();
    }
    Ok((status.success(), text))
}

fn attempt_dirs_a(root: &Path) -> Vec<PathBuf> {
    attempt_child_dirs(root)
        .into_iter()
        .filter(|path| file_name(path).starts_with('A'))
        .collect()
}

fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut paths = read_dir_paths(dir);
    paths.retain(|path| path.extension().is_some_and(|ext| ext == "md") && path.is_file());
    paths.sort();
    paths
}

fn read_dir_paths(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.flatten().map(|ent| ent.path()).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
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
    use crate::panel::panel_id;
    use crucible_contract::{EventKind, FixedClock, WalkSnapshot};
    use crucible_kernel::read_events;
    use std::sync::atomic::{AtomicU64, Ordering};

    const EPOCH: i64 = 1_700_000_000;
    const STAMP: &str = "20231114T221320Z";

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-guided-cycle-{}-{}",
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

    fn clock() -> FixedClock {
        FixedClock::new(EPOCH)
    }

    fn program(prog: &Path, repo: &Path, extra: &str) {
        fs::create_dir_all(prog).unwrap();
        fs::write(
            prog.join("PROGRAM"),
            format!(
                "repo: {}\nprogram: work\nlifecycle: managed\ncycle: guided\n{extra}",
                repo.display()
            ),
        )
        .unwrap();
        fs::write(prog.join("STATE.tsv"), format!("{STATE_HEADER}\n")).unwrap();
    }

    fn agents(prog: &Path) {
        fs::write(
            prog.join("agents.tsv"),
            "\
c0\tkindA\tm\thigh\techo {BRIEF}\n\
a1\tkindA\tm\thigh\techo {BRIEF}\n\
a2\tkindB\tm\thigh\techo {BRIEF}\n\
mk1\tkindA\tm\thigh\techo {BRIEF}\n\
j1\tkindB\tm\thigh\techo {BRIEF}\n\
j2\tkindB\tm\thigh\techo {BRIEF}\n",
        )
        .unwrap();
    }

    fn panel(prog: &Path) {
        fs::write(
            prog.join("PANEL.md"),
            "\
# Panel

## Agents

- c0, a1, a2, mk1, j1, j2

## Roles

Cast in PANEL.ASSIGN.tsv.

## Risk posture

LOW for fixture onboarding verification.

## Isolation transport

Prefer multi-agent. ACP before subagent on single-product hosts.

## Independence ladder

1. multi-agent
2. acp
3. subagent after ACP probe failure

## Waivers

NONE for this fixture.
",
        )
        .unwrap();
        fs::write(
            prog.join("PANEL.ASSIGN.tsv"),
            "\
role\tagent\trequired\tnotes
coordinator\tc0\tyes\tthis session
claim-auditor\ta1\tyes\t
claim-auditor\ta2\tyes\t
scout\ta1\tno\t
maker\tmk1\tyes\t
reviewer\tj1\tyes\t
contract-auditor\tj2\tyes\t
",
        )
        .unwrap();
    }

    fn floor_of(repo: &Path) -> crucible_contract::Floor {
        let snap = WalkSnapshot::from_wm_dir(repo, &clock());
        assert!(snap.available, "{snap:?}");
        assert!(snap.closed.is_none());
        assert!(!repo.join(".wm").join("CLOSED").exists());
        snap.floor.expect("floor")
    }

    #[test]
    fn archive_stamp_is_compact_utc() {
        assert_eq!(archive_stamp(EPOCH), STAMP);
        assert_eq!(archive_stamp(1_700_000_111), "20231114T221511Z");
    }

    #[test]
    fn cycle_lines_project_until_working_mode() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        let clock = clock();

        let out = cycle(&prog, &[], &clock).unwrap();
        assert!(out.starts_with("NEXT CONFIGURE — replace placeholder"));
        let status = fs::read_to_string(prog.join("STATUS.md")).unwrap();
        assert!(status.contains(&format!("updated-epoch: {EPOCH}\n")));
        assert!(status.contains("state: CONFIGURE\n"));
        let floor = floor_of(&repo);
        assert_eq!(floor.station, "SHAPE");
        assert!(floor.card.starts_with("NEXT CONFIGURE"));
        assert_eq!(floor.independence, "-");
        assert!(read_events(&repo)
            .unwrap()
            .iter()
            .all(|e| e.kind != EventKind::Halt));

        agents(&prog);
        let out = cycle(&prog, &["status"], &clock).unwrap();
        assert!(out.starts_with("NEXT CONFIGURE — cast personas"));

        fs::write(
            prog.join("PANEL.ASSIGN.tsv"),
            "role\tagent\trequired\tnotes\n",
        )
        .unwrap();
        let out = cycle(&prog, &[], &clock).unwrap();
        assert!(out.starts_with("NEXT CONFIGURE — fix PANEL.md"));

        panel(&prog);
        let out = cycle(&prog, &[], &clock).unwrap();
        let id = panel_id(&prog).unwrap().unwrap();
        assert_eq!(
            out,
            format!("WAIT PANEL — show agent inventory and role casting {id} to the operator; do not investigate or build\n")
        );
        let floor = floor_of(&repo);
        assert_eq!(floor.station, "ANDON");
        assert_eq!(floor.card, format!("STOP-ASK {out}").trim());
        assert_eq!(floor.independence, "-");
        assert_eq!(floor.andon, floor.card);
        let halts: Vec<_> = read_events(&repo)
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == EventKind::Halt)
            .collect();
        assert_eq!(halts.len(), 1);

        fs::create_dir(prog.join(".drive.lock")).unwrap();
        let err = cycle(&prog, &["approve-panel"], &clock)
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "refused: cycle approve-panel is a human gate — stop drive first (drive never auto-approves)"
        );
        fs::remove_dir(prog.join(".drive.lock")).unwrap();
        fs::write(prog.join(".drive.lock"), "not a lock\n").unwrap();
        let approved = cycle(&prog, &["approve-panel"], &clock).unwrap();
        assert_eq!(approved, format!("approved panel {id}\n"));
        fs::remove_file(prog.join(".drive.lock")).unwrap();
        let record =
            fs::read_to_string(prog.join("panel-approvals").join(format!("{id}.md"))).unwrap();
        assert!(record.contains(&format!("approved-epoch: {EPOCH}\n")));
        assert!(!record.contains("reapproved-epoch"));
        assert_eq!(
            cycle(&prog, &["approve-panel"], &clock).unwrap(),
            format!("panel {id} is already approved\n")
        );

        let saved = fs::read(prog.join("PANEL.md")).unwrap();
        let mut changed = saved.clone();
        changed.extend(b"\nnote\n");
        fs::write(prog.join("PANEL.md"), &changed).unwrap();
        assert!(cycle(&prog, &[], &clock).unwrap().starts_with("WAIT PANEL"));
        cycle(&prog, &["approve-panel"], &clock).unwrap();
        fs::write(prog.join("PANEL.md"), &saved).unwrap();
        cycle(&prog, &["approve-panel"], &clock).unwrap();
        let again =
            fs::read_to_string(prog.join("panel-approvals").join(format!("{id}.md"))).unwrap();
        assert!(again.contains(&format!("reapproved-epoch: {EPOCH}\n")));

        fs::write(
            prog.join("PROBLEM.md"),
            "# Problem\n\nTEMPLATE-PROBLEM-NEEDS-INPUT\n",
        )
        .unwrap();
        let out = cycle(&prog, &[], &clock).unwrap();
        assert_eq!(
            out,
            "NEXT INTAKE — capture the operator's problem in PROBLEM.md\n"
        );
        let floor = floor_of(&repo);
        assert_eq!(floor.station, "SHAPE");
        assert_eq!(floor.card, out.trim());
        assert_eq!(floor.independence, "multi-agent");

        let report = tmp.root.join("report.md");
        fs::write(
            &report,
            "# Broken behavior\n\nThe program does not preserve approved scope.\n",
        )
        .unwrap();
        let bound = cycle(&prog, &["problem", report.to_str().unwrap()], &clock).unwrap();
        assert_eq!(bound, format!("{}/PROBLEM.md\n", prog.display()));
        let ctx = fs::read_to_string(prog.join("PANEL.CONTEXT.md")).unwrap();
        assert!(ctx.contains("problem-title: Broken behavior\n"));
        assert!(ctx.contains("risk: LOW\n"));
        assert!(ctx.contains(&format!("refreshed-epoch: {EPOCH}\n")));
        assert!(ctx.contains("Synced from PROBLEM.md"));
        let out = cycle(&prog, &[], &clock).unwrap();
        assert_eq!(
            out,
            "NEXT INVESTIGATE — split PROBLEM.md into atomic claims and verify them via independent agents\n"
        );
        assert_eq!(floor_of(&repo).station, "SHAPE");

        fs::write(
            prog.join("CLAIMS.md"),
            "\
# CLAIMS

### C1 scope is wrong
    scout: FULLY-EXISTS
    item: done
    status: CLOSED
",
        )
        .unwrap();
        let verdicts = prog.join("claims").join("C1").join("verdicts");
        fs::create_dir_all(&verdicts).unwrap();
        fs::write(verdicts.join("a1.md"), "CLAIM-VERDICT: TRUE\n").unwrap();
        let attempt = prog.join("attempts").join("A1.2.3");
        fs::create_dir_all(&attempt).unwrap();
        fs::write(
            attempt.join("meta.tsv"),
            "\
attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of
A1.2.3\tC1\t-\tCLAIM\tclaim-auditor\ta1\tkindA\t-\tFOCUSED\tDISPATCHED\t1\t2\t-
",
        )
        .unwrap();
        fs::write(attempt.join("transport"), "multi-agent\n").unwrap();
        fs::write(attempt.join("contract-audit.md"), "VERDICT: PASS\n").unwrap();
        fs::write(
            attempt.join("events.tsv"),
            "state\tepoch\tpid\treason\nSTOPPED\t1\t-\tfixture\n",
        )
        .unwrap();
        fs::write(
            prog.join("PROPOSAL.md"),
            "\
# Proposal
## Verified problem
The scope was not preserved.
## Proposed outcome
Preserve it.
## Non-goals
No extra product.
## Backlog
None.
## Verification
Two recorded checks.
",
        )
        .unwrap();
        let out = cycle(&prog, &[], &clock).unwrap();
        let proposal = proposal_id(&prog).unwrap().unwrap();
        assert_eq!(
            out,
            format!(
                "WAIT APPROVAL — show proposal {proposal} to the operator; do not plan or build\n"
            )
        );
        let floor = floor_of(&repo);
        assert_eq!(floor.station, "ANDON");
        assert!(floor.card.starts_with("STOP-ASK WAIT APPROVAL"));

        let approved = cycle(&prog, &["approve"], &clock).unwrap();
        assert_eq!(approved, format!("approved proposal {proposal}\n"));
        let prec =
            fs::read_to_string(prog.join("approvals").join(format!("{proposal}.md"))).unwrap();
        assert!(prec.contains(&format!("approved-epoch: {EPOCH}\n")));
        assert_eq!(
            cycle(&prog, &["approve"], &clock).unwrap(),
            format!("proposal {proposal} is already approved\n")
        );

        let out = cycle(&prog, &[], &clock).unwrap();
        let done = "DONE — no admittable claim remains. Next: cycle clean --dry-run (or cycle problem FILE --next). Drive never --apply.";
        assert!(out.starts_with(&format!("{done}\n")));
        assert!(out.contains("CLEANUP — next verb:"));
        let status = fs::read_to_string(prog.join("STATUS.md")).unwrap();
        assert!(status.contains(&format!("updated-epoch: {EPOCH}\n")));
        assert!(status.contains("worth: NO-BUILD\n"));
        assert!(status.contains("next-human-gate: cycle-clean\n"));
        let floor = floor_of(&repo);
        assert_eq!(floor.station, "DONE");
        assert_eq!(floor.card, done);
        assert_eq!(floor.andon, "-");
        assert_eq!(floor.independence, "multi-agent");

        let cleaned = cycle(&prog, &["clean", "--dry-run"], &clock).unwrap();
        assert!(cleaned.contains("NO_SESSION_ARTIFACTS\n"));
        assert!(cleaned.contains("KEEP "));
        assert!(prog.join("PROBLEM.md").is_file());

        let next = tmp.root.join("next.md");
        fs::write(
            &next,
            "# Next title here\n\nA falsifiable outcome remains.\n",
        )
        .unwrap();
        let archived = cycle(
            &prog,
            &["problem", next.to_str().unwrap(), "--next"],
            &clock,
        )
        .unwrap();
        let dest = prog.join("history").join(format!("{STAMP}-{proposal}"));
        assert!(archived.starts_with(&format!("archived {}\n", dest.display())));
        assert!(archived.contains(&format!("{}/PROBLEM.md\n", prog.display())));
        assert!(dest.join("PROPOSAL.md").is_file());
        let ctx = fs::read_to_string(prog.join("PANEL.CONTEXT.md")).unwrap();
        assert!(ctx.contains("problem-title: Next title here\n"));
        assert!(ctx.contains(&format!("refreshed-epoch: {EPOCH}\n")));
        assert!(ctx.contains("Refreshed by cycle problem --next"));
        let approval = fs::read_to_string(prog.join("PANEL.APPROVAL")).unwrap();
        assert!(approval.contains(&format!("approved-epoch: {EPOCH}\n")));
        assert!(approval.contains("refreshed-by: cycle-problem-next\n"));
        let panel_text = fs::read_to_string(prog.join("PANEL.md")).unwrap();
        assert!(panel_text.starts_with("# Next title here\n"));
        assert!(panel_text.contains("## Agents\n"));
        let out = cycle(&prog, &[], &clock).unwrap();
        assert!(out.starts_with("NEXT INVESTIGATE — split PROBLEM.md"));
        assert!(!out.starts_with("WAIT PANEL"));
    }

    #[test]
    fn working_mode_does_not_project() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(repo.join(".wm")).unwrap();
        program(&prog, &repo, "working-mode: yes\n");
        let sentinel = "station: SHAPE\ncard: KEEP\nwip: -\nandon: -\nindependence: CROSS-FAMILY\nelapsed: 0\nevidence:\n";
        fs::write(repo.join(".wm").join("FLOOR.md"), sentinel).unwrap();
        let out = cycle(&prog, &[], &clock()).unwrap();
        assert!(out.starts_with("NEXT CONFIGURE"));
        assert_eq!(
            fs::read_to_string(repo.join(".wm").join("FLOOR.md")).unwrap(),
            sentinel
        );
        assert!(!repo.join(".wm").join("TRACE.tsv").exists());
        assert!(!repo.join(".wm").join("EVENTS").exists());
        assert!(prog.join("STATUS.md").is_file());
    }

    #[test]
    fn abandon_predicts_archive_and_refuses_a_second_one() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        let clock = clock();
        let out = cycle(
            &prog,
            &["problem", "--abandon", "leftover", "title"],
            &clock,
        )
        .unwrap();
        let dest = prog.join("history").join(format!("{STAMP}-noproposal"));
        assert_eq!(out, format!("abandoned {}\n", dest.display()));
        let abandon = fs::read_to_string(dest.join("ABANDON.md")).unwrap();
        assert_eq!(
            abandon,
            format!("reason: leftover title\nepoch: {EPOCH}\ndecision: ABANDONED\n")
        );
        assert!(dest.join("STATE.tsv").is_file());
        assert!(prog.join("STATE.tsv").is_file());
        assert!(!prog.join("PROBLEM.md").exists());
        let err = cycle(&prog, &["problem", "--abandon", "again"], &clock)
            .unwrap_err()
            .to_string();
        assert!(err.contains("archive already exists"));
        assert!(err.contains(&dest.display().to_string()));
    }

    #[test]
    fn regular_lock_file_is_not_a_human_gate() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        agents(&prog);
        panel(&prog);
        fs::write(prog.join(".drive.lock"), "regular\n").unwrap();
        let out = cycle(&prog, &["approve-panel"], &clock()).unwrap();
        assert!(out.starts_with("approved panel "));
        fs::create_dir(prog.join(".drive.lock.dir")).unwrap();
        fs::rename(prog.join(".drive.lock"), prog.join(".drive.lock.bak")).unwrap();
        fs::rename(prog.join(".drive.lock.dir"), prog.join(".drive.lock")).unwrap();
        let err = cycle(&prog, &["problem", "--abandon", "nope"], &clock())
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "refused: cycle problem --abandon is a human gate — stop drive first"
        );
    }

    #[test]
    fn problem_file_refusals_match_the_shell() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        agents(&prog);
        panel(&prog);
        cycle(&prog, &["approve-panel"], &clock()).unwrap();
        let bad = tmp.root.join("bad.md");
        fs::write(&bad, "# pr-status leftover\n").unwrap();
        let err = cycle(&prog, &["problem", bad.to_str().unwrap()], &clock())
            .unwrap_err()
            .to_string();
        assert!(err.contains("leftover pr-status title"));
        fs::write(&bad, "`workgraph nosuchverb` is not a CLI verb.\n").unwrap();
        let err = cycle(&prog, &["problem", bad.to_str().unwrap()], &clock())
            .unwrap_err()
            .to_string();
        assert!(err.contains("is not a CLI verb"));
        fs::write(prog.join(".drive.lock"), "").unwrap();
        let err = cycle(&prog, &["approve"], &clock())
            .unwrap_err()
            .to_string();
        assert!(!err.contains("human gate"), "{err}");
    }

    const PROPOSAL: &str = "\
# Proposal
## Verified problem
The scope was not preserved.
## Proposed outcome
Preserve it.
## Non-goals
No extra product.
## Backlog
None.
## Verification
Two recorded checks.
";

    fn approve_proposal(prog: &Path) {
        let id = proposal_id(prog).unwrap().unwrap();
        fs::write(
            prog.join("APPROVAL"),
            format!("proposal-id: {id}\nrecord: approvals/{id}.md\n"),
        )
        .unwrap();
    }

    fn item_file_guided_done(prog: &Path, repo: &Path, events: &str) {
        fs::create_dir_all(prog).unwrap();
        fs::write(
            prog.join("PROGRAM"),
            format!("repo: {}\nprogram: work\ncycle: guided\n", repo.display()),
        )
        .unwrap();
        agents(prog);
        panel(prog);
        cycle(prog, &["approve-panel"], &clock()).unwrap();
        fs::write(
            prog.join("PROBLEM.md"),
            "# Real gap\n\nA falsifiable outcome is missing.\n",
        )
        .unwrap();
        fs::write(
            prog.join("CLAIMS.md"),
            "\
# CLAIMS

### C1 scope is wrong
    scout: FULLY-EXISTS
    item: done
",
        )
        .unwrap();
        let verdicts = prog.join("claims").join("C1").join("verdicts");
        fs::create_dir_all(&verdicts).unwrap();
        fs::write(verdicts.join("a1.md"), "CLAIM-VERDICT: TRUE\n").unwrap();
        fs::write(prog.join("PROPOSAL.md"), PROPOSAL).unwrap();
        approve_proposal(prog);
        let attempt = prog.join("attempts").join("A1.2.3");
        fs::create_dir_all(&attempt).unwrap();
        fs::write(
            attempt.join("meta.tsv"),
            "\
attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of
A1.2.3\tC1\t-\tCLAIM\tclaim-auditor\ta1\tkindA\t-\tFOCUSED\tDISPATCHED\t1\t2\t-
",
        )
        .unwrap();
        fs::write(attempt.join("transport"), "multi-agent\n").unwrap();
        fs::write(attempt.join("contract-audit.md"), "VERDICT: PASS\n").unwrap();
        fs::write(attempt.join("events.tsv"), events).unwrap();
    }

    #[test]
    fn nonguided_done_does_not_write_independence_receipt() {
        let tmp = Tmp::new();
        let prog = tmp.root.join("prog");
        fs::create_dir_all(&prog).unwrap();
        fs::write(prog.join("PROGRAM"), "program: work\n").unwrap();
        fs::write(
            prog.join("PROBLEM.md"),
            "# Real\n\nA falsifiable outcome.\n",
        )
        .unwrap();
        fs::write(prog.join("CLAIMS.md"), "# C\n\n### C1 something\n").unwrap();
        let verdicts = prog.join("claims").join("C1").join("verdicts");
        fs::create_dir_all(&verdicts).unwrap();
        fs::write(verdicts.join("a1.md"), "CLAIM-VERDICT: FALSE\n").unwrap();
        fs::write(prog.join("PROPOSAL.md"), PROPOSAL).unwrap();
        approve_proposal(&prog);
        let junk = prog.join("attempts").join("Anotvalid");
        fs::create_dir_all(&junk).unwrap();
        fs::write(junk.join("meta.tsv"), "not-an-attempt\n").unwrap();

        let out = cycle(&prog, &[], &clock()).unwrap();
        assert!(out.starts_with("DONE — "), "{out}");
        assert!(!prog.join("INDEPENDENCE.md").exists());
    }

    #[test]
    fn clean_on_item_file_refuses_dead_attempt_without_stopping_it() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        item_file_guided_done(
            &prog,
            &repo,
            "state\tepoch\tpid\treason\nRUNNING\t1\t999999\tstart\n",
        );
        assert!(cycle(&prog, &[], &clock()).unwrap().starts_with("DONE — "));
        let err = cycle(&prog, &["clean", "--dry-run"], &clock())
            .unwrap_err()
            .to_string();
        assert_eq!(err, "attempt requires managed lifecycle behavior");
        let events =
            fs::read_to_string(prog.join("attempts").join("A1.2.3").join("events.tsv")).unwrap();
        assert!(!events.contains("STOPPED"), "{events}");
    }

    #[test]
    fn empty_abandon_uses_the_short_usage() {
        let tmp = Tmp::new();
        let prog = tmp.root.join("prog");
        fs::create_dir_all(&prog).unwrap();
        let err = cycle(&prog, &["problem", "--abandon"], &clock())
            .unwrap_err()
            .to_string();
        assert_eq!(err, "usage: crucible cycle problem --abandon REASON");
        let err = cycle(&prog, &["problem", "--abandon", ""], &clock())
            .unwrap_err()
            .to_string();
        assert_eq!(err, "usage: crucible cycle problem --abandon REASON");
        let err = cycle(&prog, &["problem", "--next", "--abandon"], &clock())
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "usage: crucible cycle problem FILE [--next] | --abandon REASON"
        );
    }

    #[test]
    fn dot_attempt_directory_is_ignored() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        item_file_guided_done(
            &prog,
            &repo,
            "state\tepoch\tpid\treason\nSTOPPED\t1\t-\tfixture\n",
        );
        fs::create_dir_all(prog.join("attempts").join(".hidden")).unwrap();
        let out = cycle(&prog, &["clean", "--dry-run"], &clock()).unwrap();
        assert!(out.contains("PRESERVE "), "{out}");
    }

    #[test]
    fn remove_empty_dirs_does_not_follow_symlink() {
        let tmp = Tmp::new();
        let outside = tmp.root.join("outside");
        fs::create_dir_all(outside.join("child")).unwrap();
        let worktrees = tmp.root.join("worktrees");
        fs::create_dir_all(&worktrees).unwrap();
        std::os::unix::fs::symlink(&outside, worktrees.join("link")).unwrap();
        remove_empty_dirs(&worktrees);
        assert!(outside.join("child").is_dir());
        assert!(worktrees
            .join("link")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn husk_report_skips_dot_directories() {
        let tmp = Tmp::new();
        let parent = tmp.root.join(".crucible");
        let prog = parent.join("work");
        fs::create_dir_all(&prog).unwrap();
        fs::create_dir_all(parent.join(".secret")).unwrap();
        fs::create_dir_all(parent.join("husk")).unwrap();
        assert_eq!(cycle_husk_programs(&prog), vec!["husk/".to_string()]);
    }

    #[test]
    fn locked_worktree_refusal_is_one_line() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
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
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "user.name", "test"]);
        fs::write(repo.join("tracked.txt"), "baseline\n").unwrap();
        git(&["add", "tracked.txt"]);
        git(&["commit", "-qm", "baseline"]);

        let prog = repo.join(".crucible").join("work");
        item_file_guided_done(
            &prog,
            &repo,
            "state\tepoch\tpid\treason\nSTOPPED\t1\t-\tfixture\n",
        );
        fs::create_dir_all(prog.join("worktrees")).unwrap();
        let wt = fs::canonicalize(prog.join("worktrees")).unwrap().join("wt");
        let wt_arg = wt.to_string_lossy().into_owned();
        git(&["worktree", "add", "-q", "-b", "wt-branch", &wt_arg]);
        // A broken gitfile makes `worktree remove` print one fatal line and no blocker.
        fs::write(wt.join(".git"), "gitdir: /no/such/gitdir\n").unwrap();

        let err = cycle(&prog, &["clean", "--apply"], &clock())
            .unwrap_err()
            .to_string();
        assert!(!err.contains('\n'), "{err:?}");
        assert!(err.contains("could not safely remove worktree:"), "{err}");
        assert!(err.contains("git refused:"), "{err}");
        assert!(err.contains("fatal:"), "{err}");
        assert!(wt.is_dir());
    }

    #[test]
    fn auditor_override_keeps_raw_text_and_refuses_overflow() {
        let tmp = Tmp::new();
        let prog = tmp.root.join("prog");
        fs::create_dir_all(&prog).unwrap();
        fs::write(prog.join("PROGRAM"), "program: work\n").unwrap();
        fs::write(prog.join("PROBLEM.md"), "# Real\n\nSomething is wrong.\n").unwrap();
        fs::write(prog.join("CLAIMS.md"), "### C1 gap\n    polarity: DEFECT\n").unwrap();
        with_override("CRUCIBLE_MIN_AUDITORS", "03", || {
            let out = cycle(&prog, &[], &clock()).unwrap();
            assert!(out.contains("admit needs 03 sealed TRUE"), "{out}");
            assert_eq!(crate::panel::guided_min_auditors(&prog).unwrap(), 3);
        });
        with_override("CRUCIBLE_MIN_AUDITORS", &"9".repeat(40), || {
            let err = cycle(&prog, &[], &clock()).unwrap_err().to_string();
            assert_eq!(err, "CRUCIBLE_MIN_AUDITORS must be a positive integer");
        });
        with_override("CRUCIBLE_MIN_KINDS", &"9".repeat(40), || {
            let err = crate::panel::min_kinds().unwrap_err().to_string();
            assert_eq!(err, "CRUCIBLE_MIN_KINDS must be a positive integer");
        });
    }

    fn with_override(key: &str, value: &str, body: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let old = std::env::var(key).ok();
        // SAFETY: this test holds ENV_LOCK and restores the previous value before unlock.
        // No other test reads CRUCIBLE_MIN_AUDITORS or CRUCIBLE_MIN_KINDS.
        unsafe { std::env::set_var(key, value) };
        struct Restore(String, Option<String>);
        impl Drop for Restore {
            fn drop(&mut self) {
                // SAFETY: same lock as the setter; restores the process environment.
                unsafe {
                    match self.1.take() {
                        Some(prev) => std::env::set_var(&self.0, prev),
                        None => std::env::remove_var(&self.0),
                    }
                }
            }
        }
        let _restore = Restore(key.to_string(), old);
        body();
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
