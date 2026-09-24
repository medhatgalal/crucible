//! Attempt ledger commands: transport, reclaim, start, finish, overdue.
//!
//! Epochs in `events.tsv` come from [`Clock::now_unix`] inside [`attempt_event`].
//! Not `SystemTime`.

use std::fs;
use std::path::{Path, PathBuf};

use crucible_contract::Clock;

use crate::claims::{require_attempt_independence, require_panel_approval};
use crate::cycle::{
    attempt_child_dirs, attempt_dir, attempt_event, attempt_meta, attempt_pid, attempt_state,
    is_claim_slug, reclaim_dead_attempt, self_path, state_attempt_update,
};
use crate::panel::{acp_probe_failed, is_posint, is_regular, transport_ladder_ok};
use crate::program::{uses_guided_cycle, uses_managed_lifecycle};
use crate::{message, records, GuidedError};

const USAGE: &str = "usage: crucible attempt start|overdue|finish|transport|reclaim ATTEMPT ...";

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
}
