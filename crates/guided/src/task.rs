//! `ready`, `task`, and task integration.
//!
//! `validate_task_dag` stays in dispatch. This module calls it.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crucible_contract::Clock;

use crate::cycle::{state_attempt_update, workid};
use crate::dispatch::{
    dispatch, git_quiet, item_dir, need, section_lines, task_all_pass, task_assert_frozen,
    task_contract_id, task_dependencies_pass, task_live_attempt, task_live_count, task_pass_exists,
    task_render_file, task_result_file, task_retry_available, task_topological_order, tgt,
    validate_managed_item, validate_task_dag,
};
use crate::phase::phase_of;
use crate::program::uses_managed_lifecycle;
use crate::state::{state_update_item, state_validate_file, state_value};
use crate::{message, records, GuidedError};

static RENDER_SEQ: AtomicU64 = AtomicU64::new(0);

/// `crucible ready SLUG`.
pub fn ready(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if !uses_managed_lifecycle(root)? {
        return Err(message("ready requires managed lifecycle behavior"));
    }
    let slug = args.first().copied().unwrap_or("");
    let dir = need(root, slug)?;
    state_validate_file(&root.join("STATE.tsv"))?;
    let old = state_value(root, slug, 3)?.unwrap_or_default();
    if old != "DRAFT" {
        let shown = if old.is_empty() { "missing" } else { &old };
        return Err(message(format!(
            "refused: ready requires DRAFT, found {shown}"
        )));
    }
    validate_managed_item(root, slug)?;
    if dir.join("TASKS.tsv").is_file() {
        validate_task_dag(root, slug)?;
        freeze_tasks(root, slug, &dir)?;
    } else if dir.join("TASKS.md").exists() || dir.join("TASKS.id").exists() {
        return Err(message(
            "refused: generated task artifacts exist without TASKS.tsv",
        ));
    }
    let item = fs::read_to_string(dir.join("ITEM.md"))?;
    let risk = section_lines(&item, "Risk").join("\n");
    let wid = workid(root, slug)?;
    state_update_item(root, clock, slug, "ACTIVE", "READY", &wid, &risk, "-", "-")?;
    Ok(format!("{slug} is now READY\n"))
}

fn freeze_tasks(root: &Path, slug: &str, dir: &Path) -> Result<(), GuidedError> {
    let pid = std::process::id();
    let view_tmp = dir.join(format!(".TASKS.md.{pid}.tmp"));
    let id_tmp = dir.join(format!(".TASKS.id.{pid}.tmp"));
    let wrote = (|| -> Result<(), GuidedError> {
        task_render_file(root, slug, &view_tmp)?;
        let id = task_contract_id(root, slug)?;
        fs::write(&id_tmp, format!("{id}\n"))?;
        fs::rename(&view_tmp, dir.join("TASKS.md"))?;
        fs::rename(&id_tmp, dir.join("TASKS.id"))?;
        Ok(())
    })();
    if wrote.is_err() {
        let _ = fs::remove_file(&view_tmp);
        let _ = fs::remove_file(&id_tmp);
    }
    wrote
}

/// `crucible task list|ready|dispatch|integrate`.
pub fn task(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if !uses_managed_lifecycle(root)? {
        return Err(message("task requires managed lifecycle behavior"));
    }
    let sub = args.first().copied().unwrap_or("");
    let slug = args.get(1).copied().unwrap_or("");
    if slug.is_empty() {
        return Err(message(
            "usage: crucible task list|ready|dispatch|integrate ITEM ...",
        ));
    }
    let dir = need(root, slug)?;
    if !dir.join("TASKS.tsv").is_file() {
        return Err(message(format!("item has no task DAG: {slug}")));
    }
    validate_task_dag(root, slug)?;
    task_assert_frozen(root, slug)?;
    match sub {
        "list" => render_stdout(root, slug),
        "ready" => ready_view(root, slug),
        "dispatch" => dispatch_task(root, clock, args, slug),
        "integrate" => {
            if args.len() != 2 {
                return Err(message("usage: crucible task integrate ITEM"));
            }
            integrate_tasks(root, clock, slug)
        }
        _ => Err(message(
            "usage: crucible task list|ready|dispatch|integrate ITEM ...",
        )),
    }
}

fn render_stdout(root: &Path, slug: &str) -> Result<String, GuidedError> {
    let tmp = std::env::temp_dir().join(format!(
        "crucible-task-render-{}-{}",
        std::process::id(),
        RENDER_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let wrote = (|| -> Result<String, GuidedError> {
        task_render_file(root, slug, &tmp)?;
        Ok(fs::read_to_string(&tmp)?)
    })();
    let _ = fs::remove_file(&tmp);
    wrote
}

fn ready_view(root: &Path, slug: &str) -> Result<String, GuidedError> {
    if task_all_pass(root, slug)? {
        return Ok("READY INTEGRATE\n".to_string());
    }
    let text = fs::read_to_string(item_dir(root, slug).join("TASKS.tsv"))?;
    let mut out = String::new();
    let mut found = false;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields = split_row(rec);
        let task_id = fields.first().copied().unwrap_or("");
        let deps = fields.get(1).copied().unwrap_or("");
        if task_pass_exists(root, slug, task_id)? {
            continue;
        }
        // Shell swallows a lookup error (`2>/dev/null || true`) and treats it as no id.
        if let Some(live) = task_live_attempt(root, slug, task_id).ok().flatten() {
            if crate::cycle::attempt_state(root, &live)? == "RETURNED" {
                out.push_str(&format!("RESULT {task_id} {live}\n"));
                found = true;
            }
            continue;
        }
        if let Some(retry) = task_retry_available(root, slug, task_id).ok().flatten() {
            out.push_str(&format!("RETRY {task_id} {retry}\n"));
            found = true;
            continue;
        }
        if task_dependencies_pass(root, slug, deps)? {
            out.push_str(&format!("READY {task_id}\n"));
            found = true;
        }
    }
    if !found {
        let live = task_live_count(root, slug)?;
        if live == 0 {
            out.push_str("WAIT TASK_DEPENDENCIES\n");
        } else {
            out.push_str(&format!("WAIT TASK_ATTEMPTS {live}\n"));
        }
    }
    Ok(out)
}

fn dispatch_task(
    root: &Path,
    clock: &dyn Clock,
    args: &[&str],
    slug: &str,
) -> Result<String, GuidedError> {
    let task_id = args.get(2).copied().unwrap_or("");
    let agent = args.get(3).copied().unwrap_or("");
    let criterion = args.get(4).copied().unwrap_or("");
    if task_id.is_empty() || agent.is_empty() || criterion.is_empty() {
        return Err(message(
            "usage: crucible task dispatch ITEM TASK AGENT CRITERION [EVIDENCE_CLASS] [RETRY_OF]",
        ));
    }
    let class = match args.get(5).copied() {
        Some(value) if !value.is_empty() => value,
        _ => "FOCUSED",
    };
    let retry = match args.get(6).copied() {
        Some(value) if !value.is_empty() => value,
        _ => "-",
    };
    dispatch(
        root,
        &[slug, "maker", agent, criterion, class, retry, task_id],
        clock,
    )
}

fn integrate_tasks(root: &Path, clock: &dyn Clock, slug: &str) -> Result<String, GuidedError> {
    let dir = item_dir(root, slug);
    if phase_of(root, slug)? != "BUILD" {
        return Err(message("task integration requires BUILD"));
    }
    if task_live_count(root, slug)? != 0 {
        return Err(message("task attempts are still live"));
    }
    if !task_all_pass(root, slug)? {
        return Err(message("task integration requires PASS for every task"));
    }
    if !dir.join("TARGET").is_file() {
        return Err(message("task integration requires a Git target"));
    }
    if dir.join("INTEGRATION.tsv").exists() {
        return Err(message("task integration is already recorded"));
    }
    let repo = tgt(root, slug, "repo")?;
    let item_branch = tgt(root, slug, "branch")?;
    if !git_quiet(&repo, &["rev-parse", "--verify", "--quiet", &item_branch]) {
        return Err(message(format!(
            "item integration branch does not exist: {item_branch}"
        )));
    }
    let worktree = root.join("worktrees").join(format!("integrate-{slug}"));
    if worktree.exists() {
        return Err(message(format!(
            "integration worktree already exists: {}",
            worktree.display()
        )));
    }
    fs::create_dir_all(root.join("worktrees"))?;
    if !git_quiet(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            &worktree.display().to_string(),
            &item_branch,
        ],
    ) {
        return Err(message(format!(
            "could not create integration worktree for {item_branch}"
        )));
    }
    let tmp = dir.join(format!(".INTEGRATION.{}.tmp", std::process::id()));
    let mut body = String::from("task_id\ttask_result\tsource_commit\tintegration_commit\n");
    for task_id in task_topological_order(root, slug)? {
        let result_path = task_result_file(root, slug, &task_id)?;
        let text = match &result_path {
            Some(path) => fs::read_to_string(path)?,
            None => String::new(),
        };
        let dispatch_wid = crate::attempt::result_field(&text, "DISPATCH-WORK-ID");
        let output_wid = crate::attempt::result_field(&text, "WORK-ID");
        if !git_quiet(
            &repo,
            &["merge-base", "--is-ancestor", &dispatch_wid, &output_wid],
        ) {
            state_attempt_update(root, clock, slug, "BLOCKED", "-", "INTEGRATION_CONFLICT")?;
            return Err(message(format!(
                "task {task_id} result is not descended from its dispatch base"
            )));
        }
        let commits = git_lines(
            &repo,
            &[
                "rev-list",
                "--reverse",
                &format!("{dispatch_wid}..{output_wid}"),
            ],
        )?;
        if commits.is_empty() {
            state_attempt_update(root, clock, slug, "BLOCKED", "-", "TASK_BLOCKED")?;
            return Err(message(format!(
                "task {task_id} has no commits to integrate"
            )));
        }
        for commit in &commits {
            apply_commit(root, clock, slug, &task_id, &worktree, commit)?;
        }
        let integrated = git_stdout_12(
            &worktree_repo(&worktree),
            &["rev-parse", "--verify", "HEAD"],
        )?;
        let rel = match &result_path {
            Some(path) => rel_under(root, path),
            None => String::new(),
        };
        body.push_str(&format!("{task_id}\t{rel}\t{output_wid}\t{integrated}\n"));
    }
    fs::write(&tmp, &body)?;
    fs::rename(&tmp, dir.join("INTEGRATION.tsv"))?;
    let integrated = workid(root, slug)?;
    let risk = state_value(root, slug, 5)?.unwrap_or_default();
    state_update_item(
        root,
        clock,
        slug,
        "ACTIVE",
        "BUILD",
        &integrated,
        &risk,
        "-",
        "-",
    )?;
    Ok(format!(
        "integrated {slug} at {integrated}; worktree retained at {}\n",
        worktree.display()
    ))
}

fn worktree_repo(worktree: &Path) -> String {
    worktree.display().to_string()
}

fn apply_commit(
    root: &Path,
    clock: &dyn Clock,
    slug: &str,
    task_id: &str,
    worktree: &Path,
    commit: &str,
) -> Result<(), GuidedError> {
    let (ok, cherry_err) = git_merged(&worktree_repo(worktree), &["cherry-pick", commit])?;
    if ok {
        return Ok(());
    }
    let unmerged = git_stdout(
        &worktree_repo(worktree),
        &["diff", "--name-only", "--diff-filter=U"],
    )
    .unwrap_or_default();
    if !unmerged.trim().is_empty() {
        state_attempt_update(root, clock, slug, "BLOCKED", "-", "INTEGRATION_CONFLICT")?;
        eprintln!("task {task_id} conflict retained in {}", worktree.display());
        return Err(message(format!(
            "integration conflict while applying {commit}"
        )));
    }
    Err(message(format!(
        "could not apply {commit} in {}, which is left mid-cherry-pick, and git did not report a conflict: {cherry_err}",
        worktree.display()
    )))
}

fn git_lines(repo: &str, args: &[&str]) -> Result<Vec<String>, GuidedError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(message(if err.is_empty() {
            format!("git {} failed", args.first().copied().unwrap_or("rev-list"))
        } else {
            err.to_string()
        }));
    }
    // stdout only: a hint on stderr must not become a commit id.
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn git_stdout_12(repo: &str, args: &[&str]) -> Result<String, GuidedError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        return Err(message(if err.is_empty() {
            "git rev-parse failed".to_string()
        } else {
            err.to_string()
        }));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(12)
        .collect())
}

fn git_stdout(repo: &str, args: &[&str]) -> Result<String, GuidedError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stderr(Stdio::null())
        .output()?;
    if !out.status.success() {
        return Err(message("git diff failed"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_merged(repo: &str, args: &[&str]) -> Result<(bool, String), GuidedError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    let mut text = String::from_utf8_lossy(&out.stderr).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stdout));
    while text.ends_with('\n') || text.ends_with('\r') {
        text.pop();
    }
    Ok((out.status.success(), text))
}

fn rel_under(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|rest| rest.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn split_row(rec: &str) -> Vec<&str> {
    if rec.is_empty() {
        Vec::new()
    } else {
        rec.split('\t').collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cycle::self_path;
    use crate::state::STATE_HEADER;
    use crucible_contract::FixedClock;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-task-{}-{}",
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

    fn item_md() -> &'static str {
        "\
# alpha — bounded parallel tasks

## Goal

Freeze the graph.

## Non-goals

No launch.

## Risk

MEDIUM

## Owned files

- src/one
- src/two
- src/three

## Acceptance criteria

- [ ] A1: malformed task graphs refuse before BUILD.

## Focused falsifier

scripts/verify-task-dag.sh

## Expensive evidence

NONE

## Stop conditions

Stop if task dispatch can bypass the frozen graph.
"
    }

    fn write_verify(dir: &Path, task: &str) {
        let path = dir.join(format!("tasks/{task}.verify.sh"));
        fs::write(&path, "#!/bin/sh\ntest -n \"$1\"\n").unwrap();
        let mut perm = fs::metadata(&path).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&path, perm).unwrap();
    }

    fn valid_dag(root: &Path) {
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tDRAFT\tw1\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        let dir = root.join("items/alpha");
        fs::create_dir_all(dir.join("tasks")).unwrap();
        fs::create_dir_all(dir.join("work")).unwrap();
        fs::write(dir.join("ITEM.md"), item_md()).unwrap();
        fs::write(
            dir.join("TASKS.tsv"),
            "\
task_id\tdepends_on\tpaths_file\tverify_script
T1\t-\ttasks/T1.paths\ttasks/T1.verify.sh
T2\tT1\ttasks/T2.paths\ttasks/T2.verify.sh
T3\tT1\ttasks/T3.paths\ttasks/T3.verify.sh
",
        )
        .unwrap();
        fs::write(dir.join("tasks/T1.paths"), "src/one\n").unwrap();
        fs::write(dir.join("tasks/T2.paths"), "src/two\n").unwrap();
        fs::write(dir.join("tasks/T3.paths"), "src/three\n").unwrap();
        for task in ["T1", "T2", "T3"] {
            write_verify(&dir, task);
        }
    }

    #[test]
    fn dangling_edge_and_cycle_refuse_like_verify_task_dag() {
        let clock = FixedClock::new(1_700_000_000);
        let tmp = Tmp::new();
        valid_dag(&tmp.0);
        let tasks = tmp.0.join("items/alpha/TASKS.tsv");
        let original = fs::read_to_string(&tasks).unwrap();
        let dangling = original.replace("T2\tT1\t", "T2\tT9\t");
        fs::write(&tasks, dangling).unwrap();
        assert_eq!(
            ready(&tmp.0, &["alpha"], &clock).unwrap_err().to_string(),
            "invalid TASKS.tsv: unknown dependency T9 for T2"
        );

        let cyc = original
            .replace("T1\t-\t", "T1\tT2\t")
            .replace("T3\tT1\t", "T3\t-\t");
        fs::write(&tasks, cyc).unwrap();
        assert_eq!(
            ready(&tmp.0, &["alpha"], &clock).unwrap_err().to_string(),
            "invalid TASKS.tsv: dependency cycle"
        );
    }

    #[test]
    fn ready_freezes_and_exposes_only_the_root_task() {
        let clock = FixedClock::new(1_700_000_000);
        let tmp = Tmp::new();
        valid_dag(&tmp.0);
        assert_eq!(
            ready(&tmp.0, &["alpha"], &clock).unwrap(),
            "alpha is now READY\n"
        );
        assert!(tmp.0.join("items/alpha/TASKS.id").is_file());
        assert!(tmp.0.join("items/alpha/TASKS.md").is_file());
        let listed = task(&tmp.0, &["list", "alpha"], &clock).unwrap();
        assert!(listed.starts_with("# TASKS — alpha\n"), "{listed}");
        assert_eq!(
            task(&tmp.0, &["ready", "alpha"], &clock).unwrap(),
            "READY T1\n"
        );
        let sp = self_path(&tmp.0);
        assert_eq!(
            crate::inspect::next(&tmp.0).unwrap(),
            format!("NEXT alpha BUILD {sp} phase alpha BUILD\n")
        );
    }

    #[test]
    fn unmanaged_ready_and_task_use_the_shell_die_strings() {
        let clock = FixedClock::new(1);
        let tmp = Tmp::new();
        assert_eq!(
            ready(&tmp.0, &["alpha"], &clock).unwrap_err().to_string(),
            "ready requires managed lifecycle behavior"
        );
        assert_eq!(
            task(&tmp.0, &["list", "alpha"], &clock)
                .unwrap_err()
                .to_string(),
            "task requires managed lifecycle behavior"
        );
    }
}
