//! Attempt ledger commands: transport, reclaim, start, finish, overdue,
//! stop, resume, restart, and correct.
//!
//! Epochs in `events.tsv` come from [`Clock::now_unix`] inside [`attempt_event`].
//! Not `SystemTime`. Stop signals the recorded pid only. It does not scan
//! processes and it does not start `orchestrate run`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crucible_contract::Clock;

use crate::claims::{require_attempt_independence, require_panel_approval};
use crate::cycle::{
    attempt_child_dirs, attempt_dir, attempt_event, attempt_meta, attempt_pid, attempt_pid_alive,
    attempt_resume_event, attempt_state, is_claim_slug, reclaim_dead_attempt, self_path,
    state_attempt_update, valid_attempt_id,
};
use crate::panel::{acp_probe_failed, is_posint, is_regular, transport_ladder_ok};
use crate::program::{uses_guided_cycle, uses_managed_lifecycle};
use crate::{message, records, GuidedError};

const USAGE: &str = "usage: crucible attempt start|overdue|finish|transport|reclaim|stop|resume|restart|correct ATTEMPT ...";

/// `crucible attempt`. Stdout on success. Refusals are the shell `die` strings.
pub fn attempt(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if !uses_managed_lifecycle(root)? {
        return Err(message("attempt requires managed lifecycle behavior"));
    }
    let mut rest = args;
    let sub = shift(&mut rest);
    let id = shift(&mut rest);
    if id.is_empty() {
        return Err(message(USAGE));
    }
    let _ad = attempt_dir(root, id)?;
    let slug = attempt_meta(root, id, 2)?;
    match sub {
        "start" => attempt_start(root, clock, id, rest),
        "transport" => attempt_transport_record(root, id, rest),
        "overdue" => attempt_overdue(root, clock, id, rest),
        "finish" => attempt_finish(root, clock, id, &slug, rest),
        "reclaim" => attempt_reclaim(root, clock, id, rest),
        "stop" => attempt_stop(root, clock, id, rest),
        "resume" => attempt_resume(root, clock, id, rest),
        "restart" => attempt_restart(root, clock, id, rest),
        "correct" => attempt_correct(root, clock, id, rest),
        _ => Err(message(USAGE)),
    }
}

fn shift<'a>(args: &mut &'a [&'a str]) -> &'a str {
    if args.is_empty() {
        ""
    } else {
        let head = args[0];
        *args = &args[1..];
        head
    }
}

fn attempt_start(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    args: &[&str],
) -> Result<String, GuidedError> {
    if args.len() != 1 {
        return Err(message("usage: crucible attempt start ATTEMPT PID"));
    }
    let value = args[0];
    if !is_posint(value) {
        return Err(message("attempt start requires a positive PID"));
    }
    if attempt_state(root, id)? != "DISPATCHED" {
        return Err(message("attempt start requires DISPATCHED"));
    }
    // Independence artifacts must be sealed before the process is observed running.
    require_attempt_independence(root, id)?;
    attempt_event(root, clock, id, "RUNNING", value, "observed-start")?;
    Ok(format!("{id} RUNNING pid {value}\n"))
}

fn attempt_transport_record(root: &Path, id: &str, args: &[&str]) -> Result<String, GuidedError> {
    if args.len() != 1 {
        return Err(message(
            "usage: crucible attempt transport ATTEMPT multi-agent|acp|subagent",
        ));
    }
    let value = args[0];
    if !matches!(value, "multi-agent" | "acp" | "subagent") {
        return Err(message("transport must be multi-agent, acp, or subagent"));
    }
    if attempt_state(root, id)? != "DISPATCHED" {
        return Err(message(
            "refused: transport may only be recorded while DISPATCHED (before attempt start)",
        ));
    }
    require_panel_approval(root)?;
    if uses_guided_cycle(root)? {
        enforce_transport_ladder(root, value)?;
    } else if value == "subagent" && !acp_probe_failed(root)? {
        return Err(message(
            "refused: subagent requires a recorded ACP probe failure (ACP-PROBE.md status: failed, or PANEL notes ACP unavailable)",
        ));
    }
    let ad = attempt_dir(root, id)?;
    let path = ad.join("transport");
    if path.exists() {
        if !is_regular(&path) {
            return Err(message(format!("transport is immutable: {id}")));
        }
        let existing = first_line(&fs::read_to_string(&path)?).replace('\r', "");
        if existing != value {
            return Err(message(format!("transport is immutable: {id}")));
        }
        return Ok(format!("{id} transport {value} (already recorded)\n"));
    }
    let tmp = ad.join(format!(".transport.{}.tmp", std::process::id()));
    fs::write(&tmp, format!("{value}\n"))?;
    fs::rename(&tmp, &path)?;
    Ok(format!("{id} transport {value}\n"))
}

fn attempt_overdue(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    args: &[&str],
) -> Result<String, GuidedError> {
    if !args.is_empty() {
        return Err(message("usage: crucible attempt overdue ATTEMPT"));
    }
    if attempt_state(root, id)? != "RUNNING" {
        return Err(message("attempt overdue requires RUNNING"));
    }
    let deadline = attempt_meta(root, id, 12)?;
    let now = clock.now_unix();
    let passed = deadline.parse::<i64>().ok().is_some_and(|at| now >= at);
    if !passed {
        return Err(message(format!(
            "refused: deadline {deadline} has not passed"
        )));
    }
    let pid = attempt_pid(root, id)?;
    attempt_event(root, clock, id, "OVERDUE", &pid, "deadline-passed")?;
    Ok(format!(
        "{id} OVERDUE; process outcome is still unobserved\n"
    ))
}

fn attempt_finish(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    slug: &str,
    args: &[&str],
) -> Result<String, GuidedError> {
    if args.is_empty() {
        return Err(message(
            "finish state must be RETURNED TIMEOUT STOPPED or ABANDONED",
        ));
    }
    let value = args[0];
    if !matches!(value, "RETURNED" | "TIMEOUT" | "STOPPED" | "ABANDONED") {
        return Err(message(
            "finish state must be RETURNED TIMEOUT STOPPED or ABANDONED",
        ));
    }
    let mut reason = args[1..].join(" ");
    let current = attempt_state(root, id)?;
    match current.as_str() {
        "RUNNING" | "OVERDUE" => {}
        "DISPATCHED" => {
            let pid = attempt_pid(root, id)?;
            if pid != "-" {
                return Err(message(format!(
                    "attempt finish requires RUNNING or OVERDUE: {id} is DISPATCHED with pid {pid} — record the start first"
                )));
            }
            if !matches!(value, "STOPPED" | "ABANDONED") {
                return Err(message(format!(
                    "attempt finish requires RUNNING or OVERDUE for {value}: {id} never started, so record STOPPED or ABANDONED instead"
                )));
            }
            if reason.is_empty() {
                return Err(message(format!(
                    "refused: ending a never-started attempt requires an observation: {} attempt finish {id} {value} \"<what you observed>\"",
                    self_path(root)
                )));
            }
        }
        _ => return Err(message("attempt finish requires RUNNING or OVERDUE")),
    }
    if reason.is_empty() {
        reason = "observed".to_string();
    }
    let pid = attempt_pid(root, id)?;
    attempt_event(root, clock, id, value, &pid, &reason)?;
    let retry = attempt_meta(root, id, 13)?;
    let task = attempt_meta(root, id, 3)?;
    match value {
        "RETURNED" => Ok(format!("{id} RETURNED; record its result next\n")),
        "TIMEOUT" => finish_timeout(root, clock, id, slug, &task, &retry),
        _ => finish_stopped(root, clock, id, slug, value, &current, &task),
    }
}

fn finish_timeout(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    slug: &str,
    task: &str,
    retry: &str,
) -> Result<String, GuidedError> {
    if is_claim_slug(slug) {
        return Ok(format!("{id} TIMEOUT; claim attempt has no item state\n"));
    }
    if task != "-" && retry == "-" {
        state_attempt_update(root, clock, slug, "ACTIVE", "TASKS", "RETRY_AVAILABLE")?;
        Ok(format!("{id} TIMEOUT; one task retry is available\n"))
    } else if task != "-" {
        state_attempt_update(root, clock, slug, "BLOCKED", "-", "RETRY_EXHAUSTED")?;
        Ok(format!("{id} TIMEOUT; item blocked RETRY_EXHAUSTED\n"))
    } else if retry == "-" {
        state_attempt_update(root, clock, slug, "ACTIVE", id, "RETRY_AVAILABLE")?;
        Ok(format!("{id} TIMEOUT; one retry is available\n"))
    } else {
        state_attempt_update(root, clock, slug, "BLOCKED", "-", "RETRY_EXHAUSTED")?;
        Ok(format!("{id} TIMEOUT; item blocked RETRY_EXHAUSTED\n"))
    }
}

fn finish_stopped(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    slug: &str,
    value: &str,
    current: &str,
    task: &str,
) -> Result<String, GuidedError> {
    if is_claim_slug(slug) {
        return Ok(format!("{id} {value}; claim attempt has no item state\n"));
    }
    if current == "DISPATCHED" {
        // Nothing ran. Release the pointer the never-started dispatch left behind.
        let inflight = state_value_or_empty(root, slug, 6)?;
        if inflight == id {
            let status = state_value_or_empty(root, slug, 2)?;
            let block = state_value_or_empty(root, slug, 7)?;
            state_attempt_update(root, clock, slug, &status, "-", &block)?;
            return Ok(format!(
                "{id} {value}; never started, in-flight pointer released for {slug}\n"
            ));
        }
        return Ok(format!(
            "{id} {value}; never started, {slug} was not pinned to it\n"
        ));
    }
    let block_code = if task != "-" {
        "TASK_BLOCKED"
    } else {
        "OPERATOR_DECISION"
    };
    state_attempt_update(root, clock, slug, "BLOCKED", "-", block_code)?;
    Ok(format!("{id} {value}; item blocked {block_code}\n"))
}

fn attempt_stop(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    args: &[&str],
) -> Result<String, GuidedError> {
    if !args.is_empty() {
        return Err(message("usage: crucible attempt stop ATTEMPT"));
    }
    let current = attempt_state(root, id)?;
    if !matches!(current.as_str(), "RUNNING" | "OVERDUE") {
        return Err(message("attempt stop requires RUNNING or OVERDUE"));
    }
    let pid = attempt_pid(root, id)?;
    if signalable_pid(&pid)? {
        signal_term(&pid)?;
    }
    attempt_event(root, clock, id, "STOPPED", &pid, "operator-stop")?;
    let slug = attempt_meta(root, id, 2)?;
    let task = attempt_meta(root, id, 3)?;
    finish_stopped(root, clock, id, &slug, "STOPPED", &current, &task)
}

/// `-` or empty is not a process. A pid below 2, or not a number, is refused.
fn signalable_pid(pid: &str) -> Result<bool, GuidedError> {
    if pid.is_empty() || pid == "-" {
        return Ok(false);
    }
    match pid.parse::<u32>() {
        Ok(n) if n >= 2 => Ok(true),
        _ => Err(message(format!("refusing pid {pid}"))),
    }
}

fn signal_term(pid: &str) -> Result<(), GuidedError> {
    let status = Command::new("kill")
        .args(["-TERM", pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| message(format!("attempt stop could not signal pid {pid}: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(message(format!("attempt stop could not signal pid {pid}")))
    }
}

fn attempt_resume(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    args: &[&str],
) -> Result<String, GuidedError> {
    if !args.is_empty() {
        return Err(message("usage: crucible attempt resume ATTEMPT"));
    }
    if attempt_state(root, id)? != "STOPPED" {
        return Err(message("attempt resume requires STOPPED"));
    }
    if !attempt_pid_alive(root, id)? {
        return Err(message(format!(
            "resume requires the recorded pid of {id} to be alive"
        )));
    }
    let pid = attempt_pid(root, id)?;
    attempt_resume_event(root, clock, id, &pid)?;
    let slug = attempt_meta(root, id, 2)?;
    if !is_claim_slug(&slug) {
        state_attempt_update(root, clock, &slug, "ACTIVE", id, "-")?;
    }
    Ok(format!("{id} RUNNING pid {pid}\n"))
}

fn attempt_restart(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    args: &[&str],
) -> Result<String, GuidedError> {
    if args.len() != 1 {
        return Err(message("usage: crucible attempt restart ATTEMPT NEW"));
    }
    let new_id = args[0];
    if !valid_attempt_id(new_id) {
        return Err(message(format!("invalid attempt id: {new_id}")));
    }
    let new_dir = root.join("attempts").join(new_id);
    if new_dir.exists() {
        return Err(message(format!("attempt {new_id} already exists")));
    }
    if attempt_state(root, id)? != "STOPPED" {
        return Err(message("attempt restart requires STOPPED"));
    }
    let old = attempt_dir(root, id)?;
    let meta = retry_meta(&fs::read_to_string(old.join("meta.tsv"))?, new_id, id)?;
    fs::create_dir(&new_dir)?;
    fs::write(new_dir.join("meta.tsv"), meta)?;
    fs::write(
        new_dir.join("events.tsv"),
        format!(
            "state\tepoch\tpid\treason\nDISPATCHED\t{}\t-\trestart\n",
            clock.now_unix()
        ),
    )?;
    let slug = attempt_meta(root, id, 2)?;
    if !is_claim_slug(&slug) {
        state_attempt_update(root, clock, &slug, "ACTIVE", new_id, "-")?;
    }
    Ok(format!("{new_id} DISPATCHED retry of {id}\n"))
}

fn retry_meta(text: &str, new_id: &str, old_id: &str) -> Result<String, GuidedError> {
    let mut lines: Vec<String> = crate::records(text)
        .into_iter()
        .map(str::to_string)
        .collect();
    if lines.len() < 2 {
        return Err(message(format!("attempt {old_id} meta is unreadable")));
    }
    let mut fields: Vec<String> = lines[1].split('\t').map(str::to_string).collect();
    while fields.len() < 13 {
        fields.push("-".to_string());
    }
    fields[0] = new_id.to_string();
    fields[9] = "DISPATCHED".to_string();
    fields[12] = old_id.to_string();
    lines[1] = fields.join("\t");
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

fn attempt_correct(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    args: &[&str],
) -> Result<String, GuidedError> {
    if args.is_empty() {
        return Err(message("usage: crucible attempt correct ATTEMPT TEXT"));
    }
    let text = args.join(" ");
    // Kind `correction` does not add an orders row and does not clear a pause.
    // The path `append` prints is the messages file; this command does not repeat it.
    crate::messages::append(root, &["manager", "correction", &text], clock)?;
    Ok(format!("{id} correction recorded\n"))
}

fn attempt_reclaim(
    root: &Path,
    clock: &dyn Clock,
    id: &str,
    args: &[&str],
) -> Result<String, GuidedError> {
    if !args.is_empty() {
        return Err(message("usage: crucible attempt reclaim ATTEMPT"));
    }
    let slug = attempt_meta(root, id, 2)?;
    let task = attempt_meta(root, id, 3)?;
    let msg = if is_claim_slug(&slug) {
        format!("{id} STOPPED; dead pid reclaimed (claim attempt)\n")
    } else {
        let block_code = if task != "-" {
            "TASK_BLOCKED"
        } else {
            "OPERATOR_DECISION"
        };
        format!("{id} STOPPED; dead pid reclaimed; item blocked {block_code}\n")
    };
    reclaim_dead_attempt(root, clock, id)?;
    Ok(msg)
}

fn state_value_or_empty(root: &Path, slug: &str, column: usize) -> Result<String, GuidedError> {
    Ok(crate::state::state_value(root, slug, column)?.unwrap_or_default())
}

/// Same refusals as the shell: subagent without a recorded ACP failure, or not a transport.
pub(crate) fn enforce_transport_ladder(root: &Path, transport: &str) -> Result<(), GuidedError> {
    match transport_ladder_ok(root, transport)? {
        0 => Ok(()),
        1 => Err(message(
            "refused: subagent requires a recorded ACP probe failure (ACP-PROBE.md status: failed, or PANEL notes ACP unavailable)",
        )),
        _ => Err(message("refused: transport must be multi-agent|acp|subagent")),
    }
}

fn first_line(text: &str) -> String {
    records(text).into_iter().next().unwrap_or("").to_string()
}

pub(crate) fn result_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for dir in attempt_child_dirs(root) {
        let path = dir.join("result.md");
        if path.is_file() {
            paths.push(path);
        }
    }
    paths
}

pub(crate) fn result_field(text: &str, key: &str) -> String {
    let prefix = format!("{key}: ");
    records(text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string()
}

fn read_result(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// First non-terminal attempt with this item, role, criterion, and task. Errors skip, matching
/// the caller's `|| true`.
pub(crate) fn attempt_live_for(
    root: &Path,
    slug: &str,
    role: &str,
    criterion: &str,
    task: &str,
) -> Option<String> {
    for dir in attempt_child_dirs(root) {
        let id = crate::cycle::file_name(&dir);
        let Ok(item) = attempt_meta(root, &id, 2) else {
            continue;
        };
        if item != slug {
            continue;
        }
        let Ok(got_role) = attempt_meta(root, &id, 5) else {
            continue;
        };
        if got_role != role {
            continue;
        }
        let Ok(got_criterion) = attempt_meta(root, &id, 8) else {
            continue;
        };
        if got_criterion != criterion {
            continue;
        }
        let Ok(got_task) = attempt_meta(root, &id, 3) else {
            continue;
        };
        if got_task != task {
            continue;
        }
        let Ok(state) = attempt_state(root, &id) else {
            continue;
        };
        if !crate::cycle::attempt_terminal(&state) {
            return Some(id);
        }
    }
    None
}

pub(crate) fn attempt_pass_exists(
    root: &Path,
    slug: &str,
    wid: &str,
    role: &str,
    agent: &str,
    criterion: &str,
    task: &str,
) -> bool {
    result_files(root).into_iter().any(|path| {
        let Some(text) = read_result(&path) else {
            return false;
        };
        result_field(&text, "OUTCOME") == "PASS"
            && result_field(&text, "ITEM") == slug
            && result_field(&text, "WORK-ID") == wid
            && result_field(&text, "ROLE") == role
            && result_field(&text, "AGENT") == agent
            && result_field(&text, "CRITERION") == criterion
            && result_field(&text, "TASK-ID") == task
    })
}

pub(crate) fn canonical_pass_exists(root: &Path, slug: &str, wid: &str, class: &str) -> bool {
    result_files(root).into_iter().any(|path| {
        let Some(text) = read_result(&path) else {
            return false;
        };
        result_field(&text, "OUTCOME") == "PASS"
            && result_field(&text, "ITEM") == slug
            && result_field(&text, "WORK-ID") == wid
            && result_field(&text, "EVIDENCE-CLASS") == class
    })
}

pub(crate) fn maker_pass_exists(root: &Path, slug: &str, wid: &str) -> bool {
    result_files(root).into_iter().any(|path| {
        let Some(text) = read_result(&path) else {
            return false;
        };
        result_field(&text, "OUTCOME") == "PASS"
            && result_field(&text, "ITEM") == slug
            && result_field(&text, "WORK-ID") == wid
            && result_field(&text, "ROLE") == "maker"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::STATE_HEADER;
    use crucible_contract::FixedClock;
    use std::sync::atomic::{AtomicU64, Ordering};

    const EPOCH: i64 = 1_700_000_000;

    struct Tmp(PathBuf);

    impl Tmp {
        fn new() -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "crucible-attempt-{}-{}",
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

    fn clock() -> FixedClock {
        FixedClock::new(EPOCH)
    }

    fn managed(root: &Path) {
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(root.join("STATE.tsv"), format!("{STATE_HEADER}\n")).unwrap();
    }

    #[allow(clippy::too_many_arguments)] // one column per attempt meta field
    fn seed_attempt(
        root: &Path,
        id: &str,
        slug: &str,
        task: &str,
        state: &str,
        pid: &str,
        deadline: i64,
        retry: &str,
    ) {
        let ad = root.join("attempts").join(id);
        fs::create_dir_all(&ad).unwrap();
        fs::write(
            ad.join("meta.tsv"),
            format!(
                "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\n{id}\t{slug}\t{task}\twid\tmaker\tada\tgrok\tA1\tFOCUSED\tDISPATCHED\t{EPOCH}\t{deadline}\t{retry}\n"
            ),
        )
        .unwrap();
        fs::write(
            ad.join("events.tsv"),
            format!("state\tepoch\tpid\treason\n{state}\t{EPOCH}\t{pid}\tseed\n"),
        )
        .unwrap();
    }

    fn state_row(root: &Path, slug: &str, status: &str, stage: &str, inflight: &str, block: &str) {
        fs::write(
            root.join("STATE.tsv"),
            format!(
                "{STATE_HEADER}\n{slug}\t{status}\t{stage}\twid\tLOW\t{inflight}\t{block}\t1\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn overdue_uses_the_fixed_clock_against_the_deadline() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        managed(root);
        let id = "A1700000000.9.1";
        seed_attempt(root, id, "alpha", "-", "RUNNING", "42", EPOCH + 10, "-");
        state_row(root, "alpha", "ACTIVE", "BUILD", id, "-");
        assert_eq!(
            attempt(root, &["overdue", id], &clock())
                .unwrap_err()
                .to_string(),
            format!("refused: deadline {} has not passed", EPOCH + 10)
        );
        let later = FixedClock::new(EPOCH + 10);
        assert_eq!(
            attempt(root, &["overdue", id], &later).unwrap(),
            format!("{id} OVERDUE; process outcome is still unobserved\n")
        );
        let events = fs::read_to_string(root.join("attempts").join(id).join("events.tsv")).unwrap();
        assert!(events.ends_with(&format!("OVERDUE\t{}\t42\tdeadline-passed\n", EPOCH + 10)));
    }

    #[test]
    fn finish_of_a_never_started_attempt_releases_the_pointer() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        managed(root);
        let id = "A1700000000.9.2";
        seed_attempt(root, id, "alpha", "-", "DISPATCHED", "-", EPOCH + 1800, "-");
        state_row(root, "alpha", "ACTIVE", "BUILD", id, "-");
        assert_eq!(
            attempt(root, &["finish", id, "RETURNED"], &clock())
                .unwrap_err()
                .to_string(),
            format!(
                "attempt finish requires RUNNING or OVERDUE for RETURNED: {id} never started, so record STOPPED or ABANDONED instead"
            )
        );
        assert_eq!(
            attempt(
                root,
                &["finish", id, "ABANDONED", "worker", "never", "spawned"],
                &clock()
            )
            .unwrap(),
            format!("{id} ABANDONED; never started, in-flight pointer released for alpha\n")
        );
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(state.contains("alpha\tACTIVE\tBUILD\twid\tLOW\t-\t-\t1700000000\n"));
        let events = fs::read_to_string(root.join("attempts").join(id).join("events.tsv")).unwrap();
        assert!(events.contains("ABANDONED\t1700000000\t-\tworker never spawned\n"));
    }

    #[test]
    fn reclaim_dead_pid_blocks_the_item() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        managed(root);
        let id = "A1700000000.9.3";
        seed_attempt(
            root,
            id,
            "alpha",
            "-",
            "RUNNING",
            "999999",
            EPOCH + 1800,
            "-",
        );
        state_row(root, "alpha", "ACTIVE", "BUILD", id, "-");
        assert_eq!(
            attempt(root, &["reclaim", id], &clock()).unwrap(),
            format!("{id} STOPPED; dead pid reclaimed; item blocked OPERATOR_DECISION\n")
        );
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(
            state.contains("alpha\tBLOCKED\tBUILD\twid\tLOW\t-\tOPERATOR_DECISION\t1700000000\n")
        );
    }

    #[test]
    fn subagent_transport_requires_a_recorded_probe_failure() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::write(root.join("PROGRAM"), "lifecycle: managed\ncycle: guided\n").unwrap();
        fs::write(root.join("STATE.tsv"), format!("{STATE_HEADER}\n")).unwrap();
        fs::write(root.join("PANEL.md"), "panel\n").unwrap();
        fs::write(root.join("PANEL.ASSIGN.tsv"), "role\tagent\n").unwrap();
        fs::write(root.join("agents.tsv"), "ada\tgrok\n").unwrap();
        let panel = crate::panel::panel_id(root).unwrap().unwrap();
        fs::write(root.join("PANEL.APPROVAL"), format!("panel-id: {panel}\n")).unwrap();
        let id = "A1700000000.9.4";
        seed_attempt(root, id, "alpha", "-", "DISPATCHED", "-", EPOCH + 1800, "-");
        assert_eq!(
            attempt(root, &["transport", id, "subagent"], &clock())
                .unwrap_err()
                .to_string(),
            "refused: subagent requires a recorded ACP probe failure (ACP-PROBE.md status: failed, or PANEL notes ACP unavailable)"
        );
        fs::write(
            root.join("ACP-PROBE.md"),
            "status: failed\nprobed-epoch: 1\nnote: none\n",
        )
        .unwrap();
        assert_eq!(
            attempt(root, &["transport", id, "subagent"], &clock()).unwrap(),
            format!("{id} transport subagent\n")
        );
        assert_eq!(
            fs::read_to_string(root.join("attempts").join(id).join("transport")).unwrap(),
            "subagent\n"
        );
    }

    #[cfg(unix)]
    struct Reap(Option<std::process::Child>);

    #[cfg(unix)]
    impl Drop for Reap {
        fn drop(&mut self) {
            if let Some(mut child) = self.0.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn stop_signals_the_recorded_pid_and_refuses_pid_one() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        managed(root);
        let idle = "A1700000000.9.5";
        seed_attempt(
            root,
            idle,
            "beta",
            "-",
            "DISPATCHED",
            "-",
            EPOCH + 1800,
            "-",
        );
        state_row(root, "beta", "ACTIVE", "BUILD", idle, "-");
        assert_eq!(
            attempt(root, &["stop", idle], &clock())
                .unwrap_err()
                .to_string(),
            "attempt stop requires RUNNING or OVERDUE"
        );

        let refused = "A1700000000.9.6";
        seed_attempt(
            root,
            refused,
            "beta",
            "-",
            "RUNNING",
            "1",
            EPOCH + 1800,
            "-",
        );
        let before = fs::read(root.join("attempts").join(refused).join("events.tsv")).unwrap();
        assert_eq!(
            attempt(root, &["stop", refused], &clock())
                .unwrap_err()
                .to_string(),
            "refusing pid 1"
        );
        assert_eq!(
            fs::read(root.join("attempts").join(refused).join("events.tsv")).unwrap(),
            before
        );
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(state.contains(&format!(
            "beta\tACTIVE\tBUILD\twid\tLOW\t{idle}\t-\t1\n"
        )));

        let bare = "A1700000000.9.7";
        seed_attempt(root, bare, "gamma", "-", "RUNNING", "-", EPOCH + 1800, "-");
        state_row(root, "gamma", "ACTIVE", "BUILD", bare, "-");
        assert_eq!(
            attempt(root, &["stop", bare], &clock()).unwrap(),
            format!("{bare} STOPPED; item blocked OPERATOR_DECISION\n")
        );
        let bare_events =
            fs::read_to_string(root.join("attempts").join(bare).join("events.tsv")).unwrap();
        assert!(bare_events.ends_with(&format!("STOPPED\t{EPOCH}\t-\toperator-stop\n")));

        let mut target = Reap(Some(
            Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("spawn sleep"),
        ));
        let mut decoy = Reap(Some(
            Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("spawn decoy sleep"),
        ));
        let target_pid = target.0.as_ref().unwrap().id().to_string();
        let decoy_pid = decoy.0.as_ref().unwrap().id().to_string();
        let id = "A1700000000.9.8";
        seed_attempt(
            root,
            id,
            "alpha",
            "-",
            "RUNNING",
            &target_pid,
            EPOCH + 1800,
            "-",
        );
        state_row(root, "alpha", "ACTIVE", "BUILD", id, "-");
        assert_eq!(
            attempt(root, &["stop", id, "extra"], &clock())
                .unwrap_err()
                .to_string(),
            "usage: crucible attempt stop ATTEMPT"
        );
        assert_eq!(
            attempt(root, &["stop", id], &clock()).unwrap(),
            format!("{id} STOPPED; item blocked OPERATOR_DECISION\n")
        );
        let events = fs::read_to_string(root.join("attempts").join(id).join("events.tsv")).unwrap();
        assert!(events.ends_with(&format!("STOPPED\t{EPOCH}\t{target_pid}\toperator-stop\n")));
        assert!(events
            .lines()
            .all(|line| line.split('\t').nth(2) != Some(decoy_pid.as_str())));
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(
            state.contains("alpha\tBLOCKED\tBUILD\twid\tLOW\t-\tOPERATOR_DECISION\t1700000000\n")
        );
        let mut child = target.0.take().expect("target");
        // The recorded pid was signaled. Reap it before asserting it is gone.
        let status = child.wait().unwrap();
        assert!(!status.success());
        assert!(decoy.0.as_mut().unwrap().try_wait().unwrap().is_none());
    }

    #[test]
    fn resume_requires_the_recorded_pid_to_be_alive() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        managed(root);
        let dead = "A1700000000.9.9";
        seed_attempt(
            root,
            dead,
            "alpha",
            "-",
            "RUNNING",
            "999999",
            EPOCH + 1800,
            "-",
        );
        state_row(root, "alpha", "ACTIVE", "BUILD", dead, "-");
        assert_eq!(
            attempt(root, &["resume", dead], &clock())
                .unwrap_err()
                .to_string(),
            "attempt resume requires STOPPED"
        );
        seed_attempt(
            root,
            dead,
            "alpha",
            "-",
            "STOPPED",
            "999999",
            EPOCH + 1800,
            "-",
        );
        let before = fs::read(root.join("attempts").join(dead).join("events.tsv")).unwrap();
        assert_eq!(
            attempt(root, &["resume", dead], &clock())
                .unwrap_err()
                .to_string(),
            format!("resume requires the recorded pid of {dead} to be alive")
        );
        assert_eq!(
            fs::read(root.join("attempts").join(dead).join("events.tsv")).unwrap(),
            before
        );

        let id = "A1700000000.9.10";
        let pid = std::process::id().to_string();
        seed_attempt(root, id, "alpha", "-", "STOPPED", &pid, EPOCH + 1800, "-");
        state_row(root, "alpha", "BLOCKED", "BUILD", "-", "OPERATOR_DECISION");
        assert_eq!(
            attempt(root, &["resume", id], &clock()).unwrap(),
            format!("{id} RUNNING pid {pid}\n")
        );
        let events = fs::read_to_string(root.join("attempts").join(id).join("events.tsv")).unwrap();
        assert!(events.ends_with(&format!("RUNNING\t{EPOCH}\t{pid}\toperator-resume\n")));
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(state.contains(&format!(
            "alpha\tACTIVE\tBUILD\twid\tLOW\t{id}\t-\t1700000000\n"
        )));
        assert_eq!(
            fs::read(root.join("attempts").join(dead).join("events.tsv")).unwrap(),
            before
        );
    }

    #[test]
    fn restart_records_a_new_dispatched_attempt_without_spawning() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        managed(root);
        let id = "A1700000000.9.11";
        seed_attempt(
            root,
            id,
            "alpha",
            "-",
            "RUNNING",
            "999999",
            EPOCH + 1800,
            "-",
        );
        state_row(root, "alpha", "ACTIVE", "BUILD", id, "-");
        assert_eq!(
            attempt(root, &["restart", id, "A1700000000.9.12"], &clock())
                .unwrap_err()
                .to_string(),
            "attempt restart requires STOPPED"
        );
        seed_attempt(
            root,
            id,
            "alpha",
            "-",
            "STOPPED",
            "999999",
            EPOCH + 1800,
            "-",
        );
        let old_events = fs::read(root.join("attempts").join(id).join("events.tsv")).unwrap();
        assert_eq!(
            attempt(root, &["restart", id, "not-an-id"], &clock())
                .unwrap_err()
                .to_string(),
            "invalid attempt id: not-an-id"
        );
        assert_eq!(
            attempt(root, &["restart", id, id], &clock())
                .unwrap_err()
                .to_string(),
            format!("attempt {id} already exists")
        );
        let new_id = "A1700000000.9.12";
        assert_eq!(
            attempt(root, &["restart", id, new_id], &clock()).unwrap(),
            format!("{new_id} DISPATCHED retry of {id}\n")
        );
        assert_eq!(
            fs::read(root.join("attempts").join(id).join("events.tsv")).unwrap(),
            old_events
        );
        assert_eq!(attempt_state(root, id).unwrap(), "STOPPED");
        assert_eq!(attempt_meta(root, new_id, 13).unwrap(), id);
        assert_eq!(
            fs::read_to_string(root.join("attempts").join(new_id).join("events.tsv")).unwrap(),
            format!("state\tepoch\tpid\treason\nDISPATCHED\t{EPOCH}\t-\trestart\n")
        );
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(state.contains(&format!(
            "alpha\tACTIVE\tBUILD\twid\tLOW\t{new_id}\t-\t1700000000\n"
        )));
    }

    #[test]
    fn correct_records_a_manager_correction_and_leaves_the_pause() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        managed(root);
        let id = "A1700000000.9.13";
        seed_attempt(root, id, "alpha", "-", "RUNNING", "42", EPOCH + 1800, "-");
        fs::write(
            root.join("ORDERS.tsv"),
            "order_id\tdepends_on\tpaths_file\tverify_script\nA\t-\ta.paths\ta.sh\n",
        )
        .unwrap();
        fs::write(
            root.join("MESSAGES.tsv"),
            "epoch\trole\tkind\ttext\n1\tmachine\tneed-a-fact\tA\n",
        )
        .unwrap();
        let orders = fs::read(root.join("ORDERS.tsv")).unwrap();
        let paused = crate::messages::queue(root).unwrap();
        assert_eq!(
            paused,
            "queue\nA paused\ngraph\nA -\npaused\nA\nescalated\n"
        );
        assert_eq!(
            attempt(root, &["correct", id], &clock())
                .unwrap_err()
                .to_string(),
            "usage: crucible attempt correct ATTEMPT TEXT"
        );
        let out = attempt(root, &["correct", id, "use", "the", "door"], &clock()).unwrap();
        assert_eq!(out, format!("{id} correction recorded\n"));
        assert!(!out.contains("MESSAGES"));
        assert_eq!(fs::read(root.join("ORDERS.tsv")).unwrap(), orders);
        assert_eq!(crate::messages::queue(root).unwrap(), paused);
        let messages = fs::read_to_string(root.join("MESSAGES.tsv")).unwrap();
        assert!(messages.contains(&format!("{EPOCH}\tmanager\tcorrection\tuse the door\n")));
    }
}
