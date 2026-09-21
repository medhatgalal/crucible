//! File writers for FLOOR, TRACE, and EVENTS. Minimal foreground `go` and `run`. Git worktree mint. NEXT RED stub with keep-on-failure. CLOSE stamps CLOSED/halt/lesson without removing worktrees. STOP-ASK QUESTIONS when QUESTIONS.md has no ANSWERS.md. `go` wires QUESTIONS then injected `next_red` (no grok, no auto-close). No HTTP. No Herdr / Grok / EngOS types.

mod close;
mod error;
mod events;
mod floor;
mod go;
mod invoke;
mod metrics;
mod paths;
mod questions;
mod red;
mod station;
mod trace;
mod worktree;

pub use close::{close_walk, CloseWalk};
pub use error::KernelError;
pub use events::{append_event, read_events};
pub use floor::{floor_write, FloorWriteResult};
pub use go::{go, GoRun};
pub use invoke::{run, InvokeRun};
pub use metrics::{metrics_append, METRICS_HEADER};
pub use questions::{stop_ask_questions, StopAskQuestions};
pub use red::{next_red, next_red_with, NextRed, NextRedOpts};
pub use station::floor_station;
pub use trace::{go_start, GoStart, TRACE_HEADER};
pub use worktree::{mint_worktree, remove_worktree};

pub use crucible_contract::{
    format_rfc3339_z, parse_rfc3339_z, Clock, Event, EventKind, FixedClock, SystemClock,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crucible_contract::WalkSnapshot;
    use std::ffi::OsStr;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, MutexGuard};

    static RED_ENV_LOCK: Mutex<()> = Mutex::new(());

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-kernel-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn wm(&self) -> PathBuf {
            self.root.join(".wm")
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn t0() -> i64 {
        parse_rfc3339_z("2026-09-20T12:00:00Z").unwrap()
    }

    fn clock() -> FixedClock {
        FixedClock::new(t0() + 12)
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

    fn plant_trace(wm: &Path, card: &str) {
        fs::create_dir_all(wm).unwrap();
        fs::write(wm.join("t0"), format!("{}\n", t0())).unwrap();
        fs::write(
            wm.join("TRACE.tsv"),
            format!("when\tcard\toutcome\n2026-09-20T12:00:00Z\t{card}\tANDON\n"),
        )
        .unwrap();
    }

    #[test]
    fn floor_write_keys_t0_elapsed_and_trace_on_change() {
        let tmp = Tmp::new();
        let r = floor_write(
            tmp.path(),
            "NEXT INTAKE\nignored",
            "SUBAGENT-ISOLATED",
            &clock(),
        )
        .unwrap();
        assert_eq!(r.card, "NEXT INTAKE");
        assert_eq!(r.station, "SHAPE");
        assert_eq!(r.wip, "-");
        assert_eq!(r.andon, "-");
        assert_eq!(r.elapsed_s, 0, "t0 written as now when missing");
        assert!(r.trace_appended);

        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("station: SHAPE\n"));
        assert!(floor.contains("card: NEXT INTAKE\n"));
        assert!(floor.contains("wip: -\n"));
        assert!(floor.contains("andon: -\n"));
        assert!(floor.contains("independence: SUBAGENT-ISOLATED\n"));
        assert!(floor.contains("elapsed: 0\n"));
        assert!(floor.contains("evidence:\n"));

        let t0_text = fs::read_to_string(tmp.wm().join("t0")).unwrap();
        assert_eq!(t0_text.trim(), &(t0() + 12).to_string());

        let trace = fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap();
        assert!(trace.starts_with(TRACE_HEADER));
        assert!(trace.contains("NEXT INTAKE\tSHAPE"));

        let again = floor_write(tmp.path(), "NEXT INTAKE", "SUBAGENT-ISOLATED", &clock()).unwrap();
        assert!(!again.trace_appended);
        let trace2 = fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap();
        assert_eq!(
            trace2.lines().filter(|l| l.contains("NEXT INTAKE")).count(),
            1
        );

        let red = floor_write(tmp.path(), "NEXT RED", "SUBAGENT-ISOLATED", &clock()).unwrap();
        assert!(red.trace_appended);
        assert_eq!(red.station, "BUILD");
        let trace3 = fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap();
        assert_eq!(
            trace3
                .lines()
                .filter(|l| !l.starts_with("when") && !l.is_empty())
                .count(),
            2
        );
    }

    #[test]
    fn floor_write_preserves_existing_t0() {
        let tmp = Tmp::new();
        fs::create_dir_all(tmp.wm()).unwrap();
        fs::write(tmp.wm().join("t0"), format!("{}\n", t0())).unwrap();
        let r = floor_write(tmp.path(), "NEXT MAP", "CROSS-FAMILY", &clock()).unwrap();
        assert_eq!(r.elapsed_s, 12);
        assert_eq!(
            fs::read_to_string(tmp.wm().join("t0")).unwrap().trim(),
            &t0().to_string()
        );
        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("elapsed: 12\n"));
        assert!(floor.contains("independence: CROSS-FAMILY\n"));
        assert_eq!(r.station, "DESIGN");
    }

    #[test]
    fn floor_write_andon_wip_and_relative_evidence() {
        let tmp = Tmp::new();
        let wm = tmp.wm();
        fs::create_dir_all(wm.join("evidence")).unwrap();
        fs::create_dir_all(tmp.path().join("reviews")).unwrap();
        fs::write(wm.join("FALSIFIER"), "x\n").unwrap();
        fs::write(
            wm.join("CLOSED"),
            "CLOSED PASS\nindependence: SUBAGENT-ISOLATED\n",
        )
        .unwrap();
        fs::write(wm.join("evidence").join("shot.txt"), "img\n").unwrap();
        fs::write(tmp.path().join("reviews").join("review.md"), "# r\n").unwrap();
        fs::write(wm.join("slice-in-flight"), "id: s1\n").unwrap();
        fs::write(wm.join("t0"), format!("{}\n", t0())).unwrap();

        let r = floor_write(tmp.path(), "STOP-ASK INTAKE", "SUBAGENT-ISOLATED", &clock()).unwrap();
        assert_eq!(r.station, "ANDON");
        assert_eq!(r.andon, "STOP-ASK INTAKE");
        assert_eq!(r.wip, "s1");

        let floor = fs::read_to_string(wm.join("FLOOR.md")).unwrap();
        assert!(floor.contains("wip: s1\n"));
        assert!(floor.contains("  .wm/FALSIFIER\n"));
        assert!(floor.contains("  reviews/review.md\n"));
        assert!(floor.contains("  .wm/evidence/shot.txt\n"));
        assert!(
            !floor.contains("CLOSED"),
            "CLOSED is not evidence in the snapshot contract"
        );

        let snap = WalkSnapshot::from_wm_dir(tmp.path(), &clock());
        assert!(snap.available);
        let f = snap.floor.unwrap();
        assert_eq!(f.elapsed_s, Some(12));
        assert_eq!(f.wip, "s1");
        assert_eq!(f.andon, "STOP-ASK INTAKE");
    }

    #[test]
    fn go_start_archives_existing_trace_before_rewrite() {
        let tmp = Tmp::new();
        plant_trace(&tmp.wm(), "STOP-ASK INTAKE");
        let previous = fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap();
        assert!(previous.contains("STOP-ASK INTAKE"));

        let started = go_start(tmp.path(), &clock()).unwrap();
        let archived = started
            .archived
            .as_ref()
            .expect("fake fail: TRACE rewrite without archive");
        assert!(
            archived.starts_with(tmp.wm().join("archive")),
            "archive path follows Data table .wm/archive/TRACE-<t0>-<iso>.tsv"
        );
        let name = archived.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with(&format!("TRACE-{}-", t0())));
        assert!(name.ends_with(".tsv"));
        let archived_body = fs::read_to_string(archived).unwrap();
        assert_eq!(
            archived_body, previous,
            "archive must be the pre-rewrite TRACE; copying after truncate would be header-only"
        );
        assert!(archived_body.contains("STOP-ASK INTAKE"));

        let live = fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap();
        assert_eq!(live, TRACE_HEADER);
        assert!(
            !live.contains("STOP-ASK INTAKE"),
            "live TRACE is this run only"
        );
        assert_eq!(started.t0_unix, t0() + 12);
        assert_eq!(
            fs::read_to_string(tmp.wm().join("t0")).unwrap().trim(),
            &(t0() + 12).to_string()
        );
        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev.len(), 1, "archive then walk_start; no extra events");
        assert_eq!(ev[0].kind, EventKind::WalkStart);
        assert_eq!(ev[0].t0, Some(t0() + 12));
    }

    #[test]
    fn go_start_first_walk_and_header_only_need_no_archive() {
        let tmp = Tmp::new();
        let first = go_start(tmp.path(), &clock()).unwrap();
        assert!(first.archived.is_none());
        assert_eq!(
            fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap(),
            TRACE_HEADER
        );

        let second = go_start(tmp.path(), &clock()).unwrap();
        assert!(
            second.archived.is_none(),
            "header-only TRACE has no data rows to keep"
        );
        assert!(tmp
            .wm()
            .join("archive")
            .read_dir()
            .ok()
            .is_none_or(|rd| rd.count() == 0));
    }

    #[test]
    fn events_jsonl_appends_design_wal_kinds() {
        let tmp = Tmp::new();
        let t = format_rfc3339_z(t0() + 12);
        append_event(tmp.path(), &Event::walk_start(&t, t0() + 12)).unwrap();
        append_event(tmp.path(), &Event::card(&t, "NEXT RED", "BUILD")).unwrap();
        append_event(
            tmp.path(),
            &Event::invoke_end(&t, "NEXT RUN maker-build", "sess-1", 7, 0),
        )
        .unwrap();
        append_event(tmp.path(), &Event::halt(&t, "STOP-ASK QUESTIONS", 90, 3)).unwrap();
        append_event(tmp.path(), &Event::halt(&t, "ESCALATE BOUND", 90, 3)).unwrap();
        append_event(tmp.path(), &Event::halt(&t, "CLOSED PASS", 90, 3)).unwrap();
        append_event(
            tmp.path(),
            &Event::halt(&t, "INDEPENDENCE_UNAVAILABLE", 12, 1),
        )
        .unwrap();
        append_event(
            tmp.path(),
            &Event::lesson(&t, "prefer archive before rewrite"),
        )
        .unwrap();

        let text = fs::read_to_string(tmp.wm().join("EVENTS")).unwrap();
        let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 8);
        let kinds: Vec<EventKind> = read_events(tmp.path())
            .unwrap()
            .into_iter()
            .map(|e| e.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                EventKind::WalkStart,
                EventKind::Card,
                EventKind::InvokeEnd,
                EventKind::Halt,
                EventKind::Halt,
                EventKind::Halt,
                EventKind::Halt,
                EventKind::Lesson,
            ]
        );
        assert!(text.contains("\"kind\":\"card\""));
        assert!(text.contains("\"kind\":\"invoke_end\""));
        assert!(text.contains("\"kind\":\"halt\""));
        assert!(text.contains("\"kind\":\"walk_start\""));
        assert!(text.contains("\"session\":\"sess-1\""));
        assert!(text.contains("\"exit\":0"));
        assert!(text.contains("STOP-ASK QUESTIONS"));
        assert!(text.contains("INDEPENDENCE_UNAVAILABLE"));
        assert!(!text.contains("\"kind\":\"dispatch\""));
        assert!(!text.contains("\"kind\":\"stop_ask\""));

        let n = text.len();
        append_event(tmp.path(), &Event::lesson(&t, "second")).unwrap();
        let text2 = fs::read_to_string(tmp.wm().join("EVENTS")).unwrap();
        assert!(text2.len() > n, "EVENTS is append-only");
    }

    #[test]
    fn metrics_append_tsv_columns_and_halt_event() {
        let tmp = Tmp::new();
        fs::write(
            tmp.path().join("slices.tsv"),
            "id\ttitle\ns1\ta\ns2\tb\n# skip\n\n",
        )
        .unwrap();
        metrics_append(tmp.path(), "STOP-ASK QUESTIONS", "-", 40, 90, 3, &clock()).unwrap();
        let metrics = fs::read_to_string(tmp.wm().join("METRICS.tsv")).unwrap();
        assert!(metrics.starts_with(METRICS_HEADER));
        let row = metrics.lines().nth(1).unwrap();
        let cols: Vec<&str> = row.split('\t').collect();
        assert_eq!(cols.len(), 5, "when/outcome/slices/bound/note");
        assert_eq!(cols[0], format_rfc3339_z(t0() + 12));
        assert_eq!(cols[1], "STOP-ASK QUESTIONS");
        assert_eq!(cols[2], "2");
        assert_eq!(cols[3], "40");
        assert_eq!(cols[4], "-");

        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].kind, EventKind::Halt);
        assert_eq!(ev[0].card.as_deref(), Some("STOP-ASK QUESTIONS"));
        assert_eq!(ev[0].elapsed_s, Some(90));
        assert_eq!(ev[0].iterations, Some(3));

        metrics_append(tmp.path(), "CLOSED PASS", "ok", 40, 12, 1, &clock()).unwrap();
        let metrics2 = fs::read_to_string(tmp.wm().join("METRICS.tsv")).unwrap();
        assert_eq!(metrics2.lines().filter(|l| !l.is_empty()).count(), 3);
        let ev2 = read_events(tmp.path()).unwrap();
        assert_eq!(ev2.len(), 2);
        assert_eq!(ev2[1].kind, EventKind::Halt);
        assert_eq!(ev2[1].card.as_deref(), Some("CLOSED PASS"));
        assert!(fs::read_to_string(tmp.wm().join("EVENTS"))
            .unwrap()
            .contains("\"kind\":\"halt\""));
    }

    #[test]
    fn go_start_and_floor_write_emit_events() {
        let tmp = Tmp::new();
        go_start(tmp.path(), &clock()).unwrap();
        floor_write(tmp.path(), "NEXT INTAKE", "SUBAGENT-ISOLATED", &clock()).unwrap();
        floor_write(tmp.path(), "NEXT INTAKE", "SUBAGENT-ISOLATED", &clock()).unwrap();
        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev[0].kind, EventKind::WalkStart);
        assert_eq!(ev.len(), 2, "card event only when TRACE appends");
        assert_eq!(ev[1].kind, EventKind::Card);
        assert_eq!(ev[1].card.as_deref(), Some("NEXT INTAKE"));
        assert_eq!(ev[1].station.as_deref(), Some("SHAPE"));
    }

    #[test]
    fn floor_station_matches_posix_table() {
        assert_eq!(floor_station("STOP-ASK INTAKE"), "ANDON");
        assert_eq!(floor_station("ESCALATE BOUND"), "ANDON");
        assert_eq!(floor_station("INDEPENDENCE_UNAVAILABLE"), "ANDON");
        assert_eq!(floor_station("CLOSED PASS"), "DONE");
        assert_eq!(floor_station("NEXT INTAKE"), "SHAPE");
        assert_eq!(floor_station("NEXT MAP"), "DESIGN");
        assert_eq!(floor_station("NEXT RUN scout"), "DESIGN");
        assert_eq!(floor_station("NEXT RUN reviewer"), "INSPECT");
        assert_eq!(floor_station("NEXT CLOSE"), "INSPECT");
        assert_eq!(floor_station("NEXT RED"), "BUILD");
        assert_eq!(floor_station("NEXT SLICE s1"), "BUILD");
        assert_eq!(floor_station("MAP-HUMAN"), "ANDON");
    }

    #[test]
    fn crate_tomls_have_no_herdr_grok_engos() {
        for src in [
            include_str!("../Cargo.toml"),
            include_str!("../../contract/Cargo.toml"),
            include_str!("../../cli/Cargo.toml"),
        ] {
            let l = src.to_ascii_lowercase();
            assert!(!l.contains("herdr"), "forbidden dep token in {src}");
            assert!(!l.contains("grok"), "forbidden dep token in {src}");
            assert!(!l.contains("engos"), "forbidden dep token in {src}");
        }
    }

    #[test]
    fn go_without_idea_stop_ask_intake_archives_trace() {
        let tmp = Tmp::new();
        plant_trace(&tmp.wm(), "STOP-ASK QUESTIONS");
        let previous = fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap();

        let r = go(tmp.path(), &clock()).unwrap();
        assert_eq!(r.exit, 1);
        assert!(r.stdout.contains("STOP-ASK INTAKE"));

        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("card: STOP-ASK INTAKE\n"));
        assert!(floor.contains("station: ANDON\n"));

        let archived = r
            .archived
            .as_ref()
            .expect("go_start must archive previous TRACE");
        assert_eq!(fs::read_to_string(archived).unwrap(), previous);

        let live = fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap();
        assert!(live.starts_with(TRACE_HEADER));
        assert!(live.contains("STOP-ASK INTAKE"));
        assert!(!live.contains("STOP-ASK QUESTIONS"));

        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev[0].kind, EventKind::WalkStart);
        assert!(
            ev.iter()
                .any(|e| e.kind == EventKind::Halt && e.card.as_deref() == Some("STOP-ASK INTAKE")),
            "EVENTS halt for STOP-ASK INTAKE: {ev:?}"
        );
        let metrics = fs::read_to_string(tmp.wm().join("METRICS.tsv")).unwrap();
        assert!(metrics.contains("STOP-ASK INTAKE"));
    }

    #[test]
    fn go_empty_idea_stop_ask_intake() {
        let tmp = Tmp::new();
        fs::write(tmp.path().join("IDEA.md"), "").unwrap();
        let r = go(tmp.path(), &clock()).unwrap();
        assert_eq!(
            r.exit, 1,
            "zero-byte IDEA.md is POSIX ! -s, same as missing"
        );
        assert!(r.stdout.contains("STOP-ASK INTAKE"));
        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("card: STOP-ASK INTAKE\n"));
        assert!(floor.contains("station: ANDON\n"));
        let ev = read_events(tmp.path()).unwrap();
        assert!(ev
            .iter()
            .any(|e| e.kind == EventKind::Halt && e.card.as_deref() == Some("STOP-ASK INTAKE")));
        let metrics = fs::read_to_string(tmp.wm().join("METRICS.tsv")).unwrap();
        assert!(metrics.contains("STOP-ASK INTAKE"));
    }

    fn lock_red_env() -> MutexGuard<'static, ()> {
        RED_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set_red_env(program: Option<&OsStr>, args: Option<&str>) {
        // SAFETY: callers hold RED_ENV_LOCK; tests do not read/write these
        // keys from other threads while the guard is live.
        unsafe {
            match program {
                Some(p) => std::env::set_var("CRUCIBLE_RED_PROGRAM", p),
                None => std::env::remove_var("CRUCIBLE_RED_PROGRAM"),
            }
            match args {
                Some(a) => std::env::set_var("CRUCIBLE_RED_ARGS", a),
                None => std::env::remove_var("CRUCIBLE_RED_ARGS"),
            }
        }
    }

    struct ClearRedEnv;
    impl Drop for ClearRedEnv {
        fn drop(&mut self) {
            set_red_env(None, None);
        }
    }

    #[test]
    fn go_idea_without_closed_refuses_brick_loop() {
        let _lock = lock_red_env();
        set_red_env(None, None);
        let tmp = Tmp::new();
        fs::write(tmp.path().join("IDEA.md"), "receipt\n").unwrap();
        let r = go(tmp.path(), &clock()).unwrap();
        assert_eq!(r.exit, 2);
        assert!(
            r.stderr.contains("go: brick loop not ported"),
            "stderr={:?}",
            r.stderr
        );
        assert!(!tmp.wm().join("go.pid").exists());
        assert!(
            !tmp.wm().join("METRICS.tsv").is_file(),
            "brick refuse must not metrics_append"
        );
        let ev = read_events(tmp.path()).unwrap();
        assert!(
            !ev.iter().any(|e| e.kind == EventKind::Halt),
            "brick refuse is not a halt: {ev:?}"
        );
        assert!(
            !tmp.wm().join("CLOSED").exists(),
            "brick refuse must not close_walk"
        );
    }

    #[test]
    fn go_idea_questions_without_answers_stop_ask() {
        let _lock = lock_red_env();
        set_red_env(None, None);
        let tmp = Tmp::new();
        fs::write(tmp.path().join("IDEA.md"), "receipt\n").unwrap();
        fs::write(tmp.path().join("QUESTIONS.md"), "What should we build?\n").unwrap();
        let r = go(tmp.path(), &clock()).unwrap();
        assert_eq!(r.exit, 1);
        assert!(
            r.stdout.contains("STOP-ASK QUESTIONS"),
            "stdout={:?} stderr={:?}",
            r.stdout,
            r.stderr
        );
        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("card: STOP-ASK QUESTIONS\n"), "{floor}");
        assert!(floor.contains("station: ANDON\n"), "{floor}");
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "{floor}"
        );
        assert!(
            !floor.contains("NEXT "),
            "QUESTIONS gate must not proceed to NEXT RED: {floor}"
        );
        assert!(!floor.to_ascii_lowercase().contains("grok"));
        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter()
                .any(|e| e.kind == EventKind::Halt
                    && e.card.as_deref() == Some("STOP-ASK QUESTIONS")),
            "halt STOP-ASK QUESTIONS: {ev:?}"
        );
        let metrics = fs::read_to_string(tmp.wm().join("METRICS.tsv")).unwrap();
        assert!(metrics.contains("STOP-ASK QUESTIONS"));
        assert!(
            !tmp.path().join("ANSWERS.md").exists(),
            "must not invent ANSWERS.md"
        );
        assert!(
            !tmp.wm().join("CLOSED").exists(),
            "must not auto close_walk"
        );
        assert!(!tmp.wm().join("go.pid").exists());
    }

    #[test]
    fn go_idea_env_red_program_floor_next_red_build() {
        let _lock = lock_red_env();
        let tmp = Tmp::new();
        init_git_product(tmp.path());
        fs::write(tmp.path().join("IDEA.md"), "receipt\n").unwrap();
        let sh = posix_tool("sh");
        let _clear = ClearRedEnv;
        set_red_env(
            Some(sh.as_os_str()),
            Some("-c\necho FAIL > .wm/FALSIFIER; pwd > marker"),
        );
        let r = go(tmp.path(), &clock()).unwrap();
        assert_eq!(r.exit, 0, "stderr={:?} stdout={:?}", r.stderr, r.stdout);
        assert!(
            r.stdout.contains("NEXT RED"),
            "stdout={:?} stderr={:?}",
            r.stdout,
            r.stderr
        );

        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("card: NEXT RED\n"), "{floor}");
        assert!(floor.contains("station: BUILD\n"), "{floor}");
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "{floor}"
        );
        assert!(!floor.to_ascii_lowercase().contains("grok"));

        let wt = tmp.wm().join("worktrees").join("s1");
        assert_minted_cwd_marker(tmp.path(), &wt);
        assert_eq!(
            fs::read_to_string(tmp.wm().join("FALSIFIER"))
                .unwrap()
                .trim(),
            "FAIL"
        );
        assert!(
            !tmp.wm().join("CLOSED").exists(),
            "must not auto close_walk after NEXT RED"
        );
        assert!(!tmp.wm().join("go.pid").exists());

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::Card
                && e.card.as_deref() == Some("NEXT RED")
                && e.station.as_deref() == Some("BUILD")),
            "card NEXT RED / BUILD: {ev:?}"
        );
        assert!(
            ev.iter().any(|e| e.kind == EventKind::InvokeEnd
                && e.card.as_deref() == Some("NEXT RED")
                && e.exit == Some(0)),
            "invoke_end NEXT RED: {ev:?}"
        );
        assert!(
            !ev.iter().any(|e| e.kind == EventKind::Halt),
            "one-card NEXT RED must not halt/close: {ev:?}"
        );
    }

    #[test]
    fn go_closed_with_idea_is_noop() {
        let tmp = Tmp::new();
        fs::create_dir_all(tmp.wm()).unwrap();
        fs::write(tmp.path().join("IDEA.md"), "receipt\n").unwrap();
        fs::write(
            tmp.wm().join("CLOSED"),
            "CLOSED PASS\nindependence: SUBAGENT-ISOLATED\n",
        )
        .unwrap();
        let r = go(tmp.path(), &clock()).unwrap();
        assert_eq!(r.exit, 0);
        assert!(r.stdout.contains("CLOSED PASS"));
        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("card: CLOSED PASS\n"));
        assert!(floor.contains("station: DONE\n"));
        assert!(
            !tmp.wm().join("METRICS.tsv").is_file(),
            "CLOSED no-op must not metrics_append"
        );
        let ev = read_events(tmp.path()).unwrap();
        assert!(
            !ev.iter().any(|e| e.kind == EventKind::Halt),
            "CLOSED no-op must not write EVENTS halt: {ev:?}"
        );
        assert!(!tmp.wm().join("go.pid").exists());
    }

    #[test]
    fn run_true_records_invoke_end_exit_zero() {
        let tmp = Tmp::new();
        let r = run(
            tmp.path(),
            tmp.path(),
            posix_tool("true"),
            &[],
            "sess-1",
            "NEXT RUN maker-build",
            &clock(),
        )
        .unwrap();
        assert_eq!(r.exit, 0);
        assert!(r.elapsed_s >= 0, "elapsed_s is measured, not absent");
        assert!(
            r.elapsed_s < 5,
            "true must not hang: elapsed_s={}",
            r.elapsed_s
        );

        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev.len(), 1, "one invoke_end, no extra kinds: {ev:?}");
        assert_eq!(ev[0].kind, EventKind::InvokeEnd);
        assert_eq!(ev[0].session.as_deref(), Some("sess-1"));
        assert_eq!(ev[0].card.as_deref(), Some("NEXT RUN maker-build"));
        assert_eq!(ev[0].elapsed_s, Some(r.elapsed_s));
        assert_eq!(ev[0].exit, Some(0));
        let text = fs::read_to_string(tmp.wm().join("EVENTS")).unwrap();
        assert!(text.contains("\"kind\":\"invoke_end\""));
        assert!(text.contains("\"session\":\"sess-1\""));
        assert!(text.contains("\"exit\":0"));
        assert!(!text.contains("\"kind\":\"dispatch\""));
        assert!(!tmp.wm().join("go.pid").exists());
    }

    #[test]
    fn run_false_records_invoke_end_nonzero_exit() {
        let tmp = Tmp::new();
        let r = run(
            tmp.path(),
            tmp.path(),
            posix_tool("false"),
            &[],
            "sess-fail",
            "NEXT RUN maker-build",
            &clock(),
        )
        .unwrap();
        assert_eq!(r.exit, 1);
        assert!(r.elapsed_s >= 0);
        assert!(r.elapsed_s < 5);

        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].kind, EventKind::InvokeEnd);
        assert_eq!(ev[0].session.as_deref(), Some("sess-fail"));
        assert_eq!(ev[0].card.as_deref(), Some("NEXT RUN maker-build"));
        assert_eq!(ev[0].elapsed_s, Some(r.elapsed_s));
        assert_eq!(ev[0].exit, Some(1));
        assert!(
            !ev.iter().any(|e| e.kind == EventKind::Halt),
            "nonzero worker exit is invoke_end, not halt: {ev:?}"
        );
        let text = fs::read_to_string(tmp.wm().join("EVENTS")).unwrap();
        assert!(text.contains("\"kind\":\"invoke_end\""));
        assert!(text.contains("\"exit\":1"));
        assert!(!text.contains("\"kind\":\"halt\""));
    }

    #[test]
    fn run_sleep_one_waits_foreground() {
        use std::time::{Duration, Instant};

        let tmp = Tmp::new();
        let marker = tmp.path().join("waited");
        let sleep = posix_tool("sleep");
        let script = format!("{} 1; printf ok > {}", sleep.display(), marker.display());
        let started = Instant::now();
        let r = run(
            tmp.path(),
            tmp.path(),
            posix_tool("sh"),
            &["-c", &script],
            "sess-wait",
            "NEXT RUN maker-build",
            &clock(),
        )
        .unwrap();
        let wall = started.elapsed();
        assert_eq!(r.exit, 0);
        assert!(
            wall >= Duration::from_secs(1),
            "run returned before sleep 1 finished: wall={wall:?} (spawn-without-wait + hardcoded exit 0 stays green on sleep 0)"
        );
        assert!(
            r.elapsed_s >= 1,
            "elapsed_s must reflect the waited child, not hardcoded 0: {}",
            r.elapsed_s
        );
        assert!(
            r.elapsed_s < 5,
            "sleep 1 must not hang: elapsed_s={}",
            r.elapsed_s
        );
        assert!(
            marker.is_file(),
            "marker is written at the end of sleep 1; must exist when run() returns"
        );
        assert_eq!(fs::read_to_string(&marker).unwrap(), "ok");
        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev[0].kind, EventKind::InvokeEnd);
        assert_eq!(ev[0].session.as_deref(), Some("sess-wait"));
        assert_eq!(ev[0].exit, Some(0));
        assert_eq!(ev[0].elapsed_s, Some(r.elapsed_s));
        assert!(ev[0].elapsed_s.unwrap() >= 1);
        assert!(!tmp.wm().join("go.pid").exists());
    }

    #[test]
    fn run_uses_worktree_cwd_and_repo_events() {
        let tmp = Tmp::new();
        let wt = tmp.path().join("worktree");
        fs::create_dir_all(&wt).unwrap();
        let r = run(
            tmp.path(),
            &wt,
            "/bin/sh",
            &["-c", "printf x > here.txt"],
            "sess-wt",
            "NEXT RUN reviewer",
            &clock(),
        )
        .unwrap();
        assert_eq!(r.exit, 0);
        assert_eq!(fs::read_to_string(wt.join("here.txt")).unwrap(), "x");
        assert!(
            !tmp.path().join("here.txt").exists(),
            "child cwd is the worktree, not the repo root"
        );
        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].kind, EventKind::InvokeEnd);
        assert_eq!(ev[0].session.as_deref(), Some("sess-wt"));
        assert_eq!(ev[0].card.as_deref(), Some("NEXT RUN reviewer"));
        assert!(tmp.wm().join("EVENTS").is_file());
        assert!(
            !wt.join(".wm").exists(),
            "EVENTS belong on the repo .wm, not a worktree .wm"
        );
    }

    fn require_git() {
        let ok = Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(
            ok,
            "git is required for worktree tests (do not ignore; mint uses git worktree add)"
        );
    }

    fn git(dir: &Path, args: &[&str]) {
        require_git();
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
        require_git();
        git(dir, &["init", "-q"]);
        git(dir, &["config", "user.name", "crucible"]);
        git(dir, &["config", "user.email", "crucible@example.test"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
        fs::write(dir.join(".gitignore"), ".wm/\n").unwrap();
        fs::write(dir.join("README"), "product\n").unwrap();
        git(dir, &["add", ".gitignore", "README"]);
        git(dir, &["commit", "-qm", "init"]);
    }

    /// Lock child cwd as the minted worktree. `.wm/FALSIFIER` via the product
    /// symlink also succeeds from the repo root.
    fn assert_minted_cwd_marker(repo: &Path, wt: &Path) {
        let marker = wt.join("marker");
        assert!(
            marker.is_file(),
            "child must write marker in .wm/worktrees/<id> (echo FAIL > .wm/FALSIFIER stays green from repo cwd): {}",
            wt.display()
        );
        assert!(
            !repo.join("marker").exists(),
            "marker must not land on the repo root"
        );
        let pwd = fs::read_to_string(&marker).unwrap();
        let pwd_path = PathBuf::from(pwd.trim());
        let pwd_c = fs::canonicalize(&pwd_path).unwrap_or(pwd_path);
        let wt_c = fs::canonicalize(wt).unwrap_or_else(|_| wt.to_path_buf());
        assert_eq!(
            pwd_c,
            wt_c,
            "pwd must be the minted worktree, not the repo root: {}",
            pwd.trim()
        );
    }

    #[test]
    fn mint_worktree_under_wm_run_writes_cwd_events_on_repo() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());

        let wt = mint_worktree(tmp.path(), "s1").unwrap();
        assert_eq!(wt, tmp.wm().join("worktrees").join("s1"));
        assert!(wt.exists(), "worktree path must exist: {}", wt.display());
        assert!(
            wt.join(".git").is_file(),
            "git worktree has a .git file; a clone --local would be a .git directory"
        );
        let list = Command::new("git")
            .args(["worktree", "list"])
            .current_dir(tmp.path())
            .output()
            .expect("git worktree list");
        assert!(list.status.success());
        let list = String::from_utf8_lossy(&list.stdout);
        assert!(
            list.contains("worktrees/s1"),
            "git worktree list must contain worktrees/s1 (not a detached clone): {list}"
        );

        let r = run(
            tmp.path(),
            &wt,
            posix_tool("sh"),
            &["-c", "pwd > marker"],
            "sess-mint",
            "NEXT RUN maker-build",
            &clock(),
        )
        .unwrap();
        assert_eq!(r.exit, 0);
        let marker = wt.join("marker");
        assert!(
            marker.is_file(),
            "child must write marker in .wm/worktrees/s1 (true would stay green if cwd were the repo): {}",
            wt.display()
        );
        assert!(
            !tmp.path().join("marker").exists(),
            "marker must not land on the repo root"
        );
        let pwd = fs::read_to_string(&marker).unwrap();
        let pwd_path = PathBuf::from(pwd.trim());
        let pwd_c = fs::canonicalize(&pwd_path).unwrap_or(pwd_path);
        let wt_c = fs::canonicalize(&wt).unwrap_or_else(|_| wt.clone());
        assert_eq!(
            pwd_c,
            wt_c,
            "pwd must be the minted worktree, not the repo root: {}",
            pwd.trim()
        );

        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev.len(), 1, "one invoke_end on the repo .wm: {ev:?}");
        assert_eq!(ev[0].kind, EventKind::InvokeEnd);
        assert_eq!(ev[0].session.as_deref(), Some("sess-mint"));
        assert_eq!(ev[0].exit, Some(0));
        assert!(tmp.wm().join("EVENTS").is_file());
        assert!(
            !wt.join(".wm").exists(),
            "EVENTS stay on the parent repo .wm, not in the worktree"
        );
        assert!(
            !wt.join(".wm").join("EVENTS").exists(),
            "worktree must not grow its own EVENTS file"
        );

        let r = go(tmp.path(), &clock()).unwrap();
        assert_eq!(r.exit, 1);
        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "this slice has no panel; isolation stays SUBAGENT-ISOLATED: {floor}"
        );
        assert!(!floor.to_ascii_lowercase().contains("grok"));
    }

    #[test]
    fn next_red_go_start_mints_runs_falsifier_next_red_build() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());

        let r = next_red(
            tmp.path(),
            "s1",
            posix_tool("sh"),
            &["-c", "echo FAIL > .wm/FALSIFIER; pwd > marker"],
            "sess-red",
            &clock(),
        )
        .unwrap();

        let wt = tmp.wm().join("worktrees").join("s1");
        assert_eq!(r.worktree, wt);
        assert!(
            wt.join(".git").is_file(),
            "minted path must be a git worktree: {}",
            wt.display()
        );
        assert_eq!(r.exit, 0);
        assert_eq!(r.card, "NEXT RED");
        assert_eq!(r.station, "BUILD");
        assert_minted_cwd_marker(tmp.path(), &wt);

        let fals = tmp.wm().join("FALSIFIER");
        assert!(
            fals.is_file(),
            "POSIX FALSIFIER lives on product .wm (worktree .wm is the same file): {}",
            fals.display()
        );
        let body = fs::read_to_string(&fals).unwrap();
        assert_eq!(body.trim(), "FAIL");
        let wt_fals = wt.join(".wm").join("FALSIFIER");
        assert!(
            wt_fals.is_file(),
            "worktree cwd echo FAIL > .wm/FALSIFIER must resolve (symlink or same path)"
        );

        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("card: NEXT RED\n"), "{floor}");
        assert!(floor.contains("station: BUILD\n"), "{floor}");
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "{floor}"
        );
        assert!(floor.contains("wip: s1\n"), "{floor}");
        assert!(floor.contains("  .wm/FALSIFIER\n"), "{floor}");
        assert!(!floor.to_ascii_lowercase().contains("grok"));

        let ev = read_events(tmp.path()).unwrap();
        assert_eq!(ev[0].kind, EventKind::WalkStart);
        assert!(
            ev.iter().any(|e| e.kind == EventKind::InvokeEnd
                && e.session.as_deref() == Some("sess-red")
                && e.card.as_deref() == Some("NEXT RED")
                && e.exit == Some(0)),
            "invoke_end for NEXT RED: {ev:?}"
        );
        assert!(
            ev.iter().any(|e| e.kind == EventKind::Card
                && e.card.as_deref() == Some("NEXT RED")
                && e.station.as_deref() == Some("BUILD")),
            "card NEXT RED / BUILD: {ev:?}"
        );
        assert!(
            !ev.iter().any(|e| e.kind == EventKind::Halt),
            "present FALSIFIER is NEXT RED, not halt: {ev:?}"
        );
        assert!(tmp.wm().join("EVENTS").is_file());
        assert!(
            !wt.join(".wm").join("EVENTS").exists()
                || fs::canonicalize(wt.join(".wm")).ok() == fs::canonicalize(tmp.wm()).ok(),
            "EVENTS stay on the product .wm"
        );
        assert!(!tmp.wm().join("go.pid").exists());
    }

    #[test]
    fn next_red_missing_falsifier_floor_andon() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());

        let r = next_red(
            tmp.path(),
            "s1",
            posix_tool("sh"),
            &["-c", "pwd > marker"],
            "sess-nofals",
            &clock(),
        )
        .unwrap();

        let wt = tmp.wm().join("worktrees").join("s1");
        assert_eq!(r.worktree, wt);
        assert_eq!(r.exit, 0);
        assert_eq!(r.station, "ANDON");
        assert_minted_cwd_marker(tmp.path(), &wt);
        assert!(
            r.card.starts_with("STOP-ASK"),
            "POSIX missing FALSIFIER refuses red → STOP-ASK / ANDON, got {}",
            r.card
        );
        assert!(
            !tmp.wm().join("FALSIFIER").is_file(),
            "true must not invent FALSIFIER"
        );

        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("station: ANDON\n"), "{floor}");
        assert!(floor.contains(&format!("card: {}\n", r.card)), "{floor}");
        assert!(floor.contains("andon: "), "{floor}");
        assert!(!floor.contains("  .wm/FALSIFIER\n"), "{floor}");
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "{floor}"
        );

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::InvokeEnd
                && e.session.as_deref() == Some("sess-nofals")
                && e.exit == Some(0)),
            "invoke_end still recorded: {ev:?}"
        );
        assert!(
            ev.iter()
                .any(|e| e.kind == EventKind::Card && e.station.as_deref() == Some("ANDON")),
            "ANDON card event: {ev:?}"
        );
        assert_eq!(ev[0].kind, EventKind::WalkStart);
    }

    #[test]
    fn next_red_skips_go_start_when_t0_present() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());
        plant_trace(&tmp.wm(), "NEXT INTAKE");
        let previous = fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap();
        assert!(previous.contains("NEXT INTAKE"));

        let r = next_red(
            tmp.path(),
            "s1",
            posix_tool("sh"),
            &["-c", "echo FAIL > .wm/FALSIFIER; pwd > marker"],
            "sess-live",
            &clock(),
        )
        .unwrap();
        assert_eq!(r.card, "NEXT RED");
        assert_eq!(r.station, "BUILD");
        assert_minted_cwd_marker(tmp.path(), &r.worktree);
        assert!(r.archived.is_none(), "live walk must not archive TRACE");
        assert_eq!(
            fs::read_to_string(tmp.wm().join("t0")).unwrap().trim(),
            &t0().to_string(),
            "go_start would rewrite t0 to now"
        );

        let live = fs::read_to_string(tmp.wm().join("TRACE.tsv")).unwrap();
        assert!(
            live.contains("NEXT INTAKE"),
            "skipping go_start keeps this-run TRACE: {live}"
        );
        assert!(live.contains("NEXT RED"));
        assert!(
            tmp.wm()
                .join("archive")
                .read_dir()
                .ok()
                .is_none_or(|rd| rd.count() == 0),
            "no TRACE archive when t0 already exists"
        );

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            !ev.iter().any(|e| e.kind == EventKind::WalkStart),
            "go_start if needed: t0 present means no walk_start: {ev:?}"
        );
        assert!(
            ev.iter().any(
                |e| e.kind == EventKind::InvokeEnd && e.session.as_deref() == Some("sess-live")
            ),
            "{ev:?}"
        );
    }

    fn worktree_list(repo: &Path) -> String {
        let list = Command::new("git")
            .args(["worktree", "list"])
            .current_dir(repo)
            .output()
            .expect("git worktree list");
        assert!(
            list.status.success(),
            "git worktree list failed: {}",
            String::from_utf8_lossy(&list.stderr)
        );
        String::from_utf8_lossy(&list.stdout).into_owned()
    }

    fn assert_worktree_kept(repo: &Path, id: &str) {
        let wt = repo.join(".wm").join("worktrees").join(id);
        assert!(
            wt.exists(),
            "worktree dir must remain after nonzero exit (remove_worktree on failure would delete it): {}",
            wt.display()
        );
        assert!(
            wt.join(".git").is_file(),
            "git worktree .git file must remain after nonzero exit: {}",
            wt.display()
        );
        let list = worktree_list(repo);
        let needle = format!("worktrees/{id}");
        assert!(
            list.contains(&needle),
            "git worktree list must still contain {needle} after nonzero (git worktree remove on failure would drop it): {list}"
        );
    }

    #[test]
    fn next_red_nonzero_exit_keeps_worktree_and_records_invoke_end() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());

        let r = next_red(
            tmp.path(),
            "s1",
            posix_tool("false"),
            &[],
            "sess-fail",
            &clock(),
        )
        .unwrap();

        assert_eq!(r.exit, 1, "posix false must exit nonzero");
        assert_eq!(r.worktree, tmp.wm().join("worktrees").join("s1"));
        assert_worktree_kept(tmp.path(), "s1");
        assert_eq!(r.station, "ANDON");
        assert!(
            r.card.starts_with("STOP-ASK"),
            "false writes no FALSIFIER → STOP-ASK / ANDON, got {}",
            r.card
        );

        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "{floor}"
        );
        assert!(!floor.to_ascii_lowercase().contains("grok"));

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::InvokeEnd
                && e.session.as_deref() == Some("sess-fail")
                && e.card.as_deref() == Some("NEXT RED")
                && e.exit == Some(1)),
            "invoke_end with nonzero exit must still land on repo .wm: {ev:?}"
        );
        assert!(tmp.wm().join("EVENTS").is_file());
        assert!(
            !r.worktree.join(".wm").join("EVENTS").exists()
                || fs::canonicalize(r.worktree.join(".wm")).ok() == fs::canonicalize(tmp.wm()).ok(),
            "EVENTS stay on the product .wm"
        );
    }

    #[test]
    fn next_red_nonzero_with_falsifier_keeps_next_red_floor_and_worktree() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());

        let r = next_red(
            tmp.path(),
            "s1",
            posix_tool("sh"),
            &["-c", "echo FAIL > .wm/FALSIFIER; exit 1"],
            "sess-red-fail",
            &clock(),
        )
        .unwrap();

        assert_eq!(r.exit, 1);
        assert_eq!(r.card, "NEXT RED");
        assert_eq!(r.station, "BUILD");
        assert_worktree_kept(tmp.path(), "s1");
        assert_eq!(
            fs::read_to_string(tmp.wm().join("FALSIFIER"))
                .unwrap()
                .trim(),
            "FAIL"
        );

        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("card: NEXT RED\n"), "{floor}");
        assert!(floor.contains("station: BUILD\n"), "{floor}");
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "{floor}"
        );

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::InvokeEnd
                && e.session.as_deref() == Some("sess-red-fail")
                && e.exit == Some(1)),
            "invoke_end nonzero on repo .wm: {ev:?}"
        );
        assert!(
            ev.iter().any(|e| e.kind == EventKind::Card
                && e.card.as_deref() == Some("NEXT RED")
                && e.station.as_deref() == Some("BUILD")),
            "child failure must not skip NEXT RED FLOOR: {ev:?}"
        );
    }

    #[test]
    fn next_red_prune_on_success_removes_worktree_only_when_exit_zero() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());
        let opts = NextRedOpts {
            keep_on_failure: true,
            prune_on_success: true,
        };

        let r = next_red_with(
            tmp.path(),
            "s1",
            posix_tool("sh"),
            &["-c", "echo FAIL > .wm/FALSIFIER"],
            "sess-prune",
            &clock(),
            &opts,
        )
        .unwrap();
        assert_eq!(r.exit, 0);
        assert_eq!(r.card, "NEXT RED");
        assert_eq!(r.station, "BUILD");

        let wt = tmp.wm().join("worktrees").join("s1");
        assert!(
            !wt.join(".git").is_file(),
            "prune_on_success must git worktree remove after exit 0: {}",
            wt.display()
        );
        let list = worktree_list(tmp.path());
        assert!(
            !list.contains("worktrees/s1"),
            "git worktree list must not contain worktrees/s1 after prune_on_success: {list}"
        );

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::InvokeEnd
                && e.session.as_deref() == Some("sess-prune")
                && e.exit == Some(0)),
            "{ev:?}"
        );
    }

    #[test]
    fn next_red_prune_on_success_does_not_remove_on_nonzero() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());
        let opts = NextRedOpts {
            keep_on_failure: true,
            prune_on_success: true,
        };

        let r = next_red_with(
            tmp.path(),
            "s1",
            posix_tool("false"),
            &[],
            "sess-prune-fail",
            &clock(),
            &opts,
        )
        .unwrap();
        assert_eq!(r.exit, 1);
        assert_worktree_kept(tmp.path(), "s1");

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::InvokeEnd
                && e.session.as_deref() == Some("sess-prune-fail")
                && e.exit == Some(1)),
            "failure path must not skip invoke_end: {ev:?}"
        );
        assert!(
            tmp.wm().join("FLOOR.md").is_file(),
            "NEXT RED FLOOR write still happens on failure"
        );
        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "{floor}"
        );
    }

    #[test]
    fn close_walk_pass_stamps_closed_halt_lesson_keeps_worktree() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());
        let wt = mint_worktree(tmp.path(), "s1").unwrap();
        assert!(wt.join(".git").is_file());
        fs::create_dir_all(tmp.wm()).unwrap();
        fs::write(tmp.wm().join("t0"), format!("{}\n", t0())).unwrap();

        let r = close_walk(
            tmp.path(),
            "PASS",
            Some("prefer keep worktree on close"),
            3,
            &clock(),
        )
        .unwrap();
        assert_eq!(r.card, "CLOSED PASS");
        assert_eq!(r.station, "DONE");
        assert_eq!(r.elapsed_s, 12);

        let closed = fs::read_to_string(tmp.wm().join("CLOSED")).unwrap();
        assert!(
            closed.starts_with("CLOSED PASS\n"),
            "first line is CLOSED PASS: {closed}"
        );
        assert!(
            closed.contains("independence: SUBAGENT-ISOLATED\n"),
            "{closed}"
        );

        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("station: DONE\n"), "{floor}");
        assert!(floor.contains("card: CLOSED PASS\n"), "{floor}");
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "{floor}"
        );
        assert!(!floor.to_ascii_lowercase().contains("grok"));

        let metrics = fs::read_to_string(tmp.wm().join("METRICS.tsv")).unwrap();
        assert!(metrics.starts_with(METRICS_HEADER));
        let row = metrics.lines().nth(1).unwrap();
        let cols: Vec<&str> = row.split('\t').collect();
        assert_eq!(cols[1], "CLOSED PASS");
        assert_eq!(cols[4], "-");

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::Halt
                && e.card.as_deref() == Some("CLOSED PASS")
                && e.elapsed_s == Some(12)
                && e.iterations == Some(3)),
            "halt CLOSED PASS elapsed_s/iterations: {ev:?}"
        );
        assert!(
            ev.iter().any(|e| e.kind == EventKind::Lesson
                && e.note.as_deref() == Some("prefer keep worktree on close")),
            "EVENTS lesson line: {ev:?}"
        );
        assert_worktree_kept(tmp.path(), "s1");
        assert!(!tmp.wm().join("go.pid").exists());
    }

    #[test]
    fn close_walk_nobuild_stamps_closed_and_halt() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());
        let _wt = mint_worktree(tmp.path(), "s1").unwrap();
        fs::create_dir_all(tmp.wm()).unwrap();
        fs::write(tmp.wm().join("t0"), format!("{}\n", t0())).unwrap();

        let r = close_walk(tmp.path(), "NO-BUILD", Some("no product path"), 1, &clock()).unwrap();
        assert_eq!(r.card, "CLOSED NO-BUILD");
        assert_eq!(r.station, "DONE");
        assert_eq!(r.elapsed_s, 12);

        let closed = fs::read_to_string(tmp.wm().join("CLOSED")).unwrap();
        assert!(closed.starts_with("CLOSED NO-BUILD\n"), "{closed}");
        assert!(
            closed.contains("independence: SUBAGENT-ISOLATED\n"),
            "{closed}"
        );
        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("station: DONE\n"), "{floor}");
        assert!(floor.contains("card: CLOSED NO-BUILD\n"), "{floor}");

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::Halt
                && e.card.as_deref() == Some("CLOSED NO-BUILD")
                && e.elapsed_s == Some(12)
                && e.iterations == Some(1)),
            "halt CLOSED NO-BUILD: {ev:?}"
        );
        assert!(
            ev.iter().any(
                |e| e.kind == EventKind::Lesson && e.note.as_deref() == Some("no product path")
            ),
            "{ev:?}"
        );
        assert_worktree_kept(tmp.path(), "s1");
    }

    #[test]
    fn close_walk_missing_lesson_refuses_and_does_not_write_closed() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());
        let _wt = mint_worktree(tmp.path(), "s1").unwrap();
        fs::create_dir_all(tmp.wm()).unwrap();
        fs::write(tmp.wm().join("t0"), format!("{}\n", t0())).unwrap();

        let err = close_walk(tmp.path(), "PASS", None, 1, &clock()).unwrap_err();
        assert!(
            err.to_string().contains("close without a lesson line"),
            "POSIX cmd_close dies without a lesson: {err}"
        );
        assert!(
            !tmp.wm().join("CLOSED").exists(),
            "missing lesson must not write CLOSED"
        );
        assert!(
            !tmp.wm().join("METRICS.tsv").is_file(),
            "missing lesson must not metrics_append"
        );
        assert!(
            !tmp.wm().join("FLOOR.md").is_file(),
            "missing lesson must not floor_write"
        );
        let ev = read_events(tmp.path()).unwrap();
        assert!(
            !ev.iter()
                .any(|e| e.kind == EventKind::Halt || e.kind == EventKind::Lesson),
            "refuse must not halt/lesson: {ev:?}"
        );
        assert_worktree_kept(tmp.path(), "s1");

        let err = close_walk(tmp.path(), "PASS", Some(""), 1, &clock()).unwrap_err();
        assert!(
            err.to_string().contains("close without a lesson line"),
            "empty lesson: {err}"
        );
        assert!(!tmp.wm().join("CLOSED").exists());

        let err = close_walk(tmp.path(), "PASS", Some("one\ntwo"), 1, &clock()).unwrap_err();
        assert!(
            err.to_string().contains("lesson must be one line"),
            "POSIX close_append_lesson / cmd_close one-line: {err}"
        );
        assert!(!tmp.wm().join("CLOSED").exists());
        assert_worktree_kept(tmp.path(), "s1");
    }

    #[test]
    fn stop_ask_questions_missing_or_empty_answers_floor_halt_keeps_worktree() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());
        let _wt = mint_worktree(tmp.path(), "s1").unwrap();
        fs::create_dir_all(tmp.wm()).unwrap();
        fs::write(tmp.wm().join("t0"), format!("{}\n", t0())).unwrap();
        fs::write(tmp.path().join("QUESTIONS.md"), "What should we build?\n").unwrap();

        let r = stop_ask_questions(tmp.path(), &clock()).unwrap();
        assert!(r.stop);
        assert_eq!(r.exit, 1);
        assert_eq!(r.card, "STOP-ASK QUESTIONS");
        assert_eq!(r.station, "ANDON");
        assert_eq!(r.elapsed_s, 12);

        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("station: ANDON\n"), "{floor}");
        assert!(floor.contains("card: STOP-ASK QUESTIONS\n"), "{floor}");
        assert!(floor.contains("andon: STOP-ASK QUESTIONS\n"), "{floor}");
        assert!(
            floor.contains("independence: SUBAGENT-ISOLATED\n"),
            "{floor}"
        );
        assert!(
            !floor.contains("NEXT "),
            "must not proceed to next card: {floor}"
        );
        assert!(!floor.to_ascii_lowercase().contains("grok"));

        let metrics = fs::read_to_string(tmp.wm().join("METRICS.tsv")).unwrap();
        assert!(metrics.starts_with(METRICS_HEADER));
        let cols: Vec<&str> = metrics.lines().nth(1).unwrap().split('\t').collect();
        assert_eq!(cols[1], "STOP-ASK QUESTIONS");
        assert_eq!(cols[4], "-");

        let ev = read_events(tmp.path()).unwrap();
        assert!(
            ev.iter().any(|e| e.kind == EventKind::Halt
                && e.card.as_deref() == Some("STOP-ASK QUESTIONS")
                && e.elapsed_s == Some(12)
                && e.iterations == Some(0)),
            "halt STOP-ASK QUESTIONS: {ev:?}"
        );
        assert_worktree_kept(tmp.path(), "s1");
        assert!(
            !tmp.path().join("ANSWERS.md").exists(),
            "kernel must not invent ANSWERS.md"
        );
        assert!(!tmp.wm().join("go.pid").exists());

        let tmp = Tmp::new();
        init_git_product(tmp.path());
        let _wt = mint_worktree(tmp.path(), "s1").unwrap();
        fs::create_dir_all(tmp.wm()).unwrap();
        fs::write(tmp.wm().join("t0"), format!("{}\n", t0())).unwrap();
        fs::write(tmp.path().join("QUESTIONS.md"), "What should we build?\n").unwrap();
        fs::write(tmp.path().join("ANSWERS.md"), "").unwrap();
        let r = stop_ask_questions(tmp.path(), &clock()).unwrap();
        assert!(
            r.stop,
            "zero-byte ANSWERS.md is POSIX ! -s, same as missing"
        );
        assert_eq!(r.exit, 1);
        assert_eq!(r.card, "STOP-ASK QUESTIONS");
        assert_eq!(
            fs::read_to_string(tmp.path().join("ANSWERS.md")).unwrap(),
            "",
            "must not invent answers into an empty ANSWERS.md"
        );
        let floor = fs::read_to_string(tmp.wm().join("FLOOR.md")).unwrap();
        assert!(floor.contains("card: STOP-ASK QUESTIONS\n"), "{floor}");
        assert_worktree_kept(tmp.path(), "s1");
    }

    #[test]
    fn stop_ask_questions_present_line_continues_without_inventing() {
        let tmp = Tmp::new();
        init_git_product(tmp.path());
        let _wt = mint_worktree(tmp.path(), "s1").unwrap();
        fs::create_dir_all(tmp.wm()).unwrap();
        fs::write(tmp.wm().join("t0"), format!("{}\n", t0())).unwrap();
        fs::write(tmp.path().join("QUESTIONS.md"), "What should we build?\n").unwrap();
        let answers = "A local hello file is enough.\n";
        fs::write(tmp.path().join("ANSWERS.md"), answers).unwrap();

        let r = stop_ask_questions(tmp.path(), &clock()).unwrap();
        assert!(!r.stop, "non-empty ANSWERS.md is not STOP-ASK QUESTIONS");
        assert_eq!(r.exit, 0);

        assert!(
            !tmp.wm().join("FLOOR.md").is_file(),
            "continue must not floor_write STOP-ASK QUESTIONS"
        );
        assert!(
            !tmp.wm().join("METRICS.tsv").is_file(),
            "continue must not metrics_append"
        );
        let ev = read_events(tmp.path()).unwrap();
        assert!(
            !ev.iter().any(|e| e.kind == EventKind::Halt),
            "present ANSWERS is not a halt: {ev:?}"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("ANSWERS.md")).unwrap(),
            answers,
            "kernel must not invent or rewrite answers"
        );
        assert_worktree_kept(tmp.path(), "s1");
        assert!(!tmp.wm().join("go.pid").exists());
    }
}
