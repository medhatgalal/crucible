use std::fs;
use std::path::{Path, PathBuf};

use crucible_contract::resolve_wm;

use crate::error::KernelError;

pub fn ensure_wm(dir: &Path) -> Result<PathBuf, KernelError> {
    let (_repo, wm) = resolve_wm(dir);
    fs::create_dir_all(&wm)?;
    Ok(wm)
}

pub fn first_nonempty_line(s: &str) -> &str {
    s.lines()
        .map(|l| l.trim_end_matches('\r'))
        .find(|l| !l.trim().is_empty())
        .map(str::trim)
        .unwrap_or("")
}

pub fn sanitize_tsv(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

pub fn kv_get(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some((k, v)) = line.split_once(':') {
            if k.trim() == key {
                let v = first_nonempty_line(v);
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

pub fn parse_t0(wm: &Path) -> Option<i64> {
    let text = fs::read_to_string(wm.join("t0")).ok()?;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let first = t.split_whitespace().next()?;
        if first.chars().all(|c| c.is_ascii_digit()) {
            return first.parse().ok();
        }
        return None;
    }
    None
}
