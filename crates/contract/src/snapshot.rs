use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clock::Clock;
use crate::layout;

pub const WALK_SCHEMA: &str = "crucible.walk/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkSnapshot {
    pub schema: String,
    pub available: bool,
    pub floor: Option<Floor>,
    pub closed: Option<Closed>,
    pub t0_unix: Option<i64>,
    pub trace: Option<Vec<TraceRow>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Floor {
    pub station: String,
    pub card: String,
    pub wip: String,
    pub andon: String,
    pub independence: String,
    pub elapsed_s: Option<i64>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Closed {
    pub kind: String,
    pub independence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRow {
    pub when: String,
    pub card: String,
    pub station: String,
}

fn unavailable() -> WalkSnapshot {
    WalkSnapshot {
        schema: WALK_SCHEMA.to_string(),
        available: false,
        floor: None,
        closed: None,
        t0_unix: None,
        trace: None,
    }
}

struct ParsedFloor {
    station: String,
    card: String,
    wip: String,
    andon: String,
    independence: String,
    evidence_raw: Vec<String>,
}

fn parse_floor(text: &str) -> Option<ParsedFloor> {
    let mut station = None;
    let mut card = None;
    let mut wip = "-".to_string();
    let mut andon = "-".to_string();
    let mut independence = String::new();
    let mut evidence_raw = Vec::new();
    let mut in_evidence = false;
    for line in text.lines() {
        let raw = line.trim_end_matches('\r');
        if in_evidence {
            if raw.starts_with(' ') || raw.starts_with('\t') {
                let v = raw.trim();
                if !v.is_empty() {
                    evidence_raw.push(v.to_string());
                }
                continue;
            }
            if raw.trim().is_empty() {
                continue;
            }
            in_evidence = false;
        }
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        if t.to_ascii_lowercase().starts_with("evidence:") {
            in_evidence = true;
            if let Some((_, rest)) = t.split_once(':') {
                let rest = rest.trim();
                if !rest.is_empty() {
                    evidence_raw.push(rest.to_string());
                }
            }
            continue;
        }
        if let Some((k, v)) = t.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().to_string();
            match key.as_str() {
                "station" => station = Some(val),
                "card" => card = Some(val),
                "wip" => wip = val,
                "andon" => andon = val,
                "independence" => independence = val,
                _ => {}
            }
        }
    }
    let station = station.filter(|s| !s.is_empty())?;
    let card = card.filter(|s| !s.is_empty())?;
    Some(ParsedFloor {
        station,
        card,
        wip,
        andon,
        independence,
        evidence_raw,
    })
}

fn parse_t0(wm: &Path) -> Option<i64> {
    let text = fs::read_to_string(wm.join("t0")).ok()?;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let first = t.split_whitespace().next()?;
        if first.chars().all(|c| c.is_ascii_digit()) {
            return first.parse().ok();
        }
        return None;
    }
    None
}

fn parse_trace(path: &Path) -> Vec<TraceRow> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
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
        if card.is_empty() {
            continue;
        }
        let station = parts.next().unwrap_or("").to_string();
        rows.push(TraceRow {
            when: when.to_string(),
            card: card.to_string(),
            station,
        });
    }
    rows
}

fn parse_closed(wm: &Path) -> Option<Closed> {
    let text = fs::read_to_string(wm.join("CLOSED")).ok()?;
    let mut kind = None;
    let mut independence = String::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("CLOSED ") {
            let k = rest.trim();
            if k == "PASS" || k == "NO-BUILD" {
                kind = Some(k.to_string());
            }
            continue;
        }
        if let Some((k, v)) = t.split_once(':') {
            if k.trim().eq_ignore_ascii_case("independence") {
                independence = v.trim().to_string();
            }
        }
    }
    Some(Closed {
        kind: kind?,
        independence,
    })
}

fn normalize_evidence(repo: &Path, raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let path = Path::new(raw);
    let rel = if path.is_absolute() {
        if let Ok(stripped) = path.strip_prefix(repo) {
            stripped.to_string_lossy().replace('\\', "/")
        } else if let Some(idx) = raw.find("/.wm/") {
            raw[idx + 1..].replace('\\', "/")
        } else {
            let idx = raw.find("/reviews/")?;
            raw[idx + 1..].replace('\\', "/")
        }
    } else {
        raw.replace('\\', "/")
    };
    let rel = rel.trim_start_matches("./").to_string();
    if rel == ".wm/CLOSED" || rel.ends_with("/CLOSED") && rel.contains(".wm/") {
        return None;
    }
    Some(rel)
}

fn collect_evidence(repo: &Path, listed: &[String]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for raw in listed {
        if let Some(p) = normalize_evidence(repo, raw) {
            set.insert(p);
        }
    }
    let falsifier: (PathBuf, &str) = (repo.join(".wm").join("FALSIFIER"), ".wm/FALSIFIER");
    let review: (PathBuf, &str) = (repo.join("reviews").join("review.md"), "reviews/review.md");
    for (path, rel) in [falsifier, review] {
        if path.is_file() {
            set.insert(rel.to_string());
        }
    }
    if let Ok(rd) = fs::read_dir(repo.join(".wm").join("evidence")) {
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_file() {
                if let Some(name) = p.file_name() {
                    set.insert(format!(".wm/evidence/{}", name.to_string_lossy()));
                }
            }
        }
    }
    set.into_iter().collect()
}

impl WalkSnapshot {
    /// Parse a product repo (or its `.wm` directory). Never writes.
    pub fn from_wm_dir(dir: impl AsRef<Path>, clock: &dyn Clock) -> Self {
        let (repo, wm) = layout::resolve(dir.as_ref());
        if !wm.is_dir() {
            return unavailable();
        }
        let Ok(floor_text) = fs::read_to_string(wm.join("FLOOR.md")) else {
            return unavailable();
        };
        let Some(parsed) = parse_floor(&floor_text) else {
            return unavailable();
        };
        let t0_unix = parse_t0(&wm);
        let elapsed_s = t0_unix.map(|t0| clock.now_unix() - t0);
        let evidence = collect_evidence(&repo, &parsed.evidence_raw);
        let floor = Floor {
            station: parsed.station,
            card: parsed.card,
            wip: parsed.wip,
            andon: parsed.andon,
            independence: parsed.independence,
            elapsed_s,
            evidence,
        };
        WalkSnapshot {
            schema: WALK_SCHEMA.to_string(),
            available: true,
            floor: Some(floor),
            closed: parse_closed(&wm),
            t0_unix,
            trace: Some(parse_trace(&wm.join("TRACE.tsv"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-contract-{}-{}",
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

    const T0: i64 = 1_773_964_800;
    const NOW: i64 = T0 + 12;

    fn clock() -> FixedClock {
        FixedClock::new(NOW)
    }

    #[test]
    fn missing_wm_available_false() {
        let tmp = Tmp::new();
        let snap = WalkSnapshot::from_wm_dir(tmp.path(), &clock());
        assert_eq!(snap.schema, WALK_SCHEMA);
        assert!(!snap.available, "missing .wm must be available: false");
        assert!(snap.floor.is_none());
        assert!(snap.trace.is_none());
        assert!(snap.closed.is_none());
        assert!(snap.t0_unix.is_none());
    }

    #[test]
    fn empty_wm_available_false() {
        let tmp = Tmp::new();
        fs::create_dir_all(tmp.wm()).unwrap();
        let snap = WalkSnapshot::from_wm_dir(tmp.path(), &clock());
        assert!(!snap.available, "empty .wm must not invent INTAKE theatre");
        assert!(snap.floor.is_none());
        assert!(snap.trace.is_none());
        assert!(snap.t0_unix.is_none());
    }

    #[test]
    fn t0_and_trace_without_floor_available_false() {
        let tmp = Tmp::new();
        let wm = tmp.wm();
        fs::create_dir_all(&wm).unwrap();
        fs::write(wm.join("t0"), format!("{T0}\n")).unwrap();
        fs::write(
            wm.join("TRACE.tsv"),
            "when\tcard\toutcome\n2026-09-20T12:00:00Z\tSTOP-ASK INTAKE\tANDON\n",
        )
        .unwrap();
        let snap = WalkSnapshot::from_wm_dir(tmp.path(), &clock());
        assert!(
            !snap.available,
            "fake fail: available true without readable FLOOR after go started"
        );
        assert!(snap.floor.is_none());
    }

    #[test]
    fn garbage_floor_available_false() {
        let tmp = Tmp::new();
        let wm = tmp.wm();
        fs::create_dir_all(&wm).unwrap();
        fs::write(wm.join("FLOOR.md"), "this is not a floor\n").unwrap();
        let snap = WalkSnapshot::from_wm_dir(tmp.path(), &clock());
        assert!(!snap.available);
        assert!(snap.floor.is_none());
    }

    #[test]
    fn readable_floor_without_t0_null_elapsed() {
        let tmp = Tmp::new();
        let wm = tmp.wm();
        fs::create_dir_all(&wm).unwrap();
        fs::write(
            wm.join("FLOOR.md"),
            "station: SHAPE\ncard: NEXT INTAKE\nwip: -\nandon: -\nindependence: SUBAGENT-ISOLATED\nevidence:\n",
        )
        .unwrap();
        let snap = WalkSnapshot::from_wm_dir(tmp.path(), &clock());
        assert!(snap.available);
        assert!(snap.t0_unix.is_none());
        let floor = snap.floor.expect("floor");
        assert!(floor.elapsed_s.is_none(), "do not compute clock - 0");
        assert_eq!(floor.station, "SHAPE");
        assert_eq!(floor.card, "NEXT INTAKE");
    }

    #[test]
    fn golden_floor_trace_relative_evidence_injected_clock() {
        let tmp = Tmp::new();
        let wm = tmp.wm();
        fs::create_dir_all(wm.join("evidence")).unwrap();
        fs::create_dir_all(tmp.path().join("reviews")).unwrap();
        fs::write(wm.join("FALSIFIER"), "x\n").unwrap();
        fs::write(wm.join("evidence").join("shot.txt"), "img\n").unwrap();
        fs::write(tmp.path().join("reviews").join("review.md"), "# r\n").unwrap();
        fs::write(
            wm.join("CLOSED"),
            "CLOSED PASS\nindependence: SUBAGENT-ISOLATED\n",
        )
        .unwrap();
        fs::write(wm.join("t0"), format!("{T0}\n")).unwrap();
        let abs_f = wm.join("FALSIFIER");
        let abs_c = wm.join("CLOSED");
        let abs_e = wm.join("evidence").join("shot.txt");
        let floor = format!(
            "station: ANDON\n\
             card: STOP-ASK INTAKE\n\
             wip: -\n\
             andon: STOP-ASK INTAKE\n\
             independence: SUBAGENT-ISOLATED\n\
             elapsed: 12\n\
             evidence:\n\
               {}\n\
               {}\n\
               reviews/review.md\n\
               {}\n",
            abs_f.display(),
            abs_c.display(),
            abs_e.display()
        );
        fs::write(wm.join("FLOOR.md"), floor).unwrap();
        fs::write(
            wm.join("TRACE.tsv"),
            "when\tcard\toutcome\n2026-09-20T12:00:00Z\tSTOP-ASK INTAKE\tANDON\n",
        )
        .unwrap();

        let snap = WalkSnapshot::from_wm_dir(tmp.path(), &clock());
        assert!(snap.available);
        assert_eq!(snap.t0_unix, Some(T0));
        let floor = snap.floor.expect("floor");
        assert_eq!(floor.station, "ANDON");
        assert_eq!(floor.card, "STOP-ASK INTAKE");
        assert_eq!(floor.wip, "-");
        assert_eq!(floor.andon, "STOP-ASK INTAKE");
        assert_eq!(floor.independence, "SUBAGENT-ISOLATED");
        assert_eq!(floor.elapsed_s, Some(12));
        assert_eq!(
            floor.evidence,
            vec![
                ".wm/FALSIFIER".to_string(),
                ".wm/evidence/shot.txt".to_string(),
                "reviews/review.md".to_string(),
            ]
        );
        assert!(!floor.evidence.iter().any(|p| p.contains("CLOSED")));
        assert!(floor.evidence.iter().all(|p| !p.starts_with('/')));
        let closed = snap.closed.expect("closed");
        assert_eq!(closed.kind, "PASS");
        assert_eq!(closed.independence, "SUBAGENT-ISOLATED");
        let trace = snap.trace.expect("trace");
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].card, "STOP-ASK INTAKE");
        assert_eq!(trace[0].station, "ANDON");
        assert_eq!(trace[0].when, "2026-09-20T12:00:00Z");
    }

    #[test]
    fn from_wm_dir_accepts_dot_wm_path() {
        let tmp = Tmp::new();
        let wm = tmp.wm();
        fs::create_dir_all(&wm).unwrap();
        fs::write(
            wm.join("FLOOR.md"),
            "station: DONE\ncard: CLOSED PASS\nwip: -\nandon: -\nindependence: CROSS-FAMILY\nevidence:\n",
        )
        .unwrap();
        let snap = WalkSnapshot::from_wm_dir(&wm, &clock());
        assert!(snap.available);
        assert_eq!(snap.floor.unwrap().station, "DONE");
    }
}
