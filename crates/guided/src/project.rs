use std::path::Path;

use crucible_contract::{format_rfc3339_z, Clock, Event};
use crucible_kernel::{append_event, floor_write_with, FloorWriteResult, KernelError};

/// Map one cycle line onto `.wm/FLOOR.md` through [`floor_write_with`].
///
/// Always projects. Callers skip working-mode programs themselves.
pub fn project_cycle_line(
    product_repo: impl AsRef<Path>,
    cycle_line: &str,
    independence: &str,
    clock: &dyn Clock,
) -> Result<FloorWriteResult, KernelError> {
    let card = if cycle_line.starts_with("WAIT PANEL") || cycle_line.starts_with("WAIT APPROVAL") {
        format!("STOP-ASK {cycle_line}")
    } else {
        cycle_line.to_string()
    };
    let wrote = floor_write_with(product_repo.as_ref(), &card, independence, clock, true)?;
    // Unchanged cards skip TRACE inside floor_write_with; do not append a second halt.
    let halt = wrote.card.starts_with("STOP-ASK WAIT PANEL")
        || wrote.card.starts_with("STOP-ASK WAIT APPROVAL")
        || wrote.card.starts_with("ESCALATE");
    if wrote.trace_appended && halt {
        append_event(
            product_repo.as_ref(),
            &Event::halt(
                format_rfc3339_z(clock.now_unix()),
                &wrote.card,
                wrote.elapsed_s,
                0,
            ),
        )?;
    }
    Ok(wrote)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crucible_contract::{EventKind, FixedClock, WalkSnapshot};
    use crucible_kernel::read_events;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-guided-project-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &std::path::Path {
            &self.root
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn floor(dir: &std::path::Path, clock: &FixedClock) -> crucible_contract::Floor {
        let snap = WalkSnapshot::from_wm_dir(dir, clock);
        assert!(snap.available, "walk snapshot missing: {snap:?}");
        assert_eq!(snap.schema, "crucible.walk/v1");
        assert!(snap.closed.is_none());
        assert!(!dir.join(".wm").join("CLOSED").exists());
        snap.floor.expect("floor")
    }

    fn trace_len(dir: &std::path::Path, clock: &FixedClock) -> usize {
        WalkSnapshot::from_wm_dir(dir, clock)
            .trace
            .unwrap_or_default()
            .len()
    }

    fn halt_cards(dir: &std::path::Path) -> Vec<String> {
        read_events(dir)
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == EventKind::Halt)
            .map(|e| e.card.unwrap_or_default())
            .collect()
    }

    #[test]
    fn projects_cycle_lines_into_walk_snapshot() {
        let tmp = Tmp::new();
        let clock = FixedClock::new(1_700_000_000);
        let when = format_rfc3339_z(clock.now_unix());
        let dir = tmp.path();

        let execute =
            "NEXT EXECUTE alpha — dispatch dependency-ready tasks or integrate passing work";
        let wrote = project_cycle_line(dir, execute, "-", &clock).unwrap();
        assert!(wrote.trace_appended);
        assert_eq!(wrote.station, "BUILD");
        assert_eq!(wrote.card, execute);
        assert_eq!(wrote.elapsed_s, 0);
        let f = floor(dir, &clock);
        assert_eq!(f.station, "BUILD");
        assert_eq!(f.card, execute);
        assert_eq!(f.andon, "-");
        assert_eq!(f.independence, "-");
        assert_eq!(f.elapsed_s, Some(0));
        assert!(halt_cards(dir).is_empty());
        let snap = WalkSnapshot::from_wm_dir(dir, &clock);
        assert_eq!(snap.t0_unix, Some(1_700_000_000));
        let trace = snap.trace.unwrap();
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].when, when);
        assert_eq!(trace[0].card, execute);
        assert_eq!(trace[0].station, "BUILD");

        let review =
            "NEXT REVIEW alpha — independently falsify current work; close or return findings";
        let wrote = project_cycle_line(dir, review, "-", &clock).unwrap();
        assert!(wrote.trace_appended);
        let f = floor(dir, &clock);
        assert_eq!(f.station, "INSPECT");
        assert_eq!(f.card, review);
        assert_eq!(f.andon, "-");
        assert!(halt_cards(dir).is_empty());

        let inflight = "WAIT alpha — agent work or review is in flight (A1)";
        project_cycle_line(dir, inflight, "-", &clock).unwrap();
        let f = floor(dir, &clock);
        assert_eq!(f.station, "WAIT");
        assert_eq!(f.card, inflight);
        assert_eq!(f.andon, "-");
        assert!(halt_cards(dir).is_empty());

        let panel = "WAIT PANEL — show agent inventory and role casting p1 to the operator; do not investigate or build";
        let panel_card = format!("STOP-ASK {panel}");
        let wrote = project_cycle_line(dir, panel, "-", &clock).unwrap();
        assert!(wrote.trace_appended);
        assert_eq!(wrote.card, panel_card);
        assert_eq!(wrote.station, "ANDON");
        let f = floor(dir, &clock);
        assert_eq!(f.card, panel_card);
        assert_eq!(f.station, "ANDON");
        assert_eq!(f.andon, panel_card);
        assert_eq!(halt_cards(dir), vec![panel_card.clone()]);
        let halts: Vec<_> = read_events(dir)
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == EventKind::Halt)
            .collect();
        assert_eq!(halts.len(), 1);
        assert_eq!(halts[0].t, when);
        assert_eq!(halts[0].card.as_deref(), Some(panel_card.as_str()));
        assert_eq!(halts[0].elapsed_s, Some(0));
        assert_eq!(halts[0].iterations, Some(0));

        let before = trace_len(dir, &clock);
        let events_before = read_events(dir).unwrap().len();
        let again = project_cycle_line(dir, panel, "-", &clock).unwrap();
        assert!(!again.trace_appended);
        assert_eq!(trace_len(dir, &clock), before);
        assert_eq!(read_events(dir).unwrap().len(), events_before);
        assert_eq!(halt_cards(dir), vec![panel_card]);

        let approval = "WAIT APPROVAL — show proposal p1 to the operator; do not plan or build";
        let approval_card = format!("STOP-ASK {approval}");
        project_cycle_line(dir, approval, "-", &clock).unwrap();
        let f = floor(dir, &clock);
        assert_eq!(f.card, approval_card);
        assert_eq!(f.station, "ANDON");
        assert_eq!(f.andon, approval_card);
        assert_eq!(
            halt_cards(dir),
            vec![format!("STOP-ASK {panel}"), approval_card]
        );

        let escalate = "ESCALATE alpha — blocked";
        project_cycle_line(dir, escalate, "-", &clock).unwrap();
        let f = floor(dir, &clock);
        assert_eq!(f.card, escalate);
        assert_eq!(f.station, "ANDON");
        assert_eq!(f.andon, escalate);
        assert_eq!(halt_cards(dir).last().map(String::as_str), Some(escalate));

        let halt_n = halt_cards(dir).len();
        let trace_n = trace_len(dir, &clock);
        let again = project_cycle_line(dir, escalate, "-", &clock).unwrap();
        assert!(!again.trace_appended);
        assert_eq!(trace_len(dir, &clock), trace_n);
        assert_eq!(halt_cards(dir).len(), halt_n);

        let done = "DONE — no admittable claim remains. Next: cycle clean --dry-run (or cycle problem FILE --next). Drive never --apply.";
        project_cycle_line(dir, done, "-", &clock).unwrap();
        let f = floor(dir, &clock);
        assert_eq!(f.card, done);
        assert_eq!(f.station, "DONE");
        assert_eq!(f.andon, "-");
        assert_eq!(halt_cards(dir).len(), halt_n);

        project_cycle_line(dir, "DONE", "-", &clock).unwrap();
        assert_eq!(floor(dir, &clock).station, "DONE");
        assert_eq!(halt_cards(dir).len(), halt_n);

        for line in [
            "NEXT CONFIGURE — cast personas to independent agents in PANEL.ASSIGN.tsv (role→agent) and complete PANEL.md",
            "NEXT INVESTIGATE — split PROBLEM.md into atomic claims and verify them via independent agents",
            "NEXT PROPOSE — write a refined, evidence-grounded PROPOSAL.md",
            "NEXT PLAN alpha — validate the bounded breakdown before execution",
            "NEXT PLAN — admit the next bounded item from approved proposal p1",
            "NEXT INTAKE — capture the operator's problem in PROBLEM.md",
        ] {
            project_cycle_line(dir, line, "-", &clock).unwrap();
            let f = floor(dir, &clock);
            assert_eq!(f.card, line, "{line}");
            assert_eq!(f.station, "SHAPE", "{line}");
            assert_eq!(f.andon, "-", "{line}");
        }
        assert_eq!(halt_cards(dir).len(), halt_n, "shape lines are not halts");
    }

    #[test]
    fn repeated_build_card_does_not_append_trace() {
        let tmp = Tmp::new();
        let clock = FixedClock::new(1_700_000_111);
        let line = "NEXT EXECUTE alpha — make, verify, review, and fix until accepted";
        let first = project_cycle_line(tmp.path(), line, "-", &clock).unwrap();
        assert!(first.trace_appended);
        let second = project_cycle_line(tmp.path(), line, "-", &clock).unwrap();
        assert!(!second.trace_appended);
        assert_eq!(trace_len(tmp.path(), &clock), 1);
        let events = read_events(tmp.path()).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::Card);
        assert!(halt_cards(tmp.path()).is_empty());
    }

    #[test]
    fn empty_independence_is_not_rewritten() {
        let tmp = Tmp::new();
        let clock = FixedClock::new(1_700_000_200);
        project_cycle_line(tmp.path(), "NEXT EXECUTE alpha — dispatch", "", &clock).unwrap();
        assert_eq!(floor(tmp.path(), &clock).independence, "");
    }
}
