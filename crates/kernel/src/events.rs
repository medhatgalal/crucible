use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crucible_contract::{resolve_wm, Event};

use crate::error::KernelError;
use crate::paths::ensure_wm;

/// Append one JSON object as a line of `.wm/EVENTS` (D22). Create the file if missing.
pub fn append_event(dir: impl AsRef<Path>, event: &Event) -> Result<(), KernelError> {
    let wm = ensure_wm(dir.as_ref())?;
    let path = wm.join("EVENTS");
    let line = event
        .to_jsonl_line()
        .map_err(|e| KernelError::Json(e.to_string()))?;
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// Parse `.wm/EVENTS` JSONL. Missing file → empty.
pub fn read_events(dir: impl AsRef<Path>) -> Result<Vec<Event>, KernelError> {
    let (_repo, wm) = resolve_wm(dir.as_ref());
    let path = wm.join("EVENTS");
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let ev = Event::from_jsonl_line(line).map_err(|e| KernelError::Json(e.to_string()))?;
        out.push(ev);
    }
    Ok(out)
}
