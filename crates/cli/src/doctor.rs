//! Keep-current for the Grok loop-router.
//!
//! Callers inject paths. This module does not read `$HOME`.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub const ROUTER_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/loop-router.md"
));

pub const HOME_UNSET_WARNING: &str = "home loop-router not written (HOME unset)";

const HASH_KEY: &str = "ADR-HASH:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterSite {
    Repo,
    Home,
}

#[derive(Debug)]
pub struct HomeWriteError {
    path: PathBuf,
    detail: String,
}

impl std::fmt::Display for HomeWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "home loop-router not written ({}): {}",
            self.path.display(),
            self.detail
        )
    }
}

impl std::error::Error for HomeWriteError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub warnings: Vec<String>,
    pub oks: Vec<String>,
}

impl DoctorReport {
    #[cfg(test)]
    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
    }

    pub fn write_lines(&self, mut out: impl Write) -> io::Result<()> {
        for w in &self.warnings {
            writeln!(out, "warn: {w}")?;
        }
        for o in &self.oks {
            writeln!(out, "ok: {o}")?;
        }
        Ok(())
    }
}

pub fn router_path(root: &Path) -> PathBuf {
    root.join(".grok").join("rules").join("loop-router.md")
}

pub fn fixture_adr_hash() -> String {
    parse_adr_hash(ROUTER_FIXTURE).expect("testdata/loop-router.md needs ADR-HASH")
}

pub fn parse_adr_hash(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix(HASH_KEY) else {
            continue;
        };
        let token = rest.split_whitespace().next()?;
        if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(token.to_ascii_lowercase());
        }
    }
    None
}

pub fn check_router(path: &Path, expected_hash: &str, site: RouterSite) -> DoctorReport {
    let mut report = DoctorReport {
        warnings: Vec::new(),
        oks: Vec::new(),
    };
    let label = site_label(site);
    // Stat of the final path follows a symlink parent. Classify parents first.
    let rules = path.parent();
    let grok = rules.and_then(Path::parent);
    let (Some(rules), Some(grok)) = (rules, grok) else {
        report.warnings.push(format!(
            "{label} loop-router unreadable ({}): no parent",
            path.display()
        ));
        return report;
    };
    for component in [grok, rules] {
        match node_kind(component) {
            NodeKind::Missing => {
                report
                    .warnings
                    .push(format!("{label} loop-router missing ({})", path.display()));
                return report;
            }
            NodeKind::Err(e) => {
                report.warnings.push(format!(
                    "{label} loop-router unreadable ({}): {e}",
                    component.display()
                ));
                return report;
            }
            NodeKind::Symlink => {
                report.warnings.push(format!(
                    "{label} loop-router parent is a symlink ({})",
                    component.display()
                ));
                return report;
            }
            NodeKind::Dir => {}
            NodeKind::File | NodeKind::Other => {
                report.warnings.push(format!(
                    "{label} loop-router parent is not a directory ({})",
                    component.display()
                ));
                return report;
            }
        }
    }
    match node_kind(path) {
        NodeKind::Missing => {
            report
                .warnings
                .push(format!("{label} loop-router missing ({})", path.display()));
        }
        NodeKind::Err(e) => {
            report.warnings.push(format!(
                "{label} loop-router unreadable ({}): {e}",
                path.display()
            ));
        }
        NodeKind::File => match fs::read_to_string(path) {
            Ok(text) => push_hash_result(&mut report, label, path, expected_hash, &text),
            Err(e) => {
                report.warnings.push(format!(
                    "{label} loop-router unreadable ({}): {e}",
                    path.display()
                ));
            }
        },
        NodeKind::Symlink | NodeKind::Dir | NodeKind::Other => {
            report.warnings.push(format!(
                "{label} loop-router is not a regular file ({})",
                path.display()
            ));
        }
    }
    report
}

/// Writes `ROUTER_FIXTURE` under `home`. Does not read the environment.
/// `Err` is a refusal or an I/O failure. The caller prints `warn:` and returns 1.
/// It does not call `check_router` on that error.
pub fn install_home_router(home: &Path) -> Result<PathBuf, HomeWriteError> {
    let dest = router_path(home);
    let rules = dest.parent().expect("router parent");
    let grok = rules.parent().expect("rules parent");
    ensure_real_dir(grok)?;
    ensure_real_dir(rules)?;
    match node_kind(&dest) {
        NodeKind::Missing => write_new_file(&dest, ROUTER_FIXTURE)?,
        NodeKind::Symlink => {
            fs::remove_file(&dest).map_err(|e| home_write_err(&dest, e.to_string()))?;
            write_new_file(&dest, ROUTER_FIXTURE)?;
        }
        NodeKind::Dir => return Err(home_write_err(&dest, "path is a directory")),
        NodeKind::File => replace_regular(&dest, ROUTER_FIXTURE)?,
        NodeKind::Other => return Err(home_write_err(&dest, "path is not a regular file")),
        NodeKind::Err(e) => return Err(home_write_err(&dest, e.to_string())),
    }
    Ok(dest)
}

fn site_label(site: RouterSite) -> &'static str {
    match site {
        RouterSite::Repo => "repo",
        RouterSite::Home => "home",
    }
}

enum NodeKind {
    Missing,
    Symlink,
    Dir,
    File,
    Other,
    Err(io::Error),
}

fn node_kind(path: &Path) -> NodeKind {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            let ft = meta.file_type();
            if ft.is_symlink() {
                NodeKind::Symlink
            } else if ft.is_dir() {
                NodeKind::Dir
            } else if ft.is_file() {
                NodeKind::File
            } else {
                NodeKind::Other
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => NodeKind::Missing,
        Err(e) => NodeKind::Err(e),
    }
}

fn push_hash_result(
    report: &mut DoctorReport,
    label: &str,
    path: &Path,
    expected_hash: &str,
    text: &str,
) {
    match parse_adr_hash(text) {
        Some(got) if got == expected_hash => {
            report.oks.push(format!(
                "{label} loop-router matches ADR-HASH {expected_hash} ({})",
                path.display()
            ));
        }
        Some(got) => {
            report.warnings.push(format!(
                "{label} loop-router stale vs ADR-HASH {expected_hash} (got {got}) ({})",
                path.display()
            ));
        }
        None => {
            report.warnings.push(format!(
                "{label} loop-router stale vs ADR-HASH {expected_hash} (no ADR-HASH line) ({})",
                path.display()
            ));
        }
    }
}

fn home_write_err(path: &Path, detail: impl Into<String>) -> HomeWriteError {
    HomeWriteError {
        path: path.to_path_buf(),
        detail: detail.into(),
    }
}

fn ensure_real_dir(path: &Path) -> Result<(), HomeWriteError> {
    match node_kind(path) {
        NodeKind::Dir => Ok(()),
        NodeKind::Missing => match fs::create_dir(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => match node_kind(path) {
                NodeKind::Dir => Ok(()),
                NodeKind::Symlink => Err(home_write_err(path, "parent is a symlink")),
                NodeKind::File | NodeKind::Other => {
                    Err(home_write_err(path, "parent is not a directory"))
                }
                NodeKind::Missing | NodeKind::Err(_) => Err(home_write_err(path, e.to_string())),
            },
            Err(e) => Err(home_write_err(path, e.to_string())),
        },
        NodeKind::Symlink => Err(home_write_err(path, "parent is a symlink")),
        NodeKind::File | NodeKind::Other => Err(home_write_err(path, "parent is not a directory")),
        NodeKind::Err(e) => Err(home_write_err(path, e.to_string())),
    }
}

fn write_new_file(path: &Path, bytes: &str) -> Result<(), HomeWriteError> {
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(e) => return Err(home_write_err(path, e.to_string())),
    };
    if let Err(e) = file.write_all(bytes.as_bytes()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(home_write_err(path, e.to_string()));
    }
    drop(file);
    // OpenOptions mode is masked by umask; set_permissions is not.
    if let Err(e) = fs::set_permissions(path, fs::Permissions::from_mode(0o644)) {
        let _ = fs::remove_file(path);
        return Err(home_write_err(path, e.to_string()));
    }
    Ok(())
}

fn replace_regular(dest: &Path, bytes: &str) -> Result<(), HomeWriteError> {
    let parent = dest
        .parent()
        .ok_or_else(|| home_write_err(dest, "no parent"))?;
    let tmp = parent.join(format!(".loop-router.md.tmp.{}", std::process::id()));
    if let Err(err) = write_new_file(&tmp, bytes) {
        let _ = fs::remove_file(&tmp);
        return Err(home_write_err(dest, err.detail));
    }
    if let Err(e) = fs::rename(&tmp, dest) {
        let _ = fs::remove_file(&tmp);
        return Err(home_write_err(dest, e.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    const ADR_0001: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../architecture/adr/0001-rust-operating-layer.md"
    ));

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-doctor-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn assert_repo_text(text: &str, path: &Path) {
        assert!(text.contains(&path.display().to_string()), "{text}");
        assert!(!text.contains("~/.grok"), "{text}");
        assert!(!text.contains("home loop-router"), "{text}");
        assert!(!text.contains("copy testdata"), "{text}");
    }

    fn adr_keep_current_payload(adr: &str) -> String {
        let d8 = adr.lines().find(|l| l.starts_with("| D8 |")).unwrap_or("");
        let d15 = adr.lines().find(|l| l.starts_with("| D15 |")).unwrap_or("");
        let start = adr.find("## Signal (D8)").unwrap();
        let rest = &adr[start..];
        let end = rest[1..].find("\n## ").map(|i| i + 1).unwrap_or(rest.len());
        let signal = rest[..end].trim_end();
        format!("{d8}\n{d15}\n{signal}\n")
    }

    fn file_sha256(path: &Path) -> String {
        for (bin, args) in [
            ("sha256sum", &[] as &[&str]),
            ("shasum", &["-a", "256"]),
            ("openssl", &["dgst", "-sha256"]),
        ] {
            let mut cmd = Command::new(bin);
            cmd.args(args).arg(path);
            if let Ok(out) = cmd.output() {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if let Some(hex) = s
                        .split_whitespace()
                        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
                    {
                        return hex.to_ascii_lowercase();
                    }
                }
            }
        }
        panic!("no sha256 tool for {}", path.display());
    }

    fn adr_hash(adr: &str) -> String {
        let tmp = Tmp::new();
        let path = tmp.root.join("payload");
        fs::write(&path, adr_keep_current_payload(adr)).unwrap();
        file_sha256(&path)
    }

    #[test]
    fn testdata_loop_router_matches_adr_hash_and_d8() {
        let want = adr_hash(ADR_0001);
        let got = parse_adr_hash(ROUTER_FIXTURE).expect("testdata/loop-router.md needs ADR-HASH");
        assert_eq!(got, want, "CI fixture ADR-HASH must track D8/D15 payload");
        assert_eq!(got, fixture_adr_hash());

        let body = ROUTER_FIXTURE.to_ascii_lowercase();
        assert!(
            body.contains("/crucible"),
            "fixture must name /crucible live → Crucible"
        );
        assert!(body.contains("grok-native"), "else branch is Grok-native");
        assert!(body.contains("next:"), "Grok-native keeps NEXT:");
        assert!(
            body.contains("interrupt"),
            "interrupt wins until /crucible or go"
        );
        assert!(
            body.contains("/execute-plan"),
            "must mention /execute-plan in order to forbid it"
        );
        assert!(
            body.contains("not") && body.contains("force") && body.contains("/execute-plan"),
            "must NOT force /execute-plan inside /crucible: {ROUTER_FIXTURE}"
        );
        assert!(
            !body.contains("inside /crucible, run /execute-plan")
                && !body.contains("inside /crucible: /execute-plan")
                && !body.contains("force /execute-plan inside /crucible"),
            "fixture must not force /execute-plan inside /crucible"
        );
        let users = concat!("/", "Users/");
        let home = concat!("/", "home/");
        assert!(
            !ROUTER_FIXTURE.contains(users) && !ROUTER_FIXTURE.contains(home),
            "CI fixture must not embed machine home paths"
        );
        let payload = adr_keep_current_payload(ADR_0001);
        assert!(payload.contains("| D8 |"), "payload includes frozen D8");
        assert!(payload.contains("## Signal (D8)"));
    }

    #[test]
    fn tracked_router_bytes_match_fixture() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.grok/rules/loop-router.md");
        let bytes = fs::read_to_string(&path).unwrap();
        assert_eq!(bytes, ROUTER_FIXTURE);
        let meta = fs::symlink_metadata(&path).unwrap();
        assert!(meta.file_type().is_file());
        assert!(!meta.file_type().is_symlink());
    }

    #[test]
    fn doctor_warns_missing_without_reading_process_home() {
        let tmp = Tmp::new();
        let path = router_path(&tmp.root);
        assert!(!path.exists());
        let r = check_router(&path, "abcd", RouterSite::Repo);
        assert!(!r.is_clean());
        let w = r.warnings.join("\n");
        assert!(w.contains("missing"), "{w}");
        assert_repo_text(&w, &path);
    }

    #[test]
    fn home_unset_warning_is_exact() {
        assert_eq!(
            HOME_UNSET_WARNING,
            "home loop-router not written (HOME unset)"
        );
        assert!(!HOME_UNSET_WARNING.contains("~/.grok"));
        assert!(!HOME_UNSET_WARNING.contains("copy testdata"));
    }

    #[test]
    fn doctor_warns_stale_hash() {
        let tmp = Tmp::new();
        let path = router_path(&tmp.root);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "ADR-HASH: 0000000000000000000000000000000000000000000000000000000000000000\n",
        )
        .unwrap();
        let want = fixture_adr_hash();
        let r = check_router(&path, &want, RouterSite::Repo);
        let w = r.warnings.join("\n");
        assert!(w.contains("stale"), "{w}");
        assert!(w.contains("got 0000000000000000000000000000000000000000000000000000000000000000"));
        assert_repo_text(&w, &path);
    }

    #[test]
    fn doctor_ok_when_router_has_current_hash() {
        let tmp = Tmp::new();
        let path = router_path(&tmp.root);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, ROUTER_FIXTURE).unwrap();
        let want = fixture_adr_hash();
        let r = check_router(&path, &want, RouterSite::Repo);
        assert!(r.is_clean(), "{:?}", r.warnings);
        let ok = r.oks.join("\n");
        assert!(ok.contains(&want), "{ok}");
        assert!(ok.contains("repo loop-router matches ADR-HASH"), "{ok}");
        assert_repo_text(&ok, &path);
    }

    #[test]
    fn doctor_stale_when_hash_line_missing() {
        let tmp = Tmp::new();
        let path = router_path(&tmp.root);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "/crucible live → follow Crucible\n").unwrap();
        let r = check_router(&path, &fixture_adr_hash(), RouterSite::Repo);
        let w = r.warnings.join("\n");
        assert!(w.contains("stale"), "{w}");
        assert!(w.contains("no ADR-HASH line"), "{w}");
        assert_repo_text(&w, &path);
    }

    #[test]
    fn doctor_does_not_read_symlink_router() {
        let tmp = Tmp::new();
        let path = router_path(&tmp.root);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let sentinel = tmp.root.join("sentinel.md");
        let body = "ADR-HASH: 1111111111111111111111111111111111111111111111111111111111111111\n";
        fs::write(&sentinel, body).unwrap();
        symlink(&sentinel, &path).unwrap();
        let r = check_router(&path, &fixture_adr_hash(), RouterSite::Repo);
        let w = r.warnings.join("\n");
        assert!(w.contains("not a regular file"), "{w}");
        assert!(!w.contains("stale"), "{w}");
        assert_repo_text(&w, &path);
        assert_eq!(fs::read_to_string(&sentinel).unwrap(), body);
    }

    #[test]
    fn doctor_does_not_follow_grok_parent_symlink() {
        let tmp = Tmp::new();
        let real = tmp.root.join("real");
        fs::create_dir_all(real.join("rules")).unwrap();
        let sentinel_hash = "2222222222222222222222222222222222222222222222222222222222222222";
        fs::write(
            real.join("rules/loop-router.md"),
            format!("ADR-HASH: {sentinel_hash}\n"),
        )
        .unwrap();
        let grok = tmp.root.join(".grok");
        symlink(&real, &grok).unwrap();
        let path = router_path(&tmp.root);
        let r = check_router(&path, &fixture_adr_hash(), RouterSite::Repo);
        let w = r.warnings.join("\n");
        assert!(w.contains("repo loop-router parent is a symlink"), "{w}");
        assert!(w.contains(&grok.display().to_string()), "{w}");
        assert!(!w.contains("stale"), "{w}");
        assert!(!w.contains(sentinel_hash), "{w}");
        assert_repo_text(&w, &grok);
    }

    #[test]
    fn install_home_router_writes_fixture_and_refuses_parent_symlink() {
        let fresh = Tmp::new();
        let path = install_home_router(&fresh.root).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), ROUTER_FIXTURE);
        let meta = fs::symlink_metadata(&path).unwrap();
        assert!(meta.file_type().is_file());
        assert!(!meta.file_type().is_symlink());
        assert_eq!(meta.permissions().mode() & 0o777, 0o644);
        let again = install_home_router(&fresh.root).unwrap();
        assert_eq!(fs::read_to_string(&again).unwrap(), ROUTER_FIXTURE);
        let names: Vec<_> = fs::read_dir(again.parent().unwrap())
            .unwrap()
            .map(|ent| ent.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("loop-router.md")]);

        let linked = Tmp::new();
        let home = linked.root.join("home");
        fs::create_dir_all(home.join(".grok/rules")).unwrap();
        let sentinel = linked.root.join("sentinel");
        fs::write(&sentinel, "do-not-touch").unwrap();
        let dest = home.join(".grok/rules/loop-router.md");
        symlink(&sentinel, &dest).unwrap();
        let written = install_home_router(&home).unwrap();
        assert_eq!(fs::read_to_string(&written).unwrap(), ROUTER_FIXTURE);
        let written_meta = fs::symlink_metadata(&written).unwrap();
        assert!(written_meta.file_type().is_file());
        assert!(!written_meta.file_type().is_symlink());
        assert_eq!(written_meta.permissions().mode() & 0o777, 0o644);
        assert_eq!(fs::read_to_string(&sentinel).unwrap(), "do-not-touch");

        let blocked = Tmp::new();
        let outside = blocked.root.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("marker"), "untouched").unwrap();
        let home = blocked.root.join("home");
        fs::create_dir_all(&home).unwrap();
        symlink(&outside, home.join(".grok")).unwrap();
        let err = install_home_router(&home).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("parent is a symlink"), "{msg}");
        assert!(msg.contains("not written"), "{msg}");
        assert!(!msg.contains("copy testdata"), "{msg}");
        assert!(!msg.contains("~/.grok"), "{msg}");
        assert!(!outside.join("rules").exists());
        assert!(!outside.join("loop-router.md").exists());
        assert_eq!(
            fs::read_to_string(outside.join("marker")).unwrap(),
            "untouched"
        );
    }

    #[test]
    fn parse_adr_hash_takes_first_whitespace_field() {
        let h = "ef61196fb93aad0ff3c782506a2e49a7cd05a112d19c4aafe44ed272c17a295d";
        assert_eq!(
            parse_adr_hash(&format!("ADR-HASH: {h} trailing note\n")),
            Some(h.to_string())
        );
        assert_eq!(parse_adr_hash("title\nADR-HASH: not-hex\n"), None);
        assert_eq!(parse_adr_hash("**ADR-HASH:** deadbeef\n"), None);
    }
}
