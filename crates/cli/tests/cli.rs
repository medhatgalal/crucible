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
    assert!(!tmp.root.join(".wm").exists(), "must not mkdir .wm");

    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    let trace = "when\tcard\toutcome\n2026-09-20T12:00:00Z\tNEXT INTAKE\tSHAPE\n";
    fs::write(wm.join("TRACE.tsv"), trace).unwrap();
    fs::write(wm.join("t0"), "1773964800\n").unwrap();
    let out = bin().current_dir(&tmp.root).arg("status").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(fs::read_to_string(wm.join("TRACE.tsv")).unwrap(), trace);
    assert_eq!(fs::read_to_string(wm.join("t0")).unwrap(), "1773964800\n");
    assert!(!wm.join("FLOOR.md").exists());
    assert!(!wm.join("EVENTS").exists());

    let floor = "station: SHAPE\nwip: -\n";
    fs::write(wm.join("FLOOR.md"), floor).unwrap();
    let out = bin().current_dir(&tmp.root).arg("status").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
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
    assert_eq!(ver_before.trim(), "1.18.0");

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
    assert_eq!(want, "1.18.0");
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
    assert_eq!(health["version"], "1.18.0");
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
        stderr.to_ascii_lowercase().contains("herdr"),
        "stderr should name herdr: {stderr:?}"
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
  printf '%s\n' '{{"result":{{"workspaces":[]}}}}'
elif [ "$cmd" = "workspace create" ]; then
  printf '%s\n' '{{"result":{{"workspace":{{"workspace_id":"ws1","label":"crucible"}}}}}}'
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
  printf '%s\n' '{{"result":{{"tab":{{"label":"'"$label"'","tab_id":"tab-'"$label"'"}}}}}}'
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
    assert!(stdout.contains("1.18.0"), "health version: {stdout:?}");
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
    assert!(!log.contains("pane-chat"), "no pane run in chat:\n{log}");
    assert!(
        !log.split_whitespace().any(|w| w == "reap"),
        "pid 0 must not reap:\n{log}"
    );
}
