//! Claim, managed, and legacy dispatch, plus the task helpers those commands call.
//!
//! Attempt ids are `A{now}.{pid}.{n}`. `{now}` is [`Clock::now_unix`]. `{pid}` is
//! [`std::process::id`]. Not `SystemTime`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crucible_contract::Clock;

use crate::attempt::{attempt_live_for, attempt_pass_exists, canonical_pass_exists, result_field};
use crate::claims::require_panel_approval;
use crate::cycle::{
    attempt_child_dirs, attempt_dir, attempt_meta, attempt_state, attempt_terminal, file_name,
    is_claim_slug, list_files, ls_mode, program_field, self_path, workid,
};
use crate::panel::{
    agent_col, assign_role_normalize, h12, hash_file, is_posint, is_regular, kind_of, split_tabs,
};
use crate::phase::phase_of;
use crate::program::uses_managed_lifecycle;
use crate::state::{state_update_item, state_value};
use crate::{message, records, GuidedError};

const META_HEADER: &str = "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of";
const TASK_HEADER: &str = "task_id\tdepends_on\tpaths_file\tverify_script";

/// `crucible dispatch`.
pub fn dispatch(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    let slug = args.first().copied().unwrap_or("");
    let role = args.get(1).copied().unwrap_or("");
    let name = args.get(2).copied().unwrap_or("");
    if slug.is_empty() || role.is_empty() || name.is_empty() {
        return Err(message("usage: crucible dispatch CN|ITEM ROLE AGENT"));
    }
    if is_claim_slug(slug) {
        return dispatch_claim(root, clock, args);
    }
    if uses_managed_lifecycle(root)? {
        return dispatch_managed(root, clock, args);
    }
    dispatch_legacy(root, slug, role, name)
}

pub(crate) fn item_dir(root: &Path, slug: &str) -> PathBuf {
    root.join("items").join(slug)
}

pub(crate) fn need(root: &Path, slug: &str) -> Result<PathBuf, GuidedError> {
    if slug.is_empty() {
        return Err(message("need a slug"));
    }
    let dir = item_dir(root, slug);
    if !dir.is_dir() {
        return Err(message(format!("no such item: {slug}")));
    }
    Ok(dir)
}

pub(crate) fn require_registered(root: &Path, name: &str) -> Result<(), GuidedError> {
    if kind_of(root, name)?.is_empty() {
        return Err(message(format!(
            "unregistered agent: {name} — add '{name}<TAB>kind' to {}/agents.tsv",
            root.display()
        )));
    }
    Ok(())
}

pub(crate) fn require_role_cast(root: &Path, role: &str, who: &str) -> Result<(), GuidedError> {
    if !crate::program::uses_guided_cycle(root)? {
        return Ok(());
    }
    let nrole = assign_role_normalize(role);
    if agent_cast_for_role(root, nrole, who)? {
        return Ok(());
    }
    Err(message(format!(
        "refused: agent {who} is not cast as {nrole} in PANEL.ASSIGN.tsv — update casting and cycle approve-panel"
    )))
}

fn agent_cast_for_role(root: &Path, lookup: &str, agent: &str) -> Result<bool, GuidedError> {
    let path = root.join("PANEL.ASSIGN.tsv");
    if !path.is_file() {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    for line in records(&text) {
        let fields = split_tabs(line);
        let mut assigned = fields.first().copied().unwrap_or("");
        if assigned.is_empty() || assigned.starts_with('#') || assigned == "role" {
            continue;
        }
        if assigned == "judge" {
            assigned = "reviewer";
        }
        let who = fields.get(1).copied().unwrap_or("");
        if assigned == lookup && who == agent {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn is_maker(root: &Path, slug: &str, agent: &str) -> Result<bool, GuidedError> {
    let dir = item_dir(root, slug);
    let makers = dir.join("MAKERS.tsv");
    if makers.is_file() {
        let Ok(text) = fs::read_to_string(&makers) else {
            return Ok(false);
        };
        for (idx, rec) in records(&text).into_iter().enumerate() {
            if idx == 0 {
                continue;
            }
            if split_tabs(rec).first().copied() == Some(agent) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let maker = dir.join("MAKER");
    if maker.is_file() {
        let Ok(text) = fs::read_to_string(&maker) else {
            return Ok(false);
        };
        return Ok(records(&text).first().copied() == Some(agent));
    }
    Ok(false)
}

pub(crate) fn record_maker(root: &Path, slug: &str, agent: &str) -> Result<(), GuidedError> {
    let dir = item_dir(root, slug);
    let maker = dir.join("MAKER");
    let nonempty = fs::metadata(&maker)
        .map(|meta| meta.is_file() && meta.len() > 0)
        .unwrap_or(false);
    if !nonempty {
        fs::write(&maker, format!("{agent}\n"))?;
    }
    let path = dir.join("MAKERS.tsv");
    if !path.exists() {
        fs::write(&path, "agent\tkind\n")?;
    }
    let text = fs::read_to_string(&path).unwrap_or_default();
    let found = records(&text)
        .into_iter()
        .skip(1)
        .any(|rec| split_tabs(rec).first().copied() == Some(agent));
    if !found {
        let kind = kind_of(root, agent)?;
        let mut file = OpenOptions::new().append(true).open(&path)?;
        writeln!(file, "{agent}\t{kind}")?;
    }
    Ok(())
}

pub(crate) fn review_relation(
    root: &Path,
    slug: &str,
    reviewer: &str,
) -> Result<String, GuidedError> {
    let dir = item_dir(root, slug);
    let names = if dir.join("MAKERS.tsv").is_file() {
        let text = fs::read_to_string(dir.join("MAKERS.tsv")).unwrap_or_default();
        records(&text)
            .into_iter()
            .skip(1)
            .map(|rec| split_tabs(rec).first().copied().unwrap_or("").to_string())
            .collect::<Vec<_>>()
    } else if dir.join("MAKER").is_file() {
        let text = fs::read_to_string(dir.join("MAKER")).unwrap_or_default();
        let line = records(&text).first().copied().unwrap_or("");
        if line.is_empty() {
            return Ok("UNPROVEN".to_string());
        }
        vec![line.to_string()]
    } else {
        return Ok("UNPROVEN".to_string());
    };
    if names.iter().all(|name| name.is_empty()) && !dir.join("MAKERS.tsv").is_file() {
        return Ok("UNPROVEN".to_string());
    }
    let reviewer_kind = kind_of(root, reviewer)?;
    let mut same = false;
    let mut different = false;
    for name in names.iter().flat_map(|name| name.split_whitespace()) {
        let maker_kind = kind_of(root, name)?;
        if !maker_kind.is_empty() && maker_kind == reviewer_kind {
            same = true;
        } else {
            different = true;
        }
    }
    Ok(if same && different {
        "MIXED-FAMILY"
    } else if same {
        "SAME-FAMILY"
    } else {
        "CROSS-FAMILY"
    }
    .to_string())
}

pub(crate) fn tgt(root: &Path, slug: &str, key: &str) -> Result<String, GuidedError> {
    let path = item_dir(root, slug).join("TARGET");
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(String::new());
    };
    let prefix = format!("{key}: ");
    Ok(records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string())
}

pub(crate) fn invocation(root: &Path, name: &str, brief: &str) -> Result<String, GuidedError> {
    let cmd = agent_col(root, name, 5)?;
    if cmd.is_empty() {
        return Ok(format!(
            "(no command registered for {name} — add column 5 to agents.tsv)"
        ));
    }
    let model = agent_col(root, name, 3)?;
    let effort = agent_col(root, name, 4)?;
    let mut out = cmd.replace("{BRIEF}", brief);
    out = out.replace("{MODEL}", &model);
    out = out.replace("{EFFORT}", &effort);
    Ok(out)
}

fn eprint_run(root: &Path, name: &str, role: &str, brief: &Path, claim: Option<&str>, hint: bool) {
    let Ok(cmd) = invocation(root, name, &brief.display().to_string()) else {
        return;
    };
    if let Some(cn) = claim {
        eprintln!("\n# {name} as {role} on claim {cn}. Run this:\n{cmd}");
    } else if hint {
        eprintln!(
            "\n# {name} as {role}. Run this:\n{cmd}\n# or give {name} any way you like, with exactly: read {} and follow it exactly",
            brief.display()
        );
    } else {
        eprintln!("\n# {name} as {role}. Run this:\n{cmd}");
    }
}

struct Minted {
    id: String,
    staging: PathBuf,
    target: PathBuf,
}

fn mint_attempt(root: &Path, now: i64) -> Result<Minted, GuidedError> {
    let attempts = root.join("attempts");
    fs::create_dir_all(&attempts)?;
    let pid = std::process::id();
    let mut n = 1u64;
    loop {
        let id = format!("A{now}.{pid}.{n}");
        let target = attempts.join(&id);
        let staging = attempts.join(format!(".{id}.{pid}.tmp"));
        if !target.exists() {
            match fs::create_dir(&staging) {
                Ok(()) => {
                    return Ok(Minted {
                        id,
                        staging,
                        target,
                    })
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err.into()),
            }
        }
        n += 1;
        if n > 100_000 {
            return Err(message("could not allocate attempt id"));
        }
    }
}

fn write_ledger(staging: &Path, row: &str, now: i64) -> Result<(), GuidedError> {
    fs::write(staging.join("meta.tsv"), format!("{META_HEADER}\n{row}\n"))?;
    fs::write(
        staging.join("events.tsv"),
        format!("state\tepoch\tpid\treason\nDISPATCHED\t{now}\t-\tdispatch-recorded\n"),
    )?;
    Ok(())
}

fn dispatch_claim(root: &Path, clock: &dyn Clock, args: &[&str]) -> Result<String, GuidedError> {
    let cn = args[0];
    let role = args[1];
    let name = args[2];
    require_panel_approval(root)?;
    let role_path = root.join("roles").join(format!("{role}.md"));
    if !role_path.is_file() {
        return Err(message(format!("no such role: {role}")));
    }
    require_registered(root, name)?;
    require_role_cast(root, role, name)?;
    let claims = root.join("CLAIMS.md");
    if !claims.is_file() {
        return Err(message("no CLAIMS.md here"));
    }
    let claims_text = fs::read_to_string(&claims)?;
    let heading = format!("### {cn} ");
    if !records(&claims_text)
        .iter()
        .any(|line| line.starts_with(&heading))
    {
        return Err(message(format!("no such claim: {cn}")));
    }
    let cdir = root.join("claims").join(cn);
    fs::create_dir_all(cdir.join("verdicts"))?;
    fs::create_dir_all(cdir.join("dispatches"))?;
    fs::create_dir_all(cdir.join("evidence"))?;
    let dispatches = cdir.join("dispatches");
    let mut n = 1u64;
    let out = loop {
        let path = dispatches.join(format!("{n}-{role}-{name}.md"));
        if !path.exists() {
            break path;
        }
        n += 1;
    };
    let role_text = fs::read_to_string(&role_path)?;
    let self_cmd = self_path(root);
    let mut body = String::new();
    push(
        &mut body,
        &format!("# Dispatch {n} — {role} — {name} — claim {cn}"),
    );
    push(&mut body, "");
    push(
        &mut body,
        &format!(
            "You are **{name}**, acting as **{role}**, auditing claim **{cn}**. This file is your"
        ),
    );
    push(
        &mut body,
        "complete task. Read it and follow it exactly. If it is insufficient, return",
    );
    push(&mut body, "NEEDS_CONTEXT naming what is missing.");
    push(&mut body, "");
    push(
        &mut body,
        &format!(
            "- **Purpose right now:** {}",
            role_field(&role_text, "purpose")
        ),
    );
    push(
        &mut body,
        &format!("- **You may read:** {}", role_field(&role_text, "may-read")),
    );
    push(
        &mut body,
        &format!(
            "- **You must not read:** {}",
            role_field(&role_text, "must-not-read")
        ),
    );
    push(
        &mut body,
        &format!(
            "- **You must return:** {}",
            role_field(&role_text, "return")
        ),
    );
    push(
        &mut body,
        &format!(
            "- **How to verify your own work:** {}",
            role_field(&role_text, "verify")
        ),
    );
    push(&mut body, "");
    push(&mut body, "Record every command you run:");
    push(&mut body, "");
    push(
        &mut body,
        &format!("    {self_cmd} run-claim {cn} {name} -- <command>"),
    );
    push(&mut body, "");
    push(&mut body, "Then record your result:");
    push(&mut body, "");
    if role == "scout" {
        push(
            &mut body,
            &format!(
                "- **You must write:** claims/{cn}/evidence/ (via run-claim) and the scout result below"
            ),
        );
        push(&mut body, "");
        push(
            &mut body,
            &format!("    {self_cmd} claim scout {cn} ABSENT|PARTLY-EXISTS|FULLY-EXISTS {name}"),
        );
        push(&mut body, "");
        push(
            &mut body,
            "FULLY-EXISTS means the repository already does this; it blocks admission, so say where.",
        );
        push(
            &mut body,
            "PARTLY-EXISTS means some of it exists: name what does and what is missing, because the",
        );
        push(
            &mut body,
            "item will be narrowed to the gap. ABSENT is a claim in its own right, so list every",
        );
        push(
            &mut body,
            "search you ran and let it be checked. Search by behaviour, not by name — the thing that",
        );
        push(
            &mut body,
            "would be built rarely contains the words someone would call it.",
        );
        push(&mut body, "");
        push(
            &mut body,
            "If you independently verify the claim itself, you may also record this affirmative",
        );
        push(
            &mut body,
            "verdict. It does not replace the separately required scout result:",
        );
        push(&mut body, "");
        push(
            &mut body,
            &format!("    {self_cmd} claim verdict {cn} {name} TRUE"),
        );
    } else {
        push(
            &mut body,
            &format!("    {self_cmd} claim verdict {cn} {name} TRUE|FALSE|STALE|UNVERIFIABLE"),
        );
        push(&mut body, "");
        push(
            &mut body,
            "TRUE means true of the code as it exists now, cited to file:line. STALE means it was",
        );
        push(
            &mut body,
            "true and has since been fixed — cite the fix. UNVERIFIABLE means the code cannot settle",
        );
        push(
            &mut body,
            "it; name what is missing. Never guess, and never resolve doubt in favour of the claim.",
        );
    }
    push(&mut body, "");
    push(&mut body, "---");
    push(&mut body, "");
    push(&mut body, "## The claim");
    push(&mut body, "");
    body.push_str(&claim_section(&claims_text, cn));
    push(&mut body, "");
    push(&mut body, "---");
    push(&mut body, "");
    push(&mut body, "## Your role in full");
    push(&mut body, "");
    body.push_str(&from_heading(&role_text, "## Instructions"));
    push(&mut body, "");
    push(&mut body, "---");
    push(&mut body, "");
    push(&mut body, "## The rules that bind you");
    push(&mut body, "");
    match fs::read_to_string(root.join("RULES.md")) {
        Ok(rules) => body.push_str(&ensure_nl(&rules)),
        Err(_) => push(&mut body, "(RULES.md missing)"),
    }
    fs::write(&out, &body)?;
    if uses_managed_lifecycle(root)? {
        let now = clock.now_unix();
        let minted = mint_attempt(root, now)?;
        let kind = kind_of(root, name)?;
        let row = format!(
            "{}\t{cn}\t-\tCLAIM\t{role}\t{name}\t{kind}\t-\tFOCUSED\tDISPATCHED\t{now}\t{}\t-",
            minted.id,
            now.saturating_add(1800)
        );
        write_ledger(&minted.staging, &row, now)?;
        let mut published = body.clone();
        if role == "scout" {
            published.push_str(&seal_block(&self_cmd, &minted.id, true));
            fs::write(&out, &published)?;
        }
        let mut contract = fs::read_to_string(&out)?;
        if role != "scout" {
            contract.push_str(&seal_block(&self_cmd, &minted.id, false));
        }
        fs::write(minted.staging.join("contract.md"), contract)?;
        fs::write(
            minted.staging.join("claim-dispatch.txt"),
            format!("claim-dispatch: {}\n", rel_under(root, &out)),
        )?;
        fs::rename(&minted.staging, &minted.target)?;
        if role != "scout" {
            let mut updated = fs::read_to_string(&out)?;
            if !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(&format!("attempt-id: {}\n", minted.id));
            fs::write(&out, updated)?;
        }
        eprintln!("attempt {} (claim independence ledger)", minted.id);
    }
    eprint_run(root, name, role, &out, Some(cn), false);
    Ok(format!("{}\n", out.display()))
}

fn seal_block(self_cmd: &str, id: &str, with_attempt_line: bool) -> String {
    let mut out = String::new();
    if with_attempt_line {
        out.push_str(&format!("attempt-id: {id}\n"));
    }
    out.push_str("\n## Independence seal (before verdict)\n\n");
    if with_attempt_line {
        out.push_str("Before a TRUE verdict or scout result, while DISPATCHED, record transport and a contract-audit PASS:\n\n");
    } else {
        out.push_str("While DISPATCHED, record transport and a contract-audit PASS:\n\n");
    }
    out.push_str(&format!(
        "    {self_cmd} attempt transport {id} multi-agent|acp|subagent\n"
    ));
    out.push_str(&format!(
        "    {self_cmd} contract-audit {id} <contract-auditor> PASS\n"
    ));
    out
}

fn push(buf: &mut String, line: &str) {
    buf.push_str(line);
    buf.push('\n');
}

fn ensure_nl(text: &str) -> String {
    if text.is_empty() || text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    }
}

fn role_field(text: &str, key: &str) -> String {
    let prefix = format!("{key}: ");
    records(text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string()
}

fn from_heading(text: &str, heading: &str) -> String {
    let lines = records(text);
    let Some(start) = lines.iter().position(|line| line.starts_with(heading)) else {
        return String::new();
    };
    let mut out = String::new();
    for line in &lines[start..] {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn claim_section(text: &str, cn: &str) -> String {
    let marker = format!("### {cn} ");
    let mut out = String::new();
    let mut on = false;
    for line in records(text) {
        if !on && line.starts_with(&marker) {
            on = true;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if on && next_claim_heading(line) {
            break;
        }
        if on {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn next_claim_heading(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() > 5 && bytes.starts_with(b"### C") && bytes[5].is_ascii_digit()
}

fn rel_under(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|rest| rest.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

struct DispatchGuard {
    lock: PathBuf,
    staging: Option<PathBuf>,
    armed: bool,
}

impl Drop for DispatchGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Some(staging) = self.staging.take() {
            for name in ["meta.tsv", "events.tsv", "contract.md", "task.tsv"] {
                let _ = fs::remove_file(staging.join(name));
            }
            let _ = fs::remove_dir(&staging);
        }
        let _ = fs::remove_dir(&self.lock);
    }
}

/// Managed dispatch. `task dispatch` calls this directly so a slug like `C1` is an item, not a claim.
pub(crate) fn dispatch_managed(
    root: &Path,
    clock: &dyn Clock,
    args: &[&str],
) -> Result<String, GuidedError> {
    let slug = args[0];
    let role = args[1];
    let agent = args[2];
    let criterion = arg_or(args, 3, "A1");
    let class = arg_or(args, 4, "FOCUSED");
    let retry = arg_or(args, 5, "-");
    let task = arg_or(args, 6, "-");
    let dir = need(root, slug)?;
    require_panel_approval(root)?;
    if !token_ok(criterion) {
        return Err(message(format!("invalid criterion: {criterion}")));
    }
    let item_text = fs::read_to_string(dir.join("ITEM.md")).unwrap_or_default();
    if !declares_criterion(&item_text, criterion) {
        return Err(message(format!(
            "criterion is not declared by the item: {criterion}"
        )));
    }
    if !matches!(class, "FOCUSED" | "FULL_SUITE" | "EXTERNAL" | "MANUAL") {
        return Err(message(format!("invalid evidence class: {class}")));
    }
    let role_path = root.join("roles").join(format!("{role}.md"));
    if !role_path.is_file() {
        return Err(message(format!("no such role: {role}")));
    }
    require_registered(root, agent)?;
    require_role_cast(root, role, agent)?;
    let lock = root.join(".dispatch.lock");
    if fs::create_dir(&lock).is_err() {
        return Err(message("managed dispatch already in progress"));
    }
    let mut guard = DispatchGuard {
        lock: lock.clone(),
        staging: None,
        armed: true,
    };
    let stage = phase_of(root, slug)?;
    let mut wid = workid(root, slug)?;
    let mut deps: Option<String> = None;
    match role {
        "maker" => {
            if stage != "BUILD" {
                return Err(message("maker dispatch requires BUILD"));
            }
            if crate::program::uses_guided_cycle(root)?
                && !plan_audit_pass(&dir.join("plan-audit.md"))
            {
                return Err(message("refused: maker dispatch requires plan-audit PASS"));
            }
            if dir.join("TASKS.tsv").is_file() {
                if task == "-" {
                    return Err(message(format!(
                        "item uses a task DAG; dispatch through: {} task dispatch",
                        self_path(root)
                    )));
                }
                if task_field(root, slug, task, 1)?.is_empty() {
                    return Err(message(format!("no such task: {task}")));
                }
                let task_deps = task_field(root, slug, task, 2)?;
                if task_deps.contains(',') {
                    return Err(message(format!(
                        "task execution with multiple direct dependencies is not available: {task}"
                    )));
                }
                if !task_dependencies_pass(root, slug, &task_deps)? {
                    return Err(message(format!(
                        "task dependencies have not passed: {task}"
                    )));
                }
                if task_pass_exists(root, slug, task)? {
                    return Err(message(format!("task already has PASS: {task}")));
                }
                if let Some(live_task) = task_live_attempt(root, slug, task)? {
                    return Err(message(format!(
                        "task already has live attempt {live_task}"
                    )));
                }
                if let Some(pending) = task_retry_available(root, slug, task)? {
                    if retry == "-" {
                        return Err(message(format!("task {task} requires retry of {pending}")));
                    }
                }
                let max_parallel = env_or("CRUCIBLE_MAX_PARALLEL_MAKERS", "3");
                if !is_posint(&max_parallel) {
                    return Err(message(
                        "CRUCIBLE_MAX_PARALLEL_MAKERS must be a positive integer",
                    ));
                }
                let live_count = task_live_count(root, slug)?;
                let max_n: u64 = max_parallel.parse().unwrap_or(0);
                if live_count >= max_n {
                    return Err(message(format!(
                        "parallel maker limit reached: {live_count}/{max_parallel}"
                    )));
                }
                if !dir.join("TARGET").is_file() {
                    return Err(message("task dispatch requires a Git target"));
                }
                deps = Some(task_deps);
            } else if task != "-" {
                return Err(message(format!("item has no task DAG: {slug}")));
            }
        }
        "judge" | "adversary" => {
            if stage != "REVIEW" {
                return Err(message(format!("{role} dispatch requires REVIEW")));
            }
            if is_maker(root, slug, agent)? {
                return Err(message(format!("refused: {agent} is a maker of {slug}")));
            }
        }
        _ => {
            return Err(message(
                "managed lifecycle dispatch role must be maker, judge, or adversary",
            ));
        }
    }
    if task != "-" {
        let task_repo = tgt(root, slug, "repo")?;
        let item_branch = tgt(root, slug, "branch")?;
        let item_base = tgt(root, slug, "base")?;
        if !git_quiet(
            &task_repo,
            &["rev-parse", "--verify", "--quiet", &item_branch],
        ) && !git_quiet(&task_repo, &["branch", &item_branch, &item_base])
        {
            return Err(message(format!(
                "could not create item integration branch {item_branch} from {item_base}"
            )));
        }
        let deps = deps
            .as_deref()
            .ok_or_else(|| message("deps: parameter not set"))?;
        if deps == "-" {
            wid = git_rev12(&task_repo, &["rev-parse", "--verify", &item_branch]);
        } else if let Some(dep_result) = task_result_file(root, slug, deps)? {
            let text = fs::read_to_string(dep_result)?;
            wid = result_field(&text, "WORK-ID");
        } else {
            wid.clear();
        }
    }
    if retry != "-" {
        attempt_dir(root, retry)?;
        if attempt_state(root, retry)? != "TIMEOUT" {
            return Err(message("retry source must be an observed TIMEOUT"));
        }
        if attempt_meta(root, retry, 13)? != "-" {
            return Err(message("refused: only one infrastructure retry is allowed"));
        }
        let same = attempt_meta(root, retry, 2)? == slug
            && attempt_meta(root, retry, 3)? == task
            && attempt_meta(root, retry, 5)? == role
            && attempt_meta(root, retry, 6)? == agent
            && attempt_meta(root, retry, 8)? == criterion
            && attempt_meta(root, retry, 9)? == class
            && attempt_meta(root, retry, 4)? == wid;
        if !same {
            return Err(message("retry key does not match the prior attempt"));
        }
    }
    if state_value(root, slug, 2)?.as_deref() != Some("ACTIVE") {
        return Err(message(format!("item is not active: {slug}")));
    }
    let recorded = state_value(root, slug, 6)?.unwrap_or_default();
    if task != "-" && (recorded == "-" || recorded == "TASKS") {
        // Parallel task attempts share the TASKS pointer.
    } else if retry != "-" && recorded == retry {
        // The retry replaces the attempt it names.
    } else if recorded != "-" {
        return Err(message(format!(
            "refused: item already has in-flight attempt {recorded}"
        )));
    }
    if let Some(live) = attempt_live_for(root, slug, role, criterion, task) {
        return Err(message(format!(
            "refused: live attempt {live} already owns {slug}/{role}/{criterion}"
        )));
    }
    if attempt_pass_exists(root, slug, &wid, role, agent, criterion, task) {
        return Err(message(format!(
            "refused: {agent} already has current-work PASS for {role}/{criterion}"
        )));
    }
    if matches!(class, "FULL_SUITE" | "EXTERNAL") && canonical_pass_exists(root, slug, &wid, class)
    {
        return Err(message(format!(
            "refused: canonical {class} PASS already exists for {slug} at {wid}"
        )));
    }
    let seconds = match role {
        "maker" => env_or("CRUCIBLE_MAKER_SECONDS", "2700"),
        _ => env_or("CRUCIBLE_REVIEW_SECONDS", "1800"),
    };
    if !is_posint(&seconds) {
        return Err(message("attempt budget must be a positive integer"));
    }
    // `$((now + seconds))`: a leading 0 is octal, so 010 is 8 and 08 aborts.
    let seconds = shell_arith(&seconds)?;
    let now = clock.now_unix();
    let minted = mint_attempt(root, now)?;
    guard.staging = Some(minted.staging.clone());
    let mut task_branch = String::new();
    let mut task_worktree = PathBuf::new();
    let mut task_repo = String::new();
    let mut item_branch = String::new();
    if task != "-" {
        task_repo = tgt(root, slug, "repo")?;
        item_branch = tgt(root, slug, "branch")?;
        let program = program_field(root, "program").unwrap_or_default();
        task_branch = format!("ai/{program}-{slug}-{task}-{}", minted.id);
        task_worktree = root.join("worktrees").join(&minted.id);
        fs::create_dir_all(root.join("worktrees"))?;
        if !git_quiet(
            &task_repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                &task_branch,
                &task_worktree.display().to_string(),
                &wid,
            ],
        ) {
            return Err(message(format!(
                "could not create isolated worktree for {task}"
            )));
        }
    }
    let deadline = now.saturating_add(seconds);
    let kind = kind_of(root, agent)?;
    let row = format!(
        "{}\t{slug}\t{task}\t{wid}\t{role}\t{agent}\t{kind}\t{criterion}\t{class}\tDISPATCHED\t{now}\t{deadline}\t{retry}",
        minted.id
    );
    write_ledger(&minted.staging, &row, now)?;
    if task != "-" {
        let paths = task_field(root, slug, task, 3)?;
        let verify = task_field(root, slug, task, 4)?;
        fs::write(
            minted.staging.join("task.tsv"),
            format!(
                "repo\tbranch\tworktree\titem_branch\tpaths_file\tverify_script\n{task_repo}\t{task_branch}\t{}\t{item_branch}\t{paths}\t{verify}\n",
                task_worktree.display()
            ),
        )?;
    }
    let self_cmd = if task == "-" {
        self_path(root)
    } else {
        format!("{}/crucible", root.display())
    };
    let relation = review_relation(root, slug, agent)?;
    let contract = managed_contract(
        &ManagedContract {
            id: &minted.id,
            role,
            slug,
            task,
            wid: &wid,
            criterion,
            class,
            self_cmd: &self_cmd,
            role_text: &fs::read_to_string(&role_path)?,
            item_text: &item_text,
            relation: &relation,
            task_worktree: &task_worktree.display().to_string(),
            task_branch: &task_branch,
            repo: &tgt(root, slug, "repo")?,
            branch: &tgt(root, slug, "branch")?,
            base: &tgt(root, slug, "base")?,
            paths_file: &if task == "-" {
                String::new()
            } else {
                task_field(root, slug, task, 3)?
            },
            verify_script: &if task == "-" {
                String::new()
            } else {
                task_field(root, slug, task, 4)?
            },
            agent,
            dir: &dir,
            rules: &fs::read_to_string(root.join("RULES.md")).unwrap_or_default(),
            rules_missing: !root.join("RULES.md").is_file(),
        },
        root,
    )?;
    fs::write(minted.staging.join("contract.md"), contract)?;
    fs::rename(&minted.staging, &minted.target)?;
    guard.staging = None;
    if role == "maker" {
        record_maker(root, slug, agent)?;
    }
    let risk = state_value(root, slug, 5)?.unwrap_or_default();
    if task != "-" {
        let item_wid = workid(root, slug)?;
        state_update_item(
            root, clock, slug, "ACTIVE", &stage, &item_wid, &risk, "TASKS", "-",
        )?;
    } else {
        state_update_item(
            root, clock, slug, "ACTIVE", &stage, &wid, &risk, &minted.id, "-",
        )?;
    }
    let stdout = format!("{}/contract.md\n", minted.target.display());
    eprint_run(
        root,
        agent,
        role,
        &minted.target.join("contract.md"),
        None,
        false,
    );
    guard.armed = false;
    let _ = fs::remove_dir(&lock);
    Ok(stdout)
}

fn arg_or<'a>(args: &'a [&'a str], index: usize, default: &'a str) -> &'a str {
    match args.get(index).copied() {
        Some(value) if !value.is_empty() => value,
        _ => default,
    }
}

fn token_ok(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn declares_criterion(text: &str, criterion: &str) -> bool {
    records(text).iter().any(|line| {
        let Some(rest) = checkbox_body(line) else {
            return false;
        };
        rest.split([':', '@', ' ']).next().unwrap_or("") == criterion
    })
}

fn checkbox_body(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    if bytes.len() < 6 || &bytes[..3] != b"- [" {
        return None;
    }
    if !matches!(bytes[3], b' ' | b'x' | b'X') || bytes[4] != b']' || bytes[5] != b' ' {
        return None;
    }
    Some(&line[6..])
}

fn plan_audit_pass(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    records(&text)
        .iter()
        .any(|line| line.starts_with("VERDICT: PASS"))
}

fn env_or(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(value) if !value.is_empty() => value,
        _ => default.to_string(),
    }
}

/// POSIX `$(( ))` on a digit string. A leading zero selects octal; 8 and 9 are not digits there.
fn shell_arith(value: &str) -> Result<i64, GuidedError> {
    let bytes = value.as_bytes();
    if bytes.first() == Some(&b'0') {
        let mut n: i64 = 0;
        for &byte in bytes {
            if !matches!(byte, b'0'..=b'7') {
                return Err(message(format!(
                    "{value}: value too great for base (error token is \"{value}\")"
                )));
            }
            n = n
                .checked_mul(8)
                .and_then(|n| n.checked_add(i64::from(byte - b'0')))
                .ok_or_else(|| {
                    message(format!(
                        "{value}: value too great for base (error token is \"{value}\")"
                    ))
                })?;
        }
        return Ok(n);
    }
    value
        .parse()
        .map_err(|_| message("attempt budget must be a positive integer"))
}

pub(crate) fn git_quiet(repo: &str, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(crate) fn git_rev12(repo: &str, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output();
    let Ok(out) = out else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(12)
        .collect()
}

struct ManagedContract<'a> {
    id: &'a str,
    role: &'a str,
    slug: &'a str,
    task: &'a str,
    wid: &'a str,
    criterion: &'a str,
    class: &'a str,
    self_cmd: &'a str,
    role_text: &'a str,
    item_text: &'a str,
    relation: &'a str,
    task_worktree: &'a str,
    task_branch: &'a str,
    repo: &'a str,
    branch: &'a str,
    base: &'a str,
    paths_file: &'a str,
    verify_script: &'a str,
    agent: &'a str,
    dir: &'a Path,
    rules: &'a str,
    rules_missing: bool,
}

fn managed_contract(c: &ManagedContract<'_>, root: &Path) -> Result<String, GuidedError> {
    let mut body = String::new();
    body.push_str(&format!("# Attempt {} — {} — {}\n\n", c.id, c.role, c.slug));
    body.push_str(&format!(
        "This attempt is bound to item `{}`, task `{}`, work `{}`, criterion `{}`, and evidence class `{}`.\n\n",
        c.slug, c.task, c.wid, c.criterion, c.class
    ));
    body.push_str("Read this file and follow it exactly. Do not accept coach notes outside this contract.\n\n");
    body.push_str("## Independence seal (before start)\n\n");
    body.push_str("While DISPATCHED, record transport and a contract-audit PASS:\n\n");
    body.push_str(&format!(
        "    {} attempt transport {} multi-agent|acp|subagent\n",
        c.self_cmd, c.id
    ));
    body.push_str(&format!(
        "    {} contract-audit {} <contract-auditor> PASS\n\n",
        c.self_cmd, c.id
    ));
    body.push_str("Then record the observed PID:\n\n");
    body.push_str(&format!(
        "    {} attempt start {} <PID>\n\n",
        c.self_cmd, c.id
    ));
    body.push_str("Record commands through:\n\n");
    body.push_str(&format!(
        "    {} run {} {} -- <command>\n\n",
        c.self_cmd, c.slug, c.agent
    ));
    body.push_str("When the process exits, record RETURNED, TIMEOUT, STOPPED, or ABANDONED:\n\n");
    body.push_str(&format!(
        "    {} attempt finish {} <STATE> \"<observation>\"\n\n",
        c.self_cmd, c.id
    ));
    body.push_str("After RETURNED, record exactly one result:\n\n");
    body.push_str(&format!(
        "    {} result {} <OUTCOME> <EVIDENCE-FILE> <NEXT> [FINGERPRINT]\n\n",
        c.self_cmd, c.id
    ));
    body.push_str("## Role\n\n");
    body.push_str(&from_heading(c.role_text, "## Instructions"));
    body.push_str("\n## Item\n\n");
    body.push_str(&ensure_nl(c.item_text));
    if c.role == "maker" {
        body.push_str("\n## Work location\n\n");
        if c.task != "-" {
            body.push_str(&format!(
                "Work only in isolated worktree `{}` on branch `{}`.\n\n",
                c.task_worktree, c.task_branch
            ));
            body.push_str("Owned paths:\n\n");
            let paths = fs::read_to_string(c.dir.join(c.paths_file))?;
            for owned in records(&paths) {
                body.push_str(&format!("- `{owned}`\n"));
            }
            body.push_str("\nBefore returning, commit your changes and run the task verifier through `crucible run`:\n\n");
            body.push_str(&format!(
                "    {} run {} {} -- {}/{} <task-commit>\n",
                c.self_cmd,
                c.slug,
                c.agent,
                c.dir.display(),
                c.verify_script
            ));
        } else if c.dir.join("TARGET").is_file() {
            body.push_str(&format!(
                "Repository: `{}`\n\nBranch: `{}`\n\nBase: `{}`\n",
                c.repo, c.branch, c.base
            ));
        } else {
            body.push_str(&format!("Write only under `{}/work/`.\n", c.dir.display()));
        }
        body.push_str("\n## Maker inner loop\n\n");
        body.push_str(
            "Run the item falsifier until it passes, or stop after one infrastructure retry.\n",
        );
        body.push_str("Do not admit the next backlog row. Do not merge.\n");
    } else {
        body.push_str(&format!(
            "\n## Review independence\n\nRelation to recorded maker families: `{}`.\n",
            c.relation
        ));
        body.push_str(&format!("\n## Work — work id {}\n\n", c.wid));
        body.push_str(&emit_work(root, c.slug, c.wid)?);
        body.push_str("\n## Recorded evidence\n\n");
        body.push_str(&evidence_block(&c.dir.join("evidence"))?);
    }
    body.push_str("\n## Rules\n\n");
    if c.rules_missing {
        // Shell `cat || true` leaves this section empty when RULES.md is absent.
    } else {
        body.push_str(&ensure_nl(c.rules));
    }
    Ok(body)
}

pub(crate) fn evidence_block(dir: &Path) -> Result<String, GuidedError> {
    let files = list_files(dir);
    if files.is_empty() {
        return Ok("(none. Absence of evidence is a finding, never a pass.)\n".to_string());
    }
    let mut out = String::new();
    for path in files {
        let name = file_name(&path);
        out.push_str(&format!("### {name}\n```\n"));
        let bytes = fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes);
        push_cat(&mut out, &text);
        out.push_str("```\n");
    }
    Ok(out)
}

/// `cat` of an empty file adds nothing. A non-empty body gets a newline only if it lacks one.
fn push_cat(out: &mut String, text: &str) {
    out.push_str(text);
    if !text.is_empty() && !text.ends_with('\n') {
        out.push('\n');
    }
}

pub(crate) fn emit_work(root: &Path, slug: &str, wid: &str) -> Result<String, GuidedError> {
    let dir = item_dir(root, slug);
    if dir.join("TARGET").is_file() {
        let repo = tgt(root, slug, "repo")?;
        let branch = tgt(root, slug, "branch")?;
        let base = tgt(root, slug, "base")?;
        let mut out =
            format!("Repository: {repo}\nBranch: {branch}  Base: {base}  Tree: {wid}\n\n");
        out.push_str("### commits\n```\n");
        let log = git_capture(&repo, &["log", "--oneline", &format!("{base}..{branch}")]);
        if log.is_none() {
            out.push_str("(none)\n");
        } else {
            out.push_str(&ensure_nl(&log.unwrap_or_default()));
        }
        out.push_str("```\n### files changed\n```\n");
        let stat = git_required(&repo, &["diff", "--stat", &format!("{base}..{branch}")])?;
        out.push_str(&ensure_nl(&stat));
        out.push_str("```\n### diff\n```diff\n");
        let diff = git_required(&repo, &["diff", "-U10", &format!("{base}..{branch}")])?;
        out.push_str(&ensure_nl(&diff));
        out.push_str("```\n");
        return Ok(out);
    }
    let mut out = String::new();
    for path in list_files(&dir.join("work")) {
        let rel = path.strip_prefix(&dir).unwrap_or(&path);
        out.push_str(&format!("### {}\n```\n", rel.display()));
        let bytes = fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes);
        push_cat(&mut out, &text);
        out.push_str("```\n");
    }
    Ok(out)
}

fn git_required(repo: &str, args: &[&str]) -> Result<String, GuidedError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stderr(Stdio::piped())
        .output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        if err.is_empty() {
            return Err(message(format!(
                "git {} failed",
                args.first().copied().unwrap_or("diff")
            )));
        }
        return Err(message(err.to_string()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_capture(repo: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn dispatch_legacy(root: &Path, slug: &str, role: &str, name: &str) -> Result<String, GuidedError> {
    let dir = need(root, slug)?;
    require_panel_approval(root)?;
    let role_path = root.join("roles").join(format!("{role}.md"));
    if !role_path.is_file() {
        return Err(message(format!(
            "no such role: {role} (see {}/roles/)",
            root.display()
        )));
    }
    require_registered(root, name)?;
    let wid = workid(root, slug)?;
    let self_cmd = self_path(root);
    let phase = phase_of(root, slug)?;
    let role_text = fs::read_to_string(&role_path)?;
    let maker = fs::read_to_string(dir.join("MAKER"))
        .ok()
        .and_then(|text| records(&text).first().copied().map(str::to_string))
        .unwrap_or_default();
    if matches!(role, "judge" | "adversary") {
        if matches!(wid.as_str(), "EMPTY" | "NOBRANCH") {
            return Err(message("refused: no work yet, nothing to judge"));
        }
        if name == maker {
            return Err(message(format!("refused: {name} is the maker of {slug}")));
        }
        archive_verdict(root, &dir, name, &wid)?;
    } else if role == "maker" {
        fs::write(dir.join("MAKER"), format!("{name}\n"))?;
    }
    let dispatches = dir.join("dispatches");
    fs::create_dir_all(&dispatches)?;
    let mut n = 1u64;
    let out = loop {
        let path = dispatches.join(format!("{n}-{role}-{name}.md"));
        if !path.exists() {
            break path;
        }
        n += 1;
    };
    let mut body = String::new();
    push(&mut body, &format!("# Dispatch {n} — {role} — {name}"));
    push(&mut body, "");
    push(
        &mut body,
        &format!(
            "You are **{name}**, acting as **{role}**, on item **{slug}**, phase **{phase}**."
        ),
    );
    push(
        &mut body,
        "This file is your complete task. Read it and follow it exactly. Do not ask for more",
    );
    push(
        &mut body,
        "context: if this file is insufficient, return NEEDS_CONTEXT naming what is missing.",
    );
    push(&mut body, "");
    for (label, key) in [
        ("Purpose right now", "purpose"),
        ("You may read", "may-read"),
        ("You must not read", "must-not-read"),
        ("You must write", "must-write"),
        ("You may dispatch", "may-call"),
        ("You must return", "return"),
        ("How to verify your own work", "verify"),
    ] {
        push(
            &mut body,
            &format!("- **{label}:** {}", role_field(&role_text, key)),
        );
    }
    push(&mut body, "");
    push(
        &mut body,
        "Record every command you run, and name at least one of those files in what you write:",
    );
    push(&mut body, "");
    push(
        &mut body,
        &format!("    {self_cmd} run {slug} {name} -- <command>"),
    );
    push(&mut body, "");
    push(
        &mut body,
        "Hand-written evidence is refused. Check what still blocks this item with",
    );
    push(
        &mut body,
        &format!("`{self_cmd} check {slug}`. Current work id: **{wid}**."),
    );
    if dir.join("TARGET").is_file() {
        push(&mut body, "");
        push(
            &mut body,
            &format!(
                "**Where the work lives:** repository `{}`, branch",
                tgt(root, slug, "repo")?
            ),
        );
        push(
            &mut body,
            &format!(
                "`{}`, branched from `{}`. Commit there.",
                tgt(root, slug, "branch")?,
                tgt(root, slug, "base")?
            ),
        );
        push(
            &mut body,
            "Do not push, do not merge, do not touch any other branch. The work id above is that",
        );
        push(
            &mut body,
            "branch's tree, so every commit changes it and voids any verdict already given.",
        );
        push(&mut body, "");
        push(
            &mut body,
            "**Order matters: commit first, then record your evidence.** Evidence is stamped with the",
        );
        push(
            &mut body,
            "branch tree at the moment it runs, so anything you record before your final commit goes",
        );
        push(
            &mut body,
            "stale the instant you commit. Finish the code, commit, then run your checks.",
        );
    } else {
        push(&mut body, "");
        push(
            &mut body,
            &format!(
                "**Where the work lives:** `{}/work/`. No symlinks, no newlines in filenames.",
                dir.display()
            ),
        );
    }
    if matches!(role, "judge" | "adversary") {
        push(&mut body, "");
        push(
            &mut body,
            &format!(
                "Write your verdict to `{}/verdicts/{name}.md`. First two lines exactly:",
                dir.display()
            ),
        );
        push(&mut body, "");
        push(&mut body, "    VERDICT: PASS");
        push(&mut body, &format!("    WORK-ID: {wid}"));
        push(&mut body, "");
        push(
            &mut body,
            "substituting REJECT, INSUFFICIENT_EVIDENCE or SCOPE_CONFLICT as warranted.",
        );
    }
    push(&mut body, "");
    push(&mut body, "---");
    push(&mut body, "");
    push(&mut body, "## Your role in full");
    push(&mut body, "");
    body.push_str(&from_heading(&role_text, "## Instructions"));
    push(&mut body, "");
    push(&mut body, "---");
    push(&mut body, "");
    push(&mut body, "## The rules that bind you");
    push(&mut body, "");
    match fs::read_to_string(root.join("RULES.md")) {
        Ok(rules) => body.push_str(&ensure_nl(&rules)),
        Err(_) => push(&mut body, "(RULES.md missing)"),
    }
    push(&mut body, "");
    push(&mut body, "---");
    push(&mut body, "");
    push(&mut body, "## The item");
    push(&mut body, "");
    body.push_str(&ensure_nl(&fs::read_to_string(dir.join("ITEM.md"))?));
    for extra in ["DESIGN.md", "TASKS.md"] {
        let path = dir.join(extra);
        if path.is_file() {
            push(&mut body, "");
            push(&mut body, &format!("## {extra}"));
            push(&mut body, "");
            body.push_str(&ensure_nl(&fs::read_to_string(path)?));
        }
    }
    if !matches!(role, "judge" | "adversary") {
        if let Ok(lessons) = fs::read_to_string(root.join("LESSONS.md")) {
            if !lessons.is_empty() {
                push(&mut body, "");
                push(&mut body, "## Lessons from earlier items — these bind you");
                push(&mut body, "");
                body.push_str(&ensure_nl(&lessons));
            }
        }
    }
    if matches!(role, "judge" | "adversary") {
        push(&mut body, "");
        push(&mut body, &format!("## The work — work id {wid}"));
        push(&mut body, "");
        body.push_str(&emit_work(root, slug, &wid)?);
        push(&mut body, "");
        push(&mut body, "## Recorded evidence");
        push(&mut body, "");
        body.push_str(&evidence_block(&dir.join("evidence"))?);
    }
    fs::write(&out, body)?;
    eprint_run(root, name, role, &out, None, true);
    Ok(format!("{}\n", out.display()))
}

fn archive_verdict(root: &Path, dir: &Path, name: &str, wid: &str) -> Result<(), GuidedError> {
    let old = dir.join("verdicts").join(format!("{name}.md"));
    if !old.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&old)?;
    let ow = result_field(&text, "WORK-ID");
    if ow.is_empty() || ow == wid {
        return Ok(());
    }
    let history = dir.join("verdicts").join("history");
    fs::create_dir_all(&history)?;
    let mut dest = history.join(format!("{ow}-{name}.md"));
    let mut n2 = 1u64;
    while dest.exists() {
        dest = history.join(format!("{ow}-{name}.{n2}.md"));
        n2 += 1;
    }
    fs::rename(&old, &dest)?;
    eprintln!(
        "archived the superseded verdict: {}",
        rel_under(root, &dest)
    );
    Ok(())
}

pub fn validate_managed_item(root: &Path, slug: &str) -> Result<(), GuidedError> {
    let text = fs::read_to_string(item_dir(root, slug).join("ITEM.md"))?;
    let expected = [
        "## Goal",
        "## Non-goals",
        "## Risk",
        "## Owned files",
        "## Acceptance criteria",
        "## Focused falsifier",
        "## Expensive evidence",
        "## Stop conditions",
    ];
    let actual: Vec<&str> = records(&text)
        .into_iter()
        .filter(|line| line.starts_with("## "))
        .collect();
    if actual != expected {
        return Err(message(
            "refused: ITEM.md sections are missing, duplicated, or out of order",
        ));
    }
    let risk = section_lines(&text, "Risk").join("\n");
    if !matches!(risk.as_str(), "LOW" | "MEDIUM" | "HIGH") {
        return Err(message(
            "refused: risk must be exactly LOW, MEDIUM, or HIGH",
        ));
    }
    if let Some(err) = owned_error(&section_lines(&text, "Owned files")) {
        return Err(message(format!("refused: {err}")));
    }
    let criteria = section_lines(&text, "Acceptance criteria")
        .into_iter()
        .filter(|line| is_criterion_line(line))
        .count();
    if !(1..=3).contains(&criteria) {
        return Err(message(
            "refused: need one to three A1-A3 acceptance criteria",
        ));
    }
    let falsifier = section_lines(&text, "Focused falsifier");
    if falsifier.is_empty() {
        return Err(message("refused: focused falsifier is empty"));
    }
    if falsifier
        .join("\n")
        .contains("TEMPLATE-FALSIFIER-UNWRITTEN")
    {
        return Err(message("refused: focused falsifier is not written"));
    }
    if falsifier.len() != 1 {
        return Err(message(
            "refused: focused falsifier must be one bounded command or script",
        ));
    }
    Ok(())
}

pub(crate) fn section_lines(text: &str, heading: &str) -> Vec<String> {
    let want = format!("## {heading}");
    let mut on = false;
    let mut lines = Vec::new();
    for line in records(text) {
        if !on {
            if line == want {
                on = true;
            }
            continue;
        }
        if line.starts_with("## ") {
            break;
        }
        if line.bytes().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }
        lines.push(line.to_string());
    }
    lines
}

fn owned_error(lines: &[String]) -> Option<String> {
    let mut count = 0usize;
    let mut errs = Vec::new();
    for line in lines {
        if !line.starts_with("- ") {
            errs.push("owned paths must be list items".to_string());
            break;
        }
        let path = &line[2..];
        if path.is_empty() || unsafe_owned(path) {
            errs.push(format!("unsafe owned path {path}"));
            break;
        }
        count += 1;
    }
    if count == 0 {
        errs.push("owned files are empty".to_string());
    }
    if errs.is_empty() {
        None
    } else {
        Some(errs.join("\n"))
    }
}

fn unsafe_owned(path: &str) -> bool {
    path.is_empty()
        || path.starts_with('/')
        || path.contains("//")
        || path.split('/').any(|seg| seg == ".." || seg == ".")
        || path.contains('*')
        || path.contains('?')
        || path.contains('[')
}

fn is_criterion_line(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.len() < 8 || &bytes[..3] != b"- [" {
        return false;
    }
    if !matches!(bytes[3], b' ' | b'x' | b'X') || bytes[4] != b']' || bytes[5] != b' ' {
        return false;
    }
    if bytes[6] != b'A' || !matches!(bytes[7], b'1' | b'2' | b'3') {
        return false;
    }
    match bytes.get(8).copied() {
        None => true,
        Some(b':' | b'@' | b' ') => true,
        Some(_) => false,
    }
}

pub(crate) fn validate_task_dag(root: &Path, slug: &str) -> Result<(), GuidedError> {
    let dir = item_dir(root, slug);
    let tasks_path = dir.join("TASKS.tsv");
    if !is_regular(&tasks_path) {
        return Err(message("invalid TASKS.tsv: missing regular file"));
    }
    let text = fs::read_to_string(&tasks_path)?;
    let rows = records(&text);
    if rows.first().copied() != Some(TASK_HEADER) {
        return Err(message("invalid TASKS.tsv: header mismatch"));
    }
    let mut tasks = Vec::new();
    for (idx, rec) in rows.iter().enumerate().skip(1) {
        let row_no = idx + 1;
        let fields = split_tabs(rec);
        if fields.len() != 4 {
            return Err(message(format!(
                "invalid TASKS.tsv: row {row_no} has {} fields, need 4",
                fields.len()
            )));
        }
        let id = fields[0];
        if !task_id_ok(id) {
            return Err(message(format!(
                "invalid TASKS.tsv: invalid task id at row {row_no}"
            )));
        }
        if tasks.iter().any(|row: &TaskRow| row.id == id) {
            return Err(message(format!(
                "invalid TASKS.tsv: duplicate task id {id}"
            )));
        }
        if !deps_ok(fields[1]) {
            return Err(message(format!(
                "invalid TASKS.tsv: invalid dependencies for {id}"
            )));
        }
        if !paths_ok(fields[2]) {
            return Err(message(format!(
                "invalid TASKS.tsv: invalid paths file for {id}"
            )));
        }
        if !verify_ok(fields[3]) {
            return Err(message(format!(
                "invalid TASKS.tsv: invalid verify script for {id}"
            )));
        }
        tasks.push(TaskRow {
            id: id.to_string(),
            deps: fields[1].to_string(),
            paths_file: fields[2].to_string(),
            verify_script: fields[3].to_string(),
        });
    }
    if tasks.is_empty() {
        return Err(message("invalid TASKS.tsv: no tasks"));
    }
    if tasks.len() > 32 {
        return Err(message("invalid TASKS.tsv: more than 32 tasks"));
    }
    if let Some(err) = graph_error(&tasks) {
        return Err(message(format!("invalid TASKS.tsv: {err}")));
    }
    for task in &tasks {
        let paths = dir.join(&task.paths_file);
        let verify = dir.join(&task.verify_script);
        if !is_regular(&paths) {
            return Err(message(format!(
                "invalid task {}: missing regular {}",
                task.id, task.paths_file
            )));
        }
        if fs::metadata(&paths)
            .map(|meta| meta.len() == 0)
            .unwrap_or(true)
        {
            return Err(message(format!(
                "invalid task {}: empty {}",
                task.id, task.paths_file
            )));
        }
        if !is_regular(&verify) {
            return Err(message(format!(
                "invalid task {}: missing regular {}",
                task.id, task.verify_script
            )));
        }
        if !is_executable(&verify) {
            return Err(message(format!(
                "invalid task {}: {} is not executable",
                task.id, task.verify_script
            )));
        }
        let shebang = fs::read_to_string(&verify)
            .ok()
            .and_then(|text| records(&text).first().copied().map(str::to_string))
            .unwrap_or_default();
        if shebang != "#!/bin/sh" {
            return Err(message(format!(
                "invalid task {}: {} must start with #!/bin/sh",
                task.id, task.verify_script
            )));
        }
    }
    let mut owned = Vec::new();
    for task in &tasks {
        let text = fs::read_to_string(dir.join(&task.paths_file))?;
        for line in records(&text) {
            // The shell joins task and path with a tab, so a tab inside the path makes NF != 2.
            if line.is_empty() || line.contains('\t') {
                return Err(message("invalid TASKS.tsv: blank or malformed owned path"));
            }
            owned.push((task.id.clone(), line.to_string()));
        }
    }
    if let Some(err) = overlap_error(&owned) {
        return Err(message(format!("invalid TASKS.tsv: {err}")));
    }
    // `sed 's/^- //'` keeps a non-list line as itself; it does not drop it.
    let item_owned = section_lines(&fs::read_to_string(dir.join("ITEM.md"))?, "Owned files")
        .into_iter()
        .map(|line| line.strip_prefix("- ").map(str::to_string).unwrap_or(line))
        .collect::<Vec<_>>();
    for (task, path) in &owned {
        let allowed = item_owned
            .iter()
            .any(|item| path == item || path.starts_with(&format!("{item}/")));
        if !allowed {
            return Err(message(format!(
                "invalid TASKS.tsv: task {task} owns undeclared item path {path}"
            )));
        }
    }
    Ok(())
}

struct TaskRow {
    id: String,
    deps: String,
    paths_file: String,
    verify_script: String,
}

fn task_id_ok(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() >= 2
        && bytes[0] == b'T'
        && (b'1'..=b'9').contains(&bytes[1])
        && bytes[2..].iter().all(|b| b.is_ascii_digit())
}

fn deps_ok(deps: &str) -> bool {
    if deps == "-" {
        return true;
    }
    !deps.is_empty() && deps.split(',').all(task_id_ok)
}

fn paths_ok(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("tasks/") else {
        return false;
    };
    let Some(stem) = rest.strip_suffix(".paths") else {
        return false;
    };
    !stem.is_empty()
        && !stem.contains('/')
        && stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn verify_ok(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("tasks/") else {
        return false;
    };
    let Some(stem) = rest.strip_suffix(".verify.sh") else {
        return false;
    };
    !stem.is_empty()
        && !stem.contains('/')
        && stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn graph_error(tasks: &[TaskRow]) -> Option<String> {
    let ids: Vec<&str> = tasks.iter().map(|task| task.id.as_str()).collect();
    let mut edges = 0usize;
    for task in tasks {
        if task.deps == "-" {
            continue;
        }
        for dep in task.deps.split(',') {
            edges += 1;
            if !ids.contains(&dep) {
                return Some(format!("unknown dependency {dep} for {}", task.id));
            }
            if dep == task.id {
                return Some(format!("dependency cycle at {}", task.id));
            }
        }
    }
    if edges > 128 {
        return Some("more than 128 dependency edges".to_string());
    }
    let mut done = vec![false; tasks.len()];
    let mut completed = 0usize;
    for _ in 0..tasks.len() {
        let mut progressed = false;
        for (idx, task) in tasks.iter().enumerate() {
            if done[idx] {
                continue;
            }
            let ready = task.deps == "-"
                || task.deps.split(',').all(|dep| {
                    ids.iter()
                        .position(|id| *id == dep)
                        .is_some_and(|at| done[at])
                });
            if ready {
                done[idx] = true;
                completed += 1;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    if completed != tasks.len() {
        Some("dependency cycle".to_string())
    } else {
        None
    }
}

fn overlap_error(owned: &[(String, String)]) -> Option<String> {
    for (idx, (task, path)) in owned.iter().enumerate() {
        if path.is_empty() {
            return Some("blank or malformed owned path".to_string());
        }
        if unsafe_owned(path) {
            return Some(format!("unsafe owned path {path} for {task}"));
        }
        for (prev_task, prev) in owned.iter().take(idx) {
            if prev == path
                || prev.starts_with(&format!("{path}/"))
                || path.starts_with(&format!("{prev}/"))
            {
                return Some(format!(
                    "ownership overlap {prev} ({prev_task}) and {path} ({task})"
                ));
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    Command::new("test")
        .arg("-x")
        .arg(path)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(crate) fn task_contract_id(root: &Path, slug: &str) -> Result<String, GuidedError> {
    let dir = item_dir(root, slug);
    let tasks = dir.join("TASKS.tsv");
    let mut manifest = format!("TASKS.tsv {}\n", hash_file(&tasks)?);
    let text = fs::read_to_string(&tasks)?;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields = split_tabs(rec);
        if fields.len() < 4 {
            continue;
        }
        let paths = dir.join(fields[2]);
        let verify = dir.join(fields[3]);
        manifest.push_str(&format!(
            "{} {} {} {}\n",
            fields[0],
            fields[1],
            fields[2],
            hash_file(&paths)?
        ));
        let mode = fs::symlink_metadata(&verify)
            .map(|meta| ls_mode(meta.mode()))
            .unwrap_or_else(|_| "----------".to_string());
        manifest.push_str(&format!("{} {} {}\n", fields[3], mode, hash_file(&verify)?));
    }
    Ok(h12(manifest.as_bytes()))
}

pub(crate) fn task_assert_frozen(root: &Path, slug: &str) -> Result<(), GuidedError> {
    let path = item_dir(root, slug).join("TASKS.id");
    if !is_regular(&path) {
        return Err(message(format!("task DAG is not frozen: {slug}")));
    }
    let frozen = fs::read_to_string(&path)?;
    let frozen = records(&frozen).first().copied().unwrap_or("");
    let current = task_contract_id(root, slug)?;
    if frozen != current {
        return Err(message(format!(
            "task DAG changed after READY: {frozen} -> {current}"
        )));
    }
    Ok(())
}

pub(crate) fn task_pass_exists(root: &Path, slug: &str, task: &str) -> Result<bool, GuidedError> {
    for path in attempt_child_dirs(root) {
        let result = path.join("result.md");
        if !result.is_file() {
            continue;
        }
        let Ok(text) = fs::read_to_string(&result) else {
            continue;
        };
        if result_field(&text, "OUTCOME") == "PASS"
            && result_field(&text, "ITEM") == slug
            && result_field(&text, "TASK-ID") == task
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn task_field(
    root: &Path,
    slug: &str,
    task: &str,
    column: usize,
) -> Result<String, GuidedError> {
    let text = fs::read_to_string(item_dir(root, slug).join("TASKS.tsv")).unwrap_or_default();
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields = split_tabs(rec);
        if fields.first().copied() == Some(task) {
            return Ok(fields
                .get(column.saturating_sub(1))
                .copied()
                .unwrap_or("")
                .to_string());
        }
    }
    Ok(String::new())
}

pub(crate) fn task_result_file(
    root: &Path,
    slug: &str,
    task: &str,
) -> Result<Option<PathBuf>, GuidedError> {
    let mut found = None;
    for path in attempt_child_dirs(root) {
        let result = path.join("result.md");
        if !result.is_file() {
            continue;
        }
        let Ok(text) = fs::read_to_string(&result) else {
            continue;
        };
        if result_field(&text, "OUTCOME") == "PASS"
            && result_field(&text, "ITEM") == slug
            && result_field(&text, "TASK-ID") == task
        {
            found = Some(result);
        }
    }
    Ok(found)
}

pub(crate) fn task_live_attempt(
    root: &Path,
    slug: &str,
    task: &str,
) -> Result<Option<String>, GuidedError> {
    for path in attempt_child_dirs(root) {
        let id = file_name(&path);
        if attempt_meta(root, &id, 2).ok().as_deref() != Some(slug) {
            continue;
        }
        if attempt_meta(root, &id, 3).ok().as_deref() != Some(task) {
            continue;
        }
        let Ok(status) = attempt_state(root, &id) else {
            continue;
        };
        if !attempt_terminal(&status) || (status == "RETURNED" && !path.join("result.md").is_file())
        {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

pub(crate) fn task_retry_available(
    root: &Path,
    slug: &str,
    task: &str,
) -> Result<Option<String>, GuidedError> {
    for path in attempt_child_dirs(root) {
        let id = file_name(&path);
        if attempt_meta(root, &id, 2).ok().as_deref() != Some(slug) {
            continue;
        }
        if attempt_meta(root, &id, 3).ok().as_deref() != Some(task) {
            continue;
        }
        if attempt_meta(root, &id, 13).ok().as_deref() != Some("-") {
            continue;
        }
        if attempt_state(root, &id).ok().as_deref() != Some("TIMEOUT") {
            continue;
        }
        let mut retried = false;
        for child in attempt_child_dirs(root) {
            let child_id = file_name(&child);
            if attempt_meta(root, &child_id, 13).ok().as_deref() == Some(id.as_str()) {
                retried = true;
                break;
            }
        }
        if !retried {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

pub(crate) fn task_live_count(root: &Path, slug: &str) -> Result<u64, GuidedError> {
    if !root.join("attempts").is_dir() {
        return Ok(0);
    }
    let mut count = 0u64;
    for path in attempt_child_dirs(root) {
        let id = file_name(&path);
        // A stray directory whose ledger cannot be read is skipped, as the shell's `|| continue` does.
        let Ok(item) = attempt_meta(root, &id, 2) else {
            continue;
        };
        if item != slug {
            continue;
        }
        let Ok(task) = attempt_meta(root, &id, 3) else {
            continue;
        };
        if task == "-" {
            continue;
        }
        let Ok(status) = attempt_state(root, &id) else {
            continue;
        };
        if !attempt_terminal(&status) || (status == "RETURNED" && !path.join("result.md").is_file())
        {
            count += 1;
        }
    }
    Ok(count)
}

pub fn task_attempt_for_run(
    root: &Path,
    slug: &str,
    agent: &str,
    cwd: &Path,
) -> Result<Option<String>, GuidedError> {
    let cwd = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    for path in attempt_child_dirs(root) {
        let task_tsv = path.join("task.tsv");
        if !task_tsv.is_file() {
            continue;
        }
        let id = file_name(&path);
        if attempt_meta(root, &id, 2).ok().as_deref() != Some(slug) {
            continue;
        }
        if attempt_meta(root, &id, 6).ok().as_deref() != Some(agent) {
            continue;
        }
        if attempt_state(root, &id)
            .ok()
            .is_some_and(|status| attempt_terminal(&status))
        {
            continue;
        }
        let text = fs::read_to_string(&task_tsv).unwrap_or_default();
        let row = records(&text).get(1).copied().unwrap_or("");
        let recorded = split_tabs(row).get(2).copied().unwrap_or("");
        let Ok(recorded) = fs::canonicalize(recorded) else {
            continue;
        };
        if recorded == cwd {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

pub(crate) fn task_topological_order(root: &Path, slug: &str) -> Result<Vec<String>, GuidedError> {
    let text = fs::read_to_string(item_dir(root, slug).join("TASKS.tsv"))?;
    let mut order = Vec::new();
    let mut deps = Vec::new();
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields = split_tabs(rec);
        order.push(fields.first().copied().unwrap_or("").to_string());
        deps.push(fields.get(1).copied().unwrap_or("-").to_string());
    }
    let n = order.len();
    let mut done = vec![false; n];
    let mut emitted = Vec::new();
    for _ in 0..n {
        for i in 0..n {
            if done[i] {
                continue;
            }
            let ready = deps[i] == "-"
                || deps[i].split(',').all(|dep| {
                    order
                        .iter()
                        .position(|id| id == dep)
                        .is_some_and(|at| done[at])
                });
            if ready {
                emitted.push(order[i].clone());
                done[i] = true;
            }
        }
    }
    if emitted.len() != n {
        return Err(message("invalid TASKS.tsv: dependency cycle"));
    }
    Ok(emitted)
}

pub fn task_all_pass(root: &Path, slug: &str) -> Result<bool, GuidedError> {
    for task in task_topological_order(root, slug)? {
        if !task_pass_exists(root, slug, &task)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn task_integration_complete(root: &Path, slug: &str) -> Result<bool, GuidedError> {
    let path = item_dir(root, slug).join("INTEGRATION.tsv");
    if !path.is_file() {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    let mut recorded = String::new();
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        recorded = split_tabs(rec).get(3).copied().unwrap_or("").to_string();
    }
    if recorded.is_empty() {
        return Ok(false);
    }
    Ok(recorded == workid(root, slug)?)
}

pub(crate) fn task_owns_path(
    root: &Path,
    slug: &str,
    task: &str,
    changed: &str,
) -> Result<bool, GuidedError> {
    let paths_file = task_field(root, slug, task, 3)?;
    let text = fs::read_to_string(item_dir(root, slug).join(paths_file)).unwrap_or_default();
    for owned in records(&text) {
        if changed == owned || changed.starts_with(&format!("{owned}/")) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn task_dependencies_pass(
    root: &Path,
    slug: &str,
    deps: &str,
) -> Result<bool, GuidedError> {
    if deps == "-" {
        return Ok(true);
    }
    for dependency in deps.split(',') {
        if !task_pass_exists(root, slug, dependency)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn task_render_file(root: &Path, slug: &str, out: &Path) -> Result<(), GuidedError> {
    let dir = item_dir(root, slug);
    let mut body = format!("# TASKS — {slug}\n\nGenerated from frozen `TASKS.tsv`; do not edit this file by hand.\n\n| Task | Dependencies | Owned paths | Verify script | Derived state |\n| --- | --- | --- | --- | --- |\n");
    let text = fs::read_to_string(dir.join("TASKS.tsv"))?;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields = split_tabs(rec);
        if fields.len() < 4 {
            continue;
        }
        let paths = fs::read_to_string(dir.join(fields[2])).unwrap_or_default();
        let mut owned = String::new();
        let mut first = true;
        for line in records(&paths) {
            if !first {
                owned.push_str("<br>");
            }
            first = false;
            owned.push('`');
            owned.push_str(line);
            owned.push('`');
        }
        let derived = if task_pass_exists(root, slug, fields[0])? {
            "PASS"
        } else if task_dependencies_pass(root, slug, fields[1])? {
            "READY"
        } else {
            "WAITING"
        };
        body.push_str(&format!(
            "| {} | {} | {owned} | `{}` | {derived} |\n",
            fields[0], fields[1], fields[3]
        ));
    }
    fs::write(out, body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::probe_acp;
    use crate::panel::panel_id;
    use crate::state::STATE_HEADER;
    use crucible_contract::FixedClock;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    const EPOCH: i64 = 1_700_000_123;

    struct Tmp(PathBuf);

    impl Tmp {
        fn new() -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "crucible-dispatch-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn approve(root: &Path) {
        let id = panel_id(root).unwrap().unwrap();
        fs::write(root.join("PANEL.APPROVAL"), format!("panel-id: {id}\n")).unwrap();
    }

    #[test]
    fn fixed_clock_predicts_claim_and_managed_attempt_ids() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let clock = FixedClock::new(EPOCH);
        let pid = std::process::id();
        fs::write(
            root.join("PROGRAM"),
            "program: work\nlifecycle: managed\ncycle: guided\n",
        )
        .unwrap();
        fs::write(root.join("PANEL.md"), "panel body\n").unwrap();
        fs::write(
            root.join("PANEL.ASSIGN.tsv"),
            "role\tagent\trequired\tnotes\nclaim-auditor\tbea\tyes\t-\nmaker\tada\tyes\t-\ncontract-auditor\tcy\tyes\t-\n",
        )
        .unwrap();
        fs::write(
            root.join("agents.tsv"),
            "ada\tgrok\tm\thigh\techo {BRIEF}\nbea\tcodex\tm\thigh\techo {BRIEF}\ncy\tkiro\tm\thigh\techo {BRIEF}\n",
        )
        .unwrap();
        approve(root);
        fs::create_dir_all(root.join("roles")).unwrap();
        fs::write(
            root.join("roles/claim-auditor.md"),
            "purpose: audit\nmay-read: code\nmust-not-read: verdicts\nreturn: TRUE\nverify: read\n\n## Instructions\nLook.\n",
        )
        .unwrap();
        fs::write(
            root.join("roles/maker.md"),
            "purpose: make\nmay-read: item\nmust-not-read: verdicts\nreturn: DONE\nverify: run\n\n## Instructions\nBuild.\n",
        )
        .unwrap();
        fs::write(
            root.join("CLAIMS.md"),
            "# CLAIMS\n\n### C1 the widget is absent\nbody\n",
        )
        .unwrap();
        fs::write(root.join("RULES.md"), "rules\n").unwrap();
        let claim_out = dispatch(root, &["C1", "claim-auditor", "bea"], &clock).unwrap();
        let claim_id = format!("A{EPOCH}.{pid}.1");
        assert!(
            claim_out.contains("claims/C1/dispatches/1-claim-auditor-bea.md"),
            "{claim_out}"
        );
        let meta =
            fs::read_to_string(root.join("attempts").join(&claim_id).join("meta.tsv")).unwrap();
        assert!(meta.contains(&format!(
            "{claim_id}\tC1\t-\tCLAIM\tclaim-auditor\tbea\tcodex\t-\tFOCUSED\tDISPATCHED\t{EPOCH}\t{}\t-\n",
            EPOCH + 1800
        )));
        let events =
            fs::read_to_string(root.join("attempts").join(&claim_id).join("events.tsv")).unwrap();
        assert!(events.contains(&format!("DISPATCHED\t{EPOCH}\t-\tdispatch-recorded\n")));
        let item = root.join("items/alpha");
        fs::create_dir_all(item.join("work")).unwrap();
        fs::write(item.join("ITEM.md"), "# alpha\n\n- [ ] A1 do the thing\n").unwrap();
        fs::write(item.join("plan-audit.md"), "VERDICT: PASS\nAUDITOR: cy\n").unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tBUILD\tEMPTY\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        let managed_out = dispatch(root, &["alpha", "maker", "ada"], &clock).unwrap();
        let managed_id = format!("A{EPOCH}.{pid}.2");
        assert_eq!(
            managed_out,
            format!(
                "{}/contract.md\n",
                root.join("attempts").join(&managed_id).display()
            )
        );
        let managed_meta =
            fs::read_to_string(root.join("attempts").join(&managed_id).join("meta.tsv")).unwrap();
        assert!(
            managed_meta.contains(&format!(
                "{managed_id}\talpha\t-\tEMPTY\tmaker\tada\tgrok\tA1\tFOCUSED\tDISPATCHED\t{EPOCH}\t{}\t-\n",
                EPOCH + 2700
            )),
            "{managed_meta}"
        );
        let probe = probe_acp(root, &["failed", "down"], &clock).unwrap();
        assert_eq!(probe, format!("{}/ACP-PROBE.md\n", root.display()));
        let summary = fs::read_to_string(root.join("ACP-PROBE.md")).unwrap();
        assert!(summary.contains(&format!("probed-epoch: {EPOCH}\n")));
    }

    #[test]
    fn task_dag_refuses_a_cycle_and_renders_a_frozen_table() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let item = root.join("items/alpha");
        fs::create_dir_all(item.join("tasks")).unwrap();
        fs::write(
            item.join("ITEM.md"),
            "\
## Goal
g
## Non-goals
n
## Risk
LOW
## Owned files
- src/a.rs
## Acceptance criteria
- [ ] A1
## Focused falsifier
echo ok
## Expensive evidence
NONE
## Stop conditions
stop
",
        )
        .unwrap();
        validate_managed_item(root, "alpha").unwrap();
        fs::write(
            item.join("TASKS.tsv"),
            "task_id\tdepends_on\tpaths_file\tverify_script\nT1\tT2\ttasks/a.paths\ttasks/a.verify.sh\nT2\tT1\ttasks/b.paths\ttasks/b.verify.sh\n",
        )
        .unwrap();
        fs::write(item.join("tasks/a.paths"), "src/a.rs\n").unwrap();
        fs::write(item.join("tasks/b.paths"), "src/b.rs\n").unwrap();
        for name in ["a.verify.sh", "b.verify.sh"] {
            let path = item.join("tasks").join(name);
            fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).unwrap();
        }
        let err = validate_task_dag(root, "alpha").unwrap_err().to_string();
        assert!(err.contains("dependency cycle"), "{err}");
        fs::write(
            item.join("TASKS.tsv"),
            "task_id\tdepends_on\tpaths_file\tverify_script\nT1\t-\ttasks/a.paths\ttasks/a.verify.sh\n",
        )
        .unwrap();
        fs::write(item.join("tasks/a.paths"), "src/a.rs\n").unwrap();
        validate_task_dag(root, "alpha").unwrap();
        let id = task_contract_id(root, "alpha").unwrap();
        assert_eq!(id.len(), 12);
        fs::write(item.join("TASKS.id"), format!("{id}\n")).unwrap();
        task_assert_frozen(root, "alpha").unwrap();
        let rendered = item.join("TASKS.md");
        task_render_file(root, "alpha", &rendered).unwrap();
        let table = fs::read_to_string(&rendered).unwrap();
        assert!(table.contains("| T1 | - | `src/a.rs` | `tasks/a.verify.sh` | READY |\n"));
        assert!(!task_all_pass(root, "alpha").unwrap());
        assert!(task_attempt_for_run(root, "alpha", "ada", root)
            .unwrap()
            .is_none());
        fs::write(item.join("tasks/a.paths"), "src/a.rs\tb.rs\n").unwrap();
        assert_eq!(
            validate_task_dag(root, "alpha").unwrap_err().to_string(),
            "invalid TASKS.tsv: blank or malformed owned path"
        );
    }

    #[test]
    fn unreadable_attempt_does_not_fail_task_live_count() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let bad = root.join("attempts/A1.0.1");
        let good = root.join("attempts/A2.0.1");
        write_min_attempt(&bad, "A1.0.1", "alpha", "T1", "RUNNING");
        write_min_attempt(&good, "A2.0.1", "alpha", "T1", "RUNNING");
        let mut perms = fs::metadata(&bad).unwrap().permissions();
        perms.set_mode(0o0);
        fs::set_permissions(&bad, perms).unwrap();
        let _unlock = Unlock(bad);
        assert_eq!(task_live_count(root, "alpha").unwrap(), 1);
    }

    #[test]
    fn octal_maker_budget_matches_shell_arithmetic() {
        let _lock = crate::claims::ENV_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        maker_ready(root);
        let clock = FixedClock::new(EPOCH);
        let _env = EnvSet::set("CRUCIBLE_MAKER_SECONDS", "010");
        dispatch(root, &["alpha", "maker", "ada"], &clock).unwrap();
        let id = format!("A{EPOCH}.{}.1", std::process::id());
        let meta = fs::read_to_string(root.join("attempts").join(&id).join("meta.tsv")).unwrap();
        assert!(
            meta.contains(&format!("\t{EPOCH}\t{}\t-\n", EPOCH + 8)),
            "{meta}"
        );
        drop(tmp);
        drop(_env);

        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        maker_ready(root);
        let _env = EnvSet::set("CRUCIBLE_MAKER_SECONDS", "08");
        assert_eq!(
            dispatch(root, &["alpha", "maker", "ada"], &clock)
                .unwrap_err()
                .to_string(),
            "08: value too great for base (error token is \"08\")"
        );
        assert!(!root.join("attempts").exists());
    }

    #[test]
    fn failing_git_diff_does_not_publish_the_attempt() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(root.join("agents.tsv"), "dee\tother\tm\th\techo {BRIEF}\n").unwrap();
        fs::create_dir_all(root.join("roles")).unwrap();
        fs::write(
            root.join("roles/judge.md"),
            "purpose: j\nmay-read: a\nmust-not-read: b\nreturn: c\nverify: d\n\n## Instructions\nLook.\n",
        )
        .unwrap();
        let item = root.join("items/alpha");
        fs::create_dir_all(&item).unwrap();
        fs::write(item.join("ITEM.md"), "# alpha\n\n- [ ] A1\n").unwrap();
        let repo = root.join("not-a-repo");
        fs::write(
            item.join("TARGET"),
            format!("repo: {}\nbranch: missing\nbase: missing\n", repo.display()),
        )
        .unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tREVIEW\tEMPTY\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        assert!(dispatch(root, &["alpha", "judge", "dee"], &FixedClock::new(EPOCH)).is_err());
        let attempts = root.join("attempts");
        if attempts.is_dir() {
            for ent in fs::read_dir(&attempts).unwrap().flatten() {
                let name = ent.file_name().to_string_lossy().into_owned();
                assert!(
                    !name.starts_with('A') && !name.starts_with('.'),
                    "{name} was published"
                );
            }
        }
    }

    #[test]
    fn empty_evidence_file_has_no_blank_line_inside_the_fence() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(root.join("agents.tsv"), "dee\tother\tm\th\techo {BRIEF}\n").unwrap();
        fs::create_dir_all(root.join("roles")).unwrap();
        fs::write(
            root.join("roles/judge.md"),
            "purpose: j\nmay-read: a\nmust-not-read: b\nreturn: c\nverify: d\n\n## Instructions\nLook.\n",
        )
        .unwrap();
        let item = root.join("items/alpha");
        fs::create_dir_all(item.join("evidence")).unwrap();
        fs::create_dir_all(item.join("work")).unwrap();
        fs::write(item.join("ITEM.md"), "# alpha\n\n- [ ] A1\n").unwrap();
        fs::write(item.join("evidence/empty.txt"), "").unwrap();
        fs::write(item.join("work/blank"), "").unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tREVIEW\tEMPTY\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        dispatch(root, &["alpha", "judge", "dee"], &FixedClock::new(EPOCH)).unwrap();
        let id = format!("A{EPOCH}.{}.1", std::process::id());
        let contract =
            fs::read_to_string(root.join("attempts").join(&id).join("contract.md")).unwrap();
        assert!(
            contract.contains("### work/blank\n```\n```\n"),
            "{contract}"
        );
        assert!(contract.contains("### empty.txt\n```\n```\n"), "{contract}");
        assert!(!contract.contains("```\n\n```"));
    }

    fn maker_ready(root: &Path) {
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(root.join("agents.tsv"), "ada\tgrok\tm\th\techo {BRIEF}\n").unwrap();
        fs::create_dir_all(root.join("roles")).unwrap();
        fs::write(
            root.join("roles/maker.md"),
            "purpose: make\nmay-read: item\nmust-not-read: v\nreturn: DONE\nverify: run\n\n## Instructions\nBuild.\n",
        )
        .unwrap();
        let item = root.join("items/alpha");
        fs::create_dir_all(item.join("work")).unwrap();
        fs::write(item.join("ITEM.md"), "# alpha\n\n- [ ] A1 do the thing\n").unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tBUILD\tEMPTY\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
    }

    fn write_min_attempt(dir: &Path, id: &str, slug: &str, task: &str, state: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("meta.tsv"),
            format!(
                "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\n{id}\t{slug}\t{task}\tw\tmaker\tada\tgrok\tA1\tFOCUSED\tDISPATCHED\t1\t2\t-\n"
            ),
        )
        .unwrap();
        fs::write(
            dir.join("events.tsv"),
            format!("state\tepoch\tpid\treason\n{state}\t1\t-\tseed\n"),
        )
        .unwrap();
    }

    struct Unlock(PathBuf);

    impl Drop for Unlock {
        fn drop(&mut self) {
            let perms = fs::Permissions::from_mode(0o755);
            let _ = fs::set_permissions(&self.0, perms);
        }
    }

    struct EnvSet(&'static str);

    impl EnvSet {
        fn set(key: &'static str, value: &str) -> Self {
            // SAFETY: the caller holds ENV_LOCK for this process-global key.
            unsafe { std::env::set_var(key, value) };
            Self(key)
        }
    }

    impl Drop for EnvSet {
        fn drop(&mut self) {
            // SAFETY: dropped while ENV_LOCK is still held.
            unsafe { std::env::remove_var(self.0) };
        }
    }
}
