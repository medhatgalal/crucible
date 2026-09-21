use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crucible_contract::{format_rfc3339_z, resolve_wm, Clock, Event};

use crate::error::KernelError;
use crate::events::append_event;
use crate::paths::{ensure_wm, sanitize_tsv};

pub const METRICS_HEADER: &str = "when\toutcome\tslices\tbound\tnote\n";

/// Append a METRICS.tsv halt row and a matching `.wm/EVENTS` `kind: halt` line.
pub fn metrics_append(
    dir: impl AsRef<Path>,
    outcome: &str,
    note: &str,
    bound: i64,
    elapsed_s: i64,
    iterations: i64,
    clock: &dyn Clock,
) -> Result<(), KernelError> {
    let dir = dir.as_ref();
    let (repo, _) = resolve_wm(dir);
    let wm = ensure_wm(dir)?;
    let path = wm.join("METRICS.tsv");
    if !path.is_file() {
        fs::write(&path, METRICS_HEADER)?;
    }
    let slices = count_slices(&repo);
    let when = format_rfc3339_z(clock.now_unix());
    let outcome_t = sanitize_tsv(outcome);
    let note_t = {
        let n = sanitize_tsv(note);
        if n.is_empty() {
            "-".to_string()
        } else {
            n
        }
    };
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{when}\t{outcome_t}\t{slices}\t{bound}\t{note_t}")?;
    append_event(dir, &Event::halt(&when, &outcome_t, elapsed_s, iterations))?;
    Ok(())
}

fn count_slices(repo: &Path) -> i64 {
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
