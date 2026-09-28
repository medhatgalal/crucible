//! Harness paths are the cycle card and the program tree, not owned product files.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::records;

const EXCLUDE_BLOCK: &str = "\
# crucible harness paths are not product porcelain
.crucible/
.wm/
/START.md
";

/// `.crucible/` (except task `worktrees/`), a `.wm` projection, or repo-root `START.md`.
pub(crate) fn path_is_harness(path: &str) -> bool {
    let path = path.trim_start_matches("./");
    if path == "START.md" {
        return true;
    }
    if path == ".wm" || path.starts_with(".wm/") || path.contains("/.wm/") {
        return true;
    }
    if path.ends_with("worktrees") || path.contains("worktrees/") {
        return false;
    }
    path == ".crucible" || path.starts_with(".crucible/") || path.contains("/.crucible/")
}

pub(crate) fn porcelain_path(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let rest = if chars.len() >= 3 && chars[2] == ' ' {
        chars[3..].iter().collect()
    } else {
        line.to_string()
    };
    match rest.split_once(" -> ") {
        Some((_, new)) => new.to_string(),
        None => rest,
    }
}

pub(crate) fn owned_file_tokens(item_md: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut on = false;
    for line in records(item_md) {
        if !on {
            if line.starts_with("## Owned files") {
                on = true;
            }
            continue;
        }
        if line.starts_with("## ") {
            break;
        }
        if let Some(rest) = line.strip_prefix("- ") {
            let rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
            for tok in rest.split_whitespace() {
                if !tok.is_empty() {
                    tokens.push(tok.to_string());
                }
            }
        }
    }
    tokens
}

/// Non-harness `git status` paths that are not an owned file of the item.
pub(crate) fn porcelain_outside_owned(repo: &Path, owned: &[String]) -> Vec<String> {
    let Ok(out) = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain", "-uall"])
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut outside = Vec::new();
    for line in records(&text) {
        if line.is_empty() {
            continue;
        }
        let path = porcelain_path(line);
        if path.is_empty() || path_is_harness(&path) {
            continue;
        }
        let owned_ok = owned
            .iter()
            .any(|op| path == *op || path.starts_with(&format!("{op}/")));
        if !owned_ok {
            outside.push(path);
        }
    }
    outside
}

/// Hide harness paths from later `git status` in this repo.
///
/// Exclude covers untracked files. `skip-worktree` covers tracked files, which
/// exclude does not hide once they are already in the index.
pub(crate) fn conceal_harness(repo: &Path) {
    if !repo.join(".git").exists() {
        return;
    }
    let info = repo.join(".git/info");
    let _ = fs::create_dir_all(&info);
    let exclude = info.join("exclude");
    let existing = fs::read_to_string(&exclude).unwrap_or_default();
    if !existing.contains("/START.md") || !existing.contains(".wm/") {
        let mut file = match OpenOptions::new().create(true).append(true).open(&exclude) {
            Ok(file) => file,
            Err(_) => return,
        };
        let _ = file.write_all(EXCLUDE_BLOCK.as_bytes());
    }
    let listed = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files", "-z", "--", ".crucible", ".wm", "START.md"])
        .stderr(Stdio::null())
        .output();
    let Ok(listed) = listed else {
        return;
    };
    if listed.stdout.iter().all(|byte| *byte == 0) {
        return;
    }
    let Ok(mut child) = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["update-index", "--skip-worktree", "-z", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&listed.stdout);
    }
    let _ = child.wait();
}
