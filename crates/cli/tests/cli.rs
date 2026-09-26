//! CLI: `status --json` stays read-only; bare `status` rewrites FLOOR without TRACE; `go` writes both.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crucible_contract::{format_rfc3339_z, parse_rfc3339_z, Clock, SystemClock};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct Tmp {
    root: PathBuf,
}

impl Tmp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "crucible-cli-{}-{}",
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

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_crucible"))
}

fn posix_tool(name: &str) -> PathBuf {
    for p in [format!("/bin/{name}"), format!("/usr/bin/{name}")] {
        let p = PathBuf::from(p);
        if p.is_file() {
            return p;
        }
    }
    panic!("{name} not found in /bin or /usr/bin");
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "crucible")
        .env("GIT_AUTHOR_EMAIL", "crucible@example.test")
        .env("GIT_COMMITTER_NAME", "crucible")
        .env("GIT_COMMITTER_EMAIL", "crucible@example.test")
        .output()
        .expect("spawn git");
    assert!(
        o.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

fn init_git_product(dir: &Path) {
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.name", "crucible"]);
    git(dir, &["config", "user.email", "crucible@example.test"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
    fs::write(dir.join(".gitignore"), ".wm/\n").unwrap();
    fs::write(dir.join("README"), "product\n").unwrap();
    git(dir, &["add", ".gitignore", "README"]);
    git(dir, &["commit", "-qm", "init"]);
}

fn golden_board(root: &std::path::Path) -> String {
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
    let trace = "when\tcard\toutcome\n2026-09-20T12:00:00Z\tSTOP-ASK INTAKE\tANDON\n";
    fs::write(wm.join("TRACE.tsv"), trace).unwrap();
    fs::write(wm.join("t0"), "1773964800\n").unwrap();
    trace.to_string()
}

#[test]
fn status_json_does_not_mutate_trace() {
    let tmp = Tmp::new();
    let before = golden_board(&tmp.root);
    let before_bytes = fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap();
    let floor_before = fs::read(tmp.root.join(".wm/FLOOR.md")).unwrap();
    let t0_before = fs::read(tmp.root.join(".wm/t0")).unwrap();
    let out = bin()
        .current_dir(&tmp.root)
        .args(["status", "--json"])
        .output()
        .expect("run status --json");
    assert!(
        out.status.success(),
        "status --json failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after = fs::read_to_string(tmp.root.join(".wm/TRACE.tsv")).unwrap();
    assert_eq!(after, before, "status --json must not rewrite TRACE");
    let after_bytes = fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap();
    assert_eq!(after_bytes, before_bytes);
    assert_eq!(
        fs::read(tmp.root.join(".wm/FLOOR.md")).unwrap(),
        floor_before,
        "status --json must not rewrite FLOOR"
    );
    assert_eq!(fs::read(tmp.root.join(".wm/t0")).unwrap(), t0_before);
    assert!(!tmp.root.join(".wm/EVENTS").exists());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["schema"], "crucible.walk/v1");
    assert_eq!(v["available"], true);
    assert!(v["available"].is_boolean());
    assert_ne!(v["available"], "unavailable");
    assert_eq!(v["floor"]["evidence"][0], ".wm/FALSIFIER");
    assert_eq!(v["floor"]["evidence"][1], "reviews/review.md");
}

#[test]
fn status_json_missing_wm_does_not_create_files() {
    let tmp = Tmp::new();
    let out = bin()
        .current_dir(&tmp.root)
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!tmp.root.join(".wm").exists(), "must not mkdir .wm");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["available"], false);
}

#[test]
fn debrief_prints_floor_and_trace_deltas() {
    let tmp = Tmp::new();
    golden_board(&tmp.root);
    let out = bin()
        .current_dir(&tmp.root)
        .arg("debrief")
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("debrief"));
    assert!(stdout.contains("station: ANDON"));
    assert!(stdout.contains("STOP-ASK INTAKE"));
    assert!(stdout.contains("delta_s"));
    let want = "\
debrief
station: ANDON
card: STOP-ASK INTAKE
wip: -
andon: STOP-ASK INTAKE
independence: SUBAGENT-ISOLATED
evidence:
  .wm/FALSIFIER
  reviews/review.md
when  delta_s  total_s  card  station
2026-09-20T12:00:00Z  0  0  STOP-ASK INTAKE  ANDON
";
    assert_eq!(stdout, want);
    assert!(out.stderr.is_empty());
}

#[test]
fn debrief_missing_floor_writes_stdout() {
    let tmp = Tmp::new();
    let out = bin()
        .current_dir(&tmp.root)
        .arg("debrief")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, b"no FLOOR.md (run go or status)\n");
    assert!(out.stderr.is_empty());
    assert!(!tmp.root.join(".wm").exists());
}

#[test]
fn debrief_missing_trace_writes_stdout() {
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    let floor = b"station: ANDON\ncard: STOP-ASK INTAKE\nwip: -\nandon: STOP-ASK INTAKE\nindependence: SUBAGENT-ISOLATED\nevidence:\n  .wm/FALSIFIER\n  reviews/review.md\n";
    fs::write(wm.join("FLOOR.md"), floor).unwrap();
    let out = bin()
        .current_dir(&tmp.root)
        .arg("debrief")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, b"no TRACE.tsv\n");
    assert!(out.stderr.is_empty());
    assert_eq!(fs::read(wm.join("FLOOR.md")).unwrap(), floor);
    assert!(!wm.join("TRACE.tsv").exists());
}

#[cfg(unix)]
#[test]
fn debrief_unreadable_floor_writes_stdout() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    let floor = wm.join("FLOOR.md");
    fs::write(&floor, b"x\n").unwrap();
    fs::write(wm.join("TRACE.tsv"), b"when\tcard\toutcome\n").unwrap();
    let mut perm = fs::metadata(&floor).unwrap().permissions();
    perm.set_mode(0o000);
    fs::set_permissions(&floor, perm).unwrap();
    let err = fs::read_to_string(&floor).expect_err("floor must be unreadable");
    let out = bin()
        .current_dir(&tmp.root)
        .arg("debrief")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty());
    assert_eq!(out.stdout, format!("{err}\n").as_bytes());
    assert!(!out.stdout.starts_with(b"debrief\n"));
}

#[cfg(unix)]
#[test]
fn debrief_unreadable_trace_writes_stdout() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    fs::write(wm.join("FLOOR.md"), b"station: ANDON\n").unwrap();
    let trace = wm.join("TRACE.tsv");
    fs::write(&trace, b"when\tcard\toutcome\n").unwrap();
    let mut perm = fs::metadata(&trace).unwrap().permissions();
    perm.set_mode(0o000);
    fs::set_permissions(&trace, perm).unwrap();
    let err = fs::read_to_string(&trace).expect_err("trace must be unreadable");
    let out = bin()
        .current_dir(&tmp.root)
        .arg("debrief")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty());
    assert_eq!(out.stdout, format!("{err}\n").as_bytes());
    assert!(!out.stdout.starts_with(b"debrief\n"));
}

fn write_wal_only(root: &Path) -> (String, Vec<u8>) {
    let wm = root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    // SystemClock in the binary: stamp EVENTS at "now" so 8h|24h|7d include the rows.
    let when = format_rfc3339_z(SystemClock.now_unix());
    let body = format!(
        "{{\"t\":\"{when}\",\"kind\":\"card\",\"card\":\"NEXT RED\",\"station\":\"BUILD\"}}\n\
         {{\"t\":\"{when}\",\"kind\":\"invoke_end\",\"session\":\"00000000-0000-0000-0000-000000000001\",\"card\":\"NEXT RUN maker-build\",\"elapsed_s\":7,\"exit\":0}}\n\
         {{\"t\":\"{when}\",\"kind\":\"halt\",\"card\":\"STOP-ASK FROM-EVENTS\",\"elapsed_s\":90,\"iterations\":3}}\n"
    );
    let path = wm.join("EVENTS");
    fs::write(&path, &body).unwrap();
    assert!(
        !wm.join("METRICS.tsv").exists(),
        "WAL-only tree must not include METRICS"
    );
    (when, fs::read(&path).unwrap())
}

#[test]
fn stats_since_json_from_events_wal() {
    let tmp = Tmp::new();
    let (when, events_before) = write_wal_only(&tmp.root);
    for window in ["8h", "24h", "7d"] {
        let out = bin()
            .current_dir(&tmp.root)
            .args(["stats", "--since", window, "--json"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "stats --since {window}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["schema"], "crucible.stats/v1");
        assert_eq!(v["available"], true);
        assert_eq!(v["source"], "events");
        let halts = v["halts"].as_array().expect("halts array");
        assert_eq!(halts.len(), 1, "window {window}: EVENTS halt only");
        assert_eq!(halts[0]["outcome"], "STOP-ASK FROM-EVENTS");
        assert_eq!(halts[0]["slices"], 0);
        assert_eq!(halts[0]["bound"], 0);
        assert_eq!(halts[0]["note"], "-");
        assert_eq!(halts[0]["elapsed_s"], 90);
        assert_eq!(halts[0]["iterations"], 3);
        assert_eq!(halts[0]["t"], when);
        assert_eq!(v["counts"]["halt"], 1);
        assert_eq!(v["counts"]["card"], 1);
        assert_eq!(v["counts"]["invoke_end"], 1);
        let dumped = serde_json::to_string(&v).unwrap();
        assert!(
            !dumped.contains("STOP-ASK QUESTIONS"),
            "METRICS outcome must not appear in stats"
        );
        assert!(!dumped.contains("NEXT RED"));
    }
    assert_eq!(
        fs::read(tmp.root.join(".wm/EVENTS")).unwrap(),
        events_before,
        "stats --json must not write EVENTS"
    );
    assert!(!tmp.root.join(".wm/METRICS.tsv").exists());
}

#[test]
fn status_without_json_writes_floor_not_trace() {
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    fs::write(wm.join("FALSIFIER"), "x\n").unwrap();
    fs::write(wm.join("slice-in-flight"), "id: s9\n").unwrap();
    fs::write(
        wm.join("FLOOR.md"),
        "station: SHAPE\ncard: STOP-ASK INTAKE\nwip: stale\nandon: -\nindependence: CROSS-FAMILY\nevidence:\n  .wm/CLOSED\n",
    )
    .unwrap();
    let trace = "when\tcard\toutcome\n2026-09-20T12:00:00Z\tNEXT MAP\tDESIGN\n";
    fs::write(wm.join("TRACE.tsv"), trace).unwrap();

    let out = bin().current_dir(&tmp.root).arg("status").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.trim();
    assert!(
        line.starts_with("FLOOR t=+")
            && (line.contains("t=+0s ") || line.contains("t=+1s "))
            && line.ends_with("station=ANDON card=STOP-ASK INTAKE wip=s9"),
        "stdout={stdout:?}"
    );
    assert_eq!(
        fs::read_to_string(wm.join("TRACE.tsv")).unwrap(),
        trace,
        "bare status must not append TRACE"
    );
    assert!(
        !wm.join("EVENTS").exists(),
        "bare status must not create EVENTS"
    );
    assert!(wm.join("t0").is_file(), "FLOOR without t0 may create t0");
    let floor = fs::read_to_string(wm.join("FLOOR.md")).unwrap();
    assert!(floor.contains("station: ANDON\n"), "{floor}");
    assert!(floor.contains("card: STOP-ASK INTAKE\n"), "{floor}");
    assert!(floor.contains("wip: s9\n"), "{floor}");
    assert!(floor.contains("andon: STOP-ASK INTAKE\n"), "{floor}");
    assert!(floor.contains("independence: CROSS-FAMILY\n"), "{floor}");
    assert!(floor.contains("  .wm/FALSIFIER\n"), "{floor}");
    assert!(!floor.contains("CLOSED"), "{floor}");
    assert!(!floor.contains("stale"), "{floor}");
}

#[test]
fn status_missing_independence_keeps_the_default() {
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    fs::write(
        wm.join("FLOOR.md"),
        "station: SHAPE\ncard: NEXT INTAKE\nwip: -\nandon: -\nevidence:\n",
    )
    .unwrap();
    let out = bin().current_dir(&tmp.root).arg("status").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let floor = fs::read_to_string(wm.join("FLOOR.md")).unwrap();
    assert!(
        floor.contains("independence: SUBAGENT-ISOLATED\n"),
        "{floor}"
    );
}

#[test]
fn status_without_card_exits_1_and_writes_nothing() {
    let tmp = Tmp::new();
    let out = bin().current_dir(&tmp.root).arg("status").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "no card must exit 1: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"no card on disk\n");
    assert!(
        out.stderr.is_empty(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!tmp.root.join(".wm").exists(), "must not mkdir .wm");

    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    let trace = "when\tcard\toutcome\n2026-09-20T12:00:00Z\tNEXT INTAKE\tSHAPE\n";
    fs::write(wm.join("TRACE.tsv"), trace).unwrap();
    fs::write(wm.join("t0"), "1773964800\n").unwrap();
    let out = bin().current_dir(&tmp.root).arg("status").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, b"no card on disk\n");
    assert!(
        out.stderr.is_empty(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fs::read_to_string(wm.join("TRACE.tsv")).unwrap(), trace);
    assert_eq!(fs::read_to_string(wm.join("t0")).unwrap(), "1773964800\n");
    assert!(!wm.join("FLOOR.md").exists());
    assert!(!wm.join("EVENTS").exists());

    let floor = "station: SHAPE\nwip: -\n";
    fs::write(wm.join("FLOOR.md"), floor).unwrap();
    let out = bin().current_dir(&tmp.root).arg("status").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, b"no card on disk\n");
    assert!(
        out.stderr.is_empty(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fs::read_to_string(wm.join("FLOOR.md")).unwrap(), floor);
    assert_eq!(fs::read_to_string(wm.join("TRACE.tsv")).unwrap(), trace);
    assert!(!wm.join("EVENTS").exists());
}

#[test]
fn status_json_floor_without_t0_stays_read_only() {
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    let floor = "station: SHAPE\ncard: NEXT INTAKE\nwip: -\nandon: -\nindependence: SUBAGENT-ISOLATED\nevidence:\n";
    fs::write(wm.join("FLOOR.md"), floor).unwrap();
    let out = bin()
        .current_dir(&tmp.root)
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fs::read_to_string(wm.join("FLOOR.md")).unwrap(), floor);
    assert!(!wm.join("t0").exists(), "status --json must not create t0");
    assert!(!wm.join("TRACE.tsv").exists());
    assert!(!wm.join("EVENTS").exists());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["available"], true);
    assert!(v["t0_unix"].is_null());
    assert_eq!(v["floor"]["card"], "NEXT INTAKE");
}

#[test]
fn go_without_idea_stop_ask_intake_and_archives_trace() {
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    let previous = "when\tcard\toutcome\n2026-09-20T12:00:00Z\tSTOP-ASK QUESTIONS\tANDON\n";
    fs::write(wm.join("TRACE.tsv"), previous).unwrap();
    let old_t0 = parse_rfc3339_z("2026-09-20T12:00:00Z").unwrap();
    fs::write(wm.join("t0"), format!("{old_t0}\n")).unwrap();
    fs::write(tmp.root.join("crucible"), "#!/bin/sh\n# posix sentinel\n").unwrap();
    fs::write(tmp.root.join("VERSION"), "1.16.4\n").unwrap();

    let out = bin()
        .current_dir(&tmp.root)
        .arg("go")
        .output()
        .expect("run go");
    assert_eq!(
        out.status.code(),
        Some(1),
        "STOP-ASK INTAKE is nonzero: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("STOP-ASK INTAKE"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let floor = fs::read_to_string(wm.join("FLOOR.md")).unwrap();
    assert!(
        floor.contains("card: STOP-ASK INTAKE\n"),
        "FLOOR must be STOP-ASK INTAKE, got:\n{floor}"
    );
    assert!(floor.contains("station: ANDON\n"));

    let archived = wm
        .join("archive")
        .join(format!("TRACE-{old_t0}-2026-09-20T12:00:00Z.tsv"));
    assert!(
        archived.is_file(),
        "go_start must archive previous TRACE before rewrite; archive dir: {:?}",
        fs::read_dir(wm.join("archive")).map(|rd| rd
            .filter_map(|e| e.ok().map(|e| e.file_name()))
            .collect::<Vec<_>>())
    );
    assert_eq!(fs::read_to_string(&archived).unwrap(), previous);

    let live = fs::read_to_string(wm.join("TRACE.tsv")).unwrap();
    assert!(live.starts_with("when\tcard\toutcome\n"));
    assert!(live.contains("STOP-ASK INTAKE"));
    assert!(
        !live.contains("STOP-ASK QUESTIONS"),
        "live TRACE is this run only"
    );

    let events = fs::read_to_string(wm.join("EVENTS")).unwrap();
    assert!(events.contains("\"kind\":\"walk_start\""));
    assert!(events.contains("\"kind\":\"halt\""));
    assert!(events.contains("STOP-ASK INTAKE"));

    let metrics = fs::read_to_string(wm.join("METRICS.tsv")).unwrap();
    assert!(metrics.contains("STOP-ASK INTAKE"));

    assert_eq!(
        fs::read_to_string(tmp.root.join("crucible")).unwrap(),
        "#!/bin/sh\n# posix sentinel\n",
        "must not overwrite POSIX ./crucible"
    );
    assert_eq!(
        fs::read_to_string(tmp.root.join("VERSION")).unwrap().trim(),
        "1.16.4",
        "must not bump VERSION"
    );
    assert!(
        !wm.join("go.pid").exists(),
        "go stays foreground (no pid file)"
    );
}

#[test]
fn go_does_not_overwrite_workspace_posix_or_version() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let posix_before = fs::read(repo.join("crucible")).expect("workspace POSIX ./crucible");
    let ver_before = fs::read_to_string(repo.join("VERSION")).expect("VERSION");
    assert!(
        posix_before.starts_with(b"#!/bin/sh"),
        "workspace ./crucible must remain the POSIX script"
    );
    assert_eq!(ver_before.trim(), "1.23.1");

    let tmp = Tmp::new();
    let _ = bin().current_dir(&tmp.root).arg("go").output().unwrap();

    let posix_after = fs::read(repo.join("crucible")).unwrap();
    let ver_after = fs::read_to_string(repo.join("VERSION")).unwrap();
    assert_eq!(posix_after, posix_before);
    assert_eq!(ver_after, ver_before);
}

#[test]
fn version_flag_prints_product_version() {
    let want = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../VERSION"))
        .expect("VERSION")
        .trim()
        .to_string();
    assert_eq!(want, "1.23.1");
    for flag in ["--version", "-V"] {
        let out = bin().arg(flag).output().unwrap();
        assert!(
            out.status.success(),
            "{flag} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            want,
            "{flag} stdout"
        );
    }
}

#[test]
fn help_lists_go_query_serve_and_room() {
    let out = bin().arg("help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("go"), "help must list go: {stdout}");
    assert!(stdout.contains("status"));
    assert!(stdout.contains("debrief"));
    assert!(stdout.contains("stats"));
    assert!(
        !stdout.contains("no EVENTS"),
        "stats help must not claim it ignores EVENTS: {stdout}"
    );
    assert!(
        stdout.to_ascii_lowercase().contains("serve"),
        "help must list serve: {stdout}"
    );
    assert!(
        stdout.to_ascii_lowercase().contains("room"),
        "help must list room: {stdout}"
    );
    assert!(
        stdout.to_ascii_lowercase().contains("doctor"),
        "help must list doctor: {stdout}"
    );
    assert!(
        stdout.contains(".grok/rules/loop-router.md"),
        "help must name the repo router: {stdout}"
    );
    assert!(
        !stdout.contains("home loop-router"),
        "help must not describe a home loop-router check: {stdout}"
    );

    let tmp = Tmp::new();
    golden_board(&tmp.root);
    let status = bin()
        .current_dir(&tmp.root)
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "query verbs still work: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

fn router_fixture() -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/loop-router.md"),
    )
    .expect("testdata/loop-router.md")
}

fn fixture_adr_hash(text: &str) -> String {
    text.lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("ADR-HASH:")?;
            let token = rest.split_whitespace().next()?;
            (token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()))
                .then(|| token.to_ascii_lowercase())
        })
        .expect("fixture ADR-HASH")
}

fn output_parts(out: &std::process::Output) -> (i32, String, String) {
    (
        out.status.code().unwrap_or(1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn doctor_warns_missing_on_cwd_and_leaves_canary_home() {
    let repo = Tmp::new();
    let canary = Tmp::new();
    fs::write(canary.root.join("marker"), "canary").unwrap();
    let out = bin()
        .current_dir(&repo.root)
        .env("HOME", &canary.root)
        .arg("doctor")
        .output()
        .expect("run doctor");
    let (code, stdout, stderr) = output_parts(&out);
    assert_eq!(code, 0, "doctor warn is not a walk CHECK: stderr={stderr}");
    let router = repo.root.join(".grok/rules/loop-router.md");
    assert!(
        stdout.contains("warn:") && stdout.contains("missing"),
        "stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.contains(router.to_string_lossy().as_ref()),
        "must report cwd router path: {stdout}"
    );
    assert!(!stdout.contains("~/.grok"), "{stdout}");
    assert_eq!(
        fs::read_to_string(canary.root.join("marker")).unwrap(),
        "canary"
    );
    assert!(fs::symlink_metadata(canary.root.join(".grok")).is_err());
}

#[test]
fn doctor_warns_when_cwd_grok_is_symlink_and_leaves_canary_home() {
    let repo = Tmp::new();
    let canary = Tmp::new();
    fs::write(canary.root.join("marker"), "canary").unwrap();
    let sentinel = repo.root.join("sentinel");
    fs::write(&sentinel, "sentinel-bytes").unwrap();
    std::os::unix::fs::symlink(&sentinel, repo.root.join(".grok")).unwrap();
    let out = bin()
        .current_dir(&repo.root)
        .env("HOME", &canary.root)
        .arg("doctor")
        .output()
        .expect("run doctor");
    let (code, stdout, stderr) = output_parts(&out);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(
        stdout.contains("parent is a symlink"),
        "stdout={stdout:?} stderr={stderr:?}"
    );
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "sentinel-bytes");
    assert_eq!(
        fs::read_to_string(canary.root.join("marker")).unwrap(),
        "canary"
    );
    assert!(fs::symlink_metadata(canary.root.join(".grok")).is_err());
}

#[test]
fn doctor_home_writes_fixture_under_its_own_home() {
    use std::os::unix::fs::PermissionsExt;
    let home = Tmp::new();
    let cwd = Tmp::new();
    let fixture = router_fixture();
    let hash = fixture_adr_hash(&fixture);
    let out = bin()
        .current_dir(&cwd.root)
        .env("HOME", &home.root)
        .args(["doctor", "--home"])
        .output()
        .expect("run doctor --home");
    let (code, stdout, stderr) = output_parts(&out);
    assert_eq!(code, 0, "stdout={stdout:?} stderr={stderr:?}");
    let path = home.root.join(".grok/rules/loop-router.md");
    let line = format!(
        "ok: home loop-router matches ADR-HASH {hash} ({})",
        path.display()
    );
    assert!(stdout.contains(&line), "stdout={stdout:?}");
    assert!(!stdout.contains("warn:"), "{stdout}");
    assert_eq!(fs::read_to_string(&path).unwrap(), fixture);
    let meta = fs::symlink_metadata(&path).unwrap();
    assert!(meta.file_type().is_file());
    assert!(!meta.file_type().is_symlink());
    assert_eq!(meta.permissions().mode() & 0o777, 0o644);
    let names: Vec<_> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|ent| ent.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("loop-router.md")]);
    assert!(fs::symlink_metadata(cwd.root.join(".grok")).is_err());
}

#[test]
fn doctor_home_unset_does_not_check_or_write() {
    let cwd = Tmp::new();
    let canary = Tmp::new();
    fs::write(canary.root.join("marker"), "canary").unwrap();
    let out = bin()
        .current_dir(&cwd.root)
        .env_remove("HOME")
        .args(["doctor", "--home"])
        .output()
        .expect("run doctor --home");
    let (code, stdout, stderr) = output_parts(&out);
    assert_eq!(code, 1, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stdout, "warn: home loop-router not written (HOME unset)\n");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(!stdout.contains("copy testdata"));
    assert!(fs::symlink_metadata(cwd.root.join(".grok")).is_err());
    assert_eq!(
        fs::read_to_string(canary.root.join("marker")).unwrap(),
        "canary"
    );
    assert!(fs::symlink_metadata(canary.root.join(".grok")).is_err());
}

#[test]
fn doctor_home_refuses_parent_symlink() {
    let home = Tmp::new();
    let cwd = Tmp::new();
    let sentinel = Tmp::new();
    fs::write(sentinel.root.join("marker"), "sentinel").unwrap();
    std::os::unix::fs::symlink(&sentinel.root, home.root.join(".grok")).unwrap();
    let out = bin()
        .current_dir(&cwd.root)
        .env("HOME", &home.root)
        .args(["doctor", "--home"])
        .output()
        .expect("run doctor --home");
    let (code, stdout, stderr) = output_parts(&out);
    assert_eq!(code, 1, "stdout={stdout:?} stderr={stderr:?}");
    assert!(stdout.contains("not written"), "{stdout}");
    assert!(stdout.contains("parent is a symlink"), "{stdout}");
    assert!(!stdout.contains("ok:"), "{stdout}");
    assert_eq!(
        fs::read_to_string(sentinel.root.join("marker")).unwrap(),
        "sentinel"
    );
    assert!(fs::symlink_metadata(sentinel.root.join("rules")).is_err());
    assert!(fs::symlink_metadata(sentinel.root.join("loop-router.md")).is_err());
}

#[test]
fn doctor_unknown_args_name_the_reported_token() {
    let cwd = Tmp::new();
    let nope = bin()
        .current_dir(&cwd.root)
        .env_remove("HOME")
        .args(["doctor", "--nope"])
        .output()
        .expect("run doctor --nope");
    let (code, stdout, stderr) = output_parts(&nope);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stderr, "doctor: unknown arg --nope\n");
    assert!(stdout.is_empty(), "{stdout}");

    let extra = bin()
        .current_dir(&cwd.root)
        .env_remove("HOME")
        .args(["doctor", "--home", "extra"])
        .output()
        .expect("run doctor --home extra");
    let (code, stdout, stderr) = output_parts(&extra);
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert_eq!(stderr, "doctor: unknown arg extra\n");
    assert!(stdout.is_empty(), "{stdout}");
}

#[test]
fn go_empty_idea_stop_ask_intake() {
    let tmp = Tmp::new();
    fs::write(tmp.root.join("IDEA.md"), "").unwrap();
    let out = bin().current_dir(&tmp.root).arg("go").output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "zero-byte IDEA.md must STOP-ASK INTAKE: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("STOP-ASK INTAKE"));
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: STOP-ASK INTAKE\n"));
    assert!(floor.contains("station: ANDON\n"));
}

#[test]
fn go_idea_without_closed_refuses_brick_loop() {
    let tmp = Tmp::new();
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    let out = bin()
        .current_dir(&tmp.root)
        .env_remove("CRUCIBLE_RED_PROGRAM")
        .env_remove("CRUCIBLE_RED_ARGS")
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("go: brick loop not ported"),
        "stderr={stderr:?} stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(!tmp.root.join(".wm/go.pid").exists());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
}

#[test]
fn go_idea_questions_without_answers_stop_ask() {
    let tmp = Tmp::new();
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    fs::write(tmp.root.join("QUESTIONS.md"), "What should we build?\n").unwrap();
    let out = bin()
        .current_dir(&tmp.root)
        .env_remove("CRUCIBLE_RED_PROGRAM")
        .env_remove("CRUCIBLE_RED_ARGS")
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "QUESTIONS without ANSWERS must STOP-ASK: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("STOP-ASK QUESTIONS"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: STOP-ASK QUESTIONS\n"), "{floor}");
    assert!(floor.contains("station: ANDON\n"), "{floor}");
    assert!(!tmp.root.join("ANSWERS.md").exists());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_questions_need_ask_beats_injected_red_program() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    fs::write(tmp.root.join("QUESTIONS.md"), "What should we build?\n").unwrap();
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "QUESTIONS must win over CRUCIBLE_RED_PROGRAM: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("STOP-ASK QUESTIONS"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: STOP-ASK QUESTIONS\n"), "{floor}");
    assert!(floor.contains("station: ANDON\n"), "{floor}");
    assert!(
        !floor.contains("NEXT "),
        "must not proceed to NEXT RED: {floor}"
    );
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(
        !wt.join(".git").is_file(),
        "next_red must not mint when QUESTIONS need ask"
    );
    assert!(!tmp.root.join(".wm/FALSIFIER").is_file());
    assert!(!wt.join("marker").exists());
    assert!(!tmp.root.join("ANSWERS.md").exists());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

fn plant_high_ready(dir: &Path) {
    fs::write(
        dir.join("slices.tsv"),
        "id\tmodule\towned_paths\tdepends_on\trisk\tstatus\n\
s1\twidget\tsrc/widget/api.py\t-\tHIGH\tREADY\n",
    )
    .unwrap();
    fs::write(dir.join("MAP.md"), "slice s1 is HIGH\n").unwrap();
}

fn sha256_file(path: &Path) -> String {
    for (bin, args) in [
        ("sha256sum", &[] as &[&str]),
        ("shasum", &["-a", "256"]),
        ("openssl", &["dgst", "-sha256"]),
    ] {
        let mut cmd = Command::new(bin);
        cmd.args(args).arg(path);
        if let Ok(out) = cmd.output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(hex) = s
                    .split_whitespace()
                    .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
                {
                    return hex.to_ascii_lowercase();
                }
            }
        }
    }
    panic!("no sha256 tool for {}", path.display());
}

fn write_valid_map_human(dir: &Path) {
    let hash = sha256_file(&dir.join("MAP.md"));
    fs::write(
        dir.join("MAP-HUMAN"),
        format!("SIGNED: operator\nMAP: MAP.md\nSHA256: {hash}\n"),
    )
    .unwrap();
}

fn plant_map_verdict(dir: &Path, word: &str) {
    fs::create_dir_all(dir.join(".wm")).unwrap();
    fs::write(
        dir.join(".wm").join("map-verdict"),
        format!("WORD: {word}\nAGENT: bob\nMAP: MAP.md\nwhen: 2026-09-21T00:00:00Z\n"),
    )
    .unwrap();
}

fn plant_panel(
    dir: &Path,
    maker: &str,
    maker_kind: &str,
    maker_cmd: &str,
    reviewer: &str,
    reviewer_kind: &str,
    reviewer_cmd: &str,
) {
    fs::create_dir_all(dir.join(".wm")).unwrap();
    fs::write(
        dir.join(".wm").join("PANEL.tsv"),
        format!(
            "maker\t{maker}\t{maker_kind}\t{maker_cmd}\nreviewer\t{reviewer}\t{reviewer_kind}\t{reviewer_cmd}\n"
        ),
    )
    .unwrap();
}

#[test]
fn go_map_human_missing_beats_injected_red_program() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    plant_high_ready(&tmp.root);
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "MAP-HUMAN must win over CRUCIBLE_RED_PROGRAM: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("STOP-ASK MAP-HUMAN"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: STOP-ASK MAP-HUMAN\n"), "{floor}");
    assert!(floor.contains("station: ANDON\n"), "{floor}");
    assert!(
        !floor.contains("NEXT "),
        "must not proceed to NEXT RED: {floor}"
    );
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(
        !wt.join(".git").is_file(),
        "next_red must not mint when MAP-HUMAN is missing"
    );
    assert!(!tmp.root.join(".wm/FALSIFIER").is_file());
    assert!(!wt.join("marker").exists());
    assert!(
        !tmp.root.join("MAP-HUMAN").exists(),
        "must not invent MAP-HUMAN"
    );
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_map_human_present_continues_to_injected_red() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    plant_high_ready(&tmp.root);
    write_valid_map_human(&tmp.root);
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "valid MAP-HUMAN must continue to red: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("NEXT RED"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: NEXT RED\n"), "{floor}");
    assert!(floor.contains("station: BUILD\n"), "{floor}");
    assert!(!floor.contains("STOP-ASK MAP-HUMAN"), "{floor}");
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(wt.join("marker").is_file());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_map_human_present_continues_to_brick_refuse() {
    let tmp = Tmp::new();
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    plant_high_ready(&tmp.root);
    write_valid_map_human(&tmp.root);
    let out = bin()
        .current_dir(&tmp.root)
        .env_remove("CRUCIBLE_RED_PROGRAM")
        .env_remove("CRUCIBLE_RED_ARGS")
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("go: brick loop not ported"),
        "stderr={stderr:?} stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(!tmp.root.join(".wm/go.pid").exists());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
}

#[test]
fn go_independence_same_agent_beats_injected_red_program() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    plant_panel(
        &tmp.root,
        "carol",
        "grok",
        "./tools/maker.sh",
        "carol",
        "grok",
        "./tools/reviewer.sh",
    );
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "same-agent panel must win over CRUCIBLE_RED_PROGRAM: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("INDEPENDENCE_UNAVAILABLE"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !stdout.contains("CROSS-FAMILY"),
        "must not print CROSS-FAMILY as the card: {stdout:?}"
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(
        floor.contains("card: INDEPENDENCE_UNAVAILABLE\n"),
        "{floor}"
    );
    assert!(floor.contains("station: ANDON\n"), "{floor}");
    assert!(
        !floor.contains("NEXT "),
        "must not proceed to NEXT RED: {floor}"
    );
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(
        !wt.join(".git").is_file(),
        "next_red must not mint when independence refuses"
    );
    assert!(!tmp.root.join(".wm/FALSIFIER").is_file());
    assert!(!wt.join("marker").exists());
    assert!(!tmp.root.join("PANEL.ASSIGN.tsv").exists());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_independence_one_kind_continues_to_injected_red() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    plant_panel(
        &tmp.root,
        "carol",
        "grok",
        "./tools/maker.sh",
        "dave",
        "grok",
        "./tools/reviewer.sh",
    );
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "one-kind panel must continue to red: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("NEXT RED"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!stdout.contains("CROSS-FAMILY"), "{stdout:?}");
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: NEXT RED\n"), "{floor}");
    assert!(floor.contains("station: BUILD\n"), "{floor}");
    assert!(!floor.contains("INDEPENDENCE_UNAVAILABLE"), "{floor}");
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(wt.join("marker").is_file());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_idea_env_red_program_floor_next_red_build() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "injected red program: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: NEXT RED\n"), "{floor}");
    assert!(floor.contains("station: BUILD\n"), "{floor}");
    assert!(!floor.to_ascii_lowercase().contains("grok"));
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(
        wt.join("marker").is_file(),
        "child must write marker in minted cwd"
    );
    assert!(!tmp.root.join("marker").exists());
    assert_eq!(
        fs::read_to_string(tmp.root.join(".wm/FALSIFIER"))
            .unwrap()
            .trim(),
        "FAIL"
    );
    assert!(
        !tmp.root.join(".wm/CLOSED").exists(),
        "must not auto close_walk"
    );
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_second_brick_does_not_floor_next_red_from_leftover_falsifier() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    let sh = posix_tool("sh");
    let args =
        "-c\nif [ -f .wm/red-once ]; then pwd > marker2; else echo FAIL > .wm/FALSIFIER; echo 1 > .wm/red-once; pwd > marker; fi";

    let first = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env("CRUCIBLE_RED_ARGS", args)
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        first.status.code(),
        Some(0),
        "first go still NEXT RED when child writes FALSIFIER: stderr={} stdout={}",
        String::from_utf8_lossy(&first.stderr),
        String::from_utf8_lossy(&first.stdout)
    );
    let stdout1 = String::from_utf8_lossy(&first.stdout);
    assert!(
        stdout1.contains("NEXT RED"),
        "stdout={stdout1:?} stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );
    let floor1 = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor1.contains("card: NEXT RED\n"), "{floor1}");
    assert!(floor1.contains("station: BUILD\n"), "{floor1}");
    assert_eq!(
        fs::read_to_string(tmp.root.join(".wm/FALSIFIER"))
            .unwrap()
            .trim(),
        "FAIL"
    );

    let second = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env("CRUCIBLE_RED_ARGS", args)
        .arg("go")
        .output()
        .unwrap();
    let stdout2 = String::from_utf8_lossy(&second.stdout);
    let stderr2 = String::from_utf8_lossy(&second.stderr);
    let floor2 = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(
        !floor2.contains("card: NEXT RED\n"),
        "second go must not floor NEXT RED from leftover FALSIFIER: {floor2}"
    );
    assert!(
        !floor2.contains("station: BUILD\n"),
        "second go must not floor BUILD from leftover FALSIFIER: {floor2}"
    );
    assert!(
        floor2.contains("station: ANDON\n"),
        "missing FALSIFIER this brick is ANDON: {floor2}"
    );
    assert!(
        stdout2.contains("STOP-ASK red refused"),
        "POSIX missing-falsifier path: stdout={stdout2:?} stderr={stderr2:?} floor={floor2}"
    );
    assert!(floor2.contains("card: STOP-ASK red refused\n"), "{floor2}");
    assert!(
        !tmp.root.join(".wm/FALSIFIER").is_file(),
        "second child writes no FALSIFIER; leftover must not remain as this run's success"
    );
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(
        !wt.join("marker").is_file(),
        "next brick must not reuse a live s1 worktree (first child's marker would remain)"
    );
    assert!(
        wt.join("marker2").is_file(),
        "second child must run in a minted cwd"
    );
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_next_is_not_ported() {
    let tmp = Tmp::new();
    let out = bin()
        .current_dir(&tmp.root)
        .args(["go", "--next"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("go --next is not ported"),
        "stderr={stderr:?}"
    );
    assert!(
        !tmp.root.join(".wm").exists(),
        "--next refuse is argv-only; must not go_start"
    );
}

#[test]
fn go_closed_with_idea_is_foreground_noop() {
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    fs::write(tmp.root.join("IDEA.md"), "ship the intake receipt\n").unwrap();
    fs::write(
        wm.join("CLOSED"),
        "CLOSED PASS\nindependence: SUBAGENT-ISOLATED\n",
    )
    .unwrap();
    let out = bin().current_dir(&tmp.root).arg("go").output().unwrap();
    assert!(
        out.status.success(),
        "already CLOSED is a no-op, not a hang: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("CLOSED PASS"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(wm.join("FLOOR.md")).unwrap();
    assert!(
        floor.contains("card: CLOSED PASS\n"),
        "CLI FLOOR must be CLOSED PASS, got:\n{floor}"
    );
    assert!(
        !wm.join("METRICS.tsv").is_file(),
        "CLOSED no-op must not write METRICS halt"
    );
    let events = fs::read_to_string(wm.join("EVENTS")).unwrap_or_default();
    assert!(
        !events.contains("\"kind\":\"halt\""),
        "CLOSED no-op must not write EVENTS halt: {events}"
    );
    assert!(!wm.join("go.pid").exists());
}

#[test]
fn go_map_revise_under_cap_beats_injected_red_program() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    plant_map_verdict(&tmp.root, "MAP-REVISE");
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "MAP-REVISE under cap must win over CRUCIBLE_RED_PROGRAM: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("STOP-ASK NEXT MAP"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: STOP-ASK NEXT MAP\n"), "{floor}");
    assert!(floor.contains("station: ANDON\n"), "{floor}");
    assert!(
        !floor.contains("NEXT RED"),
        "must not proceed to NEXT RED: {floor}"
    );
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(
        !wt.join(".git").is_file(),
        "next_red must not mint when MAP-REVISE under cap"
    );
    assert!(!tmp.root.join(".wm/FALSIFIER").is_file());
    assert!(!wt.join("marker").exists());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_map_revise_at_cap_escalates_beats_injected_red_program() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    plant_map_verdict(&tmp.root, "MAP-REVISE");
    fs::write(tmp.root.join(".wm").join("map-revise-count"), "2\n").unwrap();
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "MAP-REVISE at cap must win over CRUCIBLE_RED_PROGRAM: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ESCALATE MAP_REVISE"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: ESCALATE MAP_REVISE\n"), "{floor}");
    assert!(floor.contains("station: ANDON\n"), "{floor}");
    assert!(
        !floor.contains("NEXT RED"),
        "must not proceed to NEXT RED: {floor}"
    );
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(
        !wt.join(".git").is_file(),
        "next_red must not mint when MAP-REVISE is at cap"
    );
    assert!(!tmp.root.join(".wm/FALSIFIER").is_file());
    assert!(!wt.join("marker").exists());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_map_accept_continues_to_injected_red() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    plant_map_verdict(&tmp.root, "MAP-ACCEPT");
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "MAP-ACCEPT must continue to red: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("NEXT RED"),
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: NEXT RED\n"), "{floor}");
    assert!(floor.contains("station: BUILD\n"), "{floor}");
    assert!(!floor.contains("STOP-ASK NEXT MAP"), "{floor}");
    assert!(!floor.contains("ESCALATE MAP_REVISE"), "{floor}");
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(wt.join("marker").is_file());
    assert_eq!(
        fs::read_to_string(tmp.root.join(".wm/FALSIFIER"))
            .unwrap()
            .trim(),
        "FAIL"
    );
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_map_stop_ask_beats_injected_red_program() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    plant_map_verdict(&tmp.root, "MAP-STOP-ASK");
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "MAP-STOP-ASK must win over CRUCIBLE_RED_PROGRAM: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout,
        "STOP-ASK\n",
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor.contains("card: STOP-ASK\n"), "{floor}");
    assert!(floor.contains("station: ANDON\n"), "{floor}");
    assert!(
        !floor.contains("NEXT RED"),
        "must not proceed to NEXT RED: {floor}"
    );
    assert!(!floor.contains("STOP-ASK NEXT MAP"), "{floor}");
    assert!(!floor.contains("ESCALATE MAP_REVISE"), "{floor}");
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(
        !wt.join(".git").is_file(),
        "next_red must not mint when MAP-STOP-ASK"
    );
    assert!(
        !tmp.root.join(".wm/FALSIFIER").is_file(),
        "injected red program must not write FALSIFIER on MAP-STOP-ASK"
    );
    assert!(!wt.join("marker").exists());
    assert!(!tmp.root.join(".wm/map-revise-count").exists());
    assert!(!tmp.root.join(".wm/CLOSED").exists());
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn go_env_red_success_no_build_cannot_skip_inspect_halt() {
    let tmp = Tmp::new();
    init_git_product(&tmp.root);
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    let sh = posix_tool("sh");
    let out = bin()
        .current_dir(&tmp.root)
        .env("CRUCIBLE_RED_PROGRAM", &sh)
        .env(
            "CRUCIBLE_RED_ARGS",
            "-c\necho FAIL > .wm/FALSIFIER; echo no-build > .wm/red.status; pwd > marker",
        )
        .arg("go")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "env red success must not skip inspect halt: stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout,
        "STOP-ASK NEXT RUN reviewer\n",
        "stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let floor = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(
        floor.contains("card: STOP-ASK NEXT RUN reviewer\n"),
        "{floor}"
    );
    assert!(floor.contains("station: ANDON\n"), "{floor}");
    assert!(
        !floor.contains("card: NEXT RED\n"),
        "final FLOOR must not stay BUILD: {floor}"
    );
    let wt = tmp.root.join(".wm/worktrees/s1");
    assert!(
        wt.join(".git").is_file(),
        "inspect halt must keep the minted worktree: {}",
        wt.display()
    );
    assert!(wt.join("marker").is_file());
    assert_eq!(
        fs::read_to_string(tmp.root.join(".wm/FALSIFIER"))
            .unwrap()
            .trim(),
        "FAIL"
    );
    assert_eq!(
        fs::read_to_string(tmp.root.join(".wm/red.status"))
            .unwrap()
            .trim(),
        "no-build"
    );
    assert!(
        !tmp.root.join(".wm/CLOSED").exists(),
        "must not auto close_walk"
    );
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn cargo_tree_has_no_herdr_grok_engos() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("cargo")
        .args(["tree", "--workspace", "-e", "normal"])
        .current_dir(&root)
        .output()
        .expect("cargo tree");
    assert!(
        out.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
    assert!(!tree.contains("herdr"), "herdr in cargo tree:\n{tree}");
    assert!(!tree.contains("grok"), "grok in cargo tree:\n{tree}");
    assert!(!tree.contains("engos"), "engos in cargo tree:\n{tree}");
    for pkg in ["crucible-kernel", "crucible-contract"] {
        let out = Command::new("cargo")
            .args(["tree", "-p", pkg, "-e", "normal"])
            .current_dir(&root)
            .output()
            .expect("cargo tree -p");
        assert!(
            out.status.success(),
            "cargo tree -p {pkg}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let tree = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
        assert!(
            !tree.contains("herdr"),
            "herdr in {pkg} cargo tree:\n{tree}"
        );
    }
}

struct ServeProc {
    child: Child,
    addr: String,
}

impl Drop for ServeProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_serve(dir: &Path, bind: &str) -> ServeProc {
    let mut child = bin()
        .current_dir(dir)
        .args(["serve", "--bind", bind])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdout = child.stdout.take().expect("serve stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut buf = [0u8; 256];
        match stdout.read(&mut buf) {
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
            if let Some(mut s) = child.stderr.take() {
                let _ = s.read_to_string(&mut err);
            }
            let st = child.try_wait();
            panic!("serve did not print listening: stderr={err:?} status={st:?}");
        }
    };
    let addr = line
        .lines()
        .find_map(|l| l.strip_prefix("listening "))
        .unwrap_or(line.trim())
        .trim()
        .to_string();
    assert!(
        addr.starts_with("127.0.0.1:") || addr.starts_with("[::1]:"),
        "bind must be loopback: {line:?}"
    );
    ServeProc { child, addr }
}

fn start_web(dir: &Path) -> ServeProc {
    let mut child = bin()
        .current_dir(dir)
        .env("CRUCIBLE_ROOT", dir)
        .args(["web", "--bind", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn web");
    let mut stdout = child.stdout.take().expect("web stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut buf = [0u8; 256];
        match stdout.read(&mut buf) {
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
            if let Some(mut s) = child.stderr.take() {
                let _ = s.read_to_string(&mut err);
            }
            let st = child.try_wait();
            panic!("web did not print listening: stderr={err:?} status={st:?}");
        }
    };
    let addr = line
        .lines()
        .find_map(|l| l.strip_prefix("listening "))
        .unwrap_or(line.trim())
        .trim()
        .to_string();
    assert!(
        addr.starts_with("127.0.0.1:") || addr.starts_with("[::1]:"),
        "bind must be loopback: {line:?}"
    );
    ServeProc { child, addr }
}

#[test]
fn web_debrief_missing_floor_body_is_the_refusal() {
    fn post(addr: &str) -> (u16, String, Vec<u8>) {
        let sock: std::net::SocketAddr = addr.parse().expect("bind addr");
        let body = br#"{"args":[]}"#;
        let mut req = format!(
            "POST /act/debrief HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Crucible-Act: 1\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        req.extend_from_slice(body);
        let mut raw = Vec::new();
        let mut last_err = String::new();
        for _ in 0..50 {
            match TcpStream::connect_timeout(&sock, Duration::from_millis(100)) {
                Ok(mut s) => {
                    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                    if s.write_all(&req).is_err() {
                        last_err = "write".to_string();
                        thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    raw.clear();
                    match s.read_to_end(&mut raw) {
                        Ok(_) => {}
                        Err(e) if !raw.is_empty() => last_err = e.to_string(),
                        Err(e) => {
                            last_err = e.to_string();
                            thread::sleep(Duration::from_millis(20));
                            continue;
                        }
                    }
                    if raw.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    last_err = "short response".to_string();
                }
                Err(e) => {
                    last_err = e.to_string();
                    thread::sleep(Duration::from_millis(20));
                }
            }
        }
        let sep = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap_or_else(|| panic!("debrief act response: {last_err} raw={raw:?}"));
        let head = String::from_utf8_lossy(&raw[..sep]).into_owned();
        let status = head
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        (status, head, raw[sep + 4..].to_vec())
    }

    let tmp = Tmp::new();
    let srv = start_web(&tmp.root);
    let (status, headers, body) = post(&srv.addr);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert!(
        headers.lines().any(|l| l == "X-Crucible-Exit: 1"),
        "{headers}"
    );
    assert_eq!(body, b"no FLOOR.md (run go or status)\n");
    assert!(!tmp.root.join(".wm").exists());

    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    let floor = b"any\n";
    fs::write(wm.join("FLOOR.md"), floor).unwrap();
    let (status, headers, body) = post(&srv.addr);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert!(
        headers.lines().any(|l| l == "X-Crucible-Exit: 1"),
        "{headers}"
    );
    assert_eq!(body, b"no TRACE.tsv\n");
    assert_eq!(fs::read(wm.join("FLOOR.md")).unwrap(), floor);
    assert!(!wm.join("TRACE.tsv").exists());
}

fn post_act(addr: &str, path: &str, json: &str) -> (u16, String, Vec<u8>) {
    let sock: std::net::SocketAddr = addr.parse().expect("bind addr");
    let body = json.as_bytes();
    let mut req = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Crucible-Act: 1\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    req.extend_from_slice(body);
    let mut raw = Vec::new();
    let mut last_err = String::new();
    for _ in 0..50 {
        match TcpStream::connect_timeout(&sock, Duration::from_millis(100)) {
            Ok(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                if s.write_all(&req).is_err() {
                    last_err = "write".to_string();
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
                raw.clear();
                match s.read_to_end(&mut raw) {
                    Ok(_) => {}
                    Err(e) if !raw.is_empty() => last_err = e.to_string(),
                    Err(e) => {
                        last_err = e.to_string();
                        thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                }
                if raw.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
                last_err = "short response".to_string();
            }
            Err(e) => {
                last_err = e.to_string();
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    let sep = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("{path} response: {last_err} raw={raw:?}"));
    let head = String::from_utf8_lossy(&raw[..sep]).into_owned();
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    (status, head, raw[sep + 4..].to_vec())
}

#[test]
fn web_bare_status_rewrites_floor_and_json_does_not() {
    let tmp = Tmp::new();
    golden_board(&tmp.root);
    let trace = fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap();
    let floor_before = fs::read(tmp.root.join(".wm/FLOOR.md")).unwrap();
    let srv = start_web(&tmp.root);
    let (status, headers, body) = post_act(&srv.addr, "/act/status", r#"{"args":[]}"#);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert!(
        headers.lines().any(|l| l == "X-Crucible-Exit: 0"),
        "{headers}"
    );
    let text = String::from_utf8_lossy(&body);
    assert!(text.starts_with("FLOOR t=+"), "{text:?}");
    let floor_after = fs::read(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert_ne!(floor_after, floor_before, "bare status must rewrite FLOOR");
    assert!(
        String::from_utf8_lossy(&floor_after).contains("card: STOP-ASK INTAKE\n"),
        "{floor_after:?}"
    );
    assert_eq!(fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap(), trace);
    let (status, headers, body) = post_act(&srv.addr, "/act/status", r#"{"args":["--json"]}"#);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert!(
        headers.lines().any(|l| l == "X-Crucible-Exit: 0"),
        "{headers}"
    );
    assert_eq!(
        fs::read(tmp.root.join(".wm/FLOOR.md")).unwrap(),
        floor_after
    );
    assert_eq!(fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap(), trace);
    let (status, headers, body) = post_act(&srv.addr, "/act/status", r#"{"args":["--json","x"]}"#);
    assert_eq!(status, 404, "{headers} body={body:?}");
    assert_eq!(body, b"not found\n");
    assert_eq!(
        fs::read(tmp.root.join(".wm/FLOOR.md")).unwrap(),
        floor_after
    );
    assert_eq!(fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap(), trace);
}

#[test]
fn web_status_without_card_body_is_the_refusal() {
    let tmp = Tmp::new();
    let srv = start_web(&tmp.root);
    let (status, headers, body) = post_act(&srv.addr, "/act/status", r#"{"args":[]}"#);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert!(
        headers.lines().any(|l| l == "X-Crucible-Exit: 1"),
        "{headers}"
    );
    assert_eq!(body, b"no card on disk\n");
    assert!(!tmp.root.join(".wm").join("FLOOR.md").exists());
    assert!(!tmp.root.join(".wm").exists());
}

#[test]
fn web_close_without_slug_body_is_the_refusal() {
    let tmp = Tmp::new();
    let srv = start_web(&tmp.root);
    let started = Instant::now();
    let (status, headers, body) = post_act(&srv.addr, "/act/close", r#"{"args":[]}"#);
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "close waited {:?}",
        started.elapsed()
    );
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert_ne!(status, 504);
    assert!(
        headers.lines().any(|l| l == "X-Crucible-Exit: 2"),
        "{headers}"
    );
    assert_eq!(body, b"crucible: need a slug\n");
    assert!(!tmp.root.join("items").exists());
}

fn entry_names(dir: &Path) -> Vec<String> {
    let mut names = fs::read_dir(dir)
        .unwrap()
        .map(|ent| ent.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn assert_exit(headers: &str, code: &str) {
    assert!(
        headers
            .lines()
            .any(|l| l == format!("X-Crucible-Exit: {code}")),
        "{headers}"
    );
}

const STATE_HEADER_LINE: &str =
    "item\tstatus\tstage\twork_id\trisk\tinflight_attempt\tblock_code\tupdated_epoch\n";

#[test]
fn web_lifecycle_status_writes_nothing() {
    let tmp = Tmp::new();
    let srv = start_web(&tmp.root);
    let (status, headers, body) = post_act(&srv.addr, "/act/lifecycle", r#"{"args":["status"]}"#);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert_exit(&headers, "0");
    assert_eq!(body, b"lifecycle: item-file\n");
    assert!(!tmp.root.join("STATE.tsv").exists());
    assert!(entry_names(&tmp.root).is_empty());
}

#[test]
fn web_lifecycle_dry_run_writes_nothing() {
    let tmp = Tmp::new();
    let program = b"program: work\n";
    fs::write(tmp.root.join("PROGRAM"), program).unwrap();
    let srv = start_web(&tmp.root);
    let (status, headers, body) = post_act(
        &srv.addr,
        "/act/lifecycle",
        r#"{"args":["enable","--dry-run"]}"#,
    );
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert_exit(&headers, "0");
    let text = String::from_utf8(body).unwrap();
    let root = tmp.root.display().to_string();
    assert!(
        text.contains(&format!("CREATE {root}/STATE.tsv\n")),
        "{text}"
    );
    assert!(
        text.contains(&format!("REPLACE {root}/STATE.md\n")),
        "{text}"
    );
    assert!(
        text.contains(&format!("UPDATE {root}/PROGRAM lifecycle: managed\n")),
        "{text}"
    );
    assert!(!text.contains("enabled managed lifecycle\n"), "{text}");
    assert!(!tmp.root.join("STATE.tsv").exists());
    assert!(!tmp.root.join("STATE.md").exists());
    assert!(!tmp.root.join(".state.lock").exists());
    assert_eq!(fs::read(tmp.root.join("PROGRAM")).unwrap(), program);
    assert_eq!(entry_names(&tmp.root), vec!["PROGRAM".to_string()]);
}

#[test]
fn web_target_and_brief_empty_args_write_nothing() {
    let tmp = Tmp::new();
    let srv = start_web(&tmp.root);
    for path in ["/act/target", "/act/brief"] {
        let (status, headers, body) = post_act(&srv.addr, path, r#"{"args":[]}"#);
        assert_eq!(status, 200, "{path} {headers} body={body:?}");
        assert_exit(&headers, "2");
        assert!(body.is_empty(), "{path} stdout must stay empty: {body:?}");
    }
    assert!(!tmp.root.join("TARGET").exists());
    assert!(!tmp.root.join("MAKER").exists());
    assert!(!tmp.root.join("briefs").exists());
    assert!(entry_names(&tmp.root).is_empty());
}

#[test]
fn web_state_unmanaged_writes_nothing() {
    let tmp = Tmp::new();
    let srv = start_web(&tmp.root);
    let (status, headers, body) = post_act(&srv.addr, "/act/state", r#"{"args":[]}"#);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert_exit(&headers, "2");
    assert!(body.is_empty(), "{body:?}");
    assert!(!tmp.root.join(".state.lock").exists());
    assert!(!tmp.root.join("STATE.md").exists());
    assert!(entry_names(&tmp.root).is_empty());

    let program = b"program: work\n";
    fs::write(tmp.root.join("PROGRAM"), program).unwrap();
    let (status, headers, body) = post_act(&srv.addr, "/act/state", r#"{"args":[]}"#);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert_exit(&headers, "2");
    assert!(body.is_empty(), "{body:?}");
    assert!(!tmp.root.join(".state.lock").exists());
    assert!(!tmp.root.join("STATE.md").exists());
    assert_eq!(fs::read(tmp.root.join("PROGRAM")).unwrap(), program);
    assert_eq!(entry_names(&tmp.root), vec!["PROGRAM".to_string()]);
}

#[test]
fn web_state_managed_rewrites_state_md() {
    let tmp = Tmp::new();
    let program = b"lifecycle: managed\n";
    fs::write(tmp.root.join("PROGRAM"), program).unwrap();
    fs::write(tmp.root.join("STATE.tsv"), STATE_HEADER_LINE).unwrap();
    fs::write(tmp.root.join("STATE.md"), b"old\n").unwrap();
    let srv = start_web(&tmp.root);
    let (status, headers, body) = post_act(&srv.addr, "/act/state", r#"{"args":[]}"#);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert_exit(&headers, "0");
    let root = tmp.root.display().to_string();
    assert_eq!(body, format!("{root}/STATE.md\n").into_bytes());
    let rendered = fs::read_to_string(tmp.root.join("STATE.md")).unwrap();
    assert_ne!(rendered, "old\n");
    assert!(
        rendered.contains("Generated from `STATE.tsv`"),
        "{rendered}"
    );
    assert_eq!(
        fs::read(tmp.root.join("STATE.tsv")).unwrap(),
        STATE_HEADER_LINE.as_bytes()
    );
    assert_eq!(fs::read(tmp.root.join("PROGRAM")).unwrap(), program);
    assert!(!tmp.root.join(".state.lock").exists());
    assert!(!tmp.root.join("items").exists());
    assert!(!tmp.root.join("TARGET").exists());
    assert!(!tmp.root.join("MAKER").exists());
    assert!(!tmp.root.join("briefs").exists());
    for name in entry_names(&tmp.root) {
        assert!(
            !(name.starts_with(".STATE.md.") && name.ends_with(".tmp")),
            "{name}"
        );
    }
    assert_eq!(
        entry_names(&tmp.root),
        vec![
            "PROGRAM".to_string(),
            "STATE.md".to_string(),
            "STATE.tsv".to_string(),
        ]
    );
}

#[test]
fn web_lifecycle_enable_apply_writes_managed_files() {
    let tmp = Tmp::new();
    fs::write(tmp.root.join("PROGRAM"), b"program: work\n").unwrap();
    let srv = start_web(&tmp.root);
    let (status, headers, body) = post_act(
        &srv.addr,
        "/act/lifecycle",
        r#"{"args":["enable","--apply"]}"#,
    );
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert_exit(&headers, "0");
    let text = String::from_utf8(body).unwrap();
    let root = tmp.root.display().to_string();
    assert!(
        text.contains(&format!("CREATE {root}/STATE.tsv\n")),
        "{text}"
    );
    assert!(
        text.contains(&format!("REPLACE {root}/STATE.md\n")),
        "{text}"
    );
    assert!(
        text.contains(&format!("UPDATE {root}/PROGRAM lifecycle: managed\n")),
        "{text}"
    );
    assert!(text.ends_with("enabled managed lifecycle\n"), "{text}");
    assert_eq!(
        fs::read(tmp.root.join("PROGRAM")).unwrap(),
        b"program: work\nlifecycle: managed\n"
    );
    assert_eq!(
        fs::read(tmp.root.join("STATE.tsv")).unwrap(),
        STATE_HEADER_LINE.as_bytes()
    );
    let rendered = fs::read_to_string(tmp.root.join("STATE.md")).unwrap();
    assert!(
        rendered.contains("Generated from `STATE.tsv`"),
        "{rendered}"
    );
    assert!(!tmp.root.join(".state.lock").exists());
    assert!(!tmp.root.join("items").exists());
    assert!(!tmp.root.join("TARGET").exists());
    assert!(!tmp.root.join("MAKER").exists());
    assert!(!tmp.root.join("briefs").exists());
    for name in entry_names(&tmp.root) {
        assert!(
            !(name.starts_with(".STATE.tsv.") && name.ends_with(".tmp")),
            "{name}"
        );
        assert!(
            !(name.starts_with(".STATE.md.") && name.ends_with(".tmp")),
            "{name}"
        );
        assert!(
            !(name.starts_with(".PROGRAM.") && name.ends_with(".tmp")),
            "{name}"
        );
    }
    assert_eq!(
        entry_names(&tmp.root),
        vec![
            "PROGRAM".to_string(),
            "STATE.md".to_string(),
            "STATE.tsv".to_string(),
        ]
    );
}

#[test]
fn web_evidence_click_writes_nothing_and_archive_moves_stale_txt() {
    let tmp = Tmp::new();
    let evidence = tmp.root.join("items/slug/evidence");
    fs::create_dir_all(&evidence).unwrap();
    let stale = b"stale-bytes\n";
    fs::write(evidence.join("stale.tok.DEAD.txt"), stale).unwrap();
    fs::write(evidence.join("kept.tok.EMPTY.txt"), b"kept\n").unwrap();
    fs::write(evidence.join(".partial.hidden.txt"), b"hidden\n").unwrap();
    fs::write(evidence.join("notes.md"), b"notes\n").unwrap();
    fs::write(tmp.root.join("agents.tsv"), "a1\tkindA\tm\thigh\ttrue\n").unwrap();
    fs::create_dir_all(tmp.root.join("claims/C1")).unwrap();
    assert!(!tmp.root.join("PROGRAM").exists());
    assert!(!tmp.root.join("items/slug/TARGET").exists());
    assert!(!tmp.root.join("items/slug/work").exists());

    let srv = start_web(&tmp.root);
    let (status, headers, body) = post_act(&srv.addr, "/act/evidence", r#"{"args":[]}"#);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert_exit(&headers, "2");
    assert!(body.is_empty(), "{body:?}");
    let text = String::from_utf8_lossy(&body);
    assert!(!text.contains("crucible:"), "{text}");
    assert!(!text.contains("archived"), "{text}");
    assert!(!evidence.join("history").exists());
    assert!(evidence.join("stale.tok.DEAD.txt").is_file());
    assert_eq!(
        entry_names(&evidence),
        vec![
            ".partial.hidden.txt".to_string(),
            "kept.tok.EMPTY.txt".to_string(),
            "notes.md".to_string(),
            "stale.tok.DEAD.txt".to_string(),
        ]
    );

    let (status, headers, body) =
        post_act(&srv.addr, "/act/evidence", r#"{"args":["archive","slug"]}"#);
    assert_eq!(status, 200, "{headers} body={body:?}");
    assert_exit(&headers, "0");
    let text = String::from_utf8(body).unwrap();
    assert!(
        text.contains("archived stale.tok.DEAD.txt (work-id DEAD, current EMPTY)"),
        "{text}"
    );
    let archived = evidence.join("history/stale.tok.DEAD.txt");
    assert_eq!(fs::read(&archived).unwrap(), stale);
    assert!(!evidence.join("stale.tok.DEAD.txt").exists());
    assert_eq!(
        fs::read(evidence.join("kept.tok.EMPTY.txt")).unwrap(),
        b"kept\n"
    );
    assert_eq!(
        fs::read(evidence.join(".partial.hidden.txt")).unwrap(),
        b"hidden\n"
    );
    assert_eq!(fs::read(evidence.join("notes.md")).unwrap(), b"notes\n");
    assert!(!evidence.join("history/kept.tok.EMPTY.txt").exists());
    assert!(!evidence.join("history/.partial.hidden.txt").exists());
    assert!(!evidence.join("history/notes.md").exists());
    let after_archive = vec![
        ".partial.hidden.txt".to_string(),
        "history".to_string(),
        "kept.tok.EMPTY.txt".to_string(),
        "notes.md".to_string(),
    ];
    assert_eq!(entry_names(&evidence), after_archive);
    assert_eq!(
        entry_names(&evidence.join("history")),
        vec!["stale.tok.DEAD.txt".to_string()]
    );

    for (path, json) in [
        (
            "/act/run",
            r#"{"args":["slug","a1","--","/bin/echo","hi"]}"#,
        ),
        (
            "/act/run-claim",
            r#"{"args":["C1","a1","--","/bin/echo","hi"]}"#,
        ),
    ] {
        let (status, headers, body) = post_act(&srv.addr, path, json);
        assert_eq!(status, 404, "{path} {headers} body={body:?}");
        assert_eq!(body, b"not found\n");
    }
    assert_eq!(entry_names(&evidence), after_archive);
    assert_eq!(
        entry_names(&evidence.join("history")),
        vec!["stale.tok.DEAD.txt".to_string()]
    );
    assert_eq!(fs::read(&archived).unwrap(), stale);
    assert!(!tmp.root.join("claims/C1/evidence").exists());
    assert!(entry_names(&tmp.root.join("claims/C1")).is_empty());
    assert_eq!(
        entry_names(&tmp.root),
        vec![
            "agents.tsv".to_string(),
            "claims".to_string(),
            "items".to_string(),
        ]
    );
}

fn wait_exit(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(st) = child.try_wait().unwrap() {
            return Some(st);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn http_req(addr: &str, method: &str, path: &str) -> (u16, serde_json::Value, Vec<u8>) {
    let sock: std::net::SocketAddr = addr.parse().expect("bind addr");
    let mut last = None;
    for _ in 0..50 {
        match TcpStream::connect_timeout(&sock, Duration::from_millis(100)) {
            Ok(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                write!(
                    s,
                    "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
                let mut raw = Vec::new();
                s.read_to_end(&mut raw).unwrap();
                return parse_http(&raw);
            }
            Err(e) => {
                last = Some(e);
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    panic!("connect {addr}: {last:?}");
}

fn parse_http(raw: &[u8]) -> (u16, serde_json::Value, Vec<u8>) {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .or_else(|| text.split_once("\n\n"))
        .unwrap_or((text.as_ref(), ""));
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let json = if body.trim().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(body.trim()).unwrap_or(serde_json::Value::Null)
    };
    (status, json, body.as_bytes().to_vec())
}

fn strip_clock(mut v: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = v.as_object_mut() {
        obj.remove("until");
        obj.remove("since");
        if let Some(floor) = obj.get_mut("floor") {
            if let Some(f) = floor.as_object_mut() {
                f.remove("elapsed_s");
            }
        }
    }
    v
}

#[test]
fn serve_get_walk_equals_status_json() {
    let tmp = Tmp::new();
    golden_board(&tmp.root);
    let before = fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap();
    let srv = start_serve(&tmp.root, "127.0.0.1:0");
    let (code, walk, _) = http_req(&srv.addr, "GET", "/walk");
    assert_eq!(code, 200);
    assert_eq!(walk["schema"], "crucible.walk/v1");
    assert_eq!(walk["available"], true);
    assert!(walk["available"].is_boolean());
    assert_eq!(walk["floor"]["card"], "STOP-ASK INTAKE");

    let cli = bin()
        .current_dir(&tmp.root)
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(cli.status.success());
    let status: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(strip_clock(walk), strip_clock(status));
    assert_eq!(fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap(), before);
}

#[test]
fn serve_get_walk_missing_wm_available_false() {
    let tmp = Tmp::new();
    let srv = start_serve(&tmp.root, "127.0.0.1:0");
    let (code, walk, _) = http_req(&srv.addr, "GET", "/walk");
    assert_eq!(code, 200);
    assert_eq!(walk["available"], false);
    assert!(walk["available"].is_boolean());
    assert!(!tmp.root.join(".wm").exists());
}

#[test]
fn serve_get_stats_equals_stats_json() {
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    let when = format_rfc3339_z(SystemClock.now_unix());
    fs::write(
        wm.join("METRICS.tsv"),
        format!("when\toutcome\tslices\tbound\tnote\n{when}\tSTOP-ASK QUESTIONS\t0\t40\t-\n"),
    )
    .unwrap();
    let srv = start_serve(&tmp.root, "127.0.0.1:0");
    for window in ["8h", "24h", "7d"] {
        let (code, http, _) = http_req(&srv.addr, "GET", &format!("/stats?since={window}"));
        assert_eq!(code, 200, "GET /stats?since={window}");
        assert_eq!(http["schema"], "crucible.stats/v1");
        assert_eq!(http["available"], true);
        assert_eq!(http["source"], "metrics");
        assert_eq!(http["halts"][0]["outcome"], "STOP-ASK QUESTIONS");

        let cli = bin()
            .current_dir(&tmp.root)
            .args(["stats", "--since", window, "--json"])
            .output()
            .unwrap();
        assert!(
            cli.status.success(),
            "stats --since {window}: {}",
            String::from_utf8_lossy(&cli.stderr)
        );
        let stats: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
        assert_eq!(
            strip_clock(http),
            strip_clock(stats),
            "GET /stats?since={window} == stats --json"
        );
    }
}

#[test]
fn serve_get_stats_events_wal_equals_stats_json() {
    let tmp = Tmp::new();
    let (_when, events_before) = write_wal_only(&tmp.root);
    let srv = start_serve(&tmp.root, "127.0.0.1:0");
    for window in ["8h", "24h", "7d"] {
        let (code, http, body) = http_req(&srv.addr, "GET", &format!("/stats?since={window}"));
        assert_eq!(code, 200, "GET /stats?since={window}");
        assert_eq!(http["schema"], "crucible.stats/v1");
        assert_eq!(http["available"], true);
        assert_eq!(http["source"], "events");
        assert_eq!(http["halts"][0]["outcome"], "STOP-ASK FROM-EVENTS");
        assert_eq!(http["counts"]["halt"], 1);
        assert_eq!(http["counts"]["card"], 1);
        assert_eq!(http["counts"]["invoke_end"], 1);

        let cli = bin()
            .current_dir(&tmp.root)
            .args(["stats", "--since", window, "--json"])
            .output()
            .unwrap();
        assert!(
            cli.status.success(),
            "stats --since {window}: {}",
            String::from_utf8_lossy(&cli.stderr)
        );
        let stats: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
        assert_eq!(stats["source"], "events");
        assert_eq!(
            strip_clock(http.clone()),
            strip_clock(stats.clone()),
            "GET /stats?since={window} == stats --json"
        );
        if http["until"] == stats["until"] {
            let http_body = String::from_utf8_lossy(&body);
            assert_eq!(
                cli.stdout,
                format!("{}\n", http_body.trim_end()).as_bytes(),
                "stats --json and GET /stats JSON stay byte-equal aside from the CLI newline"
            );
        }
    }
    assert_eq!(
        fs::read(tmp.root.join(".wm/EVENTS")).unwrap(),
        events_before,
        "GET /stats and stats --json must not write EVENTS"
    );
    assert!(!tmp.root.join(".wm/METRICS.tsv").exists());
    assert!(!tmp.root.join(".wm/TRACE.tsv").exists());
}

#[test]
fn serve_get_health_includes_bind_and_version() {
    let tmp = Tmp::new();
    let srv = start_serve(&tmp.root, "127.0.0.1:0");
    let (code, health, _) = http_req(&srv.addr, "GET", "/health");
    assert_eq!(code, 200);
    assert_eq!(health["ok"], true);
    assert_eq!(health["bind"], srv.addr);
    assert_eq!(health["version"], "1.23.1");
    assert!(!tmp.root.join(".wm").exists());
}

#[test]
fn serve_post_go_does_not_start_walk() {
    let tmp = Tmp::new();
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    let before = golden_board(&tmp.root);
    let srv = start_serve(&tmp.root, "127.0.0.1:0");
    let (code, _, _) = http_req(&srv.addr, "POST", "/go");
    assert!(
        code == 404 || code == 405,
        "POST /go must 404/405, got {code}"
    );
    let after = fs::read_to_string(tmp.root.join(".wm/TRACE.tsv")).unwrap();
    assert_eq!(after, before);
    assert!(!tmp.root.join(".wm/go.pid").exists());
}

#[test]
fn serve_refuses_non_loopback_and_does_not_listen() {
    let tmp = Tmp::new();
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let spec = format!("0.0.0.0:{port}");
    let mut child = bin()
        .current_dir(&tmp.root)
        .args(["serve", "--bind", &spec])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let st = wait_exit(&mut child, Duration::from_secs(2)).expect("non-loopback serve must exit");
    assert_ne!(st.code(), Some(0), "non-loopback must be nonzero");
    assert!(
        TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(150)
        )
        .is_err(),
        "must not listen on {port}"
    );
}

#[test]
fn serve_second_on_busy_port_fails() {
    let tmp = Tmp::new();
    let srv = start_serve(&tmp.root, "127.0.0.1:0");
    let mut child = bin()
        .current_dir(&tmp.root)
        .args(["serve", "--bind", &srv.addr])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let st = wait_exit(&mut child, Duration::from_secs(2)).expect("second serve must exit");
    assert!(!st.success(), "busy port must fail (no SO_REUSEPORT)");
}

struct KillServe(Option<String>);

impl Drop for KillServe {
    fn drop(&mut self) {
        if let Some(pid) = self.0.take() {
            let _ = Command::new("kill").args(["-TERM", pid.trim()]).status();
        }
    }
}

fn path_without_herdr() -> std::ffi::OsString {
    let mut dirs = Vec::new();
    for d in ["/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
        let p = PathBuf::from(d);
        if p.join("herdr").is_file() {
            continue;
        }
        if p.is_dir() {
            dirs.push(p);
        }
    }
    std::env::join_paths(dirs).expect("PATH without herdr")
}

fn path_prefix(first: &Path) -> std::ffi::OsString {
    let mut dirs = vec![first.to_path_buf()];
    for d in ["/usr/bin", "/bin"] {
        dirs.push(PathBuf::from(d));
    }
    std::env::join_paths(dirs).expect("PATH")
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

fn read_child_stdio(child: &mut Child) -> (String, String) {
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_string(&mut stdout);
    }
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_string(&mut stderr);
    }
    (stdout, stderr)
}

fn plant_room_layout(root: &Path) {
    let dir = root.join(".crucible/herdr");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("workspace"), "crucible\n").unwrap();
    fs::write(
        dir.join("roles"),
        "chat\norchestrator\nwatcher\nreaper\ndashboard\n",
    )
    .unwrap();
}

fn assert_not_listening_on_printed_addrs(stdout: &str, stderr: &str) {
    for line in stdout.lines().chain(stderr.lines()) {
        let line = line.trim();
        let Some(addr) = line.strip_prefix("listening ") else {
            continue;
        };
        let addr = addr.trim();
        if addr.is_empty() {
            continue;
        }
        let sock: std::net::SocketAddr = match addr.parse() {
            Ok(s) => s,
            Err(_) => continue,
        };
        assert!(
            TcpStream::connect_timeout(&sock, Duration::from_millis(200)).is_err(),
            "missing herdr still binds a port: {addr}"
        );
    }
}

#[test]
fn room_missing_herdr_does_not_serve_listen_or_write_trace() {
    let tmp = Tmp::new();
    plant_room_layout(&tmp.root);
    let before = golden_board(&tmp.root);
    let before_bytes = fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap();
    let path = path_without_herdr();
    for d in std::env::split_paths(&path) {
        assert!(
            !d.join("herdr").is_file(),
            "test PATH must not contain herdr: {}",
            d.display()
        );
    }
    let mut child = bin()
        .current_dir(&tmp.root)
        .env("PATH", &path)
        .env_remove("CRUCIBLE_HERDR")
        .arg("room")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn room");
    let st = wait_exit(&mut child, Duration::from_secs(3))
        .expect("missing herdr must exit (must not hang in serve)");
    let (stdout, stderr) = read_child_stdio(&mut child);
    assert_eq!(
        st.code(),
        Some(2),
        "missing herdr must exit 2: stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        !stdout.to_ascii_lowercase().contains("listening"),
        "must not start serve: stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        !stderr.to_ascii_lowercase().contains("listening"),
        "must not start serve: stderr={stderr:?}"
    );
    assert!(
        stderr.contains("herdr not found"),
        "stderr should name the missing binary, not a layout miss: {stderr:?}"
    );
    assert_eq!(
        fs::read_to_string(tmp.root.join(".wm/TRACE.tsv")).unwrap(),
        before
    );
    assert_eq!(
        fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap(),
        before_bytes
    );
    assert!(!tmp.root.join(".wm/go.pid").exists());
    assert_not_listening_on_printed_addrs(&stdout, &stderr);
}

#[test]
fn room_with_herdr_spawns_current_exe_serve_not_path_bin() {
    let tmp = Tmp::new();
    plant_room_layout(&tmp.root);
    let before = golden_board(&tmp.root);
    let before_bytes = fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap();
    fs::write(tmp.root.join("IDEA.md"), "receipt\n").unwrap();
    let bindir = tmp.root.join("bin");
    fs::create_dir(&bindir).unwrap();
    let log = tmp.root.join("herdr.log");
    let state = tmp.root.join("tabs.txt");
    write_exec(
        &bindir.join("herdr"),
        &format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> {log}
state={state}
cmd="$1 $2"
if [ "$cmd" = "workspace list" ]; then
  printf '%s\n' '{{"result":{{"workspaces":[{{"workspace_id":"ws1","label":"crucible","cwd":"/herdr-keeps-its-cwd"}}]}}}}'
elif [ "$cmd" = "workspace create" ]; then
  printf '%s\n' '{{"result":{{"workspace":{{"workspace_id":"ws1","label":"crucible","cwd":"/herdr-keeps-its-cwd"}}}}}}'
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
  printf '%s\n' '{{"result":{{"tab":{{"label":"'"$label"'","tab_id":"tab-'"$label"'","workspace_id":"ws1"}},"root_pane":{{"pane_id":"pane-'"$label"'"}}}}}}'
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
        ),
    );
    write_exec(
        &bindir.join("crucible"),
        "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$CRUCIBLE_ROOM_MARKER/path-crucible\"\nexit 1\n",
    );
    write_exec(
        &bindir.join("wm"),
        "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$CRUCIBLE_ROOM_MARKER/path-wm\"\nexit 1\n",
    );
    let mut child = bin()
        .current_dir(&tmp.root)
        .env("PATH", path_prefix(&bindir))
        .env("CRUCIBLE_ROOM_MARKER", &tmp.root)
        .env_remove("CRUCIBLE_HERDR")
        .arg("room")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn room");
    let st = wait_exit(&mut child, Duration::from_secs(8))
        .expect("room must exit after health GET (must not hang)");
    let (stdout, stderr) = read_child_stdio(&mut child);
    let _kill = KillServe(
        stdout
            .lines()
            .find_map(|l| l.strip_prefix("serve pid "))
            .map(|s| s.trim().to_string()),
    );
    assert_eq!(
        st.code(),
        Some(0),
        "room with herdr: stdout={stdout:?} stderr={stderr:?}"
    );
    for role in ["chat", "orchestrator", "watcher", "reaper", "dashboard"] {
        assert!(
            stdout.contains(role),
            "standing role {role} missing: {stdout:?}"
        );
    }
    assert!(
        stdout.contains("GET /health"),
        "must GET /health: {stdout:?}"
    );
    assert!(
        stdout.contains("\"ok\":true") || stdout.contains("\"ok\": true"),
        "health body: {stdout:?}"
    );
    assert!(stdout.contains("1.23.1"), "health version: {stdout:?}");
    assert!(
        !tmp.root.join("path-crucible").exists(),
        "must spawn current_exe, not PATH crucible"
    );
    assert!(!tmp.root.join("path-wm").exists(), "must not spawn PATH wm");
    assert_eq!(
        fs::read(tmp.root.join(".wm/TRACE.tsv")).unwrap(),
        before_bytes,
        "room must not write TRACE (no POST /go)"
    );
    assert_eq!(
        fs::read_to_string(tmp.root.join(".wm/TRACE.tsv")).unwrap(),
        before
    );
    assert!(!tmp.root.join(".wm/go.pid").exists());
    assert!(
        stdout.contains("go orchestrator"),
        "IDEA.md must start go as a process, not HTTP: {stdout}"
    );
    assert!(!stdout.contains("POST"), "room must not POST /go: {stdout}");
    let reused = stdout.contains("serve reused");
    if reused {
        assert!(
            !stdout.contains("serve pid "),
            "reuse must not spawn: {stdout}"
        );
        assert!(
            !stdout.lines().any(|l| l.starts_with("listening ")),
            "{stdout}"
        );
    } else {
        assert!(
            stdout.contains("listening 127.0.0.1:1734"),
            "spawn bind: stdout={stdout:?} stderr={stderr:?}"
        );
    }
    let log = fs::read_to_string(tmp.root.join("herdr.log")).unwrap();
    assert!(
        !log.split_whitespace().any(|w| w == "server"),
        "no herdr server:\n{log}"
    );
    assert!(!log.contains("config.toml"), "{log}");
    assert!(!log.contains("workspace create"), "{log}");
    assert!(!log.contains("--session"), "{log}");
    assert!(!log.contains("pane-chat"), "no pane run in chat:\n{log}");
    assert!(
        !log.split_whitespace().any(|w| w == "reap"),
        "pid 0 must not reap:\n{log}"
    );
}

const SHAPING_ROW: &str = "  shaping                              read modules/shaping/SKILL.md\n";

fn crucible_at(root: &Path, home: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
    bin()
        .current_dir(cwd)
        .env("CRUCIBLE_ROOT", root)
        .env("HOME", home)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("CRUCIBLE_WRAPPER")
        .args(args)
        .output()
        .expect("spawn crucible")
}

fn help_stdout(root: &Path, home: &Path, cwd: &Path) -> String {
    let out = crucible_at(root, home, cwd, &["help"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn tree_snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut ents: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
        ents.sort_by_key(|ent| ent.file_name());
        for ent in ents {
            let path = ent.path();
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&path).unwrap();
                out.push((format!("link:{rel}:{}", target.display()), Vec::new()));
            } else if meta.is_dir() {
                out.push((format!("dir:{rel}"), Vec::new()));
                walk(base, &path, out);
            } else {
                out.push((rel, fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn plant_tracked_shaping(dir: &Path) {
    let tracked = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../modules/shaping");
    let dest = dir.join("modules/shaping");
    fs::create_dir_all(&dest).unwrap();
    fs::copy(tracked.join("module.txt"), dest.join("module.txt")).unwrap();
    fs::copy(tracked.join("SKILL.md"), dest.join("SKILL.md")).unwrap();
}

fn problem_file_command(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.len() <= 39 || !line.is_char_boundary(39) || bytes[39] == b' ' {
        return false;
    }
    let field = &line[..39];
    field.ends_with("  ")
        && field.trim_end().strip_prefix("  ") == Some("crucible cycle problem FILE")
}

fn with_shaping_row(baseline: &str) -> String {
    let mut out = String::new();
    let mut inserted = false;
    for line in baseline.split_inclusive('\n') {
        out.push_str(line);
        if !inserted && problem_file_command(line.trim_end_matches('\n')) {
            out.push_str(SHAPING_ROW);
            inserted = true;
        }
    }
    assert!(inserted, "problem FILE row missing:\n{baseline}");
    assert_eq!(out.matches(SHAPING_ROW).count(), 1);
    out
}

fn home_names(home: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(home)
        .unwrap()
        .map(|ent| ent.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn help_without_shaping_value_matches_off() {
    assert_eq!(SHAPING_ROW.as_bytes()[39], b'r');
    assert!(!SHAPING_ROW.starts_with("  crucible"));

    let tmp = Tmp::new();
    let bare = tmp.root.join("bare");
    let root = tmp.root.join("root");
    let cwd = tmp.root.join("cwd");
    let home = tmp.root.join("home");
    fs::create_dir_all(&bare).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("MARKER"), b"CANARY\n").unwrap();
    plant_tracked_shaping(&root);
    fs::create_dir_all(cwd.join("modules/shaping")).unwrap();
    fs::write(cwd.join("shaping"), "grok\n").unwrap();
    fs::write(cwd.join("modules/shaping/module.txt"), b"id: shaping\n").unwrap();
    fs::write(cwd.join("modules/shaping/SKILL.md"), b"cwd\n").unwrap();
    fs::create_dir_all(home.join("modules/shaping")).unwrap();
    fs::write(home.join("shaping"), "grok\n").unwrap();
    fs::write(home.join("modules/shaping/SKILL.md"), b"home\n").unwrap();
    let home_before = tree_snapshot(&home);

    let baseline = help_stdout(&bare, &home, &cwd);
    assert!(!baseline.contains("shaping"), "{baseline}");
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
    fs::write(root.join("shaping"), "off\n").unwrap();
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
    fs::write(root.join("shaping"), "banana\n").unwrap();
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
    fs::write(root.join("shaping"), "").unwrap();
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
    fs::write(root.join("shaping"), "grok extra\n").unwrap();
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
    let before = tree_snapshot(&root);
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
    assert_eq!(tree_snapshot(&root), before);
    assert_eq!(tree_snapshot(&home), home_before);

    fs::remove_file(root.join("shaping")).unwrap();
    let target = root.join("grok-target");
    fs::write(&target, "grok\n").unwrap();
    std::os::unix::fs::symlink(&target, root.join("shaping")).unwrap();
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
    assert_eq!(fs::read(&target).unwrap(), b"grok\n");

    let unknown = crucible_at(&root, &home, &cwd, &["nope"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    let err = String::from_utf8(unknown.stderr).unwrap();
    assert!(err.starts_with("crucible: unknown verb: nope\n\n"), "{err}");
    assert_eq!(
        err.trim_start_matches("crucible: unknown verb: nope\n\n"),
        baseline
    );
    assert!(!err.contains(SHAPING_ROW));
}

#[test]
fn help_grok_inserts_one_row_and_writes_nothing() {
    let tmp = Tmp::new();
    let bare = tmp.root.join("bare");
    let root = tmp.root.join("root");
    let cwd = tmp.root.join("cwd");
    let home = tmp.root.join("home");
    fs::create_dir_all(&bare).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("MARKER"), b"CANARY\n").unwrap();
    plant_tracked_shaping(&root);
    fs::write(root.join("shaping"), " \tgrok\n").unwrap();
    let baseline = help_stdout(&bare, &home, &cwd);
    let expected = with_shaping_row(&baseline);
    let before = tree_snapshot(&root);
    assert_eq!(help_stdout(&root, &home, &cwd), expected);
    assert_eq!(tree_snapshot(&root), before);
    assert_eq!(home_names(&home), vec!["MARKER".to_string()]);
    assert_eq!(fs::read(home.join("MARKER")).unwrap(), b"CANARY\n");
    assert!(!home.join("modules").exists());

    for args in [&[][..], &["--help"], &["-h"], &["help"]] {
        let out = crucible_at(&root, &home, &cwd, args);
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        assert!(out.stderr.is_empty(), "{args:?}");
        assert_eq!(String::from_utf8(out.stdout).unwrap(), expected, "{args:?}");
    }
    let protocol = crucible_at(&root, &home, &cwd, &["help", "protocol"]);
    assert_eq!(protocol.status.code(), Some(0));
    let protocol_out = String::from_utf8(protocol.stdout).unwrap();
    assert!(!protocol_out.contains("shaping"), "{protocol_out}");
    let unknown = crucible_at(&root, &home, &cwd, &["nope"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    let err = String::from_utf8(unknown.stderr).unwrap();
    assert_eq!(
        err.trim_start_matches("crucible: unknown verb: nope\n\n"),
        expected
    );
    assert_eq!(tree_snapshot(&root), before);
    assert_eq!(fs::read(home.join("MARKER")).unwrap(), b"CANARY\n");
    assert!(!home.join("modules").exists());
    assert!(!root.join("IDEA.md").exists());
}

#[test]
fn help_ignores_other_modules_symlink_and_bad_manifest() {
    let tmp = Tmp::new();
    let bare = tmp.root.join("bare");
    let root = tmp.root.join("root");
    let cwd = tmp.root.join("cwd");
    let home = tmp.root.join("home");
    fs::create_dir_all(&bare).unwrap();
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("MARKER"), b"CANARY\n").unwrap();
    let tracked = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../modules/shaping");
    let manifest = fs::read_to_string(tracked.join("module.txt")).unwrap();
    let baseline = help_stdout(&bare, &home, &cwd);
    let expected = with_shaping_row(&baseline);

    fs::write(root.join("shaping"), "grok\n").unwrap();
    let other = root.join("modules/other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("module.txt"), &manifest).unwrap();
    fs::copy(tracked.join("SKILL.md"), other.join("SKILL.md")).unwrap();
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);

    let real = tmp.root.join("real");
    fs::create_dir_all(&real).unwrap();
    fs::copy(tracked.join("module.txt"), real.join("module.txt")).unwrap();
    fs::copy(tracked.join("SKILL.md"), real.join("SKILL.md")).unwrap();
    std::os::unix::fs::symlink(&real, root.join("modules/shaping")).unwrap();
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
    assert_eq!(
        fs::read(real.join("SKILL.md")).unwrap(),
        fs::read(tracked.join("SKILL.md")).unwrap()
    );
    fs::remove_file(root.join("modules/shaping")).unwrap();

    plant_tracked_shaping(&root);
    assert_eq!(
        help_stdout(&root, &home, &cwd).matches(SHAPING_ROW).count(),
        1
    );
    assert_eq!(help_stdout(&root, &home, &cwd), expected);

    let cases = [
        manifest.replacen("applies: always\n", "", 1),
        manifest.replace("writes:\n", "writes:\nextra: no\n"),
        manifest.replacen("applies: always\n", "applies: bug\n", 1),
        manifest.replacen("writes:\n", "writes: IDEA.md\n", 1),
        manifest.replacen(
            "id: shaping\napplies: always\n",
            "applies: always\nid: shaping\n",
            1,
        ),
        manifest.replace('\n', "\r\n"),
    ];
    for body in cases {
        fs::write(root.join("modules/shaping/module.txt"), body).unwrap();
        let before = tree_snapshot(&root);
        assert_eq!(help_stdout(&root, &home, &cwd), baseline);
        assert_eq!(tree_snapshot(&root), before);
        assert!(!root.join("IDEA.md").exists());
    }
    fs::copy(
        tracked.join("module.txt"),
        root.join("modules/shaping/module.txt"),
    )
    .unwrap();
    fs::write(root.join("modules/shaping/SKILL.md"), "").unwrap();
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
    fs::remove_file(root.join("modules/shaping/module.txt")).unwrap();
    fs::copy(
        tracked.join("SKILL.md"),
        root.join("modules/shaping/SKILL.md"),
    )
    .unwrap();
    assert_eq!(help_stdout(&root, &home, &cwd), baseline);
}

#[test]
fn adopt_home_canary_copies_shaping_module() {
    let tmp = Tmp::new();
    let repo = tmp.root.join("product");
    let src = tmp.root.join("engine");
    let home = tmp.root.join("home");
    fs::create_dir_all(&repo).unwrap();
    init_git_product(&repo);
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("MARKER"), b"CANARY\n").unwrap();
    fs::create_dir_all(src.join(".grok/rules")).unwrap();
    fs::write(src.join(".grok/rules/loop-router.md"), b"router\n").unwrap();
    fs::create_dir_all(src.join("templates/herdr")).unwrap();
    fs::write(src.join("templates/herdr/workspace"), b"crucible\n").unwrap();
    fs::write(src.join("templates/herdr/roles"), b"chat\n").unwrap();
    plant_tracked_shaping(&src);
    fs::write(src.join("VERSION"), b"1.20.0\n").unwrap();
    fs::write(src.join("START.md"), b"start\n").unwrap();

    let out = bin()
        .current_dir(&repo)
        .env("CRUCIBLE_ROOT", &src)
        .env("CRUCIBLE_RUST_BIN", env!("CARGO_BIN_EXE_crucible"))
        .env("HOME", &home)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("CRUCIBLE_WRAPPER")
        .args(["adopt", "work", "--managed"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let dest = repo.join(".crucible/work/modules/shaping");
    let meta = fs::symlink_metadata(&dest).unwrap();
    assert!(meta.is_dir());
    assert!(!meta.file_type().is_symlink());
    assert_eq!(
        fs::read(dest.join("module.txt")).unwrap(),
        fs::read(src.join("modules/shaping/module.txt")).unwrap()
    );
    assert_eq!(
        fs::read(dest.join("SKILL.md")).unwrap(),
        fs::read(src.join("modules/shaping/SKILL.md")).unwrap()
    );
    assert!(!repo.join(".crucible/work/shaping").exists());
    assert_eq!(home_names(&home), vec!["MARKER".to_string()]);
    assert_eq!(fs::read(home.join("MARKER")).unwrap(), b"CANARY\n");
    assert!(!home.join("modules").exists());

    let refreshed = bin()
        .current_dir(&repo)
        .env("CRUCIBLE_ROOT", &src)
        .env("CRUCIBLE_RUST_BIN", env!("CARGO_BIN_EXE_crucible"))
        .env("HOME", &home)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("CRUCIBLE_WRAPPER")
        .args(["adopt", "work", "--refresh"])
        .output()
        .unwrap();
    assert!(
        refreshed.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&refreshed.stdout),
        String::from_utf8_lossy(&refreshed.stderr)
    );
    assert_eq!(home_names(&home), vec!["MARKER".to_string()]);
    assert_eq!(fs::read(home.join("MARKER")).unwrap(), b"CANARY\n");
    assert!(!home.join("modules").exists());
}
