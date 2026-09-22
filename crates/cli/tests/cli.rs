//! CLI: query verbs stay read-only; `go` writes FLOOR/TRACE via kernel (foreground).

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
    assert_eq!(ver_before.trim(), "1.17.0");

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
    assert_eq!(want, "1.17.0");
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
fn help_lists_go_query_and_serve_not_room() {
    let out = bin().arg("help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("go"), "help must list go: {stdout}");
    assert!(stdout.contains("status"));
    assert!(stdout.contains("debrief"));
    assert!(stdout.contains("stats"));
    assert!(
        stdout.to_ascii_lowercase().contains("serve"),
        "help must list serve: {stdout}"
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
fn serve_get_health_includes_bind_and_version() {
    let tmp = Tmp::new();
    let srv = start_serve(&tmp.root, "127.0.0.1:0");
    let (code, health, _) = http_req(&srv.addr, "GET", "/health");
    assert_eq!(code, 200);
    assert_eq!(health["ok"], true);
    assert_eq!(health["bind"], srv.addr);
    assert_eq!(health["version"], "1.17.0");
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
