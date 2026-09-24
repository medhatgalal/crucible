use std::path::{Path, PathBuf};

use crate::{message, GuidedError};

/// Engine root: `CRUCIBLE_ROOT`, else the directory of `CRUCIBLE_WRAPPER`, else `current_exe`'s directory.
///
/// `CRUCIBLE_WRAPPER` is a path (`$0`), not a command string. An empty variable is unset.
pub fn root() -> Result<PathBuf, GuidedError> {
    if let Some(value) = nonempty_var("CRUCIBLE_ROOT") {
        return Ok(PathBuf::from(value));
    }
    if let Some(value) = nonempty_var("CRUCIBLE_WRAPPER") {
        return Ok(parent_dir(Path::new(&value)));
    }
    let exe = std::env::current_exe()
        .map_err(|e| message(format!("cannot resolve current executable: {e}")))?;
    Ok(parent_dir(&exe))
}

fn nonempty_var(key: &str) -> Option<std::ffi::OsString> {
    let value = std::env::var_os(key)?;
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn parent_dir(path: &Path) -> PathBuf {
    // `dirname` of a bare filename is `.`; `Path::parent` of that is empty.
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}
