//! `crucible room` probes loopback `GET /health` before listen, then asks
//! external `herdr` for standing tabs. Cameras GET the API. `go` is a process
//! in the orchestrator tab, never `POST /go`. Do not vendor an init tree.
//! No Herdr crate.

use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::Value;

const ROOM_BIND: &str = "127.0.0.1:1734";

const PRODUCT_VERSION: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../VERSION"));

pub const ROLES: [&str; 5] = ["chat", "orchestrator", "watcher", "reaper", "dashboard"];

fn product_version() -> &'static str {
    PRODUCT_VERSION.trim()
}

fn resolve_herdr(path: &OsStr) -> Option<PathBuf> {
    for dir in std::env::split_paths(path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let candidate = dir.join("herdr");
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
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

/// Read `<cwd>/.crucible/herdr/` before herdr and before listen. A bad layout
/// exits 2 with no herdr and no listen. Missing herdr: exit 2, no listen, no
/// TRACE. Any other probe result: exit 1, no listen, no herdr calls.
pub fn run(exe: &Path, cwd: &Path, path: &OsStr, herdr_override: Option<&OsStr>) -> i32 {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    drive(
        exe,
        cwd,
        path,
        herdr_override,
        ROOM_BIND,
        &mut stdout,
        &mut stderr,
    )
}

// Resolve before the probe so a missing binary never listens.
fn locate_herdr(
    path: &OsStr,
    override_bin: Option<&OsStr>,
    err: &mut dyn Write,
) -> Option<PathBuf> {
    if let Some(found) = resolve_herdr(path) {
        return Some(found);
    }
    let Some(raw) = override_bin else {
        let _ = writeln!(err, "room: herdr not found on PATH");
        return None;
    };
    let candidate = PathBuf::from(raw);
    if is_executable(&candidate) {
        let _ = writeln!(
            err,
            "room: herdr not found on PATH; using CRUCIBLE_HERDR {}",
            candidate.display()
        );
        Some(candidate)
    } else {
        let _ = writeln!(
            err,
            "room: herdr not found on PATH; CRUCIBLE_HERDR {} is missing or not executable",
            candidate.display()
        );
        None
    }
}

#[derive(Debug)]
enum HealthProbe {
    Refused,
    Match(String),
    Other(String),
}

fn health_matches(body: &str, version: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return false;
    };
    v.get("ok").and_then(Value::as_bool) == Some(true)
        && v.get("version").and_then(Value::as_str) == Some(version)
}

/// One connection: 100ms connect, then a 2s read. No reconnect.
fn probe_health(addr: &str, version: &str) -> HealthProbe {
    let sock: SocketAddr = match addr.parse() {
        Ok(s) => s,
        Err(e) => return HealthProbe::Other(format!("addr {addr}: {e}")),
    };
    let mut stream = match TcpStream::connect_timeout(&sock, Duration::from_millis(100)) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => return HealthProbe::Refused,
        Err(e) => return HealthProbe::Other(format!("GET /health {addr}: {e}")),
    };
    if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(2))) {
        return HealthProbe::Other(format!("GET /health {addr}: {e}"));
    }
    if let Err(e) = stream.set_write_timeout(Some(Duration::from_secs(2))) {
        return HealthProbe::Other(format!("GET /health {addr}: {e}"));
    }
    if let Err(e) = write!(
        stream,
        "GET /health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .and_then(|_| stream.flush())
    {
        return HealthProbe::Other(format!("GET /health {addr}: {e}"));
    }
    let mut raw = Vec::new();
    if let Err(e) = stream.read_to_end(&mut raw) {
        return HealthProbe::Other(format!("GET /health {addr}: {e}"));
    }
    let text = String::from_utf8_lossy(&raw);
    let body = text
        .split("\r\n\r\n")
        .nth(1)
        .or_else(|| text.split("\n\n").nth(1))
        .unwrap_or("")
        .trim();
    if body.is_empty() {
        return HealthProbe::Other(format!("GET /health {addr}: empty body"));
    }
    if health_matches(body, version) {
        HealthProbe::Match(body.to_string())
    } else {
        HealthProbe::Other(format!("GET /health {addr}: version mismatch"))
    }
}

fn drive(
    exe: &Path,
    cwd: &Path,
    path: &OsStr,
    herdr_override: Option<&OsStr>,
    bind: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    // A bad layout must not resolve herdr or listen.
    if let Err(e) = load_room_layout(cwd) {
        let _ = writeln!(err, "room: {e}");
        return 2;
    }
    let Some(herdr) = locate_herdr(path, herdr_override, err) else {
        return 2;
    };
    let version = product_version();
    // Only connection refused may spawn. Anything else must not listen.
    let (addr, body, spawned) = match probe_health(bind, version) {
        HealthProbe::Match(body) => (bind.to_string(), body, None),
        HealthProbe::Refused => {
            let (addr, pid, guard) = match spawn_serve(exe, cwd, bind) {
                Ok(v) => v,
                Err(e) => {
                    let _ = writeln!(err, "room: {e}");
                    return 1;
                }
            };
            match probe_health(&addr, version) {
                HealthProbe::Match(body) => (addr, body, Some((pid, guard))),
                HealthProbe::Refused => {
                    let _ = writeln!(err, "room: spawned serve did not accept {addr}");
                    return 1;
                }
                HealthProbe::Other(msg) => {
                    let _ = writeln!(err, "room: {msg}");
                    return 1;
                }
            }
        }
        HealthProbe::Other(msg) => {
            let _ = writeln!(err, "room: {msg}");
            return 1;
        }
    };
    let report = match arrange(&herdr, exe, cwd, &addr) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "room: {e}");
            return 1;
        }
    };
    let _ = writeln!(
        out,
        "standing roles: chat, orchestrator, watcher, reaper, dashboard"
    );
    if let Some((pid, guard)) = spawned {
        let _ = writeln!(out, "listening {addr}");
        let _ = writeln!(out, "serve pid {pid}");
        // Serve stays up for cameras. Forgetting the guard skips the kill-on-drop.
        std::mem::forget(guard);
    } else {
        let _ = writeln!(out, "serve reused");
    }
    let _ = writeln!(out, "GET /health {addr}");
    let _ = writeln!(out, "{body}");
    let _ = writeln!(out, "workspace {}", report.workspace_id);
    let _ = writeln!(out, "tabs {}", report.labels.join(" "));
    // Print the three-way line. started_go stays true only for Started.
    let go_line = match report.go {
        GoLine::Started => "go orchestrator",
        GoLine::Waiting => "go waiting",
        GoLine::AlreadyOpen => "go not started (orchestrator tab already open)",
    };
    let _ = writeln!(out, "{go_line}");
    let _ = out.flush();
    0
}

fn spawn_serve(exe: &Path, cwd: &Path, bind: &str) -> Result<(String, u32, ChildGuard), String> {
    let mut child = Command::new(exe)
        .args(["serve", "--bind", bind])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {exe:?} serve: {e}"))?;
    let pid = child.id();
    let mut pipe = child
        .stdout
        .take()
        .ok_or_else(|| "serve stdout".to_string())?;
    let mut err_pipe = child.stderr.take();
    let guard = ChildGuard { child };
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
            return Err(format!("serve did not print listening: stderr={err:?}"));
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
    Ok((addr, pid, guard))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GoLine {
    Started,
    Waiting,
    AlreadyOpen,
}

#[derive(Debug)]
struct RoomReport {
    workspace_id: String,
    labels: Vec<String>,
    // Arrange tests assert this. drive prints `go` and does not branch on it.
    #[allow(dead_code)]
    started_go: bool,
    go: GoLine,
}

struct RoomLayout {
    label: String,
}

struct CreatedTab {
    role: &'static str,
    tab_id: String,
    pane_id: Option<String>,
}

fn arrange(herdr: &Path, exe: &Path, cwd: &Path, addr: &str) -> Result<RoomReport, String> {
    let layout = load_room_layout(cwd)?;
    let workspace_id = attach_workspace(herdr, &layout.label)?;
    let mut created = ensure_tabs(herdr, cwd, &workspace_id)?;
    // A failed create already returned. Do not pane-run any role, including ones created earlier.
    resolve_created_panes(herdr, &workspace_id, &mut created)?;
    let exe_s = exe.display().to_string();
    let ready = intake_ready(cwd);
    let orchestrator = created.iter().find(|tab| tab.role == "orchestrator");
    let go = if !ready {
        GoLine::Waiting
    } else if let Some(orch) = orchestrator {
        let pane = created_pane(orch)?;
        herdr_ok(herdr, &["pane", "run", pane, &exe_s, "go"])?;
        GoLine::Started
    } else {
        GoLine::AlreadyOpen
    };
    // Reap only a reaper tab this call created, and only after go started now.
    if go == GoLine::Started {
        let orch = created.iter().find(|tab| tab.role == "orchestrator");
        let reaper = created.iter().find(|tab| tab.role == "reaper");
        if let (Some(orch), Some(reaper)) = (orch, reaper) {
            if let Some(pid) = go_pid(herdr, created_pane(orch)?) {
                let pid_s = pid.to_string();
                herdr_ok(
                    herdr,
                    &[
                        "pane",
                        "run",
                        created_pane(reaper)?,
                        &exe_s,
                        "reap",
                        "--pid",
                        &pid_s,
                    ],
                )?;
            }
        }
    }
    for role in ["watcher", "dashboard"] {
        if let Some(tab) = created.iter().find(|tab| tab.role == role) {
            herdr_ok(
                herdr,
                &[
                    "pane",
                    "run",
                    created_pane(tab)?,
                    &exe_s,
                    "camera",
                    "--bind",
                    addr,
                ],
            )?;
        }
    }
    let labels: Vec<String> = ROLES.iter().map(|s| (*s).to_string()).collect();
    Ok(RoomReport {
        workspace_id,
        labels,
        started_go: go == GoLine::Started,
        go,
    })
}

fn created_pane(tab: &CreatedTab) -> Result<&str, String> {
    tab.pane_id
        .as_deref()
        .ok_or_else(|| no_single_pane(tab.role))
}

fn load_room_layout(cwd: &Path) -> Result<RoomLayout, String> {
    let label = workspace_label(&cwd.join(".crucible/herdr/workspace"))?;
    roles_match(&cwd.join(".crucible/herdr/roles"))?;
    Ok(RoomLayout { label })
}

fn read_regular(path: &Path) -> Result<Vec<u8>, String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("{} is missing", path.display()));
        }
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if meta.file_type().is_symlink() {
        return Err(format!("{} is a symlink", path.display()));
    }
    if !meta.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn workspace_label(path: &Path) -> Result<String, String> {
    let bytes = read_regular(path)?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| format!("{} is not UTF-8", path.display()))?;
    // One trailing newline is the line ending. A second newline is another line.
    let line = text.strip_suffix('\n').unwrap_or(text);
    if line.contains('\n') || line.contains('\r') {
        return Err(format!("{} is multi-line", path.display()));
    }
    let trimmed = line.trim_ascii();
    if trimmed.is_empty() {
        return Err(format!("{} is empty", path.display()));
    }
    if trimmed.bytes().any(|b| b.is_ascii_whitespace()) {
        return Err(format!("{} has internal whitespace", path.display()));
    }
    Ok(trimmed.to_string())
}

fn roles_match(path: &Path) -> Result<(), String> {
    let bytes = read_regular(path)?;
    let text =
        std::str::from_utf8(&bytes).map_err(|_| format!("{} is not UTF-8", path.display()))?;
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines == ROLES {
        Ok(())
    } else {
        Err(format!(
            "{} is not the five role names in order",
            path.display()
        ))
    }
}

fn attach_workspace(herdr: &Path, label: &str) -> Result<String, String> {
    let listed = herdr_ok(herdr, &["workspace", "list"])?;
    let mut ids = workspace_ids_for_label(&listed, label)?;
    match ids.len() {
        1 => Ok(ids.remove(0)),
        0 => Err(format!("no herdr workspace labeled {label}")),
        n => Err(format!("{n} herdr workspaces labeled {label}")),
    }
}

fn workspace_ids_for_label(text: &str, label: &str) -> Result<Vec<String>, String> {
    let value = parse_json(text)?;
    let mut ids = Vec::new();
    collect_workspace_ids(&value, label, &mut ids);
    Ok(ids)
}

fn collect_workspace_ids(value: &Value, label: &str, ids: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            let same_label = map.get("label").and_then(Value::as_str) == Some(label);
            // A tab or pane object also carries workspace_id. Those are not workspaces.
            if same_label && !map.contains_key("tab_id") && !map.contains_key("pane_id") {
                if let Some(id) = map.get("workspace_id").and_then(Value::as_str) {
                    if !ids.iter().any(|seen| seen == id) {
                        ids.push(id.to_string());
                    }
                }
            }
            for child in map.values() {
                collect_workspace_ids(child, label, ids);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_workspace_ids(child, label, ids);
            }
        }
        _ => {}
    }
}

fn ensure_tabs(herdr: &Path, cwd: &Path, workspace_id: &str) -> Result<Vec<CreatedTab>, String> {
    let cwd_s = cwd.display().to_string();
    let listed = herdr_ok(herdr, &["tab", "list", "--workspace", workspace_id])?;
    let have = tab_labels(&listed);
    let mut created = Vec::new();
    for role in ROLES {
        if have.iter().any(|label| label == role) {
            continue;
        }
        let body = herdr_ok(
            herdr,
            &[
                "tab",
                "create",
                "--workspace",
                workspace_id,
                "--cwd",
                &cwd_s,
                "--label",
                role,
                "--no-focus",
            ],
        )
        .map_err(|_| tab_create_failed(role))?;
        let Some(tab_id) = json_string_at(&body, &["result", "tab", "tab_id"]) else {
            return Err(tab_create_failed(role));
        };
        let pane_id = json_string_at(&body, &["result", "root_pane", "pane_id"]);
        created.push(CreatedTab {
            role,
            tab_id,
            pane_id,
        });
    }
    Ok(created)
}

fn resolve_created_panes(
    herdr: &Path,
    workspace_id: &str,
    created: &mut [CreatedTab],
) -> Result<(), String> {
    // root_pane.pane_id wins. Do not list panes when every new tab already has one.
    if created.iter().all(|tab| tab.pane_id.is_some()) {
        return Ok(());
    }
    let listed = herdr_ok(herdr, &["pane", "list", "--workspace", workspace_id])?;
    let value = parse_json(&listed)?;
    let mut pairs = Vec::new();
    collect_pairs(&value, "tab_id", "pane_id", &mut pairs);
    for tab in created.iter_mut() {
        if tab.pane_id.is_some() {
            continue;
        }
        let hits: Vec<&str> = pairs
            .iter()
            .filter(|(id, _)| id == &tab.tab_id)
            .map(|(_, pane)| pane.as_str())
            .collect();
        if hits.len() != 1 {
            return Err(no_single_pane(tab.role));
        }
        tab.pane_id = Some(hits[0].to_string());
    }
    Ok(())
}

fn json_string_at(text: &str, path: &[&str]) -> Option<String> {
    let mut cur = parse_json(text).ok()?;
    for key in path {
        cur = cur.get(*key)?.clone();
    }
    cur.as_str().map(str::to_string)
}

fn tab_create_failed(role: &str) -> String {
    format!(
        "tab create {role} failed; tabs created earlier in this call were not started and will not be started until those tabs are closed in Herdr"
    )
}

fn no_single_pane(role: &str) -> String {
    format!(
        "tab {role} has no single pane; tabs created earlier in this call were not started and will not be started until those tabs are closed in Herdr"
    )
}

fn go_pid(herdr: &Path, pane: &str) -> Option<u32> {
    // Live herdr takes --pane. A positional id is "unknown option" and the reaper never runs.
    // Prefer the process group: kill -TERM -N signals a group, and a nested pid can be the pane shell.
    let text = herdr_ok(herdr, &["pane", "process-info", "--pane", pane]).ok()?;
    let raw =
        first_key(&text, "foreground_process_group_id").or_else(|| first_key(&text, "pid"))?;
    let pid: u32 = raw.parse().ok()?;
    if pid < 2 {
        None
    } else {
        Some(pid)
    }
}

fn herdr_ok(bin: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("spawn herdr {args:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "herdr {args:?} exited {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn parse_json(text: &str) -> Result<Value, String> {
    let start = text
        .find('{')
        .ok_or_else(|| format!("herdr stdout is not JSON: {text}"))?;
    serde_json::from_str(&text[start..]).map_err(|e| format!("herdr json: {e}: {text}"))
}

fn first_key(text: &str, key: &str) -> Option<String> {
    let v = parse_json(text).ok()?;
    let mut found = Vec::new();
    collect_key(&v, key, &mut found);
    found.into_iter().next()
}

fn collect_key(v: &Value, key: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                if k == key {
                    if let Some(s) = child.as_str() {
                        out.push(s.to_string());
                    } else if let Some(n) = child.as_u64() {
                        out.push(n.to_string());
                    }
                }
                collect_key(child, key, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_key(child, key, out);
            }
        }
        _ => {}
    }
}

fn collect_pairs(v: &Value, id_key: &str, other_key: &str, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(map) => {
            let id = map.get(id_key).and_then(|x| x.as_str());
            let other = map.get(other_key).and_then(|x| x.as_str());
            if let (Some(id), Some(other)) = (id, other) {
                out.push((id.to_string(), other.to_string()));
            }
            for child in map.values() {
                collect_pairs(child, id_key, other_key, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_pairs(child, id_key, other_key, out);
            }
        }
        _ => {}
    }
}

fn tab_labels(text: &str) -> Vec<String> {
    let Ok(v) = parse_json(text) else {
        return Vec::new();
    };
    let mut pairs = Vec::new();
    collect_pairs(&v, "tab_id", "label", &mut pairs);
    pairs.into_iter().map(|(_, label)| label).collect()
}

// Room and the web camera share one check: non-empty IDEA.md or a READY row.
pub use crucible_contract::intake_ready;

/// SIGTERM the process group of `pid` (the go process herdr started). Refuses pid < 2.
pub fn kill_process_group(pid: u32) -> Result<(), String> {
    if pid < 2 {
        return Err(format!("reap: refusing pid {pid}"));
    }
    let st = Command::new("kill")
        .args(["-TERM", &format!("-{pid}")])
        .status()
        .map_err(|e| format!("reap: kill: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("reap: kill -TERM -{pid} exited {st}"))
    }
}

pub fn http_get(addr: &str, path: &str) -> Result<String, String> {
    if !path.starts_with('/') || path.contains("..") {
        return Err(format!("camera: refusing path {path}"));
    }
    let sock: std::net::SocketAddr = addr.parse().map_err(|e| format!("addr {addr:?}: {e}"))?;
    let mut last = None;
    for _ in 0..50 {
        match TcpStream::connect_timeout(&sock, Duration::from_millis(100)) {
            Ok(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                write!(
                    s,
                    "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
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
                    return Err(format!("GET {path} empty body"));
                }
                return Ok(body.to_string());
            }
            Err(e) => {
                last = Some(e);
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    Err(format!("GET {path} {addr}: {last:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;

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

    struct FakeSpec {
        pid: u32,
        workspace_list: Option<String>,
        fail_tab: &'static str,
        root: &'static str,
        panes: &'static str,
    }

    impl Default for FakeSpec {
        fn default() -> Self {
            Self {
                pid: 0,
                workspace_list: None,
                fail_tab: "__no_fail__",
                root: "label",
                panes: ONE_PANE_LIST,
            }
        }
    }

    const ONE_PANE_LIST: &str = r#"{"result":{"panes":[{"pane_id":"pane-chat","tab_id":"tab-chat"},{"pane_id":"pane-orchestrator","tab_id":"tab-orchestrator"},{"pane_id":"pane-watcher","tab_id":"tab-watcher"},{"pane_id":"pane-reaper","tab_id":"tab-reaper"},{"pane_id":"pane-dashboard","tab_id":"tab-dashboard"}]}}"#;
    const TWO_PANE_LIST: &str = r#"{"result":{"panes":[{"pane_id":"pane-chat","tab_id":"tab-chat"},{"pane_id":"pane-chat-b","tab_id":"tab-chat"},{"pane_id":"pane-orchestrator","tab_id":"tab-orchestrator"},{"pane_id":"pane-orchestrator-b","tab_id":"tab-orchestrator"},{"pane_id":"pane-watcher","tab_id":"tab-watcher"},{"pane_id":"pane-watcher-b","tab_id":"tab-watcher"},{"pane_id":"pane-reaper","tab_id":"tab-reaper"},{"pane_id":"pane-reaper-b","tab_id":"tab-reaper"},{"pane_id":"pane-dashboard","tab_id":"tab-dashboard"},{"pane_id":"pane-dashboard-b","tab_id":"tab-dashboard"}]}}"#;
    const DEFAULT_WORKSPACES: &str = r#"{"result":{"workspaces":[{"workspace_id":"ws1","label":"crucible","cwd":"/herdr-keeps-its-cwd"}]}}"#;

    fn plant_layout(root: &Path) {
        let dir = root.join(".crucible/herdr");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("workspace"), "crucible\n").unwrap();
        let mut roles = ROLES.join("\n");
        roles.push('\n');
        fs::write(dir.join("roles"), roles).unwrap();
    }

    fn seed_tabs(tmp: &Tmp, labels: &[&str]) {
        let body = labels
            .iter()
            .map(|label| format!("{label}\n"))
            .collect::<String>();
        fs::write(tmp.root.join("tabs.jsonl"), body).unwrap();
    }

    fn fake_herdr(tmp: &Tmp) -> PathBuf {
        write_fake(tmp, &FakeSpec::default())
    }

    fn fake_herdr_pid(tmp: &Tmp, pid: u32) -> PathBuf {
        write_fake(
            tmp,
            &FakeSpec {
                pid,
                ..FakeSpec::default()
            },
        )
    }

    fn write_fake(tmp: &Tmp, spec: &FakeSpec) -> PathBuf {
        let bin = tmp.root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let state = tmp.root.join("tabs.jsonl");
        let log = tmp.root.join("herdr.log");
        let list = spec
            .workspace_list
            .clone()
            .unwrap_or_else(|| DEFAULT_WORKSPACES.to_string());
        let script = FAKE_HERDR
            .replace("@@LOG@@", &log.display().to_string())
            .replace("@@STATE@@", &state.display().to_string())
            .replace("@@LIST@@", &list)
            .replace("@@FAIL@@", spec.fail_tab)
            .replace("@@ROOT@@", spec.root)
            .replace("@@PANES@@", spec.panes)
            .replace("@@PID@@", &spec.pid.to_string());
        let path = bin.join("herdr");
        write_exec(&path, &script);
        path
    }

    const FAKE_HERDR: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "@@LOG@@"
state="@@STATE@@"
cmd="$1 $2"
if [ "$cmd" = "workspace list" ]; then
  printf '%s\n' '@@LIST@@'
elif [ "$cmd" = "workspace create" ]; then
  printf '%s\n' '{"result":{"workspace":{"workspace_id":"ws1","label":"crucible","cwd":"'"$4"'"}}}'
elif [ "$cmd" = "tab list" ]; then
  printf '%s' '{"result":{"tabs":['
  sep=""
  if [ -f "$state" ]; then
    while IFS= read -r label; do
      [ -n "$label" ] || continue
      printf '%s%s' "$sep" '{"label":"'"$label"'","tab_id":"tab-'"$label"'"}'
      sep=","
    done < "$state"
  fi
  printf '%s\n' ']}}'
elif [ "$cmd" = "tab create" ]; then
  label=""
  prev=""
  for a in "$@"; do
    if [ "$prev" = "--label" ]; then label=$a; fi
    prev=$a
  done
  if [ "$label" = "@@FAIL@@" ]; then
    printf '%s\n' "tab create failed" >&2
    exit 1
  fi
  printf '%s\n' "$label" >> "$state"
  if [ "@@ROOT@@" = "absent" ]; then
    printf '%s\n' '{"result":{"tab":{"label":"'"$label"'","tab_id":"tab-'"$label"'","workspace_id":"ws1"}}}'
  elif [ "@@ROOT@@" = "fixed" ]; then
    printf '%s\n' '{"result":{"tab":{"label":"'"$label"'","tab_id":"tab-'"$label"'","workspace_id":"ws1"},"root_pane":{"pane_id":"pane-from-create"}}}'
  else
    printf '%s\n' '{"result":{"tab":{"label":"'"$label"'","tab_id":"tab-'"$label"'","workspace_id":"ws1"},"root_pane":{"pane_id":"pane-'"$label"'"}}}'
  fi
elif [ "$cmd" = "pane list" ]; then
  printf '%s\n' '@@PANES@@'
elif [ "$cmd" = "pane process-info" ]; then
  printf '%s\n' '{"result":{"pid":@@PID@@}}'
else
  printf '%s\n' '{"result":{"ok":true}}'
fi
exit 0
"#;

    fn exe_marker(tmp: &Tmp) -> PathBuf {
        let path = tmp.root.join("crucible");
        write_exec(&path, "#!/bin/sh\nexit 0\n");
        path
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
        assert!(resolve_herdr(empty.as_os_str()).is_none());
    }

    #[test]
    fn path_has_herdr_true_for_executable_on_path() {
        let tmp = Tmp::new();
        let bin = tmp.root.join("bin");
        fs::create_dir(&bin).unwrap();
        write_exec(&bin.join("herdr"), "#!/bin/sh\nexit 0\n");
        assert!(resolve_herdr(bin.as_os_str()).is_some());
    }

    #[test]
    fn missing_herdr_does_not_spawn_exe() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let empty = tmp.root.join("empty");
        fs::create_dir(&empty).unwrap();
        let exe = spawn_marker(&tmp);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &tmp.root,
            empty.as_os_str(),
            None,
            "127.0.0.1:9",
            &mut out,
            &mut err,
        );
        assert_eq!(code, 2, "missing herdr is nonzero");
        let stderr = String::from_utf8(err).unwrap();
        assert!(
            stderr.contains("herdr not found"),
            "missing binary, not a layout miss: {stderr}"
        );
        assert!(
            !tmp.root.join("SPAWNED").exists(),
            "must not spawn current_exe serve when herdr is missing"
        );
        assert!(
            !tmp.root.join(".wm").exists(),
            "missing herdr must not write TRACE"
        );
        assert!(out.is_empty(), "missing herdr must not print a room report");
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
        assert!(!prod.contains("SO_REUSEPORT"));
        assert!(!prod.contains("reuseport"));
        assert!(!prod.contains("env_clear"));
        assert!(!prod.contains("config.toml"));
        assert!(!prod.contains("\"server\""));
        assert!(!prod.contains("127.0.0.1:0"));
        assert!(prod.contains("127.0.0.1:1734"));
        assert!(prod.contains("serve reused"));
        assert!(!ROLES.contains(&"terminal"));
        assert_eq!(ROLES.len(), 5);
        for banned in [
            "workspace create",
            "workspace close",
            "workspace rename",
            "worktree",
            "--session",
            "session stop",
            "session delete",
            "HERDR_CONFIG_PATH",
            "XDG_CONFIG_HOME",
            "HERDR_SESSION",
            ".env(",
        ] {
            assert!(!prod.contains(banned), "{banned} in room source");
        }
    }

    #[test]
    fn arrange_creates_five_tabs_and_does_not_duplicate() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = fake_herdr(&tmp);
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert_eq!(report.go, GoLine::Waiting);
        assert!(!report.started_go);
        assert_eq!(report.workspace_id, "ws1");
        arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        let creates = log.matches("tab create").count();
        assert_eq!(
            creates, 5,
            "second attach must not create tabs again:\n{log}"
        );
        assert_eq!(
            log.matches("pane run").count(),
            2,
            "second attach must not pane run:\n{log}"
        );
        assert!(
            log.contains("camera"),
            "watcher must GET via camera:\n{log}"
        );
        assert!(!log.contains(" go"), "no IDEA means no go:\n{log}");
        assert!(!log.contains("\tgo"), "no go argv:\n{log}");
        let watcher = log.lines().find(|l| l.contains("camera")).unwrap();
        assert!(
            !watcher.contains("pane-orchestrator"),
            "watcher must not target the go pane: {watcher}"
        );
        assert!(!log.contains("pane-chat"), "no pane run in chat:\n{log}");
        assert!(!log.split_whitespace().any(|w| w == "server"), "{log}");
        assert!(!log.contains("config.toml"), "{log}");
        assert!(!log.contains("terminal"), "{log}");
        assert!(!log.contains("workspace create"), "{log}");
        assert!(!log.contains("workspace close"), "{log}");
        assert!(!log.contains("workspace rename"), "{log}");
        assert!(!log.contains("--session"), "{log}");
        assert!(!log.contains("session"), "{log}");
        assert!(!log.contains("worktree"), "{log}");
    }

    #[test]
    fn idea_starts_go_only_in_orchestrator_argv() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = fake_herdr(&tmp);
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert!(report.started_go);
        assert_eq!(report.go, GoLine::Started);
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        let go_lines: Vec<_> = log.lines().filter(|l| l.contains(" go")).collect();
        assert_eq!(go_lines.len(), 1, "one go process:\n{log}");
        assert!(
            go_lines[0].contains("pane-orchestrator"),
            "go stays in orchestrator: {}",
            go_lines[0]
        );
        assert!(
            !tmp.root.join(".wm").exists(),
            "arrange must not write TRACE"
        );
    }

    #[test]
    fn ready_backlog_starts_go_without_idea() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(
            tmp.root.join("BACKLOG.tsv"),
            "id\tsize\trisk\tidea_path\tstatus\ns1\tS\tLOW\tIDEA.md\tREADY\n",
        )
        .unwrap();
        assert!(intake_ready(&tmp.root));
        let herdr = fake_herdr(&tmp);
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert!(report.started_go);
        assert_eq!(report.go, GoLine::Started);
    }

    #[test]
    fn intake_ready_is_nonempty_idea_or_ready_backlog_row() {
        let tmp = Tmp::new();
        assert!(!intake_ready(&tmp.root));
        fs::write(tmp.root.join("IDEA.md"), "  \n").unwrap();
        assert!(!intake_ready(&tmp.root), "whitespace IDEA is not intake");
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        assert!(intake_ready(&tmp.root));
        fs::remove_file(tmp.root.join("IDEA.md")).unwrap();
        fs::write(
            tmp.root.join("BACKLOG.tsv"),
            "id\tsize\trisk\tidea_path\tstatus\n# note\n\ns1\tS\tLOW\tIDEA.md\tDONE\n",
        )
        .unwrap();
        assert!(!intake_ready(&tmp.root));
        fs::write(
            tmp.root.join("BACKLOG.tsv"),
            "id\tsize\trisk\tidea_path\tstatus\ns1\tS\tLOW\tIDEA.md\tREADY\n",
        )
        .unwrap();
        assert!(intake_ready(&tmp.root));
    }

    #[test]
    fn reap_sends_sigterm_to_the_process_group() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut child = cmd.spawn().unwrap();
        let pid = child.id();
        kill_process_group(pid).unwrap();
        let st = child.wait().unwrap();
        assert!(!st.success() || cfg!(not(unix)), "sleep should die: {st}");
        assert!(kill_process_group(0).is_err());
        assert!(kill_process_group(1).is_err());
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
            let raw = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            // Manifest paths are not crates. This checkout lives under `.grok`.
            let mut tree = String::new();
            let mut depth = 0i32;
            for c in raw.chars() {
                match c {
                    '(' => depth += 1,
                    ')' if depth > 0 => depth -= 1,
                    _ if depth == 0 => tree.push(c),
                    _ => {}
                }
            }
            assert!(!tree.contains("herdr"), "herdr in {pkg} tree:\n{raw}");
            assert!(!tree.contains("grok"), "grok in {pkg} tree:\n{raw}");
            assert!(!tree.contains("engos"), "engos in {pkg} tree:\n{raw}");
        }
    }

    fn http_json(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    struct HealthSrv {
        addr: String,
        hits: Arc<AtomicUsize>,
        stop: Arc<AtomicBool>,
        join: Option<thread::JoinHandle<()>>,
    }

    impl HealthSrv {
        fn start(body: String) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap().to_string();
            let hits = Arc::new(AtomicUsize::new(0));
            let stop = Arc::new(AtomicBool::new(false));
            let hits2 = hits.clone();
            let stop2 = stop.clone();
            let join = thread::spawn(move || {
                listener.set_nonblocking(true).unwrap();
                while !stop2.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut sock, _)) => {
                            hits2.fetch_add(1, Ordering::Relaxed);
                            let _ = sock.set_read_timeout(Some(Duration::from_secs(1)));
                            let mut buf = [0u8; 2048];
                            let _ = sock.read(&mut buf);
                            let resp = http_json(&body);
                            let _ = sock.write_all(resp.as_bytes());
                            let _ = sock.flush();
                            let _ = sock.shutdown(std::net::Shutdown::Write);
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });
            Self {
                addr,
                hits,
                stop,
                join: Some(join),
            }
        }
    }

    impl Drop for HealthSrv {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }

    fn spawn_marker(tmp: &Tmp) -> PathBuf {
        let path = tmp.root.join("crucible");
        write_exec(
            &path,
            "#!/bin/sh\nprintf spawned > \"$(dirname \"$0\")/SPAWNED\"\nexit 0\n",
        );
        path
    }

    #[test]
    fn health_reuse_keeps_fixture_accepting() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let body = serde_json::json!({
            "ok": true,
            "version": product_version(),
            "bind": "127.0.0.1:9"
        })
        .to_string();
        let srv = HealthSrv::start(body);
        let herdr = fake_herdr(&tmp);
        let exe = spawn_marker(&tmp);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &tmp.root,
            herdr.parent().unwrap().as_os_str(),
            None,
            &srv.addr,
            &mut out,
            &mut err,
        );
        let stdout = String::from_utf8(out).unwrap();
        let stderr = String::from_utf8(err).unwrap();
        assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
        assert!(stdout.contains("go waiting"), "{stdout}");
        assert!(stdout.contains("serve reused"), "{stdout}");
        assert!(
            !stdout.lines().any(|l| l.starts_with("listening ")),
            "{stdout}"
        );
        assert!(!stdout.contains("serve pid "), "{stdout}");
        assert!(!tmp.root.join("SPAWNED").exists(), "reuse must not spawn");
        assert!(!tmp.root.join(".wm").exists(), "reuse must not write TRACE");
        let again = http_get(&srv.addr, "/health").expect("fixture still accepts");
        assert!(again.contains(product_version()), "{again}");
        assert!(
            srv.hits.load(Ordering::Relaxed) >= 2,
            "probe plus a later accept"
        );
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("camera"), "{log}");
        assert!(!log.contains("pane-chat"), "{log}");
        assert!(!log.split_whitespace().any(|w| w == "server"), "{log}");
    }

    #[test]
    fn stranger_port_does_not_listen_or_call_herdr() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let body = serde_json::json!({
            "ok": true,
            "version": "0.0.0",
            "bind": "127.0.0.1:9"
        })
        .to_string();
        let srv = HealthSrv::start(body);
        let herdr = fake_herdr(&tmp);
        let exe = spawn_marker(&tmp);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &tmp.root,
            herdr.parent().unwrap().as_os_str(),
            None,
            &srv.addr,
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1, "{}", String::from_utf8_lossy(&err));
        assert!(String::from_utf8_lossy(&err).contains("version mismatch"));
        assert!(!tmp.root.join("SPAWNED").exists());
        assert!(
            !tmp.root.join("herdr.log").exists(),
            "stranger must not call herdr"
        );
        assert!(!tmp.root.join(".wm").exists());
        let again = http_get(&srv.addr, "/health").expect("stranger fixture still accepts");
        assert!(again.contains("0.0.0"), "{again}");
        assert!(out.is_empty(), "stranger must not print serve reused");
    }

    #[test]
    fn probe_refuses_not_ok_even_when_version_matches() {
        let body = serde_json::json!({
            "ok": false,
            "version": product_version()
        })
        .to_string();
        let srv = HealthSrv::start(body);
        match probe_health(&srv.addr, product_version()) {
            HealthProbe::Other(msg) => assert!(msg.contains("version mismatch"), "{msg}"),
            other => panic!("expected other, got {other:?}"),
        }
        let again = http_get(&srv.addr, "/health").expect("fixture still accepts");
        assert!(
            again.contains("false") || again.contains("False") || again.contains("ok"),
            "{again}"
        );
    }

    #[test]
    fn probe_connection_refused_is_not_other() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        drop(listener);
        match probe_health(&addr, product_version()) {
            HealthProbe::Refused => {}
            other => panic!("expected refused, got {other:?}"),
        }
    }

    #[test]
    fn refused_spawns_once_and_skips_herdr_when_child_is_not_healthy() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let addr = format!("127.0.0.1:{port}");
        let args_path = tmp.root.join("args");
        let exe = tmp.root.join("crucible");
        write_exec(
            &exe,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" > {}\nprintf 'listening {}\\n'\nexec sleep 30\n",
                args_path.display(),
                addr
            ),
        );
        let herdr = fake_herdr(&tmp);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &tmp.root,
            herdr.parent().unwrap().as_os_str(),
            None,
            &addr,
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1, "{}", String::from_utf8_lossy(&err));
        let args = fs::read_to_string(&args_path).unwrap();
        assert_eq!(args.trim(), format!("serve --bind {addr}"));
        assert!(!tmp.root.join("herdr.log").exists());
        assert!(!String::from_utf8(out).unwrap().contains("serve reused"));
    }

    #[test]
    fn accept_without_body_exits_without_spawn_or_herdr() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let thr = thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            while !stop2.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut sock, _)) => {
                        let _ = sock.set_read_timeout(Some(Duration::from_secs(1)));
                        let mut buf = [0u8; 512];
                        let _ = sock.read(&mut buf);
                        while !stop2.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = fake_herdr(&tmp);
        let exe = spawn_marker(&tmp);
        let started = std::time::Instant::now();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &tmp.root,
            herdr.parent().unwrap().as_os_str(),
            None,
            &addr,
            &mut out,
            &mut err,
        );
        let elapsed = started.elapsed();
        stop.store(true, Ordering::Relaxed);
        let _ = thr.join();
        assert_eq!(code, 1, "{}", String::from_utf8_lossy(&err));
        assert!(
            elapsed < Duration::from_secs(5),
            "probe must not retry for long: {elapsed:?}"
        );
        assert!(!tmp.root.join("SPAWNED").exists());
        assert!(!tmp.root.join("herdr.log").exists());
    }

    #[test]
    fn missing_override_names_path_and_does_not_listen() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let empty = tmp.root.join("empty");
        fs::create_dir(&empty).unwrap();
        let override_path = tmp.root.join("missing-herdr");
        let exe = spawn_marker(&tmp);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &tmp.root,
            empty.as_os_str(),
            Some(override_path.as_os_str()),
            "127.0.0.1:9",
            &mut out,
            &mut err,
        );
        assert_eq!(code, 2);
        let stderr = String::from_utf8(err).unwrap();
        assert!(
            stderr.contains(&override_path.display().to_string()),
            "{stderr}"
        );
        assert!(!tmp.root.join("SPAWNED").exists());
        assert!(!tmp.root.join(".wm").exists());
        assert!(!tmp.root.join("herdr.log").exists());
    }

    #[test]
    fn path_miss_uses_executable_override_and_names_it() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let empty = tmp.root.join("empty");
        fs::create_dir(&empty).unwrap();
        let herdr = fake_herdr(&tmp);
        let body = serde_json::json!({
            "ok": true,
            "version": product_version()
        })
        .to_string();
        let srv = HealthSrv::start(body);
        let exe = spawn_marker(&tmp);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &tmp.root,
            empty.as_os_str(),
            Some(herdr.as_os_str()),
            &srv.addr,
            &mut out,
            &mut err,
        );
        let stdout = String::from_utf8(out).unwrap();
        let stderr = String::from_utf8(err).unwrap();
        assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
        assert!(stderr.contains(&herdr.display().to_string()), "{stderr}");
        assert!(stdout.contains("serve reused"), "{stdout}");
        assert!(!tmp.root.join("SPAWNED").exists());
        assert!(tmp.root.join("herdr.log").exists());
        let _ = http_get(&srv.addr, "/health").expect("fixture still accepts");
    }

    #[test]
    fn reaper_command_absent_when_pid_is_0() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = fake_herdr_pid(&tmp, 0);
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert!(report.started_go);
        assert_eq!(report.go, GoLine::Started);
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(
            !log.split_whitespace().any(|w| w == "reap"),
            "pid 0 must not reap:\n{log}"
        );
        assert!(!log.contains("pane-chat"), "no pane run in chat:\n{log}");
        let go = log
            .lines()
            .find(|l| l.split_whitespace().any(|w| w == "go"))
            .expect(&log);
        assert!(go.contains("pane-orchestrator"), "{go}");
    }

    #[test]
    fn reap_follows_go_only_when_pid_at_least_2() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = fake_herdr_pid(&tmp, 2);
        let exe = exe_marker(&tmp);
        arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        let lines: Vec<_> = log.lines().collect();
        let go_at = lines
            .iter()
            .position(|l| l.split_whitespace().any(|w| w == "go"))
            .expect(&log);
        let reap_at = lines
            .iter()
            .position(|l| l.split_whitespace().any(|w| w == "reap"))
            .expect(&log);
        assert!(reap_at > go_at, "{log}");
        let reap = lines[reap_at];
        assert!(reap.contains("pane-reaper"), "{reap}");
        assert!(
            reap.split_whitespace()
                .collect::<Vec<_>>()
                .windows(2)
                .any(|w| w == ["--pid", "2"]),
            "{reap}"
        );
        assert!(!log.contains("pane-chat"), "{log}");
    }

    fn layout_drive(tmp: &Tmp) -> (i32, String, String) {
        let herdr = fake_herdr(tmp);
        let exe = spawn_marker(tmp);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &tmp.root,
            herdr.parent().unwrap().as_os_str(),
            None,
            "127.0.0.1:9",
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn assert_layout_refused(tmp: &Tmp) {
        let (code, out, err) = layout_drive(tmp);
        assert_eq!(code, 2, "stdout={out} stderr={err}");
        assert!(!err.contains("herdr not found"), "{err}");
        assert!(!tmp.root.join("herdr.log").exists(), "{err}");
        assert!(!tmp.root.join("SPAWNED").exists(), "{err}");
        assert!(!tmp.root.join(".wm").exists(), "{err}");
        assert!(out.is_empty(), "{out}");
    }

    fn write_workspace(root: &Path, body: &str) {
        plant_layout(root);
        fs::write(root.join(".crucible/herdr/workspace"), body).unwrap();
    }

    fn write_roles(root: &Path, body: &str) {
        plant_layout(root);
        fs::write(root.join(".crucible/herdr/roles"), body).unwrap();
    }

    #[test]
    fn templates_match_roles_and_default_label() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates/herdr");
        for name in ["workspace", "roles"] {
            let path = root.join(name);
            let meta = fs::symlink_metadata(&path).unwrap();
            assert!(meta.is_file(), "{}", path.display());
            assert!(!meta.file_type().is_symlink(), "{}", path.display());
        }
        let mut roles = ROLES.join("\n");
        roles.push('\n');
        assert_eq!(fs::read(root.join("roles")).unwrap(), roles.as_bytes());
        assert_eq!(fs::read(root.join("workspace")).unwrap(), b"crucible\n");
    }

    #[test]
    fn layout_missing_exits_before_herdr() {
        let tmp = Tmp::new();
        assert_layout_refused(&tmp);
    }

    #[test]
    fn empty_or_multiline_layout_does_not_call_herdr() {
        for body in [
            "",
            "crucible\n\n",
            "crucible\nother\n",
            "crucible extra\n",
            "crucible\r\n",
        ] {
            let tmp = Tmp::new();
            write_workspace(&tmp.root, body);
            assert_layout_refused(&tmp);
        }
        for body in [
            "chat\norchestrator\nwatcher\nreaper\n",
            "chat\norchestrator\nwatcher\nreaper\ndashboard\nterminal\n",
            "chat\nwatcher\norchestrator\nreaper\ndashboard\n",
        ] {
            let tmp = Tmp::new();
            write_roles(&tmp.root, body);
            assert_layout_refused(&tmp);
        }
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let dir = tmp.root.join(".crucible/herdr");
        fs::remove_file(dir.join("workspace")).unwrap();
        std::os::unix::fs::symlink(dir.join("roles"), dir.join("workspace")).unwrap();
        assert_layout_refused(&tmp);

        let trimmed = Tmp::new();
        write_workspace(&trimmed.root, "  crucible  \n");
        let layout = load_room_layout(&trimmed.root).unwrap();
        assert_eq!(layout.label, "crucible");
        assert!(!trimmed.root.join("herdr.log").exists());

        let nbsp = Tmp::new();
        write_workspace(&nbsp.root, "crucible\u{00a0}\n");
        let layout = load_room_layout(&nbsp.root).unwrap();
        assert!(layout.label.contains('\u{00a0}'), "{:?}", layout.label);
        assert_ne!(layout.label, "crucible");
        assert!(!nbsp.root.join("herdr.log").exists());
    }

    fn arrange_listed(tmp: &Tmp, list: &str) -> Result<RoomReport, String> {
        plant_layout(&tmp.root);
        let herdr = write_fake(
            tmp,
            &FakeSpec {
                workspace_list: Some(list.to_string()),
                ..FakeSpec::default()
            },
        );
        let exe = exe_marker(tmp);
        arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9")
    }

    fn assert_list_did_not_mutate(tmp: &Tmp) {
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("workspace list"), "{log}");
        assert!(!log.contains("workspace create"), "{log}");
        assert!(!log.contains("workspace close"), "{log}");
        assert!(!log.contains("workspace rename"), "{log}");
    }

    #[test]
    fn attach_ignores_workspace_cwd() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = fake_herdr(&tmp);
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert_eq!(report.workspace_id, "ws1");
        assert_ne!(tmp.root.display().to_string(), "/herdr-keeps-its-cwd");
        assert_list_did_not_mutate(&tmp);
    }

    #[test]
    fn zero_workspaces_do_not_create() {
        let tmp = Tmp::new();
        let err = arrange_listed(&tmp, r#"{"result":{"workspaces":[]}}"#).unwrap_err();
        assert!(err.contains("no herdr workspace labeled"), "{err}");
        assert_list_did_not_mutate(&tmp);
    }

    #[test]
    fn two_workspaces_do_not_create() {
        let tmp = Tmp::new();
        let err = arrange_listed(
            &tmp,
            r#"{"result":{"workspaces":[{"workspace_id":"ws1","label":"crucible"},{"workspace_id":"ws2","label":"crucible"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("2 herdr workspaces labeled"), "{err}");
        assert_list_did_not_mutate(&tmp);
    }

    #[test]
    fn same_id_nested_twice_is_one_hit() {
        let tmp = Tmp::new();
        let report = arrange_listed(
            &tmp,
            r#"{"result":{"workspace":{"workspace_id":"ws1","label":"crucible","child":{"workspace_id":"ws1","label":"crucible"}}}}"#,
        )
        .unwrap();
        assert_eq!(report.workspace_id, "ws1");
    }

    #[test]
    fn nested_tab_is_not_a_second_workspace() {
        let tmp = Tmp::new();
        let report = arrange_listed(
            &tmp,
            r#"{"result":{"workspaces":[{"workspace_id":"ws1","label":"crucible","active_tab_id":"tab-old","tabs":[{"workspace_id":"ws-tab","label":"crucible","tab_id":"tab-nested"}]}]}}"#,
        )
        .unwrap();
        assert_eq!(report.workspace_id, "ws1");
    }

    #[test]
    fn existing_tab_is_not_pane_run() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = fake_herdr(&tmp);
        seed_tabs(&tmp, &["orchestrator"]);
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert_eq!(report.go, GoLine::AlreadyOpen);
        assert!(!report.started_go);
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(!log.contains("pane-orchestrator"), "{log}");
        assert!(log.contains("pane-watcher"), "{log}");
        assert!(log.contains("pane-dashboard"), "{log}");

        let body = serde_json::json!({
            "ok": true,
            "version": product_version()
        })
        .to_string();
        let srv = HealthSrv::start(body);
        let live = Tmp::new();
        plant_layout(&live.root);
        fs::write(live.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = fake_herdr(&live);
        seed_tabs(&live, &["orchestrator"]);
        let exe = spawn_marker(&live);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &live.root,
            herdr.parent().unwrap().as_os_str(),
            None,
            &srv.addr,
            &mut out,
            &mut err,
        );
        let stdout = String::from_utf8(out).unwrap();
        let stderr = String::from_utf8(err).unwrap();
        assert_eq!(code, 0, "stdout={stdout} stderr={stderr}");
        assert!(
            stdout.contains("go not started (orchestrator tab already open)"),
            "{stdout}"
        );
        assert!(!stdout.contains("go orchestrator"), "{stdout}");
        assert!(!stdout.contains("go waiting"), "{stdout}");
    }

    #[test]
    fn new_reaper_old_orchestrator_does_not_go_or_reap() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = fake_herdr(&tmp);
        seed_tabs(&tmp, &["orchestrator"]);
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert_eq!(report.go, GoLine::AlreadyOpen);
        assert!(!report.started_go);
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(!log.split_whitespace().any(|w| w == "go"), "{log}");
        assert!(!log.split_whitespace().any(|w| w == "reap"), "{log}");
    }

    #[test]
    fn new_orchestrator_and_old_reaper_does_not_reap() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = fake_herdr_pid(&tmp, 2);
        seed_tabs(&tmp, &["reaper"]);
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert_eq!(report.go, GoLine::Started);
        assert!(report.started_go);
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        let go = log
            .lines()
            .find(|l| l.split_whitespace().any(|w| w == "go"))
            .expect(&log);
        assert!(go.contains("pane-orchestrator"), "{go}");
        assert!(!log.split_whitespace().any(|w| w == "reap"), "{log}");
    }

    #[test]
    fn tab_create_failure_pane_runs_nothing() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = write_fake(
            &tmp,
            &FakeSpec {
                fail_tab: "watcher",
                ..FakeSpec::default()
            },
        );
        let body = serde_json::json!({
            "ok": true,
            "version": product_version()
        })
        .to_string();
        let srv = HealthSrv::start(body);
        let exe = spawn_marker(&tmp);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = drive(
            &exe,
            &tmp.root,
            herdr.parent().unwrap().as_os_str(),
            None,
            &srv.addr,
            &mut out,
            &mut err,
        );
        let stderr = String::from_utf8(err).unwrap();
        assert_eq!(code, 1, "{stderr}");
        assert!(
            stderr.contains("tabs created earlier in this call were not started"),
            "{stderr}"
        );
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(!log.contains("pane run"), "{log}");
        assert!(!log.contains("tab close"), "{log}");
        let exe = exe_marker(&tmp);
        let err = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap_err();
        assert!(
            err.contains("tabs created earlier in this call were not started"),
            "{err}"
        );
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(!log.contains("pane run"), "{log}");
        assert!(!log.contains("pane-chat"), "{log}");
        assert!(!log.contains("pane-orchestrator"), "{log}");
        assert!(!log.contains("tab close"), "{log}");
    }

    #[test]
    fn root_pane_beats_two_pane_rows() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = write_fake(
            &tmp,
            &FakeSpec {
                root: "fixed",
                panes: TWO_PANE_LIST,
                ..FakeSpec::default()
            },
        );
        let exe = exe_marker(&tmp);
        arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("pane-from-create"), "{log}");
        assert!(!log.contains("pane list"), "{log}");
        assert!(log.contains("pane run"), "{log}");
    }

    #[test]
    fn missing_root_pane_uses_the_one_pane_row() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = write_fake(
            &tmp,
            &FakeSpec {
                root: "absent",
                panes: ONE_PANE_LIST,
                ..FakeSpec::default()
            },
        );
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert_eq!(report.go, GoLine::Started);
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert_eq!(log.matches("pane list").count(), 1, "{log}");
        let go = log
            .lines()
            .find(|l| l.split_whitespace().any(|w| w == "go"))
            .expect(&log);
        assert!(go.contains("pane-orchestrator"), "{go}");
    }

    #[test]
    fn two_panes_and_no_root_pane_do_not_run() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = write_fake(
            &tmp,
            &FakeSpec {
                root: "absent",
                panes: TWO_PANE_LIST,
                ..FakeSpec::default()
            },
        );
        let exe = exe_marker(&tmp);
        let err = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap_err();
        assert!(err.contains("no single pane"), "{err}");
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("pane list"), "{log}");
        assert!(!log.contains("pane run"), "{log}");
    }

    #[test]
    #[ignore = "live herdr list; set CRUCIBLE_ROOM_LIVE=1"]
    fn live_herdr_list_commands() {
        if std::env::var("CRUCIBLE_ROOM_LIVE").ok().as_deref() != Some("1") {
            return;
        }
        let bin = std::env::var("CRUCIBLE_HERDR").unwrap_or_else(|_| "herdr".to_string());
        for args in [
            &["workspace", "list"][..],
            &["tab", "list"][..],
            &["pane", "list"][..],
        ] {
            let out = Command::new(&bin)
                .args(args)
                .output()
                .unwrap_or_else(|e| panic!("spawn {bin} {args:?}: {e}"));
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let text = String::from_utf8_lossy(&out.stdout);
            parse_json(&text).unwrap_or_else(|e| panic!("{args:?}: {e}: {text}"));
        }
    }
}
