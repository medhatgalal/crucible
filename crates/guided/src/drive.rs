//! `crucible drive`: one sealed worker or one coordinator action per tick.
//!
//! The worker budget is `deadline - clock.now_unix()`. The wait is a real sleep,
//! not [`FixedClock`]. A regular file at `.drive.lock` is not a lock.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

use crucible_contract::Clock;

use crate::attempt::attempt;
use crate::audit::{
    contract_audit, contract_audit_copy_like, contract_structural_ok, contracts_isomorphic,
};
use crate::claims::{claim_attempt_is_sealed, claim_copy_verdict_like, suggest_contract_auditor};
use crate::cycle::{
    attempt_dirs_a, attempt_meta, attempt_pid, attempt_pid_alive, attempt_state, attempt_transport,
    claim_attempt_matches, claim_field, claim_verdict_word, count_claim_headings,
    drive_active_item, drive_inflight, drive_last_evidence, drive_state_name, file_name,
    program_field, read_dir_paths, self_path, write_cycle_status,
};
use crate::dispatch::{dispatch, invocation};
use crate::panel::{h12, hash_file, is_posint, kind_of, split_tabs};
use crate::phase::{judge_requested_fix, phase};
use crate::program::{uses_guided_cycle, uses_managed_lifecycle};
use crate::state::state_value;
use crate::{message, records, GuidedError};

struct Ctx<'a> {
    root: &'a Path,
    clock: &'a dyn Clock,
    out: String,
    worker_returned: bool,
    iso_copied: bool,
}

struct DriveLock {
    path: PathBuf,
    held: bool,
}

impl Drop for DriveLock {
    fn drop(&mut self) {
        if self.held {
            let _ = fs::remove_dir(&self.path);
            self.held = false;
        }
    }
}

/// `crucible drive`. Stdout on success, including STOP. Refusals are the shell `die` strings.
pub fn drive(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if !uses_guided_cycle(root)? {
        return Err(message("drive requires a guided cycle"));
    }
    if !uses_managed_lifecycle(root)? {
        return Err(message("drive requires managed lifecycle behavior"));
    }
    let mode = args.first().copied().unwrap_or("loop");
    match mode {
        "stop" => {
            if args.len() != 1 {
                return Err(message("usage: crucible drive stop"));
            }
            return drive_stop(root, clock);
        }
        "tick" | "loop" | "" => {
            if args.len() > 1 {
                return Err(message("usage: crucible drive [tick|loop|stop]"));
            }
        }
        _ => return Err(message("usage: crucible drive [tick|loop|stop]")),
    }
    let tick = mode == "tick";
    let max = drive_max()?;
    // Drop releases the directory. A regular file is left in place; it is not a lock.
    let _lock = acquire_lock(root)?;
    let mut ctx = Ctx {
        root,
        clock,
        out: String::new(),
        worker_returned: false,
        iso_copied: false,
    };
    // A later refusal must keep stdout already produced, including an earlier iteration.
    match drive_loop(&mut ctx, tick, max) {
        Ok(()) => Ok(ctx.out),
        Err(err) => Err(combine_out(&ctx.out, err)),
    }
}

fn drive_loop(ctx: &mut Ctx, tick: bool, max: u64) -> Result<(), GuidedError> {
    let root = ctx.root;
    let clock = ctx.clock;
    let mut n = 0u64;
    loop {
        n += 1;
        if n > max {
            return Err(message(format!("drive stopped: iteration cap {max}")));
        }
        ctx.worker_returned = false;
        if !uses_guided_cycle(root)? {
            return Err(message(
                "drive requires a guided cycle (PROGRAM cycle: guided was removed)",
            ));
        }
        if !uses_managed_lifecycle(root)? {
            return Err(message("drive requires managed lifecycle behavior"));
        }
        let repo = program_field(root, "repo").unwrap_or_default();
        if repo.is_empty() || !Path::new(&repo).is_dir() {
            return Err(message("drive requires PROGRAM repo:"));
        }
        let repo = PathBuf::from(repo);
        let line = captured(write_cycle_status(root, clock)?);
        let state = drive_state_name(&line);
        if is_human_gate(state) {
            print_human(ctx, state, &line);
            return Ok(());
        }
        if tick && already_returned(ctx)? {
            return Ok(());
        }
        fs::create_dir_all(root.join("items"))?;
        if state == "INVESTIGATE" && investigate_parent_tick(ctx, &repo)? {
            if flow(tick, finish_tick(ctx)?) {
                continue;
            }
            return Ok(());
        }
        apply_isomorphic_audits(ctx)?;
        if invoke_sealed_worker(ctx, &repo)? {
            if flow(tick, finish_tick(ctx)?) {
                continue;
            }
            return Ok(());
        }
        take_snapshot(root, &repo)?;
        let before_disp = count_dispatches(root);
        invoke_coordinator(ctx, &repo)?;
        check_discipline(root, &repo)?;
        if state == "WAIT" {
            refuse_second_live(root, &repo)?;
        }
        if count_dispatches(root) <= before_disp {
            perform_legal(ctx, state, &line)?;
        }
        apply_isomorphic_audits(ctx)?;
        if invoke_sealed_worker(ctx, &repo)? {
            if flow(tick, finish_tick(ctx)?) {
                continue;
            }
            return Ok(());
        }
        if flow(tick, finish_tick(ctx)?) {
            continue;
        }
        return Ok(());
    }
}

fn flow(tick: bool, keep_going: bool) -> bool {
    keep_going && !tick
}

fn combine_out(prior: &str, err: GuidedError) -> GuidedError {
    join_out(prior, &err.to_string())
}

fn join_out(prior: &str, msg: &str) -> GuidedError {
    if prior.is_empty() {
        return message(msg);
    }
    if prior.ends_with('\n') {
        message(format!("{prior}{msg}"))
    } else {
        message(format!("{prior}\n{msg}"))
    }
}

fn drive_stop(root: &Path, clock: &dyn Clock) -> Result<String, GuidedError> {
    let mut out = String::new();
    for path in attempt_dirs_a(root) {
        let id = file_name(&path);
        let st = attempt_state(root, &id)?;
        if st == "RUNNING" || st == "OVERDUE" {
            if attempt_pid_alive(root, &id)? {
                let pid = attempt_pid(root, &id)?;
                out.push_str(&format!(
                    "live-attempt: {id} {st} pid {pid} (not reclaimed)\n"
                ));
            } else {
                attempt(root, &["reclaim", &id], clock)?;
                out.push_str(&format!("reclaimed {id} (dead pid)\n"));
            }
        }
    }
    let lock = root.join(".drive.lock");
    // `[ -d ]` follows a symlink. A regular file is not a lock and is left alone.
    if lock.is_dir() {
        if fs::remove_dir(&lock).is_err() {
            return Err(message(format!(
                "refused: could not remove {}/.drive.lock",
                root.display()
            )));
        }
        out.push_str(&format!("released {}/.drive.lock\n", root.display()));
    } else {
        out.push_str("no .drive.lock\n");
    }
    Ok(out)
}

fn acquire_lock(root: &Path) -> Result<DriveLock, GuidedError> {
    let path = root.join(".drive.lock");
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if meta.file_type().is_file() {
            return Ok(DriveLock { path, held: false });
        }
    }
    match fs::create_dir(&path) {
        Ok(()) => Ok(DriveLock { path, held: true }),
        Err(_) => Err(message("drive already running")),
    }
}

fn captured(mut text: String) -> String {
    while text.ends_with('\n') || text.ends_with('\r') {
        text.pop();
    }
    text
}

fn is_human_gate(state: &str) -> bool {
    matches!(state, "WAIT_PANEL" | "WAIT_APPROVAL" | "ESCALATE" | "DONE")
}

fn print_human(ctx: &mut Ctx, state: &str, line: &str) {
    let self_cmd = self_path(ctx.root);
    match state {
        "WAIT_PANEL" => ctx.out.push_str(&format!(
            "HUMAN — approve the agent panel: {self_cmd} cycle approve-panel\n"
        )),
        "WAIT_APPROVAL" => ctx.out.push_str(&format!(
            "HUMAN — approve the current proposal: {self_cmd} cycle approve\n"
        )),
        "ESCALATE" => ctx.out.push_str(&format!("HUMAN — {line}\n")),
        "DONE" => {
            ctx.out.push_str(&format!(
                "HUMAN — cycle is DONE. Next: {self_cmd} cycle clean --dry-run\n"
            ));
            ctx.out.push_str(&format!(
                "HUMAN — or bind the next problem: {self_cmd} cycle problem FILE --next\n"
            ));
            ctx.out
                .push_str("HUMAN — drive never runs cycle clean --apply and never --nexts\n");
        }
        _ => {}
    }
}

fn already_returned(ctx: &mut Ctx) -> Result<bool, GuidedError> {
    let inf = inflight(ctx.root)?;
    if inf.is_empty() || inf == "-" {
        return Ok(false);
    }
    let ast = attempt_state(ctx.root, &inf).unwrap_or_default();
    if ast != "RETURNED" || next_sealed_id(ctx.root)?.is_some() {
        return Ok(false);
    }
    let result = ctx.root.join("attempts").join(&inf).join("result.md");
    if !result.is_file() {
        return Ok(false);
    }
    let text = fs::read_to_string(result)?;
    let next = records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix("NEXT: "))
        .unwrap_or("");
    if next == "FIX" {
        return Ok(false);
    }
    ctx.out
        .push_str(&format!("drive: attempt {inf} already RETURNED\n"));
    Ok(true)
}

fn finish_tick(ctx: &mut Ctx) -> Result<bool, GuidedError> {
    let line = captured(write_cycle_status(ctx.root, ctx.clock)?);
    let state = drive_state_name(&line);
    let token = progress_token(ctx.root, &line)?;
    let prev = drive_state_field(ctx.root, "token");
    let stalls_s = drive_state_field(ctx.root, "stalls");
    let mut stalls = if stalls_s.is_empty() {
        0
    } else {
        stalls_s.parse::<i64>().unwrap_or(0)
    };
    if !prev.is_empty() && prev == token {
        stalls += 1;
    } else {
        stalls = 0;
    }
    write_drive_state(ctx.root, &token, stalls)?;
    if ctx.worker_returned {
        write_drive_state(ctx.root, &token, 0)?;
        if is_human_gate(state) {
            print_human(ctx, state, &line);
        }
        return Ok(true);
    }
    if stalls >= 1 {
        ctx.out
            .push_str("STOP — no progress (same status and no new evidence twice)\n");
        return Ok(false);
    }
    if is_human_gate(state) {
        print_human(ctx, state, &line);
        return Ok(false);
    }
    Ok(true)
}

fn write_drive_state(root: &Path, token: &str, stalls: i64) -> Result<(), GuidedError> {
    fs::write(
        root.join("DRIVE.state"),
        format!("token: {token}\nstalls: {stalls}\n"),
    )?;
    Ok(())
}

fn drive_state_field(root: &Path, key: &str) -> String {
    let Ok(text) = fs::read_to_string(root.join("DRIVE.state")) else {
        return String::new();
    };
    let prefix = format!("{key}: ");
    records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string()
}

fn progress_token(root: &Path, line: &str) -> Result<String, GuidedError> {
    let nd = count_dispatches(root);
    let na = count_attempt_dirs(root);
    let ev = drive_last_evidence(root)?;
    let seals = if root.join("attempts").is_dir() {
        dash_hash(&seal_body(root)?)
    } else {
        "-".to_string()
    };
    let claims = optional_hash(&root.join("CLAIMS.md"))?;
    let proposal = optional_hash(&root.join("PROPOSAL.md"))?;
    let mut live_body = String::new();
    for id in live_attempt_ids(root)? {
        live_body.push_str(&id);
        live_body.push('\n');
    }
    let live = dash_hash(&live_body);
    Ok(format!(
        "{line}|{nd}|{na}|{ev}|{seals}|{claims}|{proposal}|{live}"
    ))
}

fn dash_hash(body: &str) -> String {
    let hashed = h12(body.as_bytes());
    if hashed.is_empty() {
        "-".to_string()
    } else {
        hashed
    }
}

fn optional_hash(path: &Path) -> Result<String, GuidedError> {
    if path.is_file() {
        hash_file(path)
    } else {
        Ok("-".to_string())
    }
}

fn seal_body(root: &Path) -> Result<String, GuidedError> {
    let mut files = Vec::new();
    collect_matching(&root.join("attempts"), &mut files, &|path| {
        matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("transport") | Some("contract-audit.md") | Some("events.tsv")
        )
    });
    files.sort();
    let mut body = String::new();
    for path in files {
        body.push_str(&format!("{} {}\n", path.display(), hash_file(&path)?));
    }
    Ok(body)
}

fn investigate_parent_tick(ctx: &mut Ctx, repo: &Path) -> Result<bool, GuidedError> {
    ctx.iso_copied = false;
    let dispatched = investigate_dispatch(ctx)?;
    parent_seal(ctx, dispatched)?;
    apply_isomorphic_audits(ctx)?;
    apply_isomorphic_verdicts(ctx)?;
    let mut started = false;
    if let Some(nid) = next_sealed_id(ctx.root)? {
        let role = attempt_meta(ctx.root, &nid, 5)?;
        if matches!(role.as_str(), "claim-auditor" | "scout") && invoke_sealed_worker(ctx, repo)? {
            started = true;
            apply_isomorphic_audits(ctx)?;
            apply_isomorphic_verdicts(ctx)?;
        }
    }
    Ok(dispatched || started || ctx.iso_copied)
}

fn investigate_dispatch(ctx: &mut Ctx) -> Result<bool, GuidedError> {
    let path = ctx.root.join("CLAIMS.md");
    if !path.is_file() {
        return Ok(false);
    }
    let text = fs::read_to_string(&path)?;
    let total = count_claim_headings(&text);
    if total == 0 {
        return Ok(false);
    }
    let auditors = cast_agents(ctx.root, "claim-auditor")?;
    let Some(agent) = auditors.first().cloned() else {
        return Ok(false);
    };
    let mut dispatched = false;
    for i in 1..=total {
        let cn = format!("C{i}");
        if !has_claim_dispatch(ctx.root, &cn, "claim-auditor", &agent) {
            // Claim dispatch `die` exits. `|| true` does not catch it, so do not seal after.
            record_ok(
                ctx,
                dispatch(ctx.root, &[&cn, "claim-auditor", &agent], ctx.clock),
            )?;
            dispatched = true;
        }
    }
    for i in 1..=total {
        let cn = format!("C{i}");
        let truth = any_true(&ctx.root.join("claims").join(&cn).join("verdicts"));
        let scout = claim_field(&text, i, "scout").unwrap_or_default();
        if truth && scout.is_empty() {
            let scouts = cast_agents(ctx.root, "scout")?;
            let scout_agent = scouts.first().cloned().unwrap_or_else(|| agent.clone());
            if !scout_agent.is_empty() && !has_claim_dispatch(ctx.root, &cn, "scout", &scout_agent)
            {
                record_ok(
                    ctx,
                    dispatch(ctx.root, &[&cn, "scout", &scout_agent], ctx.clock),
                )?;
                dispatched = true;
            }
        }
    }
    Ok(dispatched)
}

fn has_claim_dispatch(root: &Path, cn: &str, role: &str, agent: &str) -> bool {
    let dir = root.join("claims").join(cn).join("dispatches");
    let suffix = format!("-{role}-{agent}.md");
    read_dir_paths(&dir)
        .into_iter()
        .any(|path| path.is_file() && file_name(&path).ends_with(&suffix))
}

fn any_true(dir: &Path) -> bool {
    verdict_md(dir)
        .into_iter()
        .any(|path| claim_verdict_word(&path) == Some("TRUE"))
}

fn parent_seal(ctx: &mut Ctx, auto_pass: bool) -> Result<(), GuidedError> {
    if !ctx.root.join("attempts").is_dir() {
        return Ok(());
    }
    for path in attempt_dirs_a(ctx.root) {
        let id = file_name(&path);
        if attempt_state(ctx.root, &id)? != "DISPATCHED" {
            continue;
        }
        let role = attempt_meta(ctx.root, &id, 5)?;
        if !matches!(role.as_str(), "claim-auditor" | "scout") {
            continue;
        }
        let agent = attempt_meta(ctx.root, &id, 6)?;
        if attempt_transport(ctx.root, &id)?.is_none() {
            let hop = claim_hop(ctx.root, &agent)?;
            if attempt(ctx.root, &["transport", &id, &hop], ctx.clock).is_err() {
                let _ = attempt(ctx.root, &["transport", &id, "multi-agent"], ctx.clock);
            }
        }
    }
    if !auto_pass {
        return Ok(());
    }
    let auditor = suggest_contract_auditor(ctx.root)?;
    if auditor.is_empty() || auditor == "<auditor-name>" {
        return Ok(());
    }
    for path in attempt_dirs_a(ctx.root) {
        let id = file_name(&path);
        if attempt_state(ctx.root, &id)? != "DISPATCHED" {
            continue;
        }
        let role = attempt_meta(ctx.root, &id, 5)?;
        if !matches!(role.as_str(), "claim-auditor" | "scout") {
            continue;
        }
        if claim_attempt_is_sealed(ctx.root, &id) {
            continue;
        }
        let agent = attempt_meta(ctx.root, &id, 6)?;
        if auditor == agent {
            continue;
        }
        if !contract_structural_ok(&path, &role)? {
            continue;
        }
        if attempt_transport(ctx.root, &id)?.is_none() {
            continue;
        }
        let note = "engine-template INVESTIGATE fallback; parent sealed without coordinator ACP";
        if contract_audit(ctx.root, &[&id, &auditor, "PASS", note], ctx.clock).is_ok() {
            break;
        }
    }
    Ok(())
}

fn claim_hop(root: &Path, agent: &str) -> Result<String, GuidedError> {
    let nk = registered_kind_count(root)?;
    let ak = kind_of(root, agent)?;
    let ck = match coordinator_agent(root)? {
        Some(coord) => kind_of(root, &coord)?,
        None => String::new(),
    };
    if nk >= 2 && !ak.is_empty() && !ck.is_empty() && ak != ck {
        Ok("multi-agent".to_string())
    } else {
        Ok("acp".to_string())
    }
}

fn registered_kind_count(root: &Path) -> Result<usize, GuidedError> {
    let path = root.join("agents.tsv");
    if !path.is_file() {
        return Ok(0);
    }
    let text = fs::read_to_string(path)?;
    let mut kinds = std::collections::BTreeSet::new();
    for line in records(&text) {
        if line.starts_with('#') || line.bytes().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let fields = split_tabs(line);
        if let Some(kind) = fields.get(1).copied() {
            if !kind.is_empty() {
                kinds.insert(kind.to_string());
            }
        }
    }
    Ok(kinds.len())
}

fn coordinator_agent(root: &Path) -> Result<Option<String>, GuidedError> {
    cast_agents(root, "coordinator").map(|agents| agents.into_iter().next())
}

fn cast_agents(root: &Path, role: &str) -> Result<Vec<String>, GuidedError> {
    let path = root.join("PANEL.ASSIGN.tsv");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    let mut out = Vec::new();
    for line in records(&text) {
        let fields = split_tabs(line);
        if fields.first().copied() == Some(role) {
            let who = fields.get(1).copied().unwrap_or("");
            if !who.is_empty() {
                out.push(who.to_string());
            }
        }
    }
    Ok(out)
}

fn apply_isomorphic_audits(ctx: &mut Ctx) -> Result<(), GuidedError> {
    if !ctx.root.join("attempts").is_dir() {
        return Ok(());
    }
    let attempts = attempt_dirs_a(ctx.root);
    for tad in &attempts {
        let audit = tad.join("contract-audit.md");
        if !audit.is_file() {
            continue;
        }
        let text = fs::read_to_string(&audit)?;
        if !records(&text)
            .iter()
            .any(|line| line.starts_with("VERDICT: PASS"))
        {
            continue;
        }
        let tid = file_name(tad);
        for dad in &attempts {
            let did = file_name(dad);
            if did == tid || dad.join("contract-audit.md").exists() {
                continue;
            }
            if contract_audit_copy_like(ctx.root, &tid, &did)?.is_some() {
                ctx.iso_copied = true;
            }
        }
    }
    Ok(())
}

fn apply_isomorphic_verdicts(ctx: &mut Ctx) -> Result<(), GuidedError> {
    if !ctx.root.join("claims").is_dir() {
        return Ok(());
    }
    let mut verdicts = Vec::new();
    for dir in claim_id_dirs(ctx.root) {
        verdicts.extend(verdict_md(&dir.join("verdicts")));
    }
    verdicts.sort();
    for vd in verdicts {
        let Some(word) = claim_verdict_word(&vd) else {
            continue;
        };
        if !matches!(word, "STALE" | "FALSE" | "UNVERIFIABLE") {
            continue;
        }
        let Some(who) = header_value(&vd, "AGENT") else {
            continue;
        };
        let Some(from) = vd
            .parent()
            .and_then(|parent| parent.parent())
            .map(file_name)
        else {
            continue;
        };
        if who.is_empty() || from.is_empty() {
            continue;
        }
        let Some(from_id) = claim_attempt_id(ctx.root, &from, "claim-auditor", &who)? else {
            continue;
        };
        for cdir in claim_id_dirs(ctx.root) {
            let to = file_name(&cdir);
            if to == from || cdir.join("verdicts").join(format!("{who}.md")).is_file() {
                continue;
            }
            let Some(to_id) = claim_attempt_id(ctx.root, &to, "claim-auditor", &who)? else {
                continue;
            };
            let from_contract = ctx.root.join("attempts").join(&from_id).join("contract.md");
            let to_contract = ctx.root.join("attempts").join(&to_id).join("contract.md");
            if !from_contract.is_file() || !to_contract.is_file() {
                continue;
            }
            if !contracts_isomorphic(&from_contract, &to_contract)? {
                continue;
            }
            if claim_copy_verdict_like(ctx.root, &from, &to, &who)?.is_some() {
                ctx.iso_copied = true;
            }
        }
    }
    Ok(())
}

fn header_value(path: &Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let prefix = format!("{key}: ");
    records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(str::to_string)
}

fn claim_attempt_id(
    root: &Path,
    cn: &str,
    role: &str,
    agent: &str,
) -> Result<Option<String>, GuidedError> {
    if !root.join("attempts").is_dir() {
        return Ok(None);
    }
    for path in attempt_dirs_a(root) {
        let id = file_name(&path);
        if claim_attempt_matches(root, &id, cn, agent, Some(role))? {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn claim_id_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = read_dir_paths(&root.join("claims"));
    dirs.retain(|path| path.is_dir() && file_name(path).starts_with('C'));
    dirs.sort();
    dirs
}

fn verdict_md(dir: &Path) -> Vec<PathBuf> {
    let mut files = read_dir_paths(dir);
    files.retain(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"));
    files.sort();
    files
}

fn next_sealed_id(root: &Path) -> Result<Option<String>, GuidedError> {
    if !root.join("attempts").is_dir() {
        return Ok(None);
    }
    let inf = inflight(root)?;
    if !inf.is_empty() && inf != "-" && attempt_is_runnable(root, &inf)? {
        return Ok(Some(inf));
    }
    let item = active_item(root)?;
    if !item.is_empty() && item != "-" {
        for path in attempt_dirs_a(root) {
            let id = file_name(&path);
            if attempt_meta(root, &id, 2)? != item {
                continue;
            }
            if attempt_is_runnable(root, &id)? {
                return Ok(Some(id));
            }
        }
    }
    for path in attempt_dirs_a(root) {
        let id = file_name(&path);
        if attempt_is_runnable(root, &id)? {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn attempt_is_runnable(root: &Path, id: &str) -> Result<bool, GuidedError> {
    if attempt_state(root, id)? != "DISPATCHED" || !claim_attempt_is_sealed(root, id) {
        return Ok(false);
    }
    let cn = attempt_meta(root, id, 2)?;
    let agent = attempt_meta(root, id, 6)?;
    Ok(!claim_has_agent_verdict(root, &cn, &agent))
}

fn claim_has_agent_verdict(root: &Path, cn: &str, agent: &str) -> bool {
    !cn.is_empty()
        && !agent.is_empty()
        && root
            .join("claims")
            .join(cn)
            .join("verdicts")
            .join(format!("{agent}.md"))
            .is_file()
}

fn active_item(root: &Path) -> Result<String, GuidedError> {
    let item = drive_active_item(root)?;
    if item.is_empty() {
        Ok("-".to_string())
    } else {
        Ok(item)
    }
}

fn inflight(root: &Path) -> Result<String, GuidedError> {
    let item = active_item(root)?;
    let value = drive_inflight(root, &item)?;
    if value.is_empty() {
        Ok("-".to_string())
    } else {
        Ok(value)
    }
}

fn invoke_sealed_worker(ctx: &mut Ctx, repo: &Path) -> Result<bool, GuidedError> {
    let Some(id) = next_sealed_id(ctx.root)? else {
        return Ok(false);
    };
    let ad = ctx.root.join("attempts").join(&id);
    let agent = attempt_meta(ctx.root, &id, 6)?;
    let brief = ad.join("contract.md");
    if !brief.is_file() {
        return Ok(false);
    }
    let cmd = invocation(ctx.root, &agent, &brief.display().to_string())?;
    if cmd.is_empty() || cmd.starts_with("(no command registered") {
        ctx.out.push_str(&format!(
            "HUMAN — sealed attempt {id} has no agents.tsv command for {agent}\n"
        ));
        return Ok(false);
    }
    let deadline = attempt_meta(ctx.root, &id, 12)?;
    // Budget uses the injected clock. The sleep below is wall time, not FixedClock.
    let timeout = worker_budget(&deadline, ctx.clock.now_unix(), worker_timeout());
    let log = File::create(ad.join("invoke.log"))?;
    let log_err = log.try_clone()?;
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .current_dir(repo)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()?;
    let pid = child.id();
    let pid_s = pid.to_string();
    if attempt(ctx.root, &["start", &id, &pid_s], ctx.clock).is_err() {
        sigterm(pid);
        let _ = child.wait();
        return Ok(false);
    }
    let mut elapsed = 0u64;
    loop {
        match child.try_wait()? {
            Some(status) => {
                let rc = exit_code(status);
                if rc == 0 {
                    attempt(
                        ctx.root,
                        &["finish", &id, "RETURNED", "drive worker exit 0"],
                        ctx.clock,
                    )?;
                    ctx.worker_returned = true;
                } else {
                    attempt(
                        ctx.root,
                        &["finish", &id, "STOPPED", &format!("drive worker exit {rc}")],
                        ctx.clock,
                    )?;
                }
                ctx.out.push_str(&format!(
                    "drive: started {id} pid {pid} ({agent}); finish recorded\n"
                ));
                return Ok(true);
            }
            None if elapsed >= timeout => {
                sigterm(pid);
                let _ = child.wait();
                attempt(
                    ctx.root,
                    &[
                        "finish",
                        &id,
                        "TIMEOUT",
                        &format!("drive worker exceeded {timeout}s"),
                    ],
                    ctx.clock,
                )?;
                ctx.out
                    .push_str(&format!("drive: {id} TIMEOUT after {timeout}s\n"));
                return Ok(true);
            }
            None => {
                thread::sleep(Duration::from_secs(1));
                elapsed += 1;
            }
        }
    }
}

fn worker_budget(deadline: &str, now: i64, cap: u64) -> u64 {
    let Some(dl) = parse_posint(deadline) else {
        return cap;
    };
    if dl > i64::MAX as u64 {
        return cap;
    }
    let Some(rem) = (dl as i64).checked_sub(now) else {
        return cap;
    };
    if rem > 0 && (rem as u64) < cap {
        rem as u64
    } else {
        cap
    }
}

fn worker_timeout() -> u64 {
    match env_value("CRUCIBLE_DRIVE_WORKER_TIMEOUT") {
        Some(value) => parse_posint(&value).unwrap_or(900),
        None => 900,
    }
}

fn drive_max() -> Result<u64, GuidedError> {
    match env_value("CRUCIBLE_DRIVE_MAX") {
        Some(value) => parse_posint(&value)
            .ok_or_else(|| message("CRUCIBLE_DRIVE_MAX must be a positive integer")),
        None => Ok(32),
    }
}

fn env_value(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

fn parse_posint(value: &str) -> Option<u64> {
    if !is_posint(value) {
        None
    } else {
        value.parse().ok()
    }
}

fn sigterm(pid: u32) {
    let _ = Command::new("kill")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    1
}

fn invoke_coordinator(ctx: &mut Ctx, repo: &Path) -> Result<(), GuidedError> {
    if next_sealed_id(ctx.root)?.is_some() {
        return Ok(());
    }
    let Some(agent) = coordinator_agent(ctx.root)? else {
        return Ok(());
    };
    write_brief(ctx)?;
    let brief = ctx.root.join("DRIVE.BRIEF.md");
    let cmd = invocation(ctx.root, &agent, &brief.display().to_string())?;
    if cmd.is_empty() || cmd.starts_with("(no command registered") {
        return Ok(());
    }
    match run_sh(repo, &cmd) {
        Ok(text) => {
            ctx.out.push_str(&text);
            Ok(())
        }
        Err(text) => Err(join_out(
            &text,
            &format!("refused: coordinator invoke failed — {cmd}"),
        )),
    }
}

fn run_sh(repo: &Path, cmd: &str) -> Result<String, String> {
    let out = match Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(repo)
        .output()
    {
        Ok(out) => out,
        Err(err) => return Err(err.to_string()),
    };
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(text)
    } else {
        Err(text)
    }
}

fn record_ok(ctx: &mut Ctx, result: Result<String, GuidedError>) -> Result<(), GuidedError> {
    ctx.out.push_str(&result?);
    Ok(())
}

fn keep_ok(ctx: &mut Ctx, result: Result<String, GuidedError>) {
    if let Ok(text) = result {
        ctx.out.push_str(&text);
    }
}

fn write_brief(ctx: &Ctx) -> Result<(), GuidedError> {
    let self_cmd = self_path(ctx.root);
    let body = format!(
        "\
# DRIVE BRIEF

If drive is running, only do the next cycle action. Conversational \"keep looping\" is not a waiver to implement.

1. Read START.md and STATUS.md.
2. Run: {self_cmd} cycle
3. Perform only the single next legal orchestrator action for the current state.
4. Dispatch and seal (transport + contract-audit). Do not start ACP workers — drive starts sealed attempts.
5. Write nothing a maker, reviewer, or auditor should write.
6. Do not edit owned product paths. Do not write verdicts. Do not merge.
7. Exit.

Legal actions: see STATUS.md and docs/drive.md. INVESTIGATE fallback dispatches one missing claim contract. Do not implement, commit, merge, write verdicts, or flip cycle: guided.
"
    );
    fs::write(ctx.root.join("DRIVE.BRIEF.md"), body)?;
    Ok(())
}

fn perform_legal(ctx: &mut Ctx, state: &str, line: &str) -> Result<(), GuidedError> {
    match state {
        "INVESTIGATE" => {
            let _ = investigate_dispatch(ctx)?;
        }
        "WAIT" => {}
        "EXECUTE" => {
            let slug = legal_slug(ctx.root, line)?;
            let inf = inflight(ctx.root)?;
            if !slug.is_empty() && slug != "-" && (inf.is_empty() || inf == "-") {
                if let Some(maker) = cast_agents(ctx.root, "maker")?.into_iter().next() {
                    if state_col(ctx.root, &slug, 3) == "BUILD" {
                        // Managed dispatch is a subshell, so its `die` is swallowed.
                        keep_ok(
                            ctx,
                            dispatch(ctx.root, &[&slug, "maker", &maker], ctx.clock),
                        );
                    }
                }
            }
        }
        "REVIEW" => {
            let slug = legal_slug(ctx.root, line)?;
            if slug.is_empty() || slug == "-" {
                return Ok(());
            }
            let inf = inflight(ctx.root)?;
            let (ast, role) = if !inf.is_empty() && inf != "-" {
                (
                    attempt_state(ctx.root, &inf).unwrap_or_default(),
                    attempt_meta(ctx.root, &inf, 5).unwrap_or_default(),
                )
            } else {
                (String::new(), String::new())
            };
            if judge_requested_fix(ctx.root, &slug) && state_col(ctx.root, &slug, 3) == "REVIEW" {
                let idle = inf.is_empty() || inf == "-";
                if idle || (ast == "RETURNED" && role == "judge") {
                    // `cmd_phase` is not a subshell, so `|| true` does not catch `die`.
                    record_ok(ctx, phase(ctx.root, &[&slug, "BUILD"], ctx.clock))?;
                    if let Some(maker) = cast_agents(ctx.root, "maker")?.into_iter().next() {
                        keep_ok(
                            ctx,
                            dispatch(ctx.root, &[&slug, "maker", &maker], ctx.clock),
                        );
                    }
                }
            } else if ast == "RETURNED" && role == "maker" {
                let mut reviewers = cast_agents(ctx.root, "reviewer")?;
                if reviewers.is_empty() {
                    reviewers = cast_agents(ctx.root, "judge")?;
                }
                if let Some(who) = reviewers.into_iter().next() {
                    if state_col(ctx.root, &slug, 3) == "REVIEW" {
                        keep_ok(ctx, dispatch(ctx.root, &[&slug, "judge", &who], ctx.clock));
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn legal_slug(root: &Path, line: &str) -> Result<String, GuidedError> {
    let slug = line.split_whitespace().nth(2).unwrap_or("");
    if !slug.is_empty() && slug != "—" {
        return Ok(slug.to_string());
    }
    active_item(root)
}

fn state_col(root: &Path, slug: &str, column: usize) -> String {
    state_value(root, slug, column)
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn check_discipline(root: &Path, repo: &Path) -> Result<(), GuidedError> {
    if repo.as_os_str().is_empty() || !repo.is_dir() {
        return Ok(());
    }
    let snap = snap_dir(root);
    if !uses_guided_cycle(root)? {
        restore_all(root, repo)?;
        return Err(message("refused: coordinator removed cycle: guided"));
    }
    let head = snap.join("head");
    if head.is_file() {
        let before = first_line(&fs::read(&head)?);
        let now = first_line(&git_stdout(repo, &["rev-parse", "HEAD"]));
        if !before.is_empty() && now != before {
            restore_all(root, repo)?;
            return Err(message(
                "refused: coordinator committed or merged product history",
            ));
        }
    }
    if repo.join(".git").join("MERGE_HEAD").is_file() {
        restore_all(root, repo)?;
        return Err(message(
            "refused: coordinator merged — drive does not allow merge",
        ));
    }
    let after = git_stdout(repo, &["status", "--porcelain", "-uall"]);
    fs::write(snap.join("porcelain_after"), &after)?;
    let before_text = fs::read_to_string(snap.join("porcelain")).unwrap_or_default();
    let before_lines = records(&before_text);
    let hash_text = fs::read_to_string(snap.join("product_hashes")).unwrap_or_default();
    let hash_lines: Vec<String> = records(&hash_text)
        .into_iter()
        .map(str::to_string)
        .collect();
    let after_text = String::from_utf8_lossy(&after).into_owned();
    let mut hits = Vec::new();
    for line in records(&after_text) {
        if line.is_empty() {
            continue;
        }
        let path = porcelain_path(line);
        if path_is_program(&path) {
            continue;
        }
        if before_lines.contains(&line) {
            let full = repo.join(&path);
            if full.is_file() && snap.join("product_hashes").is_file() {
                if let Some(old) = hash_field(&hash_lines, &path) {
                    if !old.is_empty() && old != hash_file(&full)? {
                        hits.push(path);
                    }
                }
            }
            continue;
        }
        hits.push(path);
    }
    if let Some(hit) = hits.first() {
        fs::write(snap.join("product_hits"), format!("{}\n", hits.join("\n")))?;
        let hit = hit.clone();
        restore_all(root, repo)?;
        return Err(message(format!(
            "refused: coordinator edited owned product path: {hit}"
        )));
    }
    fs::write(snap.join("product_hits"), "")?;
    let verdicts_after = hash_list(root, &verdict_files(root))?;
    fs::write(snap.join("verdicts_after"), &verdicts_after)?;
    let verdicts_before = fs::read_to_string(snap.join("verdicts_hashes")).unwrap_or_default();
    if verdicts_before != verdicts_after {
        restore_all(root, repo)?;
        return Err(message("refused: coordinator wrote a verdict file"));
    }
    let wt_after = hash_list(root, &worktree_files(root))?;
    fs::write(snap.join("worktree_after"), &wt_after)?;
    let wt_before = fs::read_to_string(snap.join("worktree_hashes")).unwrap_or_default();
    if wt_before != wt_after {
        restore_all(root, repo)?;
        return Err(message("refused: coordinator wrote a task worktree"));
    }
    Ok(())
}

fn refuse_second_live(root: &Path, repo: &Path) -> Result<(), GuidedError> {
    let after = live_attempt_ids(root)?;
    let mut body = String::new();
    for id in &after {
        body.push_str(id);
        body.push('\n');
    }
    fs::write(snap_dir(root).join("live_ids_after"), body)?;
    let before_text = fs::read_to_string(snap_dir(root).join("live_ids")).unwrap_or_default();
    let before = records(&before_text);
    if let Some(extra) = after.iter().find(|id| !before.iter().any(|row| row == id)) {
        restore_all(root, repo)?;
        return Err(message(format!(
            "refused: WAIT inflight — do not start a second attempt ({extra})"
        )));
    }
    Ok(())
}

fn take_snapshot(root: &Path, repo: &Path) -> Result<(), GuidedError> {
    let snap = snap_dir(root);
    remove_path_all(&snap);
    fs::create_dir_all(snap.join("verdicts"))?;
    fs::create_dir_all(snap.join("wt"))?;
    fs::write(snap.join("head"), git_stdout(repo, &["rev-parse", "HEAD"]))?;
    let merge = if repo.join(".git").join("MERGE_HEAD").is_file() {
        "yes\n"
    } else {
        "no\n"
    };
    fs::write(snap.join("merge_head"), merge)?;
    let porcelain = git_stdout(repo, &["status", "--porcelain", "-uall"]);
    fs::write(snap.join("porcelain"), &porcelain)?;
    let porcelain_text = String::from_utf8_lossy(&porcelain).into_owned();
    fs::write(
        snap.join("product_hashes"),
        product_hash_body(repo, &porcelain_text)?,
    )?;
    let verdicts = verdict_files(root);
    fs::write(snap.join("verdicts_hashes"), hash_list(root, &verdicts)?)?;
    let worktrees = worktree_files(root);
    fs::write(snap.join("worktree_hashes"), hash_list(root, &worktrees)?)?;
    let mut live = String::new();
    for id in live_attempt_ids(root)? {
        live.push_str(&id);
        live.push('\n');
    }
    fs::write(snap.join("live_ids"), live)?;
    let program = root.join("PROGRAM");
    if program.is_file() {
        fs::copy(&program, snap.join("PROGRAM"))?;
    }
    copy_rel(root, &verdicts, &snap.join("verdicts"))?;
    if root.join("worktrees").is_dir() {
        copy_rel(root, &worktrees, &snap.join("wt"))?;
    }
    Ok(())
}

fn restore_all(root: &Path, repo: &Path) -> Result<(), GuidedError> {
    if !repo.join(".git").is_dir() {
        return Ok(());
    }
    let snap = snap_dir(root);
    let head = snap.join("head");
    if head.is_file() {
        let before = first_line(&fs::read(&head).unwrap_or_default());
        let now = first_line(&git_stdout(repo, &["rev-parse", "HEAD"]));
        if !before.is_empty() && now != before {
            git_quiet(repo, &["reset", "--hard", &before]);
        }
    }
    let saved = snap.join("PROGRAM");
    if saved.is_file() {
        fs::copy(saved, root.join("PROGRAM"))?;
    }
    let before_text = fs::read_to_string(snap.join("porcelain")).unwrap_or_default();
    let before_lines = records(&before_text);
    let hash_text = fs::read_to_string(snap.join("product_hashes")).unwrap_or_default();
    let hash_lines: Vec<String> = records(&hash_text)
        .into_iter()
        .map(str::to_string)
        .collect();
    let after = String::from_utf8_lossy(&git_stdout(repo, &["status", "--porcelain", "-uall"]))
        .into_owned();
    for line in records(&after) {
        if line.is_empty() {
            continue;
        }
        let path = porcelain_path(line);
        if path.is_empty() || path_is_program(&path) {
            continue;
        }
        if before_lines.contains(&line) {
            let full = repo.join(&path);
            if full.is_file() && snap.join("product_hashes").is_file() {
                if let Some(old) = hash_field(&hash_lines, &path) {
                    if !old.is_empty() {
                        let nowh = hash_file(&full).unwrap_or_default();
                        if old != nowh {
                            git_quiet(repo, &["checkout", "--", &path]);
                        }
                    }
                }
            }
            continue;
        }
        if line.starts_with("?? ") {
            remove_path_all(&repo.join(&path));
        } else {
            git_quiet(repo, &["checkout", "--", &path]);
        }
    }
    if root.join("items").is_dir() || root.join("claims").is_dir() {
        let snap_verdicts = snap.join("verdicts");
        for path in verdict_files(root) {
            let rel = path.strip_prefix(root).unwrap_or(path.as_path());
            if !snap_verdicts.join(rel).is_file() {
                let _ = fs::remove_file(&path);
            }
        }
        if snap_verdicts.is_dir() {
            restore_copied(root, &snap_verdicts)?;
        }
    }
    if root.join("worktrees").is_dir() {
        let snap_wt = snap.join("wt");
        for path in worktree_files(root) {
            let rel = path.strip_prefix(root).unwrap_or(path.as_path());
            if !snap_wt.join(rel).is_file() {
                let _ = fs::remove_file(&path);
            }
        }
    }
    let snap_wt = snap.join("wt");
    if snap_wt.is_dir() {
        restore_copied(root, &snap_wt)?;
    }
    Ok(())
}

fn snap_dir(root: &Path) -> PathBuf {
    root.join(".drive-snap")
}

fn live_attempt_ids(root: &Path) -> Result<Vec<String>, GuidedError> {
    let mut ids = Vec::new();
    for path in attempt_dirs_a(root) {
        let id = file_name(&path);
        let st = attempt_state(root, &id)?;
        if matches!(st.as_str(), "DISPATCHED" | "RUNNING" | "OVERDUE") {
            ids.push(id);
        }
    }
    ids.sort();
    Ok(ids)
}

fn count_dispatches(root: &Path) -> usize {
    let mut files = Vec::new();
    collect_matching(&root.join("claims"), &mut files, &|path| {
        path.to_string_lossy().contains("/dispatches/")
    });
    files.len()
}

fn count_attempt_dirs(root: &Path) -> usize {
    read_dir_paths(&root.join("attempts"))
        .into_iter()
        .filter(|path| path.is_dir())
        .count()
}

fn verdict_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for name in ["items", "claims"] {
        collect_matching(&root.join(name), &mut files, &|path| {
            path.extension().is_some_and(|ext| ext == "md")
                && path
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .is_some_and(|name| name == "verdicts")
        });
    }
    files.sort();
    files
}

fn worktree_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_matching(&root.join("worktrees"), &mut files, &|path| {
        !path.to_string_lossy().contains("/.git/")
    });
    files.sort();
    files
}

fn hash_list(root: &Path, files: &[PathBuf]) -> Result<String, GuidedError> {
    let mut lines = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(root)
            .map(|rel| rel.display().to_string())
            .unwrap_or_else(|_| path.display().to_string());
        lines.push(format!("{rel} {}", hash_file(path)?));
    }
    lines.sort();
    Ok(join_lines(&lines))
}

fn product_hash_body(repo: &Path, porcelain: &str) -> Result<String, GuidedError> {
    let mut lines = Vec::new();
    for line in records(porcelain) {
        if line.is_empty() {
            continue;
        }
        let path = porcelain_path(line);
        if path_is_program(&path) {
            continue;
        }
        let full = repo.join(&path);
        if full.is_file() {
            lines.push(format!("{path} {}", hash_file(&full)?));
        } else if full.is_dir() {
            lines.push(format!("{path} DIR"));
        }
    }
    lines.sort();
    Ok(join_lines(&lines))
}

fn join_lines(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn hash_field(lines: &[String], path: &str) -> Option<String> {
    for line in lines {
        let mut parts = line.split_whitespace();
        if parts.next() == Some(path) {
            return Some(parts.next().unwrap_or("").to_string());
        }
    }
    None
}

fn porcelain_path(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let rest = if chars.len() >= 3 && chars[2] == ' ' {
        chars[3..].iter().collect()
    } else {
        line.to_string()
    };
    match rest.split_once(" -> ") {
        Some((_, new)) => new.to_string(),
        None => rest,
    }
}

fn path_is_program(path: &str) -> bool {
    if path.ends_with("worktrees") || path.contains("worktrees/") {
        return false;
    }
    path == ".crucible"
        || path.ends_with("/.crucible")
        || path.starts_with(".crucible/")
        || path.contains("/.crucible/")
}

fn copy_rel(root: &Path, files: &[PathBuf], dest_root: &Path) -> Result<(), GuidedError> {
    for path in files {
        let rel = path.strip_prefix(root).unwrap_or(path.as_path());
        let dest = dest_root.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(path, dest)?;
    }
    Ok(())
}

fn restore_copied(root: &Path, snap_root: &Path) -> Result<(), GuidedError> {
    let mut files = Vec::new();
    collect_matching(snap_root, &mut files, &|_| true);
    for path in files {
        let rel = path.strip_prefix(snap_root).unwrap_or(path.as_path());
        let dest = root.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&path, dest)?;
    }
    Ok(())
}

fn collect_matching(dir: &Path, out: &mut Vec<PathBuf>, pred: &impl Fn(&Path) -> bool) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            collect_matching(&path, out, pred);
        } else if meta.is_file() && pred(&path) {
            out.push(path);
        }
    }
}

fn remove_path_all(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if meta.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
}

fn git_stdout(repo: &Path, args: &[&str]) -> Vec<u8> {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .map(|out| out.stdout)
        .unwrap_or_default()
}

fn git_quiet(repo: &Path, args: &[&str]) {
    let _ = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .next()
        .unwrap_or("")
        .trim_end_matches('\r')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cycle::cycle;
    use crate::state::STATE_HEADER;
    use crucible_contract::FixedClock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    const EPOCH: i64 = 1_700_000_000;

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-guided-drive-{}-{}",
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

    fn agents(prog: &Path, coord: &str) {
        fs::write(
            prog.join("agents.tsv"),
            format!(
                "\
c0\tkindA\tm\thigh\t{coord}\n\
a1\tkindA\tm\thigh\techo {{BRIEF}}\n\
a2\tkindB\tm\thigh\techo {{BRIEF}}\n\
mk1\tkindA\tm\thigh\techo {{BRIEF}}\n\
j1\tkindB\tm\thigh\techo {{BRIEF}}\n\
j2\tkindB\tm\thigh\techo {{BRIEF}}\n"
            ),
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

    fn git_init(repo: &Path) {
        fs::create_dir_all(repo).unwrap();
        let run = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "{args:?}");
        };
        run(&["init"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "t"]);
        run(&["config", "commit.gpgsign", "false"]);
        fs::write(repo.join("README"), "hello\n").unwrap();
        run(&["add", "README"]);
        run(&["-c", "commit.gpgsign=false", "commit", "-m", "init"]);
    }

    #[test]
    fn budget_cap_is_deadline_minus_clock() {
        assert_eq!(worker_budget("100", 90, 900), 10);
        assert_eq!(worker_budget("100", 100, 900), 900);
        assert_eq!(worker_budget("100", 50, 10), 10);
        assert_eq!(worker_budget("0", 1, 900), 900);
        assert_eq!(worker_budget("nope", 1, 900), 900);
        assert_eq!(worker_budget("01", 0, 900), 1);
        assert!(path_is_program(".crucible/work/PROGRAM"));
        assert!(!path_is_program(".crucible/work/worktrees/file"));
        assert!(!path_is_program("src/main.rs"));
        assert_eq!(porcelain_path("?? src/a.rs"), "src/a.rs");
        assert_eq!(porcelain_path("R  old -> new"), "new");
    }

    #[test]
    fn human_gate_exits_without_spawning() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        let sentinel = tmp.root.join("spawned");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        agents(&prog, &format!("touch {}", sentinel.display()));
        panel(&prog);
        let out = drive(&prog, &["tick"], &clock()).unwrap();
        assert!(
            out.contains("HUMAN — approve the agent panel:")
                && out.contains("cycle approve-panel\n"),
            "{out}"
        );
        assert!(!out.contains("drive: started"), "{out}");
        assert!(!sentinel.exists(), "human gate spawned a worker");
        assert!(!prog.join("panel-approvals").exists());
        assert!(!prog.join(".drive.lock").exists());
        assert!(repo.join(".wm").join("FLOOR.md").is_file());
        let again = drive(&prog, &["loop"], &clock()).unwrap();
        assert!(
            again.contains("HUMAN — approve the agent panel:"),
            "{again}"
        );
    }

    #[test]
    fn stop_releases_directory_lock_and_ignores_a_regular_file() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        agents(&prog, "true");
        panel(&prog);
        fs::create_dir(prog.join(".drive.lock")).unwrap();
        let out = drive(&prog, &["stop"], &clock()).unwrap();
        assert_eq!(out, format!("released {}/.drive.lock\n", prog.display()));
        assert!(!prog.join(".drive.lock").exists());

        fs::write(prog.join(".drive.lock"), "not a lock\n").unwrap();
        let out = drive(&prog, &["stop"], &clock()).unwrap();
        assert_eq!(out, "no .drive.lock\n");
        assert_eq!(
            fs::read_to_string(prog.join(".drive.lock")).unwrap(),
            "not a lock\n"
        );
        let out = drive(&prog, &["tick"], &clock()).unwrap();
        assert!(out.contains("HUMAN — approve the agent panel:"), "{out}");
        assert_eq!(
            fs::read_to_string(prog.join(".drive.lock")).unwrap(),
            "not a lock\n"
        );

        fs::remove_file(prog.join(".drive.lock")).unwrap();
        fs::create_dir(prog.join(".drive.lock")).unwrap();
        let err = drive(&prog, &["tick"], &clock()).unwrap_err().to_string();
        assert_eq!(err, "drive already running");
        assert!(prog.join(".drive.lock").is_dir());
    }

    #[test]
    fn second_live_attempt_on_wait_restores_and_refuses() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        git_init(&repo);
        let prog = repo.join(".crucible").join("work");
        let script = tmp.root.join("coord.sh");
        let extra = prog.join("attempts").join("A1700000099.1.2");
        let body = format!(
            "#!/bin/sh\nprintf '%s\\n' 'note: touched' >> '{prog}'\nmkdir -p '{dir}'\nprintf '%s\\n' 'state\tepoch\tpid\treason' 'DISPATCHED\t1\t-\tfake' > '{dir}/events.tsv'\n",
            prog = prog.join("PROGRAM").display(),
            dir = extra.display(),
        );
        fs::write(&script, body).unwrap();
        program(&prog, &repo, "");
        agents(&prog, &format!("/bin/sh {}", script.display()));
        panel(&prog);
        let clock = clock();
        cycle(&prog, &["approve-panel"], &clock).unwrap();
        fs::write(
            prog.join("PROBLEM.md"),
            "# Broken behavior\n\nThe program does not preserve approved scope.\n",
        )
        .unwrap();
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
        let claim = prog.join("attempts").join("A1.2.3");
        fs::create_dir_all(&claim).unwrap();
        fs::write(
            claim.join("meta.tsv"),
            "\
attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of
A1.2.3\tC1\t-\tCLAIM\tclaim-auditor\ta1\tkindA\t-\tFOCUSED\tDISPATCHED\t1\t2\t-
",
        )
        .unwrap();
        fs::write(claim.join("transport"), "multi-agent\n").unwrap();
        fs::write(claim.join("contract-audit.md"), "VERDICT: PASS\n").unwrap();
        fs::write(
            claim.join("events.tsv"),
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
        cycle(&prog, &["approve"], &clock).unwrap();
        fs::write(
            prog.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tBUILD\tw1\tLOW\tA1700000000.9.1\t-\t1\n"),
        )
        .unwrap();
        let running = prog.join("attempts").join("A1700000000.9.1");
        fs::create_dir_all(&running).unwrap();
        fs::write(
            running.join("meta.tsv"),
            "\
attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of
A1700000000.9.1\talpha\t-\tw1\tmaker\tmk1\tkindA\t-\tFOCUSED\tRUNNING\t1\t2\t-
",
        )
        .unwrap();
        fs::write(
            running.join("events.tsv"),
            "state\tepoch\tpid\treason\nRUNNING\t1\t999999\tfixture\n",
        )
        .unwrap();
        let saved = fs::read(prog.join("PROGRAM")).unwrap();
        let err = drive(&prog, &["tick"], &clock).unwrap_err().to_string();
        assert_eq!(
            err,
            "refused: WAIT inflight — do not start a second attempt (A1700000099.1.2)"
        );
        assert_eq!(fs::read(prog.join("PROGRAM")).unwrap(), saved);
        assert!(!fs::read_to_string(prog.join("PROGRAM"))
            .unwrap()
            .contains("note: touched"));
        assert!(extra.join("events.tsv").is_file());
        assert!(!prog.join(".drive.lock").exists());
    }

    #[test]
    fn working_mode_does_not_call_project_cycle_line() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "working-mode: yes\n");
        agents(&prog, "true");
        panel(&prog);
        let out = drive(&prog, &["tick"], &clock()).unwrap();
        assert!(out.contains("HUMAN — approve the agent panel:"), "{out}");
        assert!(
            !repo.join(".wm").exists(),
            "working-mode program projected a floor"
        );
        assert!(prog.join("STATUS.md").is_file());
    }

    #[test]
    fn worker_timeout_uses_clock_budget_and_real_sleep() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        agents(&prog, "exec sleep 5");
        panel(&prog);
        let clock = clock();
        cycle(&prog, &["approve-panel"], &clock).unwrap();
        let id = "A1700000000.1.1";
        let attempt = prog.join("attempts").join(id);
        fs::create_dir_all(&attempt).unwrap();
        fs::write(attempt.join("contract.md"), "contract\n").unwrap();
        fs::write(attempt.join("transport"), "multi-agent\n").unwrap();
        fs::write(attempt.join("contract-audit.md"), "VERDICT: PASS\n").unwrap();
        fs::write(
            attempt.join("meta.tsv"),
            "\
attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of
A1700000000.1.1\tC1\t-\tCLAIM\tmaker\tc0\tkindA\t-\tFOCUSED\tDISPATCHED\t1\t1700000001\t-
",
        )
        .unwrap();
        fs::write(
            attempt.join("events.tsv"),
            "state\tepoch\tpid\treason\nDISPATCHED\t1\t-\tfixture\n",
        )
        .unwrap();
        let started = Instant::now();
        let out = drive(&prog, &["tick"], &clock).unwrap();
        let elapsed = started.elapsed();
        assert!(
            out.contains(&format!("drive: {id} TIMEOUT after 1s\n")),
            "{out}"
        );
        assert!(!out.contains("drive: started"), "{out}");
        let events = fs::read_to_string(attempt.join("events.tsv")).unwrap();
        assert!(
            events.contains("TIMEOUT\t1700000000\t"),
            "event epoch must be the fixed clock, not wall time: {events}"
        );
        assert!(elapsed >= Duration::from_millis(700), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(4), "{elapsed:?}");
        assert!(!prog.join(".drive.lock").exists());
    }

    fn meta_row(id: &str, item: &str, role: &str, agent: &str, state: &str) -> String {
        format!(
            "\
attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of
{id}\t{item}\t-\tw1\t{role}\t{agent}\tkindA\t-\tFOCUSED\t{state}\t1\t2\t-
"
        )
    }

    #[test]
    fn claim_dispatch_die_does_not_seal_or_skip_coordinator() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        let sentinel = tmp.root.join("coordinator-ran");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        agents(&prog, &format!("touch {}", sentinel.display()));
        panel(&prog);
        let clock = clock();
        cycle(&prog, &["approve-panel"], &clock).unwrap();
        fs::write(
            prog.join("PROBLEM.md"),
            "# Gap\n\nA falsifiable outcome is missing.\n",
        )
        .unwrap();
        fs::write(
            prog.join("CLAIMS.md"),
            "# CLAIMS\n\n### C1 scope is wrong\n    status: OPEN\n",
        )
        .unwrap();
        // Unsealed claim attempt. A counted dispatch would make the parent seal it and
        // return before the coordinator. A missing role file must die first.
        let attempt = prog.join("attempts").join("A1700000000.3.1");
        fs::create_dir_all(&attempt).unwrap();
        fs::write(
            attempt.join("contract.md"),
            "Read this file and follow it exactly.\nAuditing claim C1.\n",
        )
        .unwrap();
        fs::write(attempt.join("transport"), "multi-agent\n").unwrap();
        fs::write(
            attempt.join("meta.tsv"),
            meta_row("A1700000000.3.1", "C1", "claim-auditor", "a1", "DISPATCHED"),
        )
        .unwrap();
        fs::write(
            attempt.join("events.tsv"),
            "state\tepoch\tpid\treason\nDISPATCHED\t1\t-\tfixture\n",
        )
        .unwrap();
        let err = drive(&prog, &["tick"], &clock).unwrap_err().to_string();
        assert!(err.contains("no such role: claim-auditor"), "{err}");
        assert!(
            !attempt.join("contract-audit.md").exists(),
            "refused claim dispatch still sealed"
        );
        assert!(
            !sentinel.exists() && !prog.join("DRIVE.BRIEF.md").exists(),
            "claim dispatch die returned success and skipped the coordinator"
        );
    }

    #[test]
    fn refused_phase_to_build_does_not_start_sealed_worker() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        let sealed = prog.join("attempts").join("A1700000000.4.1");
        let script = tmp.root.join("coord.sh");
        // Planted after the pre-coordinator invoke, which sees no sealed attempt.
        // A swallowed phase refusal would then start this worker.
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nmkdir -p '{dir}'\ncat > '{dir}/meta.tsv' <<'EOF'\n{meta}EOF\nprintf '%s\\n' 'state\tepoch\tpid\treason' 'DISPATCHED\t1\t-\tfixture' > '{dir}/events.tsv'\nprintf '%s\\n' 'multi-agent' > '{dir}/transport'\nprintf '%s\\n' 'VERDICT: PASS' > '{dir}/contract-audit.md'\nprintf '%s\\n' 'contract' > '{dir}/contract.md'\n",
                dir = sealed.display(),
                meta = meta_row("A1700000000.4.1", "alpha", "maker", "c0", "DISPATCHED"),
            ),
        )
        .unwrap();
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        agents(&prog, &format!("/bin/sh {}", script.display()));
        panel(&prog);
        let clock = clock();
        cycle(&prog, &["approve-panel"], &clock).unwrap();
        fs::write(
            prog.join("PROBLEM.md"),
            "# Broken behavior\n\nThe program does not preserve approved scope.\n",
        )
        .unwrap();
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
        let claim = prog.join("attempts").join("A1.2.3");
        fs::create_dir_all(&claim).unwrap();
        fs::write(
            claim.join("meta.tsv"),
            meta_row("A1.2.3", "C1", "claim-auditor", "a1", "DISPATCHED"),
        )
        .unwrap();
        fs::write(claim.join("transport"), "multi-agent\n").unwrap();
        fs::write(claim.join("contract-audit.md"), "VERDICT: PASS\n").unwrap();
        fs::write(
            claim.join("events.tsv"),
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
        cycle(&prog, &["approve"], &clock).unwrap();
        fs::write(
            prog.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tREVIEW\tw1\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        let judged = prog.join("attempts").join("A1700000000.5.1");
        fs::create_dir_all(&judged).unwrap();
        fs::write(
            judged.join("result.md"),
            "ITEM: alpha\nROLE: judge\nNEXT: FIX\n",
        )
        .unwrap();
        fs::write(
            judged.join("meta.tsv"),
            meta_row("A1700000000.5.1", "alpha", "judge", "j1", "STOPPED"),
        )
        .unwrap();
        fs::write(
            judged.join("events.tsv"),
            "state\tepoch\tpid\treason\nSTOPPED\t1\t-\tfixture\n",
        )
        .unwrap();
        let err = drive(&prog, &["tick"], &clock).unwrap_err().to_string();
        assert!(err.contains("no such item: alpha"), "{err}");
        assert!(sealed.join("contract.md").is_file(), "{err}");
        assert!(!sealed.join("invoke.log").exists(), "{err}");
        let events = fs::read_to_string(sealed.join("events.tsv")).unwrap();
        assert!(!events.contains("RUNNING"), "{events}");
    }

    #[test]
    fn failing_coordinator_keeps_child_output_with_the_refusal() {
        let tmp = Tmp::new();
        let repo = tmp.root.join("repo");
        let prog = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        program(&prog, &repo, "");
        agents(
            &prog,
            "printf 'stdout line\\n'; printf 'coordinator blew up\\n' >&2; exit 1",
        );
        panel(&prog);
        let clock = clock();
        cycle(&prog, &["approve-panel"], &clock).unwrap();
        let err = drive(&prog, &["tick"], &clock).unwrap_err().to_string();
        let stdout_at = err.find("stdout line\n").expect(&err);
        let stderr_at = err.find("coordinator blew up\n").expect(&err);
        let refused_at = err
            .find("refused: coordinator invoke failed —")
            .expect(&err);
        assert!(stdout_at < stderr_at && stderr_at < refused_at, "{err}");
    }
}
