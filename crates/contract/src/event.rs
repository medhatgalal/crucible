use serde::{Deserialize, Serialize};

/// JSONL `kind` values written to `.wm/EVENTS`.
///
/// Design WAL table (binding): `card`, `invoke_end`, `halt`.
/// Extra kinds `walk_start` and `lesson` are walker bookkeeping, not replacements.
///
/// Mapping: dispatch → [`EventKind::InvokeEnd`] (`session`, `card`, `elapsed_s`, `exit`);
/// stop_ask / escalate / closed / independence unavailable → [`EventKind::Halt`]
/// with [`Event::card`] holding the outcome string (e.g. `STOP-ASK QUESTIONS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    WalkStart,
    Card,
    InvokeEnd,
    Halt,
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

    /// Walker `dispatch` → design `invoke_end`.
    pub fn invoke_end(
        t: impl Into<String>,
        card: impl Into<String>,
        session: impl Into<String>,
        elapsed_s: i64,
        exit: i32,
    ) -> Self {
        let mut e = Self::base(t, EventKind::InvokeEnd);
        e.card = Some(card.into());
        e.session = Some(session.into());
        e.elapsed_s = Some(elapsed_s);
        e.exit = Some(exit);
        e
    }

    /// Walker stop_ask / escalate / closed / independence unavailable → design `halt`.
    /// `outcome` is stored in `card` (e.g. `STOP-ASK QUESTIONS`, `CLOSED PASS`).
    pub fn halt(
        t: impl Into<String>,
        outcome: impl Into<String>,
        elapsed_s: i64,
        iterations: i64,
    ) -> Self {
        let mut e = Self::base(t, EventKind::Halt);
        e.card = Some(outcome.into());
        e.elapsed_s = Some(elapsed_s);
        e.iterations = Some(iterations);
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
    fn jsonl_kind_names_include_design_wal_table() {
        let card = Event::card("2026-09-20T12:00:00Z", "NEXT RED", "BUILD");
        let inv = Event::invoke_end(
            "2026-09-20T12:01:00Z",
            "NEXT RUN maker-build",
            "sess-1",
            7,
            0,
        );
        let halt = Event::halt("2026-09-20T12:02:00Z", "STOP-ASK QUESTIONS", 90, 3);
        let card_l = card.to_jsonl_line().unwrap();
        let inv_l = inv.to_jsonl_line().unwrap();
        let halt_l = halt.to_jsonl_line().unwrap();
        assert!(card_l.contains("\"kind\":\"card\""));
        assert!(inv_l.contains("\"kind\":\"invoke_end\""));
        assert!(inv_l.contains("\"session\":\"sess-1\""));
        assert!(inv_l.contains("\"elapsed_s\":7"));
        assert!(inv_l.contains("\"exit\":0"));
        assert!(halt_l.contains("\"kind\":\"halt\""));
        assert!(halt_l.contains("\"card\":\"STOP-ASK QUESTIONS\""));
        assert!(halt_l.contains("\"elapsed_s\":90"));
        assert!(halt_l.contains("\"iterations\":3"));
        assert_eq!(Event::from_jsonl_line(&inv_l).unwrap(), inv);
        assert_eq!(Event::from_jsonl_line(&halt_l).unwrap(), halt);
    }
}
