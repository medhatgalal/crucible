use std::fs;
use std::path::{Path, PathBuf};

use crucible_contract::{format_rfc3339_z, Clock, Event};

use crate::error::KernelError;
use crate::events::append_event;
use crate::paths::{ensure_wm, parse_t0};

pub const TRACE_HEADER: &str = "when\tcard\toutcome\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoStart {
    pub t0_unix: i64,
    /// Absolute path of the archived previous TRACE, if a data-bearing TRACE existed.
    pub archived: Option<PathBuf>,
}

/// Start a walk: archive previous TRACE (if it has data rows), reset `t0`, rewrite TRACE header.
///
/// Fake-fail of this slice: rewriting TRACE without copying it to `.wm/archive/` first.
pub fn go_start(dir: impl AsRef<Path>, clock: &dyn Clock) -> Result<GoStart, KernelError> {
    let dir = dir.as_ref();
    let wm = ensure_wm(dir)?;
    let trace_path = wm.join("TRACE.tsv");
    let old_t0 = parse_t0(&wm);
    let archived = archive_trace_if_needed(&wm, &trace_path, old_t0, clock)?;

    let t0_unix = clock.now_unix();
    fs::write(wm.join("t0"), format!("{t0_unix}\n"))?;
    fs::write(&trace_path, TRACE_HEADER)?;
    append_event(dir, &Event::walk_start(format_rfc3339_z(t0_unix), t0_unix))?;

    Ok(GoStart { t0_unix, archived })
}

fn archive_trace_if_needed(
    wm: &Path,
    trace_path: &Path,
    old_t0: Option<i64>,
    clock: &dyn Clock,
) -> Result<Option<PathBuf>, KernelError> {
    if !trace_path.is_file() {
        return Ok(None);
    }
    let body = fs::read_to_string(trace_path)?;
    if !trace_has_data_rows(&body) {
        return Ok(None);
    }
    let t0 = old_t0.unwrap_or_else(|| clock.now_unix());
    let iso = format_rfc3339_z(t0);
    let archive_dir = wm.join("archive");
    fs::create_dir_all(&archive_dir)?;
    let dest = archive_dir.join(format!("TRACE-{t0}-{iso}.tsv"));
    fs::copy(trace_path, &dest)?;
    Ok(Some(dest))
}

pub fn ensure_trace_header(wm: &Path) -> Result<(), KernelError> {
    let path = wm.join("TRACE.tsv");
    if !path.is_file() {
        fs::write(&path, TRACE_HEADER)?;
    }
    Ok(())
}

pub fn last_trace_card(wm: &Path) -> Option<String> {
    let text = fs::read_to_string(wm.join("TRACE.tsv")).ok()?;
    let mut last = None;
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if i == 0 && line.starts_with("when") {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let _when = parts.next()?;
        let card = parts.next()?;
        if !card.is_empty() {
            last = Some(card.to_string());
        }
    }
    last
}

fn trace_has_data_rows(text: &str) -> bool {
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if i == 0 && line.starts_with("when") {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        if parts.next().is_some() && parts.next().is_some_and(|c| !c.is_empty()) {
            return true;
        }
    }
    false
}
