//! Append one factory sentence to the program record.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crucible_contract::{speech_line, Clock, SPEECH_HEADER};

use crate::message;
use crate::GuidedError;

pub fn speech(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if args == ["queue"] {
        return queue(root);
    }
    let (role, sentence, text) = match args {
        [role, sentence, text] => (*role, *sentence, *text),
        _ => return Err(message("usage: crucible speech ROLE SENTENCE TEXT")),
    };
    let line = speech_line(clock.now_unix(), role, sentence, text).map_err(message)?;
    let path = root.join("SPEECH.tsv");
    if !path.is_file() {
        fs::write(&path, SPEECH_HEADER)?;
    }
    let mut file = OpenOptions::new().append(true).open(&path)?;
    file.write_all(line.as_bytes())?;
    Ok(format!("{}\n", path.display()))
}

/// One line per order: `id status`. The dashboard and the page both print this.
pub fn queue(root: &Path) -> Result<String, GuidedError> {
    let path = root.join("ORDERS.tsv");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok("idle\n".to_string()),
        Err(err) => return Err(err.into()),
    };
    let speech = fs::read_to_string(root.join("SPEECH.tsv")).unwrap_or_default();
    let mut out = String::new();
    for line in crate::records(&text).into_iter().skip(1) {
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let Some(id) = fields.next() else {
            continue;
        };
        if id.is_empty() || id == "assembly" {
            continue;
        }
        out.push_str(id);
        out.push(' ');
        out.push_str(order_status(&speech, id));
        out.push('\n');
    }
    if out.is_empty() {
        Ok("idle\n".to_string())
    } else {
        Ok(out)
    }
}

fn order_status(speech: &str, id: &str) -> &'static str {
    let mut status = "waiting";
    let mut need_at = None;
    for (idx, line) in speech.lines().enumerate() {
        let mut fields = line.split('\t');
        let Some(_epoch) = fields.next() else {
            continue;
        };
        let Some(_role) = fields.next() else {
            continue;
        };
        let Some(sentence) = fields.next() else {
            continue;
        };
        let Some(text) = fields.next() else {
            continue;
        };
        if text != id {
            continue;
        }
        match sentence {
            "escalated" => status = "escalated",
            "landed" => status = "landed",
            "dispatched" => status = "dispatched",
            "need-a-fact" => need_at = Some(idx),
            "answer" => {
                if need_at.is_some() {
                    status = "waiting";
                    need_at = None;
                }
            }
            _ => {}
        }
    }
    if need_at.is_some() && status != "landed" && status != "escalated" {
        "paused"
    } else {
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crucible_contract::FixedClock;

    #[test]
    fn appends_one_record_and_a_refusal_does_not_change_it() {
        let dir = std::env::temp_dir().join(format!(
            "crucible-speech-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let clock = FixedClock::new(42);
        speech(&dir, &["manager", "source", "fix the greeting"], &clock).unwrap();
        let before = fs::read(&dir.join("SPEECH.tsv")).unwrap();
        assert_eq!(
            std::str::from_utf8(&before).unwrap(),
            "epoch\trole\tsentence\ttext\n42\tmanager\tsource\tfix the greeting\n"
        );
        let err = speech(&dir, &["manager", "landed", "no"], &clock).unwrap_err();
        assert!(err.to_string().contains("speech sentence refused"), "{err}");
        assert_eq!(fs::read(&dir.join("SPEECH.tsv")).unwrap(), before);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn queue_names_each_order_status() {
        let dir =
            std::env::temp_dir().join(format!("crucible-queue-{}-{}", std::process::id(), line!()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("ORDERS.tsv"),
            "order_id\tdepends_on\tpaths_file\tverify_script\nA\t-\ta.paths\ta.sh\nB\tA\tb.paths\tb.sh\nassembly\tA,B\t-\tassembly.sh\n",
        )
        .unwrap();
        fs::write(
            dir.join("SPEECH.tsv"),
            "epoch\trole\tsentence\ttext\n1\torchestrator\tdispatched\tA\n2\tmachine\tescalated\tB\n",
        )
        .unwrap();
        assert_eq!(queue(&dir).unwrap(), "A dispatched\nB escalated\n");
        let _ = fs::remove_dir_all(&dir);
    }
}
