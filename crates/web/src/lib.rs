//! Loopback web camera. It serves a page and proxies GET `/walk`, `/stats`, and
//! `/health` from `crucible serve`. It does not start a walk and it does not
//! write FLOOR or TRACE.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use crucible_http::ServeError;

pub const DEFAULT_WEB_BIND: &str = "127.0.0.1:1735";
pub const DEFAULT_API_BIND: &str = "127.0.0.1:1734";

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
    crucible_http::parse_bind(api)?;
    for conn in listener.incoming() {
        let mut stream = conn.map_err(|e| ServeError::Io(e.to_string()))?;
        if let Err(e) = handle_one(&mut stream, api) {
            let _ = writeln!(io::stderr(), "web: {e}");
        }
    }
    Ok(())
}

pub fn serve_n(listener: TcpListener, api: &str, n: usize) -> Result<(), ServeError> {
    crucible_http::parse_bind(api)?;
    for _ in 0..n {
        let mut stream = listener
            .accept()
            .map_err(|e| ServeError::Io(e.to_string()))?
            .0;
        if let Err(e) = handle_one(&mut stream, api) {
            let _ = writeln!(io::stderr(), "web: {e}");
        }
    }
    Ok(())
}

fn handle_one(stream: &mut TcpStream, api: &str) -> Result<(), String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let line = req.lines().next().unwrap_or("");
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        return write_resp(stream, 405, "text/plain", "POST is not a walk\n");
    }
    let path = target.split('?').next().unwrap_or("/");
    if path == "/" || path == "/index.html" {
        if method == "HEAD" {
            return write_resp(stream, 200, "text/html; charset=utf-8", "");
        }
        return write_resp(stream, 200, "text/html; charset=utf-8", PAGE);
    }
    let upstream = match path {
        "/api/health" => "/health",
        "/api/walk" => "/walk",
        "/api/stats" => {
            if target.contains("since=") {
                target.trim_start_matches("/api")
            } else {
                "/stats?since=1h"
            }
        }
        _ => {
            return write_resp(stream, 404, "text/plain", "not found\n");
        }
    };
    match proxy_get(api, upstream) {
        Ok(body) => write_resp(stream, 200, "application/json", &body),
        Err(e) => write_resp(stream, 502, "text/plain", &e),
    }
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

fn write_resp(stream: &mut TcpStream, code: u16, ctype: &str, body: &str) -> Result<(), String> {
    let reason = match code {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        502 => "Bad Gateway",
        _ => "OK",
    };
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
    use std::thread;

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
        assert!(!body.contains("POST /go"));
        let (code, body) = read_http(&web_addr, "GET", "/api/walk");
        assert_eq!(code, 200, "{body}");
        assert!(body.contains("NEXT RED"), "{body}");
        let (code, body) = read_http(&web_addr, "POST", "/go");
        assert_eq!(code, 405, "{body}");
        assert!(body.contains("not a walk"));
    }
}
