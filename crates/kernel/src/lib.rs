//! File writers for FLOOR, TRACE, and EVENTS. Minimal foreground `go`. No HTTP. No Herdr / Grok / EngOS types.

mod error;
mod events;
mod floor;
mod go;
mod metrics;
mod paths;
mod station;
mod trace;

pub use error::KernelError;
pub use events::{append_event, read_events};
pub use floor::{floor_write, FloorWriteResult};
pub use go::{go, GoRun};
pub use metrics::{metrics_append, METRICS_HEADER};
pub use station::floor_station;
pub use trace::{go_start, GoStart, TRACE_HEADER};

pub use crucible_contract::{
    format_rfc3339_z, parse_rfc3339_z, Clock, Event, EventKind, FixedClock, SystemClock,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crucible_contract::WalkSnapshot;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

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

    #[test]
    fn go_idea_without_closed_refuses_brick_loop() {
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
}
