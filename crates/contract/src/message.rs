//! One factory message. Three roles, and the kinds each role may record.

pub const MESSAGE_HEADER: &str = "epoch\trole\tkind\ttext\n";

const MANAGER: &[&str] = &["source", "correction", "answer"];
const MACHINE: &[&str] = &["started", "need-a-fact", "escalated", "landed"];
const ORCHESTRATOR: &[&str] = &["dispatched", "paused", "advanced", "asking"];

/// One TSV record. A role may record only its own kinds. Text is one field.
pub fn message_line(
    epoch: i64,
    role: &str,
    kind: &str,
    text: &str,
) -> Result<String, &'static str> {
    let allowed = match role {
        "manager" => MANAGER,
        "machine" => MACHINE,
        "orchestrator" => ORCHESTRATOR,
        _ => return Err("message refused"),
    };
    if !allowed.contains(&kind) {
        return Err("message refused");
    }
    if text.is_empty() || text.bytes().any(|b| matches!(b, b'\t' | b'\n' | b'\r')) {
        return Err("message refused");
    }
    Ok(format!("{epoch}\t{role}\t{kind}\t{text}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_role_has_only_its_kinds() {
        let line = message_line(10, "manager", "source", "a one-liner").unwrap();
        assert_eq!(line, "10\tmanager\tsource\ta one-liner\n");
        assert!(message_line(10, "machine", "landed", "done").is_ok());
        assert!(message_line(10, "orchestrator", "paused", "need the repo").is_ok());
        assert_eq!(
            message_line(10, "manager", "landed", "no"),
            Err("message refused")
        );
        assert_eq!(
            message_line(10, "guest", "source", "no"),
            Err("message refused")
        );
        assert_eq!(
            message_line(10, "manager", "source", "has\ttab"),
            Err("message refused")
        );
    }
}
