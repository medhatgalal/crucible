//! Home Grok loop-router keep-current (D8/D15).
//!
//! Doctor warns when the home copy is missing or stale versus the ADR hash.
//! Callers inject the home path; unit tests never read process `$HOME`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const ADR_0001: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../architecture/adr/0001-rust-operating-layer.md"
));

#[cfg(test)]
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

pub fn adr_keep_current_payload(adr: &str) -> String {
    let d8 = adr.lines().find(|l| l.starts_with("| D8 |")).unwrap_or("");
    let d15 = adr.lines().find(|l| l.starts_with("| D15 |")).unwrap_or("");
    let signal = signal_d8_section(adr).unwrap_or("");
    format!("{d8}\n{d15}\n{signal}\n")
}

fn signal_d8_section(adr: &str) -> Option<&str> {
    let start = adr.find("## Signal (D8)")?;
    let rest = &adr[start..];
    let end = rest[1..].find("\n## ").map(|i| i + 1).unwrap_or(rest.len());
    Some(rest[..end].trim_end())
}

pub fn adr_hash(adr: &str) -> String {
    sha256_hex(adr_keep_current_payload(adr).as_bytes())
}

pub fn parse_adr_hash(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(HASH_KEY).or_else(|| {
            line.strip_prefix("**")
                .and_then(|s| s.strip_prefix(HASH_KEY))
                .or_else(|| {
                    line.strip_prefix('`')
                        .and_then(|s| s.strip_prefix(HASH_KEY))
                })
        }) else {
            continue;
        };
        let hex: String = rest
            .trim()
            .trim_matches('`')
            .trim_matches('*')
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .take(64)
            .collect();
        if hex.len() == 64 {
            return Some(hex.to_ascii_lowercase());
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

pub fn sha256_hex(data: &[u8]) -> String {
    sha256(data).iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut msg = Vec::with_capacity(data.len() + 72);
    msg.extend_from_slice(data);
    let bit_len = (data.len() as u64).saturating_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            let j = i * 4;
            *word = u32::from_be_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut hh = state[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, v) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

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

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn testdata_loop_router_matches_adr_hash_and_d8() {
        let want = adr_hash(ADR_0001);
        let got = parse_adr_hash(ROUTER_FIXTURE).expect("testdata/loop-router.md needs ADR-HASH");
        assert_eq!(got, want, "CI fixture ADR-HASH must track D8/D15 payload");

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
        assert!(
            adr_keep_current_payload(ADR_0001).contains("| D8 |"),
            "payload includes frozen D8"
        );
        assert!(adr_keep_current_payload(ADR_0001).contains("## Signal (D8)"));
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
        let want = adr_hash(ADR_0001);
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
        let want = adr_hash(ADR_0001);
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
        let r = check_router(Some(&path), &adr_hash(ADR_0001));
        assert!(r.warnings.iter().any(|w| w.contains("stale")));
    }
}
