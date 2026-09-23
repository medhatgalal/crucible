//! Loopback web camera. It serves a page and proxies GET `/walk`, `/stats`, and
//! `/health`. It appends `BACKLOG.tsv` and `.wm/CHAT.md`. `POST /act/go` spawns
//! `go` in a new process group and does not walk. `POST /go` is not a walk.

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
const CHAT_LINE_CAP: usize = 4096;
const NOT_A_WALK: &str = "POST is not a walk\n";

const PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Crucible</title>
<style>
body { font: 16px/1.4 ui-sans-serif, system-ui, sans-serif; margin: 2rem; }
pre { background: #f4f4f4; padding: 1rem; overflow: auto; }
</style>
</head>
<body>
<h1>Crucible</h1>
<p>This page only reads the walk. It cannot start one.</p>
<p>Backlog: <a href="/api/backlog">/api/backlog</a>. Chat: <a href="/api/chat">/api/chat</a> (plain text).</p>
<button id="reload" type="button">Reload</button>
<h2>Health</h2><pre id="health"></pre>
<h2>Walk</h2><pre id="walk"></pre>
<h2>Stats</h2><pre id="stats"></pre>
<script>
async function load() {
  for (const [id, path] of [["health","/api/health"],["walk","/api/walk"],["stats","/api/stats?since=1h"]]) {
    const el = document.getElementById(id);
    try {
      const res = await fetch(path, { method: "GET" });
      el.textContent = await res.text();
    } catch (e) {
      el.textContent = String(e);
    }
  }
}
document.getElementById("reload").addEventListener("click", load);
load();
</script>
</body>
</html>
"#;

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
    if req.method != "GET" && req.method != "HEAD" {
        return write_resp(stream, 405, "text/plain", NOT_A_WALK, false);
    }
    if req.path == "/" || req.path == "/index.html" {
        return write_resp(stream, 200, "text/html; charset=utf-8", PAGE, head);
    }
    if req.path == "/api/backlog" {
        match backlog_json(cwd) {
            Ok(body) => return write_resp(stream, 200, "application/json", &body, head),
            Err(e) => return write_resp(stream, 500, "text/plain", &format!("{e}\n"), head),
        }
    }
    if req.path == "/api/chat" {
        match chat_text(cwd) {
            Ok(body) => return write_resp(stream, 200, "text/plain; charset=utf-8", &body, head),
            Err(e) => return write_resp(stream, 500, "text/plain", &format!("{e}\n"), head),
        }
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

fn is_act(path: &str) -> bool {
    matches!(path, "/act/backlog" | "/act/chat" | "/act/go")
}

fn act(stream: &mut TcpStream, cwd: &Path, exe: &Path, req: &Incoming) -> Result<(), String> {
    if let Err(msg) = act_headers(&req.headers) {
        return write_resp(stream, 400, "text/plain", msg, false);
    }
    match req.path.as_str() {
        "/act/backlog" => post_backlog(stream, cwd, &req.body),
        "/act/chat" => post_chat(stream, cwd, &req.body),
        "/act/go" => post_go(stream, cwd, exe, &req.body),
        _ => write_resp(stream, 405, "text/plain", NOT_A_WALK, false),
    }
}

fn act_headers(headers: &[(String, String)]) -> Result<(), &'static str> {
    let ct = header(headers, "content-type").unwrap_or("");
    if !json_content_type(ct) {
        return Err("content-type\n");
    }
    match header(headers, "x-crucible-act") {
        Some(v) if v.trim() == "1" => Ok(()),
        _ => Err("X-Crucible-Act\n"),
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
        Err(_) => return write_resp(stream, 400, "text/plain", "json\n", false),
    };
    let row = match row_from_json(&v) {
        Ok(row) => row,
        Err(msg) => return write_resp(stream, 400, "text/plain", msg, false),
    };
    if let Err(e) = append_backlog(cwd, &row) {
        return write_resp(stream, 500, "text/plain", &format!("{e}\n"), false);
    }
    write_resp(stream, 200, "text/plain", "ok\n", false)
}

fn post_chat(stream: &mut TcpStream, cwd: &Path, body: &[u8]) -> Result<(), String> {
    let v = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(v) => v,
        Err(_) => return write_resp(stream, 400, "text/plain", "json\n", false),
    };
    let line = match chat_line(&v) {
        Ok(line) => line,
        Err(msg) => return write_resp(stream, 400, "text/plain", msg, false),
    };
    if let Err(e) = append_chat(cwd, &line) {
        return write_resp(stream, 500, "text/plain", &format!("{e}\n"), false);
    }
    write_resp(stream, 200, "text/plain", "ok\n", false)
}

fn post_go(stream: &mut TcpStream, cwd: &Path, exe: &Path, body: &[u8]) -> Result<(), String> {
    if !body.is_empty() && serde_json::from_slice::<serde_json::Value>(body).is_err() {
        return write_resp(stream, 400, "text/plain", "json\n", false);
    }
    // 200 means the process group exists, not that the walk succeeded.
    if !crucible_contract::intake_ready(cwd) {
        return write_resp(stream, 409, "text/plain", "intake not ready\n", false);
    }
    match spawn_go(exe, cwd) {
        Ok(pid) => write_resp(stream, 200, "application/json", &pid_json(pid), false),
        Err(e) => write_resp(stream, 500, "text/plain", &format!("{e}\n"), false),
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
        return Err("json\n");
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
        return Err("bad field\n");
    };
    if s.bytes().any(|b| matches!(b, b'\t' | b'\n' | b'\r')) {
        return Err("bad field\n");
    }
    Ok(s.to_string())
}

fn chat_line(v: &serde_json::Value) -> Result<String, &'static str> {
    if !v.is_object() {
        return Err("json\n");
    }
    let Some(s) = v.get("line").and_then(|x| x.as_str()) else {
        return Err("bad field\n");
    };
    if s.len() > CHAT_LINE_CAP || s.bytes().any(|b| matches!(b, b'\n' | b'\r')) {
        return Err("line\n");
    }
    Ok(s.to_string())
}

fn append_backlog(cwd: &Path, row: &Row) -> Result<(), String> {
    let path = cwd.join("BACKLOG.tsv");
    let prev = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    if prev.is_empty() {
        writeln!(f, "id\tsize\trisk\tidea_path\tstatus").map_err(|e| e.to_string())?;
    } else if !prev.ends_with(b"\n") {
        writeln!(f).map_err(|e| e.to_string())?;
    }
    writeln!(
        f,
        "{}\t{}\t{}\t{}\t{}",
        row.id, row.size, row.risk, row.idea_path, row.status
    )
    .map_err(|e| e.to_string())
}

fn append_chat(cwd: &Path, line: &str) -> Result<(), String> {
    let dir = cwd.join(".wm");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("CHAT.md");
    let prev = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    if !prev.is_empty() && !prev.ends_with(b"\n") {
        writeln!(f).map_err(|e| e.to_string())?;
    }
    writeln!(f, "{line}").map_err(|e| e.to_string())
}

fn backlog_json(cwd: &Path) -> Result<String, String> {
    let rows = read_backlog(cwd)?;
    let body = serde_json::json!({ "rows": rows });
    crucible_contract::canonical_json(&body).map_err(|e| e.to_string())
}

fn read_backlog(cwd: &Path) -> Result<Vec<serde_json::Value>, String> {
    let text = match fs::read_to_string(cwd.join("BACKLOG.tsv")) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 5 {
            continue;
        }
        rows.push(serde_json::json!({
            "id": cols[0],
            "size": cols[1],
            "risk": cols[2],
            "idea_path": cols[3],
            "status": cols[4],
        }));
    }
    Ok(rows)
}

fn chat_text(cwd: &Path) -> Result<String, String> {
    match fs::read_to_string(cwd.join(".wm").join("CHAT.md")) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

fn pid_json(pid: u32) -> String {
    let v = serde_json::json!({ "pid": pid });
    crucible_contract::canonical_json(&v).unwrap_or_else(|_| format!("{{\"pid\":{pid}}}"))
}

fn spawn_go(exe: &Path, cwd: &Path) -> Result<u32, String> {
    let mut cmd = Command::new(exe);
    cmd.arg("go")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // New group so `kill -TERM -<pid>` cannot signal this camera.
    // Dropping the child neither waits nor signals; waiting would be the walk.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    drop(child);
    Ok(pid)
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
                        if is_act(&head.path) && head.method == "POST" {
                            need = Some(framed_total(&head.headers, end)?);
                        }
                        continue;
                    } else if buf.len() >= HEADER_CAP {
                        return Err(ReadErr::Http(400, "malformed request\n"));
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
            "#!/bin/sh\nprintf '%s\\n' \"$1\" > ARGV\nprintf '%s\\n' \"$$\" > SPAWNED\nexec sleep 30\n",
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
        assert!(body.contains("cannot start"));
        assert!(body.contains("/api/backlog"));
        assert!(body.contains("/api/chat"));
        assert!(!body.contains("innerHTML"));
        assert!(!body.contains("POST /go"));
        let (code, body) = read_http(&web_addr, "GET", "/api/walk");
        assert_eq!(code, 200, "{body}");
        assert!(body.contains("NEXT RED"), "{body}");
        let (code, body) = read_http(&web_addr, "POST", "/go");
        assert_eq!(code, 405, "{body}");
        assert!(body.contains("not a walk"));
    }

    #[test]
    fn backlog_get_does_not_create_and_head_is_empty() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 3);
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
        fs::write(
            tmp.root.join("BACKLOG.tsv"),
            "id\tsize\trisk\tidea_path\tstatus\n# note\n\ns1\tS\tLOW\tideas/a.md\tREADY\nt2\tM\tHIGH\tideas/b.md\tDONE\nshort\n",
        )
        .unwrap();
        let before = fs::read(tmp.root.join("BACKLOG.tsv")).unwrap();
        let (code, _, body) = exchange(&addr, &simple("GET", "/api/backlog"));
        assert_eq!(code, 200, "{body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "s1");
        assert_eq!(rows[0]["size"], "S");
        assert_eq!(rows[0]["risk"], "LOW");
        assert_eq!(rows[0]["idea_path"], "ideas/a.md");
        assert_eq!(rows[0]["status"], "READY");
        assert_eq!(rows[1]["status"], "DONE");
        assert_eq!(fs::read(tmp.root.join("BACKLOG.tsv")).unwrap(), before);
        assert!(!marker(&tmp.root));
    }

    #[test]
    fn backlog_post_appends_one_row() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 5);
        let a = serde_json::json!({
            "id": "a",
            "size": "S",
            "risk": "LOW",
            "idea_path": "ideas/a.md",
            "status": "READY"
        })
        .to_string();
        let (code, _, body) = exchange(
            &addr,
            &act_request("/act/backlog", &a, "application/json", Some("1")),
        );
        assert_eq!(code, 200, "{body}");
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
        assert_eq!(
            fs::read_to_string(tmp.root.join("BACKLOG.tsv")).unwrap(),
            "id\tsize\trisk\tidea_path\tstatus\na\tS\tLOW\tideas/a.md\tREADY\nb\tM\tHIGH\tideas/b.md\tDONE\n"
        );
        let (code, _, body) = exchange(&addr, &simple("GET", "/api/backlog"));
        assert_eq!(code, 200, "{body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["rows"].as_array().unwrap().len(), 2);
        let bad = serde_json::json!({
            "id": "a\tb",
            "size": "S",
            "risk": "LOW",
            "idea_path": "ideas/a.md",
            "status": "READY"
        })
        .to_string();
        let (code, _, _) = exchange(
            &addr,
            &act_request("/act/backlog", &bad, "application/json", Some("1")),
        );
        assert_eq!(code, 400);
        let (code, _, body) = exchange(
            &addr,
            &act_request("/api/backlog", &a, "application/json", Some("1")),
        );
        assert_eq!(code, 405, "{body}");
        assert!(body.contains("not a walk"));
        assert_eq!(
            fs::read_to_string(tmp.root.join("BACKLOG.tsv"))
                .unwrap()
                .lines()
                .count(),
            3
        );
        assert!(!tmp.root.join(".wm").exists());
        assert!(!marker(&tmp.root));
    }

    #[test]
    fn chat_appends_one_line_and_does_not_touch_trace() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        fs::create_dir_all(tmp.root.join(".wm")).unwrap();
        let trace = tmp.root.join(".wm").join("TRACE.tsv");
        fs::write(&trace, "when\tcard\toutcome\n").unwrap();
        let addr = start_server(&tmp.root, &exe, 6);
        let (code, _, body) = exchange(&addr, &simple("GET", "/api/chat"));
        assert_eq!(code, 200, "{body}");
        assert_eq!(body, "");
        assert!(!tmp.root.join(".wm").join("CHAT.md").exists());
        let (code, _, body) = exchange(
            &addr,
            &act_request(
                "/act/chat",
                &serde_json::json!({"line": "hello"}).to_string(),
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 200, "{body}");
        let (code, _, _) = exchange(
            &addr,
            &act_request(
                "/act/chat",
                &serde_json::json!({"line": "second"}).to_string(),
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 200);
        assert_eq!(
            fs::read_to_string(tmp.root.join(".wm").join("CHAT.md")).unwrap(),
            "hello\nsecond\n"
        );
        let (code, _, body) = exchange(&addr, &simple("GET", "/api/chat"));
        assert_eq!(body, "hello\nsecond\n");
        assert_eq!(code, 200);
        let (code, _, body) = exchange(&addr, &simple("HEAD", "/api/chat"));
        assert_eq!(code, 200, "{body}");
        assert!(body.is_empty());
        let (code, _, _) = exchange(
            &addr,
            &act_request(
                "/act/chat",
                &serde_json::json!({"line": "a\nb"}).to_string(),
                "application/json",
                Some("1"),
            ),
        );
        assert_eq!(code, 400);
        assert_eq!(
            fs::read_to_string(tmp.root.join(".wm").join("CHAT.md")).unwrap(),
            "hello\nsecond\n"
        );
        assert_eq!(fs::read_to_string(&trace).unwrap(), "when\tcard\toutcome\n");
        assert!(!tmp.root.join(".wm").join("EVENTS").exists());
        assert!(!tmp.root.join(".wm").join("CLOSED").exists());
        assert!(!tmp.root.join(".wm").join("FLOOR.md").exists());
        assert!(!marker(&tmp.root));
    }

    #[test]
    fn chat_line_cap_is_the_decoded_line() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 2);
        let too_long = serde_json::json!({ "line": "a".repeat(4097) }).to_string();
        assert!(too_long.len() <= 8192);
        let (code, _, _) = exchange(
            &addr,
            &act_request("/act/chat", &too_long, "application/json", Some("1")),
        );
        assert_eq!(code, 400);
        assert!(!tmp.root.join(".wm").join("CHAT.md").exists());
        let ok = serde_json::json!({ "line": "b".repeat(4096) }).to_string();
        let (code, _, _) = exchange(
            &addr,
            &act_request("/act/chat", &ok, "application/json", Some("1")),
        );
        assert_eq!(code, 200);
        let text = fs::read_to_string(tmp.root.join(".wm").join("CHAT.md")).unwrap();
        assert_eq!(text, format!("{}\n", "b".repeat(4096)));
        assert!(!marker(&tmp.root));
    }

    #[test]
    fn act_bodies_are_framed_by_content_length() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        ready_backlog(&tmp.root);
        let addr = start_server(&tmp.root, &exe, 4);
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
        let (code, _, _) = exchange(
            &addr,
            &act_request("/act/go", &exact, "application/json", Some("1")),
        );
        assert_eq!(code, 400);
        assert!(!marker(&tmp.root));
        assert!(!tmp.root.join(".wm").exists());
    }

    #[test]
    fn form_post_and_missing_act_header_do_not_spawn() {
        let tmp = Tmp::new();
        let exe = sleeper(&tmp.root);
        ready_backlog(&tmp.root);
        let before = fs::read(tmp.root.join("BACKLOG.tsv")).unwrap();
        let addr = start_server(&tmp.root, &exe, 4);
        let (code, _, _) = exchange(
            &addr,
            &act_request(
                "/act/go",
                "a=b",
                "application/x-www-form-urlencoded",
                Some("1"),
            ),
        );
        assert_eq!(code, 400);
        let (code, _, _) = exchange(
            &addr,
            &act_request("/act/go", "{}", "application/json", None),
        );
        assert_eq!(code, 400);
        let (code, _, _) = exchange(
            &addr,
            &act_request("/act/go", "{}", "application/json", Some("2")),
        );
        assert_eq!(code, 400);
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
        let addr = start_server(&tmp.root, &exe, 1);
        let (code, headers, body) = exchange(
            &addr,
            &act_request("/act/go", "{}", "application/json", Some("1")),
        );
        assert_eq!(code, 409, "{body}");
        assert!(body.contains("intake"), "{body}");
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
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let pid = v["pid"].as_u64().expect("pid number") as u32;
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
    }
}
