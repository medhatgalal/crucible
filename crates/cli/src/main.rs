//! Working-mode binary `crucible` (go/status/debrief/stats/serve/room/doctor). Repo-root POSIX `./crucible` stays guided adopt.

mod doctor;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;

use crucible_contract::{
    canonical_json, parse_rfc3339_z, Clock, StatsWindow, SystemClock, WalkSnapshot,
};
use crucible_kernel::{floor_write_with, go};

const PRODUCT_VERSION: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../VERSION"));

fn product_version() -> &'static str {
    PRODUCT_VERSION.trim()
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    process::exit(dispatch(&args, &cwd, &SystemClock));
}

fn dispatch(args: &[String], cwd: &Path, clock: &dyn Clock) -> i32 {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        help();
        return 0;
    }
    if args[0] == "--version" || args[0] == "-V" {
        println!("{}", product_version());
        return 0;
    }
    if args[0] == "help" {
        if args.len() > 1 {
            return exec_guided(args);
        }
        help();
        return 0;
    }
    match args[0].as_str() {
        "go" => cmd_go(&args[1..], cwd, clock),
        "status" => cmd_status(&args[1..], cwd, clock),
        "debrief" => cmd_debrief(cwd),
        "stats" => cmd_stats(&args[1..], cwd, clock),
        "serve" => cmd_serve(&args[1..], cwd, clock),
        "room" => cmd_room(&args[1..], cwd),
        "camera" => cmd_camera(&args[1..]),
        "reap" => cmd_reap(&args[1..]),
        "web" => cmd_web(&args[1..]),
        "doctor" => cmd_doctor(&args[1..]),
        other => exec_guided_or_unknown(other, args),
    }
}

fn guided_path() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    let p = dir.join("crucible-guided");
    p.is_file().then_some(p)
}

fn exec_guided(args: &[String]) -> i32 {
    let Some(guided) = guided_path() else {
        let cmd = args.first().map(String::as_str).unwrap_or("help");
        let _ = writeln!(io::stderr(), "unknown command: {cmd}");
        help();
        return 2;
    };
    match process::Command::new(guided).args(args).status() {
        Ok(st) => st.code().unwrap_or(1),
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            1
        }
    }
}

fn exec_guided_or_unknown(other: &str, args: &[String]) -> i32 {
    if guided_path().is_some() {
        return exec_guided(args);
    }
    let _ = writeln!(io::stderr(), "unknown command: {other}");
    help();
    2
}

fn help() {
    println!("commands: go status debrief stats serve room web camera reap doctor help");
    println!("  go                                start walk (foreground; STOP-ASK INTAKE without IDEA.md)");
    println!(
        "  status                            rewrite FLOOR from the on-disk card (no TRACE/EVENTS)"
    );
    println!(
        "  status --json                     read-only WalkSnapshot (does not write FLOOR/TRACE)"
    );
    println!("  debrief                           FLOOR + TRACE deltas (read-only)");
    println!("  stats --since 8h|24h|7d --json    .wm/EVENTS if readable, else METRICS.tsv");
    println!(
        "  serve [--bind 127.0.0.1:PORT]    GET /walk /stats /health (loopback; default 127.0.0.1:1734)"
    );
    println!(
        "  room                              herdr tabs; cameras GET; go is a process in orchestrator"
    );
    println!(
        "  web [--bind 127.0.0.1:1735]       GET-only page; proxies /walk /stats /health; no POST /go"
    );
    println!("  camera --bind ADDR                GET /walk and /stats (read-only)");
    println!("  reap --pid N                      SIGTERM the go process group");
    println!(
        "  doctor                            warn if home loop-router is missing or stale vs ADR-HASH"
    );
    println!("  --version, -V                     product VERSION");
}

fn cmd_go(args: &[String], cwd: &Path, clock: &dyn Clock) -> i32 {
    let mut idea_src: Option<&str> = None;
    for a in args {
        if a == "--next" {
            let _ = writeln!(io::stderr(), "go --next is not ported");
            return 2;
        }
        if a.starts_with('-') {
            let _ = writeln!(io::stderr(), "idea path must not start with -");
            return 2;
        }
        if idea_src.is_some() {
            let _ = writeln!(io::stderr(), "usage: go [--next] [IDEA.md]");
            return 2;
        }
        idea_src = Some(a.as_str());
    }
    if let Some(src) = idea_src {
        let dest = cwd.join("IDEA.md");
        let src_path = {
            let p = Path::new(src);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                cwd.join(p)
            }
        };
        if src_path.is_file() && !dest.is_file() {
            if let Err(e) = fs::copy(&src_path, &dest) {
                let _ = writeln!(io::stderr(), "{e}");
                return 1;
            }
        }
    }
    match go(cwd, clock) {
        Ok(r) => {
            if !r.stdout.is_empty() {
                print!("{}", r.stdout);
                let _ = io::stdout().flush();
            }
            if !r.stderr.is_empty() {
                eprint!("{}", r.stderr);
                let _ = io::stderr().flush();
            }
            r.exit
        }
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            1
        }
    }
}

fn cmd_status(args: &[String], cwd: &Path, clock: &dyn Clock) -> i32 {
    if args.iter().any(|a| a == "--json") {
        let snap = WalkSnapshot::from_wm_dir(cwd, clock);
        return match canonical_json(&snap) {
            Ok(s) => {
                println!("{s}");
                0
            }
            Err(e) => {
                let _ = writeln!(io::stderr(), "{e}");
                1
            }
        };
    }
    if !args.is_empty() {
        let _ = writeln!(io::stderr(), "usage: status [--json]");
        return 2;
    }
    // Read-only snapshot first. No card means write nothing, including t0.
    let snap = WalkSnapshot::from_wm_dir(cwd, clock);
    let Some(floor) = snap.floor else {
        let _ = writeln!(io::stderr(), "no card on disk");
        return 1;
    };
    match floor_write_with(cwd, &floor.card, &floor.independence, clock, false) {
        Ok(r) => {
            println!(
                "FLOOR t=+{}s station={} card={} wip={}",
                r.elapsed_s, r.station, r.card, r.wip
            );
            0
        }
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            1
        }
    }
}

fn cmd_stats(args: &[String], cwd: &Path, clock: &dyn Clock) -> i32 {
    let mut since: Option<&str> = None;
    let mut json = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json = true,
            "--since" => {
                i += 1;
                if i >= args.len() {
                    let _ = writeln!(
                        io::stderr(),
                        "stats: --since needs a window (8h|24h|7d or RFC3339 Z)"
                    );
                    return 2;
                }
                since = Some(args[i].as_str());
            }
            a if let Some(v) = a.strip_prefix("--since=") => since = Some(v),
            other => {
                let _ = writeln!(io::stderr(), "stats: unknown arg {other}");
                return 2;
            }
        }
        i += 1;
    }
    let Some(since) = since else {
        let _ = writeln!(io::stderr(), "usage: stats --since 8h|24h|7d --json");
        return 2;
    };
    if !json {
        let _ = writeln!(
            io::stderr(),
            "stats is query-only in this binary; pass --json"
        );
        return 2;
    }
    match StatsWindow::from_wm_dir(cwd, since, clock) {
        Ok(w) => match canonical_json(&w) {
            Ok(s) => {
                println!("{s}");
                0
            }
            Err(e) => {
                let _ = writeln!(io::stderr(), "{e}");
                1
            }
        },
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            2
        }
    }
}

fn cmd_serve(args: &[String], cwd: &Path, clock: &dyn Clock) -> i32 {
    let mut bind = crucible_http::DEFAULT_BIND.to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bind" => {
                i += 1;
                if i >= args.len() {
                    let _ = writeln!(
                        io::stderr(),
                        "serve: --bind needs 127.0.0.1:PORT or [::1]:PORT"
                    );
                    return 2;
                }
                bind = args[i].clone();
            }
            a if let Some(v) = a.strip_prefix("--bind=") => bind = v.to_string(),
            other => {
                let _ = writeln!(io::stderr(), "serve: unknown arg {other}");
                return 2;
            }
        }
        i += 1;
    }
    match crucible_http::bind_listener(&bind) {
        Ok(listener) => {
            let addr = match listener.local_addr() {
                Ok(a) => a,
                Err(e) => {
                    let _ = writeln!(io::stderr(), "serve: {e}");
                    return 1;
                }
            };
            println!("listening {addr}");
            let _ = io::stdout().flush();
            match crucible_http::serve_listener(listener, cwd, clock, product_version()) {
                Ok(()) => 0,
                Err(e) => {
                    let _ = writeln!(io::stderr(), "serve: {e}");
                    1
                }
            }
        }
        Err(crucible_http::ServeError::Usage(m)) => {
            let _ = writeln!(io::stderr(), "{m}");
            2
        }
        Err(crucible_http::ServeError::Io(m)) => {
            let _ = writeln!(io::stderr(), "{m}");
            1
        }
    }
}

fn cmd_room(args: &[String], cwd: &Path) -> i32 {
    if !args.is_empty() {
        let _ = writeln!(io::stderr(), "usage: room");
        return 2;
    }
    let exe = match env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            let _ = writeln!(io::stderr(), "room: current_exe: {e}");
            return 1;
        }
    };
    let path = env::var_os("PATH").unwrap_or_default();
    let herdr_override = env::var_os("CRUCIBLE_HERDR");
    crucible_room::run(&exe, cwd, &path, herdr_override.as_deref())
}

fn cmd_camera(args: &[String]) -> i32 {
    let mut bind = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bind" => {
                i += 1;
                if i >= args.len() {
                    let _ = writeln!(io::stderr(), "camera: --bind needs 127.0.0.1:PORT");
                    return 2;
                }
                bind = Some(args[i].clone());
            }
            other => {
                let _ = writeln!(io::stderr(), "camera: unknown arg {other}");
                return 2;
            }
        }
        i += 1;
    }
    let Some(addr) = bind else {
        let _ = writeln!(io::stderr(), "usage: camera --bind 127.0.0.1:PORT");
        return 2;
    };
    for path in ["/walk", "/stats?since=1h", "/health"] {
        match crucible_room::http_get(&addr, path) {
            Ok(body) => println!("{body}"),
            Err(e) => {
                let _ = writeln!(io::stderr(), "camera: {e}");
                return 1;
            }
        }
    }
    0
}

fn cmd_reap(args: &[String]) -> i32 {
    let mut pid = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--pid" => {
                i += 1;
                if i >= args.len() {
                    let _ = writeln!(io::stderr(), "reap: --pid needs a process id");
                    return 2;
                }
                match args[i].parse::<u32>() {
                    Ok(n) => pid = Some(n),
                    Err(_) => {
                        let _ = writeln!(io::stderr(), "reap: bad pid {}", args[i]);
                        return 2;
                    }
                }
            }
            other => {
                let _ = writeln!(io::stderr(), "reap: unknown arg {other}");
                return 2;
            }
        }
        i += 1;
    }
    let Some(pid) = pid else {
        let _ = writeln!(io::stderr(), "usage: reap --pid N");
        return 2;
    };
    match crucible_room::kill_process_group(pid) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            1
        }
    }
}

fn cmd_web(args: &[String]) -> i32 {
    let mut bind = crucible_web::DEFAULT_WEB_BIND.to_string();
    let mut api = crucible_web::DEFAULT_API_BIND.to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bind" => {
                i += 1;
                if i >= args.len() {
                    let _ = writeln!(io::stderr(), "web: --bind needs 127.0.0.1:PORT");
                    return 2;
                }
                bind = args[i].clone();
            }
            "--api" => {
                i += 1;
                if i >= args.len() {
                    let _ = writeln!(io::stderr(), "web: --api needs 127.0.0.1:PORT");
                    return 2;
                }
                api = args[i].clone();
            }
            other => {
                let _ = writeln!(io::stderr(), "web: unknown arg {other}");
                return 2;
            }
        }
        i += 1;
    }
    match crucible_web::bind_web(&bind) {
        Ok(listener) => {
            let addr = match listener.local_addr() {
                Ok(a) => a,
                Err(e) => {
                    let _ = writeln!(io::stderr(), "web: {e}");
                    return 1;
                }
            };
            println!("listening {addr}");
            let _ = io::stdout().flush();
            if let Err(e) = crucible_web::serve_web(listener, &api) {
                let _ = writeln!(io::stderr(), "web: {e}");
                return 1;
            }
            0
        }
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            2
        }
    }
}

fn cmd_doctor(args: &[String]) -> i32 {
    if let Some(other) = args.first() {
        let _ = writeln!(io::stderr(), "doctor: unknown arg {other}");
        return 2;
    }
    let home = env::var_os("HOME").map(PathBuf::from);
    let router = doctor::home_router_path(home.as_deref());
    let report = doctor::check_router(router.as_deref(), &doctor::fixture_adr_hash());
    let _ = report.write_lines(io::stdout());
    0
}

fn cmd_debrief(cwd: &Path) -> i32 {
    let wm = if cwd.file_name().is_some_and(|n| n == ".wm") {
        cwd.to_path_buf()
    } else {
        cwd.join(".wm")
    };
    let floor = wm.join("FLOOR.md");
    let trace = wm.join("TRACE.tsv");
    if !floor.is_file() {
        let _ = writeln!(io::stderr(), "no FLOOR.md (run go or status)");
        return 1;
    }
    if !trace.is_file() {
        let _ = writeln!(io::stderr(), "no TRACE.tsv");
        return 1;
    }
    let floor_text = match fs::read_to_string(&floor) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            return 1;
        }
    };
    let trace_text = match fs::read_to_string(&trace) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(io::stderr(), "{e}");
            return 1;
        }
    };
    println!("debrief");
    print!("{floor_text}");
    if !floor_text.ends_with('\n') {
        println!();
    }
    println!("when  delta_s  total_s  card  station");
    let mut prev: Option<i64> = None;
    let mut t0: Option<i64> = None;
    for (i, line) in trace_text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if i == 0 && line.starts_with("when") {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(when) = parts.next() else {
            continue;
        };
        let Some(card) = parts.next() else {
            continue;
        };
        let station = parts.next().unwrap_or("");
        let Some(t) = parse_rfc3339_z(when) else {
            continue;
        };
        if t0.is_none() {
            t0 = Some(t);
        }
        let delta = prev.map(|p| t - p).unwrap_or(0);
        let total = t - t0.unwrap_or(t);
        println!("{when}  {delta}  {total}  {card}  {station}");
        prev = Some(t);
    }
    0
}
