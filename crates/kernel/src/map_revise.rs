use std::fs;
use std::path::Path;

use crucible_contract::{resolve_wm, Clock};

use crate::error::KernelError;
use crate::floor::floor_write;
use crate::metrics::metrics_append;
use crate::paths::kv_get;

const INDEPENDENCE: &str = "SUBAGENT-ISOLATED";
const CAP: i64 = 2;
const CARD_STOP: &str = "STOP-ASK";
const CARD_STOP_ASK: &str = "STOP-ASK NEXT MAP";
const CARD_ESCALATE: &str = "ESCALATE MAP_REVISE";

/// Result of the POSIX MAP-REVISE gate (`map_word_recorded` /
/// `map_revise_count` / `cmd_next` send-back / loop NEXT MAP without specifier).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapReviseCheck {
    pub stop: bool,
    pub card: String,
    pub station: String,
    pub elapsed_s: i64,
    pub exit: i32,
}

/// Halt when POSIX would send MAP-REVISE back (under cap: `STOP-ASK NEXT MAP`
/// because specifier is not ported; at cap: `ESCALATE MAP_REVISE`) or
/// `MAP-STOP-ASK` (`STOP-ASK`).
///
/// Missing `.wm/map-verdict`, missing/empty WORD, MAP-ACCEPT, and any WORD
/// other than MAP-REVISE / MAP-STOP-ASK continue. This function does not
/// invent `map-verdict` or increment `map-revise-count`. Does not
/// `git worktree remove`. Isolation is `SUBAGENT-ISOLATED`. Does not
/// invoke grok.
pub fn check_map_revise(
    dir: impl AsRef<Path>,
    clock: &dyn Clock,
) -> Result<MapReviseCheck, KernelError> {
    let dir = dir.as_ref();
    let (repo, wm) = resolve_wm(dir);

    let word = map_word_recorded(&wm);
    let card = match word.as_deref() {
        Some("MAP-STOP-ASK") => CARD_STOP,
        Some("MAP-REVISE") if map_revise_count(&wm) >= CAP => CARD_ESCALATE,
        Some("MAP-REVISE") => CARD_STOP_ASK,
        _ => {
            return Ok(MapReviseCheck {
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
    Ok(MapReviseCheck {
        stop: true,
        card: floor.card,
        station: floor.station,
        elapsed_s: floor.elapsed_s,
        exit: 1,
    })
}

fn map_word_recorded(wm: &Path) -> Option<String> {
    let text = fs::read_to_string(wm.join("map-verdict")).ok()?;
    kv_get(&text, "WORD")
}

/// POSIX `map_revise_count`: missing or non-numeric first field → 0.
fn map_revise_count(wm: &Path) -> i64 {
    let Ok(text) = fs::read_to_string(wm.join("map-revise-count")) else {
        return 0;
    };
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let first = t.split_whitespace().next().unwrap_or("");
        if first.is_empty() || !first.chars().all(|c| c.is_ascii_digit()) {
            return 0;
        }
        return first.parse().unwrap_or(0);
    }
    0
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
