//! `crucible room` starts loopback `serve` if needed, then asks external `herdr`
//! for standing tabs. Cameras GET the API. `go` is a process in the orchestrator
//! tab, never `POST /go`. Do not vendor an init tree. No Herdr crate.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::Value;

const SERVE_BIND: &str = "127.0.0.1:0";

pub const ROLES: [&str; 5] = ["chat", "orchestrator", "watcher", "reaper", "dashboard"];

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

/// Require `herdr` on `path`, spawn this `exe serve`, then create standing tabs.
/// Missing herdr: exit 2, do not spawn, do not listen.
pub fn run(exe: &Path, cwd: &Path, path: &OsStr) -> i32 {
    let Some(herdr) = resolve_herdr(path) else {
        let _ = writeln!(io::stderr(), "room: herdr not found on PATH");
        return 2;
    };
    let (addr, pid, guard) = match spawn_serve(exe, cwd) {
        Ok(v) => v,
        Err(e) => {
            let _ = writeln!(io::stderr(), "room: {e}");
            return 1;
        }
    };
    let body = match http_get(&addr, "/health") {
        Ok(b) => b,
        Err(e) => {
            let _ = writeln!(io::stderr(), "room: {e}");
            drop(guard);
            return 1;
        }
    };
    let report = match arrange(&herdr, exe, cwd, &addr) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(io::stderr(), "room: {e}");
            drop(guard);
            return 1;
        }
    };
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let _ = writeln!(
        stdout,
        "standing roles: chat, orchestrator, watcher, reaper, dashboard"
    );
    let _ = writeln!(stdout, "listening {addr}");
    let _ = writeln!(stdout, "serve pid {pid}");
    let _ = writeln!(stdout, "GET /health {addr}");
    let _ = writeln!(stdout, "{body}");
    let _ = writeln!(stdout, "workspace {}", report.workspace_id);
    let _ = writeln!(stdout, "tabs {}", report.labels.join(" "));
    if report.started_go {
        let _ = writeln!(stdout, "go orchestrator");
    } else {
        let _ = writeln!(stdout, "go waiting");
    }
    let _ = stdout.flush();
    // Serve stays up for cameras. Forgetting the guard skips the kill-on-drop.
    std::mem::forget(guard);
    0
}

fn spawn_serve(exe: &Path, cwd: &Path) -> Result<(String, u32, ChildGuard), String> {
    let mut child = Command::new(exe)
        .args(["serve", "--bind", SERVE_BIND])
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

struct RoomReport {
    workspace_id: String,
    labels: Vec<String>,
    started_go: bool,
}

fn arrange(herdr: &Path, exe: &Path, cwd: &Path, addr: &str) -> Result<RoomReport, String> {
    let workspace_id = ensure_workspace(herdr, cwd)?;
    ensure_tabs(herdr, cwd, &workspace_id)?;
    let panes = pane_by_label(herdr, &workspace_id)?;
    for role in ROLES {
        if !panes.iter().any(|(label, _)| label == role) {
            return Err(format!("herdr tab {role} has no pane"));
        }
    }
    let exe_s = exe.display().to_string();
    let mut started_go = false;
    if intake_ready(cwd) {
        let orch = pane_of(&panes, "orchestrator")?;
        herdr_ok(herdr, &["pane", "run", orch, &exe_s, "go"])?;
        started_go = true;
        if let Some(pid) = go_pid(herdr, orch) {
            let reap = pane_of(&panes, "reaper")?;
            let pid_s = pid.to_string();
            herdr_ok(
                herdr,
                &["pane", "run", reap, &exe_s, "reap", "--pid", &pid_s],
            )?;
        }
    }
    let watch = pane_of(&panes, "watcher")?;
    let dash = pane_of(&panes, "dashboard")?;
    herdr_ok(
        herdr,
        &["pane", "run", watch, &exe_s, "camera", "--bind", addr],
    )?;
    herdr_ok(
        herdr,
        &["pane", "run", dash, &exe_s, "camera", "--bind", addr],
    )?;
    let labels: Vec<String> = ROLES.iter().map(|s| (*s).to_string()).collect();
    Ok(RoomReport {
        workspace_id,
        labels,
        started_go,
    })
}

fn pane_of<'a>(panes: &'a [(String, String)], role: &str) -> Result<&'a str, String> {
    panes
        .iter()
        .find(|(label, _)| label == role)
        .map(|(_, pane)| pane.as_str())
        .ok_or_else(|| format!("missing pane for {role}"))
}

fn ensure_workspace(herdr: &Path, cwd: &Path) -> Result<String, String> {
    let cwd_s = cwd.display().to_string();
    let listed = herdr_ok(herdr, &["workspace", "list"])?;
    if let Some(id) = workspace_matching_cwd(&listed, &cwd_s) {
        return Ok(id);
    }
    let created = herdr_ok(
        herdr,
        &[
            "workspace",
            "create",
            "--cwd",
            &cwd_s,
            "--label",
            "crucible",
            "--no-focus",
        ],
    )?;
    first_key(&created, "workspace_id")
        .ok_or_else(|| format!("herdr workspace create did not return workspace_id: {created}"))
}

fn ensure_tabs(herdr: &Path, cwd: &Path, workspace_id: &str) -> Result<(), String> {
    let cwd_s = cwd.display().to_string();
    let listed = herdr_ok(herdr, &["tab", "list", "--workspace", workspace_id])?;
    let have = tab_labels(&listed);
    for role in ROLES {
        if have.iter().any(|l| l == role) {
            continue;
        }
        herdr_ok(
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
        )?;
    }
    Ok(())
}

fn pane_by_label(herdr: &Path, workspace_id: &str) -> Result<Vec<(String, String)>, String> {
    let tabs = herdr_ok(herdr, &["tab", "list", "--workspace", workspace_id])?;
    let panes = herdr_ok(herdr, &["pane", "list", "--workspace", workspace_id])?;
    let mut tab_to_label = Vec::new();
    collect_pairs(&parse_json(&tabs)?, "tab_id", "label", &mut tab_to_label);
    let mut tab_to_pane = Vec::new();
    collect_pairs(&parse_json(&panes)?, "tab_id", "pane_id", &mut tab_to_pane);
    let mut out = Vec::new();
    for role in ROLES {
        let Some(tab_id) = tab_to_label
            .iter()
            .find(|(_, label)| label == role)
            .map(|(id, _)| id)
        else {
            continue;
        };
        if let Some((_, pane)) = tab_to_pane.iter().find(|(id, _)| id == tab_id) {
            out.push(((*role).to_string(), pane.clone()));
        }
    }
    Ok(out)
}

fn go_pid(herdr: &Path, pane: &str) -> Option<u32> {
    let text = herdr_ok(herdr, &["pane", "process-info", pane]).ok()?;
    let raw = first_key(&text, "pid")?;
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

fn workspace_matching_cwd(text: &str, cwd: &str) -> Option<String> {
    let v = parse_json(text).ok()?;
    let mut hits = Vec::new();
    walk_workspace(&v, cwd, &mut hits);
    hits.into_iter().next()
}

fn walk_workspace(v: &Value, cwd: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            let label = map.get("label").and_then(|x| x.as_str()).unwrap_or("");
            let id = map.get("workspace_id").and_then(|x| x.as_str());
            let paths = string_values(map.get("cwd"))
                .into_iter()
                .chain(string_values(map.get("checkout_path")))
                .chain(string_values(map.get("worktree")));
            if label == "crucible" {
                if let Some(id) = id {
                    if paths.into_iter().any(|p| p == cwd) {
                        out.push(id.to_string());
                    }
                }
            }
            for child in map.values() {
                walk_workspace(child, cwd, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                walk_workspace(child, cwd, out);
            }
        }
        _ => {}
    }
}

fn string_values(v: Option<&Value>) -> Vec<String> {
    let Some(v) = v else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_all_strings(v, &mut out);
    out
}

fn collect_all_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Object(map) => {
            for child in map.values() {
                collect_all_strings(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_all_strings(child, out);
            }
        }
        _ => {}
    }
}

pub fn intake_ready(cwd: &Path) -> bool {
    idea_nonempty(cwd) || backlog_ready(cwd)
}

fn idea_nonempty(cwd: &Path) -> bool {
    fs::read_to_string(cwd.join("IDEA.md"))
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

fn backlog_ready(cwd: &Path) -> bool {
    let Ok(text) = fs::read_to_string(cwd.join("BACKLOG.tsv")) else {
        return false;
    };
    for line in text.lines().skip(1) {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split('\t');
        let status = cols.nth(4).unwrap_or("");
        if status == "READY" {
            return true;
        }
    }
    false
}

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

    fn fake_herdr(tmp: &Tmp) -> PathBuf {
        let bin = tmp.root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let state = tmp.root.join("tabs.jsonl");
        let log = tmp.root.join("herdr.log");
        let script = format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> {log}
state={state}
cmd="$1 $2"
if [ "$cmd" = "workspace list" ]; then
  printf '%s\n' '{{"result":{{"workspaces":[]}}}}'
elif [ "$cmd" = "workspace create" ]; then
  printf '%s\n' '{{"result":{{"workspace":{{"workspace_id":"ws1","label":"crucible","cwd":"'"$4"'"}}}}}}'
elif [ "$cmd" = "tab list" ]; then
  printf '%s' '{{"result":{{"tabs":['
  sep=""
  if [ -f "$state" ]; then
    while IFS= read -r label; do
      [ -n "$label" ] || continue
      printf '%s%s' "$sep" '{{"label":"'"$label"'","tab_id":"tab-'"$label"'"}}'
      sep=","
    done < "$state"
  fi
  printf '%s\n' ']}}}}'
elif [ "$cmd" = "tab create" ]; then
  label=""
  prev=""
  for a in "$@"; do
    if [ "$prev" = "--label" ]; then label=$a; fi
    prev=$a
  done
  printf '%s\n' "$label" >> "$state"
  printf '%s\n' '{{"result":{{"tab":{{"label":"'"$label"'","tab_id":"tab-'"$label"'","workspace_id":"ws1"}}}}}}'
elif [ "$cmd" = "pane list" ]; then
  printf '%s\n' '{{"result":{{"panes":[{{"pane_id":"pane-chat","tab_id":"tab-chat"}},{{"pane_id":"pane-orchestrator","tab_id":"tab-orchestrator"}},{{"pane_id":"pane-watcher","tab_id":"tab-watcher"}},{{"pane_id":"pane-reaper","tab_id":"tab-reaper"}},{{"pane_id":"pane-dashboard","tab_id":"tab-dashboard"}}]}}}}'
elif [ "$cmd" = "pane process-info" ]; then
  printf '%s\n' '{{"result":{{"pid":0}}}}'
else
  printf '%s\n' '{{"result":{{"ok":true}}}}'
fi
exit 0
"#,
            log = log.display(),
            state = state.display(),
        );
        let path = bin.join("herdr");
        write_exec(&path, &script);
        path
    }

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
    fn arrange_creates_five_tabs_and_does_not_duplicate() {
        let tmp = Tmp::new();
        let herdr = fake_herdr(&tmp);
        let exe = exe_marker(&tmp);
        arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
        let creates = log.matches("tab create").count();
        assert_eq!(
            creates, 5,
            "second attach must not create tabs again:\n{log}"
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
    }

    #[test]
    fn idea_starts_go_only_in_orchestrator_argv() {
        let tmp = Tmp::new();
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let herdr = fake_herdr(&tmp);
        let exe = exe_marker(&tmp);
        let report = arrange(&herdr, &exe, &tmp.root, "127.0.0.1:9").unwrap();
        assert!(report.started_go);
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
            let tree = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            assert!(!tree.contains("herdr"), "herdr in {pkg} tree:\n{tree}");
            assert!(!tree.contains("grok"), "grok in {pkg} tree:\n{tree}");
            assert!(!tree.contains("engos"), "engos in {pkg} tree:\n{tree}");
        }
    }
}
