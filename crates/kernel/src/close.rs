use std::fs;
use std::path::Path;

use crucible_contract::{format_rfc3339_z, resolve_wm, Clock, Event};

use crate::error::KernelError;
use crate::events::append_event;
use crate::floor::floor_write;
use crate::metrics::metrics_append;
use crate::paths::{ensure_wm, first_nonempty_line};

const INDEPENDENCE: &str = "SUBAGENT-ISOLATED";

/// Result of a kernel close (no worktree teardown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseWalk {
    pub card: String,
    pub station: String,
    pub elapsed_s: i64,
}

/// Stamp `.wm/CLOSED`, FLOOR DONE, halt metrics/EVENTS, and a lesson line.
///
/// POSIX `cmd_close` requires a one-line lesson before writing CLOSED.
/// Does **not** `git worktree remove` (keep-on-failure still holds). Isolation
/// is `SUBAGENT-ISOLATED` (no panel). Does not invoke grok.
pub fn close_walk(
    dir: impl AsRef<Path>,
    outcome: &str,
    lesson: Option<&str>,
    iterations: i64,
    clock: &dyn Clock,
) -> Result<CloseWalk, KernelError> {
    let dir = dir.as_ref();
    let lesson = require_lesson(lesson)?;
    let card = close_card(outcome)?;
    let (repo, _) = resolve_wm(dir);
    let wm = ensure_wm(&repo)?;

    if closed_closeable(&wm) {
        return Err(KernelError::Message("already closed".to_string()));
    }

    fs::write(
        wm.join("CLOSED"),
        format!("{card}\nindependence: {INDEPENDENCE}\n"),
    )?;

    let floor = floor_write(dir, card, INDEPENDENCE, clock)?;
    let bound = posix_loop_bound(&repo);
    metrics_append(dir, card, "-", bound, floor.elapsed_s, iterations, clock)?;
    append_event(
        dir,
        &Event::lesson(format_rfc3339_z(clock.now_unix()), lesson),
    )?;

    Ok(CloseWalk {
        card: floor.card,
        station: floor.station,
        elapsed_s: floor.elapsed_s,
    })
}

fn require_lesson(lesson: Option<&str>) -> Result<&str, KernelError> {
    let Some(raw) = lesson else {
        return Err(KernelError::Message(
            "close without a lesson line".to_string(),
        ));
    };
    if raw.is_empty() {
        return Err(KernelError::Message(
            "close without a lesson line".to_string(),
        ));
    }
    if raw.contains('\n') || raw.contains('\r') {
        return Err(KernelError::Message("lesson must be one line".to_string()));
    }
    Ok(raw)
}

fn close_card(outcome: &str) -> Result<&'static str, KernelError> {
    match outcome {
        "PASS" => Ok("CLOSED PASS"),
        "NO-BUILD" => Ok("CLOSED NO-BUILD"),
        _ => Err(KernelError::Message(
            "close without PASS or worker NO-BUILD".to_string(),
        )),
    }
}

fn closed_closeable(wm: &Path) -> bool {
    let Ok(text) = fs::read_to_string(wm.join("CLOSED")) else {
        return false;
    };
    let line = first_nonempty_line(&text);
    line == "CLOSED PASS" || line == "CLOSED NO-BUILD"
}

fn posix_loop_bound(repo: &Path) -> i64 {
    let n = count_slice_rows(repo);
    (16 + 12 * n).clamp(40, 240)
}

fn count_slice_rows(repo: &Path) -> i64 {
    let Ok(text) = fs::read_to_string(repo.join("slices.tsv")) else {
        return 0;
    };
    text.lines()
        .enumerate()
        .filter(|(i, line)| {
            if *i == 0 {
                return false;
            }
            let t = line.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .count() as i64
}
