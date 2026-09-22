//! Home Grok loop-router keep-current (D8/D15).
//!
//! Doctor compares the home `ADR-HASH` token to the compile-time fixture.
//! Callers inject the home path; unit tests never read process `$HOME`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const ROUTER_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/loop-router.md"
));

const HASH_KEY: &str = "ADR-HASH:";

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

pub fn home_router_path(home: Option<&Path>) -> Option<PathBuf> {
    Some(home?.join(".grok").join("rules").join("loop-router.md"))
}

/// `ADR-HASH` baked into `testdata/loop-router.md` (CI keeps that line in sync with the ADR).
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

/// Compare an injected home path to the expected ADR hash (fixture is the golden copy).
pub fn check_router(home_router: Option<&Path>, expected_hash: &str) -> DoctorReport {
    let mut report = DoctorReport {
        warnings: Vec::new(),
        oks: Vec::new(),
    };
    let Some(path) = home_router else {
        report.warnings.push(
            "home loop-router missing (HOME unset); copy testdata/loop-router.md to ~/.grok/rules/loop-router.md"
                .to_string(),
        );
        return report;
    };
    if !path.is_file() {
        report.warnings.push(format!(
            "home loop-router missing ({}); copy testdata/loop-router.md",
            path.display()
        ));
        return report;
    }
    let text = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            report.warnings.push(format!(
                "home loop-router unreadable ({}): {e}",
                path.display()
            ));
            return report;
        }
    };
    match parse_adr_hash(&text) {
        Some(got) if got == expected_hash => {
            report
                .oks
                .push(format!("home loop-router matches ADR-HASH {expected_hash}"));
        }
        Some(got) => {
            report.warnings.push(format!(
                "home loop-router stale vs ADR-HASH {expected_hash} (got {got})"
            ));
        }
        None => {
            report.warnings.push(format!(
                "home loop-router stale vs ADR-HASH {expected_hash} (no ADR-HASH line)"
            ));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn router_path(tmp: &Tmp) -> PathBuf {
        tmp.root.join(".grok").join("rules").join("loop-router.md")
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
        assert!(
            !ROUTER_FIXTURE.contains("/Users/") && !ROUTER_FIXTURE.contains("/home/"),
            "CI fixture must not embed machine home paths"
        );
        let payload = adr_keep_current_payload(ADR_0001);
        assert!(payload.contains("| D8 |"), "payload includes frozen D8");
        assert!(payload.contains("## Signal (D8)"));
    }

    #[test]
    fn doctor_warns_missing_without_reading_process_home() {
        let tmp = Tmp::new();
        let path = home_router_path(Some(&tmp.root)).unwrap();
        assert!(!path.exists());
        let r = check_router(Some(&path), "abcd");
        assert!(!r.is_clean());
        assert!(
            r.warnings.iter().any(|w| w.contains("missing")),
            "{:?}",
            r.warnings
        );
        let none = check_router(None, "abcd");
        assert!(none.warnings.iter().any(|w| w.contains("missing")));
    }

    #[test]
    fn doctor_warns_stale_hash() {
        let tmp = Tmp::new();
        let path = router_path(&tmp);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "ADR-HASH: 0000000000000000000000000000000000000000000000000000000000000000\n",
        )
        .unwrap();
        let want = fixture_adr_hash();
        let r = check_router(Some(&path), &want);
        assert!(
            r.warnings.iter().any(|w| w.contains("stale")),
            "{:?}",
            r.warnings
        );
    }

    #[test]
    fn doctor_ok_when_home_copy_has_current_hash() {
        let tmp = Tmp::new();
        let path = router_path(&tmp);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, ROUTER_FIXTURE).unwrap();
        let want = fixture_adr_hash();
        let r = check_router(Some(&path), &want);
        assert!(r.is_clean(), "{:?}", r.warnings);
        assert!(r.oks.iter().any(|o| o.contains(&want)));
    }

    #[test]
    fn doctor_stale_when_hash_line_missing() {
        let tmp = Tmp::new();
        let path = router_path(&tmp);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "/crucible live → follow Crucible\n").unwrap();
        let r = check_router(Some(&path), &fixture_adr_hash());
        assert!(r.warnings.iter().any(|w| w.contains("stale")));
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
