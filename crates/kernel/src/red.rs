use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crucible_contract::{resolve_wm, Clock};

use crate::error::KernelError;
use crate::floor::floor_write;
use crate::invoke::run;
use crate::paths::{ensure_wm, parse_t0};
use crate::trace::go_start;
use crate::worktree::mint_worktree;

const INDEPENDENCE: &str = "SUBAGENT-ISOLATED";
const CARD_RED: &str = "NEXT RED";
const CARD_ANDON: &str = "STOP-ASK red refused";

/// One-card NEXT RED stub (no MAP-REVISE / keep-on-failure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextRed {
    pub worktree: PathBuf,
    pub exit: i32,
    pub elapsed_s: i64,
    pub card: String,
    pub station: String,
    pub archived: Option<PathBuf>,
}

/// `go_start` if `t0` is missing, mint `.wm/worktrees/<id>`, run injected
/// `program`/`args` in that cwd, then `floor_write` NEXT RED / BUILD when
/// FALSIFIER is present, or STOP-ASK / ANDON when it is not.
///
/// FALSIFIER path matches POSIX bet worktree: `<wt>/.wm` → product `.wm`.
/// EVENTS stay on the repo `.wm` via [`run`]. Does not invoke grok.
pub fn next_red(
    dir: impl AsRef<Path>,
    slice_id: &str,
    program: impl AsRef<OsStr>,
    args: &[&str],
    session: &str,
    clock: &dyn Clock,
) -> Result<NextRed, KernelError> {
    let dir = dir.as_ref();
    let (repo, _) = resolve_wm(dir);
    let wm = ensure_wm(&repo)?;

    let archived = if parse_t0(&wm).is_none() {
        go_start(dir, clock)?.archived
    } else {
        None
    };

    let worktree = mint_worktree(dir, slice_id)?;
    link_product_wm(&worktree, &wm)?;
    fs::write(wm.join("slice-in-flight"), format!("id: {slice_id}\n"))?;

    let invoked = run(dir, &worktree, program, args, session, CARD_RED, clock)?;

    let card = if falsifier_present(&wm, &worktree) {
        CARD_RED
    } else {
        CARD_ANDON
    };
    let floor = floor_write(dir, card, INDEPENDENCE, clock)?;

    Ok(NextRed {
        worktree,
        exit: invoked.exit,
        elapsed_s: invoked.elapsed_s,
        card: floor.card,
        station: floor.station,
        archived,
    })
}

fn falsifier_present(wm: &Path, wt: &Path) -> bool {
    wm.join("FALSIFIER").is_file() || wt.join(".wm").join("FALSIFIER").is_file()
}

fn link_product_wm(wt: &Path, wm: &Path) -> Result<(), KernelError> {
    let dest = wt.join(".wm");
    if let Ok(meta) = dest.symlink_metadata() {
        if meta.file_type().is_dir() && !meta.file_type().is_symlink() {
            fs::remove_dir_all(&dest)?;
        } else {
            fs::remove_file(&dest)?;
        }
    }
    let target = fs::canonicalize(wm)?;
    symlink(target, dest)?;
    Ok(())
}
