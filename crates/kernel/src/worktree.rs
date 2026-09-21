use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crucible_contract::resolve_wm;

use crate::error::KernelError;
use crate::paths::ensure_wm;

/// Mint a git worktree at `<repo>/.wm/worktrees/<id>` from `HEAD`.
///
/// Uses `git worktree add` (not `clone --local`). EVENTS stay on the repo `.wm`;
/// this helper does not symlink or create `<wt>/.wm`. Isolation is still
/// `SUBAGENT-ISOLATED` (no panel). Does not invoke grok.
pub fn mint_worktree(dir: impl AsRef<Path>, id: &str) -> Result<PathBuf, KernelError> {
    if !slice_id_ok(id) {
        return Err(KernelError::Message(format!("invalid slice id: {id}")));
    }
    let dir = dir.as_ref();
    let (repo, _) = resolve_wm(dir);
    let wm = ensure_wm(&repo)?;
    let path = wm.join("worktrees").join(id);
    if path.join(".git").is_file() {
        return Ok(path);
    }
    fs::create_dir_all(path.parent().unwrap_or(&wm))?;
    if path.exists() {
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    let branch = format!("wm/{id}");
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["worktree", "add", "-q", "-B"])
        .arg(&branch)
        .arg(&path)
        .arg("HEAD")
        .output()?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(KernelError::Message(format!(
            "git worktree add failed: {err}"
        )));
    }
    Ok(path)
}

fn slice_id_ok(id: &str) -> bool {
    if id.is_empty() || id == "." || id == ".." {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}
