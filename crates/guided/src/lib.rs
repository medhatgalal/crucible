//! Root resolution, managed STATE / PROGRAM parsers, and adopt.

mod adopt;
mod program;
mod project;
mod root;
mod state;

use std::fmt;
use std::io;

pub use adopt::{
    adopt_battery_kept, adopt_check_required_batteries, adopt_copy_panel_from, adopt_find_rust_bin,
    adopt_gitignore_reason, adopt_install_engine, adopt_install_working_mode, adopt_is_script,
    adopt_refresh_skill_views, adopt_restore_kept_batteries, adopt_src_sha256,
    adopt_sync_gitignore, adopt_working_mode_installed, adopt_write_engine_source, cmd_adopt,
};
pub use crucible_contract::{Clock, FixedClock};
pub use program::{lifecycle_mode, uses_guided_cycle, uses_managed_lifecycle, LifecycleMode};
pub use project::project_cycle_line;
pub use root::root;
pub use state::{
    state_add_item, state_commit, state_lock, state_render_file, state_unlock, state_update_item,
    state_validate_file, state_value, StateLock, STATE_HEADER,
};

#[derive(Debug)]
pub enum GuidedError {
    Io(io::Error),
    Message(String),
}

impl fmt::Display for GuidedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GuidedError::Io(e) => write!(f, "{e}"),
            GuidedError::Message(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for GuidedError {}

impl From<io::Error> for GuidedError {
    fn from(e: io::Error) -> Self {
        GuidedError::Io(e)
    }
}

pub(crate) fn message(s: impl Into<String>) -> GuidedError {
    GuidedError::Message(s.into())
}

/// Split on `\n` the way awk splits records: a trailing newline is not an extra record.
/// A trailing `\r` stays on the record so a CRLF file does not match an LF fixture.
pub(crate) fn records(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut parts: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        parts.pop();
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, MutexGuard};

    static ROOT_ENV: Mutex<()> = Mutex::new(());
    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-guided-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn lock_root_env() -> MutexGuard<'static, ()> {
        ROOT_ENV.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_root_env(root: Option<&OsStr>, wrapper: Option<&OsStr>) {
        // SAFETY: callers hold ROOT_ENV; other tests do not read these keys.
        unsafe {
            match root {
                Some(v) => std::env::set_var("CRUCIBLE_ROOT", v),
                None => std::env::remove_var("CRUCIBLE_ROOT"),
            }
            match wrapper {
                Some(v) => std::env::set_var("CRUCIBLE_WRAPPER", v),
                None => std::env::remove_var("CRUCIBLE_WRAPPER"),
            }
        }
    }

    struct RestoreRootEnv {
        root: Option<std::ffi::OsString>,
        wrapper: Option<std::ffi::OsString>,
    }

    impl Drop for RestoreRootEnv {
        fn drop(&mut self) {
            // SAFETY: dropped while ROOT_ENV is still held by the test.
            unsafe {
                match self.root.take() {
                    Some(v) => std::env::set_var("CRUCIBLE_ROOT", v),
                    None => std::env::remove_var("CRUCIBLE_ROOT"),
                }
                match self.wrapper.take() {
                    Some(v) => std::env::set_var("CRUCIBLE_WRAPPER", v),
                    None => std::env::remove_var("CRUCIBLE_WRAPPER"),
                }
            }
        }
    }

    fn with_root_env(root: Option<&OsStr>, wrapper: Option<&OsStr>, body: impl FnOnce()) {
        let _guard = lock_root_env();
        let _restore = RestoreRootEnv {
            root: std::env::var_os("CRUCIBLE_ROOT"),
            wrapper: std::env::var_os("CRUCIBLE_WRAPPER"),
        };
        set_root_env(root, wrapper);
        body();
    }

    fn header_file(rows: &[&str]) -> String {
        let mut s = String::from(STATE_HEADER);
        s.push('\n');
        for row in rows {
            s.push_str(row);
            s.push('\n');
        }
        s
    }

    fn write_state(root: &Path, rows: &[&str]) {
        fs::write(root.join("STATE.tsv"), header_file(rows)).unwrap();
    }

    fn write_program(root: &Path, text: &str) {
        fs::write(root.join("PROGRAM"), text).unwrap();
    }

    #[test]
    fn root_prefers_crucible_root_then_wrapper_dir_then_exe() {
        with_root_env(
            Some(OsStr::new("/engine/tree")),
            Some(OsStr::new("/wrap/crucible")),
            || {
                assert_eq!(root().unwrap(), PathBuf::from("/engine/tree"));
            },
        );
        with_root_env(Some(OsStr::new("rel/root")), None, || {
            assert_eq!(root().unwrap(), PathBuf::from("rel/root"));
        });
        // Empty is unset, matching `${CRUCIBLE_ROOT:-...}`. The wrapper value is a path,
        // not a command: spaces stay in the filename and are not split off.
        with_root_env(
            Some(OsStr::new("")),
            Some(OsStr::new("/tmp/bin/crucible --help")),
            || {
                assert_eq!(root().unwrap(), PathBuf::from("/tmp/bin"));
            },
        );
        with_root_env(None, Some(OsStr::new("crucible")), || {
            assert_eq!(root().unwrap(), PathBuf::from("."));
        });
        with_root_env(None, None, || {
            let exe = std::env::current_exe().unwrap();
            let expect = exe.parent().filter(|p| !p.as_os_str().is_empty()).unwrap();
            assert_eq!(root().unwrap(), expect);
        });
    }

    #[test]
    fn lifecycle_mode_and_guided_cycle_match_program_lines() {
        let tmp = Tmp::new();
        assert_eq!(lifecycle_mode(tmp.path()).unwrap(), LifecycleMode::ItemFile);
        assert!(!uses_managed_lifecycle(tmp.path()).unwrap());
        assert!(!uses_guided_cycle(tmp.path()).unwrap());
        assert_eq!(LifecycleMode::ItemFile.as_str(), "item-file");
        assert_eq!(LifecycleMode::Managed.as_str(), "managed");

        write_program(tmp.path(), "lifecycle:managed\ncycle: guided extra\n");
        assert_eq!(lifecycle_mode(tmp.path()).unwrap(), LifecycleMode::ItemFile);
        assert!(!uses_guided_cycle(tmp.path()).unwrap());

        write_program(tmp.path(), "cycle: Guided\n");
        assert!(!uses_guided_cycle(tmp.path()).unwrap());

        write_program(
            tmp.path(),
            "lifecycle: managed\nlifecycle: weird\ncycle: guided\n",
        );
        assert_eq!(lifecycle_mode(tmp.path()).unwrap(), LifecycleMode::Managed);
        assert!(uses_managed_lifecycle(tmp.path()).unwrap());
        assert!(uses_guided_cycle(tmp.path()).unwrap());

        write_program(tmp.path(), "lifecycle: weird\nlifecycle: managed\n");
        assert_eq!(
            lifecycle_mode(tmp.path()).unwrap_err().to_string(),
            "unsupported lifecycle behavior: weird"
        );
        assert_eq!(
            uses_managed_lifecycle(tmp.path()).unwrap_err().to_string(),
            "unsupported lifecycle behavior: weird"
        );

        write_program(tmp.path(), "lifecycle: managed \n");
        assert_eq!(
            lifecycle_mode(tmp.path()).unwrap_err().to_string(),
            "unsupported lifecycle behavior: managed "
        );
    }

    #[test]
    fn state_header_duplicate_symlink_and_field_rules() {
        let tmp = Tmp::new();
        let path = tmp.path().join("STATE.tsv");
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: missing regular file"
        );

        fs::write(&path, "").unwrap();
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: empty file"
        );

        fs::write(&path, format!("{STATE_HEADER}\n")).unwrap();
        state_validate_file(&path).unwrap();

        fs::write(&path, STATE_HEADER).unwrap();
        state_validate_file(&path).unwrap();

        fs::write(&path, format!("{STATE_HEADER}\r\n")).unwrap();
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: header mismatch"
        );

        fs::write(&path, "item status stage\n").unwrap();
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: header mismatch"
        );

        fs::write(&path, format!("{STATE_HEADER}\n\n")).unwrap();
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: row 2 has 0 fields, need 8"
        );

        write_state(
            tmp.path(),
            &[
                "alpha\tCLOSED\tDRAFT\tw1\tLOW\t-\t-\t1",
                "beta\tDROPPED\tREVIEW\tw2\t-\t-\t-\t2",
            ],
        );
        state_validate_file(&path).unwrap();

        write_state(
            tmp.path(),
            &[
                "alpha\tCLOSED\tDRAFT\tw1\tLOW\t-\t-\t1",
                "alpha\tDROPPED\tREADY\tw2\tHIGH\t-\t-\t2",
            ],
        );
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: duplicate item alpha"
        );

        write_state(
            tmp.path(),
            &[
                "alpha\tACTIVE\tDRAFT\tw1\tLOW\t-\t-\t1",
                "beta\tBLOCKED\tBUILD\tw2\tMEDIUM\t-\t-\t2",
            ],
        );
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: more than one current item"
        );

        write_state(tmp.path(), &["alpha\tACTIVE\tDRAFT\tw1\tLOW\t-\t-\t1"]);
        state_validate_file(&path).unwrap();
        write_state(
            tmp.path(),
            &["alpha\tBLOCKED\tREADY\tw1\tHIGH\ta1\tCODE\t10"],
        );
        state_validate_file(&path).unwrap();

        write_state(tmp.path(), &["bad slug\tCLOSED\tDRAFT\tw1\tLOW\t-\t-\t1"]);
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: invalid item at row 2"
        );
        write_state(tmp.path(), &["alpha\tOPEN\tDRAFT\tw1\tLOW\t-\t-\t1"]);
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: invalid status for alpha"
        );
        write_state(tmp.path(), &["alpha\tCLOSED\tSHIP\tw1\tLOW\t-\t-\t1"]);
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: invalid stage for alpha"
        );
        write_state(tmp.path(), &["alpha\tCLOSED\tDRAFT\tbad id\tLOW\t-\t-\t1"]);
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: invalid work_id for alpha"
        );
        write_state(tmp.path(), &["alpha\tCLOSED\tDRAFT\tw1\tHUGE\t-\t-\t1"]);
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: invalid risk for alpha"
        );
        write_state(tmp.path(), &["alpha\tCLOSED\tDRAFT\tw1\tLOW\tnope!\t-\t1"]);
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: invalid inflight_attempt for alpha"
        );
        write_state(tmp.path(), &["alpha\tCLOSED\tDRAFT\tw1\tLOW\t-\tnope!\t1"]);
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: invalid block_code for alpha"
        );
        write_state(tmp.path(), &["alpha\tCLOSED\tDRAFT\tw1\tLOW\t-\t-\t12a"]);
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: invalid updated_epoch for alpha"
        );

        fs::remove_file(&path).unwrap();
        let real = tmp.path().join("real.tsv");
        fs::write(&real, format!("{STATE_HEADER}\n")).unwrap();
        std::os::unix::fs::symlink(&real, &path).unwrap();
        assert_eq!(
            state_validate_file(&path).unwrap_err().to_string(),
            "invalid STATE.tsv: missing regular file"
        );
    }

    #[test]
    fn state_lock_is_mkdir_and_drop_releases_it() {
        let tmp = Tmp::new();
        let lock_dir = tmp.path().join(".state.lock");
        let pid = std::process::id();
        let lock = state_lock(tmp.path()).unwrap();
        assert!(lock_dir.is_dir());
        assert!(!lock_dir.is_symlink());
        assert_eq!(
            lock.tsv_tmp,
            tmp.path().join(format!(".STATE.tsv.{pid}.tmp"))
        );
        assert_eq!(lock.md_tmp, tmp.path().join(format!(".STATE.md.{pid}.tmp")));
        assert_eq!(
            lock.program_tmp,
            tmp.path().join(format!(".PROGRAM.{pid}.tmp"))
        );
        assert_eq!(
            state_lock(tmp.path()).unwrap_err().to_string(),
            "state mutation already in progress"
        );
        drop(lock);
        assert!(!lock_dir.exists());

        let mut lock = state_lock(tmp.path()).unwrap();
        state_unlock(&mut lock).unwrap();
        assert!(!lock_dir.exists());
        assert_eq!(
            state_unlock(&mut lock).unwrap_err().to_string(),
            "could not release state lock"
        );

        let missing = tmp.path().join("no-such-dir");
        assert_eq!(
            state_lock(&missing).unwrap_err().to_string(),
            "state mutation already in progress"
        );
    }

    #[test]
    fn add_and_update_stamp_fixed_clock_and_refuse_duplicates() {
        let tmp = Tmp::new();
        write_program(tmp.path(), "program: work\n");
        write_state(tmp.path(), &[]);
        let clock = FixedClock::new(1_700_000_000);
        state_add_item(tmp.path(), &clock, "alpha", "w1", "LOW").unwrap();

        let tsv = fs::read_to_string(tmp.path().join("STATE.tsv")).unwrap();
        assert_eq!(
            tsv,
            format!("{STATE_HEADER}\nalpha\tACTIVE\tDRAFT\tw1\tLOW\t-\t-\t1700000000\n")
        );
        let md = fs::read_to_string(tmp.path().join("STATE.md")).unwrap();
        assert_eq!(
            md,
            "\
# STATE — work

Generated from `STATE.tsv`; do not edit this file by hand.

| Item | Status | Stage | Work ID | Risk | In-flight attempt | Block code | Updated epoch |
| --- | --- | --- | --- | --- | --- | --- | --- |
| alpha | ACTIVE | DRAFT | w1 | LOW | - | - | 1700000000 |
"
        );
        assert!(!tmp.path().join(".state.lock").exists());
        assert!(!tmp
            .path()
            .join(format!(".STATE.tsv.{}.tmp", std::process::id()))
            .exists());
        assert_eq!(
            state_value(tmp.path(), "alpha", 1).unwrap().as_deref(),
            Some("alpha")
        );
        assert_eq!(
            state_value(tmp.path(), "alpha", 8).unwrap().as_deref(),
            Some("1700000000")
        );
        assert_eq!(state_value(tmp.path(), "missing", 1).unwrap(), None);

        assert_eq!(
            state_add_item(tmp.path(), &clock, "beta", "w2", "HIGH")
                .unwrap_err()
                .to_string(),
            "refused: another item is current"
        );
        assert_eq!(
            state_add_item(tmp.path(), &clock, "alpha", "w2", "HIGH")
                .unwrap_err()
                .to_string(),
            "item already exists in STATE.tsv: alpha"
        );
        let unchanged = fs::read_to_string(tmp.path().join("STATE.tsv")).unwrap();
        assert_eq!(unchanged, tsv);

        let clock2 = FixedClock::new(1_700_000_111);
        state_update_item(
            tmp.path(),
            &clock2,
            "alpha",
            "BLOCKED",
            "BUILD",
            "w9",
            "HIGH",
            "a1",
            "HOLD",
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(tmp.path().join("STATE.tsv")).unwrap(),
            format!("{STATE_HEADER}\nalpha\tBLOCKED\tBUILD\tw9\tHIGH\ta1\tHOLD\t1700000111\n")
        );
        assert!(fs::read_to_string(tmp.path().join("STATE.md"))
            .unwrap()
            .contains("| alpha | BLOCKED | BUILD | w9 | HIGH | a1 | HOLD | 1700000111 |\n"));

        let before = fs::read_to_string(tmp.path().join("STATE.tsv")).unwrap();
        assert_eq!(
            state_update_item(
                tmp.path(),
                &clock2,
                "missing",
                "CLOSED",
                "REVIEW",
                "w9",
                "LOW",
                "-",
                "-",
            )
            .unwrap_err()
            .to_string(),
            "item missing from STATE.tsv: missing"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("STATE.tsv")).unwrap(),
            before
        );
        assert!(state_lock(tmp.path()).is_ok());

        write_state(
            tmp.path(),
            &[
                "beta\tCLOSED\tREVIEW\twb\t-\t-\t-\t9",
                "alpha\tCLOSED\tDRAFT\tw1\tLOW\t-\t-\t1",
            ],
        );
        state_add_item(tmp.path(), &clock, "gamma", "wg", "-").unwrap();
        let added = fs::read_to_string(tmp.path().join("STATE.tsv")).unwrap();
        assert!(added.contains("beta\tCLOSED\tREVIEW\twb\t-\t-\t-\t9\n"));
        assert!(added.ends_with("gamma\tACTIVE\tDRAFT\twg\t-\t-\t-\t1700000000\n"));

        let kept = added.clone();
        assert_eq!(
            state_update_item(
                tmp.path(),
                &clock2,
                "gamma",
                "NOPE",
                "DRAFT",
                "wg",
                "-",
                "-",
                "-",
            )
            .unwrap_err()
            .to_string(),
            "invalid STATE.tsv: invalid status for gamma"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("STATE.tsv")).unwrap(),
            kept
        );
        assert!(!tmp.path().join(".state.lock").exists());
    }

    #[test]
    fn render_defaults_program_name_and_source() {
        let tmp = Tmp::new();
        write_state(tmp.path(), &["alpha\tCLOSED\tDRAFT\tw1\tLOW\t-\t-\t1"]);
        let out = tmp.path().join("out.md");
        state_render_file(tmp.path(), &out, None).unwrap();
        let md = fs::read_to_string(&out).unwrap();
        assert!(md.starts_with("# STATE — program\n\n"));
        assert!(md.contains("| alpha | CLOSED | DRAFT | w1 | LOW | - | - | 1 |\n"));
    }
}
