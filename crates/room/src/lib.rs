//! `crucible room` joins one existing Herdr workspace. It probes loopback
//! `GET /health` before listen, then ensures `terminal`, `chat`,
//! `orchestrator`, and `dashboard` when that label is absent. It does not
//! start `go`, `reap`, or `camera`. Do not vendor an init tree.
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

pub const ROLES: [&str; 4] = ["terminal", "chat", "orchestrator", "dashboard"];

const LEGACY_ROLES: [&str; 5] = ["chat", "orchestrator", "watcher", "reaper", "dashboard"];

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
    let report = match arrange(&herdr, cwd) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "room: {e}");
            return 1;
        }
    };
    let _ = writeln!(
        out,
        "standing roles: terminal, chat, orchestrator, dashboard"
    );
    if let Some((pid, guard)) = spawned {
        let _ = writeln!(out, "listening {addr}");
        let _ = writeln!(out, "serve pid {pid}");
        // Serve stays up. Forgetting the guard skips the kill-on-drop.
        std::mem::forget(guard);
    } else {
        let _ = writeln!(out, "serve reused");
    }
    let _ = writeln!(out, "GET /health {addr}");
    let _ = writeln!(out, "{body}");
    let _ = writeln!(out, "workspace {}", report.workspace_id);
    let _ = writeln!(out, "tabs {}", report.labels.join(" "));
    if guided_checkout(cwd) {
        if let Err(e) = start_floor(&herdr, exe, &report.workspace_id, cwd) {
            let _ = writeln!(err, "room: {e}");
            return 1;
        }
        let _ = writeln!(out, "orchestrator started");
    } else {
        let _ = writeln!(out, "go not started");
    }
    let _ = out.flush();
    0
}

fn guided_checkout(cwd: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(cwd.join(".crucible")) else {
        return false;
    };
    for ent in rd.flatten() {
        let Ok(text) = std::fs::read_to_string(ent.path().join("PROGRAM")) else {
            continue;
        };
        if text.lines().any(|line| line.trim() == "cycle: guided") {
            return true;
        }
    }
    false
}

fn start_floor(herdr: &Path, exe: &Path, workspace_id: &str, cwd: &Path) -> Result<(), String> {
    let tabs = herdr_ok(herdr, &["tab", "list", "--workspace", workspace_id])?;
    let listed = herdr_ok(herdr, &["pane", "list", "--workspace", workspace_id])?;
    if let Some(pane) = pane_for_role(&tabs, &listed, "orchestrator") {
        pane_verb(herdr, &pane, exe, cwd, "orchestrate", "run")?;
    }
    if let Some(pane) = pane_for_role(&tabs, &listed, "dashboard") {
        pane_verb(herdr, &pane, exe, cwd, "message", "queue")?;
    }
    Ok(())
}

/// `pane run` types one line into the existing shell. `exec` replaces that
/// shell, and Herdr then removes the tab. `cd` and `CRUCIBLE_ROOT` select
/// this checkout, not the directory that contains the binary.
fn pane_verb(
    herdr: &Path,
    pane: &str,
    exe: &Path,
    cwd: &Path,
    verb: &str,
    arg: &str,
) -> Result<(), String> {
    let root = shell_quote(&cwd.display().to_string());
    let bin = shell_quote(&exe.display().to_string());
    let line = format!("cd {root} && CRUCIBLE_ROOT={root} {bin} {verb} {arg}");
    herdr_ok(herdr, &["pane", "run", pane, &line])?;
    Ok(())
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// A Herdr tab id is not the role name. Match the tab label, then the pane
/// whose `tab_id` is that tab.
fn pane_for_role(tabs: &str, panes: &str, role: &str) -> Option<String> {
    let tabs = parse_json(tabs).ok()?;
    let mut tab_pairs = Vec::new();
    collect_pairs(&tabs, "tab_id", "label", &mut tab_pairs);
    let tab_id = tab_pairs
        .into_iter()
        .find(|(_, label)| label == role)
        .map(|(id, _)| id)?;
    let panes = parse_json(panes).ok()?;
    let mut pane_pairs = Vec::new();
    collect_pairs(&panes, "pane_id", "tab_id", &mut pane_pairs);
    pane_pairs
        .into_iter()
        .find(|(_, id)| id == &tab_id)
        .map(|(pane, _)| pane)
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

#[derive(Debug)]
struct RoomReport {
    workspace_id: String,
    labels: Vec<String>,
}

struct RoomLayout {
    label: String,
}

struct WorkspaceObj {
    id: String,
    label: String,
    cwd: Option<String>,
}

fn arrange(herdr: &Path, cwd: &Path) -> Result<RoomReport, String> {
    let layout = load_room_layout(cwd)?;
    let workspace_id = attach_workspace(herdr, &layout.label, cwd)?;
    ensure_tabs(herdr, cwd, &workspace_id)?;
    let labels: Vec<String> = ROLES.iter().map(|s| (*s).to_string()).collect();
    Ok(RoomReport {
        workspace_id,
        labels,
    })
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
    if lines == ROLES || lines == LEGACY_ROLES {
        Ok(())
    } else {
        Err(format!(
            "{} is not the four standing labels in order (terminal, chat, orchestrator, dashboard) or the legacy five names in order (chat, orchestrator, watcher, reaper, dashboard)",
            path.display()
        ))
    }
}

fn attach_workspace(herdr: &Path, label: &str, cwd: &Path) -> Result<String, String> {
    let listed = herdr_ok(herdr, &["workspace", "list"])?;
    select_workspace(&listed, label, cwd)
}

fn select_workspace(text: &str, label: &str, cwd: &Path) -> Result<String, String> {
    let value = parse_json(text)?;
    let mut found = Vec::new();
    collect_workspaces(&value, &mut found);
    let label_hits: Vec<&WorkspaceObj> = found.iter().filter(|hit| hit.label == label).collect();
    if found.iter().all(|hit| hit.cwd.is_none()) {
        return match label_hits.len() {
            1 => Ok(label_hits[0].id.clone()),
            0 => Err(format!(
                "no herdr workspace labeled {label}; create or choose the room with herdr-init"
            )),
            n => Err(format!(
                "{n} herdr workspaces labeled {label}; choose the room with herdr-init"
            )),
        };
    }
    let checkout = cwd.display().to_string();
    let cwd_hits: Vec<&WorkspaceObj> = found
        .iter()
        .filter(|hit| hit.cwd.as_deref() == Some(checkout.as_str()))
        .collect();
    if label_hits.len() == 1 && cwd_hits.len() == 1 && label_hits[0].id == cwd_hits[0].id {
        Ok(label_hits[0].id.clone())
    } else {
        Err(format!(
            "herdr workspace for {label} is not the workspace whose cwd is this checkout; choose the room with herdr-init"
        ))
    }
}

fn collect_workspaces(value: &Value, out: &mut Vec<WorkspaceObj>) {
    match value {
        Value::Object(map) => {
            // A tab or pane object also carries workspace_id. Those are not workspaces.
            if !map.contains_key("tab_id") && !map.contains_key("pane_id") {
                if let (Some(id), Some(label)) = (
                    map.get("workspace_id").and_then(Value::as_str),
                    map.get("label").and_then(Value::as_str),
                ) {
                    if !out.iter().any(|seen| seen.id == id) {
                        let cwd = map.get("cwd").and_then(Value::as_str).map(str::to_string);
                        out.push(WorkspaceObj {
                            id: id.to_string(),
                            label: label.to_string(),
                            cwd,
                        });
                    }
                }
            }
            for child in map.values() {
                collect_workspaces(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_workspaces(child, out);
            }
        }
        _ => {}
    }
}

fn ensure_tabs(herdr: &Path, cwd: &Path, workspace_id: &str) -> Result<(), String> {
    let cwd_s = cwd.display().to_string();
    let listed = herdr_ok(herdr, &["tab", "list", "--workspace", workspace_id])?;
    let have = tab_labels(&listed);
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
        if json_string_at(&body, &["result", "tab", "tab_id"]).is_none() {
            return Err(tab_create_failed(role));
        }
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

/// Type `keys` into one pane of the workspace this checkout already joined.
/// Does not create a workspace, read Herdr config, or pass `--machine` or `--remote`.
pub fn type_into_pane(
    path: &OsStr,
    herdr_override: Option<&OsStr>,
    cwd: &Path,
    pane: &str,
    keys: &[&str],
) -> Result<String, String> {
    pane_token(pane)?;
    if keys.is_empty() {
        return Err("usage: crucible keys PANE KEY...".to_string());
    }
    for key in keys {
        key_token(key)?;
    }
    let herdr = keys_herdr(path, herdr_override)?;
    let label = workspace_label(&cwd.join(".crucible/herdr/workspace"))?;
    let listed = herdr_ok(&herdr, &["workspace", "list"])?;
    let workspace_id = select_workspace(&listed, &label, cwd)?;
    let panes = herdr_ok(&herdr, &["pane", "list", "--workspace", &workspace_id])?;
    let ids = pane_ids(&panes)?;
    if !ids.iter().any(|id| id == pane) {
        return Err(format!(
            "pane {pane} is not in the workspace this checkout joined"
        ));
    }
    let mut args = vec!["pane", "send-keys", pane];
    args.extend(keys);
    herdr_ok(&herdr, &args)?;
    Ok(format!("typed {pane} {}\n", keys.join(" ")))
}

fn keys_herdr(path: &OsStr, override_bin: Option<&OsStr>) -> Result<PathBuf, String> {
    if let Some(found) = resolve_herdr(path) {
        return Ok(found);
    }
    let Some(raw) = override_bin else {
        return Err("herdr not found on PATH".to_string());
    };
    let candidate = PathBuf::from(raw);
    if is_executable(&candidate) {
        Ok(candidate)
    } else {
        Err(format!(
            "herdr not found on PATH; {} is missing or not executable",
            candidate.display()
        ))
    }
}

fn pane_token(pane: &str) -> Result<(), String> {
    if pane.is_empty()
        || pane.starts_with('-')
        || pane.contains('/')
        || pane.contains('\\')
        || pane.bytes().any(|b| b.is_ascii_whitespace())
    {
        return Err(format!("refusing pane {pane}"));
    }
    Ok(())
}

fn key_token(key: &str) -> Result<(), String> {
    if key.is_empty() || key.starts_with('-') || key.bytes().any(|b| b.is_ascii_whitespace()) {
        return Err(format!("refusing key {key}"));
    }
    Ok(())
}

fn pane_ids(text: &str) -> Result<Vec<String>, String> {
    let value = parse_json(text)?;
    let mut ids = Vec::new();
    collect_ids(&value, "pane_id", &mut ids);
    Ok(ids)
}

fn collect_ids(value: &Value, key: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(id) = map.get(key).and_then(Value::as_str) {
                if !out.iter().any(|seen| seen == id) {
                    out.push(id.to_string());
                }
            }
            for child in map.values() {
                collect_ids(child, key, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_ids(child, key, out);
            }
        }
        _ => {}
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

// Contract helper. Room does not consult intake.
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
    const DEFAULT_WORKSPACES: &str =
        r#"{"result":{"workspaces":[{"workspace_id":"ws1","label":"crucible"}]}}"#;

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
  if [ "@@ROOT@@" = "absent" ] || [ "@@ROOT@@" = "notab" ]; then
    if [ "@@ROOT@@" = "notab" ]; then
      printf '%s\n' '{"result":{"tab":{"label":"'"$label"'","workspace_id":"ws1"}}}'
    else
      printf '%s\n' '{"result":{"tab":{"label":"'"$label"'","tab_id":"tab-'"$label"'","workspace_id":"ws1"}}}'
    fi
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

    fn assert_join_log(log: &str) {
        for banned in [
            "pane run",
            "camera",
            "reap",
            "workspace create",
            "workspace close",
            "workspace rename",
            "--session",
            "session stop",
            "session delete",
            "config.toml",
            "HERDR_CONFIG_PATH",
            "XDG_CONFIG_HOME",
            "tab close",
        ] {
            assert!(!log.contains(banned), "{banned} in log:\n{log}");
        }
        assert!(
            !log.split_whitespace().any(|word| word == "server"),
            "server in log:\n{log}"
        );
    }

    #[test]
    fn keys_type_into_the_joined_workspace_pane_only() {
        let tmp = Tmp::new();
        let dir = tmp.root.join(".crucible/herdr");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("workspace"), "crucible\n").unwrap();
        let cwd = tmp.root.display().to_string();
        let list = serde_json::json!({
            "result": {"workspaces": [
                {"workspace_id": "ws-here", "label": "crucible", "cwd": cwd},
                {"workspace_id": "ws-other", "label": "other", "cwd": "/elsewhere"}
            ]}
        })
        .to_string();
        let herdr = write_fake(
            &tmp,
            &FakeSpec {
                workspace_list: Some(list),
                panes: r#"{"result":{"panes":[{"pane_id":"pane-chat","tab_id":"tab-chat"}]}}"#,
                ..FakeSpec::default()
            },
        );
        let empty = std::ffi::OsStr::new("");
        let bin = herdr.as_os_str();
        let refused =
            type_into_pane(empty, Some(bin), &tmp.root, "pane-chat", &["--machine"]).unwrap_err();
        assert_eq!(refused, "refusing key --machine");
        assert!(!tmp.root.join("herdr.log").exists());
        let typed = type_into_pane(empty, Some(bin), &tmp.root, "pane-chat", &["esc"]).unwrap();
        assert_eq!(typed, "typed pane-chat esc\n");
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.lines().any(|line| line == "workspace list"), "{log}");
        assert!(
            log.lines()
                .any(|line| line == "pane list --workspace ws-here"),
            "{log}"
        );
        assert!(
            log.lines()
                .any(|line| line == "pane send-keys pane-chat esc"),
            "{log}"
        );
        assert!(!log.contains("--workspace ws-other"), "{log}");
        assert!(!log.contains("--machine"), "{log}");
        assert!(!log.contains("--remote"), "{log}");
        assert!(!log.contains("workspace create"), "{log}");
        assert!(!log.contains("pane run"), "{log}");
        assert!(!log.contains("config.toml"), "{log}");
        let foreign =
            type_into_pane(empty, Some(bin), &tmp.root, "pane-other", &["esc"]).unwrap_err();
        assert_eq!(
            foreign,
            "pane pane-other is not in the workspace this checkout joined"
        );
        let after = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert_eq!(
            after
                .lines()
                .filter(|line| *line == "pane send-keys pane-chat esc")
                .count(),
            1,
            "{after}"
        );
        fs::remove_file(dir.join("workspace")).unwrap();
        let missing =
            type_into_pane(empty, Some(bin), &tmp.root, "pane-chat", &["esc"]).unwrap_err();
        assert!(missing.contains("is missing"), "{missing}");
        let still = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert_eq!(still, after);
    }

    #[test]
    fn standing_roles_are_the_documented_contract() {
        let prod = include_str!("lib.rs").split("#[cfg(test)]").next().unwrap();
        assert!(prod.contains("standing roles: terminal, chat, orchestrator, dashboard"));
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
        assert!(ROLES.contains(&"terminal"));
        assert_eq!(ROLES.len(), 4);
        assert!(!ROLES.contains(&"watcher"));
        assert!(!ROLES.contains(&"reaper"));
        assert!(!ROLES.contains(&"watchdog"));
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
    fn arrange_creates_four_tabs_and_does_not_duplicate() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = fake_herdr(&tmp);
        let report = arrange(&herdr, &tmp.root).unwrap();
        assert_eq!(report.workspace_id, "ws1");
        assert_eq!(
            report.labels,
            ["terminal", "chat", "orchestrator", "dashboard"]
        );
        arrange(&herdr, &tmp.root).unwrap();
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert_eq!(
            log.matches("tab create").count(),
            4,
            "second attach must not create tabs again:\n{log}"
        );
        for role in ROLES {
            assert_eq!(
                log.matches(&format!("--label {role}")).count(),
                1,
                "{role} was not created once:\n{log}"
            );
        }
        assert!(log.contains("workspace list"), "{log}");
        assert_join_log(&log);
    }

    #[test]
    fn idea_does_not_start_go() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let body = serde_json::json!({
            "ok": true,
            "version": product_version()
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
        assert!(stdout.contains("go not started"), "{stdout}");
        assert!(!stdout.contains("go orchestrator"), "{stdout}");
        assert!(!stdout.contains("go waiting"), "{stdout}");
        assert!(!stdout.contains("go not started (orchestrator"), "{stdout}");
        assert!(stdout.contains("1.28.0"), "{stdout}");
        assert!(
            stdout.contains("standing roles: terminal, chat, orchestrator, dashboard"),
            "{stdout}"
        );
        assert!(
            stdout.contains("tabs terminal chat orchestrator dashboard"),
            "{stdout}"
        );
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("workspace list"), "{log}");
        assert_join_log(&log);
        assert!(!tmp.root.join(".wm").exists(), "room must not write TRACE");
    }

    #[test]
    fn guided_room_starts_the_orchestrator_and_not_go() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let prog = tmp.root.join(".crucible").join("work");
        fs::create_dir_all(&prog).unwrap();
        fs::write(prog.join("PROGRAM"), "cycle: guided\n").unwrap();
        let body = serde_json::json!({
            "ok": true,
            "version": product_version()
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
        assert!(stdout.contains("orchestrator started"), "{stdout}");
        assert!(!stdout.contains("go not started"), "{stdout}");
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("pane run pane-orchestrator"), "{log}");
        assert!(log.contains("orchestrate run"), "{log}");
        assert!(log.contains("pane run pane-dashboard"), "{log}");
        assert!(log.contains("message queue"), "{log}");
        assert!(log.contains("CRUCIBLE_ROOT"), "{log}");
        assert!(
            !log.contains("exec "),
            "pane run must not replace the shell:\n{log}"
        );
        assert!(
            log.contains(&tmp.root.display().to_string()),
            "pane run must name this checkout:\n{log}"
        );
        assert!(log.contains("tab list"), "{log}");
        assert!(!log.contains(" go"), "{log}");
        assert!(!log.contains("workspace create"), "{log}");
        assert!(!log.contains("config.toml"), "{log}");
    }

    #[test]
    fn live_tab_ids_still_start_the_labeled_panes() {
        let tabs = r#"{"result":{"tabs":[
            {"label":"crucible","tab_id":"w4:t1"},
            {"label":"terminal","tab_id":"w4:t5"},
            {"label":"chat","tab_id":"w4:t6"},
            {"label":"orchestrator","tab_id":"w4:t7"},
            {"label":"dashboard","tab_id":"w4:t8"}
        ]}}"#;
        let panes = r#"{"result":{"panes":[
            {"pane_id":"w4:p6","tab_id":"w4:t6"},
            {"pane_id":"w4:p7","tab_id":"w4:t7"},
            {"pane_id":"w4:p8","tab_id":"w4:t8"}
        ]}}"#;
        assert_eq!(
            super::pane_for_role(tabs, panes, "orchestrator").as_deref(),
            Some("w4:p7")
        );
        assert_eq!(
            super::pane_for_role(tabs, panes, "dashboard").as_deref(),
            Some("w4:p8")
        );
        assert_eq!(
            super::pane_for_role(tabs, panes, "chat").as_deref(),
            Some("w4:p6")
        );
        assert_eq!(super::pane_for_role(tabs, panes, "terminal"), None);
    }

    #[test]
    fn ready_backlog_does_not_start_go() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(
            tmp.root.join("BACKLOG.tsv"),
            "id\tsize\trisk\tidea_path\tstatus\ns1\tS\tLOW\tIDEA.md\tREADY\n",
        )
        .unwrap();
        assert!(intake_ready(&tmp.root));
        let herdr = fake_herdr(&tmp);
        let report = arrange(&herdr, &tmp.root).unwrap();
        assert_eq!(
            report.labels,
            ["terminal", "chat", "orchestrator", "dashboard"]
        );
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(!log.split_whitespace().any(|word| word == "go"), "{log}");
        assert_join_log(&log);
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
        assert!(stdout.contains("go not started"), "{stdout}");
        assert!(!stdout.contains("go orchestrator"), "{stdout}");
        assert!(!stdout.contains("go waiting"), "{stdout}");
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
        assert!(log.contains("workspace list"), "{log}");
        assert_join_log(&log);
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
            "terminal\nchat\norchestrator\n",
            "dashboard\norchestrator\nchat\nterminal\n",
            "terminal\nchat\norchestrator\ndashboard\nwatcher\n",
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
        arrange(&herdr, &tmp.root)
    }

    fn assert_list_did_not_mutate(tmp: &Tmp) {
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("workspace list"), "{log}");
        assert!(!log.contains("workspace create"), "{log}");
        assert!(!log.contains("workspace close"), "{log}");
        assert!(!log.contains("workspace rename"), "{log}");
    }

    #[test]
    fn attach_without_cwd_uses_the_single_label() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = fake_herdr(&tmp);
        let report = arrange(&herdr, &tmp.root).unwrap();
        assert_eq!(report.workspace_id, "ws1");
        assert_list_did_not_mutate(&tmp);
    }

    #[test]
    fn cwd_equal_to_checkout_attaches() {
        let tmp = Tmp::new();
        let list = serde_json::json!({
            "result": {
                "workspaces": [{
                    "workspace_id": "ws-here",
                    "label": "crucible",
                    "cwd": tmp.root.display().to_string()
                }]
            }
        })
        .to_string();
        let report = arrange_listed(&tmp, &list).unwrap();
        assert_eq!(report.workspace_id, "ws-here");
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("tab create"), "{log}");
        assert_join_log(&log);
    }

    #[test]
    fn cwd_mismatch_does_not_attach() {
        let tmp = Tmp::new();
        let err = arrange_listed(
            &tmp,
            r#"{"result":{"workspaces":[{"workspace_id":"ws1","label":"crucible","cwd":"/herdr-keeps-its-cwd"}]}}"#,
        )
        .unwrap_err();
        assert!(err.contains("herdr-init"), "{err}");
        assert!(
            err.contains("not the workspace whose cwd is this checkout"),
            "{err}"
        );
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("workspace list"), "{log}");
        assert!(!log.contains("tab create"), "{log}");
        assert!(!log.contains("workspace create"), "{log}");
        assert_join_log(&log);
    }

    #[test]
    fn label_hit_and_cwd_hit_must_be_the_same_workspace() {
        let tmp = Tmp::new();
        let checkout = tmp.root.display().to_string();
        let cases = [
            serde_json::json!({
                "result": {"workspaces": [
                    {"workspace_id": "ws-label", "label": "crucible", "cwd": "/other"},
                    {"workspace_id": "ws-cwd", "label": "other", "cwd": checkout.clone()}
                ]}
            }),
            serde_json::json!({
                "result": {"workspaces": [
                    {"workspace_id": "ws1", "label": "crucible", "cwd": checkout.clone()},
                    {"workspace_id": "ws2", "label": "other", "cwd": checkout.clone()}
                ]}
            }),
            serde_json::json!({
                "result": {"workspaces": [
                    {"workspace_id": "ws1", "label": "crucible", "cwd": checkout.clone()},
                    {"workspace_id": "ws2", "label": "crucible"}
                ]}
            }),
            serde_json::json!({
                "result": {"workspaces": [
                    {"workspace_id": "ws1", "label": "crucible"},
                    {"workspace_id": "ws2", "label": "other", "cwd": "/elsewhere"}
                ]}
            }),
        ];
        for list in cases {
            let err = arrange_listed(&tmp, &list.to_string()).unwrap_err();
            assert!(err.contains("herdr-init"), "{err}");
            let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
            assert!(!log.contains("tab create"), "{err}\n{log}");
            assert!(!log.contains("workspace create"), "{log}");
        }
    }

    #[test]
    fn omitted_or_non_string_cwd_is_not_a_report() {
        let tmp = Tmp::new();
        for list in [
            r#"{"result":{"workspaces":[{"workspace_id":"ws1","label":"crucible","cwd":null}]}}"#,
            r#"{"result":{"workspaces":[{"workspace_id":"ws1","label":"crucible","cwd":1}]}}"#,
            r#"{"result":{"workspaces":[{"workspace_id":"ws1","label":"crucible","worktree":{"checkout_path":"/not-cwd"}}]}}"#,
        ] {
            let report = arrange_listed(&tmp, list).unwrap();
            assert_eq!(report.workspace_id, "ws1");
        }
    }

    #[test]
    fn zero_workspaces_do_not_create() {
        let tmp = Tmp::new();
        let err = arrange_listed(&tmp, r#"{"result":{"workspaces":[]}}"#).unwrap_err();
        assert!(err.contains("no herdr workspace labeled"), "{err}");
        assert!(err.contains("herdr-init"), "{err}");
        assert_list_did_not_mutate(&tmp);
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(!log.contains("tab create"), "{log}");
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
        assert!(err.contains("herdr-init"), "{err}");
        assert_list_did_not_mutate(&tmp);
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(!log.contains("tab create"), "{log}");
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
    fn legacy_five_roles_join_and_leave_watcher_tabs() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(
            tmp.root.join(".crucible/herdr/roles"),
            "chat\norchestrator\nwatcher\nreaper\ndashboard\n",
        )
        .unwrap();
        let body = serde_json::json!({
            "ok": true,
            "version": product_version()
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
        assert!(stdout.contains("go not started"), "{stdout}");
        assert!(!stdout.contains("go orchestrator"), "{stdout}");
        assert!(!stdout.contains("go waiting"), "{stdout}");
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("workspace list"), "{log}");
        for role in ["terminal", "chat", "orchestrator", "dashboard"] {
            assert!(
                log.contains(&format!("--label {role}")),
                "{role} missing:\n{log}"
            );
        }
        assert!(!log.contains("--label watcher"), "{log}");
        assert!(!log.contains("--label reaper"), "{log}");
        assert!(!log.contains("--label watchdog"), "{log}");
        assert_join_log(&log);
    }

    #[test]
    fn existing_watcher_and_reaper_tabs_are_left_alone() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = fake_herdr(&tmp);
        seed_tabs(&tmp, &["watcher", "reaper", "chat"]);
        arrange(&herdr, &tmp.root).unwrap();
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(log.contains("--label terminal"), "{log}");
        assert!(log.contains("--label orchestrator"), "{log}");
        assert!(log.contains("--label dashboard"), "{log}");
        assert!(!log.contains("--label chat"), "{log}");
        assert!(!log.contains("--label watcher"), "{log}");
        assert!(!log.contains("--label reaper"), "{log}");
        assert!(!log.contains("--label watchdog"), "{log}");
        assert_eq!(log.matches("tab create").count(), 3, "{log}");
        assert_join_log(&log);
    }

    #[test]
    fn tab_create_failure_pane_runs_nothing() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = write_fake(
            &tmp,
            &FakeSpec {
                fail_tab: "dashboard",
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
        assert_join_log(&log);
        let err = arrange(&herdr, &tmp.root).unwrap_err();
        assert!(
            err.contains("tabs created earlier in this call were not started"),
            "{err}"
        );
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert_join_log(&log);
    }

    #[test]
    fn tab_create_without_tab_id_does_not_continue() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = write_fake(
            &tmp,
            &FakeSpec {
                root: "notab",
                ..FakeSpec::default()
            },
        );
        let err = arrange(&herdr, &tmp.root).unwrap_err();
        assert!(
            err.contains("tabs created earlier in this call were not started"),
            "{err}"
        );
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(!log.contains("pane list"), "{log}");
        assert_join_log(&log);
    }

    #[test]
    fn missing_root_pane_does_not_list_panes() {
        let tmp = Tmp::new();
        plant_layout(&tmp.root);
        let herdr = write_fake(
            &tmp,
            &FakeSpec {
                root: "absent",
                ..FakeSpec::default()
            },
        );
        arrange(&herdr, &tmp.root).unwrap();
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        assert!(!log.contains("pane list"), "{log}");
        assert_eq!(log.matches("tab create").count(), 4, "{log}");
        assert_join_log(&log);
    }
}
