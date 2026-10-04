//! Loopback web camera. It serves a page and proxies GET `/walk`, `/stats`, and
//! `/health`. It appends `BACKLOG.tsv`. `GET /api/chat` spawns `message show`.
//! `GET /api/factory` spawns `message queue`. `GET /api/dashboard` reads
//! records in the served directory and does not spawn. `POST /act/go` spawns
//! `go` in a new process group and does not walk. Read-only `POST /act/<verb>`
//! spawns that verb and waits. `POST /act/drive` and `POST /act/adopt` detach.
//! `POST /act/close`, bare `POST /act/status`, `POST /act/state`,
//! `POST /act/target`, `POST /act/brief`, `POST /act/lifecycle`,
//! `POST /act/evidence`, `POST /act/add`, `POST /act/attempt`,
//! `POST /act/claim`, `POST /act/contract-audit`, `POST /act/cycle`,
//! `POST /act/phase`, `POST /act/plan-audit`, `POST /act/probe-acp`, and
//! `POST /act/ready` wait. `check` and `triage` are read-only.
//! `POST /go` is not a walk.

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crucible_http::ServeError;

pub const DEFAULT_WEB_BIND: &str = "127.0.0.1:1735";
pub const DEFAULT_API_BIND: &str = "127.0.0.1:1734";

// Raw POST body cap. The chat cap is the decoded line, not this framing limit.
const BODY_CAP: usize = 8192;
const HEADER_CAP: usize = 8192;
const FIELD_MAX: usize = 256;
const NOT_A_WALK: &str = "POST is not a walk\n";
const OK_JSON: &str = "{\"ok\":true}\n";
const BACKLOG_HEADER: &str = "id\tsize\trisk\tidea_path\tstatus";

fn page_html() -> String {
    let doc = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Crucible</title>
<style>
body { font: 16px/1.4 ui-sans-serif, system-ui, sans-serif; margin: 2rem; }
pre { background: #f4f4f4; padding: 1rem; overflow: auto; }
label { display: block; margin: 0.25rem 0; }
</style>
</head>
<body>
<h1>Crucible</h1>
<p>Backlog: <a href="/api/backlog">/api/backlog</a>. Messages: <a href="/api/chat">/api/chat</a> (plain text).</p>
<button id="reload" type="button">Reload</button>
<button id="start" type="button">Start</button>
<h2>Read</h2>
<!--READ-->
<label>args <input id="read-args" type="text" placeholder="A non-empty value replaces the button default and is split on whitespace, with no quotes"></label>
<pre id="read"></pre>
<h2>Health</h2><pre id="health"></pre>
<h2>Walk</h2><pre id="walk"></pre>
<h2>Floor</h2><pre id="floor"></pre>
<h2>Factory</h2><pre id="factory"></pre>
<pre id="question"></pre>
<h2>Dashboard</h2><pre id="dashboard"></pre>
<h2>Stats</h2><pre id="stats"></pre>
<h2>Backlog</h2>
<label>id <input id="b-id" type="text"></label>
<label>size <input id="b-size" type="text"></label>
<label>risk <input id="b-risk" type="text"></label>
<label>idea_path <input id="b-idea" type="text"></label>
<label>status <input id="b-status" type="text"></label>
<button id="backlog-add" type="button">Add backlog</button>
<pre id="backlog"></pre>
<h2>Messages</h2>
<label>kind <input id="c-kind" type="text" value="source"></label>
<label>text <input id="c-line" type="text"></label>
<button id="chat-send" type="button">Send</button>
<pre id="chat"></pre>
<pre id="go"></pre>
<script>
const actHeaders = {
  "Content-Type": "application/json",
  "X-Crucible-Act": "1"
};
async function postAct(path, payload) {
  const res = await fetch(path, {
    method: "POST",
    headers: actHeaders,
    body: JSON.stringify(payload)
  });
  return res.text();
}
function questionText(queue) {
  const q = questionLine(queue);
  if (q === "") return "";
  return q + "\nPublish, delete, or leave this machine.\n1. Publish. The work leaves this machine.\n2. Delete. The copy on this machine is removed.\n3. Leave. The work stays on this machine.\nRecommend 3. Leave keeps the work here.";
}
function questionLine(queue) {
  const lines = queue.split("\n");
  for (const line of lines) {
    if (line === "" || line === "idle") continue;
    const space = line.indexOf(" ");
    if (space < 0) continue;
    const id = line.slice(0, space);
    const status = line.slice(space + 1);
    if (status === "paused" || status === "escalated") {
      return "1 " + id;
    }
  }
  return "";
}
async function load() {
  for (const [id, path] of [["floor","/api/floor"],["health","/api/health"],["walk","/api/walk"],["factory","/api/factory"],["dashboard","/api/dashboard"],["stats","/api/stats?since=1h"],["backlog","/api/backlog"],["chat","/api/chat"]]) {
    const el = document.getElementById(id);
    try {
      const res = await fetch(path, { method: "GET" });
      el.textContent = await res.text();
    } catch (e) {
      el.textContent = String(e);
    }
    if (id === "factory") {
      document.getElementById("question").textContent = questionText(el.textContent);
    }
  }
}
document.getElementById("reload").addEventListener("click", load);
document.getElementById("backlog-add").addEventListener("click", async () => {
  const el = document.getElementById("backlog");
  try {
    el.textContent = await postAct("/act/backlog", {
      id: document.getElementById("b-id").value,
      size: document.getElementById("b-size").value,
      risk: document.getElementById("b-risk").value,
      idea_path: document.getElementById("b-idea").value,
      status: document.getElementById("b-status").value
    });
  } catch (e) {
    el.textContent = String(e);
  }
});
document.getElementById("chat-send").addEventListener("click", async () => {
  const el = document.getElementById("chat");
  try {
    const res = await fetch("/act/message", {
      method: "POST",
      headers: actHeaders,
      body: JSON.stringify({
        args: ["manager", document.getElementById("c-kind").value, document.getElementById("c-line").value]
      })
    });
    const text = await res.text();
    const exit = res.headers.get("X-Crucible-Exit");
    if (exit === "0") {
      el.textContent = text;
      load();
    } else {
      el.textContent = text;
    }
  } catch (e) {
    el.textContent = String(e);
  }
});
document.getElementById("start").addEventListener("click", async () => {
  const el = document.getElementById("go");
  try {
    el.textContent = await postAct("/act/go", {});
  } catch (e) {
    el.textContent = String(e);
  }
});
document.querySelectorAll("button[data-verb]").forEach((btn) => {
  btn.addEventListener("click", async () => {
    const raw = document.getElementById("read-args").value.trim();
    const args = raw.length ? raw.split(/\s+/) : JSON.parse(btn.getAttribute("data-args"));
    const el = document.getElementById("read");
    try {
      const res = await fetch("/act/" + btn.getAttribute("data-verb"), {
        method: "POST",
        headers: actHeaders,
        body: JSON.stringify({ args })
      });
      const text = await res.text();
      const exit = res.headers.get("X-Crucible-Exit");
      el.textContent = (exit && exit !== "0") ? (text + "\nexit " + exit + "\n") : text;
    } catch (e) {
      el.textContent = String(e);
    }
  });
});
load();
</script>
</body>
</html>
"#;
    doc.replace("<!--READ-->", "")
}

pub fn bind_web(spec: &str) -> Result<TcpListener, ServeError> {
    crucible_http::bind_listener(spec)
}

pub fn serve_web(listener: TcpListener, api: &str) -> Result<(), ServeError> {
    let cwd = std::env::current_dir().map_err(|e| ServeError::Io(e.to_string()))?;
    let exe = std::env::current_exe().map_err(|e| ServeError::Io(e.to_string()))?;
    crucible_http::parse_bind(api)?;
    for conn in listener.incoming() {
        let mut stream = conn.map_err(|e| ServeError::Io(e.to_string()))?;
        if let Err(e) = handle_one(&mut stream, api, &cwd, &exe) {
            let _ = writeln!(io::stderr(), "web: {e}");
        }
    }
    Ok(())
}

pub fn serve_n(listener: TcpListener, api: &str, n: usize) -> Result<(), ServeError> {
    let cwd = std::env::current_dir().map_err(|e| ServeError::Io(e.to_string()))?;
    let exe = std::env::current_exe().map_err(|e| ServeError::Io(e.to_string()))?;
    serve_n_at(listener, api, &cwd, &exe, n)
}

fn serve_n_at(
    listener: TcpListener,
    api: &str,
    cwd: &Path,
    exe: &Path,
    n: usize,
) -> Result<(), ServeError> {
    crucible_http::parse_bind(api)?;
    for _ in 0..n {
        let mut stream = listener
            .accept()
            .map_err(|e| ServeError::Io(e.to_string()))?
            .0;
        if let Err(e) = handle_one(&mut stream, api, cwd, exe) {
            let _ = writeln!(io::stderr(), "web: {e}");
        }
    }
    Ok(())
}

struct Incoming {
    method: String,
    path: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

struct Head {
    method: String,
    path: String,
    target: String,
    headers: Vec<(String, String)>,
}

enum ReadErr {
    Http(u16, &'static str),
    Io(String),
}

fn handle_one(stream: &mut TcpStream, api: &str, cwd: &Path, exe: &Path) -> Result<(), String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let req = match read_request(stream) {
        Ok(req) => req,
        Err(ReadErr::Http(code, msg)) => {
            return write_resp(stream, code, "text/plain", msg, false);
        }
        Err(ReadErr::Io(e)) => return Err(e),
    };
    route(stream, api, cwd, exe, &req)
}

const FLOOR_CAP: usize = 262_144;

fn read_floor(cwd: &Path) -> Result<String, &'static str> {
    let (_repo, wm) = crucible_contract::resolve_wm(cwd);
    let path = wm.join("FLOOR.md");
    // stat does not block on a FIFO. Opening a FIFO does, and take
    // never runs. is_file is false for a FIFO, a directory, a socket, and
    // a device. metadata follows a symlink, so a symlink to a FIFO is refused
    // and a symlink to a regular file is still read. The size decision is
    // the buffer after take, not the metadata length.
    match fs::metadata(&path) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Err("floor unreadable\n"),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return match fs::symlink_metadata(&path) {
                Ok(_) => Err("floor unreadable\n"),
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(String::new()),
                Err(_) => Err("floor unreadable\n"),
            };
        }
        Err(_) => return Err("floor unreadable\n"),
    }
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(String::new()),
        Err(_) => return Err("floor unreadable\n"),
    };
    let mut bytes = Vec::new();
    let mut limited = file.take((FLOOR_CAP as u64) + 1);
    if limited.read_to_end(&mut bytes).is_err() {
        return Err("floor unreadable\n");
    }
    if bytes.len() > FLOOR_CAP {
        return Err("floor too large\n");
    }
    String::from_utf8(bytes).map_err(|_| "floor unreadable\n")
}

fn route(
    stream: &mut TcpStream,
    api: &str,
    cwd: &Path,
    exe: &Path,
    req: &Incoming,
) -> Result<(), String> {
    let head = req.method == "HEAD";
    if is_act(&req.path) {
        if req.method != "POST" {
            return write_resp(stream, 405, "text/plain", NOT_A_WALK, head);
        }
        return act(stream, cwd, exe, req);
    }
    if let Some(verb) = act_segment(&req.path) {
        if req.method != "POST" {
            return write_resp(stream, 404, "text/plain", "not found\n", false);
        }
        return post_read(stream, cwd, exe, verb, req);
    }
    if req.method != "GET" && req.method != "HEAD" {
        return write_resp(stream, 405, "text/plain", NOT_A_WALK, false);
    }
    if req.path == "/api/floor" {
        return match read_floor(cwd) {
            Ok(body) => write_resp(stream, 200, "text/plain; charset=utf-8", &body, head),
            Err(msg) => write_resp(stream, 500, "text/plain", msg, head),
        };
    }
    if req.path == "/" || req.path == "/index.html" {
        return write_resp(stream, 200, "text/html; charset=utf-8", &page_html(), head);
    }
    if req.path == "/api/backlog" {
        match backlog_json(cwd) {
            Ok(body) => return write_resp(stream, 200, "application/json", &body, head),
            Err(e) => {
                let body = if e.ends_with('\n') {
                    e
                } else {
                    format!("{e}\n")
                };
                return write_resp(stream, 500, "text/plain", &body, head);
            }
        }
    }
    if req.path == "/api/dashboard" {
        let body = crucible_guided::dashboard::dashboard(cwd);
        return write_resp(stream, 200, "text/plain; charset=utf-8", &body, head);
    }
    if req.path == "/api/factory" {
        return match spawn_read(exe, cwd, "message", &["queue".to_string()]) {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                write_resp(stream, 200, "text/plain; charset=utf-8", &text, head)
            }
            Err(ReadSpawn::Spawn) => write_resp(stream, 500, "text/plain", "spawn failed\n", head),
            Err(ReadSpawn::Timeout) => {
                write_resp(stream, 504, "text/plain", "act timed out\n", head)
            }
        };
    }
    if req.path == "/api/chat" {
        return match spawn_read(exe, cwd, "message", &["show".to_string()]) {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                write_resp(stream, 200, "text/plain; charset=utf-8", &text, head)
            }
            Err(ReadSpawn::Spawn) => write_resp(stream, 500, "text/plain", "spawn failed\n", head),
            Err(ReadSpawn::Timeout) => {
                write_resp(stream, 504, "text/plain", "act timed out\n", head)
            }
        };
    }
    let upstream = match req.path.as_str() {
        "/api/health" => "/health".to_string(),
        "/api/walk" => "/walk".to_string(),
        "/api/stats" => {
            if req.target.contains("since=") {
                req.target.trim_start_matches("/api").to_string()
            } else {
                "/stats?since=1h".to_string()
            }
        }
        _ => return write_resp(stream, 404, "text/plain", "not found\n", head),
    };
    match proxy_get(api, &upstream) {
        Ok(body) => write_resp(stream, 200, "application/json", &body, head),
        Err(e) => write_resp(stream, 502, "text/plain", &e, head),
    }
}

/// Verbs the loopback page may spawn. One array literal. `scripts/selftest.sh`
/// parses this const. Do not copy it into JavaScript or into a second slice.
// One name per line: the selftest awk skips the declaration line, so a
// collapsed `= &[...];` extracts nothing.
#[rustfmt::skip]
pub const WEB_READ_ONLY: &[&str] = &[
    "agents",
    "check",
    "debrief",
    "next",
    "panes",
    "stats",
    "triage",
    "workid",
];

/// Verbs the page may spawn that write. Not part of `WEB_READ_ONLY`.
/// `scripts/selftest.sh` parses this const. One name per line.
#[rustfmt::skip]
pub const WEB_WRITERS: &[&str] = &[
    "add",
    "adopt",
    "attempt",
    "brief",
    "claim",
    "close",
    "contract-audit",
    "cycle",
    "drive",
    "evidence",
    "lifecycle",
    "message",
    "phase",
    "plan-audit",
    "probe-acp",
    "ready",
    "result",
    "state",
    "status",
    "target",
];

/// Read-only verbs allow any args. Writers other than `status` allow any
/// args the JSON parser accepted. `status` allows only `[]` or `["--json"]`.
pub fn web_act_allowed(verb: &str, args: &[String]) -> bool {
    if WEB_READ_ONLY.contains(&verb) {
        return true;
    }
    if !WEB_WRITERS.contains(&verb) {
        return false;
    }
    if verb == "status" {
        return args.is_empty() || (args.len() == 1 && args[0] == "--json");
    }
    if verb == "result" {
        return result_page_args(args);
    }
    true
}

/// Closed argv for `result`. No shell command: attempt id, outcome, one
/// evidence filename, next action, and an optional fingerprint.
fn result_page_args(args: &[String]) -> bool {
    if args.len() != 4 && args.len() != 5 {
        return false;
    }
    let attempt = args[0].as_str();
    let outcome = args[1].as_str();
    let evidence = args[2].as_str();
    let next = args[3].as_str();
    if !page_token(attempt) || !page_filename(evidence) {
        return false;
    }
    if !matches!(
        outcome,
        "PASS" | "REJECT" | "BLOCKED" | "NEEDS_CONTEXT" | "SCOPE_CONFLICT"
    ) {
        return false;
    }
    if !matches!(next, "CLOSE" | "FIX" | "DECIDE" | "ESCALATE") {
        return false;
    }
    if args.len() == 5 {
        let fp = args[4].as_str();
        if fp != "-" && !(fp.len() == 12 && fp.bytes().all(|b| b.is_ascii_alphanumeric())) {
            return false;
        }
    }
    true
}

fn page_token(s: &str) -> bool {
    !s.is_empty()
        && !s.contains('/')
        && !s.contains('\\')
        && !s.contains("..")
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn page_filename(s: &str) -> bool {
    page_token(s) && !s.starts_with('.')
}

pub struct WebAct {
    pub verb: &'static str,
    pub args: &'static [&'static str],
}

pub fn web_page_acts() -> Vec<WebAct> {
    let mut acts = Vec::with_capacity(WEB_READ_ONLY.len() + 1 + WEB_WRITERS.len());
    for verb in WEB_READ_ONLY {
        let args: &'static [&'static str] = if *verb == "stats" {
            &["--since", "24h", "--json"]
        } else {
            &[]
        };
        acts.push(WebAct { verb, args });
    }
    acts.push(WebAct {
        verb: "status",
        args: &["--json"],
    });
    for verb in WEB_WRITERS {
        let args: &'static [&'static str] = if *verb == "adopt" {
            &["--managed"]
        } else if *verb == "lifecycle" {
            &["status"]
        } else {
            &[]
        };
        acts.push(WebAct { verb, args });
    }
    acts
}

fn is_act(path: &str) -> bool {
    matches!(path, "/act/backlog" | "/act/go")
}

fn act_segment(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/act/")?;
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    let ok = rest.bytes().enumerate().all(|(i, b)| {
        if i == 0 {
            b.is_ascii_lowercase()
        } else {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'
        }
    });
    if ok {
        Some(rest)
    } else {
        None
    }
}

fn act(stream: &mut TcpStream, cwd: &Path, exe: &Path, req: &Incoming) -> Result<(), String> {
    if let Err(msg) = act_headers(&req.headers) {
        return write_resp(stream, 400, "text/plain", msg, false);
    }
    match req.path.as_str() {
        "/act/backlog" => post_backlog(stream, cwd, &req.body),
        "/act/go" => post_go(stream, cwd, exe, &req.body),
        _ => write_resp(stream, 405, "text/plain", NOT_A_WALK, false),
    }
}

fn act_headers(headers: &[(String, String)]) -> Result<(), &'static str> {
    let ct = header(headers, "content-type").unwrap_or("");
    if !json_content_type(ct) {
        return Err("not json\n");
    }
    match header(headers, "x-crucible-act") {
        Some(v) if v.trim() == "1" => Ok(()),
        _ => Err("not an act\n"),
    }
}

fn json_content_type(value: &str) -> bool {
    value
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/json")
}

fn post_backlog(stream: &mut TcpStream, cwd: &Path, body: &[u8]) -> Result<(), String> {
    let v = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(v) => v,
        Err(_) => return write_resp(stream, 400, "text/plain", "bad backlog\n", false),
    };
    let row = match row_from_json(&v) {
        Ok(row) => row,
        Err(msg) => return write_resp(stream, 400, "text/plain", msg, false),
    };
    match append_backlog(cwd, &row) {
        Ok(()) => write_resp(stream, 200, "application/json", OK_JSON, false),
        Err(BacklogWrite::Bad) => write_resp(stream, 400, "text/plain", "bad backlog\n", false),
        Err(BacklogWrite::Io(e)) => write_resp(stream, 500, "text/plain", &format!("{e}\n"), false),
    }
}

fn post_go(stream: &mut TcpStream, cwd: &Path, exe: &Path, body: &[u8]) -> Result<(), String> {
    // Reject a bad body before intake so it cannot spawn or look like "not ready".
    if let Err(msg) = go_body(body) {
        return write_resp(stream, 400, "text/plain", msg, false);
    }
    // 200 means the process group exists, not that the walk succeeded.
    // A guided program is ready without IDEA.md: go runs drive.
    if !crucible_contract::intake_ready(cwd) && !guided_checkout(cwd) {
        return write_resp(stream, 409, "text/plain", "not ready\n", false);
    }
    match spawn_go(exe, cwd) {
        Ok(pid) => write_resp(stream, 200, "application/json", &pid_json(pid), false),
        Err(()) => write_resp(stream, 500, "text/plain", "spawn failed\n", false),
    }
}

fn post_read(
    stream: &mut TcpStream,
    cwd: &Path,
    exe: &Path,
    verb: &str,
    req: &Incoming,
) -> Result<(), String> {
    if let Err(msg) = act_headers(&req.headers) {
        return write_resp(stream, 400, "text/plain", msg, false);
    }
    if !WEB_READ_ONLY.contains(&verb) && !WEB_WRITERS.contains(&verb) {
        return write_resp(stream, 404, "text/plain", "not found\n", false);
    }
    let parsed: serde_json::Value = match serde_json::from_slice(&req.body) {
        Ok(v) => v,
        Err(_) => return write_resp(stream, 400, "text/plain", "bad args\n", false),
    };
    let Some(obj) = parsed.as_object() else {
        return write_resp(stream, 400, "text/plain", "bad args\n", false);
    };
    if obj.keys().any(|k| k != "args") {
        return write_resp(stream, 400, "text/plain", "bad args\n", false);
    }
    let args = match obj.get("args") {
        None => Vec::new(),
        Some(serde_json::Value::Array(items)) => {
            if items.len() > 32 {
                return write_resp(stream, 400, "text/plain", "bad args\n", false);
            }
            let mut args = Vec::with_capacity(items.len());
            for item in items {
                let Some(s) = item.as_str() else {
                    return write_resp(stream, 400, "text/plain", "bad args\n", false);
                };
                if s.len() > 256 {
                    return write_resp(stream, 400, "text/plain", "bad args\n", false);
                }
                args.push(s.to_string());
            }
            args
        }
        Some(_) => return write_resp(stream, 400, "text/plain", "bad args\n", false),
    };
    if !web_act_allowed(verb, &args) {
        return write_resp(stream, 404, "text/plain", "not found\n", false);
    }
    // drive and adopt can outlive the 5-second wait and would freeze the
    // single accept thread. 200 means the process group exists.
    if verb == "drive" || verb == "adopt" {
        return match spawn_detached(exe, cwd, verb, &args) {
            Ok(pid) => {
                let _ = writeln!(io::stderr(), "web act: verb={verb} pid={pid}");
                write_resp(stream, 200, "application/json", &pid_json(pid), false)
            }
            Err(()) => write_resp(stream, 500, "text/plain", "spawn failed\n", false),
        };
    }
    match spawn_read(exe, cwd, verb, &args) {
        Ok(output) => {
            let exit = match output.status.code() {
                Some(code) => code.to_string(),
                None => "signal".to_string(),
            };
            let mut log = io::stderr();
            let _ = writeln!(log, "web act: verb={verb} exit={exit}");
            let _ = log.write_all(&output.stderr);
            write_act_out(stream, &output.stdout, Some(&exit))
        }
        Err(ReadSpawn::Spawn) => {
            let _ = writeln!(io::stderr(), "web act: verb={verb} exit=spawn");
            write_resp(stream, 500, "text/plain", "spawn failed\n", false)
        }
        Err(ReadSpawn::Timeout) => {
            let _ = writeln!(io::stderr(), "web act: verb={verb} exit=timeout");
            write_resp(stream, 504, "text/plain", "act timed out\n", false)
        }
    }
}

fn go_body(body: &[u8]) -> Result<(), &'static str> {
    if body.is_empty() {
        return Ok(());
    }
    let v: serde_json::Value = serde_json::from_slice(body).map_err(|_| "not json\n")?;
    match v.as_object() {
        Some(map) if map.is_empty() => Ok(()),
        _ => Err("bad go\n"),
    }
}

struct Row {
    id: String,
    size: String,
    risk: String,
    idea_path: String,
    status: String,
}

fn row_from_json(v: &serde_json::Value) -> Result<Row, &'static str> {
    if !v.is_object() {
        return Err("bad backlog\n");
    }
    Ok(Row {
        id: tsv_field(v, "id")?,
        size: tsv_field(v, "size")?,
        risk: tsv_field(v, "risk")?,
        idea_path: tsv_field(v, "idea_path")?,
        status: tsv_field(v, "status")?,
    })
}

fn tsv_field(v: &serde_json::Value, key: &str) -> Result<String, &'static str> {
    let Some(s) = v.get(key).and_then(|x| x.as_str()) else {
        return Err("bad backlog\n");
    };
    if s.is_empty() || s.len() > FIELD_MAX || s.bytes().any(|b| matches!(b, b'\t' | b'\n' | b'\r'))
    {
        return Err("bad backlog\n");
    }
    Ok(s.to_string())
}

enum BacklogWrite {
    Bad,
    Io(String),
}

fn header_line_ok(bytes: &[u8]) -> bool {
    let mut line = bytes.split(|b| *b == b'\n').next().unwrap_or(b"");
    if let Some(stripped) = line.strip_suffix(b"\r") {
        line = stripped;
    }
    line == BACKLOG_HEADER.as_bytes()
}

fn append_backlog(cwd: &Path, row: &Row) -> Result<(), BacklogWrite> {
    let path = cwd.join("BACKLOG.tsv");
    let prev = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(BacklogWrite::Io(e.to_string())),
    };
    // A headerless first row would be skipped on GET. Refuse it instead of appending.
    if !prev.is_empty() && !header_line_ok(&prev) {
        return Err(BacklogWrite::Bad);
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| BacklogWrite::Io(e.to_string()))?;
    if prev.is_empty() {
        writeln!(f, "{BACKLOG_HEADER}").map_err(|e| BacklogWrite::Io(e.to_string()))?;
    } else if !prev.ends_with(b"\n") {
        writeln!(f).map_err(|e| BacklogWrite::Io(e.to_string()))?;
    }
    writeln!(
        f,
        "{}\t{}\t{}\t{}\t{}",
        row.id, row.size, row.risk, row.idea_path, row.status
    )
    .map_err(|e| BacklogWrite::Io(e.to_string()))
}

fn backlog_json(cwd: &Path) -> Result<String, String> {
    let rows = read_backlog(cwd)?;
    Ok(rows_json(&rows))
}

fn rows_json(rows: &[Row]) -> String {
    let mut out = String::from("{\"rows\":[");
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"id\":{},\"size\":{},\"risk\":{},\"idea_path\":{},\"status\":{}}}",
            json_str(&row.id),
            json_str(&row.size),
            json_str(&row.risk),
            json_str(&row.idea_path),
            json_str(&row.status),
        ));
    }
    out.push_str("]}");
    out
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn read_backlog(cwd: &Path) -> Result<Vec<Row>, String> {
    let text = match fs::read_to_string(cwd.join("BACKLOG.tsv")) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        // An id may start with '#'. A comment line has no tab.
        if line.trim().is_empty() || (line.starts_with('#') && !line.contains('\t')) {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 5 {
            return Err("bad backlog\n".to_string());
        }
        rows.push(Row {
            id: cols[0].to_string(),
            size: cols[1].to_string(),
            risk: cols[2].to_string(),
            idea_path: cols[3].to_string(),
            status: cols[4].to_string(),
        });
    }
    Ok(rows)
}

fn pid_json(pid: u32) -> String {
    format!("{{\"pid\":{pid}}}\n")
}

fn guided_checkout(cwd: &Path) -> bool {
    let Ok(rd) = fs::read_dir(cwd.join(".crucible")) else {
        return false;
    };
    for ent in rd.flatten() {
        let Ok(text) = fs::read_to_string(ent.path().join("PROGRAM")) else {
            continue;
        };
        if text.lines().any(|line| line.trim() == "cycle: guided") {
            return true;
        }
    }
    false
}

fn spawn_go(exe: &Path, cwd: &Path) -> Result<u32, ()> {
    spawn_detached(exe, cwd, "go", &[])
}

fn spawn_detached(exe: &Path, cwd: &Path, verb: &str, args: &[String]) -> Result<u32, ()> {
    let mut cmd = Command::new(exe);
    cmd.arg(verb)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // New group so `kill -TERM -<pid>` cannot signal this camera.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|_| ())?;
    let pid = child.id();
    // Reap off the request thread. Waiting here would hold the camera; dropping
    // the child would leave a zombie for the life of the camera.
    thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
    Ok(pid)
}

enum ReadSpawn {
    Spawn,
    Timeout,
}

fn spawn_read(
    exe: &Path,
    cwd: &Path,
    verb: &str,
    args: &[String],
) -> Result<std::process::Output, ReadSpawn> {
    let mut cmd = Command::new(exe);
    cmd.arg(verb)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // No process_group. drive and adopt detach in spawn_detached.
    let mut child = cmd.spawn().map_err(|_| ReadSpawn::Spawn)?;
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ReadSpawn::Spawn);
    };
    // Read pipes on threads: a full pipe deadlocks try_wait.
    let out_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });
    let started = std::time::Instant::now();
    let limit = Duration::from_secs(5);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < limit => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_thread.join();
                let _ = err_thread.join();
                return Err(ReadSpawn::Timeout);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out_thread.join();
                let _ = err_thread.join();
                return Err(ReadSpawn::Spawn);
            }
        }
    };
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

enum LenErr {
    Missing,
    Bad,
    TooLarge,
}

fn content_length(headers: &[(String, String)]) -> Result<usize, LenErr> {
    let vals: Vec<&str> = headers
        .iter()
        .filter(|(k, _)| k == "content-length")
        .map(|(_, v)| v.trim())
        .collect();
    if vals.is_empty() {
        return Err(LenErr::Missing);
    }
    let mut n: Option<u64> = None;
    for v in vals {
        if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
            return Err(LenErr::Bad);
        }
        let parsed: u64 = v.parse().map_err(|_| LenErr::Bad)?;
        if let Some(prev) = n {
            if prev != parsed {
                return Err(LenErr::Bad);
            }
        }
        n = Some(parsed);
    }
    let n = n.unwrap_or(0);
    if n > BODY_CAP as u64 {
        return Err(LenErr::TooLarge);
    }
    Ok(n as usize)
}

fn header_end(buf: &[u8]) -> Option<usize> {
    let crlf = buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);
    let lf = buf.windows(2).position(|w| w == b"\n\n").map(|i| i + 2);
    match (crlf, lf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn read_request(stream: &mut TcpStream) -> Result<Incoming, ReadErr> {
    let mut buf = Vec::new();
    let mut header_at: Option<usize> = None;
    let mut need: Option<usize> = None;
    loop {
        if let Some(total) = need {
            if buf.len() >= total {
                break;
            }
        } else if header_at.is_some() {
            break;
        }
        let mut tmp = [0u8; 1024];
        match stream.read(&mut tmp) {
            Ok(0) => {
                if let Some(total) = need {
                    if buf.len() < total {
                        return Err(ReadErr::Http(400, "short body\n"));
                    }
                    break;
                }
                if header_at.is_none() {
                    return Err(ReadErr::Http(400, "malformed request\n"));
                }
                break;
            }
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if header_at.is_none() {
                    if let Some(end) = header_end(&buf) {
                        let head = parse_head(&buf[..end])?;
                        header_at = Some(end);
                        if act_segment(&head.path).is_some() && head.method == "POST" {
                            need = Some(framed_total(&head.headers, end)?);
                        }
                        continue;
                    } else if buf.len() >= HEADER_CAP {
                        return Err(ReadErr::Http(400, "bad headers\n"));
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock => {
                if let Some(total) = need {
                    if buf.len() < total {
                        return Err(ReadErr::Http(400, "short body\n"));
                    }
                    break;
                }
                if header_at.is_some() {
                    break;
                }
                return Err(ReadErr::Io(e.to_string()));
            }
            Err(e) => return Err(ReadErr::Io(e.to_string())),
        }
    }
    let end = header_at.ok_or(ReadErr::Http(400, "malformed request\n"))?;
    let head = parse_head(&buf[..end])?;
    let body = if let Some(total) = need {
        if buf.len() < total {
            return Err(ReadErr::Http(400, "short body\n"));
        }
        buf[end..total].to_vec()
    } else {
        Vec::new()
    };
    Ok(Incoming {
        method: head.method,
        path: head.path,
        target: head.target,
        headers: head.headers,
        body,
    })
}

fn framed_total(headers: &[(String, String)], header_end: usize) -> Result<usize, ReadErr> {
    match content_length(headers) {
        Ok(n) => Ok(header_end + n),
        Err(LenErr::TooLarge) => Err(ReadErr::Http(413, "too large\n")),
        Err(LenErr::Missing | LenErr::Bad) => Err(ReadErr::Http(400, "content-length\n")),
    }
}

fn parse_head(head: &[u8]) -> Result<Head, ReadErr> {
    let text = String::from_utf8_lossy(head);
    let mut lines = text.split('\n').map(|l| l.trim_end_matches('\r'));
    let req = lines.next().unwrap_or("");
    let mut parts = req.split_whitespace();
    let Some(method) = parts.next() else {
        return Err(ReadErr::Http(400, "malformed request\n"));
    };
    let Some(target) = parts.next() else {
        return Err(ReadErr::Http(400, "malformed request\n"));
    };
    let path = target.split('?').next().unwrap_or("/").to_string();
    if path.is_empty() {
        return Err(ReadErr::Http(400, "malformed request\n"));
    }
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            return Err(ReadErr::Http(400, "malformed request\n"));
        };
        headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
    }
    Ok(Head {
        method: method.to_ascii_uppercase(),
        path,
        target: target.to_string(),
        headers,
    })
}

fn proxy_get(api: &str, path: &str) -> Result<String, String> {
    let mut last = String::new();
    for _ in 0..5 {
        let mut sock = match TcpStream::connect(api) {
            Ok(s) => s,
            Err(e) => {
                last = e.to_string();
                thread::sleep(Duration::from_millis(20));
                continue;
            }
        };
        let _ = sock.set_read_timeout(Some(Duration::from_secs(2)));
        if write!(
            sock,
            "GET {path} HTTP/1.1\r\nHost: {api}\r\nConnection: close\r\n\r\n"
        )
        .is_err()
        {
            continue;
        }
        let mut raw = Vec::new();
        if sock.read_to_end(&mut raw).is_err() {
            last = "read reset".to_string();
            continue;
        }
        let text = String::from_utf8_lossy(&raw);
        let body = text
            .split("\r\n\r\n")
            .nth(1)
            .or_else(|| text.split("\n\n").nth(1))
            .unwrap_or("")
            .to_string();
        if body.is_empty() {
            last = format!("empty GET {path}");
            continue;
        }
        return Ok(body);
    }
    Err(last)
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        504 => "Gateway Timeout",
        _ => "Error",
    }
}

fn write_resp(
    stream: &mut TcpStream,
    code: u16,
    ctype: &str,
    body: &str,
    head: bool,
) -> Result<(), String> {
    let body = if head { "" } else { body };
    let reason = reason(code);
    write!(
        stream,
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .map_err(|e| e.to_string())
}

fn write_act_out(
    stream: &mut TcpStream,
    body: &[u8],
    exit_header: Option<&str>,
) -> Result<(), String> {
    let mut head = format!(
        "HTTP/1.1 200 {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n",
        reason(200),
        body.len()
    );
    if let Some(code) = exit_header {
        head.push_str("X-Crucible-Exit: ");
        head.push_str(code);
        head.push_str("\r\n");
    }
    head.push_str("Connection: close\r\n\r\n");
    stream
        .write_all(head.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(body).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-web-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let spawned = self.root.join("SPAWNED");
            if let Ok(text) = fs::read_to_string(&spawned) {
                if let Ok(pid) = text.trim().parse::<u32>() {
                    let _ = Command::new("kill")
                        .args(["-TERM", &pid.to_string()])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
            }
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

    fn sleeper(dir: &Path) -> PathBuf {
        let path = dir.join("sleeper.sh");
        write_exec(
            &path,
            "#!/bin/sh\nprintf '%s\\n' \"$1\" > ARGV\nprintf '%s\\n' \"$*\" > ARGV_ALL\nprintf '%s\\n' \"$$\" > SPAWNED\nexec sleep 30\n",
        );
        path
    }

    fn start_server(cwd: &Path, exe: &Path, n: usize) -> String {
        let web = bind_web("127.0.0.1:0").unwrap();
        let addr = web.local_addr().unwrap().to_string();
        let cwd = cwd.to_path_buf();
        let exe = exe.to_path_buf();
        thread::spawn(move || {
            if let Err(e) = serve_n_at(web, "127.0.0.1:9", &cwd, &exe, n) {
                eprintln!("web test: {e}");
            }
        });
        addr
    }

    fn exchange(addr: &str, raw: &[u8]) -> (u16, String, String) {
        let mut last = String::new();
        for _ in 0..200 {
            let mut s = match TcpStream::connect(addr) {
                Ok(s) => s,
                Err(e) => {
                    last = e.to_string();
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
            };
            let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
            let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
            if s.write_all(raw).is_err() {
                continue;
            }
            let _ = s.shutdown(std::net::Shutdown::Write);
            let mut buf = Vec::new();
            match s.read_to_end(&mut buf) {
                Ok(_) => {}
                Err(_) if !buf.is_empty() => {}
                Err(e) => {
                    last = e.to_string();
                    continue;
                }
            }
            return split_resp(&buf);
        }
        (0, String::new(), last)
    }

    fn split_resp(buf: &[u8]) -> (u16, String, String) {
        let text = String::from_utf8_lossy(buf).into_owned();
        let code = text
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let (headers, body) = text.split_once("\r\n\r\n").unwrap_or((text.as_str(), ""));
        (code, headers.to_string(), body.to_string())
    }

    fn simple(method: &str, path: &str) -> Vec<u8> {
        format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .into_bytes()
    }

    fn act_request(path: &str, body: &str, content_type: &str, act: Option<&str>) -> Vec<u8> {
        let mut raw = format!(
            "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n",
            body.len()
        );
        if let Some(v) = act {
            raw.push_str(&format!("X-Crucible-Act: {v}\r\n"));
        }
        raw.push_str("Connection: close\r\n\r\n");
        raw.push_str(body);
        raw.into_bytes()
    }

    fn assert_no_cors(headers: &str) {
        assert!(
            !headers
                .to_ascii_lowercase()
                .contains("access-control-allow-origin"),
            "{headers}"
        );
    }

    fn marker(dir: &Path) -> bool {
        for _ in 0..5 {
            if dir.join("SPAWNED").exists() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn ready_backlog(dir: &Path) {
        fs::write(
            dir.join("BACKLOG.tsv"),
            "id\tsize\trisk\tidea_path\tstatus\ns1\tS\tLOW\tIDEA.md\tREADY\n",
        )
        .unwrap();
    }

    fn read_http(addr: &str, method: &str, path: &str) -> (u16, String) {
        let mut s = None;
        for _ in 0..200 {
            if let Ok(sock) = TcpStream::connect(addr) {
                s = Some(sock);
                break;
            }
            thread::sleep(std::time::Duration::from_millis(20));
        }
        let Some(mut s) = s else {
            return (0, "no connect".to_string());
        };
        let mut raw = Vec::new();
        for _ in 0..5 {
            raw.clear();
            if write!(
                s,
                "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
            )
            .is_err()
            {
                s = TcpStream::connect(addr).unwrap();
                continue;
            }
            match s.read_to_end(&mut raw) {
                Ok(_) => break,
                Err(_) => s = TcpStream::connect(addr).unwrap(),
            }
        }
        let text = String::from_utf8_lossy(&raw).into_owned();
        let code = text
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (code, body)
    }

    #[test]
    fn non_loopback_web_bind_does_not_listen() {
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let spec = format!("0.0.0.0:{port}");
        assert!(bind_web(&spec).is_err());
        assert!(TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            std::time::Duration::from_millis(150)
        )
        .is_err());
    }

    #[test]
    fn page_is_get_only_and_proxies_walk() {
        let api = TcpListener::bind("127.0.0.1:0").unwrap();
        let api_addr = api.local_addr().unwrap().to_string();
        thread::spawn(move || {
            for _ in 0..4 {
                let Ok((mut c, _)) = api.accept() else {
                    break;
                };
                let mut buf = Vec::new();
                let mut tmp = [0u8; 1024];
                loop {
                    match c.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&tmp[..n]);
                            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let req = String::from_utf8_lossy(&buf);
                let body = if req.contains("GET /walk") {
                    r#"{"card":"NEXT RED"}"#
                } else if req.contains("GET /health") {
                    r#"{"ok":true}"#
                } else if req.contains("GET /stats") {
                    r#"{"n":0}"#
                } else {
                    "no"
                };
                let _ = write!(
                    c,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let web = bind_web("127.0.0.1:0").unwrap();
        let web_addr = web.local_addr().unwrap().to_string();
        let errf = std::env::temp_dir().join(format!("crucible-web-test-{}", std::process::id()));
        let errf2 = errf.clone();
        let ready = errf.with_extension("ready");
        let ready2 = ready.clone();
        thread::spawn(move || {
            let _ = std::fs::write(&ready2, "up");
            if let Err(e) = serve_n(web, &api_addr, 8) {
                let _ = std::fs::write(&errf2, e.to_string());
            }
        });
        for _ in 0..200 {
            if ready.exists() {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
        thread::sleep(std::time::Duration::from_millis(30));
        let (code, body) = read_http(&web_addr, "GET", "/");
        assert_ne!(
            code,
            0,
            "web never answered: {}",
            std::fs::read_to_string(&errf).unwrap_or_default()
        );
        assert_eq!(code, 200);
        assert!(!body.contains("cannot start"));
        assert!(!body.contains("CHAT.md"));
        assert!(body.contains("<h2>Messages</h2>"));
        assert!(body.contains("/act/backlog"));
        assert!(body.contains("/act/message"));
        assert!(body.contains("/api/factory"));
        assert!(body.contains("<h2>Walk</h2>"));
        assert!(body.contains("<h2>Factory</h2>"));
        assert!(body.contains("/act/go"));
        assert!(body.contains("X-Crucible-Act"));
        assert!(body.contains("application/json"));
        assert!(body.contains("textContent"));
        assert!(body.contains(">Start<"));
        assert!(!body.contains("innerHTML"));
        assert!(!body.contains("const verbs"));
        assert!(!body.contains("POST /go"));
        assert!(body.contains(">Read<"));
        assert!(body.contains("id=\"read-args\""));
        assert!(body.contains("id=\"read\""));
        assert!(!body.contains("data-verb=\""));
        for verb in ["dispatch", "task", "run", "run-claim"] {
            assert!(
                !body.contains(&format!("data-verb=\"{verb}\"")),
                "{verb} must stay off the page"
            );
        }
        let (code, body) = read_http(&web_addr, "GET", "/api/walk");
        assert_eq!(code, 200, "{body}");
        assert!(body.contains("NEXT RED"), "{body}");
        let (code, body) = read_http(&web_addr, "POST", "/go");
        assert_eq!(code, 405, "{body}");
        assert!(body.contains("not a walk"));
    }

    fn chat_send_listener(page: &str) -> &str {
        let start = page
            .find("getElementById(\"chat-send\")")
            .expect("chat-send listener");
        let rest = &page[start..];
        let end = rest
            .find("\ndocument.getElementById(\"start\")")
            .expect("start listener follows chat-send");
        &rest[..end]
    }

    fn between<'a>(page: &'a str, start: &str, end: &str) -> &'a str {
        let at = page
            .find(start)
            .unwrap_or_else(|| panic!("missing {start}"));
        let rest = &page[at..];
        let stop = rest.find(end).unwrap_or_else(|| panic!("missing {end}"));
        &rest[..stop]
    }

    fn question_statuses(func: &str) -> Vec<&str> {
        let ret = func
            .find("return \"1 \" + id")
            .expect("question prefix is 1 plus the id");
        let gate_at = func[..ret].rfind("if (").expect("status gate");
        let mut gate = &func[gate_at..ret];
        let mut statuses = Vec::new();
        while let Some(at) = gate.find("status === \"") {
            let after = &gate[at + "status === \"".len()..];
            let end = after.find('"').expect("status literal");
            statuses.push(&after[..end]);
            gate = &after[end + 1..];
        }
        statuses
    }

    fn question_shown(func: &str, queue: &str) -> String {
        assert!(func.contains("line === \"\" || line === \"idle\""));
        assert!(func.contains("indexOf(\" \")"));
        assert!(func.contains("if (space < 0) continue;"));
        assert!(func.contains("line.slice(0, space)"));
        assert!(func.contains("line.slice(space + 1)"));
        assert!(func.contains("return \"\";"));
        assert!(!func.contains("fetch("), "question must not GET");
        assert!(!func.contains("\"2 \""));
        assert!(!func.contains("c-kind"));
        assert!(!func.contains("c-line"));
        assert!(!func.contains(".value"));
        let statuses = question_statuses(func);
        assert_eq!(statuses, ["paused", "escalated"]);
        for line in queue.split('\n') {
            if line.is_empty() || line == "idle" {
                continue;
            }
            let Some((id, status)) = line.split_once(' ') else {
                continue;
            };
            if statuses.contains(&status) {
                return format!("1 {id}");
            }
        }
        String::new()
    }

    const PAUSE_CLASS: &str = "Publish, delete, or leave this machine.";
    const OPTIONS_ESSAY: &str = "1. Publish. The work leaves this machine.\n2. Delete. The copy on this machine is removed.\n3. Leave. The work stays on this machine.\nRecommend 3. Leave keeps the work here.";

    fn question_display(func: &str, queue: &str) -> String {
        let q = question_shown(func, queue);
        if q.is_empty() {
            String::new()
        } else {
            format!("{q}\n{PAUSE_CLASS}\n{OPTIONS_ESSAY}")
        }
    }

    #[test]
    fn chat_send_loads_only_on_exit_zero() {
        let page = page_html();
        let listener = chat_send_listener(&page);
        assert!(listener.trim_end().ends_with("});"), "{listener}");
        assert!(!listener.contains("postAct"), "{listener}");
        assert_eq!(listener.matches("fetch(").count(), 1, "{listener}");
        assert_eq!(listener.matches("/act/message").count(), 1, "{listener}");
        assert!(listener.contains("method: \"POST\""));
        assert!(listener.contains("headers: actHeaders"));
        assert!(listener.contains(
            "args: [\"manager\", document.getElementById(\"c-kind\").value, document.getElementById(\"c-line\").value]"
        ));
        assert!(
            !listener.contains("\"source\""),
            "send posts the kind field, not a fixed source"
        );
        assert!(
            !listener.contains("\"answer\""),
            "send posts the kind field, not a fixed answer"
        );
        assert!(listener.contains("res.headers.get(\"X-Crucible-Exit\")"));
        assert!(!listener.contains(".value ="));
        for banned in ["setTimeout", "setInterval", "EventSource", "WebSocket"] {
            assert!(!listener.contains(banned), "{banned} in {listener}");
        }
        let zero_at = listener.find("if (exit === \"0\")").expect("exit-0 path");
        let after = &listener[zero_at..];
        let else_at = after.find("} else {").expect("non-zero path");
        let zero_arm = &after[..else_at];
        let else_rest = &after[else_at + "} else {".len()..];
        let else_end = else_rest.find('}').expect("end of non-zero path");
        let else_arm = &else_rest[..else_end];
        assert!(zero_arm.contains("el.textContent = text"), "{zero_arm}");
        assert!(zero_arm.contains("load("), "{zero_arm}");
        assert!(zero_arm.find("textContent").unwrap() < zero_arm.find("load(").unwrap());
        assert!(else_arm.contains("el.textContent = text"), "{else_arm}");
        assert!(!else_arm.contains("load("), "{else_arm}");
        assert!(!else_arm.contains("\"2\""), "{else_arm}");
        assert_eq!(listener.matches("load(").count(), 1, "{listener}");
        let outside = listener.replacen(zero_arm, "", 1);
        assert!(!outside.contains("load("), "{outside}");
    }

    const QUESTION_TEXT_FN: &str = "function questionText(queue) {\n  const q = questionLine(queue);\n  if (q === \"\") return \"\";\n  return q + \"\\nPublish, delete, or leave this machine.\\n1. Publish. The work leaves this machine.\\n2. Delete. The copy on this machine is removed.\\n3. Leave. The work stays on this machine.\\nRecommend 3. Leave keeps the work here.\";\n}\n";

    const FLOOR_LOAD_FOR: &str = r#"  for (const [id, path] of [["floor","/api/floor"],["health","/api/health"],["walk","/api/walk"],["factory","/api/factory"],["dashboard","/api/dashboard"],["stats","/api/stats?since=1h"],["backlog","/api/backlog"],["chat","/api/chat"]]) {
    const el = document.getElementById(id);
    try {
      const res = await fetch(path, { method: "GET" });
      el.textContent = await res.text();
    } catch (e) {
      el.textContent = String(e);
    }
    if (id === "factory") {
      document.getElementById("question").textContent = questionText(el.textContent);
    }
  }"#;

    #[test]
    fn question_is_filled_only_for_paused_or_escalated() {
        let page = page_html();
        assert!(page.contains("<pre id=\"factory\"></pre>\n<pre id=\"question\"></pre>\n"));
        assert!(page.contains("id=\"c-kind\" type=\"text\" value=\"source\""));
        assert_eq!(page.matches("/api/factory").count(), 1);
        assert!(!page.contains("/api/question"));
        assert!(!page.contains("innerHTML"));
        let func = between(&page, "function questionLine", "\nasync function load");
        assert!(!func.contains("Publish"));
        assert!(!func.contains("restore_all"));
        assert!(!func.contains("fetch("));
        assert!(!func.contains("merge"));
        assert!(!func.contains("herdr-init"));
        let load = between(
            &page,
            "async function load()",
            "\ndocument.getElementById(\"reload\")",
        );
        assert!(load.contains("[\"walk\",\"/api/walk\"]"));
        assert!(load.contains("[\"factory\",\"/api/factory\"]"));
        assert!(load.contains("[\"dashboard\",\"/api/dashboard\"]"));
        assert!(!load.contains("/api/queue"));
        assert!(load.contains("[\"chat\",\"/api/chat\"]"));
        assert!(load.contains("if (id === \"factory\")"));
        assert_eq!(load.matches("fetch(").count(), 1);
        let show = |queue: &str| question_shown(func, queue);
        assert_eq!(show(""), "");
        assert_eq!(show("idle"), "");
        assert_eq!(show("idle\n"), "");
        assert_eq!(show("\n\nidle\n"), "");
        assert_eq!(show("A waiting\n"), "");
        assert_eq!(show("A dispatched\n"), "");
        assert_eq!(show("A landed\n"), "");
        assert_eq!(show("A waiting\nB dispatched\nC landed\n"), "");
        assert_eq!(show("A paused extra\n"), "");
        assert_eq!(show("A paused"), "1 A");
        assert_eq!(show("A escalated\n"), "1 A");
        assert_eq!(show("A waiting\nB paused\nC escalated\n"), "1 B");
        assert_eq!(show("A escalated\nB paused\n"), "1 A");
        assert_eq!(show("idle\n\norder-9 escalated\n"), "1 order-9");
        assert_eq!(
            show("queue\nA dispatched\nB paused\ngraph\nA -\nB A\npaused\nB\nescalated\n"),
            "1 B"
        );
        assert_eq!(show("no-space\nD paused\n"), "1 D");
        assert!(!show("A paused\n").contains('2'));
        let display = |queue: &str| question_display(func, queue);
        for queue in [
            "",
            "idle",
            "A waiting\n",
            "door waiting\n",
            "A paused extra\n",
        ] {
            let got = display(queue);
            assert_eq!(got, "", "{queue:?} -> {got:?}");
            assert!(!got.contains("Publish"), "{queue:?}");
        }
        let paused = display("A paused");
        assert!(paused.starts_with("1 A"), "{paused}");
        assert_eq!(paused.matches(PAUSE_CLASS).count(), 1, "{paused}");
        assert_eq!(
            display("A escalated\n"),
            format!("1 A\n{PAUSE_CLASS}\n{OPTIONS_ESSAY}")
        );
        assert_eq!(
            display("A waiting\nB paused\nC escalated\n"),
            format!("1 B\n{PAUSE_CLASS}\n{OPTIONS_ESSAY}")
        );
        assert_eq!(
            display("queue\nA dispatched\nB paused\ngraph\nA -\nB A\npaused\nB\nescalated\n"),
            format!("1 B\n{PAUSE_CLASS}\n{OPTIONS_ESSAY}")
        );
        let listener = chat_send_listener(&page);
        assert!(!listener.contains(PAUSE_CLASS), "{listener}");
        assert!(!page.contains(&format!(">{PAUSE_CLASS}<")));
        for banned in ["setInterval", "setTimeout", "EventSource", "WebSocket"] {
            assert!(!page.contains(banned), "{banned}");
        }
        assert!(
            load.contains(
                "document.getElementById(\"question\").textContent = questionText(el.textContent)"
            ),
            "questionText is absent; load is:\n{load}"
        );
        let filled = load
            .find("el.textContent = await res.text()")
            .expect("factory fill");
        let asked = load
            .find("questionText(el.textContent)")
            .expect("question from factory text");
        assert!(filled < asked);
        assert!(!load.contains("questionLine(el.textContent)"), "{load}");
        assert!(
            load.contains("[\"floor\",\"/api/floor\"]"),
            "load does not fetch /api/floor"
        );
        assert_eq!(load.matches(FLOOR_LOAD_FOR).count(), 1, "{load}");
        assert_eq!(load.matches("questionText(el.textContent)").count(), 1);
        assert_eq!(page.matches("function questionText").count(), 1);
        assert_eq!(load.matches("function questionText").count(), 0);
        let qtext = between(&page, "function questionText", "function questionLine");
        assert_eq!(qtext, QUESTION_TEXT_FN);
        assert!(!qtext.contains("c-kind"), "{qtext}");
        assert!(!qtext.contains("c-line"), "{qtext}");
    }

    #[test]
    fn paused_line_shows_the_options_beside_the_pause_sentence() {
        let page = page_html();
        let qtext = between(&page, "function questionText", "function questionLine");
        let line = between(&page, "function questionLine", "\nasync function load");
        assert!(qtext.contains(PAUSE_CLASS), "{qtext}");
        assert!(
            qtext.contains(&OPTIONS_ESSAY.replace('\n', "\\n")),
            "{qtext}"
        );
        assert!(qtext.find(PAUSE_CLASS).unwrap() < qtext.find("1. Publish").unwrap());
        assert!(!line.contains("1. Publish"), "{line}");
        assert!(!line.contains("Recommend 3"), "{line}");
        assert!(!page.contains("<h2>Options</h2>"));
        assert!(!page.contains("/api/options"));
        let display = |queue: &str| question_display(line, queue);
        let paused = display("A paused");
        assert_eq!(paused, format!("1 A\n{PAUSE_CLASS}\n{OPTIONS_ESSAY}"));
        assert!(paused.find(PAUSE_CLASS).unwrap() < paused.find("1. Publish").unwrap());
        assert_eq!(
            paused
                .matches("1. Publish. The work leaves this machine.")
                .count(),
            1,
            "{paused}"
        );
        assert_eq!(
            paused
                .matches("2. Delete. The copy on this machine is removed.")
                .count(),
            1,
            "{paused}"
        );
        assert_eq!(
            paused
                .matches("3. Leave. The work stays on this machine.")
                .count(),
            1,
            "{paused}"
        );
        assert_eq!(
            paused
                .matches("Recommend 3. Leave keeps the work here.")
                .count(),
            1,
            "{paused}"
        );
        for queue in ["", "idle", "door waiting\n", "A waiting\n"] {
            let got = display(queue);
            assert_eq!(got, "", "{queue:?} -> {got:?}");
            assert!(!got.contains("1. Publish"), "{queue:?}");
        }
        let listener = chat_send_listener(&page);
        assert!(!listener.contains("1. Publish"), "{listener}");
        assert!(!listener.contains(PAUSE_CLASS), "{listener}");
    }

    #[test]
    fn backlog_get_does_not_create_and_head_is_empty() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 4);
        let (code, headers, body) = exchange(&addr, &simple("GET", "/api/backlog"));
        assert_eq!(code, 200, "{body}");
        assert_eq!(body, r#"{"rows":[]}"#);
        assert!(headers.to_ascii_lowercase().contains("application/json"));
        assert_no_cors(&headers);
        assert!(!tmp.root.join("BACKLOG.tsv").exists());
        let (code, headers, body) = exchange(&addr, &simple("HEAD", "/api/backlog"));
        assert_eq!(code, 200, "{headers}");
        assert!(body.is_empty(), "{body}");
        assert!(headers.to_ascii_lowercase().contains("content-length: 0"));
        assert!(!tmp.root.join("BACKLOG.tsv").exists());
        let good = "id\tsize\trisk\tidea_path\tstatus\n# note\n\ns1\tS\tLOW\tideas/a.md\tREADY\nt2\tM\tHIGH\tideas/b.md\tDONE\n";
        fs::write(tmp.root.join("BACKLOG.tsv"), good).unwrap();
        let (code, _, body) = exchange(&addr, &simple("GET", "/api/backlog"));
        assert_eq!(code, 200, "{body}");
        assert_eq!(
            body,
            r#"{"rows":[{"id":"s1","size":"S","risk":"LOW","idea_path":"ideas/a.md","status":"READY"},{"id":"t2","size":"M","risk":"HIGH","idea_path":"ideas/b.md","status":"DONE"}]}"#
        );
        assert_eq!(
            fs::read(tmp.root.join("BACKLOG.tsv")).unwrap(),
            good.as_bytes()
        );
        let bad = format!("{good}short\n");
        fs::write(tmp.root.join("BACKLOG.tsv"), &bad).unwrap();
        let (code, _, body) = exchange(&addr, &simple("GET", "/api/backlog"));
        assert_eq!(code, 500, "{body}");
        assert_eq!(body, "bad backlog\n");
        assert_eq!(
            fs::read(tmp.root.join("BACKLOG.tsv")).unwrap(),
            bad.as_bytes()
        );
        assert!(!marker(&tmp.root));
    }

    #[test]
    fn backlog_post_appends_one_row() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 10);
        let a = serde_json::json!({
            "id": "a",
            "size": "S",
            "risk": "LOW",
            "idea_path": "ideas/a.md",
            "status": "READY"
        })
        .to_string();
        let (code, headers, body) = exchange(
            &addr,
            &act_request("/act/backlog", &a, "application/json", Some("1")),
        );
        assert_eq!(code, 200, "{body}");
        assert_eq!(body, "{\"ok\":true}\n");
        assert!(headers.to_ascii_lowercase().contains("application/json"));
        let b = serde_json::json!({
            "id": "b",
            "size": "M",
            "risk": "HIGH",
            "idea_path": "ideas/b.md",
            "status": "DONE"
        })
        .to_string();
        let (code, _, body) = exchange(
            &addr,
            &act_request(
                "/act/backlog",
                &b,
                "application/json; charset=utf-8",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{body}");
        assert_eq!(body, "{\"ok\":true}\n");
        let wide = "x".repeat(256);
        let long_ok = serde_json::json!({
            "id": wide,
            "size": "S",
            "risk": "LOW",
            "idea_path": "ideas/a.md",
            "status": "READY"
        })
        .to_string();
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/backlog", &long_ok, "application/json", Some("1")),
        );
        assert_eq!(code, 200, "{body}");
        assert_eq!(
            fs::read_to_string(tmp.root.join("BACKLOG.tsv")).unwrap(),
            format!("id\tsize\trisk\tidea_path\tstatus\na\tS\tLOW\tideas/a.md\tREADY\nb\tM\tHIGH\tideas/b.md\tDONE\n{wide}\tS\tLOW\tideas/a.md\tREADY\n")
        );
        let (code, _, body) = exchange(&addr, &simple("GET", "/api/backlog"));
        assert_eq!(code, 200, "{body}");
        assert!(body.contains(
            r#"{"id":"a","size":"S","risk":"LOW","idea_path":"ideas/a.md","status":"READY"}"#
        ));
        assert!(!body.contains(r#""idea_path":"ideas/a.md","id":"#));
        let bad = serde_json::json!({
            "id": "a\tb",
            "size": "S",
            "risk": "LOW",
            "idea_path": "ideas/a.md",
            "status": "READY"
        })
        .to_string();
        let kept = fs::read(tmp.root.join("BACKLOG.tsv")).unwrap();
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/backlog", &bad, "application/json", Some("1")),
        );
        assert_eq!(code, 400);
        assert_eq!(body, "bad backlog\n");
        let empty = serde_json::json!({
            "id": "",
            "size": "S",
            "risk": "LOW",
            "idea_path": "ideas/a.md",
            "status": "READY"
        })
        .to_string();
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/backlog", &empty, "application/json", Some("1")),
        );
        assert_eq!(code, 400);
        assert_eq!(body, "bad backlog\n");
        let too_wide = serde_json::json!({
            "id": "y".repeat(257),
            "size": "S",
            "risk": "LOW",
            "idea_path": "ideas/a.md",
            "status": "READY"
        })
        .to_string();
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/backlog", &too_wide, "application/json", Some("1")),
        );
        assert_eq!(code, 400);
        assert_eq!(body, "bad backlog\n");
        assert_eq!(fs::read(tmp.root.join("BACKLOG.tsv")).unwrap(), kept);
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/backlog", "{", "application/json", Some("1")),
        );
        assert_eq!(code, 400);
        assert_eq!(body, "bad backlog\n");
        assert_eq!(fs::read(tmp.root.join("BACKLOG.tsv")).unwrap(), kept);
        fs::write(tmp.root.join("BACKLOG.tsv"), "not a header\n").unwrap();
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/backlog", &a, "application/json", Some("1")),
        );
        assert_eq!(code, 400);
        assert_eq!(body, "bad backlog\n");
        assert_eq!(
            fs::read(tmp.root.join("BACKLOG.tsv")).unwrap(),
            b"not a header\n"
        );
        let (code, _, body) = exchange(
            &addr,
            &act_request("/api/backlog", &a, "application/json", Some("1")),
        );
        assert_eq!(code, 405, "{body}");
        assert!(body.contains("not a walk"));
        assert!(!tmp.root.join(".wm").exists());
        assert!(!marker(&tmp.root));
    }

    #[test]
    fn chat_post_does_not_write_a_second_record() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        fs::create_dir_all(tmp.root.join(".wm")).unwrap();
        let trace = tmp.root.join(".wm").join("TRACE.tsv");
        fs::write(&trace, "when\tcard\toutcome\n").unwrap();
        let addr = start_server(&tmp.root, &exe, 2);
        let (code, _, body) = exchange(
            &addr,
            &act_request(
                "/act/chat",
                &serde_json::json!({"line": "hello"}).to_string(),
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 404, "{body}");
        assert_eq!(body, "not found\n");
        assert!(!tmp.root.join(".wm").join("CHAT.md").exists());
        assert!(!tmp.root.join("MESSAGES.tsv").exists());
        assert_eq!(fs::read_to_string(&trace).unwrap(), "when\tcard\toutcome\n");
        assert!(!marker(&tmp.root));
    }

    #[test]
    fn act_bodies_are_framed_by_content_length() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        ready_backlog(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 5);
        let missing = b"POST /act/go HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nX-Crucible-Act: 1\r\nConnection: close\r\n\r\n{}";
        let (code, _, _) = exchange(&addr, missing);
        assert_eq!(code, 400);
        let short = b"POST /act/go HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nX-Crucible-Act: 1\r\nContent-Length: 10\r\nConnection: close\r\n\r\n{}";
        let (code, _, _) = exchange(&addr, short);
        assert_eq!(code, 400);
        let huge = b"POST /act/go HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nX-Crucible-Act: 1\r\nContent-Length: 8193\r\nConnection: close\r\n\r\n";
        let (code, _, body) = exchange(&addr, huge);
        assert_eq!(code, 413, "{body}");
        let exact = " ".repeat(8192);
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/go", &exact, "application/json", Some("1")),
        );
        assert_eq!(code, 400);
        assert_eq!(body, "not json\n");
        let (code, _, body) = exchange(&addr, &vec![b'A'; HEADER_CAP]);
        assert_eq!(code, 400, "{body}");
        assert_eq!(body, "bad headers\n");
        assert!(!marker(&tmp.root));
        assert!(!tmp.root.join(".wm").exists());
    }

    #[test]
    fn form_post_and_missing_act_header_do_not_spawn() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        ready_backlog(&tmp.root);
        let before = fs::read(tmp.root.join("BACKLOG.tsv")).unwrap();
        let addr = start_server(&tmp.root, &exe, 5);
        let (code, _, body) = exchange(
            &addr,
            &act_request(
                "/act/go",
                "a=b",
                "application/x-www-form-urlencoded",
                Some("1"),
            ),
        );
        assert_eq!(code, 400);
        assert_eq!(body, "not json\n");
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/go", "{}", "application/json", None),
        );
        assert_eq!(code, 400);
        assert_eq!(body, "not an act\n");
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/go", "{}", "application/json", Some("2")),
        );
        assert_eq!(code, 400);
        assert_eq!(body, "not an act\n");
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/go", r#"{"x":1}"#, "application/json", Some("1")),
        );
        assert_eq!(code, 400, "{body}");
        assert_eq!(body, "bad go\n");
        let (code, headers, body) = exchange(&addr, &simple("HEAD", "/act/go"));
        assert_eq!(code, 405, "{body}");
        assert!(body.is_empty(), "{body}");
        assert_no_cors(&headers);
        assert!(!marker(&tmp.root));
        assert_eq!(fs::read(tmp.root.join("BACKLOG.tsv")).unwrap(), before);
    }

    #[test]
    fn act_go_is_409_when_intake_is_not_ready() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), " \n").unwrap();
        let addr = start_server(&tmp.root, &exe, 5);
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/go", "[]", "application/json", Some("1")),
        );
        assert_eq!(code, 400, "{body}");
        assert_eq!(body, "bad go\n");
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/go", r#"{"x":1}"#, "application/json", Some("1")),
        );
        assert_eq!(code, 400, "{body}");
        assert_eq!(body, "bad go\n");
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/go", "{", "application/json", Some("1")),
        );
        assert_eq!(code, 400, "{body}");
        assert_eq!(body, "not json\n");
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/go", "", "application/json", Some("1")),
        );
        assert_eq!(code, 409, "{body}");
        assert_eq!(body, "not ready\n");
        let (code, headers, body) = exchange(
            &addr,
            &act_request("/act/go", "{}", "application/json", Some("1")),
        );
        assert_eq!(code, 409, "{body}");
        assert_eq!(body, "not ready\n");
        assert_no_cors(&headers);
        assert!(!marker(&tmp.root));
        assert!(!tmp.root.join(".wm").exists());
        assert!(!tmp.root.join("CLOSED").exists());
    }

    #[test]
    fn post_go_stays_405_when_intake_is_ready() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        ready_backlog(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 2);
        let (code, _, body) = exchange(&addr, &simple("POST", "/go"));
        assert_eq!(code, 405, "{body}");
        assert_eq!(body, "POST is not a walk\n");
        let (code, headers, body) = exchange(&addr, &simple("OPTIONS", "/act/go"));
        assert_eq!(code, 405, "{body}");
        assert_no_cors(&headers);
        assert!(!marker(&tmp.root));
    }

    #[cfg(unix)]
    #[test]
    fn act_go_process_group_dies_and_web_process_does_not() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        ready_backlog(&tmp.root);
        let backlog = fs::read(tmp.root.join("BACKLOG.tsv")).unwrap();
        let addr = start_server(&tmp.root, &exe, 1);
        let (code, headers, body) = exchange(
            &addr,
            &act_request(
                "/act/go",
                "{}",
                "application/json; charset=utf-8",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert_no_cors(&headers);
        assert!(headers.to_ascii_lowercase().contains("application/json"));
        let v: serde_json::Value = serde_json::from_str(body.trim_end()).unwrap();
        let pid = v["pid"].as_u64().expect("pid number") as u32;
        assert_eq!(body, format!("{{\"pid\":{pid}}}\n"));
        assert!(v["pid"].as_str().is_none());
        let web_pid = std::process::id();
        assert_ne!(pid, web_pid);
        let mut spawned = None;
        for _ in 0..50 {
            if let Ok(text) = fs::read_to_string(tmp.root.join("SPAWNED")) {
                if let Ok(n) = text.trim().parse::<u32>() {
                    spawned = Some(n);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(spawned, Some(pid), "spawned pid must be the returned pgid");
        assert_eq!(
            fs::read_to_string(tmp.root.join("ARGV")).unwrap().trim(),
            "go"
        );
        let child = ps_fields(pid).expect("child still alive after 200");
        let me = ps_fields(web_pid).expect("web process");
        assert_eq!(child.0, pid, "process_group(0) makes the child the leader");
        assert_ne!(child.0, me.0, "group kill must not include the camera");
        assert!(
            matches!(child.1.as_bytes().first(), Some(b'S' | b'R' | b'I' | b'U')),
            "200 must not mean the walk finished: {}",
            child.1
        );
        let st = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status()
            .unwrap();
        assert!(st.success(), "kill -TERM -{pid}");
        let mut dead = false;
        for _ in 0..50 {
            match ps_fields(pid) {
                None => {
                    dead = true;
                    break;
                }
                Some((_, state)) if state.starts_with('Z') => {
                    dead = true;
                    break;
                }
                _ => thread::sleep(Duration::from_millis(20)),
            }
        }
        assert!(dead, "child still running after kill -TERM -{pid}");
        assert!(ps_fields(web_pid).is_some(), "web process gone");
        let alive = Command::new("kill")
            .args(["-0", &web_pid.to_string()])
            .status()
            .unwrap();
        assert!(alive.success(), "web process died with the child group");
        assert_eq!(fs::read(tmp.root.join("BACKLOG.tsv")).unwrap(), backlog);
        assert!(!tmp.root.join(".wm").join("TRACE.tsv").exists());
        assert!(!tmp.root.join(".wm").join("CLOSED").exists());
        assert!(!tmp.root.join("CLOSED").exists());
    }

    #[cfg(unix)]
    fn ps_fields(pid: u32) -> Option<(u32, String)> {
        let out = Command::new("ps")
            .args(["-o", "pgid=,state=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let line = text.lines().next()?.trim();
        if line.is_empty() {
            return None;
        }
        let mut parts = line.split_whitespace();
        let pgid: u32 = parts.next()?.parse().ok()?;
        let state = parts.next()?.to_string();
        Some((pgid, state))
    }

    #[test]
    fn web_manifest_does_not_depend_on_kernel() {
        let toml = include_str!("../Cargo.toml");
        assert!(!toml.contains("crucible-kernel"));
        let prod = include_str!("lib.rs").split("#[cfg(test)]").next().unwrap();
        assert!(!prod.contains("crucible_kernel"));
        assert!(!prod.contains("ChildGuard"));
        assert!(!prod.contains("Access-Control-Allow-Origin"));
        assert!(prod.contains("process_group(0)"));
        assert!(prod.contains("not a walk"));
        let read_fn = prod
            .split_once("fn spawn_read")
            .expect("spawn_read")
            .1
            .split_once("\nfn ")
            .expect("fn after spawn_read")
            .0;
        assert!(
            !read_fn.contains("process_group("),
            "spawn_read must not call process_group"
        );
        let detached = prod
            .split_once("fn spawn_detached")
            .expect("spawn_detached")
            .1
            .split_once("\nfn ")
            .expect("fn after spawn_detached")
            .0;
        assert!(
            detached.contains("process_group(0)"),
            "process_group(0) stays inside spawn_detached"
        );
    }

    fn recorder(dir: &Path) -> PathBuf {
        let path = dir.join("recorder.sh");
        write_exec(
            &path,
            "#!/bin/sh\nprintf 'stdout:%s\\n' \"$1\"\nprintf '%s\\n' \"$*\" > ARGV_ALL\nexit 0\n",
        );
        path
    }

    fn argv_all(dir: &Path) -> Option<String> {
        fs::read_to_string(dir.join("ARGV_ALL")).ok()
    }

    #[test]
    fn web_read_only_is_exactly_the_non_writing_slice() {
        assert_eq!(
            WEB_READ_ONLY,
            &["agents", "check", "debrief", "next", "panes", "stats", "triage", "workid",][..]
        );
        assert!(web_act_allowed("debrief", &[]));
        assert!(web_act_allowed(
            "stats",
            &["--since".into(), "24h".into(), "--json".into()]
        ));
        assert!(web_act_allowed("status", &["--json".into()]));
        assert!(web_act_allowed("status", &[]));
        assert!(!web_act_allowed("status", &["--json".into(), "x".into()]));
        assert!(!web_act_allowed("status", &["".into()]));
        assert!(!web_act_allowed("status", &["--json".into(), "".into()]));
        assert_eq!(
            WEB_WRITERS,
            &[
                "add",
                "adopt",
                "attempt",
                "brief",
                "claim",
                "close",
                "contract-audit",
                "cycle",
                "drive",
                "evidence",
                "lifecycle",
                "message",
                "phase",
                "plan-audit",
                "probe-acp",
                "ready",
                "result",
                "state",
                "status",
                "target",
            ][..]
        );
        let acts = web_page_acts();
        for verb in [
            "check",
            "triage",
            "add",
            "attempt",
            "claim",
            "contract-audit",
            "cycle",
            "phase",
            "plan-audit",
            "probe-acp",
            "ready",
        ] {
            let args = acts.iter().find(|act| act.verb == verb).expect(verb).args;
            assert!(args.is_empty(), "{verb} button args {args:?}");
        }
        for verb in ["check", "triage"] {
            assert!(WEB_READ_ONLY.contains(&verb), "{verb}");
            assert!(!WEB_WRITERS.contains(&verb), "{verb}");
            assert!(web_act_allowed(verb, &[]), "{verb}");
        }
        for verb in ["cycle", "claim"] {
            assert!(WEB_WRITERS.contains(&verb), "{verb}");
            assert!(!WEB_READ_ONLY.contains(&verb), "{verb}");
            assert!(web_act_allowed(verb, &[]), "{verb}");
        }
        for verb in ["state", "target", "brief", "lifecycle", "evidence"] {
            assert!(!WEB_READ_ONLY.contains(&verb), "{verb}");
            assert!(web_act_allowed(verb, &[]), "{verb}");
        }
        assert!(web_act_allowed(
            "evidence",
            &["archive".into(), "slug".into()]
        ));
        assert!(web_act_allowed("lifecycle", &["status".into()]));
        assert!(web_act_allowed(
            "lifecycle",
            &["enable".into(), "--apply".into()]
        ));
        for verb in ["close", "drive", "adopt"] {
            assert!(web_act_allowed(verb, &[]), "{verb}");
        }
        assert!(result_page_args(&[
            "A1700000000.4.1".into(),
            "PASS".into(),
            "check.txt".into(),
            "CLOSE".into(),
        ]));
        assert!(!result_page_args(&[]));
        assert!(!result_page_args(&[
            "A1".into(),
            "PASS".into(),
            "a/b".into(),
            "CLOSE".into(),
        ]));
        assert!(!result_page_args(&[
            "sh".into(),
            "-c".into(),
            "echo".into(),
            "x".into(),
        ]));
        for verb in ["go", "dispatch", "task", "run", "run-claim"] {
            assert!(!WEB_READ_ONLY.contains(&verb), "{verb}");
            assert!(!WEB_WRITERS.contains(&verb), "{verb}");
            assert!(!web_act_allowed(verb, &[]), "{verb}");
            assert!(!web_act_allowed(verb, &["status".into()]), "{verb}");
            assert!(
                !web_act_allowed(verb, &["slug".into(), "maker".into(), "x".into()]),
                "{verb}"
            );
        }
    }

    #[test]
    fn station_verbs_off_the_page_do_not_spawn() {
        let tmp = Tmp::new();
        let exe = recorder(&tmp.root);
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(&tmp.root)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{args:?} {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).unwrap()
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        git(&[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-qm",
            "base",
        ]);
        let branches = git(&["branch", "--list"]);
        let refs = git(&["for-each-ref", "--format=%(refname)"]);
        let forbidden = ["dispatch", "task", "run", "run-claim"];
        let bodies = [r#"{"args":[]}"#, r#"{"args":["slug","maker","x"]}"#];
        let waited = [
            "check",
            "triage",
            "add",
            "attempt",
            "claim",
            "contract-audit",
            "cycle",
            "phase",
            "plan-audit",
            "probe-acp",
            "ready",
        ];
        let addr = start_server(
            &tmp.root,
            &exe,
            forbidden.len() * bodies.len() + waited.len() + 2,
        );
        for verb in forbidden {
            for body in bodies {
                let (code, _, resp) = exchange(
                    &addr,
                    &act_request(&format!("/act/{verb}"), body, "application/json", Some("1")),
                );
                assert_eq!(code, 404, "{verb} {body} -> {resp}");
                assert_eq!(resp, "not found\n");
                assert!(argv_all(&tmp.root).is_none(), "{verb} spawned");
                assert!(!tmp.root.join("SPAWNED").exists(), "{verb} spawned");
            }
        }
        assert!(!tmp.root.join("worktrees").exists());
        assert!(!tmp.root.join("attempts").exists());
        assert!(!tmp.root.join("items").exists());
        assert!(!tmp.root.join(".git").join("worktrees").exists());
        assert_eq!(git(&["branch", "--list"]), branches);
        assert_eq!(git(&["for-each-ref", "--format=%(refname)"]), refs);
        for verb in waited {
            let (code, headers, body) = exchange(
                &addr,
                &act_request(
                    &format!("/act/{verb}"),
                    r#"{"args":[]}"#,
                    "application/json",
                    Some("1"),
                ),
            );
            assert_eq!(code, 200, "{verb} {headers} {body}");
            assert_eq!(body, format!("stdout:{verb}\n"));
            assert!(headers.contains("X-Crucible-Exit: 0"), "{verb} {headers}");
            assert!(
                !body.starts_with('{'),
                "{verb} must not be pid JSON: {body}"
            );
            assert_eq!(argv_all(&tmp.root).unwrap().trim(), verb);
            fs::remove_file(tmp.root.join("ARGV_ALL")).unwrap();
        }
        let (code, _, resp) = exchange(
            &addr,
            &act_request(
                "/act/result",
                r#"{"args":["sh","-c","echo","x"]}"#,
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 404, "{resp}");
        assert!(argv_all(&tmp.root).is_none());
        let (code, headers, body) = exchange(
            &addr,
            &act_request(
                "/act/result",
                r#"{"args":["A1700000000.4.1","PASS","check.txt","CLOSE"]}"#,
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert_eq!(body, "stdout:result\n");
        assert_eq!(
            argv_all(&tmp.root).unwrap().trim(),
            "result A1700000000.4.1 PASS check.txt CLOSE"
        );
    }

    #[test]
    fn act_debrief_reaches_exe_and_returns_stdout() {
        let tmp = Tmp::new();
        let exe = recorder(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 1);
        let (code, headers, body) = exchange(
            &addr,
            &act_request(
                "/act/debrief",
                r#"{"args":[]}"#,
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert_eq!(body, "stdout:debrief\n");
        assert!(headers.contains("X-Crucible-Exit: 0"), "{headers}");
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("content-type: text/plain; charset=utf-8"),
            "{headers}"
        );
        assert_no_cors(&headers);
        assert_eq!(argv_all(&tmp.root).unwrap().trim(), "debrief");
    }

    #[test]
    fn act_status_without_json_is_404() {
        let tmp = Tmp::new();
        let exe = recorder(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 6);
        for body in [
            r#"{"args":["--json","x"]}"#,
            r#"{"args":[""]}"#,
            r#"{"args":["--json",""]}"#,
        ] {
            let (code, _, resp) = exchange(
                &addr,
                &act_request("/act/status", body, "application/json", Some("1")),
            );
            assert_eq!(code, 404, "{body} -> {resp}");
            assert_eq!(resp, "not found\n");
            assert!(argv_all(&tmp.root).is_none(), "{body} spawned");
        }
        for body in [r#"{}"#, r#"{"args":[]}"#] {
            let (code, headers, resp) = exchange(
                &addr,
                &act_request("/act/status", body, "application/json", Some("1")),
            );
            assert_eq!(code, 200, "{body} -> {headers} {resp}");
            assert_eq!(resp, "stdout:status\n");
            assert!(headers.contains("X-Crucible-Exit: 0"), "{headers}");
            assert_eq!(argv_all(&tmp.root).unwrap().trim(), "status");
            fs::remove_file(tmp.root.join("ARGV_ALL")).unwrap();
        }
        let (code, headers, body) = exchange(
            &addr,
            &act_request(
                "/act/status",
                r#"{"args":["--json"]}"#,
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert_eq!(body, "stdout:status\n");
        assert!(headers.contains("X-Crucible-Exit: 0"), "{headers}");
        assert_eq!(argv_all(&tmp.root).unwrap().trim(), "status --json");
    }

    #[test]
    fn act_close_is_404() {
        let tmp = Tmp::new();
        let exe = recorder(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 3);
        let (code, headers, body) = exchange(
            &addr,
            &act_request(
                "/act/close",
                r#"{"args":[]}"#,
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert_eq!(body, "stdout:close\n");
        assert!(headers.contains("X-Crucible-Exit: 0"), "{headers}");
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("content-type: text/plain; charset=utf-8"),
            "{headers}"
        );
        assert_no_cors(&headers);
        assert_eq!(argv_all(&tmp.root).unwrap().trim(), "close");
        fs::remove_file(tmp.root.join("ARGV_ALL")).unwrap();
        let (code, headers, body) = exchange(&addr, &simple("GET", "/act/close"));
        assert_eq!(code, 404, "{body}");
        assert_eq!(body, "not found\n");
        assert_no_cors(&headers);
        assert!(argv_all(&tmp.root).is_none());
        let (code, _, body) = exchange(&addr, &simple("HEAD", "/act/close"));
        assert_eq!(code, 404, "{body}");
        assert!(argv_all(&tmp.root).is_none());
    }

    #[test]
    fn act_state_target_brief_lifecycle_are_waited() {
        let tmp = Tmp::new();
        let exe = recorder(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 8);
        for verb in ["state", "target", "brief"] {
            let (code, headers, body) = exchange(
                &addr,
                &act_request(
                    &format!("/act/{verb}"),
                    r#"{"args":[]}"#,
                    "application/json",
                    Some("1"),
                ),
            );
            assert_eq!(code, 200, "{verb} {headers} {body}");
            assert_eq!(body, format!("stdout:{verb}\n"));
            assert!(headers.contains("X-Crucible-Exit: 0"), "{verb} {headers}");
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("content-type: text/plain"),
                "{verb} {headers}"
            );
            assert!(
                !body.starts_with('{'),
                "{verb} must not be pid JSON: {body}"
            );
            assert_eq!(argv_all(&tmp.root).unwrap().trim(), verb);
            fs::remove_file(tmp.root.join("ARGV_ALL")).unwrap();
        }
        let (code, headers, body) = exchange(
            &addr,
            &act_request(
                "/act/lifecycle",
                r#"{"args":["status"]}"#,
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert_eq!(body, "stdout:lifecycle\n");
        assert!(headers.contains("X-Crucible-Exit: 0"), "{headers}");
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("content-type: text/plain"),
            "{headers}"
        );
        assert!(
            !body.starts_with('{'),
            "lifecycle must not be pid JSON: {body}"
        );
        assert_eq!(argv_all(&tmp.root).unwrap().trim(), "lifecycle status");
        fs::remove_file(tmp.root.join("ARGV_ALL")).unwrap();
        let (code, headers, body) = exchange(
            &addr,
            &act_request(
                "/act/evidence",
                r#"{"args":[]}"#,
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert_eq!(body, "stdout:evidence\n");
        assert!(headers.contains("X-Crucible-Exit: 0"), "{headers}");
        assert!(
            !body.starts_with('{'),
            "evidence must not be pid JSON: {body}"
        );
        assert_eq!(argv_all(&tmp.root).unwrap().trim(), "evidence");
        assert!(!tmp.root.join("history").exists());
        fs::remove_file(tmp.root.join("ARGV_ALL")).unwrap();
        for verb in ["run", "run-claim"] {
            let (code, _, resp) = exchange(
                &addr,
                &act_request(
                    &format!("/act/{verb}"),
                    r#"{"args":[]}"#,
                    "application/json",
                    Some("1"),
                ),
            );
            assert_eq!(code, 404, "{verb} -> {resp}");
            assert_eq!(resp, "not found\n");
            assert!(argv_all(&tmp.root).is_none(), "{verb} spawned");
        }
    }

    #[cfg(unix)]
    fn assert_detached_sleeper(path: &str, json: &str, argv: &str) {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 1);
        let started = std::time::Instant::now();
        let (code, headers, body) = exchange(
            &addr,
            &act_request(path, json, "application/json", Some("1")),
        );
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "{path} waited {:?}",
            started.elapsed()
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert!(!headers.contains("X-Crucible-Exit"), "{headers}");
        assert!(
            headers.to_ascii_lowercase().contains("application/json"),
            "{headers}"
        );
        assert_no_cors(&headers);
        let v: serde_json::Value = serde_json::from_str(body.trim_end()).unwrap();
        let pid = v["pid"].as_u64().expect("pid number") as u32;
        assert_eq!(body, format!("{{\"pid\":{pid}}}\n"));
        assert!(v["pid"].as_str().is_none());
        let web_pid = std::process::id();
        assert_ne!(pid, web_pid);
        let mut recorded = None;
        for _ in 0..50 {
            if let Ok(text) = fs::read_to_string(tmp.root.join("ARGV_ALL")) {
                if !text.trim().is_empty() {
                    recorded = Some(text);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(recorded.expect("argv").trim(), argv);
        let mut spawned = None;
        for _ in 0..50 {
            if let Ok(text) = fs::read_to_string(tmp.root.join("SPAWNED")) {
                if let Ok(n) = text.trim().parse::<u32>() {
                    spawned = Some(n);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(spawned, Some(pid), "spawned pid must be the returned pgid");
        let child = ps_fields(pid).expect("child still alive after 200");
        let me = ps_fields(web_pid).expect("web process");
        assert_eq!(child.0, pid, "process_group(0) makes the child the leader");
        assert_ne!(child.0, me.0, "group kill must not include the camera");
        assert!(
            matches!(child.1.as_bytes().first(), Some(b'S' | b'R' | b'I' | b'U')),
            "200 must not mean the child finished: {}",
            child.1
        );
        let st = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status()
            .unwrap();
        assert!(st.success(), "kill -TERM -{pid}");
        let mut dead = false;
        for _ in 0..50 {
            match ps_fields(pid) {
                None => {
                    dead = true;
                    break;
                }
                Some((_, state)) if state.starts_with('Z') => {
                    dead = true;
                    break;
                }
                _ => thread::sleep(Duration::from_millis(20)),
            }
        }
        assert!(dead, "child still running after kill -TERM -{pid}");
        assert!(ps_fields(web_pid).is_some(), "web process gone");
        let alive = Command::new("kill")
            .args(["-0", &web_pid.to_string()])
            .status()
            .unwrap();
        assert!(alive.success(), "web process died with the child group");
        assert!(!tmp.root.join(".crucible").exists(), "must not copy a tree");
    }

    #[cfg(unix)]
    #[test]
    fn act_drive_detaches_and_stays_alive() {
        assert_detached_sleeper("/act/drive", r#"{"args":[]}"#, "drive");
    }

    #[cfg(unix)]
    #[test]
    fn act_adopt_detaches_and_does_not_copy() {
        assert_detached_sleeper("/act/adopt", r#"{"args":["--managed"]}"#, "adopt --managed");
    }

    #[test]
    fn post_go_on_web_stays_405_in_the_read_only_test() {
        let tmp = Tmp::new();
        let exe = recorder(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 1);
        let (code, _, body) = exchange(&addr, &simple("POST", "/go"));
        assert_eq!(code, 405, "{body}");
        assert_eq!(body, "POST is not a walk\n");
        assert!(argv_all(&tmp.root).is_none());
    }

    #[test]
    fn act_debrief_bad_headers_do_not_spawn() {
        let tmp = Tmp::new();
        let exe = recorder(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 4);
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/debrief", r#"{"args":[]}"#, "application/json", None),
        );
        assert_eq!(code, 400, "{body}");
        assert_eq!(body, "not an act\n");
        let (code, _, body) = exchange(
            &addr,
            &act_request(
                "/act/debrief",
                "a=b",
                "application/x-www-form-urlencoded",
                Some("1"),
            ),
        );
        assert_eq!(code, 400, "{body}");
        assert_eq!(body, "not json\n");
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/close", r#"{"args":[]}"#, "application/json", None),
        );
        assert_eq!(code, 400, "{body}");
        assert_eq!(body, "not an act\n");
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/status", "{}", "application/json", None),
        );
        assert_eq!(code, 400, "{body}");
        assert_eq!(body, "not an act\n");
        assert!(argv_all(&tmp.root).is_none());
    }

    #[test]
    fn act_debrief_bad_args_do_not_spawn() {
        let tmp = Tmp::new();
        let exe = recorder(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 6);
        let long = format!(r#"{{"args":["{}"]}}"#, "a".repeat(257));
        let mut many = String::from(r#"{"args":["#);
        for i in 0..33 {
            if i > 0 {
                many.push(',');
            }
            many.push_str(&format!("\"a{i}\""));
        }
        many.push_str("]}");
        for body in [
            r#"{"x":1}"#.to_string(),
            r#"{"args":[1]}"#.to_string(),
            "[]".to_string(),
            r#"{"args":["--json"],"x":1}"#.to_string(),
            long,
            many,
        ] {
            let (code, _, resp) = exchange(
                &addr,
                &act_request("/act/debrief", &body, "application/json", Some("1")),
            );
            assert_eq!(code, 400, "{body} -> {resp}");
            assert_eq!(resp, "bad args\n");
            assert!(argv_all(&tmp.root).is_none(), "{body} spawned");
        }
    }

    #[test]
    fn act_nonzero_exit_body_is_stdout_only() {
        let tmp = Tmp::new();
        let exe = tmp.root.join("out.sh");
        write_exec(
            &exe,
            "#!/bin/sh\nprintf 'out\\n'\nprintf 'err\\n' >&2\nexit 2\n",
        );
        let addr = start_server(&tmp.root, &exe, 1);
        let (code, headers, body) = exchange(
            &addr,
            &act_request(
                "/act/debrief",
                r#"{"args":[]}"#,
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert_eq!(body, "out\n");
        assert!(!body.contains("err"));
        assert!(headers.contains("X-Crucible-Exit: 2"), "{headers}");
        assert_no_cors(&headers);
    }

    #[test]
    fn act_empty_stdout_is_empty_body() {
        let tmp = Tmp::new();
        let exe = tmp.root.join("err.sh");
        write_exec(&exe, "#!/bin/sh\nprintf 'err\\n' >&2\nexit 2\n");
        let addr = start_server(&tmp.root, &exe, 1);
        let (code, headers, body) = exchange(
            &addr,
            &act_request("/act/next", "{}", "application/json", Some("1")),
        );
        assert_eq!(code, 200, "{headers} {body}");
        assert!(body.is_empty(), "{body}");
        assert!(
            headers.to_ascii_lowercase().contains("content-length: 0"),
            "{headers}"
        );
        assert!(headers.contains("X-Crucible-Exit: 2"), "{headers}");
        assert!(!body.contains("err"));
    }

    #[test]
    fn act_stdout_bytes_are_not_lossy_decoded() {
        let tmp = Tmp::new();
        let exe = tmp.root.join("ff.sh");
        write_exec(&exe, "#!/bin/sh\nprintf '\\377'\nexit 0\n");
        let addr = start_server(&tmp.root, &exe, 1);
        let raw = act_request(
            "/act/debrief",
            r#"{"args":[]}"#,
            "application/json",
            Some("1"),
        );
        let mut buf = Vec::new();
        for _ in 0..200 {
            let mut s = match TcpStream::connect(&addr) {
                Ok(s) => s,
                Err(_) => {
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
            };
            let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
            if s.write_all(&raw).is_err() {
                continue;
            }
            let _ = s.shutdown(std::net::Shutdown::Write);
            buf.clear();
            match s.read_to_end(&mut buf) {
                Ok(_) => {}
                Err(_) if !buf.is_empty() => {}
                Err(_) => continue,
            }
            break;
        }
        assert!(!buf.is_empty(), "no response");
        let split = buf
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("headers");
        let head = String::from_utf8_lossy(&buf[..split]);
        let body = &buf[split + 4..];
        assert!(head.starts_with("HTTP/1.1 200 "), "{head}");
        assert!(head.contains("X-Crucible-Exit: 0"), "{head}");
        assert_eq!(body, &[0xff]);
        assert!(!body.windows(3).any(|w| w == [0xef, 0xbf, 0xbd]));
    }

    const FLOOR_BOARD: &str = "\
station: BUILD
card: NEXT RED
wip: slice-1
andon: -
independence: SUBAGENT-ISOLATED
elapsed: 12
evidence:
  .wm/FALSIFIER
  reviews/review.md
";

    fn write_floor(root: &Path, bytes: &[u8]) -> PathBuf {
        let wm = root.join(".wm");
        fs::create_dir_all(&wm).unwrap();
        let path = wm.join("FLOOR.md");
        fs::write(&path, bytes).unwrap();
        path
    }

    fn wm_entries(root: &Path) -> Vec<String> {
        let mut names = Vec::new();
        for ent in fs::read_dir(root.join(".wm")).unwrap() {
            names.push(ent.unwrap().file_name().to_string_lossy().into_owned());
        }
        names.sort();
        names
    }

    fn assert_wm_quiet(root: &Path) {
        let wm = root.join(".wm");
        assert!(!wm.join("TRACE.tsv").exists());
        assert!(!wm.join("EVENTS").exists());
        assert!(!marker(root));
    }

    #[test]
    fn floor_pre_sits_between_walk_and_factory() {
        let page = page_html();
        assert!(page.contains("<pre id=\"walk\"></pre>"));
        assert!(page.contains("<pre id=\"factory\"></pre>\n<pre id=\"question\"></pre>\n"));
        assert!(!page.contains("<pre id=\"walk\">station:"));
        assert!(!page.contains("<pre id=\"factory\">station:"));
        assert!(
            page.contains("<h2>Walk</h2><pre id=\"walk\"></pre>\n<h2>Floor</h2><pre id=\"floor\"></pre>\n<h2>Factory</h2><pre id=\"factory\"></pre>\n<pre id=\"question\"></pre>\n"),
            "Floor pre is absent"
        );
    }

    #[test]
    fn dashboard_pre_follows_the_factory() {
        let page = page_html();
        assert!(page.contains(
            "<pre id=\"question\"></pre>\n<h2>Dashboard</h2><pre id=\"dashboard\"></pre>\n"
        ));
        assert_eq!(page.matches("/api/dashboard").count(), 1);
        assert!(!page.contains("/api/queue"));
    }

    #[test]
    fn dashboard_get_returns_the_served_directory() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        fs::write(tmp.root.join("IDEA.md"), "a small idea\n").unwrap();
        fs::create_dir_all(tmp.root.join(".git")).unwrap();
        fs::write(
            tmp.root.join(".git").join("HEAD"),
            "ref: refs/heads/dash-proof\n",
        )
        .unwrap();
        fs::write(tmp.root.join("ORDERS.tsv"), "secret-order\n").unwrap();
        let addr = start_server(&tmp.root, &exe, 3);
        let (code, _, body) = exchange(
            &addr,
            &act_request("/api/dashboard", "{\"n\":1}", "application/json", None),
        );
        assert_eq!(code, 405, "{body}");
        assert_eq!(body, "POST is not a walk\n");
        assert_eq!(
            fs::read_to_string(tmp.root.join("ORDERS.tsv")).unwrap(),
            "secret-order\n"
        );
        let (code, headers, body) = exchange(&addr, &simple("GET", "/api/dashboard"));
        assert_eq!(code, 200, "{body}");
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("text/plain; charset=utf-8"),
            "{headers}"
        );
        assert_eq!(body, crucible_guided::dashboard::dashboard(&tmp.root));
        assert!(body.contains("idea: a small idea\n"), "{body}");
        assert!(body.contains("branch dash-proof\n"), "{body}");
        assert!(!body.contains("secret-order"), "{body}");
        assert!(!tmp.root.join("SPAWNED").exists());
        let (code, _, body) = exchange(&addr, &simple("HEAD", "/api/dashboard"));
        assert_eq!(code, 200, "{body}");
        assert!(body.is_empty(), "{body}");
    }

    #[test]
    fn floor_get_returns_the_planted_file() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 4);
        let path = write_floor(&tmp.root, FLOOR_BOARD.as_bytes());
        let (code, _, body) = exchange(
            &addr,
            &act_request("/api/floor", "{\"n\":1}", "application/json", None),
        );
        assert_eq!(fs::read(&path).unwrap(), FLOOR_BOARD.as_bytes());
        assert!(!marker(&tmp.root));
        assert_eq!(body, "POST is not a walk\n");
        assert_eq!(code, 405, "{body}");
        let (walk_code, _, walk_body) = exchange(&addr, &simple("GET", "/api/walk"));
        assert_ne!(walk_body, FLOOR_BOARD);
        assert!(
            walk_code != 200 || !walk_body.contains("station: BUILD"),
            "{walk_code} {walk_body}"
        );
        let (code, headers, body) = exchange(&addr, &simple("GET", "/api/floor"));
        assert_eq!(fs::read(&path).unwrap(), FLOOR_BOARD.as_bytes());
        assert_eq!(wm_entries(&tmp.root), vec!["FLOOR.md".to_string()]);
        assert_wm_quiet(&tmp.root);
        assert_no_cors(&headers);
        assert_eq!(code, 200, "{body}");
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("text/plain; charset=utf-8"),
            "{headers}"
        );
        assert_eq!(body, FLOOR_BOARD);
        let (code, headers, body) = exchange(&addr, &simple("HEAD", "/api/floor"));
        assert_eq!(fs::read(&path).unwrap(), FLOOR_BOARD.as_bytes());
        assert_wm_quiet(&tmp.root);
        assert_eq!(code, 200, "{headers}");
        assert!(body.is_empty(), "{body}");
        assert!(
            headers.to_ascii_lowercase().contains("content-length: 0"),
            "{headers}"
        );
    }

    #[test]
    fn floor_missing_is_empty_and_creates_nothing() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 1);
        let (code, headers, body) = exchange(&addr, &simple("GET", "/api/floor"));
        assert!(!tmp.root.join(".wm").exists());
        assert!(!marker(&tmp.root));
        assert_no_cors(&headers);
        assert_eq!(code, 200, "{body}");
        assert_eq!(body, "");
    }

    #[test]
    fn floor_over_cap_is_too_large() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 1);
        let bytes = vec![b'x'; 262_144 + 1];
        let path = write_floor(&tmp.root, &bytes);
        let (code, _, body) = exchange(&addr, &simple("GET", "/api/floor"));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_wm_quiet(&tmp.root);
        assert_eq!(code, 500, "{body}");
        assert_eq!(body, "floor too large\n");
    }

    #[test]
    fn floor_non_utf8_is_unreadable() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 1);
        let bytes = [0xff, 0xfe];
        let path = write_floor(&tmp.root, &bytes);
        let (code, _, body) = exchange(&addr, &simple("GET", "/api/floor"));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_wm_quiet(&tmp.root);
        assert_eq!(code, 500, "{body}");
        assert_eq!(body, "floor unreadable\n");
    }

    // One connect and one read. A retry would hide a handler blocked in File::open.
    #[cfg(unix)]
    fn floor_once(addr: &str) -> (u16, String, String) {
        let mut stream =
            TcpStream::connect(addr).unwrap_or_else(|err| panic!("connect {addr}: {err}"));
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream.write_all(&simple("GET", "/api/floor")).unwrap();
        let _ = stream.shutdown(std::net::Shutdown::Write);
        let mut buf = Vec::new();
        let read = stream.read_to_end(&mut buf);
        assert!(!buf.is_empty(), "empty buffer: {read:?}");
        split_resp(&buf)
    }

    #[cfg(unix)]
    fn floor_request(root: &Path) -> (u16, String) {
        let exe = sleeper(root);
        let addr = start_server(root, &exe, 1);
        let (code, _, body) = floor_once(&addr);
        (code, body)
    }

    #[cfg(unix)]
    #[test]
    fn non_regular_floor_does_not_block() {
        use std::os::unix::fs::FileTypeExt;

        // FIFO first. This client does not open the write end.
        {
            let tmp = Tmp::new();
            let wm = tmp.root.join(".wm");
            fs::create_dir_all(&wm).unwrap();
            let path = wm.join("FLOOR.md");
            let status = Command::new("mkfifo")
                .arg(&path)
                .status()
                .unwrap_or_else(|err| panic!("mkfifo: {err}"));
            assert!(status.success(), "mkfifo: {status}");
            let (code, body) = floor_request(&tmp.root);
            assert!(fs::metadata(&path).unwrap().file_type().is_fifo());
            assert_eq!(wm_entries(&tmp.root), vec!["FLOOR.md".to_string()]);
            assert_wm_quiet(&tmp.root);
            assert_eq!(code, 500, "{body}");
            assert_eq!(body, "floor unreadable\n");
        }
        {
            let tmp = Tmp::new();
            let wm = tmp.root.join(".wm");
            fs::create_dir_all(&wm).unwrap();
            let fifo = tmp.root.join("held.fifo");
            let status = Command::new("mkfifo").arg(&fifo).status().unwrap();
            assert!(status.success(), "{status}");
            let abs = fifo.canonicalize().unwrap();
            let path = wm.join("FLOOR.md");
            std::os::unix::fs::symlink(&abs, &path).unwrap();
            assert!(fs::metadata(&path).unwrap().file_type().is_fifo());
            let (code, body) = floor_request(&tmp.root);
            assert!(fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink());
            assert!(fs::metadata(&path).unwrap().file_type().is_fifo());
            assert!(fs::metadata(&abs).unwrap().file_type().is_fifo());
            assert_wm_quiet(&tmp.root);
            assert_eq!(code, 500, "{body}");
            assert_eq!(body, "floor unreadable\n");
        }
        {
            let tmp = Tmp::new();
            let wm = tmp.root.join(".wm");
            fs::create_dir_all(&wm).unwrap();
            let path = wm.join("FLOOR.md");
            let missing = tmp
                .root
                .canonicalize()
                .unwrap()
                .join("missing-floor-target");
            std::os::unix::fs::symlink(&missing, &path).unwrap();
            assert_eq!(fs::metadata(&path).unwrap_err().kind(), ErrorKind::NotFound);
            assert!(fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink());
            let (code, body) = floor_request(&tmp.root);
            assert!(fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink());
            assert_eq!(wm_entries(&tmp.root), vec!["FLOOR.md".to_string()]);
            assert_wm_quiet(&tmp.root);
            assert_eq!(code, 500, "{body}");
            assert_eq!(body, "floor unreadable\n");
        }
        {
            let tmp = Tmp::new();
            let wm = tmp.root.join(".wm");
            fs::create_dir_all(&wm).unwrap();
            let path = wm.join("FLOOR.md");
            fs::create_dir(&path).unwrap();
            let (code, body) = floor_request(&tmp.root);
            assert!(path.is_dir());
            assert!(fs::read_dir(&path).unwrap().next().is_none());
            assert_wm_quiet(&tmp.root);
            assert_eq!(code, 500, "{body}");
            assert_eq!(body, "floor unreadable\n");
        }
        {
            let tmp = Tmp::new();
            let wm = tmp.root.join(".wm");
            fs::create_dir_all(&wm).unwrap();
            let path = wm.join("FLOOR.md");
            let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
            let (code, body) = floor_request(&tmp.root);
            assert!(fs::symlink_metadata(&path).unwrap().file_type().is_socket());
            assert_wm_quiet(&tmp.root);
            assert_eq!(code, 500, "{body}");
            assert_eq!(body, "floor unreadable\n");
            drop(listener);
        }
        {
            let tmp = Tmp::new();
            let wm = tmp.root.join(".wm");
            fs::create_dir_all(&wm).unwrap();
            let path = wm.join("FLOOR.md");
            std::os::unix::fs::symlink("/dev/null", &path).unwrap();
            assert!(!fs::metadata(&path).unwrap().is_file());
            let (code, body) = floor_request(&tmp.root);
            assert!(fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink());
            assert!(!fs::metadata(&path).unwrap().is_file());
            assert_wm_quiet(&tmp.root);
            assert_eq!(code, 500, "{body}");
            assert_eq!(body, "floor unreadable\n");
        }
        {
            let tmp = Tmp::new();
            let wm = tmp.root.join(".wm");
            fs::create_dir_all(&wm).unwrap();
            let board = tmp.root.join("seven.md");
            fs::write(&board, FLOOR_BOARD).unwrap();
            let abs = board.canonicalize().unwrap();
            let path = wm.join("FLOOR.md");
            std::os::unix::fs::symlink(&abs, &path).unwrap();
            assert!(fs::metadata(&path).unwrap().is_file());
            let (code, body) = floor_request(&tmp.root);
            assert_eq!(fs::read(&abs).unwrap(), FLOOR_BOARD.as_bytes());
            assert_wm_quiet(&tmp.root);
            assert_eq!(code, 200, "{body}");
            assert_eq!(body, FLOOR_BOARD);
        }
    }

    fn this_file_source() -> String {
        let raw = Path::new(file!());
        if let Ok(text) = fs::read_to_string(raw) {
            return text;
        }
        // file!() is workspace-relative when paths are trimmed. The test cwd is the package.
        let mut dir = std::env::current_dir().expect("cwd");
        loop {
            if let Ok(text) = fs::read_to_string(dir.join(raw)) {
                return text;
            }
            if !dir.pop() {
                break;
            }
        }
        panic!("cannot read {}", raw.display());
    }

    #[test]
    fn floor_reader_source_is_bounded() {
        let src = this_file_source();
        let name = ["fn ", "read", "_floor"].concat();
        let start = src
            .find(&name)
            .unwrap_or_else(|| panic!("floor reader is absent"));
        let rest = &src[start..];
        let next = rest.find("\nfn ").unwrap_or_else(|| panic!("next fn"));
        let slice = &rest[..next];
        assert_eq!(
            slice.matches("file.take((FLOOR_CAP as u64) + 1)").count(),
            1,
            "{slice}"
        );
        assert_eq!(
            slice.matches("limited.read_to_end(&mut bytes)").count(),
            1,
            "{slice}"
        );
        assert_eq!(
            slice.matches("bytes.len() > FLOOR_CAP").count(),
            1,
            "{slice}"
        );
        assert_eq!(slice.matches("File::open").count(), 1, "{slice}");
        assert_eq!(slice.matches("read_to_end").count(), 1, "{slice}");
        assert_eq!(slice.matches(".len()").count(), 1, "{slice}");
        assert!(slice.contains("bytes.len()"), "{slice}");
        assert!(slice.contains("symlink_metadata"), "{slice}");
        assert!(!slice.contains("OpenOptions"), "{slice}");
        assert!(!slice.contains("fs::read"), "{slice}");
    }
}
