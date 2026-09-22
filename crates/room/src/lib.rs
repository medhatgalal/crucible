//! `crucible room` is a client of GET HTTP serve plus a required Herdr PATH gate.
//!
//! Not a copy of herdr-init. Standing role names are the documented contract.
//! First slice: require `herdr` on PATH, spawn **this** binary's
//! `serve --bind 127.0.0.1:0`, print the contract, GET `/health`. No `POST /go`.

use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const SERVE_BIND: &str = "127.0.0.1:0";

/// True when an executable named `herdr` is on `PATH`.
fn path_has_herdr(path: impl AsRef<OsStr>) -> bool {
    for dir in std::env::split_paths(path.as_ref()) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if is_executable(&dir.join("herdr")) {
            return true;
        }
    }
    false
}

fn is_executable(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

struct ChildGuard {
    child: Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Require `herdr` on `path`, then spawn `exe serve --bind 127.0.0.1:0` (not PATH `wm`/`crucible`).
pub fn run(exe: &Path, cwd: &Path, path: &OsStr) -> i32 {
    if !path_has_herdr(path) {
        let _ = writeln!(io::stderr(), "room: herdr not found on PATH");
        return 2;
    }
    match spawn_and_health(exe, cwd) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(io::stderr(), "room: {e}");
            1
        }
    }
}

fn spawn_and_health(exe: &Path, cwd: &Path) -> Result<(), String> {
    let mut child = Command::new(exe)
        .args(["serve", "--bind", SERVE_BIND])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {exe:?} serve: {e}"))?;
    let mut pipe = child
        .stdout
        .take()
        .ok_or_else(|| "serve stdout".to_string())?;
    let mut err_pipe = child.stderr.take();
    let mut guard = ChildGuard { child };
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = [0u8; 256];
        match pipe.read(&mut buf) {
            Ok(n) => {
                let _ = tx.send(String::from_utf8_lossy(&buf[..n]).into_owned());
            }
            Err(e) => {
                let _ = tx.send(format!("read-err {e}"));
            }
        }
    });
    let line = match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(s) => s,
        Err(_) => {
            let mut err = String::new();
            if let Some(ref mut s) = err_pipe {
                let _ = s.read_to_string(&mut err);
            }
            let st = guard.child.try_wait();
            return Err(format!(
                "serve did not print listening: stderr={err:?} status={st:?}"
            ));
        }
    };
    let addr = line
        .lines()
        .find_map(|l| l.strip_prefix("listening "))
        .unwrap_or(line.trim())
        .trim()
        .to_string();
    if !(addr.starts_with("127.0.0.1:") || addr.starts_with("[::1]:")) {
        return Err(format!("serve bind must be loopback, got {line:?}"));
    }
    let body = get_health(&addr)?;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let _ = writeln!(
        stdout,
        "standing roles: chat, orchestrator, watcher, reaper, dashboard"
    );
    let _ = writeln!(stdout, "listening {addr}");
    let _ = writeln!(stdout, "GET /health {addr}");
    let _ = writeln!(stdout, "{body}");
    let _ = stdout.flush();
    drop(guard);
    Ok(())
}

fn get_health(addr: &str) -> Result<String, String> {
    let sock: std::net::SocketAddr = addr
        .parse()
        .map_err(|e| format!("serve addr {addr:?}: {e}"))?;
    let mut last = None;
    for _ in 0..50 {
        match TcpStream::connect_timeout(&sock, Duration::from_millis(100)) {
            Ok(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                write!(
                    s,
                    "GET /health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
                )
                .map_err(|e| e.to_string())?;
                let mut raw = Vec::new();
                s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
                let text = String::from_utf8_lossy(&raw);
                let body = text
                    .split("\r\n\r\n")
                    .nth(1)
                    .or_else(|| text.split("\n\n").nth(1))
                    .unwrap_or("")
                    .trim();
                if body.is_empty() {
                    return Err("GET /health empty body".to_string());
                }
                return Ok(body.to_string());
            }
            Err(e) => {
                last = Some(e);
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    Err(format!("GET /health {addr}: {last:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-room-{}-{}",
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

    fn write_exec(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = fs::metadata(path).unwrap().permissions();
            p.set_mode(0o755);
            fs::set_permissions(path, p).unwrap();
        }
    }

    #[test]
    fn standing_roles_are_the_documented_contract() {
        let prod = include_str!("lib.rs").split("#[cfg(test)]").next().unwrap();
        assert!(prod.contains("standing roles: chat, orchestrator, watcher, reaper, dashboard"));
    }

    #[test]
    fn path_has_herdr_false_when_path_has_no_herdr() {
        let tmp = Tmp::new();
        let empty = tmp.root.join("empty");
        fs::create_dir(&empty).unwrap();
        assert!(!path_has_herdr(empty.as_os_str()));
    }

    #[test]
    fn path_has_herdr_true_for_executable_on_path() {
        let tmp = Tmp::new();
        let bin = tmp.root.join("bin");
        fs::create_dir(&bin).unwrap();
        write_exec(&bin.join("herdr"), "#!/bin/sh\nexit 0\n");
        assert!(path_has_herdr(bin.as_os_str()));
    }

    #[test]
    fn missing_herdr_does_not_spawn_exe() {
        let tmp = Tmp::new();
        let empty = tmp.root.join("empty");
        fs::create_dir(&empty).unwrap();
        let exe = tmp.root.join("crucible");
        write_exec(
            &exe,
            "#!/bin/sh\nprintf spawned > \"$(dirname \"$0\")/SPAWNED\"\nexit 0\n",
        );
        let code = run(&exe, &tmp.root, empty.as_os_str());
        assert_eq!(code, 2, "missing herdr is nonzero");
        assert!(
            !tmp.root.join("SPAWNED").exists(),
            "must not spawn current_exe serve when herdr is missing"
        );
    }

    #[test]
    fn room_source_does_not_bind_a_listener() {
        let src = include_str!("lib.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        assert!(
            !prod.contains("TcpListener"),
            "room is a GET client; it must not bind"
        );
        assert!(!prod.contains("bind_listener"));
    }

    #[test]
    fn cargo_tree_kernel_and_contract_have_no_herdr() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for pkg in ["crucible-kernel", "crucible-contract"] {
            let out = Command::new("cargo")
                .args(["tree", "-p", pkg, "-e", "normal"])
                .current_dir(&root)
                .output()
                .expect("cargo tree");
            assert!(
                out.status.success(),
                "cargo tree -p {pkg}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let tree = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            assert!(!tree.contains("herdr"), "herdr in {pkg} tree:\n{tree}");
            assert!(!tree.contains("grok"), "grok in {pkg} tree:\n{tree}");
            assert!(!tree.contains("engos"), "engos in {pkg} tree:\n{tree}");
        }
    }
}
