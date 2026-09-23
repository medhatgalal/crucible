//! Non-empty `IDEA.md` or a READY `BACKLOG.tsv` row. Read-only.

use std::fs;
use std::path::Path;

pub fn intake_ready(cwd: &Path) -> bool {
    idea_nonempty(cwd) || backlog_ready(cwd)
}

fn idea_nonempty(cwd: &Path) -> bool {
    fs::read_to_string(cwd.join("IDEA.md"))
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

fn backlog_ready(cwd: &Path) -> bool {
    let Ok(text) = fs::read_to_string(cwd.join("BACKLOG.tsv")) else {
        return false;
    };
    for line in text.lines().skip(1) {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split('\t');
        let status = cols.nth(4).unwrap_or("");
        if status == "READY" {
            return true;
        }
    }
    false
}
