use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::Instant;

use crucible_contract::{format_rfc3339_z, Clock, Event};

use crate::error::KernelError;
use crate::events::append_event;
use crate::paths::ensure_wm;

/// Result of a foreground worker invoke (no daemon).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvokeRun {
    pub exit: i32,
    pub elapsed_s: i64,
}

/// Run `program` with `args` in `cwd`, wait for the child, append `invoke_end`.
///
/// `dir` is the repo (EVENTS under `.wm/`). `cwd` is the worktree or repo cwd.
/// Does not port NEXT RED / BUILD, panel, or CLI workers.
pub fn run(
    dir: impl AsRef<Path>,
    cwd: impl AsRef<Path>,
    program: impl AsRef<OsStr>,
    args: &[&str],
    session: &str,
    card: &str,
    clock: &dyn Clock,
) -> Result<InvokeRun, KernelError> {
    let dir = dir.as_ref();
    let cwd = cwd.as_ref();
    ensure_wm(dir)?;

    let started = Instant::now();
    let status = Command::new(program.as_ref())
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    let elapsed_s = started.elapsed().as_secs() as i64;
    let exit = exit_code(status);

    let when = format_rfc3339_z(clock.now_unix());
    append_event(
        dir,
        &Event::invoke_end(when, card, session, elapsed_s, exit),
    )?;

    Ok(InvokeRun { exit, elapsed_s })
}

fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    1
}
