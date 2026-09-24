//! `run-claim`. Evidence `when:` is [`format_rfc3339_z`] of [`Clock::now_unix`], not `date -u`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crucible_contract::{format_rfc3339_z, Clock};

use crate::cycle::MARK;
use crate::panel::kind_of;
use crate::{message, GuidedError};

static RUN_SEQ: AtomicU64 = AtomicU64::new(0);

/// `crucible run-claim CN NAME -- CMD...`. Command failure is recorded, not returned.
pub fn run_claim(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if args.len() < 2 {
        return Err(message("usage: crucible run-claim CN NAME -- CMD..."));
    }
    let cn = args[0];
    let name = args[1];
    let rest = &args[2..];
    if rest.first().copied() != Some("--") {
        return Err(message("usage: crucible run-claim CN NAME -- CMD..."));
    }
    let cmd = &rest[1..];
    if cmd.is_empty() {
        return Err(message("no command given"));
    }
    if kind_of(root, name)?.is_empty() {
        return Err(message(format!(
            "unregistered agent: {name} — add '{name}<TAB>kind' to {}/agents.tsv",
            root.display()
        )));
    }
    let cdir = root.join("claims").join(cn);
    if !cdir.is_dir() {
        return Err(message(format!("no such claim: {cn}")));
    }
    let evidence = cdir.join("evidence");
    fs::create_dir_all(&evidence)?;
    let seq = RUN_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    let tok = format!("p{}s{seq}", std::process::id());
    let out = evidence.join(format!("{name}.{tok}.txt"));
    let tmp = evidence.join(format!(".partial.{name}.{tok}"));
    let mut header = String::new();
    header.push_str(MARK);
    header.push('\n');
    header.push_str(&format!("agent: {name}\nclaim: {cn}\n"));
    // Shell line 1334: `date -u +%Y-%m-%dT%H:%M:%SZ`.
    header.push_str(&format!(
        "when: {}\ncommand:",
        format_rfc3339_z(clock.now_unix())
    ));
    for arg in cmd {
        header.push(' ');
        header.push_str(arg);
    }
    header.push_str("\n--- output ---\n");
    fs::write(&tmp, header)?;
    let rc = match append_command(&tmp, cmd) {
        Ok(code) => code,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => 127,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => 126,
        Err(err) => return Err(err.into()),
    };
    let mut file = OpenOptions::new().append(true).open(&tmp)?;
    writeln!(file, "--- exit {rc} ---")?;
    drop(file);
    fs::rename(&tmp, &out)?;
    Ok(format!("{} (exit {rc})\n", out.display()))
}

fn append_command(tmp: &Path, cmd: &[&str]) -> std::io::Result<i32> {
    let file = OpenOptions::new().append(true).open(tmp)?;
    let stdout = Stdio::from(file.try_clone()?);
    let stderr = Stdio::from(file);
    let mut child = Command::new(cmd[0])
        .args(&cmd[1..])
        .stdout(stdout)
        .stderr(stderr)
        .spawn()?;
    let status = child.wait()?;
    Ok(exit_code(status))
}

fn exit_code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    if let Some(code) = status.code() {
        return code;
    }
    status.signal().map(|sig| 128 + sig).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crucible_contract::FixedClock;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-run-claim-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn evidence_when_line_uses_fixed_clock() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::create_dir_all(root.join("claims/C1")).unwrap();
        fs::write(root.join("agents.tsv"), "a1\tkindA\tm\thigh\ttrue\n").unwrap();
        let clock = FixedClock::new(1_700_000_000);
        let out = run_claim(root, &["C1", "a1", "--", "/bin/echo", "hello"], &clock).unwrap();
        assert!(out.contains("(exit 0)\n"), "{out}");
        let path = out.trim_end().trim_end_matches(" (exit 0)");
        let body = fs::read_to_string(path).unwrap();
        assert_eq!(
            body,
            "\
crucible-run/1
agent: a1
claim: C1
when: 2023-11-14T22:13:20Z
command: /bin/echo hello
--- output ---
hello
--- exit 0 ---
"
        );

        let failed =
            run_claim(root, &["C1", "a1", "--", "/bin/sh", "-c", "exit 3"], &clock).unwrap();
        assert!(failed.contains("(exit 3)\n"), "{failed}");
        let missing = run_claim(
            root,
            &["C1", "a1", "--", "definitely-not-a-command-xyz"],
            &clock,
        )
        .unwrap();
        assert!(missing.contains("(exit 127)\n"), "{missing}");
        let missing_path = missing.trim_end().trim_end_matches(" (exit 127)");
        let missing_body = fs::read_to_string(missing_path).unwrap();
        assert!(
            missing_body.contains("when: 2023-11-14T22:13:20Z\n"),
            "{missing_body}"
        );
        assert!(
            missing_body.ends_with("--- exit 127 ---\n"),
            "{missing_body}"
        );
    }

    #[test]
    fn usage_and_registration_match_the_shell() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let clock = FixedClock::new(1);
        assert_eq!(
            run_claim(root, &["C1"], &clock).unwrap_err().to_string(),
            "usage: crucible run-claim CN NAME -- CMD..."
        );
        assert_eq!(
            run_claim(root, &["C1", "a1", "echo"], &clock)
                .unwrap_err()
                .to_string(),
            "usage: crucible run-claim CN NAME -- CMD..."
        );
        assert_eq!(
            run_claim(root, &["C1", "a1", "--"], &clock)
                .unwrap_err()
                .to_string(),
            "no command given"
        );
        fs::write(root.join("agents.tsv"), "a1\tkindA\tm\thigh\ttrue\n").unwrap();
        assert_eq!(
            run_claim(root, &["C1", "a1", "--", "/bin/echo", "x"], &clock)
                .unwrap_err()
                .to_string(),
            "no such claim: C1".to_string()
        );
        fs::create_dir_all(root.join("claims/C1")).unwrap();
        assert_eq!(
            run_claim(root, &["C1", "nope", "--", "/bin/echo", "x"], &clock)
                .unwrap_err()
                .to_string(),
            format!(
                "unregistered agent: nope — add 'nope<TAB>kind' to {}/agents.tsv",
                root.display()
            )
        );
    }
}
