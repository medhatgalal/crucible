use std::fs;
use std::path::Path;

use crucible_contract::{resolve_wm, Clock};

use crate::error::KernelError;
use crate::floor::floor_write;
use crate::metrics::metrics_append;

const INDEPENDENCE: &str = "SUBAGENT-ISOLATED";
const CARD: &str = "STOP-ASK QUESTIONS";

/// Result of the POSIX QUESTIONS gate (`questions_need_ask` / `loop_halt`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopAskQuestions {
    pub stop: bool,
    pub card: String,
    pub station: String,
    pub elapsed_s: i64,
    pub exit: i32,
}

/// Halt when non-empty `QUESTIONS.md` has no non-empty `ANSWERS.md`.
///
/// POSIX: `[ -s QUESTIONS.md ] && [ ! -s ANSWERS.md ]` → FLOOR / metrics
/// `STOP-ASK QUESTIONS`, exit 1. A file with a line is not a stop; this
/// function does not invent `ANSWERS.md`. Does not `git worktree remove`.
/// Isolation is `SUBAGENT-ISOLATED`. Does not invoke grok.
pub fn stop_ask_questions(
    dir: impl AsRef<Path>,
    clock: &dyn Clock,
) -> Result<StopAskQuestions, KernelError> {
    let dir = dir.as_ref();
    let (repo, _) = resolve_wm(dir);

    if !questions_need_ask(&repo) {
        return Ok(StopAskQuestions {
            stop: false,
            card: String::new(),
            station: String::new(),
            elapsed_s: 0,
            exit: 0,
        });
    }

    let floor = floor_write(dir, CARD, INDEPENDENCE, clock)?;
    let bound = posix_loop_bound(&repo);
    metrics_append(dir, CARD, "-", bound, floor.elapsed_s, 0, clock)?;
    Ok(StopAskQuestions {
        stop: true,
        card: floor.card,
        station: floor.station,
        elapsed_s: floor.elapsed_s,
        exit: 1,
    })
}

/// POSIX `questions_need_ask`: `[ -s QUESTIONS.md ] && [ ! -s ANSWERS.md ]`.
fn questions_need_ask(repo: &Path) -> bool {
    nonempty_file(&repo.join("QUESTIONS.md")) && !nonempty_file(&repo.join("ANSWERS.md"))
}

fn nonempty_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() > 0)
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
