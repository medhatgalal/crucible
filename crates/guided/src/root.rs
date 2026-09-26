use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{message, records, GuidedError};

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

/// Program root for every guided verb except `adopt`.
///
/// `root()` stays the engine source. A directory that already has `PROGRAM` is
/// that program. Otherwise, when cwd's git toplevel is that same directory,
/// one `.crucible/<name>/` record with `cycle: guided`, `lifecycle: managed`,
/// and `repo:` pointing at the toplevel is the program. Zero matches keep the
/// engine path so drive still reports `drive requires a guided cycle`.
pub fn program_root() -> Result<PathBuf, GuidedError> {
    let engine = root()?;
    let cwd = std::env::current_dir()?;
    resolve_program_root(&engine, &cwd)
}

pub(crate) fn resolve_program_root(engine: &Path, cwd: &Path) -> Result<PathBuf, GuidedError> {
    if engine.join("PROGRAM").is_file() {
        return Ok(engine.to_path_buf());
    }
    let Some(top) = git_toplevel(cwd) else {
        return Ok(engine.to_path_buf());
    };
    let Ok(engine_abs) = fs::canonicalize(engine) else {
        return Ok(engine.to_path_buf());
    };
    let Ok(top_abs) = fs::canonicalize(&top) else {
        return Ok(engine.to_path_buf());
    };
    if engine_abs != top_abs {
        return Ok(engine.to_path_buf());
    }
    let found = guided_program_dirs(&top_abs)?;
    match found.as_slice() {
        [] => Ok(engine.to_path_buf()),
        [one] => Ok(one.clone()),
        many => {
            let names: Vec<String> = many
                .iter()
                .filter_map(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .collect();
            Err(message(format!(
                "refused: more than one guided program: {}",
                names.join(" ")
            )))
        }
    }
}

fn guided_program_dirs(toplevel: &Path) -> Result<Vec<PathBuf>, GuidedError> {
    let parent = toplevel.join(".crucible");
    let rd = match fs::read_dir(&parent) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut found = Vec::new();
    for ent in rd {
        let ent = ent?;
        let name = ent.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        // `"$parent"/*` does not list dot-directories.
        if name.starts_with('.') {
            continue;
        }
        let path = ent.path();
        if path.is_dir() && program_matches(&path, toplevel) {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

fn program_matches(dir: &Path, toplevel: &Path) -> bool {
    let Ok(text) = fs::read_to_string(dir.join("PROGRAM")) else {
        return false;
    };
    let lines = records(&text);
    if !lines.contains(&"cycle: guided") {
        return false;
    }
    if lines
        .iter()
        .find_map(|line| line.strip_prefix("lifecycle: "))
        != Some("managed")
    {
        return false;
    }
    let Some(repo) = lines.iter().find_map(|line| line.strip_prefix("repo: ")) else {
        return false;
    };
    fs::canonicalize(repo).ok().as_deref() == Some(toplevel)
}

fn git_toplevel(cwd: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut line = String::from_utf8_lossy(&out.stdout).into_owned();
    if line.ends_with('\n') {
        line.pop();
    }
    if line.is_empty() {
        None
    } else {
        Some(PathBuf::from(line))
    }
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
