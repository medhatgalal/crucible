use std::fs;
use std::path::{Path, PathBuf};

use crucible_contract::{resolve_wm, Clock};

use crate::error::KernelError;
use crate::floor::floor_write;
use crate::metrics::metrics_append;
use crate::paths::first_nonempty_line;
use crate::trace::{go_start, GoStart};

const INDEPENDENCE: &str = "SUBAGENT-ISOLATED";

/// Result of a foreground `go` walk (no daemon).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoRun {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
    pub archived: Option<PathBuf>,
}

/// Minimal walker: `go_start`, then STOP-ASK INTAKE or CLOSED no-op.
///
/// Does not port NEXT RED / BUILD. Stays in-process (no daemon).
pub fn go(dir: impl AsRef<Path>, clock: &dyn Clock) -> Result<GoRun, KernelError> {
    let dir = dir.as_ref();
    let started = go_start(dir, clock)?;
    let (repo, wm) = resolve_wm(dir);

    if !idea_present(&repo) {
        return halt_stop_ask(dir, "STOP-ASK INTAKE", &started, clock);
    }

    let closed_path = wm.join("CLOSED");
    if closed_path.is_file() {
        let text = fs::read_to_string(&closed_path)?;
        let card = first_nonempty_line(&text);
        let card = if card.is_empty() { "DONE" } else { card };
        floor_write(dir, card, INDEPENDENCE, clock)?;
        let stdout = if text.ends_with('\n') || text.is_empty() {
            text
        } else {
            format!("{text}\n")
        };
        return Ok(GoRun {
            exit: 0,
            stdout,
            stderr: String::new(),
            archived: started.archived,
        });
    }

    // Brick loop (NEXT RED / BUILD) is a later slice. Honest refuse, foreground.
    Ok(GoRun {
        exit: 2,
        stdout: String::new(),
        stderr: "go: brick loop not ported\n".to_string(),
        archived: started.archived,
    })
}

fn idea_present(repo: &Path) -> bool {
    let path = repo.join("IDEA.md");
    fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() > 0)
}

fn halt_stop_ask(
    dir: &Path,
    card: &str,
    started: &GoStart,
    clock: &dyn Clock,
) -> Result<GoRun, KernelError> {
    let floor = floor_write(dir, card, INDEPENDENCE, clock)?;
    let (repo, _) = resolve_wm(dir);
    let bound = posix_loop_bound(&repo);
    metrics_append(dir, card, "-", bound, floor.elapsed_s, 0, clock)?;
    Ok(GoRun {
        exit: 1,
        stdout: format!("{card}\n"),
        stderr: String::new(),
        archived: started.archived.clone(),
    })
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
