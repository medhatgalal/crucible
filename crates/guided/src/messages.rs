//! Append one factory message to the program record.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crucible_contract::{message_line, Clock, MESSAGE_HEADER};

use crate::message;
use crate::GuidedError;

pub fn append(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if args == ["queue"] {
        return queue(root);
    }
    if args == ["show"] {
        return show(root);
    }
    let (role, kind, text) = match args {
        [role, kind, text] => (*role, *kind, *text),
        _ => {
            return Err(message(
                "usage: crucible message ROLE KIND TEXT | queue | show",
            ))
        }
    };
    let line = message_line(clock.now_unix(), role, kind, text).map_err(message)?;
    let path = root.join("MESSAGES.tsv");
    if !path.is_file() {
        fs::write(&path, MESSAGE_HEADER)?;
    }
    let mut file = OpenOptions::new().append(true).open(&path)?;
    file.write_all(line.as_bytes())?;
    if kind == "source"
        && crate::orchestrate::order_id_ok(text)
        && text != "assembly"
        && !orders_id_present(root, text)?
    {
        write_proposed_orders_row(root, text)?;
    }
    Ok(format!("{}\n", path.display()))
}

fn orders_id_present(root: &Path, id: &str) -> Result<bool, GuidedError> {
    let path = root.join("ORDERS.tsv");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };
    Ok(crate::records(&text)
        .into_iter()
        .skip(1)
        .any(|record| record.split('\t').next() == Some(id)))
}

fn write_proposed_orders_row(root: &Path, id: &str) -> Result<(), GuidedError> {
    let path = root.join("ORDERS.tsv");
    let row = format!("{id}\t-\torders/{id}.paths\torders/{id}.verify.sh\n");
    match fs::read(&path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let mut body = String::from(crate::orchestrate::ORDER_HEADER);
            body.push('\n');
            body.push_str(&row);
            fs::write(&path, body)?;
            Ok(())
        }
        Err(err) => Err(err.into()),
        Ok(existing) => {
            let mut file = OpenOptions::new().append(true).open(&path)?;
            let mut bytes = Vec::new();
            if !existing.is_empty() && !existing.ends_with(b"\n") {
                bytes.push(b'\n');
            }
            bytes.extend_from_slice(row.as_bytes());
            file.write_all(&bytes)?;
            Ok(())
        }
    }
}

/// Dashboard text. No order other than `assembly` prints `idle`.
/// Otherwise four sections: `queue` (`id status`), `graph` (`id depends_on`,
/// including `assembly`), `paused`, and `escalated` (one id per line).
pub fn queue(root: &Path) -> Result<String, GuidedError> {
    let path = root.join("ORDERS.tsv");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok("idle\n".to_string()),
        Err(err) => return Err(err.into()),
    };
    let messages = fs::read_to_string(root.join("MESSAGES.tsv")).unwrap_or_default();
    let mut graph: Vec<(String, String)> = Vec::new();
    let mut watched: Vec<(String, &'static str)> = Vec::new();
    for line in crate::records(&text).into_iter().skip(1) {
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let Some(id) = fields.next() else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        let depends = fields.next().unwrap_or("-");
        graph.push((id.to_string(), depends.to_string()));
        if id != "assembly" {
            watched.push((id.to_string(), order_status(&messages, id)));
        }
    }
    if watched.is_empty() {
        return Ok("idle\n".to_string());
    }
    let mut out = String::from("queue\n");
    for (id, status) in &watched {
        out.push_str(id);
        out.push(' ');
        out.push_str(status);
        out.push('\n');
    }
    out.push_str("graph\n");
    for (id, depends) in &graph {
        out.push_str(id);
        out.push(' ');
        out.push_str(depends);
        out.push('\n');
    }
    out.push_str("paused\n");
    for (id, status) in &watched {
        if *status == "paused" {
            out.push_str(id);
            out.push('\n');
        }
    }
    out.push_str("escalated\n");
    for (id, status) in &watched {
        if *status == "escalated" {
            out.push_str(id);
            out.push('\n');
        }
    }
    Ok(out)
}

/// The message file, or empty when it is absent. The page chat prints this.
fn show(root: &Path) -> Result<String, GuidedError> {
    match fs::read_to_string(root.join("MESSAGES.tsv")) {
        Ok(text) => Ok(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err.into()),
    }
}

fn order_status(messages: &str, id: &str) -> &'static str {
    let mut status = "waiting";
    let mut need_at = None;
    for (idx, line) in messages.lines().enumerate() {
        let mut fields = line.split('\t');
        let Some(_epoch) = fields.next() else {
            continue;
        };
        let Some(_role) = fields.next() else {
            continue;
        };
        let Some(kind) = fields.next() else {
            continue;
        };
        let Some(text) = fields.next() else {
            continue;
        };
        if text != id {
            continue;
        }
        match kind {
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
            "crucible-message-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let clock = FixedClock::new(42);
        append(&dir, &["manager", "source", "fix the greeting"], &clock).unwrap();
        let before = fs::read(&dir.join("MESSAGES.tsv")).unwrap();
        assert_eq!(
            std::str::from_utf8(&before).unwrap(),
            "epoch\trole\tkind\ttext\n42\tmanager\tsource\tfix the greeting\n"
        );
        let err = append(&dir, &["manager", "landed", "no"], &clock).unwrap_err();
        assert!(err.to_string().contains("message refused"), "{err}");
        assert_eq!(fs::read(&dir.join("MESSAGES.tsv")).unwrap(), before);
        assert!(!dir.join("ORDERS.tsv").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn source_legal_id_creates_an_orders_row_and_queue_prints_waiting() {
        let dir = std::env::temp_dir().join(format!(
            "crucible-message-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let clock = FixedClock::new(42);
        let wrote = append(&dir, &["manager", "source", "door"], &clock).unwrap();
        assert_eq!(wrote, format!("{}\n", dir.join("MESSAGES.tsv").display()));
        assert_eq!(
            fs::read_to_string(dir.join("MESSAGES.tsv")).unwrap(),
            "epoch\trole\tkind\ttext\n42\tmanager\tsource\tdoor\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("ORDERS.tsv")).unwrap(),
            "\
order_id\tdepends_on\tpaths_file\tverify_script
door\t-\torders/door.paths\torders/door.verify.sh
"
        );
        assert!(!dir.join("orders").exists());
        assert_eq!(
            append(&dir, &["queue"], &clock).unwrap(),
            "queue\ndoor waiting\ngraph\ndoor -\npaused\nescalated\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repeated_source_send_adds_a_message_and_no_second_orders_row() {
        let dir = std::env::temp_dir().join(format!(
            "crucible-message-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let clock = FixedClock::new(42);
        append(&dir, &["manager", "source", "door"], &clock).unwrap();
        append(&dir, &["manager", "source", "door"], &clock).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("MESSAGES.tsv")).unwrap(),
            "epoch\trole\tkind\ttext\n42\tmanager\tsource\tdoor\n42\tmanager\tsource\tdoor\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("ORDERS.tsv")).unwrap(),
            "\
order_id\tdepends_on\tpaths_file\tverify_script
door\t-\torders/door.paths\torders/door.verify.sh
"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn source_assembly_writes_no_orders_row() {
        let dir = std::env::temp_dir().join(format!(
            "crucible-message-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let clock = FixedClock::new(42);
        append(&dir, &["manager", "source", "assembly"], &clock).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("MESSAGES.tsv")).unwrap(),
            "epoch\trole\tkind\ttext\n42\tmanager\tsource\tassembly\n"
        );
        assert!(!dir.join("ORDERS.tsv").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn machine_landed_writes_no_orders_row() {
        let dir = std::env::temp_dir().join(format!(
            "crucible-message-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let clock = FixedClock::new(42);
        append(&dir, &["machine", "landed", "door"], &clock).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("MESSAGES.tsv")).unwrap(),
            "epoch\trole\tkind\ttext\n42\tmachine\tlanded\tdoor\n"
        );
        assert!(!dir.join("ORDERS.tsv").exists());
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
            dir.join("MESSAGES.tsv"),
            "epoch\trole\tkind\ttext\n1\torchestrator\tdispatched\tA\n2\tmachine\tescalated\tB\n",
        )
        .unwrap();
        assert_eq!(
            queue(&dir).unwrap(),
            "queue\nA dispatched\nB escalated\ngraph\nA -\nB A\nassembly A,B\npaused\nescalated\nB\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn queue_lists_a_pause_and_show_is_the_message_file() {
        let dir =
            std::env::temp_dir().join(format!("crucible-queue-{}-{}", std::process::id(), line!()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(show(&dir).unwrap(), "");
        fs::write(
            dir.join("ORDERS.tsv"),
            "order_id\tdepends_on\tpaths_file\tverify_script\nA\t-\ta.paths\ta.sh\n",
        )
        .unwrap();
        fs::write(
            dir.join("MESSAGES.tsv"),
            "epoch\trole\tkind\ttext\n1\tmachine\tneed-a-fact\tA\n",
        )
        .unwrap();
        assert_eq!(
            queue(&dir).unwrap(),
            "queue\nA paused\ngraph\nA -\npaused\nA\nescalated\n"
        );
        assert_eq!(
            show(&dir).unwrap(),
            "epoch\trole\tkind\ttext\n1\tmachine\tneed-a-fact\tA\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
