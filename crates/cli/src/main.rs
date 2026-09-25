//! Product binary `crucible`. Working-mode verbs stay here. Guided verbs call `crucible-guided`.
//! Repo-root `./crucible` is the finder wrapper, not a second kernel.

mod doctor;

use std::env;
use std::fs;
use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use crucible_contract::{
    canonical_json, parse_rfc3339_z, Clock, StatsWindow, SystemClock, WalkSnapshot,
};
use crucible_guided::GuidedError;
use crucible_kernel::{floor_write_with, go};

const PRODUCT_VERSION: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../VERSION"));

const GUIDED_USAGE: &str = r#"Crucible runs one durable problem-to-done cycle for a coordinating agent.

  crucible adopt [PROGRAM] --managed   install the cycle in the current repository
  crucible adopt [PROGRAM] --managed --working-mode
                                       also copy wm.sh, skills, harness views, ROUTING.tsv, ENGINE-SOURCE
  crucible adopt [PROGRAM] --managed --panel-from SRC
                                       sibling cycle with SRC's approved panel; leftover PROBLEM stays on SRC
  crucible adopt [PROGRAM] --refresh   additive engine update; does not delete local adapters
                                       refuses src == dst; KEEP batteries unless --overwrite-batteries
  crucible cycle [status]              resume from repository evidence
  crucible cycle problem FILE          bind a problem report to the cycle
  crucible cycle problem FILE --next   same panel; archive this investigation; bind a new PROBLEM
  crucible cycle problem --abandon R   archive junk INVESTIGATE without PASS or a new PROBLEM
  crucible cycle approve-panel         record approval of the agent panel
  crucible cycle approve               record approval of the current proposal
  crucible drive [tick|stop]           Ralph-style outer loop; stop releases leftover lock
  crucible help protocol               show internal agent protocol primitives

Operators normally do not run these commands. Point a fresh agent at START.md with a problem.
"#;

const PROTOCOL_USAGE: &str = r#"Agent protocol primitives. These are implementation details used by START.md; the operator does
not drive the cycle with them.

  claim add|list|verdict|scout|admit   verify a report before it becomes work
                                      claim add requires one predicate [ABSENT|EXISTS|DEFECT]
                                      claim verdict AGENT STALE|FALSE --like C2 C3 copies non-TRUE
  triage                              render the evidence-grounded proposal input
  add / ready / phase / task          create and advance bounded work
  dispatch / attempt / result         coordinate isolated agent attempts
  attempt transport                   record multi-agent|acp|subagent isolation
  attempt reclaim ATTEMPT             STOPPED a RUNNING attempt whose pid is dead
  evidence archive SLUG               park item evidence whose work-id is not current
  contract-audit ATTEMPT AUDITOR …    file-based contract PASS|FIX|STOP by a distinct agent
  contract-audit ATTEMPT AUDITOR PASS --like ATTEMPT2 …   copy PASS onto isomorphic DISPATCHED contracts
  plan-audit SLUG AUDITOR PASS|FIX|STOP  ITEM.md audit before maker dispatch (guided)
  probe-acp ok|failed|unavailable     record ACP availability for the ladder
  run / run-claim                     record work-bound evidence
  check / close                       refuse unsupported completion
  agents / target / state / workid    inspect supporting state
  lifecycle status|enable             compatibility setup for older programs
  panes / selftest                    optional observation and engine verification

Independence ladder: multi-agent preferred, then ACP, then subagent after ACP probe failure.
If no independent agent can be invoked, STOP and warn — do not continue as solo theatre.
"#;

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
    dispatch_verb(&args[0], &args[1..], cwd, clock)
}

/// One match so the verb set is readable as a table. `scripts/selftest.sh` enumerates these
/// arms; a second table in the wrapper would be a second kernel.
fn dispatch_verb(verb: &str, rest: &[String], cwd: &Path, clock: &dyn Clock) -> i32 {
    let args: Vec<&str> = rest.iter().map(String::as_str).collect();
    match verb {
        "go" => cmd_go(rest, cwd, clock),
        "status" => cmd_status(rest, cwd, clock),
        "debrief" => cmd_debrief(cwd),
        "stats" => cmd_stats(rest, cwd, clock),
        "serve" => cmd_serve(rest, cwd, clock),
        "room" => cmd_room(rest, cwd),
        "camera" => cmd_camera(rest),
        "reap" => cmd_reap(rest),
        "web" => cmd_web(rest),
        "doctor" => cmd_doctor(rest, cwd),
        "adopt" => with_guided_root(|root| {
            let mut out = io::stdout();
            match crucible_guided::cmd_adopt(cwd, root, &args, &mut out) {
                Ok(()) => 0,
                Err(err) => guided_fail(err),
            }
        }),
        "cycle" => with_guided_root(|root| guided_ok(crucible_guided::cycle(root, &args, clock))),
        "drive" => {
            with_guided_root(|root| guided_ok(crucible_guided::drive::drive(root, &args, clock)))
        }
        "claim" => {
            with_guided_root(|root| guided_ok(crucible_guided::claims::claim(root, &args, clock)))
        }
        "triage" => with_guided_root(|root| match crucible_guided::triage::triage(root) {
            Ok(report) => guided_report(report.text, report.status),
            Err(err) => guided_fail(err),
        }),
        "add" => {
            with_guided_root(|root| guided_ok(crucible_guided::inspect::add(root, &args, clock)))
        }
        "dispatch" => with_guided_root(|root| {
            guided_ok(crucible_guided::dispatch::dispatch(root, &args, clock))
        }),
        "attempt" => with_guided_root(|root| {
            guided_ok(crucible_guided::attempt::attempt(root, &args, clock))
        }),
        "result" => {
            with_guided_root(|root| guided_ok(crucible_guided::result::result(root, &args, clock)))
        }
        "contract-audit" => with_guided_root(|root| {
            guided_ok(crucible_guided::audit::contract_audit(root, &args, clock))
        }),
        "plan-audit" => with_guided_root(|root| {
            guided_ok(crucible_guided::audit::plan_audit(root, &args, clock))
        }),
        "probe-acp" => with_guided_root(|root| {
            guided_ok(crucible_guided::audit::probe_acp(root, &args, clock))
        }),
        "phase" => {
            with_guided_root(|root| guided_ok(crucible_guided::phase::phase(root, &args, clock)))
        }
        "task" => {
            with_guided_root(|root| guided_ok(crucible_guided::task::task(root, &args, clock)))
        }
        "ready" => {
            with_guided_root(|root| guided_ok(crucible_guided::task::ready(root, &args, clock)))
        }
        "check" => with_guided_root(|root| match crucible_guided::close::check(root, &args) {
            Ok(report) => guided_report(report.text, report.status),
            Err(err) => guided_fail(err),
        }),
        "close" => {
            with_guided_root(|root| guided_ok(crucible_guided::close::close(root, &args, clock)))
        }
        "evidence" => {
            with_guided_root(|root| guided_ok(crucible_guided::inspect::evidence(root, &args)))
        }
        "run" => {
            with_guided_root(|root| guided_ok(crucible_guided::inspect::run(root, &args, clock)))
        }
        "run-claim" => {
            with_guided_root(|root| guided_ok(crucible_guided::run::run_claim(root, &args, clock)))
        }
        "next" => with_guided_root(|root| guided_ok(crucible_guided::inspect::next(root))),
        "agents" => with_guided_root(|root| guided_ok(crucible_guided::inspect::agents(root))),
        "target" => {
            with_guided_root(|root| guided_ok(crucible_guided::inspect::target(root, &args)))
        }
        "state" => with_guided_root(|root| guided_ok(crucible_guided::inspect::state(root))),
        "lifecycle" => {
            with_guided_root(|root| guided_ok(crucible_guided::inspect::lifecycle(root, &args)))
        }
        "brief" => with_guided_root(|root| guided_ok(crucible_guided::inspect::brief(root, &args))),
        "workid" => {
            with_guided_root(|root| guided_ok(crucible_guided::inspect::workid(root, &args)))
        }
        "panes" => with_guided_root(|root| guided_ok(crucible_guided::inspect::panes(root, &args))),
        "selftest" => cmd_selftest(rest),
        "help" => cmd_help(rest),
        other => unknown_verb(other),
    }
}

fn with_guided_root(body: impl FnOnce(&Path) -> i32) -> i32 {
    match crucible_guided::root() {
        Ok(root) => body(&root),
        Err(err) => guided_fail(err),
    }
}

fn guided_ok(result: Result<String, GuidedError>) -> i32 {
    match result {
        Ok(text) => guided_report(text, 0),
        Err(err) => guided_fail(err),
    }
}

fn guided_report(text: String, status: i32) -> i32 {
    print!("{text}");
    let _ = io::stdout().flush();
    status
}

/// Refusals are the shell `die` path (exit 2, `crucible: ` prefix). IO matches `cmd_go`.
fn guided_fail(err: GuidedError) -> i32 {
    match err {
        GuidedError::Io(err) => {
            let _ = writeln!(io::stderr(), "{err}");
            1
        }
        GuidedError::Message(msg) => {
            let _ = writeln!(io::stderr(), "crucible: {msg}");
            2
        }
    }
}

fn cmd_selftest(args: &[String]) -> i32 {
    match env::var("CRUCIBLE_IN_SELFTEST") {
        Ok(value) if !value.is_empty() => {
            let _ = writeln!(
                io::stderr(),
                "crucible: already inside a selftest — refusing to recurse"
            );
            return 2;
        }
        _ => {}
    }
    with_guided_root(|root| {
        // SAFETY: this process is about to be replaced; no other thread reads the variable.
        unsafe { env::set_var("CRUCIBLE_IN_SELFTEST", "1") };
        let script = root.join("scripts/selftest.sh");
        let err = Command::new(script).args(args).exec();
        let _ = writeln!(io::stderr(), "{err}");
        1
    })
}

fn cmd_help(args: &[String]) -> i32 {
    if args.is_empty() {
        help();
        return 0;
    }
    // Extra words after `protocol` still print the protocol text, matching the shell arm.
    if args[0] == "protocol" {
        print!("{PROTOCOL_USAGE}");
        let _ = io::stdout().flush();
        return 0;
    }
    let _ = writeln!(io::stderr(), "crucible: usage: crucible help [protocol]");
    2
}

fn help() {
    let _ = write_help(&mut io::stdout());
}

fn unknown_verb(verb: &str) -> i32 {
    let _ = writeln!(io::stderr(), "crucible: unknown verb: {verb}\n");
    let _ = write_help(&mut io::stderr());
    2
}

fn write_help(out: &mut dyn Write) -> io::Result<()> {
    write!(out, "{GUIDED_USAGE}")?;
    writeln!(out)?;
    writeln!(
        out,
        "commands: go status debrief stats serve room web camera reap doctor adopt cycle drive help"
    )?;
    writeln!(
        out,
        "  go                                start walk (foreground; STOP-ASK INTAKE without IDEA.md)"
    )?;
    writeln!(
        out,
        "  status                            rewrite FLOOR from the on-disk card (no TRACE/EVENTS)"
    )?;
    writeln!(
        out,
        "  status --json                     read-only WalkSnapshot (does not write FLOOR/TRACE)"
    )?;
    writeln!(
        out,
        "  debrief                           FLOOR + TRACE deltas (read-only)"
    )?;
    writeln!(
        out,
        "  stats --since 8h|24h|7d --json    .wm/EVENTS if readable, else METRICS.tsv"
    )?;
    writeln!(
        out,
        "  serve [--bind 127.0.0.1:PORT]    GET /walk /stats /health (loopback; default 127.0.0.1:1734)"
    )?;
    writeln!(
        out,
        "  room                              attach one herdr workspace; new role tabs only; go is a process"
    )?;
    writeln!(
        out,
        "  web [--bind 127.0.0.1:1735]       backlog and chat; POST /act/go spawns; POST /go is 405"
    )?;
    writeln!(
        out,
        "  camera --bind ADDR                GET /walk and /stats (read-only)"
    )?;
    writeln!(
        out,
        "  reap --pid N                      SIGTERM the go process group"
    )?;
    writeln!(
        out,
        "  doctor                            warn if <cwd>/.grok/rules/loop-router.md is missing or stale vs ADR-HASH"
    )?;
    writeln!(
        out,
        "  doctor --home                     copy the fixture to $HOME/.grok/rules/loop-router.md, then check it"
    )?;
    writeln!(out, "  --version, -V                     product VERSION")?;
    Ok(())
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
    // A missing independence line parses as empty. Writing that blank erases the default.
    let independence = if floor.independence.is_empty() {
        "SUBAGENT-ISOLATED"
    } else {
        floor.independence.as_str()
    };
    match floor_write_with(cwd, &floor.card, independence, clock, false) {
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

fn cmd_doctor(args: &[String], cwd: &Path) -> i32 {
    match args {
        [] => doctor_repo(cwd),
        [flag] if flag == "--home" => doctor_home(),
        [flag, extra, ..] if flag == "--home" => {
            let _ = writeln!(io::stderr(), "doctor: unknown arg {extra}");
            2
        }
        [first, ..] => {
            let _ = writeln!(io::stderr(), "doctor: unknown arg {first}");
            2
        }
    }
}

fn doctor_repo(cwd: &Path) -> i32 {
    let path = doctor::router_path(cwd);
    let report = doctor::check_router(&path, &doctor::fixture_adr_hash(), doctor::RouterSite::Repo);
    let _ = report.write_lines(io::stdout());
    0
}

fn doctor_home() -> i32 {
    let home = env::var_os("HOME").filter(|value| !value.is_empty());
    let Some(home) = home else {
        let _ = writeln!(io::stdout(), "warn: {}", doctor::HOME_UNSET_WARNING);
        return 1;
    };
    let path = match doctor::install_home_router(Path::new(&home)) {
        Ok(path) => path,
        Err(err) => {
            let _ = writeln!(io::stdout(), "warn: {err}");
            return 1;
        }
    };
    let report = doctor::check_router(&path, &doctor::fixture_adr_hash(), doctor::RouterSite::Home);
    let _ = report.write_lines(io::stdout());
    if report.warnings.is_empty() && !report.oks.is_empty() {
        0
    } else {
        1
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
