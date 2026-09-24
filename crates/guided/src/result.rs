//! `crucible result`. Work-id checks shell out to git. No libgit2.

use std::fs;
use std::path::Path;
use std::process::Command;

use crucible_contract::Clock;

use crate::attempt::{result_field, result_files};
use crate::claims::require_attempt_independence;
use crate::cycle::{attempt_dir, attempt_meta, attempt_state, attempt_transport, workid, MARK};
use crate::dispatch::{item_dir, review_relation, task_live_count, task_owns_path, tgt};
use crate::panel::split_tabs;
use crate::program::uses_managed_lifecycle;
use crate::state::{state_update_item, state_value};
use crate::{message, records, GuidedError};

/// `crucible result`.
pub fn result(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if !uses_managed_lifecycle(root)? {
        return Err(message("result requires managed lifecycle behavior"));
    }
    if args.len() < 4 {
        return Err(message(
            "usage: crucible result ATTEMPT OUTCOME EVIDENCE NEXT [FINGERPRINT]",
        ));
    }
    let id = args[0];
    let outcome = args[1];
    let evidence = args[2];
    let next = args[3];
    let fingerprint = if args.len() >= 5 { args[4] } else { "-" };
    if !matches!(
        outcome,
        "PASS" | "REJECT" | "BLOCKED" | "NEEDS_CONTEXT" | "SCOPE_CONFLICT"
    ) {
        return Err(message(format!("invalid result outcome: {outcome}")));
    }
    if !matches!(next, "CLOSE" | "FIX" | "DECIDE" | "ESCALATE") {
        return Err(message(format!("invalid result next action: {next}")));
    }
    if !pair_ok(outcome, next) {
        return Err(message(format!(
            "incompatible result: {outcome} cannot request {next}"
        )));
    }
    if !fingerprint_ok(fingerprint) {
        return Err(message(
            "fingerprint must be - or 12 alphanumeric characters",
        ));
    }
    if outcome == "REJECT" && fingerprint == "-" {
        return Err(message("REJECT requires a finding fingerprint"));
    }
    let ad = attempt_dir(root, id)?;
    let rst = attempt_state(root, id)?;
    if root.join(".drive.lock").is_dir() {
        if rst != "RUNNING" {
            return Err(message(
                "refused: result while drive.lock exists must come from the RUNNING worker, not the coordinator",
            ));
        }
    } else if rst != "RETURNED" {
        return Err(message("result requires a RETURNED attempt"));
    }
    require_attempt_independence(root, id)?;
    let slug = attempt_meta(root, id, 2)?;
    let task = attempt_meta(root, id, 3)?;
    let dispatch_wid = attempt_meta(root, id, 4)?;
    let role = attempt_meta(root, id, 5)?;
    let agent = attempt_meta(root, id, 6)?;
    let kind = attempt_meta(root, id, 7)?;
    let criterion = attempt_meta(root, id, 8)?;
    let class = attempt_meta(root, id, 9)?;
    let relation = if matches!(role.as_str(), "judge" | "adversary") {
        review_relation(root, &slug, &agent)?
    } else {
        "-".to_string()
    };
    let transport = attempt_transport(root, id)?.unwrap_or_else(|| "-".to_string());
    let current_wid = workid(root, &slug)?;
    let wid = match role.as_str() {
        "maker" => maker_wid(root, &ad, &slug, &task, &dispatch_wid, &current_wid)?,
        "judge" | "adversary" => {
            if current_wid != dispatch_wid {
                return Err(message(format!(
                    "work changed during {role} attempt: dispatched {dispatch_wid}, current {current_wid}"
                )));
            }
            dispatch_wid.clone()
        }
        _ => current_wid.clone(),
    };
    if evidence.is_empty() || evidence.contains('/') || evidence.contains("..") {
        return Err(message(
            "evidence must be one filename from the item evidence directory",
        ));
    }
    let ef = item_dir(root, &slug).join("evidence").join(evidence);
    if !crate::panel::is_regular(&ef) {
        return Err(message(format!(
            "missing regular evidence file: {evidence}"
        )));
    }
    let ev = fs::read_to_string(&ef)?;
    let ev_lines = records(&ev);
    if ev_lines.first().copied() != Some(MARK) {
        return Err(message("evidence was not recorded by crucible run"));
    }
    if field_line(&ev_lines, "agent") != agent {
        return Err(message("evidence agent does not match attempt"));
    }
    if field_line(&ev_lines, "work-id") != wid {
        return Err(message("evidence work id does not match attempt"));
    }
    if field_line(&ev_lines, "attempt-id") != id {
        return Err(message("evidence attempt id does not match attempt"));
    }
    let result_path = ad.join("result.md");
    if result_path.exists() {
        if !crate::panel::is_regular(&result_path) {
            return Err(message(format!("attempt result is immutable: {id}")));
        }
        let text = fs::read_to_string(&result_path)?;
        if result_field(&text, "OUTCOME") != outcome
            || result_field(&text, "WORK-ID") != wid
            || result_field(&text, "EVIDENCE") != evidence
            || result_field(&text, "FINDING-FINGERPRINT") != fingerprint
            || result_field(&text, "NEXT") != next
        {
            return Err(message(format!("attempt result is immutable: {id}")));
        }
        if state_value(root, &slug, 6)?.as_deref() != Some(id) {
            return Err(message(format!("attempt result is immutable: {id}")));
        }
        result_finalize(
            root,
            clock,
            id,
            &slug,
            &wid,
            &role,
            &agent,
            outcome,
            evidence,
            fingerprint,
            &task,
        )?;
        return Ok(format!(
            "{}/result.md (reconciled publication)\n",
            ad.display()
        ));
    }
    if task != "-" {
        run_task_verifier(root, &ad, &slug, &task, &wid)?;
    }
    let lock = ad.join(".result.lock");
    if fs::create_dir(&lock).is_err() {
        return Err(message(format!("attempt result is immutable: {id}")));
    }
    let tmp = ad.join(format!(".result.{}.tmp", std::process::id()));
    let body = format!(
        "OUTCOME: {outcome}\nITEM: {slug}\nWORK-ID: {wid}\nDISPATCH-WORK-ID: {dispatch_wid}\nATTEMPT-ID: {id}\nROLE: {role}\nAGENT: {agent}\nKIND: {kind}\nREVIEW-RELATION: {relation}\nCRITERION: {criterion}\nTASK-ID: {task}\nEVIDENCE-CLASS: {class}\nEVIDENCE: {evidence}\nTRANSPORT: {transport}\nFINDING-FINGERPRINT: {fingerprint}\nNEXT: {next}\n\nRecorded result for {id}.\n"
    );
    if let Err(err) = fs::write(&tmp, body) {
        let _ = fs::remove_file(&tmp);
        return Err(err.into());
    }
    if let Err(err) = fs::rename(&tmp, &result_path) {
        let _ = fs::remove_file(&tmp);
        return Err(err.into());
    }
    result_finalize(
        root,
        clock,
        id,
        &slug,
        &wid,
        &role,
        &agent,
        outcome,
        evidence,
        fingerprint,
        &task,
    )?;
    Ok(format!("{}/result.md\n", ad.display()))
}

fn pair_ok(outcome: &str, next: &str) -> bool {
    matches!(
        (outcome, next),
        ("PASS", "CLOSE")
            | ("REJECT", "FIX")
            | ("BLOCKED", "DECIDE")
            | ("BLOCKED", "ESCALATE")
            | ("NEEDS_CONTEXT", "DECIDE")
            | ("NEEDS_CONTEXT", "ESCALATE")
            | ("SCOPE_CONFLICT", "DECIDE")
            | ("SCOPE_CONFLICT", "ESCALATE")
    )
}

fn fingerprint_ok(fingerprint: &str) -> bool {
    fingerprint == "-"
        || (fingerprint.len() == 12 && fingerprint.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn field_line(lines: &[&str], key: &str) -> String {
    let prefix = format!("{key}: ");
    lines
        .iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string()
}

fn maker_wid(
    root: &Path,
    ad: &Path,
    slug: &str,
    task: &str,
    dispatch_wid: &str,
    current_wid: &str,
) -> Result<String, GuidedError> {
    if task != "-" {
        let task_tsv = ad.join("task.tsv");
        if !task_tsv.is_file() {
            return Err(message("task attempt is missing task.tsv"));
        }
        let text = fs::read_to_string(&task_tsv)?;
        let row = records(&text).get(1).copied().unwrap_or("").to_string();
        let fields = split_tabs(&row);
        let task_repo = fields.first().copied().unwrap_or("");
        let task_branch = fields.get(1).copied().unwrap_or("");
        let wid = git_rev12(task_repo, &["rev-parse", "--verify", task_branch]);
        if wid == dispatch_wid {
            return Err(message(
                "task result requires work to change after dispatch",
            ));
        }
        let changed = git_text(
            task_repo,
            &["diff", "--name-only", &format!("{dispatch_wid}..{wid}")],
        );
        let changed = changed.trim_end_matches(['\n', '\r']).to_string();
        if changed.is_empty() {
            return Err(message("task result has no changed paths"));
        }
        let z = git_bytes(
            task_repo,
            &[
                "diff",
                "--name-only",
                "-z",
                &format!("{dispatch_wid}..{wid}"),
            ],
        );
        let nul = z.iter().filter(|b| **b == 0).count();
        let lines = changed.split('\n').count();
        if nul != lines {
            return Err(message("task result contains a newline in a Git path"));
        }
        let mut changed_count = 0usize;
        for changed_path in changed.split('\n') {
            if !task_owns_path(root, slug, task, changed_path)? {
                return Err(message(format!(
                    "task {task} changed unowned path: {changed_path}"
                )));
            }
            changed_count += 1;
        }
        if changed_count == 0 {
            return Err(message("task result has no changed paths"));
        }
        return Ok(wid);
    }
    if current_wid == "EMPTY" || current_wid == "NOBRANCH" {
        return Err(message("maker result requires current work"));
    }
    if current_wid == dispatch_wid {
        return Err(message(
            "maker result requires work to change after dispatch",
        ));
    }
    let item = item_dir(root, slug);
    if item.join("TARGET").is_file() {
        let item_text = fs::read_to_string(item.join("ITEM.md")).unwrap_or_default();
        if records(&item_text)
            .iter()
            .any(|line| line.starts_with("## Owned files"))
        {
            let owned = owned_tokens(&item_text);
            if !owned.is_empty() {
                let repo = tgt(root, slug, "repo")?;
                let base = tgt(root, slug, "base")?;
                let mut diff_base = dispatch_wid.to_string();
                if !git_ok(
                    &repo,
                    &[
                        "rev-parse",
                        "--verify",
                        "--quiet",
                        &format!("{dispatch_wid}^{{commit}}"),
                    ],
                ) {
                    diff_base = base;
                }
                let changed = git_text(
                    &repo,
                    &[
                        "diff",
                        "--name-only",
                        &format!("{diff_base}..{current_wid}"),
                    ],
                );
                for changed_path in changed.split('\n') {
                    if changed_path.is_empty() {
                        continue;
                    }
                    let ok = owned.iter().any(|op| {
                        changed_path == op || changed_path.starts_with(&format!("{op}/"))
                    });
                    if !ok {
                        return Err(message(format!(
                            "refused: admitting this item does not authorize {changed_path} (not in Owned files)"
                        )));
                    }
                }
            }
        }
    }
    Ok(current_wid.to_string())
}

fn owned_tokens(item_md: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut on = false;
    for line in records(item_md) {
        if !on {
            if line.starts_with("## Owned files") {
                on = true;
            }
            continue;
        }
        if line.starts_with("## ") {
            break;
        }
        if let Some(rest) = line.strip_prefix("- ") {
            let rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
            // The shell splits the whole section on IFS, so each whitespace token is a path.
            let raw = rest;
            for tok in raw.split_whitespace() {
                if !tok.is_empty() {
                    tokens.push(tok.to_string());
                }
            }
        }
    }
    tokens
}

fn run_task_verifier(
    root: &Path,
    ad: &Path,
    slug: &str,
    task: &str,
    wid: &str,
) -> Result<(), GuidedError> {
    let text = fs::read_to_string(ad.join("task.tsv"))?;
    let row = records(&text).get(1).copied().unwrap_or("");
    let verify_script = split_tabs(row).get(5).copied().unwrap_or("");
    let script = item_dir(root, slug).join(verify_script);
    let tmp = ad.join(format!(".task-verification.{}.tmp", std::process::id()));
    let mut body = Vec::new();
    let rc = match Command::new(&script).arg(wid).output() {
        Ok(out) => {
            body.extend_from_slice(&out.stdout);
            body.extend_from_slice(&out.stderr);
            out.status.code().unwrap_or(1)
        }
        Err(err) => {
            body.extend(format!("{err}\n").into_bytes());
            127
        }
    };
    body.extend(format!("\nexit: {rc}\n").into_bytes());
    fs::write(&tmp, body)?;
    let dest = ad.join("task-verification.txt");
    fs::rename(&tmp, &dest)?;
    if rc != 0 {
        return Err(message(format!(
            "task verifier failed for {task}; see {}",
            dest.display()
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // same fields the shell result_finalize closes over
fn result_finalize(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    slug: &str,
    wid: &str,
    role: &str,
    agent: &str,
    outcome: &str,
    evidence: &str,
    fingerprint: &str,
    task: &str,
) -> Result<(), GuidedError> {
    let ad = attempt_dir(root, id)?;
    let d = item_dir(root, slug);
    if role == "judge" {
        let word = match outcome {
            "NEEDS_CONTEXT" | "BLOCKED" => "INSUFFICIENT_EVIDENCE",
            other => other,
        };
        let verdicts = d.join("verdicts");
        fs::create_dir_all(&verdicts)?;
        let tmp = verdicts.join(format!(".{agent}.{}.tmp", std::process::id()));
        fs::write(
            &tmp,
            format!("VERDICT: {word}\nWORK-ID: {wid}\nresult {id}; see {evidence}\n"),
        )?;
        fs::rename(&tmp, verdicts.join(format!("{agent}.md")))?;
    }
    let mut repeated = false;
    if outcome == "REJECT" && fingerprint != "-" {
        for path in result_files(root) {
            if path == ad.join("result.md") {
                continue;
            }
            let Some(text) = fs::read_to_string(&path).ok() else {
                continue;
            };
            if result_field(&text, "ITEM") == slug
                && result_field(&text, "FINDING-FINGERPRINT") == fingerprint
            {
                repeated = true;
                break;
            }
        }
    }
    let stage = state_value(root, slug, 3)?.unwrap_or_default();
    let risk = state_value(root, slug, 5)?.unwrap_or_default();
    let (result_status, result_block) = if repeated {
        ("BLOCKED", "REPEATED_FINDING")
    } else {
        match outcome {
            "PASS" | "REJECT" => ("ACTIVE", "-"),
            "BLOCKED" => ("BLOCKED", "RESULT_BLOCKED"),
            "NEEDS_CONTEXT" => ("BLOCKED", "NEEDS_CONTEXT"),
            "SCOPE_CONFLICT" => ("BLOCKED", "SCOPE_CONFLICT"),
            _ => ("ACTIVE", "-"),
        }
    };
    if task != "-" {
        let item_wid = workid(root, slug)?;
        let remaining = task_live_count(root, slug)?;
        let task_inflight = if remaining == 0 { "-" } else { "TASKS" };
        state_update_item(
            root,
            clock,
            slug,
            result_status,
            &stage,
            &item_wid,
            &risk,
            task_inflight,
            result_block,
        )?;
    } else {
        state_update_item(
            root,
            clock,
            slug,
            result_status,
            &stage,
            wid,
            &risk,
            "-",
            result_block,
        )?;
    }
    Ok(())
}

fn git_bytes(repo: &str, args: &[&str]) -> Vec<u8> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo);
    cmd.args(args);
    cmd.output().map(|out| out.stdout).unwrap_or_default()
}

fn git_text(repo: &str, args: &[&str]) -> String {
    String::from_utf8_lossy(&git_bytes(repo, args)).into_owned()
}

fn git_rev12(repo: &str, args: &[&str]) -> String {
    let text = git_text(repo, args);
    text.chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(12)
        .collect()
}

fn git_ok(repo: &str, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
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

    struct Tmp(std::path::PathBuf);

    impl Tmp {
        fn new() -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "crucible-result-{}-{}",
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
    fn judge_result_records_the_file_and_refuses_a_second_outcome() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        let id = "A1700000000.4.1";
        let item = root.join("items/alpha");
        fs::create_dir_all(item.join("work")).unwrap();
        fs::create_dir_all(item.join("evidence")).unwrap();
        fs::create_dir_all(item.join("verdicts")).unwrap();
        fs::write(item.join("work/note.txt"), "changed\n").unwrap();
        let wid = workid(root, "alpha").unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tREVIEW\t{wid}\tLOW\t{id}\t-\t1\n"),
        )
        .unwrap();
        let ad = root.join("attempts").join(id);
        fs::create_dir_all(&ad).unwrap();
        fs::write(
            ad.join("meta.tsv"),
            format!(
                "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\n{id}\talpha\t-\t{wid}\tjudge\tbea\tcodex\tA1\tFOCUSED\tRETURNED\t1\t2\t-\n"
            ),
        )
        .unwrap();
        fs::write(
            ad.join("events.tsv"),
            "state\tepoch\tpid\treason\nDISPATCHED\t1\t-\tseed\nRETURNED\t2\t9\tobserved\n",
        )
        .unwrap();
        fs::write(
            item.join("evidence").join("check.txt"),
            format!("{MARK}\nagent: bea\nwork-id: {wid}\nattempt-id: {id}\n"),
        )
        .unwrap();
        let clock = FixedClock::new(1_700_000_050);
        let out = result(root, &[id, "PASS", "check.txt", "CLOSE"], &clock).unwrap();
        assert_eq!(out, format!("{}/result.md\n", ad.display()));
        let body = fs::read_to_string(ad.join("result.md")).unwrap();
        assert!(body.contains("OUTCOME: PASS\n"));
        assert!(body.contains(&format!("WORK-ID: {wid}\n")));
        assert!(body.contains("REVIEW-RELATION: UNPROVEN\n"));
        assert!(body.contains("NEXT: CLOSE\n"));
        let verdict = fs::read_to_string(item.join("verdicts/bea.md")).unwrap();
        assert_eq!(
            verdict,
            format!("VERDICT: PASS\nWORK-ID: {wid}\nresult {id}; see check.txt\n")
        );
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(state.contains(&format!(
            "alpha\tACTIVE\tREVIEW\t{wid}\tLOW\t-\t-\t1700000050\n"
        )));
        assert_eq!(
            result(
                root,
                &[id, "REJECT", "check.txt", "FIX", "abcdefabcdef"],
                &clock
            )
            .unwrap_err()
            .to_string(),
            format!("attempt result is immutable: {id}")
        );
    }
}
