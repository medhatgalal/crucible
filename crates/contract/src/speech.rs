//! Factory speech. One record, three speakers, the sentences each may say.

pub const SPEECH_HEADER: &str = "epoch\trole\tsentence\ttext\n";

const MANAGER: &[&str] = &["source", "correction", "answer"];
const MACHINE: &[&str] = &["started", "need-a-fact", "escalated", "landed"];
const ORCHESTRATOR: &[&str] = &["dispatched", "paused", "advanced", "asking"];

/// One TSV record. A role may say only its own sentences. Text is one field.
pub fn speech_line(
    epoch: i64,
    role: &str,
    sentence: &str,
    text: &str,
) -> Result<String, &'static str> {
    let allowed = match role {
        "manager" => MANAGER,
        "machine" => MACHINE,
        "orchestrator" => ORCHESTRATOR,
        _ => return Err("speech sentence refused"),
    };
    if !allowed.contains(&sentence) {
        return Err("speech sentence refused");
    }
    if text.is_empty() || text.bytes().any(|b| matches!(b, b'\t' | b'\n' | b'\r')) {
        return Err("speech sentence refused");
    }
    Ok(format!("{epoch}\t{role}\t{sentence}\t{text}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_speaker_has_only_its_sentences() {
        let line = speech_line(10, "manager", "source", "a one-liner").unwrap();
        assert_eq!(line, "10\tmanager\tsource\ta one-liner\n");
        assert!(speech_line(10, "machine", "landed", "done").is_ok());
        assert!(speech_line(10, "orchestrator", "paused", "need the repo").is_ok());
        assert_eq!(
            speech_line(10, "manager", "landed", "no"),
            Err("speech sentence refused")
        );
        assert_eq!(
            speech_line(10, "guest", "source", "no"),
            Err("speech sentence refused")
        );
        assert_eq!(
            speech_line(10, "manager", "source", "has\ttab"),
            Err("speech sentence refused")
        );
    }
}
