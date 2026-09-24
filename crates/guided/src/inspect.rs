//! Remaining inspect verbs: agents, target, state, lifecycle, next, brief, workid,
//! evidence archive, panes, run, and add.
//!
//! `run` evidence `when:` is [`format_rfc3339_z`] of [`Clock::now_unix`] (shell line 1377).

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crucible_contract::{format_rfc3339_z, Clock};

use crate::close::check;
use crate::cycle::{
    attempt_meta, attempt_state, claim_field, count_claim_headings, file_name, program_field,
    read_dir_paths, self_path, MARK,
};
use crate::dispatch::{
    emit_work, evidence_block, git_quiet, item_dir, need, require_registered, task_all_pass,
    task_assert_frozen, task_attempt_for_run, task_integration_complete,
};
use crate::panel::{approval_current, panel_approval_current};
use crate::program::{lifecycle_mode, uses_guided_cycle, uses_managed_lifecycle};
use crate::run::{append_command, next_run_token};
use crate::state::{
    state_add_item, state_lock, state_render_file, state_unlock, state_validate_file, state_value,
    STATE_HEADER,
};
use crate::{message, records, GuidedError};

/// `crucible agents`.
pub fn agents(root: &Path) -> Result<String, GuidedError> {
    let path = root.join("agents.tsv");
    if !path.is_file() {
        return Err(message("no agents.tsv here"));
    }
    let mut out = format!(
        "{:<10} {:<8} {:<22} {:<8} {}\n",
        "NAME", "KIND", "MODEL", "EFFORT", "COMMAND"
    );
    let text = fs::read_to_string(path)?;
    for line in records(&text) {
        if line.starts_with('#') || line.bytes().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let name = fields.first().copied().unwrap_or("");
        let kind = fields.get(1).copied().unwrap_or("");
        let model = dash(fields.get(2).copied().unwrap_or(""));
        let effort = dash(fields.get(3).copied().unwrap_or(""));
        let command = command_cell(&fields);
        out.push_str(&format!(
            "{name:<10} {kind:<8} {model:<22} {effort:<8} {command}\n"
        ));
    }
    Ok(out)
}

fn dash(value: &str) -> &str {
    if value.is_empty() {
        "-"
    } else {
        value
    }
}

fn command_cell(fields: &[&str]) -> String {
    if fields.len() < 5 {
        return "(none)".to_string();
    }
    let joined = fields[4..].join("\t");
    if joined.is_empty() {
        "(none)".to_string()
    } else {
        joined
    }
}

/// `crucible target SLUG REPO BRANCH BASE`.
pub fn target(root: &Path, args: &[&str]) -> Result<String, GuidedError> {
    let slug = args.first().copied().unwrap_or("");
    let dir = need(root, slug)?;
    let repo = args.get(1).copied().unwrap_or("");
    let branch = args.get(2).copied().unwrap_or("");
    let base = args.get(3).copied().unwrap_or("");
    if repo.is_empty() || branch.is_empty() || base.is_empty() {
        return Err(message("usage: crucible target SLUG REPO BRANCH BASE"));
    }
    let git = Path::new(repo).join(".git");
    if !git.is_dir() && !git.is_file() {
        return Err(message(format!("not a git repo: {repo}")));
    }
    if !git_quiet(repo, &["rev-parse", "--verify", "--quiet", base]) {
        return Err(message(format!("no such base ref: {base}")));
    }
    fs::write(
        dir.join("TARGET"),
        format!("repo: {repo}\nbranch: {branch}\nbase: {base}\n"),
    )?;
    Ok(format!(
        "item {slug} now tracks {repo} branch {branch} off {base}\n\
         the maker works in that repo on that branch, not in items/{slug}/work/\n"
    ))
}

/// `crucible state`.
pub fn state(root: &Path) -> Result<String, GuidedError> {
    if !uses_managed_lifecycle(root)? {
        return Err(message(format!(
            "state requires managed lifecycle behavior — run: {} lifecycle enable --dry-run",
            self_path(root)
        )));
    }
    let mut lock = state_lock(root)?;
    state_validate_file(&root.join("STATE.tsv"))?;
    state_render_file(root, &lock.md_tmp, Some(&root.join("STATE.tsv")))?;
    fs::rename(&lock.md_tmp, root.join("STATE.md"))?;
    state_unlock(&mut lock)?;
    Ok(format!("{}/STATE.md\n", root.display()))
}

/// `crucible lifecycle status|enable`.
pub fn lifecycle(root: &Path, args: &[&str]) -> Result<String, GuidedError> {
    let sub = args.first().copied().unwrap_or("status");
    let flag = args.get(1).copied().unwrap_or("");
    match sub {
        "status" => {
            if !flag.is_empty() {
                return Err(message("usage: crucible lifecycle status"));
            }
            Ok(format!("lifecycle: {}\n", lifecycle_mode(root)?))
        }
        "enable" => enable_lifecycle(root, flag),
        _ => Err(message(
            "usage: crucible lifecycle status|enable --dry-run|--apply",
        )),
    }
}

fn enable_lifecycle(root: &Path, flag: &str) -> Result<String, GuidedError> {
    if flag != "--dry-run" && flag != "--apply" {
        return Err(message(
            "usage: crucible lifecycle enable --dry-run|--apply",
        ));
    }
    if uses_managed_lifecycle(root)? {
        return Ok("lifecycle: managed\n".to_string());
    }
    if has_item_dir(root) {
        return Err(message(
            "refused: managed lifecycle can only be enabled before the first item",
        ));
    }
    let mut out = format!(
        "CREATE {}/STATE.tsv\nREPLACE {}/STATE.md\nUPDATE {}/PROGRAM lifecycle: managed\n",
        root.display(),
        root.display(),
        root.display()
    );
    if flag != "--apply" {
        return Ok(out);
    }
    let mut lock = state_lock(root)?;
    fs::write(&lock.tsv_tmp, format!("{STATE_HEADER}\n"))?;
    state_validate_file(&lock.tsv_tmp)?;
    state_render_file(root, &lock.md_tmp, Some(&lock.tsv_tmp))?;
    let program = fs::read_to_string(root.join("PROGRAM"))?;
    let mut body = String::new();
    for line in records(&program) {
        if line.starts_with("lifecycle: ") {
            continue;
        }
        body.push_str(line);
        body.push('\n');
    }
    body.push_str("lifecycle: managed\n");
    fs::write(&lock.program_tmp, body)?;
    fs::rename(&lock.tsv_tmp, root.join("STATE.tsv"))?;
    fs::rename(&lock.md_tmp, root.join("STATE.md"))?;
    fs::rename(&lock.program_tmp, root.join("PROGRAM"))?;
    state_unlock(&mut lock)?;
    out.push_str("enabled managed lifecycle\n");
    Ok(out)
}

fn has_item_dir(root: &Path) -> bool {
    read_dir_paths(&root.join("items")).into_iter().any(|path| {
        fs::symlink_metadata(&path)
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
    })
}

/// `crucible next`.
pub fn next(root: &Path) -> Result<String, GuidedError> {
    if uses_managed_lifecycle(root)? {
        return next_managed(root);
    }
    next_item_file(root)
}

fn next_managed(root: &Path) -> Result<String, GuidedError> {
    state_validate_file(&root.join("STATE.tsv"))?;
    let Some(slug) = current_slug(root)? else {
        return Ok("DONE\n".to_string());
    };
    let status = state_value(root, &slug, 2)?.unwrap_or_default();
    let stage = state_value(root, &slug, 3)?.unwrap_or_default();
    let inflight = state_value(root, &slug, 6)?.unwrap_or_default();
    let block = state_value(root, &slug, 7)?.unwrap_or_default();
    let sp = self_path(root);
    if status == "BLOCKED" {
        return Ok(format!(
            "BLOCKED {slug} {block} see {}/items/{slug}/ITEM.md\n",
            root.display()
        ));
    }
    if inflight == "TASKS" {
        return Ok(format!("NEXT {slug} TASK_READY {sp} task ready {slug}\n"));
    }
    if inflight != "-" {
        let st = attempt_state(root, &inflight)?;
        let deadline = attempt_meta(root, &inflight, 12)?;
        return match st.as_str() {
            "DISPATCHED" | "RUNNING" => Ok(format!("WAIT {inflight} {deadline}\n")),
            "OVERDUE" => Ok(format!(
                "BLOCKED {slug} OVERDUE_PROCESS record an observed finish for {inflight}\n"
            )),
            "RETURNED" => Ok(format!(
                "NEXT {slug} RESULT {sp} result {inflight} <OUTCOME> <EVIDENCE-FILE> <NEXT> [FINGERPRINT]\n"
            )),
            "TIMEOUT" => {
                let role = attempt_meta(root, &inflight, 5)?;
                let agent = attempt_meta(root, &inflight, 6)?;
                let criterion = attempt_meta(root, &inflight, 8)?;
                let class = attempt_meta(root, &inflight, 9)?;
                Ok(format!(
                    "NEXT {slug} RETRY {sp} dispatch {slug} {role} {agent} {criterion} {class} {inflight}\n"
                ))
            }
            _ => Err(message(format!(
                "state points to terminal attempt {inflight} ({st})"
            ))),
        };
    }
    match stage.as_str() {
        "DRAFT" => Ok(format!("NEXT {slug} READY {sp} ready {slug}\n")),
        "READY" => Ok(format!("NEXT {slug} BUILD {sp} phase {slug} BUILD\n")),
        "BUILD" => next_build(root, &slug, &sp),
        "REVIEW" => Ok(format!("NEXT {slug} CHECK {sp} check {slug}\n")),
        _ => Ok(String::new()),
    }
}

fn next_build(root: &Path, slug: &str, sp: &str) -> Result<String, GuidedError> {
    if item_dir(root, slug).join("TASKS.tsv").is_file() {
        task_assert_frozen(root, slug)?;
        if task_integration_complete(root, slug)? {
            return Ok(format!("NEXT {slug} REVIEW {sp} phase {slug} REVIEW\n"));
        }
        if task_all_pass(root, slug)? {
            return Ok(format!(
                "NEXT {slug} INTEGRATE {sp} task integrate {slug}\n"
            ));
        }
        return Ok(format!("NEXT {slug} TASK_READY {sp} task ready {slug}\n"));
    }
    Ok(format!("NEXT {slug} REVIEW {sp} phase {slug} REVIEW\n"))
}

fn current_slug(root: &Path) -> Result<Option<String>, GuidedError> {
    let text = fs::read_to_string(root.join("STATE.tsv"))?;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let status = rec.split('\t').nth(1).unwrap_or("");
        if status == "ACTIVE" || status == "BLOCKED" {
            let item = rec.split('\t').next().unwrap_or("");
            return Ok(Some(item.to_string()));
        }
    }
    Ok(None)
}

fn next_item_file(root: &Path) -> Result<String, GuidedError> {
    let mut out = String::new();
    let claims = root.join("CLAIMS.md");
    if claims.is_file() {
        let text = fs::read_to_string(&claims)?;
        let tot = count_claim_headings(&text);
        if tot > 0 {
            let mut unaudited = 0usize;
            let mut admitted = 0usize;
            for i in 1..=tot {
                let verdicts = root.join("claims").join(format!("C{i}")).join("verdicts");
                if count_md(&verdicts) == 0 {
                    unaudited += 1;
                }
                if claim_field(&text, i, "item")
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
                {
                    admitted += 1;
                }
            }
            out.push_str(&format!(
                "outer loop: {tot} claim(s), {unaudited} unaudited, {admitted} admitted\n"
            ));
            if unaudited > 0 {
                out.push_str(
                    "\
  do this first: dispatch two claim-auditors of DIFFERENT kinds per unaudited claim,
  record with: crucible claim verdict CN AGENT TRUE|FALSE|STALE|UNVERIFIABLE
  nothing should be planned or built until the report has been fact-checked.

",
                );
            } else if admitted == 0 {
                out.push_str(
                    "\
  every claim is audited. now: crucible triage — then take it to the operator
  and agree the backlog before admitting anything.

",
                );
            }
        } else if root.join("PROGRAM").is_file() {
            out.push_str(
                "\
no claims yet. Ask the operator for the problem document, then one claim per finding:
  crucible claim add \"<claim>\" \"<the exact sentence from the document>\"

",
            );
        }
    }
    if !root.join("items").is_dir() {
        out.push_str("no items yet. Read START.md, then: crucible add SLUG \"TITLE\"\n");
        return Ok(out);
    }
    let mut found = false;
    for dir in item_dirs(root) {
        let slug = file_name(&dir);
        if status_closed(&dir) {
            continue;
        }
        found = true;
        let phase = crate::phase::phase_of(root, &slug)?;
        let phase = if phase.is_empty() {
            "SPEC".to_string()
        } else {
            phase
        };
        out.push_str(&format!("{slug} — phase {phase}\n"));
        match phase.as_str() {
            "SPEC" => {
                if !dir.join("DESIGN.md").is_file() {
                    out.push_str("  dispatch: specifier, then judge\n");
                }
            }
            "DESIGN" => out.push_str("  dispatch: architect, then judge\n"),
            "TASKS" => out.push_str("  dispatch: planner, then judge\n"),
            "BUILD" => out.push_str("  dispatch: maker per task, then judge each\n"),
            "VERIFY" => out.push_str("  dispatch: maker to record the named checks, then judges\n"),
            "ADVERSARY" => out.push_str("  dispatch: adversary\n"),
            "GRADUATE" => out.push_str("  dispatch: integrator, then close\n"),
            _ => {}
        }
        out.push_str(&blocked_lines(root, &slug));
        out.push_str(&format!(
            "  when ready: crucible dispatch {slug} <role> <agent>\n"
        ));
    }
    if !found {
        out.push_str("every item is closed.\n");
    }
    Ok(out)
}

fn item_dirs(root: &Path) -> Vec<PathBuf> {
    let mut paths = read_dir_paths(&root.join("items"));
    paths.retain(|path| path.is_dir() && !file_name(path).starts_with('.'));
    paths.sort();
    paths
}

fn status_closed(dir: &Path) -> bool {
    fs::read_to_string(dir.join("ITEM.md"))
        .map(|text| {
            records(&text)
                .iter()
                .any(|line| line.starts_with("STATUS: CLOSED"))
        })
        .unwrap_or(false)
}

fn blocked_lines(root: &Path, slug: &str) -> String {
    let text = match check(root, &[slug]) {
        Ok(report) => report.text,
        Err(_) => return String::new(),
    };
    let mut out = String::new();
    let mut n = 0;
    for line in records(&text) {
        let Some(rest) = line.strip_prefix("  FAIL") else {
            continue;
        };
        out.push_str("  blocked: ");
        out.push_str(rest);
        out.push('\n');
        n += 1;
        if n == 6 {
            break;
        }
    }
    out
}

fn count_md(dir: &Path) -> usize {
    fn walk(dir: &Path, n: &mut usize) {
        for path in read_dir_paths(dir) {
            if file_name(&path).ends_with(".md") {
                *n += 1;
            }
            let is_dir = fs::symlink_metadata(&path)
                .map(|meta| meta.is_dir())
                .unwrap_or(false);
            if is_dir {
                walk(&path, n);
            }
        }
    }
    let mut n = 0;
    if dir.is_dir() {
        walk(dir, &mut n);
    }
    n
}

/// `crucible brief SLUG maker|judge NAME`.
pub fn brief(root: &Path, args: &[&str]) -> Result<String, GuidedError> {
    let slug = args.first().copied().unwrap_or("");
    let dir = need(root, slug)?;
    let role = args.get(1).copied().unwrap_or("");
    let name = args.get(2).copied().unwrap_or("");
    if name.is_empty() {
        return Err(message("usage: crucible brief SLUG maker|judge NAME"));
    }
    require_registered(root, name)?;
    let wid = crate::cycle::workid(root, slug)?;
    let sp = self_path(root);
    match role {
        "maker" => brief_maker(root, &dir, slug, name, &sp),
        "judge" => brief_judge(root, &dir, slug, name, &wid, &sp),
        _ => Err(message("role must be maker or judge")),
    }
}

fn brief_maker(
    root: &Path,
    dir: &Path,
    slug: &str,
    name: &str,
    sp: &str,
) -> Result<String, GuidedError> {
    fs::write(dir.join("MAKER"), format!("{name}\n"))?;
    let out_path = dir.join("briefs/maker.md");
    let mut body = format!(
        "# Maker brief: {slug}\n\nYou are {name}, the maker. Do the work described below.\n\n"
    );
    if dir.join("TARGET").is_file() {
        let repo = crate::dispatch::tgt(root, slug, "repo")?;
        let branch = crate::dispatch::tgt(root, slug, "branch")?;
        let base = crate::dispatch::tgt(root, slug, "base")?;
        body.push_str(&format!(
            "Work in the repository **{repo}** on branch **{branch}**,\n\
             which branches from **{base}**. Commit there. Do not push.\n"
        ));
    } else {
        body.push_str(&format!(
            "Write your changes only under `{}/work/`. Do not create symlinks and do not put\n\
             newlines in filenames; both are refused.\n",
            dir.display()
        ));
    }
    body.push_str(&format!(
        "\nDo not write to `{}/evidence/` by hand — hand-written evidence is refused. To record\n\
         a check, have this run it for you:\n\n\
         \x20\x20\x20\x20{sp} run {slug} {name} -- <your command>\n\n\
         That captures the command, its exit status and its output, and stamps it with the\n\
         work id at the moment it ran, so it cannot be relabelled later.\n\n\
         Do not write to `{dir}/verdicts/` or edit `{dir}/ITEM.md`. Ask the orchestrator if the\n\
         acceptance criteria are wrong.\n\n\
         Run `{sp} check {slug}` to see what still refuses. When only the judge count\n\
         refuses, you are done.\n\n\
         ---\n\n",
        dir.display(),
        dir = dir.display()
    ));
    body.push_str(&fs::read_to_string(dir.join("ITEM.md"))?);
    append_lessons(root, &mut body)?;
    fs::write(&out_path, body)?;
    Ok(format!("{}\n", out_path.display()))
}

fn brief_judge(
    root: &Path,
    dir: &Path,
    slug: &str,
    name: &str,
    wid: &str,
    sp: &str,
) -> Result<String, GuidedError> {
    if wid == "EMPTY" {
        return Err(message(
            "refused: no work yet, so there is nothing to judge",
        ));
    }
    let maker = maker_name(dir);
    if name == maker {
        return Err(message(format!("refused: {name} is the maker of {slug}")));
    }
    let out_path = dir.join(format!("briefs/judge-{name}.md"));
    let mut body = format!(
        "# Judge brief: {slug}\n\n\
         You are {name}, an independent judge. You have no knowledge of how this was built.\n\
         You get the goal, the work, and the recorded evidence. There is no implementer\n\
         report or rationale here.\n\n\
         Verify by running things, not by reading. Record every check you run with:\n\n\
         \x20\x20\x20\x20{sp} run {slug} {name} -- <your command>\n\n\
         A PASS is refused unless you recorded at least one check yourself.\n\n\
         Then write `{dir}/verdicts/{name}.md`, and no other file. First two lines exactly:\n\n\
         \x20\x20\x20\x20VERDICT: PASS\n\
         \x20\x20\x20\x20WORK-ID: {wid}\n\n\
         Use REJECT, INSUFFICIENT_EVIDENCE or SCOPE_CONFLICT instead of PASS as warranted.\n\
         PASS also requires that you ran the falsifier in the item and cite its output.\n\
         REJECT requires file:line for every finding. If you cannot decide from what you\n\
         have, return INSUFFICIENT_EVIDENCE and name the missing artifact — never PASS.\n\n\
         The evidence below was recorded by other agents. It cannot be stale, but nothing\n\
         proves it was a meaningful check. Re-derive anything you rely on.\n\n\
         ## The goal\n\n",
        dir = dir.display()
    );
    body.push_str(&fs::read_to_string(dir.join("ITEM.md"))?);
    body.push_str(&format!("\n## The work — work id {wid}\n\n"));
    body.push_str(&emit_work(root, slug, wid)?);
    body.push_str("\n## Recorded evidence\n\n");
    body.push_str(&evidence_block(&dir.join("evidence"))?);
    fs::write(&out_path, body)?;
    Ok(format!("{}\n", out_path.display()))
}

fn maker_name(dir: &Path) -> String {
    let path = dir.join("MAKER");
    if !path.is_file() {
        return String::new();
    }
    fs::read_to_string(path)
        .ok()
        .and_then(|text| records(&text).first().copied().map(str::to_string))
        .unwrap_or_default()
}

fn append_lessons(root: &Path, body: &mut String) -> Result<(), GuidedError> {
    let path = root.join("LESSONS.md");
    let Ok(meta) = fs::metadata(&path) else {
        return Ok(());
    };
    if !meta.is_file() || meta.len() == 0 {
        return Ok(());
    }
    body.push_str("\n---\n\n## Lessons from earlier items — these bind you\n\n");
    body.push_str(&fs::read_to_string(path)?);
    Ok(())
}

/// `crucible workid SLUG`.
pub fn workid(root: &Path, args: &[&str]) -> Result<String, GuidedError> {
    let slug = args.first().copied().unwrap_or("");
    need(root, slug)?;
    Ok(format!("{}\n", crate::cycle::workid(root, slug)?))
}

/// `crucible evidence archive SLUG`.
pub fn evidence(root: &Path, args: &[&str]) -> Result<String, GuidedError> {
    let sub = args.first().copied().unwrap_or("");
    let slug = args.get(1).copied().unwrap_or("");
    if sub != "archive" || slug.is_empty() {
        return Err(message("usage: crucible evidence archive SLUG"));
    }
    let dir = need(root, slug)?;
    let evidence = dir.join("evidence");
    if !evidence.is_dir() {
        return Err(message(format!("no evidence directory: {slug}")));
    }
    let wid = crate::cycle::workid(root, slug)?;
    let history = evidence.join("history");
    fs::create_dir_all(&history)?;
    let mut paths = read_dir_paths(&evidence);
    paths.retain(|path| path.is_file() && !file_name(path).starts_with('.'));
    paths.sort();
    let mut out = String::new();
    let mut moved = 0usize;
    for path in paths {
        let name = file_name(&path);
        let Some(stem) = name.strip_suffix(".txt") else {
            continue;
        };
        let id = stem.rsplit('.').next().unwrap_or(stem);
        if id == wid {
            continue;
        }
        fs::rename(&path, history.join(&name))?;
        out.push_str(&format!("archived {name} (work-id {id}, current {wid})\n"));
        moved += 1;
    }
    if moved == 0 {
        out.push_str(&format!(
            "no stale evidence to archive for {slug} (work id {wid})\n"
        ));
    }
    Ok(out)
}

/// `crucible panes [views...]`.
pub fn panes(root: &Path, args: &[&str]) -> Result<String, GuidedError> {
    if std::env::var("TMUX")
        .ok()
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return Err(message(
            "not inside tmux — start a session first (e.g. your session manager, then re-run)",
        ));
    }
    if !have("tmux") {
        return Err(message("tmux is not installed"));
    }
    let watch = root.join("scripts/watch.sh");
    if !watch.is_file() {
        return Err(message(format!("no watch.sh at {}", watch.display())));
    }
    let views = pane_views(args);
    for view in &views {
        if !matches!(
            view.as_str(),
            "gate" | "verdicts" | "evidence" | "tail" | "git" | "workids" | "memory"
        ) {
            return Err(message(format!(
                "unknown view: {view} (gate verdicts evidence tail git workids memory)"
            )));
        }
    }
    let keep = tmux_text(&["display-message", "-p", "#{pane_id}"])?;
    let _ = tmux_status(&["set-option", "-w", "pane-border-status", "top"]);
    let _ = tmux_status(&["select-pane", "-t", &keep, "-T", "agent"]);
    let mut avail = tmux_lines(&["list-panes", "-F", "#{pane_id}"])
        .into_iter()
        .filter(|id| id != &keep)
        .collect::<Vec<_>>();
    let cwd = std::env::var("PWD").unwrap_or_else(|_| {
        std::env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| ".".to_string())
    });
    let watch_q = watch.display().to_string().replace('\'', "'\\''");
    for (n, view) in views.iter().enumerate() {
        let target = if let Some(existing) = avail.get(n) {
            existing.clone()
        } else {
            let created = tmux_text(&[
                "split-window",
                "-h",
                "-d",
                "-P",
                "-F",
                "#{pane_id}",
                "-t",
                &keep,
                "-c",
                &cwd,
            ])?;
            avail.push(created.clone());
            created
        };
        let script = format!("sh '{watch_q}' {view} 5");
        if !tmux_status(&["respawn-pane", "-k", "-t", &target, &script]) {
            let _ = tmux_status(&[
                "send-keys",
                "-t",
                &target,
                "C-c",
                &format!("exec {script}"),
                "C-m",
            ]);
        }
        let _ = tmux_status(&["select-pane", "-t", &target, "-T", view]);
    }
    let _ = tmux_status(&["select-layout", "-t", &keep, "main-vertical"]);
    if !tmux_status(&["select-pane", "-t", &keep]) {
        return Err(message("tmux select-pane failed"));
    }
    let listed = views.join(" ");
    Ok(format!(
        "overlaid {} view(s) onto this window: {listed}\n\
         your pane is titled \"agent\" and was left alone. re-run any time to reset them.\n",
        views.len()
    ))
}

fn pane_views(args: &[&str]) -> Vec<String> {
    let split = args
        .iter()
        .flat_map(|arg| arg.split_whitespace().map(str::to_string))
        .collect::<Vec<_>>();
    if split.is_empty() {
        ["gate", "verdicts", "workids", "tail"]
            .into_iter()
            .map(str::to_string)
            .collect()
    } else {
        split
    }
}

fn tmux_text(args: &[&str]) -> Result<String, GuidedError> {
    let out = Command::new("tmux").args(args).output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(message(if err.is_empty() {
            "tmux failed".to_string()
        } else {
            err.to_string()
        }));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn tmux_lines(args: &[&str]) -> Vec<String> {
    let Ok(out) = Command::new("tmux").args(args).output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn tmux_status(args: &[&str]) -> bool {
    Command::new("tmux")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// `crucible run SLUG NAME -- CMD...`. The command's own failure is recorded, not returned.
pub fn run(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    let slug = args.first().copied().unwrap_or("");
    let dir = need(root, slug)?;
    if args.len() < 2 {
        return Err(message("usage: crucible run SLUG NAME -- CMD..."));
    }
    let name = args[1];
    let rest = &args[2..];
    if rest.first().copied() != Some("--") {
        return Err(message("usage: crucible run SLUG NAME -- CMD..."));
    }
    let cmd = &rest[1..];
    if cmd.is_empty() {
        return Err(message("no command given"));
    }
    require_registered(root, name)?;
    let mut wid = crate::cycle::workid(root, slug)?;
    let mut run_attempt = "-".to_string();
    if uses_managed_lifecycle(root)? {
        run_attempt = state_value(root, slug, 6)?.unwrap_or_default();
        if run_attempt == "-" {
            return Err(message("managed evidence requires an in-flight attempt"));
        }
        if run_attempt == "TASKS" {
            let cwd = std::env::current_dir()?;
            let cwd = fs::canonicalize(&cwd).unwrap_or(cwd);
            let Some(found) = task_attempt_for_run(root, slug, name, &cwd).ok().flatten() else {
                return Err(message(format!(
                    "no live task attempt for {name} in {}",
                    cwd.display()
                )));
            };
            run_attempt = found;
        }
        let owner = attempt_meta(root, &run_attempt, 6)?;
        if owner != name {
            return Err(message(format!(
                "in-flight attempt belongs to {owner}, not {name}"
            )));
        }
        let state_now = attempt_state(root, &run_attempt)?;
        if state_now != "RUNNING" && state_now != "OVERDUE" {
            return Err(message(
                "managed evidence requires a RUNNING or OVERDUE attempt",
            ));
        }
        let task = attempt_meta(root, &run_attempt, 3)?;
        if task != "-" {
            wid = task_head(root, &run_attempt);
        }
    }
    let tok = next_run_token();
    let evidence = dir.join("evidence");
    let out = evidence.join(format!("{name}.{tok}.{wid}.txt"));
    let tmp = evidence.join(format!(".partial.{name}.{tok}.{wid}"));
    let mut header = String::new();
    header.push_str(MARK);
    header.push('\n');
    header.push_str(&format!("agent: {name}\nwork-id: {wid}\n"));
    if run_attempt != "-" {
        header.push_str(&format!("attempt-id: {run_attempt}\n"));
    }
    // Shell line 1377: `date -u +%Y-%m-%dT%H:%M:%SZ`.
    header.push_str(&format!(
        "when: {}\ncommand:",
        format_rfc3339_z(clock.now_unix())
    ));
    for arg in cmd {
        header.push(' ');
        header.push_str(arg);
    }
    header.push_str("\n--- output ---\n");
    fs::write(&tmp, header)?;
    let rc = match append_command(&tmp, cmd) {
        Ok(code) => code,
        Err(err) if err.kind() == ErrorKind::NotFound => 127,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => 126,
        Err(err) => return Err(err.into()),
    };
    let mut file = OpenOptions::new().append(true).open(&tmp)?;
    writeln!(file, "--- exit {rc} ---")?;
    drop(file);
    fs::rename(&tmp, &out)?;
    Ok(format!("{} (exit {rc})\n", out.display()))
}

fn task_head(root: &Path, attempt: &str) -> String {
    let path = root.join("attempts").join(attempt).join("task.tsv");
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    let row = records(&text).get(1).copied().unwrap_or("");
    let mut fields = row.split('\t');
    let repo = fields.next().unwrap_or("");
    let branch = fields.next().unwrap_or("");
    let Ok(out) = Command::new("git")
        .args(["-C", repo, "rev-parse", "--verify", branch])
        .stderr(Stdio::null())
        .output()
    else {
        return String::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(12)
        .collect()
}

/// `crucible add SLUG TITLE...`.
pub fn add(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    let slug = args.first().copied().unwrap_or("");
    if slug.is_empty() {
        return Err(message("usage: crucible add SLUG \"TITLE\""));
    }
    if !slug
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(message("slug may only contain A-Za-z0-9._-"));
    }
    let title = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        String::new()
    };
    let managed = uses_managed_lifecycle(root)?;
    if managed {
        state_validate_file(&root.join("STATE.tsv"))?;
        if current_count(root)? != 0 {
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
    let agents_path = root.join("agents.tsv");
    if !agents_path.is_file() {
        fs::write(&agents_path, agents_seed())?;
        out.push_str(&format!(
            "wrote {} — edit it to match the agents you will actually use\n",
            agents_path.display()
        ));
    }
    let dir = item_dir(root, slug);
    if dir.is_dir() {
        return Err(message(format!("item exists: {slug}")));
    }
    fs::create_dir_all(dir.join("work"))?;
    fs::create_dir_all(dir.join("evidence"))?;
    fs::create_dir_all(dir.join("verdicts"))?;
    fs::create_dir_all(dir.join("briefs"))?;
    let item = if managed {
        managed_item(slug, &title)
    } else {
        item_file_item(slug, &title)
    };
    fs::write(dir.join("ITEM.md"), item)?;
    if let Some(repo) = program_field(root, "repo") {
        let base = program_field(root, "base").unwrap_or_else(|| "main".to_string());
        let branch = format!("ai/{slug}");
        let _ = target(root, &[slug, &repo, &branch, &base]);
    }
    if managed {
        let wid = crate::cycle::workid(root, slug)?;
        state_add_item(root, clock, slug, &wid, "LOW")?;
    }
    out.push_str(&format!("{}/ITEM.md\n", dir.display()));
    if managed {
        out.push_str("next: complete the frozen contract, then\n");
        out.push_str(&format!("      {} ready {slug}\n", self_path(root)));
    } else {
        out.push_str("next: write the ask, criteria and falsifier into that file, then\n");
        out.push_str(&format!("      crucible brief {slug} maker <name>\n"));
    }
    Ok(out)
}

fn current_count(root: &Path) -> Result<usize, GuidedError> {
    let text = fs::read_to_string(root.join("STATE.tsv"))?;
    let mut n = 0usize;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let status = rec.split('\t').nth(1).unwrap_or("");
        if status == "ACTIVE" || status == "BLOCKED" {
            n += 1;
        }
    }
    Ok(n)
}

fn heading_title(title: &str) -> &str {
    if title.is_empty() {
        "untitled"
    } else {
        title
    }
}

fn body_title(title: &str) -> &str {
    if title.is_empty() {
        "Describe the work, and quote the report sentence that caused this item."
    } else {
        title
    }
}

fn managed_item(slug: &str, title: &str) -> String {
    format!(
        "\
# {slug} — {heading}

## Goal

{body}

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
",
        heading = heading_title(title),
        body = body_title(title)
    )
}

fn item_file_item(slug: &str, title: &str) -> String {
    format!(
        "\
# {slug} — {heading}

PHASE: SPEC
STATUS: OPEN

## The ask

{body}

## Acceptance criteria

- [ ] A1

## Falsifier

TEMPLATE-FALSIFIER-UNWRITTEN. Replace this line: name the change to undo and the check
that must then fail. The gate refuses closure while this marker is present.
",
        heading = heading_title(title),
        body = body_title(title)
    )
}

fn agents_seed() -> String {
    let mut out = String::from(
        "# name\tkind\tmodel\teffort\tcommand   ({BRIEF} {MODEL} {EFFORT} are substituted)\n\
         # Only names listed here may author a verdict. Repeat a kind for a same-kind panel.\n",
    );
    if have("kiro-cli") {
        out.push_str("lead\tkiro\tclaude-opus-5\tmax\tkiro-cli chat --no-interactive --trust-all-tools --model {MODEL} --effort {EFFORT} \"read {BRIEF} and follow it exactly\"\n");
        out.push_str("mk1\tkiro\tclaude-sonnet-5\thigh\tkiro-cli chat --no-interactive --trust-all-tools --model {MODEL} --effort {EFFORT} \"read {BRIEF} and follow it exactly\"\n");
        out.push_str("j1\tkiro\tclaude-opus-5\thigh\tkiro-cli chat --no-interactive --trust-all-tools --model {MODEL} --effort {EFFORT} \"read {BRIEF} and follow it exactly\"\n");
    }
    if have("grok") {
        out.push_str("j2\tgrok\tgrok-4.5\thigh\tgrok -p \"read {BRIEF} and follow it exactly\" --model {MODEL}\n");
    }
    if have("codex") {
        out.push_str("j3\tcodex\tgpt-5-codex\thigh\tcodex exec --skip-git-repo-check -m {MODEL} \"read {BRIEF} and follow it exactly\"\n");
    }
    out
}

fn have(cmd: &str) -> bool {
    Command::new("sh")
        .args(["-c", "command -v \"$1\" >/dev/null 2>&1", "sh", cmd])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::STATE_HEADER;
    use crucible_contract::FixedClock;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-inspect-{}-{}",
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

    #[test]
    fn lifecycle_status_stays_item_file_until_enable() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        assert_eq!(lifecycle(root, &[]).unwrap(), "lifecycle: item-file\n");
        fs::write(root.join("PROGRAM"), "program: work\n").unwrap();
        assert_eq!(
            lifecycle(root, &["status"]).unwrap(),
            "lifecycle: item-file\n"
        );
        let preview = lifecycle(root, &["enable", "--dry-run"]).unwrap();
        assert!(preview.contains("CREATE "), "{preview}");
        assert!(preview.contains("lifecycle: managed\n"), "{preview}");
        assert!(!root.join("STATE.tsv").exists());
        let applied = lifecycle(root, &["enable", "--apply"]).unwrap();
        assert!(
            applied.ends_with("enabled managed lifecycle\n"),
            "{applied}"
        );
        assert_eq!(
            lifecycle(root, &["status"]).unwrap(),
            "lifecycle: managed\n"
        );
        assert!(fs::read_to_string(root.join("PROGRAM"))
            .unwrap()
            .contains("lifecycle: managed\n"));
        assert!(fs::read_to_string(root.join("STATE.tsv"))
            .unwrap()
            .starts_with(STATE_HEADER));
        fs::create_dir_all(root.join("items/later")).unwrap();
        // Already managed, so a second enable does not look at items.
        assert_eq!(
            lifecycle(root, &["enable", "--dry-run"]).unwrap(),
            "lifecycle: managed\n"
        );
    }

    #[test]
    fn enable_refuses_once_an_item_directory_exists() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::write(root.join("PROGRAM"), "program: work\n").unwrap();
        fs::create_dir_all(root.join("items/alpha")).unwrap();
        assert_eq!(
            lifecycle(root, &["enable", "--dry-run"])
                .unwrap_err()
                .to_string(),
            "refused: managed lifecycle can only be enabled before the first item"
        );
    }

    #[test]
    fn run_evidence_when_line_uses_fixed_clock() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::create_dir_all(root.join("items/alpha/evidence")).unwrap();
        fs::create_dir_all(root.join("items/alpha/work")).unwrap();
        fs::write(root.join("agents.tsv"), "a1\tkindA\tm\thigh\ttrue\n").unwrap();
        let clock = FixedClock::new(1_700_000_000);
        let out = run(root, &["alpha", "a1", "--", "/bin/echo", "hello"], &clock).unwrap();
        assert!(out.contains("(exit 0)\n"), "{out}");
        let path = out.trim_end().trim_end_matches(" (exit 0)");
        let body = fs::read_to_string(path).unwrap();
        assert!(body.contains("when: 2023-11-14T22:13:20Z\n"), "{body}");
        assert!(
            body.starts_with("crucible-run/1\nagent: a1\nwork-id: "),
            "{body}"
        );
        assert!(
            body.contains("command: /bin/echo hello\n--- output ---\nhello\n--- exit 0 ---\n"),
            "{body}"
        );
    }

    #[test]
    fn add_item_file_and_agents_header_match_the_shell() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let clock = FixedClock::new(1);
        fs::write(root.join("agents.tsv"), "a1\tkindA\t\t\t\n").unwrap();
        let listed = agents(root).unwrap();
        assert!(
            listed.starts_with("NAME       KIND     MODEL                  EFFORT   COMMAND\n"),
            "{listed}"
        );
        assert!(
            listed.contains("a1         kindA    -                      -        (none)\n"),
            "{listed}"
        );
        let created = add(root, &["alpha", "a title"], &clock).unwrap();
        assert!(created.contains("/ITEM.md\n"), "{created}");
        assert!(
            created.contains("crucible brief alpha maker <name>\n"),
            "{created}"
        );
        let item = fs::read_to_string(root.join("items/alpha/ITEM.md")).unwrap();
        assert!(item.contains("PHASE: SPEC\nSTATUS: OPEN\n"), "{item}");
        assert!(item.starts_with("# alpha — a title\n"), "{item}");
        assert_eq!(workid(root, &["alpha"]).unwrap().trim(), "EMPTY");
    }

    #[test]
    fn panes_refuses_outside_tmux() {
        let tmp = Tmp::new();
        let prev = std::env::var_os("TMUX");
        // SAFETY: restored before the test returns. Panes is the only reader here.
        unsafe { std::env::remove_var("TMUX") };
        let err = panes(tmp.0.as_path(), &[]).unwrap_err().to_string();
        unsafe {
            match prev {
                Some(value) => std::env::set_var("TMUX", value),
                None => std::env::remove_var("TMUX"),
            }
        }
        assert_eq!(
            err,
            "not inside tmux — start a session first (e.g. your session manager, then re-run)"
        );
    }
}
