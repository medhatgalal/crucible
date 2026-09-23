use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::event::{Event, EventKind};
use crate::layout;
use crate::timeutil::{format_rfc3339_z, parse_rfc3339_z, parse_since};

pub const STATS_SCHEMA: &str = "crucible.stats/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsError {
    pub message: String,
}

impl std::fmt::Display for StatsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StatsError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsWindow {
    pub schema: String,
    pub available: bool,
    pub since: String,
    pub until: String,
    pub source: String,
    pub counts: StatsCounts,
    pub halts: Vec<Halt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsCounts {
    pub halt: u64,
    pub card: u64,
    pub invoke_end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Halt {
    pub t: String,
    pub outcome: String,
    pub slices: i64,
    pub bound: i64,
    pub note: String,
    pub elapsed_s: Option<i64>,
    pub iterations: Option<i64>,
}

fn blank_window(since: String, until: String, source: &str) -> StatsWindow {
    StatsWindow {
        schema: STATS_SCHEMA.to_string(),
        available: false,
        since,
        until,
        source: source.to_string(),
        counts: StatsCounts {
            halt: 0,
            card: 0,
            invoke_end: 0,
        },
        halts: Vec::new(),
    }
}

/// One bad line must not fail the window or drop the rest of the file.
fn window_from_events(
    text: &str,
    since_unix: i64,
    now: i64,
    since_s: String,
    until_s: String,
) -> StatsWindow {
    let mut halts = Vec::new();
    let mut card = 0u64;
    let mut invoke_end = 0u64;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(ev) = Event::from_jsonl_line(line) else {
            continue;
        };
        let Some(tu) = parse_rfc3339_z(&ev.t) else {
            continue;
        };
        if tu < since_unix || tu > now {
            continue;
        }
        match ev.kind {
            EventKind::Card => card += 1,
            EventKind::InvokeEnd => invoke_end += 1,
            EventKind::Halt => {
                let outcome = ev.card.unwrap_or_default();
                if outcome.is_empty() {
                    continue;
                }
                halts.push(Halt {
                    t: ev.t,
                    outcome,
                    slices: 0,
                    bound: 0,
                    note: ev.note.unwrap_or_else(|| "-".to_string()),
                    elapsed_s: ev.elapsed_s,
                    iterations: ev.iterations,
                });
            }
            EventKind::WalkStart | EventKind::Lesson => {}
        }
    }
    let halt_n = halts.len() as u64;
    StatsWindow {
        schema: STATS_SCHEMA.to_string(),
        available: true,
        since: since_s,
        until: until_s,
        source: "events".to_string(),
        counts: StatsCounts {
            halt: halt_n,
            card,
            invoke_end,
        },
        halts,
    }
}

impl StatsWindow {
    /// Readable `.wm/EVENTS` is the only source. Missing EVENTS keeps `.wm/METRICS.tsv`.
    /// `since` is RFC3339 Z or `Ns`/`Nm`/`Nh`/`Nd`.
    pub fn from_wm_dir(
        dir: impl AsRef<Path>,
        since: &str,
        clock: &dyn Clock,
    ) -> Result<Self, StatsError> {
        let now = clock.now_unix();
        let since_unix = parse_since(since, now).ok_or_else(|| StatsError {
            message: format!(
                "invalid --since {since:?}; want RFC3339 Zulu or duration Ns/Nm/Nh/Nd"
            ),
        })?;
        let until_s = format_rfc3339_z(now);
        let since_s = format_rfc3339_z(since_unix);
        let (_repo, wm) = layout::resolve(dir.as_ref());
        let events_path = wm.join("EVENTS");
        match fs::read_to_string(&events_path) {
            Ok(text) => {
                return Ok(window_from_events(&text, since_unix, now, since_s, until_s));
            }
            // Missing WAL falls back. Any other read error must not invent METRICS rows.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Ok(blank_window(since_s, until_s, "events")),
        }
        let path = wm.join("METRICS.tsv");
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(blank_window(since_s, until_s, "metrics"));
        };
        let mut halts = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if i == 0 && line.starts_with("when") {
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            let mut parts = line.split('\t');
            let t = parts.next().unwrap_or("").to_string();
            let outcome = parts.next().unwrap_or("").to_string();
            if outcome.is_empty() {
                continue;
            }
            let slices = parts.next().unwrap_or("0").parse().unwrap_or(0);
            let bound = parts.next().unwrap_or("0").parse().unwrap_or(0);
            let note = {
                let n = parts.next().unwrap_or("-");
                if n.is_empty() {
                    "-".to_string()
                } else {
                    n.to_string()
                }
            };
            let Some(tu) = parse_rfc3339_z(&t) else {
                continue;
            };
            if tu < since_unix || tu > now {
                continue;
            }
            halts.push(Halt {
                t,
                outcome,
                slices,
                bound,
                note,
                elapsed_s: None,
                iterations: None,
            });
        }
        let halt_n = halts.len() as u64;
        Ok(StatsWindow {
            schema: STATS_SCHEMA.to_string(),
            available: true,
            since: since_s,
            until: until_s,
            source: "metrics".to_string(),
            counts: StatsCounts {
                halt: halt_n,
                card: 0,
                invoke_end: 0,
            },
            halts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-stats-{}-{}",
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

    fn now_unix() -> i64 {
        parse_rfc3339_z("2026-09-20T12:00:00Z").unwrap()
    }

    fn write_metrics(root: &Path, body: &str) {
        let wm = root.join(".wm");
        fs::create_dir_all(&wm).unwrap();
        fs::write(wm.join("METRICS.tsv"), body).unwrap();
    }

    const METRICS: &str = "\
when\toutcome\tslices\tbound\tnote
2026-09-20T01:00:00Z\tSTOP-ASK QUESTIONS\t0\t40\t-
2026-09-20T11:00:00Z\tCLOSED PASS\t1\t40\t-
2026-09-13T12:00:00Z\tSTOP-ASK INTAKE\t0\t40\told
";

    #[test]
    fn missing_metrics_available_false_no_invented_rows() {
        let tmp = Tmp::new();
        fs::create_dir_all(tmp.root.join(".wm")).unwrap();
        let w = StatsWindow::from_wm_dir(&tmp.root, "8h", &FixedClock::new(now_unix())).unwrap();
        assert!(!w.available);
        assert_eq!(w.source, "metrics");
        assert!(w.halts.is_empty());
        assert_eq!(w.counts.halt, 0);
        assert_eq!(w.counts.card, 0);
        assert_eq!(w.counts.invoke_end, 0);
    }

    #[test]
    fn stats_window_8h_filters_metrics() {
        let tmp = Tmp::new();
        write_metrics(&tmp.root, METRICS);
        let now = now_unix();
        let w = StatsWindow::from_wm_dir(&tmp.root, "8h", &FixedClock::new(now)).unwrap();
        assert!(w.available);
        assert_eq!(w.source, "metrics");
        assert_eq!(w.until, format_rfc3339_z(now));
        assert_eq!(w.since, format_rfc3339_z(now - 8 * 3600));
        assert_eq!(w.halts.len(), 1);
        assert_eq!(w.halts[0].outcome, "CLOSED PASS");
        assert_eq!(w.halts[0].slices, 1);
        assert_eq!(w.halts[0].bound, 40);
        assert_eq!(w.halts[0].note, "-");
        assert!(w.halts[0].elapsed_s.is_none());
        assert!(w.halts[0].iterations.is_none());
        assert_eq!(w.counts.halt, 1);
        assert_eq!(w.counts.card, 0);
        assert_eq!(w.counts.invoke_end, 0);
    }

    #[test]
    fn stats_window_24h_and_7d() {
        let tmp = Tmp::new();
        write_metrics(&tmp.root, METRICS);
        let now = now_unix();
        let w24 = StatsWindow::from_wm_dir(&tmp.root, "24h", &FixedClock::new(now)).unwrap();
        assert_eq!(
            w24.halts
                .iter()
                .map(|h| h.outcome.as_str())
                .collect::<Vec<_>>(),
            vec!["STOP-ASK QUESTIONS", "CLOSED PASS"]
        );
        let w7 = StatsWindow::from_wm_dir(&tmp.root, "7d", &FixedClock::new(now)).unwrap();
        assert_eq!(w7.halts.len(), 3);
        let rfc =
            StatsWindow::from_wm_dir(&tmp.root, "2026-09-20T10:00:00Z", &FixedClock::new(now))
                .unwrap();
        assert_eq!(rfc.halts.len(), 1);
        assert_eq!(rfc.halts[0].outcome, "CLOSED PASS");
    }

    #[test]
    fn invalid_since_is_error() {
        let tmp = Tmp::new();
        let err = StatsWindow::from_wm_dir(&tmp.root, "yesterday", &FixedClock::new(now_unix()));
        assert!(err.is_err());
    }

    #[test]
    fn stats_reads_events_wal_inside_window() {
        let tmp = Tmp::new();
        write_metrics(&tmp.root, METRICS);
        let events = tmp.root.join(".wm").join("EVENTS");
        fs::write(
            &events,
            concat!(
                "{not json}\n",
                r#"{"t":"2026-09-20T11:32:00+00:00","kind":"halt","card":"SKIP-OFFSET"}"#,
                "\n",
                r#"{"t":"not-a-time","kind":"halt","card":"SKIP-TIME"}"#,
                "\n",
                r#"{"t":"2026-09-20T01:00:00Z","kind":"card","card":"OUTSIDE","station":"BUILD"}"#,
                "\n",
                r#"{"t":"2026-09-20T11:30:00Z","kind":"card","card":"NEXT RED","station":"BUILD"}"#,
                "\n",
                r#"{"t":"2026-09-20T11:31:00Z","kind":"invoke_end","session":"00000000-0000-0000-0000-000000000001","card":"NEXT RUN maker-build","elapsed_s":7,"exit":0}"#,
                "\n",
                r#"{"t":"2026-09-20T11:32:00Z","kind":"halt","card":"STOP-ASK FROM-EVENTS","elapsed_s":90,"iterations":3}"#,
                "\n",
                r#"{"t":"2026-09-20T11:33:00Z","kind":"halt","card":"STOP-ASK NOTED","note":"kept","elapsed_s":4,"iterations":1}"#,
                "\n",
            ),
        )
        .unwrap();
        let metrics_before = fs::read(tmp.root.join(".wm").join("METRICS.tsv")).unwrap();
        let events_before = fs::read(&events).unwrap();
        let now = now_unix();
        let w = StatsWindow::from_wm_dir(&tmp.root, "8h", &FixedClock::new(now)).unwrap();
        assert!(w.available);
        assert_eq!(w.source, "events");
        assert_eq!(w.halts.len(), 2);
        assert_eq!(w.halts[0].t, "2026-09-20T11:32:00Z");
        assert_eq!(w.halts[0].outcome, "STOP-ASK FROM-EVENTS");
        assert_eq!(w.halts[0].slices, 0);
        assert_eq!(w.halts[0].bound, 0);
        assert_eq!(w.halts[0].note, "-");
        assert_eq!(w.halts[0].elapsed_s, Some(90));
        assert_eq!(w.halts[0].iterations, Some(3));
        assert_eq!(w.halts[1].outcome, "STOP-ASK NOTED");
        assert_eq!(w.halts[1].note, "kept");
        assert_eq!(w.halts[1].slices, 0);
        assert_eq!(w.halts[1].bound, 0);
        assert_eq!(w.counts.halt, 2);
        assert_eq!(w.counts.card, 1);
        assert_eq!(w.counts.invoke_end, 1);
        assert!(!w.halts.iter().any(|h| {
            h.outcome.contains("CLOSED")
                || h.outcome.contains("QUESTIONS")
                || h.outcome.contains("SKIP")
        }));
        assert_eq!(
            fs::read(tmp.root.join(".wm").join("METRICS.tsv")).unwrap(),
            metrics_before
        );
        assert_eq!(fs::read(&events).unwrap(), events_before);
    }
}
