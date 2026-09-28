//! One factory step, or a run of steps until idle or asking.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use crucible_contract::Clock;

use crate::dispatch::{graph_error, is_executable, TaskRow};
use crate::panel::{is_regular, split_tabs};
use crate::speech::speech;
use crate::{message, records, GuidedError};

const ORDER_HEADER: &str = "order_id\tdepends_on\tpaths_file\tverify_script";

pub fn orchestrate(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    match args {
        ["step"] => orchestrate_step(root, clock),
        ["run"] => orchestrate_run(root, clock),
        _ => Err(message("usage: crucible orchestrate step|run")),
    }
}

fn orchestrate_run(root: &Path, clock: &dyn Clock) -> Result<String, GuidedError> {
    let orders = read_orders(root)?;
    let initial = read_speech(root)?;
    check_existing_landed(root, &orders, &initial)?;
    let prior_complete = prior_success(root, &orders, &initial)?;
    let mut assembly_ran_here = false;
    for _ in 0..64 {
        refuse_stopped_awaiting(root, &orders)?;
        let before = read_speech(root)?;
        let last = orchestrate_step(root, clock)?;
        let after = read_speech(root)?;
        let fresh = fresh_rows(&before, &after);
        if fresh.iter().any(|row| {
            row.role == "machine"
                && row.text == "assembly"
                && (row.sentence == "landed" || row.sentence == "escalated")
        }) {
            assembly_ran_here = true;
        }
        if let Some(id) = fresh.iter().find_map(|row| {
            (row.role == "orchestrator" && row.sentence == "asking").then(|| row.text.clone())
        }) {
            wait_for_answer(root, &id)?;
            continue;
        }
        if let Some(id) = fresh.iter().find_map(|row| {
            (row.role == "orchestrator" && row.sentence == "dispatched").then(|| row.text.clone())
        }) {
            deliver(root, clock, &id)?;
            continue;
        }
        if fresh.iter().any(|row| {
            row.role == "machine" && row.sentence == "escalated" && row.text == "assembly"
        }) {
            speech(root, &["orchestrator", "asking", "assembly"], clock)?;
            wait_for_answer(root, "assembly")?;
            continue;
        }
        if fresh
            .iter()
            .any(|row| row.role == "machine" && row.sentence == "landed" && row.text == "assembly")
        {
            continue;
        }
        if last == "idle\n" {
            let said = read_speech(root)?;
            let makers_done = all_makers_landed(root, &orders, &said)?;
            let has_assembly = orders.iter().any(|row| row.id == "assembly");
            if makers_done && has_assembly && !assembly_ran_here && !prior_complete {
                return Err(message("assembly was not run by this process"));
            }
            if outcome_idle(root, &orders, &said, assembly_ran_here, prior_complete)? {
                return Ok("idle\n".to_string());
            }
            if let Some(id) = deliver_awaiting(root, clock, &orders, &said)? {
                deliver(root, clock, &id)?;
                continue;
            }
            if let Some(id) = open_ask(&orders, &said) {
                wait_for_answer(root, &id)?;
                continue;
            }
            let id = orders
                .iter()
                .find(|row| row.id != "assembly" && !landed(&said, &row.id))
                .map(|row| row.id.as_str())
                .unwrap_or("order");
            return Err(message(format!(
                "orchestrate run idle without outcome for {id}"
            )));
        }
    }
    Err(message("orchestrate run exceeded 64 steps"))
}

fn fresh_rows<'a>(before: &'a [Said], after: &'a [Said]) -> &'a [Said] {
    if after.len() > before.len() {
        &after[before.len()..]
    } else {
        &[]
    }
}

fn wait_for_answer(root: &Path, id: &str) -> Result<(), GuidedError> {
    loop {
        let said = read_speech(root)?;
        let asking_at = said.iter().rposition(|row| {
            row.role == "orchestrator" && row.sentence == "asking" && row.text == id
        });
        if let Some(at) = asking_at {
            let answered = said.iter().enumerate().any(|(idx, row)| {
                idx > at && row.role == "manager" && row.sentence == "answer" && row.text == id
            });
            if answered {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn orchestrate_step(root: &Path, clock: &dyn Clock) -> Result<String, GuidedError> {
    let orders = read_orders(root)?;
    let mut said = read_speech(root)?;
    let kept: Vec<&TaskRow> = orders.iter().filter(|row| row.id != "assembly").collect();
    let mut wrote = String::new();
    for order in &kept {
        let id = order.id.as_str();
        if paused(&said, id) && !asked_after(&said, id, "need-a-fact") {
            wrote.push_str(&speech(root, &["orchestrator", "paused", id], clock)?);
            wrote.push_str(&speech(root, &["orchestrator", "asking", id], clock)?);
        } else if escalated_open(&said, id) && !asked_after(&said, id, "escalated") {
            wrote.push_str(&speech(root, &["orchestrator", "asking", id], clock)?);
        }
    }
    said = read_speech(root)?;
    for order in &kept {
        let id = order.id.as_str();
        if landed(&said, id)
            || paused(&said, id)
            || escalated_open(&said, id)
            || awaiting(&said, id)
        {
            continue;
        }
        if deps_landed(&order.deps, &said) {
            wrote.push_str(&speech(root, &["orchestrator", "dispatched", id], clock)?);
            return Ok(if wrote.is_empty() {
                "idle\n".to_string()
            } else {
                wrote
            });
        }
    }
    if let Some(line) = run_assembly_once(root, &orders, &said, clock)? {
        wrote.push_str(&line);
    }
    if wrote.is_empty() {
        Ok("idle\n".to_string())
    } else {
        Ok(wrote)
    }
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

fn escalated_open(said: &[Said], id: &str) -> bool {
    let Some(at) = said
        .iter()
        .rposition(|row| row.sentence == "escalated" && row.text == id)
    else {
        return false;
    };
    !said.iter().enumerate().any(|(idx, row)| {
        idx > at && row.role == "manager" && row.sentence == "answer" && row.text == id
    })
}

fn asked_after(said: &[Said], id: &str, trigger: &str) -> bool {
    let Some(at) = said
        .iter()
        .rposition(|row| row.sentence == trigger && row.text == id)
    else {
        return false;
    };
    said.iter().enumerate().any(|(idx, row)| {
        idx > at && row.role == "orchestrator" && row.sentence == "asking" && row.text == id
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

fn run_assembly_once(
    root: &Path,
    orders: &[TaskRow],
    said: &[Said],
    clock: &dyn Clock,
) -> Result<Option<String>, GuidedError> {
    let Some(assembly) = orders.iter().find(|row| row.id == "assembly") else {
        return Ok(None);
    };
    if orders
        .iter()
        .filter(|row| row.id != "assembly")
        .any(|row| !landed(said, &row.id))
    {
        return Ok(None);
    }
    if landed(said, "assembly") || escalated_open(said, "assembly") {
        return Ok(None);
    }
    let script = root.join(&assembly.verify_script);
    if !is_regular(&script) || !is_executable(&script) {
        return Err(message(format!(
            "invalid ORDERS.tsv: order assembly: {}",
            assembly.verify_script
        )));
    }
    let shebang = fs::read_to_string(&script)
        .ok()
        .and_then(|text| records(&text).first().copied().map(str::to_string))
        .unwrap_or_default();
    if shebang != "#!/bin/sh" {
        return Err(message(
            "invalid ORDERS.tsv: order assembly: script must start with #!/bin/sh",
        ));
    }
    let code = match Command::new(&script).current_dir(root).output() {
        Ok(out) => out.status.code().unwrap_or(1),
        Err(_) => 127,
    };
    let sentence = if code == 0 { "landed" } else { "escalated" };
    Ok(Some(speech(
        root,
        &["machine", sentence, "assembly"],
        clock,
    )?))
}

fn deps_landed(deps: &str, said: &[Said]) -> bool {
    deps == "-" || deps.split(',').all(|id| landed(said, id))
}

fn check_existing_landed(
    root: &Path,
    orders: &[TaskRow],
    said: &[Said],
) -> Result<(), GuidedError> {
    for order in orders.iter().filter(|row| row.id != "assembly") {
        if landed(said, &order.id) {
            require_maker_landed(root, &order.id)?;
        }
    }
    Ok(())
}

fn prior_success(root: &Path, orders: &[TaskRow], said: &[Said]) -> Result<bool, GuidedError> {
    let makers: Vec<&TaskRow> = orders.iter().filter(|row| row.id != "assembly").collect();
    if makers.is_empty() || !orders.iter().any(|row| row.id == "assembly") {
        return Ok(false);
    }
    if !landed(said, "assembly") {
        return Ok(false);
    }
    for order in makers {
        if !landed(said, &order.id) || !maker_landed(root, &order.id)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn all_makers_landed(root: &Path, orders: &[TaskRow], said: &[Said]) -> Result<bool, GuidedError> {
    for order in orders.iter().filter(|row| row.id != "assembly") {
        if !landed(said, &order.id) {
            return Ok(false);
        }
        if !maker_landed(root, &order.id)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn outcome_idle(
    root: &Path,
    orders: &[TaskRow],
    said: &[Said],
    assembly_ran_here: bool,
    prior_complete: bool,
) -> Result<bool, GuidedError> {
    if !all_makers_landed(root, orders, said)? {
        return Ok(false);
    }
    if orders
        .iter()
        .any(|row| row.id != "assembly" && awaiting(said, &row.id))
    {
        return Ok(false);
    }
    let has_assembly = orders.iter().any(|row| row.id == "assembly");
    if !has_assembly {
        return Ok(true);
    }
    if escalated_open(said, "assembly") {
        return Ok(false);
    }
    let assembly_landed = said.iter().any(|row| {
        row.role == "machine" && row.sentence == "landed" && row.text == "assembly"
    });
    Ok(assembly_landed && (assembly_ran_here || prior_complete))
}

fn maker_landed(root: &Path, id: &str) -> Result<bool, GuidedError> {
    Ok(classify_landed(root, id)? == LandedClass::Real)
}

fn require_maker_landed(root: &Path, id: &str) -> Result<(), GuidedError> {
    match classify_landed(root, id)? {
        LandedClass::Real => Ok(()),
        LandedClass::NoResult => Err(message(format!("landed {id} has no PASS CLOSE result"))),
        LandedClass::NoShell => Err(message(format!("landed {id} has no maker shell"))),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LandedClass {
    Real,
    NoResult,
    NoShell,
}

fn classify_landed(root: &Path, id: &str) -> Result<LandedClass, GuidedError> {
    let mut saw_result = false;
    for attempt in maker_attempts(root, id)? {
        let result_path = crate::cycle::attempt_dir(root, &attempt)?.join("result.md");
        if !result_path.is_file() {
            continue;
        }
        let body = fs::read_to_string(&result_path)?;
        if crate::attempt::result_field(&body, "OUTCOME") == "PASS"
            && crate::attempt::result_field(&body, "NEXT") == "CLOSE"
            && crate::attempt::result_field(&body, "ITEM") == id
        {
            saw_result = true;
            if crate::cycle::attempt_state(root, &attempt)? == "RETURNED"
                && last_reason(root, &attempt)? == "drive worker exit 0"
            {
                return Ok(LandedClass::Real);
            }
        }
    }
    if saw_result {
        Ok(LandedClass::NoShell)
    } else {
        Ok(LandedClass::NoResult)
    }
}

fn maker_attempts(root: &Path, id: &str) -> Result<Vec<String>, GuidedError> {
    let mut out = Vec::new();
    for path in crate::cycle::attempt_dirs_a(root) {
        let attempt = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        if attempt.is_empty() {
            continue;
        }
        if crate::cycle::attempt_meta(root, &attempt, 2).unwrap_or_default() == id
            && crate::cycle::attempt_meta(root, &attempt, 5).unwrap_or_default() == "maker"
        {
            out.push(attempt);
        }
    }
    Ok(out)
}

fn newest_maker(root: &Path, id: &str) -> Result<Option<String>, GuidedError> {
    let mut best: Option<(i64, u64, String)> = None;
    for attempt in maker_attempts(root, id)? {
        let epoch = crate::cycle::attempt_meta(root, &attempt, 11)?
            .parse::<i64>()
            .unwrap_or(0);
        let n = attempt
            .rsplit('.')
            .next()
            .and_then(|part| part.parse::<u64>().ok())
            .unwrap_or(0);
        let replace = match &best {
            None => true,
            Some((best_epoch, best_n, _)) => epoch > *best_epoch || (epoch == *best_epoch && n > *best_n),
        };
        if replace {
            best = Some((epoch, n, attempt));
        }
    }
    Ok(best.map(|(_, _, attempt)| attempt))
}

fn last_reason(root: &Path, id: &str) -> Result<String, GuidedError> {
    let text = fs::read_to_string(crate::cycle::attempt_dir(root, id)?.join("events.tsv"))?;
    let mut reason = String::new();
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        if let Some(field) = split_tabs(rec).get(3) {
            reason = (*field).to_string();
        }
    }
    Ok(reason)
}

fn refuse_stopped_awaiting(root: &Path, orders: &[TaskRow]) -> Result<(), GuidedError> {
    let said = read_speech(root)?;
    for order in orders.iter().filter(|row| row.id != "assembly") {
        if !awaiting(&said, &order.id) {
            continue;
        }
        let Some(attempt) = newest_maker(root, &order.id)? else {
            continue;
        };
        let state = crate::cycle::attempt_state(root, &attempt)?;
        if state == "STOPPED" {
            return Err(message(format!(
                "maker shell exited nonzero for {}",
                order.id
            )));
        }
        if state == "TIMEOUT" {
            return Err(message(format!("drive tick timed out for {}", order.id)));
        }
    }
    Ok(())
}

fn speech_next(orders: &[TaskRow], said: &[Said]) -> Option<String> {
    orders.iter().find_map(|row| {
        let id = row.id.as_str();
        (id != "assembly"
            && !landed(said, id)
            && !paused(said, id)
            && !escalated_open(said, id)
            && !awaiting(said, id)
            && deps_landed(&row.deps, said))
        .then(|| row.id.clone())
    })
}

fn open_ask(orders: &[TaskRow], said: &[Said]) -> Option<String> {
    orders.iter().find_map(|row| {
        let id = row.id.as_str();
        let open = (paused(said, id) || escalated_open(said, id))
            && said.iter().any(|speech_row| {
                speech_row.role == "orchestrator"
                    && speech_row.sentence == "asking"
                    && speech_row.text == id
            });
        open.then(|| row.id.clone())
    })
}

fn current_slug(root: &Path) -> Result<Option<String>, GuidedError> {
    let text = match fs::read_to_string(root.join("STATE.tsv")) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields = split_tabs(rec);
        let status = fields.get(1).copied().unwrap_or("");
        if status == "ACTIVE" || status == "BLOCKED" {
            return Ok(fields.first().map(|slug| (*slug).to_string()));
        }
    }
    Ok(None)
}

fn state_fields(root: &Path, slug: &str) -> Result<Vec<String>, GuidedError> {
    let text = fs::read_to_string(root.join("STATE.tsv"))?;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let fields: Vec<String> = split_tabs(rec).into_iter().map(str::to_string).collect();
        if fields.first().map(String::as_str) == Some(slug) && fields.len() == 8 {
            return Ok(fields);
        }
    }
    Err(message(format!("item missing from STATE.tsv: {slug}")))
}

fn deliver_awaiting(
    root: &Path,
    clock: &dyn Clock,
    orders: &[TaskRow],
    said: &[Said],
) -> Result<Option<String>, GuidedError> {
    let Some(order) = orders.iter().find(|row| row.id != "assembly" && awaiting(said, &row.id))
    else {
        return Ok(None);
    };
    if paused(said, &order.id) || escalated_open(said, &order.id) {
        return Ok(None);
    }
    let current = current_slug(root)?;
    if current.as_deref() == Some(order.id.as_str()) {
        let fields = state_fields(root, &order.id)?;
        if fields.get(1).map(String::as_str) == Some("ACTIVE") {
            return Ok(Some(order.id.clone()));
        }
    }
    if current.is_none() || current.as_deref().is_some_and(|slug| landed(said, slug)) {
        handoff(root, clock, said, &order.id)?;
        return Ok(Some(order.id.clone()));
    }
    Ok(None)
}

fn handoff(root: &Path, clock: &dyn Clock, said: &[Said], activate: &str) -> Result<(), GuidedError> {
    let row = state_fields(root, activate)?;
    if row.get(1).map(String::as_str) != Some("CLOSED") || row.get(2).map(String::as_str) != Some("BUILD")
    {
        return Err(message("refused: next order is not a closed item"));
    }
    let item = root.join("items").join(activate);
    if !item.is_dir()
        || !item.join("evidence").is_dir()
        || !item.join("plan-audit.md").is_file()
        || !fs::read_to_string(item.join("plan-audit.md"))
            .unwrap_or_default()
            .contains("VERDICT: PASS")
        || !fs::read_to_string(item.join("ITEM.md"))
            .unwrap_or_default()
            .contains("A1")
        || item.join("TASKS.tsv").exists()
        || !root.join("roles/maker.md").is_file()
    {
        return Err(message("refused: next order is not a closed item"));
    }
    let current = current_slug(root)?;
    match current {
        None => {
            crate::state_update_item(
                root,
                clock,
                activate,
                "ACTIVE",
                "BUILD",
                row.get(3).map(String::as_str).unwrap_or("EMPTY"),
                row.get(4).map(String::as_str).unwrap_or("LOW"),
                "-",
                "-",
            )?;
        }
        Some(slug) if landed(said, &slug) => {
            let current_row = state_fields(root, &slug)?;
            crate::state_update_item(
                root,
                clock,
                &slug,
                "CLOSED",
                current_row.get(2).map(String::as_str).unwrap_or("BUILD"),
                current_row.get(3).map(String::as_str).unwrap_or("EMPTY"),
                current_row.get(4).map(String::as_str).unwrap_or("LOW"),
                "-",
                "-",
            )?;
            crate::state_update_item(
                root,
                clock,
                activate,
                "ACTIVE",
                "BUILD",
                row.get(3).map(String::as_str).unwrap_or("EMPTY"),
                row.get(4).map(String::as_str).unwrap_or("LOW"),
                "-",
                "-",
            )?;
        }
        Some(slug) => {
            return Err(message(format!(
                "refused: {slug} is still the current item"
            )));
        }
    }
    Ok(())
}

fn deliver(root: &Path, clock: &dyn Clock, order: &str) -> Result<(), GuidedError> {
    if !root.join("items").join(order).is_dir() {
        return Err(message(format!("no such item: {order}")));
    }
    let said = read_speech(root)?;
    if paused(&said, order) || escalated_open(&said, order) {
        return Err(message(format!("refused: {order} is still the current item")));
    }
    let current = current_slug(root)?;
    if current.as_deref() != Some(order) {
        let promotable = current.is_none() || current.as_deref().is_some_and(|slug| landed(&said, slug));
        if !promotable {
            let slug = current.unwrap_or_default();
            return Err(message(format!("refused: {slug} is still the current item")));
        }
        handoff(root, clock, &said, order)?;
    }
    let maker = crate::drive::cast_agents(root, "maker")?
        .into_iter()
        .next()
        .ok_or_else(|| message(format!("no such item: {order}")))?;
    let attempt = match newest_maker(root, order)? {
        Some(id) => id,
        None => {
            let dispatched = crate::dispatch::dispatch(root, &[order, "maker", &maker], clock)?;
            Path::new(dispatched.trim())
                .parent()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string()
        }
    };
    if attempt.is_empty() {
        return Err(message(format!("drive tick did not start {order}")));
    }
    let state = crate::cycle::attempt_state(root, &attempt)?;
    if state == "STOPPED" {
        return Err(message(format!("maker shell exited nonzero for {order}")));
    }
    if state == "TIMEOUT" {
        return Err(message(format!("drive tick timed out for {order}")));
    }
    if state == "RETURNED" {
        if last_reason(root, &attempt)? != "drive worker exit 0" {
            return Err(message(format!("landed {order} has no maker shell")));
        }
        return record_result(root, clock, order, &attempt);
    }
    if state == "RUNNING" {
        return Err(message(format!("drive tick did not start {order}")));
    }
    let contract = crate::cycle::attempt_dir(root, &attempt)?.join("contract.md");
    let invoked = crate::dispatch::invocation(root, &maker, &contract.display().to_string())?;
    let first = invoked.split_whitespace().next().unwrap_or("");
    if invoked.is_empty() || invoked.starts_with("(no command registered") || first == "true" {
        return Err(message(format!("drive tick did not start {order}")));
    }
    if !crate::claims::claim_attempt_is_sealed(root, &attempt) {
        if !contract.with_file_name("transport").is_file()
            && !crate::cycle::attempt_dir(root, &attempt)?.join("transport").is_file()
        {
            crate::attempt::attempt(root, &["transport", &attempt, "multi-agent"], clock)?;
        }
        let auditor = crate::claims::suggest_contract_auditor(root)?;
        if auditor.is_empty() || auditor == "<auditor-name>" || auditor == maker {
            return Err(message(format!("drive tick did not start {order}")));
        }
        crate::audit::contract_audit(
            root,
            &[
                &attempt,
                &auditor,
                "PASS",
                "factory orchestrate run sealed the maker shell; the shell starts only in drive tick",
            ],
            clock,
        )?;
        if !crate::claims::claim_attempt_is_sealed(root, &attempt) {
            return Err(message(format!("drive tick did not start {order}")));
        }
    }
    if let Err(err) = crate::drive::drive(root, &["tick"], clock) {
        return Err(message(format!("drive tick failed for {order}: {err}")));
    }
    let state = crate::cycle::attempt_state(root, &attempt)?;
    let reason = last_reason(root, &attempt).unwrap_or_default();
    if state == "RETURNED" && reason == "drive worker exit 0" {
        return record_result(root, clock, order, &attempt);
    }
    if state == "STOPPED" || (reason.starts_with("drive worker exit ") && reason != "drive worker exit 0")
    {
        return Err(message(format!("maker shell exited nonzero for {order}")));
    }
    if state == "TIMEOUT" {
        return Err(message(format!("drive tick timed out for {order}")));
    }
    Err(message(format!("drive tick did not start {order}")))
}

fn record_result(root: &Path, clock: &dyn Clock, order: &str, attempt: &str) -> Result<(), GuidedError> {
    let before = read_speech(root)?;
    let evidence = evidence_name(root, order, attempt)?;
    crate::result::result(root, &[attempt, "PASS", &evidence, "CLOSE"], clock)?;
    let after = read_speech(root)?;
    let landed_now = fresh_rows(&before, &after).iter().any(|row| {
        row.role == "machine" && row.sentence == "landed" && row.text == order
    });
    if !landed_now {
        return Err(message(format!("result did not land {order}")));
    }
    let said = read_speech(root)?;
    if let Some(next) = speech_next(read_orders(root)?.as_slice(), &said) {
        handoff(root, clock, &said, &next)?;
    }
    Ok(())
}

fn evidence_name(root: &Path, order: &str, attempt: &str) -> Result<String, GuidedError> {
    let dir = root.join("items").join(order).join("evidence");
    let mut hits = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|_| message(format!("result did not land {order}")))?;
    for ent in entries {
        let ent = ent.map_err(|_| message(format!("result did not land {order}")))?;
        let name = ent.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".txt") {
            continue;
        }
        let body = fs::read_to_string(ent.path()).unwrap_or_default();
        if body.starts_with(crate::cycle::MARK)
            && body.lines().any(|line| line == format!("attempt-id: {attempt}"))
        {
            hits.push(name);
        }
    }
    if hits.len() != 1 {
        return Err(message(format!("result did not land {order}")));
    }
    Ok(hits.remove(0))
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
        let script = path.join("assembly.sh");
        fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = fs::metadata(&script).unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(&script, perm).unwrap();
        }
        orchestrate(&path, &["step"], &clock).unwrap();
        let assembled = fs::read_to_string(path.join("SPEECH.tsv")).unwrap();
        assert!(
            assembled.ends_with("7\tmachine\tlanded\tassembly\n"),
            "{assembled}"
        );
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
        assert!(names.contains(&"assembly.sh".to_string()));
        assert_eq!(names.len(), 3, "{names:?}");
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
9\torchestrator\tdispatched\tB
"
        );
        let asked = fs::read_to_string(path.join("SPEECH.tsv")).unwrap();
        assert!(
            asked.contains("9\torchestrator\tdispatched\tB\n"),
            "{asked}"
        );
        speech(&path, &["manager", "answer", "A"], &clock).unwrap();
        orchestrate(&path, &["step"], &clock).unwrap();
        let went = fs::read_to_string(path.join("SPEECH.tsv")).unwrap();
        assert!(went.contains("9\torchestrator\tdispatched\tA\n"), "{went}");
        let _ = fs::remove_dir_all(&path);
    }
}
