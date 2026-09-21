use std::fs;
use std::path::Path;

use crucible_contract::{resolve_wm, Clock};

use crate::error::KernelError;
use crate::floor::floor_write;
use crate::metrics::metrics_append;
use crate::paths::first_nonempty_line;

const INDEPENDENCE: &str = "SUBAGENT-ISOLATED";
const CARD_REVIEWER: &str = "STOP-ASK NEXT RUN reviewer";
const CARD_EARLY: &str = "ESCALATE EARLY_IMPLEMENT";

/// Result of the POSIX inspect file-gate after FALSIFIER is present
/// (`cmd_next` red.status / no-build / early-implement).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectCheck {
    pub stop: bool,
    pub card: String,
    pub station: String,
    pub elapsed_s: i64,
    pub exit: i32,
}

/// Halt when POSIX would leave BUILD for inspect (or escalate) after red.
///
/// After `.wm/FALSIFIER` is present (`cmd_next` from `red.status`):
/// - `early-implement` → `ESCALATE EARLY_IMPLEMENT`
/// - `no-build` (or any status other than `red`) → `STOP-ASK NEXT RUN reviewer`
///   because reviewer exec is not ported (same as specifier → `STOP-ASK NEXT MAP`)
///
/// Missing FALSIFIER, missing `red.status`, and `red.status=red` continue
/// (still BUILD). Does not invent `red.status`. Does not `git worktree remove`.
/// Isolation is `SUBAGENT-ISOLATED`. Does not invoke grok.
pub fn check_inspect(
    dir: impl AsRef<Path>,
    clock: &dyn Clock,
) -> Result<InspectCheck, KernelError> {
    let dir = dir.as_ref();
    let (repo, wm) = resolve_wm(dir);

    let card = match inspect_halt_card(&wm) {
        Some(card) => card,
        None => {
            return Ok(InspectCheck {
                stop: false,
                card: String::new(),
                station: String::new(),
                elapsed_s: 0,
                exit: 0,
            });
        }
    };
    let floor = floor_write(dir, card, INDEPENDENCE, clock)?;
    let bound = posix_loop_bound(&repo);
    metrics_append(dir, card, "-", bound, floor.elapsed_s, 0, clock)?;
    Ok(InspectCheck {
        stop: true,
        card: floor.card,
        station: floor.station,
        elapsed_s: floor.elapsed_s,
        exit: 1,
    })
}

fn inspect_halt_card(wm: &Path) -> Option<&'static str> {
    if !wm.join("FALSIFIER").is_file() {
        return None;
    }
    let path = wm.join("red.status");
    if !path.is_file() {
        return None;
    }
    let text = fs::read_to_string(&path).ok()?;
    match first_nonempty_line(&text) {
        "early-implement" => Some(CARD_EARLY),
        "red" => None,
        _ => Some(CARD_REVIEWER),
    }
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
