//! Query-only CLI. Binary name is `crucible` under `target/` — never overwrite repo-root POSIX `./crucible`.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;

use crucible_contract::{
    canonical_json, parse_rfc3339_z, Clock, StatsWindow, SystemClock, WalkSnapshot,
};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    process::exit(dispatch(&args, &cwd, &SystemClock));
}

fn dispatch(args: &[String], cwd: &Path, clock: &dyn Clock) -> i32 {
    if args.is_empty() || args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
        help();
        return 0;
    }
    match args[0].as_str() {
        "status" => cmd_status(&args[1..], cwd, clock),
        "debrief" => cmd_debrief(cwd),
        "stats" => cmd_stats(&args[1..], cwd, clock),
        other => {
            let _ = writeln!(io::stderr(), "unknown command: {other}");
            help();
            2
        }
    }
}

fn help() {
    println!("commands: status debrief stats help");
    println!(
        "  status --json                     read-only WalkSnapshot (does not write FLOOR/TRACE)"
    );
    println!("  debrief                           FLOOR + TRACE deltas (read-only)");
    println!("  stats --since 8h|24h|7d --json    METRICS.tsv window (PR-1; no EVENTS)");
}

fn cmd_status(args: &[String], cwd: &Path, clock: &dyn Clock) -> i32 {
    if !args.iter().any(|a| a == "--json") {
        let _ = writeln!(
            io::stderr(),
            "status is query-only in this binary; pass --json (human status remains POSIX wm.sh)"
        );
        return 2;
    }
    let snap = WalkSnapshot::from_wm_dir(cwd, clock);
    match canonical_json(&snap) {
        Ok(s) => {
            println!("{s}");
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
