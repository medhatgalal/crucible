use std::fs;
use std::path::Path;

use crucible_contract::{resolve_wm, Clock};

use crate::error::KernelError;
use crate::floor::floor_write;
use crate::metrics::metrics_append;

const INDEPENDENCE: &str = "SUBAGENT-ISOLATED";
const CARD: &str = "INDEPENDENCE_UNAVAILABLE";

/// Result of the POSIX independence CHECK (`panel_valid` / `high_kinds_ok` /
/// `cmd_run` empty worker / `honest_isolation` never-CROSS-FAMILY-on-card).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndependenceCheck {
    pub stop: bool,
    pub card: String,
    pub station: String,
    pub elapsed_s: i64,
    pub exit: i32,
}

/// Halt when POSIX would refuse the panel (same maker/reviewer agent, or
/// maker/reviewer cast with no CLI worker).
///
/// Missing `.wm/PANEL.tsv` (or missing roles) continues as one-kind; this
/// function does not invent `PANEL.tsv` / `PANEL.ASSIGN.tsv`. One-kind
/// distinct agents continue. Isolation on the halt FLOOR is
/// `SUBAGENT-ISOLATED` (never `CROSS-FAMILY` on the card). Does not
/// `git worktree remove`. Does not invoke grok.
pub fn check_independence(
    dir: impl AsRef<Path>,
    clock: &dyn Clock,
) -> Result<IndependenceCheck, KernelError> {
    let dir = dir.as_ref();
    let (repo, wm) = resolve_wm(dir);

    if !independence_refused(&wm) {
        return Ok(IndependenceCheck {
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
    Ok(IndependenceCheck {
        stop: true,
        card: floor.card,
        station: floor.station,
        elapsed_s: floor.elapsed_s,
        exit: 1,
    })
}

fn independence_refused(wm: &Path) -> bool {
    let maker = panel_row(wm, "maker");
    let reviewer = panel_row(wm, "reviewer");
    if maker.is_none() && reviewer.is_none() {
        return false;
    }
    if role_has_no_cli(maker.as_ref()) || role_has_no_cli(reviewer.as_ref()) {
        return true;
    }
    let Some(maker) = maker else {
        return false;
    };
    let Some(reviewer) = reviewer else {
        return false;
    };
    agent_present(&maker.agent) && agent_present(&reviewer.agent) && maker.agent == reviewer.agent
}

fn role_has_no_cli(row: Option<&PanelRow>) -> bool {
    let Some(row) = row else {
        return false;
    };
    if !agent_present(&row.agent) {
        return false;
    }
    row.cmd.is_empty() || row.cmd == "-"
}

fn agent_present(agent: &str) -> bool {
    !agent.is_empty() && agent != "-"
}

struct PanelRow {
    agent: String,
    cmd: String,
}

fn panel_row(wm: &Path, role: &str) -> Option<PanelRow> {
    let text = fs::read_to_string(wm.join("PANEL.tsv")).ok()?;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.first().map(|s| s.trim()) != Some(role) {
            continue;
        }
        let agent = cols.get(1).map(|s| s.trim()).unwrap_or("");
        let cmd = cols.get(3).map(|s| s.trim()).unwrap_or("");
        return Some(PanelRow {
            agent: agent.to_string(),
            cmd: cmd.to_string(),
        });
    }
    None
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
