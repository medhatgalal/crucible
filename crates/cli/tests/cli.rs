//! CLI: query verbs stay read-only; `go` writes FLOOR/TRACE via kernel (foreground).

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

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
    let floor_before = "station: ANDON";
    let floor_after = fs::read_to_string(tmp.root.join(".wm/FLOOR.md")).unwrap();
    assert!(floor_after.contains(floor_before));
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

#[test]
fn stats_since_json_from_metrics_only() {
    let tmp = Tmp::new();
    let wm = tmp.root.join(".wm");
    fs::create_dir_all(&wm).unwrap();
    // SystemClock in the binary: stamp METRICS/EVENTS at "now" so 8h|24h|7d include the row.
    let when = format_rfc3339_z(SystemClock.now_unix());
    fs::write(
        wm.join("METRICS.tsv"),
        format!("when\toutcome\tslices\tbound\tnote\n{when}\tSTOP-ASK QUESTIONS\t0\t40\t-\n"),
    )
    .unwrap();
    fs::write(
        wm.join("EVENTS"),
        format!(
            "{{\"t\":\"{when}\",\"kind\":\"card\",\"card\":\"NEXT RED\",\"station\":\"BUILD\"}}\n\
             {{\"t\":\"{when}\",\"kind\":\"invoke_end\",\"session\":\"00000000-0000-0000-0000-000000000001\",\"card\":\"NEXT RUN maker-build\",\"elapsed_s\":7,\"exit\":0}}\n\
             {{\"t\":\"{when}\",\"kind\":\"halt\",\"card\":\"STOP-ASK FROM-EVENTS\",\"elapsed_s\":90,\"iterations\":3}}\n"
        ),
    )
    .unwrap();
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
        assert_eq!(v["source"], "metrics");
        let halts = v["halts"].as_array().expect("halts array");
        assert_eq!(halts.len(), 1, "window {window}: METRICS row only");
        assert_eq!(halts[0]["outcome"], "STOP-ASK QUESTIONS");
        assert_eq!(halts[0]["slices"], 0);
        assert_eq!(halts[0]["bound"], 40);
        assert_eq!(halts[0]["t"], when);
        assert_eq!(v["counts"]["halt"], 1);
        assert_eq!(v["counts"]["card"], 0);
        assert_eq!(v["counts"]["invoke_end"], 0);
        let dumped = serde_json::to_string(&v).unwrap();
        assert!(
            !dumped.contains("FROM-EVENTS"),
            "EVENTS halt must not appear in stats"
        );
        assert!(!dumped.contains("NEXT RED"));
    }
}

#[test]
fn status_without_json_does_not_write() {
    let tmp = Tmp::new();
    let before = golden_board(&tmp.root);
    let _ = bin().current_dir(&tmp.root).arg("status").output().unwrap();
    let after = fs::read_to_string(tmp.root.join(".wm/TRACE.tsv")).unwrap();
    assert_eq!(after, before);
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
    assert_eq!(ver_before.trim(), "1.16.4");

    let tmp = Tmp::new();
    let _ = bin().current_dir(&tmp.root).arg("go").output().unwrap();

    let posix_after = fs::read(repo.join("crucible")).unwrap();
    let ver_after = fs::read_to_string(repo.join("VERSION")).unwrap();
    assert_eq!(posix_after, posix_before);
    assert_eq!(ver_after, ver_before);
}

#[test]
fn help_lists_go_and_query_verbs_not_serve_room() {
    let out = bin().arg("help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("go"), "help must list go: {stdout}");
    assert!(stdout.contains("status"));
    assert!(stdout.contains("debrief"));
    assert!(stdout.contains("stats"));
    assert!(
        !stdout.to_ascii_lowercase().contains("serve"),
        "do not list serve: {stdout}"
    );
    assert!(
        !stdout.to_ascii_lowercase().contains("room"),
        "do not list room: {stdout}"
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
}
