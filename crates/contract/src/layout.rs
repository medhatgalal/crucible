use std::path::{Path, PathBuf};

/// `(repo_root, wm_dir)`.
///
/// `dir` may be the product root (contains `.wm`) or the `.wm` directory itself.
pub fn resolve(dir: &Path) -> (PathBuf, PathBuf) {
    if dir.file_name().is_some_and(|n| n == ".wm") {
        let repo = dir
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        (repo, dir.to_path_buf())
    } else {
        (dir.to_path_buf(), dir.join(".wm"))
    }
}
