//! One factory step. Reads the program directory and appends speech.

use std::fs;
use std::path::Path;

use crucible_contract::Clock;

use crate::dispatch::{graph_error, TaskRow};
use crate::panel::{is_regular, split_tabs};
use crate::speech::speech;
use crate::{message, records, GuidedError};

const ORDER_HEADER: &str = "order_id\tdepends_on\tpaths_file\tverify_script";

pub fn orchestrate(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if args != ["step"] {
        return Err(message("usage: crucible orchestrate step"));
    }
    let orders = read_orders(root)?;
    let said = read_speech(root)?;
    let kept: Vec<&TaskRow> = orders.iter().filter(|row| row.id != "assembly").collect();
    if let Some(order) = kept.iter().find(|row| paused(&said, &row.id)) {
        let id = order.id.as_str();
        let paused_at = speech(root, &["orchestrator", "paused", id], clock)?;
        let asking_at = speech(root, &["orchestrator", "asking", id], clock)?;
        return Ok(format!("{paused_at}{asking_at}"));
    }
    for order in &kept {
        if landed(&said, &order.id) || paused(&said, &order.id) || awaiting(&said, &order.id) {
            continue;
        }
        if deps_landed(&order.deps, &said) {
            return speech(root, &["orchestrator", "dispatched", &order.id], clock);
        }
    }
    Ok("idle\n".to_string())
}

fn read_orders(root: &Path) -> Result<Vec<TaskRow>, GuidedError> {
    let path = root.join("ORDERS.tsv");
    if !is_regular(&path) {
        return Err(message("invalid ORDERS.tsv: missing regular file"));
    }
    let text = fs::read_to_string(&path)?;
    let rows = records(&text);
    if rows.first().copied() != Some(ORDER_HEADER) {
        return Err(message("invalid ORDERS.tsv: header mismatch"));
    }
    let mut orders = Vec::new();
    for (idx, rec) in rows.iter().enumerate().skip(1) {
        let row_no = idx + 1;
        let fields = split_tabs(rec);
        if fields.len() != 4 {
            return Err(message(format!(
                "invalid ORDERS.tsv: row {row_no} has {} fields, need 4",
                fields.len()
            )));
        }
        let id = fields[0];
        if !order_id_ok(id) {
            return Err(message(format!(
                "invalid ORDERS.tsv: invalid order id at row {row_no}"
            )));
        }
        if orders.iter().any(|row: &TaskRow| row.id == id) {
            return Err(message(format!(
                "invalid ORDERS.tsv: duplicate order id {id}"
            )));
        }
        if !order_deps_ok(fields[1]) {
            return Err(message(format!(
                "invalid ORDERS.tsv: invalid dependencies for {id}"
            )));
        }
        orders.push(TaskRow {
            id: id.to_string(),
            deps: normalize_deps(fields[1]),
            paths_file: fields[2].to_string(),
            verify_script: fields[3].to_string(),
        });
    }
    if orders.is_empty() {
        return Err(message("invalid ORDERS.tsv: no orders"));
    }
    if orders.len() > 32 {
        return Err(message("invalid ORDERS.tsv: more than 32 orders"));
    }
    if let Some(err) = graph_error(&orders) {
        return Err(message(format!("invalid ORDERS.tsv: {err}")));
    }
    Ok(orders)
}

/// `assembly` is not a dependency. Empty and `-` mean none.
fn normalize_deps(deps: &str) -> String {
    if deps.is_empty() || deps == "-" {
        return "-".to_string();
    }
    let kept: Vec<&str> = deps
        .split(',')
        .filter(|dep| !dep.is_empty() && *dep != "assembly")
        .collect();
    if kept.is_empty() {
        "-".to_string()
    } else {
        kept.join(",")
    }
}

fn order_id_ok(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn order_deps_ok(deps: &str) -> bool {
    if deps.is_empty() || deps == "-" {
        return true;
    }
    deps.split(',').all(order_id_ok)
}

struct Said {
    role: String,
    sentence: String,
    text: String,
}

fn read_speech(root: &Path) -> Result<Vec<Said>, GuidedError> {
    let text = match fs::read_to_string(root.join("SPEECH.tsv")) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut said = Vec::new();
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 && rec == "epoch\trole\tsentence\ttext" {
            continue;
        }
        if rec.is_empty() {
            continue;
        }
        let fields = split_tabs(rec);
        if fields.len() != 4 {
            continue;
        }
        said.push(Said {
            role: fields[1].to_string(),
            sentence: fields[2].to_string(),
            text: fields[3].to_string(),
        });
    }
    Ok(said)
}

fn landed(said: &[Said], id: &str) -> bool {
    said.iter()
        .any(|row| row.sentence == "landed" && row.text == id)
}

/// A dispatch stands until the order lands or the manager answers.
fn awaiting(said: &[Said], id: &str) -> bool {
    let Some(at) = said
        .iter()
        .rposition(|row| row.sentence == "dispatched" && row.text == id)
    else {
        return false;
    };
    !said.iter().enumerate().any(|(idx, row)| {
        idx > at
            && row.text == id
            && (row.sentence == "landed" || (row.role == "manager" && row.sentence == "answer"))
    })
}

fn paused(said: &[Said], id: &str) -> bool {
    let Some(need) = said
        .iter()
        .rposition(|row| row.sentence == "need-a-fact" && row.text == id)
    else {
        return false;
    };
    !said.iter().enumerate().any(|(idx, row)| {
        idx > need && row.role == "manager" && row.sentence == "answer" && row.text == id
    })
}

fn deps_landed(deps: &str, said: &[Said]) -> bool {
    deps == "-" || deps.split(',').all(|id| landed(said, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::speech;
    use crucible_contract::FixedClock;
    use std::fs;

    fn dir(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("crucible-orch-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn orders(path: &Path, body: &str) {
        fs::write(path.join("ORDERS.tsv"), body).unwrap();
    }

    const GRAPH: &str = "\
order_id\tdepends_on\tpaths_file\tverify_script
A\t-\ta.paths\ta.sh
B\tA,assembly\tb.paths\tb.sh
assembly\tA,B\t-\tassembly.sh
";

    #[test]
    fn step_dispatches_a_again_until_a_lands_then_b() {
        let base = dir("deps");
        let path = base.join("prog");
        fs::create_dir_all(&path).unwrap();
        fs::write(base.join("PRODUCT.txt"), "leave\n").unwrap();
        orders(&path, GRAPH);
        let clock = FixedClock::new(7);
        let before = fs::read(path.join("ORDERS.tsv")).unwrap();
        orchestrate(&path, &["step"], &clock).unwrap();
        assert_eq!(orchestrate(&path, &["step"], &clock).unwrap(), "idle\n");
        let mid = fs::read_to_string(path.join("SPEECH.tsv")).unwrap();
        assert_eq!(
            mid,
            "\
epoch\trole\tsentence\ttext
7\torchestrator\tdispatched\tA
"
        );
        speech(&path, &["machine", "landed", "A"], &clock).unwrap();
        orchestrate(&path, &["step"], &clock).unwrap();
        let landed_b = fs::read_to_string(path.join("SPEECH.tsv")).unwrap();
        assert!(
            landed_b.ends_with("7\torchestrator\tdispatched\tB\n"),
            "{landed_b}"
        );
        assert!(!landed_b.contains("dispatched\tassembly"));
        speech(&path, &["machine", "landed", "B"], &clock).unwrap();
        let held = fs::read(path.join("SPEECH.tsv")).unwrap();
        assert_eq!(orchestrate(&path, &["step"], &clock).unwrap(), "idle\n");
        assert_eq!(fs::read(path.join("SPEECH.tsv")).unwrap(), held);
        assert_eq!(fs::read(path.join("ORDERS.tsv")).unwrap(), before);
        assert_eq!(fs::read(base.join("PRODUCT.txt")).unwrap(), b"leave\n");
        let names = fs::read_dir(&path)
            .unwrap()
            .map(|ent| ent.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(names.contains(&"ORDERS.tsv".to_string()));
        assert!(names.contains(&"SPEECH.tsv".to_string()));
        assert_eq!(names.len(), 2, "{names:?}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn need_a_fact_asks_and_a_later_answer_may_dispatch() {
        let path = dir("pause");
        orders(
            &path,
            "\
order_id\tdepends_on\tpaths_file\tverify_script
A\t-\ta.paths\ta.sh
B\t-\tb.paths\tb.sh
",
        );
        let clock = FixedClock::new(9);
        speech(&path, &["machine", "need-a-fact", "A"], &clock).unwrap();
        orchestrate(&path, &["step"], &clock).unwrap();
        let asked = fs::read_to_string(path.join("SPEECH.tsv")).unwrap();
        assert_eq!(
            asked,
            "\
epoch\trole\tsentence\ttext
9\tmachine\tneed-a-fact\tA
9\torchestrator\tpaused\tA
9\torchestrator\tasking\tA
"
        );
        speech(&path, &["manager", "answer", "A"], &clock).unwrap();
        orchestrate(&path, &["step"], &clock).unwrap();
        let went = fs::read_to_string(path.join("SPEECH.tsv")).unwrap();
        assert!(went.ends_with("9\torchestrator\tdispatched\tA\n"), "{went}");
        assert!(!went.contains("dispatched\tB"));
        let _ = fs::remove_dir_all(&path);
    }
}
