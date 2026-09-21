use serde::{Deserialize, Serialize};

/// JSONL `kind` values written to `.wm/EVENTS`.
///
/// Design WAL table names `card` / `invoke_end` / `halt`. Slice 1 uses the
/// walker-facing set `go` will emit; see kernel tests / task-2 report Ruling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    WalkStart,
    Card,
    Dispatch,
    StopAsk,
    Escalate,
    Closed,
    Lesson,
}

/// One `.wm/EVENTS` JSONL object. Absent optional fields are omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub t: String,
    pub kind: EventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_s: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iterations: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t0: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub independence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Event {
    fn base(t: impl Into<String>, kind: EventKind) -> Self {
        Self {
            t: t.into(),
            kind,
            card: None,
            station: None,
            session: None,
            role: None,
            elapsed_s: None,
            exit: None,
            iterations: None,
            t0: None,
            independence: None,
            note: None,
        }
    }

    pub fn walk_start(t: impl Into<String>, t0: i64) -> Self {
        let mut e = Self::base(t, EventKind::WalkStart);
        e.t0 = Some(t0);
        e
    }

    pub fn card(t: impl Into<String>, card: impl Into<String>, station: impl Into<String>) -> Self {
        let mut e = Self::base(t, EventKind::Card);
        e.card = Some(card.into());
        e.station = Some(station.into());
        e
    }

    pub fn dispatch(
        t: impl Into<String>,
        card: impl Into<String>,
        session: impl Into<String>,
    ) -> Self {
        let mut e = Self::base(t, EventKind::Dispatch);
        e.card = Some(card.into());
        e.session = Some(session.into());
        e
    }

    pub fn stop_ask(
        t: impl Into<String>,
        card: impl Into<String>,
        elapsed_s: i64,
        iterations: i64,
    ) -> Self {
        let mut e = Self::base(t, EventKind::StopAsk);
        e.card = Some(card.into());
        e.elapsed_s = Some(elapsed_s);
        e.iterations = Some(iterations);
        e
    }

    pub fn escalate(
        t: impl Into<String>,
        card: impl Into<String>,
        elapsed_s: i64,
        iterations: i64,
    ) -> Self {
        let mut e = Self::base(t, EventKind::Escalate);
        e.card = Some(card.into());
        e.elapsed_s = Some(elapsed_s);
        e.iterations = Some(iterations);
        e
    }

    pub fn closed(
        t: impl Into<String>,
        card: impl Into<String>,
        independence: impl Into<String>,
        elapsed_s: i64,
    ) -> Self {
        let mut e = Self::base(t, EventKind::Closed);
        e.card = Some(card.into());
        e.independence = Some(independence.into());
        e.elapsed_s = Some(elapsed_s);
        e
    }

    pub fn lesson(t: impl Into<String>, note: impl Into<String>) -> Self {
        let mut e = Self::base(t, EventKind::Lesson);
        e.note = Some(note.into());
        e
    }

    pub fn to_jsonl_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_jsonl_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_kind_names_are_snake_case() {
        let e = Event::card("2026-09-20T12:00:00Z", "NEXT RED", "BUILD");
        let line = e.to_jsonl_line().unwrap();
        assert!(line.contains("\"kind\":\"card\""));
        assert!(!line.contains("invoke_end"));
        assert!(!line.contains("halt"));
        let back = Event::from_jsonl_line(&line).unwrap();
        assert_eq!(back, e);
    }
}
