use std::fs;
use std::path::{Path, PathBuf};

use crucible_contract::{format_rfc3339_z, resolve_wm, Clock, Event};

use crate::error::KernelError;
use crate::events::append_event;
use crate::paths::{ensure_wm, first_nonempty_line, kv_get, parse_t0, sanitize_tsv};
use crate::station::floor_station;
use crate::trace::{ensure_trace_header, last_trace_card, TRACE_HEADER};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloorWriteResult {
    pub elapsed_s: i64,
    pub station: String,
    pub card: String,
    pub wip: String,
    pub andon: String,
    pub trace_appended: bool,
}

/// Write `.wm/FLOOR.md`. Create `t0` if missing. Append TRACE only when the card changes.
///
/// Returns `elapsed_s` (`now - t0`); does not print the POSIX `FLOOR t=+Ns` line.
pub fn floor_write(
    dir: impl AsRef<Path>,
    card: &str,
    independence: &str,
    clock: &dyn Clock,
) -> Result<FloorWriteResult, KernelError> {
    let dir = dir.as_ref();
    let (repo, _) = resolve_wm(dir);
    let wm = ensure_wm(dir)?;

    let card = {
        let line = first_nonempty_line(card);
        if line.is_empty() {
            "-"
        } else {
            line
        }
    };
    let station = floor_station(card);
    let wip = read_wip(&wm);
    let andon = if card.starts_with("STOP-ASK")
        || card.starts_with("ESCALATE")
        || card.starts_with("INDEPENDENCE_UNAVAILABLE")
    {
        card.to_string()
    } else {
        "-".to_string()
    };

    if parse_t0(&wm).is_none() {
        fs::write(wm.join("t0"), format!("{}\n", clock.now_unix()))?;
    }
    let t0 = parse_t0(&wm).unwrap_or(0);
    let elapsed_s = clock.now_unix() - t0;

    let evidence = collect_evidence(&repo);
    let mut floor = String::new();
    floor.push_str(&format!("station: {station}\n"));
    floor.push_str(&format!("card: {card}\n"));
    floor.push_str(&format!("wip: {wip}\n"));
    floor.push_str(&format!("andon: {andon}\n"));
    floor.push_str(&format!("independence: {independence}\n"));
    floor.push_str(&format!("elapsed: {elapsed_s}\n"));
    floor.push_str("evidence:\n");
    for p in &evidence {
        floor.push_str("  ");
        floor.push_str(p);
        floor.push('\n');
    }
    fs::write(wm.join("FLOOR.md"), floor)?;

    ensure_trace_header(&wm)?;
    let card_t = sanitize_tsv(card);
    let st_t = sanitize_tsv(station);
    let last = last_trace_card(&wm);
    let trace_appended = last.as_deref() != Some(card_t.as_str());
    if trace_appended {
        let when = format_rfc3339_z(clock.now_unix());
        let mut body =
            fs::read_to_string(wm.join("TRACE.tsv")).unwrap_or_else(|_| TRACE_HEADER.to_string());
        if !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&format!("{when}\t{card_t}\t{st_t}\n"));
        fs::write(wm.join("TRACE.tsv"), body)?;
        append_event(dir, &Event::card(&when, &card_t, &st_t))?;
    }

    Ok(FloorWriteResult {
        elapsed_s,
        station: station.to_string(),
        card: card.to_string(),
        wip,
        andon,
        trace_appended,
    })
}

fn read_wip(wm: &Path) -> String {
    let Ok(text) = fs::read_to_string(wm.join("slice-in-flight")) else {
        return "-".to_string();
    };
    kv_get(&text, "id")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

fn collect_evidence(repo: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if repo.join(".wm").join("FALSIFIER").is_file() {
        out.push(".wm/FALSIFIER".to_string());
    }
    if repo.join("reviews").join("review.md").is_file() {
        out.push("reviews/review.md".to_string());
    }
    let ev_dir = repo.join(".wm").join("evidence");
    if let Ok(rd) = fs::read_dir(&ev_dir) {
        let mut names: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        names.sort();
        for p in names {
            if let Some(name) = p.file_name() {
                out.push(format!(".wm/evidence/{}", name.to_string_lossy()));
            }
        }
    }
    out
}
