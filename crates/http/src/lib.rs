//! GET-only loopback HTTP for `crucible.walk/v1` and `crucible.stats/v1`.
//!
//! No `POST /go`. No FLOOR/TRACE writes. No Herdr / Grok / EngOS types.

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::path::Path;
use std::time::Duration;

use crucible_contract::{canonical_json, Clock, StatsWindow, WalkSnapshot};
use serde::Serialize;

pub const DEFAULT_BIND: &str = "127.0.0.1:1734";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServeError {
    Usage(String),
    Io(String),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServeError::Usage(m) | ServeError::Io(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ServeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub reason: &'static str,
    pub body: String,
}

#[derive(Serialize)]
struct Health {
    ok: bool,
    version: String,
    bind: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

fn is_exact_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v == Ipv4Addr::LOCALHOST,
        IpAddr::V6(v) => v == Ipv6Addr::LOCALHOST,
    }
}

/// Parse `--bind`. Only `127.0.0.1` and `[::1]` are allowed (any port, including 0).
pub fn parse_bind(spec: &str) -> Result<SocketAddr, ServeError> {
    let addr: SocketAddr = spec.parse().map_err(|_| {
        ServeError::Usage(format!(
            "serve: invalid --bind {spec:?}; want 127.0.0.1:PORT or [::1]:PORT"
        ))
    })?;
    if !is_exact_loopback(addr.ip()) {
        return Err(ServeError::Usage(format!(
            "serve: bind must be loopback (127.0.0.1 or [::1]); refused {addr}"
        )));
    }
    Ok(addr)
}

pub fn bind_listener(spec: &str) -> Result<TcpListener, ServeError> {
    let addr = parse_bind(spec)?;
    TcpListener::bind(addr).map_err(|e| ServeError::Io(format!("serve: bind {addr}: {e}")))
}

fn error_json(msg: &str) -> String {
    canonical_json(&ErrorBody {
        error: msg.to_string(),
    })
    .unwrap_or_else(|_| "{\"error\":\"internal\"}".to_string())
}

fn json_ok<T: Serialize>(value: &T) -> Response {
    match canonical_json(value) {
        Ok(body) => Response {
            status: 200,
            reason: "OK",
            body,
        },
        Err(e) => Response {
            status: 500,
            reason: "Internal Server Error",
            body: error_json(&e.to_string()),
        },
    }
}

fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        if let Some((k, v)) = pair.split_once('=') {
            if k == key && !v.is_empty() {
                return Some(v);
            }
        } else if pair == key {
            return None;
        }
    }
    None
}

/// Route a parsed request. Never writes `.wm`.
pub fn dispatch(
    method: &str,
    path: &str,
    query: &str,
    cwd: &Path,
    clock: &dyn Clock,
    version: &str,
    bind: &str,
) -> Response {
    let method = method.to_ascii_uppercase();
    if method != "GET" {
        return Response {
            status: 405,
            reason: "Method Not Allowed",
            body: error_json("POST /go is not a walk; start go as a process"),
        };
    }
    match path {
        "/walk" => {
            let snap = WalkSnapshot::from_wm_dir(cwd, clock);
            json_ok(&snap)
        }
        "/stats" => {
            let Some(since) = query_param(query, "since") else {
                return Response {
                    status: 400,
                    reason: "Bad Request",
                    body: error_json("stats: --since needs a window (8h|24h|7d or RFC3339 Z)"),
                };
            };
            match StatsWindow::from_wm_dir(cwd, since, clock) {
                Ok(w) => json_ok(&w),
                Err(e) => Response {
                    status: 400,
                    reason: "Bad Request",
                    body: error_json(&e.message),
                },
            }
        }
        "/health" => json_ok(&Health {
            ok: true,
            version: version.to_string(),
            bind: bind.to_string(),
        }),
        _ => Response {
            status: 404,
            reason: "Not Found",
            body: error_json("not found"),
        },
    }
}

fn headers_complete(buf: &[u8]) -> bool {
    buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.windows(2).any(|w| w == b"\n\n")
}

fn parse_request(buf: &str) -> Option<(String, String, String)> {
    let head = if let Some(i) = buf.find("\r\n\r\n") {
        &buf[..i]
    } else if let Some(i) = buf.find("\n\n") {
        &buf[..i]
    } else {
        buf
    };
    let line = head.lines().next()?.trim();
    let mut it = line.split_whitespace();
    let method = it.next()?.to_string();
    let target = it.next()?;
    let _ver = it.next()?;
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };
    Some((method, path.to_string(), query.to_string()))
}

fn write_response<W: Write>(w: &mut W, r: &Response) -> io::Result<()> {
    let allow = if r.status == 405 {
        "Allow: GET\r\n"
    } else {
        ""
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n",
        r.status,
        r.reason,
        r.body.len(),
        allow
    );
    w.write_all(header.as_bytes())?;
    w.write_all(r.body.as_bytes())?;
    w.flush()
}

pub fn serve_connection<S: Read + Write>(
    mut stream: S,
    cwd: &Path,
    clock: &dyn Clock,
    version: &str,
    bind: &str,
) -> io::Result<()> {
    let mut buf = vec![0u8; 8192];
    let mut filled = 0usize;
    loop {
        if filled >= buf.len() || headers_complete(&buf[..filled]) {
            break;
        }
        let n = stream.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
        if headers_complete(&buf[..filled]) {
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf[..filled]);
    let resp = match parse_request(&text) {
        Some((method, path, query)) => dispatch(&method, &path, &query, cwd, clock, version, bind),
        None => Response {
            status: 400,
            reason: "Bad Request",
            body: error_json("malformed request"),
        },
    };
    write_response(&mut stream, &resp)
}

pub fn serve_listener(
    listener: TcpListener,
    cwd: &Path,
    clock: &dyn Clock,
    version: &str,
) -> io::Result<()> {
    let bind = listener.local_addr()?.to_string();
    loop {
        let (stream, _) = listener.accept()?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
        let _ = serve_connection(stream, cwd, clock, version, &bind);
    }
}

/// Accept one connection (tests).
pub fn serve_one(
    listener: &TcpListener,
    cwd: &Path,
    clock: &dyn Clock,
    version: &str,
) -> io::Result<()> {
    let bind = listener.local_addr()?.to_string();
    let (stream, _) = listener.accept()?;
    serve_connection(stream, cwd, clock, version, &bind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crucible_contract::{canonical_json, format_rfc3339_z, FixedClock, WALK_SCHEMA};
    use std::fs;
    use std::io::{Cursor, Read, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-http-{}-{}",
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

    const T0: i64 = 1_773_964_800;
    const NOW: i64 = T0 + 12;

    fn clock() -> FixedClock {
        FixedClock::new(NOW)
    }

    fn golden(root: &Path) {
        let wm = root.join(".wm");
        fs::create_dir_all(wm.join("evidence")).unwrap();
        fs::create_dir_all(root.join("reviews")).unwrap();
        fs::write(wm.join("FALSIFIER"), "x\n").unwrap();
        fs::write(root.join("reviews").join("review.md"), "# r\n").unwrap();
        fs::write(
            wm.join("FLOOR.md"),
            "station: ANDON\ncard: STOP-ASK INTAKE\nwip: -\nandon: STOP-ASK INTAKE\nindependence: SUBAGENT-ISOLATED\nevidence:\n  .wm/FALSIFIER\n  reviews/review.md\n",
        )
        .unwrap();
        fs::write(
            wm.join("TRACE.tsv"),
            "when\tcard\toutcome\n2026-09-20T12:00:00Z\tSTOP-ASK INTAKE\tANDON\n",
        )
        .unwrap();
        fs::write(wm.join("t0"), format!("{T0}\n")).unwrap();
    }

    #[test]
    fn parse_bind_accepts_loopback_including_ephemeral() {
        let a = parse_bind("127.0.0.1:0").unwrap();
        assert_eq!(a.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(a.port(), 0);
        let b = parse_bind("[::1]:1734").unwrap();
        assert_eq!(b.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
        assert_eq!(b.port(), 1734);
        assert!(parse_bind(DEFAULT_BIND).is_ok());
    }

    #[test]
    fn parse_bind_refuses_non_loopback_without_listening() {
        for spec in [
            "0.0.0.0:1734",
            "[::]:1734",
            "192.168.1.1:1734",
            "8.8.8.8:80",
        ] {
            let err = parse_bind(spec).unwrap_err();
            match err {
                ServeError::Usage(m) => {
                    assert!(
                        m.contains("loopback") && m.contains("refused"),
                        "spec={spec} msg={m}"
                    );
                }
                ServeError::Io(m) => panic!("must refuse before bind: {spec} {m}"),
            }
        }
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        assert!(parse_bind(&format!("0.0.0.0:{port}")).is_err());
        assert!(
            std::net::TcpStream::connect_timeout(
                &format!("127.0.0.1:{port}").parse().unwrap(),
                Duration::from_millis(150)
            )
            .is_err(),
            "refuse must not listen on {port}"
        );
    }

    #[test]
    fn bind_listener_ephemeral_is_loopback() {
        let listener = bind_listener("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_ne!(addr.port(), 0);
    }

    #[test]
    fn bind_listener_busy_port_fails() {
        let held = bind_listener("127.0.0.1:0").unwrap();
        let addr = held.local_addr().unwrap();
        let err = bind_listener(&format!("127.0.0.1:{}", addr.port())).unwrap_err();
        match err {
            ServeError::Io(m) => assert!(m.contains("bind"), "{m}"),
            ServeError::Usage(m) => panic!("busy port is Io, not Usage: {m}"),
        }
    }

    #[test]
    fn dispatch_walk_missing_wm_available_false_does_not_create() {
        let tmp = Tmp::new();
        let r = dispatch(
            "GET",
            "/walk",
            "",
            &tmp.root,
            &clock(),
            "1.17.0",
            "127.0.0.1:0",
        );
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(v["schema"], WALK_SCHEMA);
        assert_eq!(v["available"], false);
        assert!(v["available"].is_boolean());
        assert_ne!(v["available"], "unavailable");
        assert!(!tmp.root.join(".wm").exists());
    }

    #[test]
    fn dispatch_walk_equals_status_json_parser() {
        let tmp = Tmp::new();
        golden(&tmp.root);
        let snap = WalkSnapshot::from_wm_dir(&tmp.root, &clock());
        let r = dispatch(
            "GET",
            "/walk",
            "",
            &tmp.root,
            &clock(),
            "1.17.0",
            "127.0.0.1:0",
        );
        assert_eq!(r.status, 200);
        assert_eq!(r.body, canonical_json(&snap).unwrap());
        let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(v["available"], true);
        assert_eq!(v["floor"]["card"], "STOP-ASK INTAKE");
    }

    #[test]
    fn dispatch_walk_does_not_mutate_trace() {
        let tmp = Tmp::new();
        golden(&tmp.root);
        let before = fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap();
        let floor_before = fs::read(tmp.root.join(".wm/FLOOR.md")).unwrap();
        let _ = dispatch(
            "GET",
            "/walk",
            "",
            &tmp.root,
            &clock(),
            "1.17.0",
            "127.0.0.1:0",
        );
        assert_eq!(fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap(), before);
        assert_eq!(
            fs::read(tmp.root.join(".wm/FLOOR.md")).unwrap(),
            floor_before
        );
    }

    #[test]
    fn dispatch_stats_windows_from_metrics() {
        let tmp = Tmp::new();
        let wm = tmp.root.join(".wm");
        fs::create_dir_all(&wm).unwrap();
        let when = format_rfc3339_z(NOW);
        fs::write(
            wm.join("METRICS.tsv"),
            format!("when\toutcome\tslices\tbound\tnote\n{when}\tSTOP-ASK QUESTIONS\t0\t40\t-\n"),
        )
        .unwrap();
        for since in ["8h", "24h", "7d"] {
            let want = StatsWindow::from_wm_dir(&tmp.root, since, &clock()).unwrap();
            let r = dispatch(
                "GET",
                "/stats",
                &format!("since={since}"),
                &tmp.root,
                &clock(),
                "1.17.0",
                "127.0.0.1:0",
            );
            assert_eq!(r.status, 200, "since={since}");
            assert_eq!(r.body, canonical_json(&want).unwrap());
            let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
            assert_eq!(v["schema"], "crucible.stats/v1");
            assert_eq!(v["available"], true);
            assert_eq!(v["source"], "metrics");
            assert_eq!(v["halts"][0]["outcome"], "STOP-ASK QUESTIONS");
        }
    }

    #[test]
    fn dispatch_stats_missing_since_is_400() {
        let tmp = Tmp::new();
        let r = dispatch("GET", "/stats", "", &tmp.root, &clock(), "1.17.0", "127.0.0.1:0");
        assert_eq!(r.status, 400);
        assert!(!tmp.root.join(".wm").exists());
    }

    #[test]
    fn dispatch_stats_missing_metrics_available_false() {
        let tmp = Tmp::new();
        let r = dispatch(
            "GET",
            "/stats",
            "since=8h",
            &tmp.root,
            &clock(),
            "1.17.0",
            "127.0.0.1:0",
        );
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(v["available"], false);
        assert!(v["halts"].as_array().unwrap().is_empty());
        assert!(!tmp.root.join(".wm").exists());
    }

    #[test]
    fn dispatch_health_ok_version_bind() {
        let tmp = Tmp::new();
        let r = dispatch(
            "GET",
            "/health",
            "",
            &tmp.root,
            &clock(),
            "1.17.0",
            "127.0.0.1:1734",
        );
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["version"], "1.17.0");
        assert_eq!(v["bind"], "127.0.0.1:1734");
        assert!(!tmp.root.join(".wm").exists());
    }

    #[test]
    fn dispatch_post_go_is_405_or_404() {
        let tmp = Tmp::new();
        fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
        let r = dispatch(
            "POST",
            "/go",
            "",
            &tmp.root,
            &clock(),
            "1.17.0",
            "127.0.0.1:0",
        );
        assert!(
            r.status == 404 || r.status == 405,
            "POST /go must not start a walk: {}",
            r.status
        );
        assert!(!tmp.root.join(".wm").exists());
    }

    struct Pipe {
        read: Cursor<Vec<u8>>,
        write: Vec<u8>,
    }

    impl Read for Pipe {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.read.read(buf)
        }
    }

    impl Write for Pipe {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.write.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn live_get_walk_on_ephemeral_port() {
        let tmp = Tmp::new();
        golden(&tmp.root);
        let listener = bind_listener("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let root = tmp.root.clone();
        let handle = std::thread::spawn(move || serve_one(&listener, &root, &clock(), "1.17.0"));
        let mut s = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
        write!(
            s,
            "GET /walk HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut buf = String::new();
        s.read_to_string(&mut buf).unwrap();
        assert!(buf.starts_with("HTTP/1.1 200 OK"), "{buf}");
        assert!(buf.contains("\"available\":true"), "{buf}");
        handle.join().unwrap().unwrap();
    }

    #[test]
    fn serve_connection_get_health() {
        let tmp = Tmp::new();
        let req = b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        let mut pipe = Pipe {
            read: Cursor::new(req.to_vec()),
            write: Vec::new(),
        };
        serve_connection(&mut pipe, &tmp.root, &clock(), "1.17.0", "127.0.0.1:9").unwrap();
        let out = String::from_utf8(pipe.write).unwrap();
        assert!(out.starts_with("HTTP/1.1 200 OK"), "{out}");
        assert!(out.contains("\"ok\":true"));
        assert!(out.contains("\"bind\":\"127.0.0.1:9\""));
        assert!(out.contains("\"version\":\"1.17.0\""));
    }
}
