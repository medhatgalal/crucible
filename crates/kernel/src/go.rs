use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crucible_contract::{resolve_wm, Clock};

use crate::error::KernelError;
use crate::floor::floor_write;
use crate::metrics::metrics_append;
use crate::paths::first_nonempty_line;
use crate::questions::stop_ask_questions;
use crate::red::next_red;
use crate::trace::{go_start, GoStart};

const INDEPENDENCE: &str = "SUBAGENT-ISOLATED";
const SLICE_ID: &str = "s1";
const RED_SESSION: &str = "go-red";

/// Result of a foreground `go` walk (no daemon).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoRun {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
    pub archived: Option<PathBuf>,
}

/// Minimal walker: `go_start`, STOP-ASK INTAKE, CLOSED no-op, QUESTIONS
/// gate, then one injected `next_red` (no `close_walk`).
///
/// Production has no grok: without `CRUCIBLE_RED_PROGRAM` the brick loop
/// still refuses. Stays in-process (no daemon).
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

    let asked = stop_ask_questions(dir, clock)?;
    if asked.stop {
        return Ok(GoRun {
            exit: asked.exit,
            stdout: format!("{}\n", asked.card),
            stderr: String::new(),
            archived: started.archived,
        });
    }

    let Some((program, args)) = red_program_from_env() else {
        return Ok(GoRun {
            exit: 2,
            stdout: String::new(),
            stderr: "go: brick loop not ported\n".to_string(),
            archived: started.archived,
        });
    };
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let red = next_red(dir, SLICE_ID, &program, &arg_refs, RED_SESSION, clock)?;
    Ok(GoRun {
        exit: red.exit,
        stdout: format!("{}\n", red.card),
        stderr: String::new(),
        archived: started.archived.or(red.archived),
    })
}

/// Injected NEXT RED child. Unset/empty `CRUCIBLE_RED_PROGRAM` means not
/// ported (do not default to grok). `CRUCIBLE_RED_ARGS` is newline-separated
/// argv (`-c` then the `sh -c` body).
fn red_program_from_env() -> Option<(String, Vec<String>)> {
    let program = env::var("CRUCIBLE_RED_PROGRAM").ok()?;
    if program.is_empty() {
        return None;
    }
    let args = match env::var("CRUCIBLE_RED_ARGS") {
        Ok(raw) if !raw.is_empty() => raw
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    };
    Some((program, args))
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
