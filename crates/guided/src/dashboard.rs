//! One read of records that already exist. No message queue and no writes.

use std::fs;
use std::path::{Path, PathBuf};

use crate::state::STATE_HEADER;

const RECORD_CAP: usize = 262_144;
const IDEA_CAP: usize = 200;

/// Headings `agents`, `repo`, `reviews`, `next`, `blocked`, `git`, and `intake`.
/// A missing record leaves the heading with nothing under it.
pub fn dashboard(cwd: &Path) -> String {
    let program = factory_dir(cwd);
    let repo_value = crate::cycle::program_field(&program, "repo");
    let product = product_dir(cwd, repo_value.as_deref());
    let mut out = match crate::agents::agent_records(&program) {
        Ok(text) => text,
        Err(_) => "agents\nunreadable\n".to_string(),
    };
    push_section(&mut out, "repo", &repo_line(repo_value.as_deref()));
    push_section(&mut out, "reviews", &review_body(&product));
    let state = read_state(&program);
    push_section(&mut out, "next", &state_lines(&state, "ACTIVE"));
    push_section(&mut out, "blocked", &state_lines(&state, "BLOCKED"));
    push_section(&mut out, "git", &git_body(&program, &product));
    push_section(&mut out, "intake", &intake_body(&product));
    out
}

fn push_section(out: &mut String, heading: &str, body: &str) {
    out.push_str(heading);
    out.push('\n');
    out.push_str(body);
}

fn repo_line(repo: Option<&str>) -> String {
    match repo {
        Some(repo) if !repo.is_empty() && !repo.contains(['\n', '\r']) => format!("{repo}\n"),
        _ => String::new(),
    }
}

fn factory_dir(cwd: &Path) -> PathBuf {
    if is_regular_file(&cwd.join("PROGRAM")) {
        return cwd.to_path_buf();
    }
    guided_program(cwd).unwrap_or_else(|| cwd.to_path_buf())
}

fn guided_program(cwd: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    let entries = fs::read_dir(cwd.join(".crucible")).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if is_real_dir(&path) && program_matches(&path, cwd) {
            found.push(path);
        }
    }
    if found.len() == 1 {
        found.pop()
    } else {
        None
    }
}

fn program_matches(dir: &Path, toplevel: &Path) -> bool {
    let Ok(text) = fs::read_to_string(dir.join("PROGRAM")) else {
        return false;
    };
    let lines = crate::records(&text);
    if !lines.contains(&"cycle: guided") {
        return false;
    }
    if lines
        .iter()
        .find_map(|line| line.strip_prefix("lifecycle: "))
        != Some("managed")
    {
        return false;
    }
    let Some(repo) = lines.iter().find_map(|line| line.strip_prefix("repo: ")) else {
        return false;
    };
    fs::canonicalize(repo).ok().as_deref() == fs::canonicalize(toplevel).ok().as_deref()
}

fn product_dir(cwd: &Path, repo: Option<&str>) -> PathBuf {
    if let Some(repo) = repo {
        let path = Path::new(repo);
        if fs::metadata(path)
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
        {
            return path.to_path_buf();
        }
    }
    cwd.to_path_buf()
}

fn review_body(product: &Path) -> String {
    match read_capped(&product.join("reviews").join("review.md"), RECORD_CAP) {
        Ok(Some(text)) => terminated(text),
        Ok(None) => String::new(),
        Err(ReadFail::TooLarge) => "review too large\n".to_string(),
        Err(ReadFail::Unreadable) => "review unreadable\n".to_string(),
    }
}

enum StateRead {
    Missing,
    Unreadable,
    Rows(Vec<StateRow>),
}

struct StateRow {
    item: String,
    status: String,
    stage: String,
    block: String,
}

fn read_state(program: &Path) -> StateRead {
    let text = match read_capped(&program.join("STATE.tsv"), RECORD_CAP) {
        Ok(None) => return StateRead::Missing,
        Ok(Some(text)) => text,
        Err(_) => return StateRead::Unreadable,
    };
    let mut rows = Vec::new();
    for (idx, rec) in crate::records(&text).into_iter().enumerate() {
        if rec.is_empty() || (idx == 0 && rec == STATE_HEADER) {
            continue;
        }
        let fields: Vec<&str> = rec.split('\t').collect();
        let item = fields.first().copied().unwrap_or("").trim();
        if item.is_empty() || item == "item" || item.contains([' ', '\t']) {
            continue;
        }
        rows.push(StateRow {
            item: item.to_string(),
            status: field(&fields, 1),
            stage: field(&fields, 2),
            block: field(&fields, 6),
        });
    }
    rows.sort_by(|a, b| a.item.cmp(&b.item));
    StateRead::Rows(rows)
}

fn state_lines(state: &StateRead, status: &str) -> String {
    let rows = match state {
        StateRead::Missing => return String::new(),
        StateRead::Unreadable => return "unreadable\n".to_string(),
        StateRead::Rows(rows) => rows,
    };
    let mut out = String::new();
    for row in rows.iter().filter(|row| row.status == status) {
        out.push_str(&row.item);
        let extra = if status == "BLOCKED" {
            row.block.as_str()
        } else {
            row.stage.as_str()
        };
        if !extra.is_empty() {
            out.push(' ');
            out.push_str(extra);
        }
        out.push('\n');
    }
    out
}

fn git_body(program: &Path, product: &Path) -> String {
    let mut out = String::new();
    if let Some(name) = branch_name(product) {
        out.push_str("branch ");
        out.push_str(&name);
        out.push('\n');
    }
    let mut lines = target_lines(program);
    lines.sort();
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn branch_name(product: &Path) -> Option<String> {
    let git = product.join(".git");
    if !is_real_dir(&git) {
        return None;
    }
    let text = read_capped(&git.join("HEAD"), 4096).ok()??;
    let line = text.lines().next()?.trim();
    let name = line.strip_prefix("ref: refs/heads/")?;
    if !safe_token(name) {
        return None;
    }
    Some(name.to_string())
}

fn target_lines(program: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    let Ok(entries) = fs::read_dir(program.join("items")) else {
        return lines;
    };
    for entry in entries.flatten() {
        let slug = entry.file_name();
        let Some(slug) = slug.to_str() else {
            continue;
        };
        if !safe_token(slug) {
            continue;
        }
        let path = entry.path();
        if !is_real_dir(&path) {
            continue;
        }
        let Ok(Some(text)) = read_capped(&path.join("TARGET"), RECORD_CAP) else {
            continue;
        };
        let branch = prefixed(&text, "branch: ");
        let base = prefixed(&text, "base: ");
        if safe_token(&branch) && safe_token(&base) {
            lines.push(format!("{slug} branch {branch} off {base}"));
        }
    }
    lines
}

fn intake_body(product: &Path) -> String {
    let mut out = String::new();
    match read_capped(&product.join("IDEA.md"), RECORD_CAP) {
        Ok(Some(text)) => {
            if let Some(line) = text.lines().map(str::trim).find(|line| !line.is_empty()) {
                let short: String = line.chars().take(IDEA_CAP).collect();
                out.push_str("idea: ");
                out.push_str(&short);
                out.push('\n');
            }
        }
        Ok(None) => {}
        Err(ReadFail::TooLarge) => out.push_str("idea too large\n"),
        Err(ReadFail::Unreadable) => out.push_str("idea unreadable\n"),
    }
    match read_capped(&product.join("BACKLOG.tsv"), RECORD_CAP) {
        Ok(Some(text)) => {
            let mut ids = Vec::new();
            for (idx, rec) in crate::records(&text).into_iter().enumerate() {
                if idx == 0 || rec.is_empty() {
                    continue;
                }
                if rec.starts_with('#') && !rec.contains('\t') {
                    continue;
                }
                let fields: Vec<&str> = rec.split('\t').collect();
                if field(&fields, 4) != "READY" {
                    continue;
                }
                let id = field(&fields, 0);
                if safe_token(&id) {
                    ids.push(id);
                }
            }
            ids.sort();
            for id in ids {
                out.push_str("ready: ");
                out.push_str(&id);
                out.push('\n');
            }
        }
        Ok(None) => {}
        Err(ReadFail::TooLarge) => out.push_str("backlog too large\n"),
        Err(ReadFail::Unreadable) => out.push_str("backlog unreadable\n"),
    }
    out
}

fn prefixed(text: &str, prefix: &str) -> String {
    crate::records(text)
        .into_iter()
        .find_map(|line| line.strip_prefix(prefix))
        .unwrap_or("")
        .trim()
        .to_string()
}

fn field(fields: &[&str], idx: usize) -> String {
    fields.get(idx).copied().unwrap_or("").trim().to_string()
}

fn terminated(text: String) -> String {
    if text.is_empty() || text.ends_with('\n') {
        text
    } else {
        format!("{text}\n")
    }
}

fn safe_token(value: &str) -> bool {
    !value.is_empty()
        && !value.contains([' ', '\t', '\\'])
        && !value.contains("..")
        && !value.starts_with('/')
}

enum ReadFail {
    TooLarge,
    Unreadable,
}

fn read_capped(path: &Path, cap: usize) -> Result<Option<String>, ReadFail> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => meta,
        Ok(_) => return Ok(None),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ReadFail::Unreadable),
    };
    if meta.len() > cap as u64 {
        return Err(ReadFail::TooLarge);
    }
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ReadFail::Unreadable),
    }
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.is_file())
        .unwrap_or(false)
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(line: u32) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("crucible-dashboard-{}-{line}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dashboard_shows_records_that_exist_and_not_the_queue() {
        let dir = scratch(line!());
        assert_eq!(
            dashboard(&dir),
            "agents\nrepo\nreviews\nnext\nblocked\ngit\nintake\n"
        );

        let attempts = dir.join("attempts").join("A1.2.3");
        fs::create_dir_all(&attempts).unwrap();
        fs::write(
            attempts.join("meta.tsv"),
            "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\nA1.2.3\titem\t-\tCLAIM\tmaker\tada\tkind\t-\tFOCUSED\tDISPATCHED\t1\t2\t-\n",
        )
        .unwrap();
        fs::write(
            attempts.join("events.tsv"),
            "state\tepoch\tpid\treason\nRUNNING\t9\t-\tfixture\n",
        )
        .unwrap();
        fs::write(dir.join("agents.tsv"), "roster-name\thost\t-\t-\t\n").unwrap();
        fs::write(
            dir.join("MESSAGES.tsv"),
            "1\tmachine\tsource\tsecret-order\n",
        )
        .unwrap();
        fs::write(dir.join("ORDERS.tsv"), "secret-order\n").unwrap();
        fs::write(
            dir.join("PROGRAM"),
            format!(
                "repo: {}\ncycle: guided\nlifecycle: managed\n",
                dir.display()
            ),
        )
        .unwrap();
        fs::write(
            dir.join("STATE.tsv"),
            format!(
                "{}\nalpha\tACTIVE\tBUILD\tw1\tLOW\t-\t-\t1\nbeta\tBLOCKED\tBUILD\tw2\tLOW\t-\tHOLD\t2\ngamma\tCLOSED\tBUILD\tw3\tLOW\t-\t-\t3\n",
                crate::state::STATE_HEADER
            ),
        )
        .unwrap();
        fs::create_dir_all(dir.join("reviews")).unwrap();
        fs::write(dir.join("reviews").join("review.md"), "lens one\n").unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(
            dir.join(".git").join("HEAD"),
            "ref: refs/heads/dash-proof\n",
        )
        .unwrap();
        let item = dir.join("items").join("alpha");
        fs::create_dir_all(&item).unwrap();
        fs::write(
            item.join("TARGET"),
            "repo: /does/not/open\nbranch: feature/s5\nbase: main\n",
        )
        .unwrap();
        fs::write(dir.join("IDEA.md"), "\na small idea\nsecond line\n").unwrap();
        fs::write(
            dir.join("BACKLOG.tsv"),
            "id\tsize\trisk\tidea_path\tstatus\ndoor\tS\tlow\t-\tREADY\ndone-row\tS\tlow\t-\tDONE\n",
        )
        .unwrap();

        let text = dashboard(&dir);
        assert_eq!(
            text,
            format!(
                "agents\nada in progress\nrepo\n{}\nreviews\nlens one\nnext\nalpha BUILD\nblocked\nbeta HOLD\ngit\nbranch dash-proof\nalpha branch feature/s5 off main\nintake\nidea: a small idea\nready: door\n",
                dir.display()
            )
        );
        assert!(!text.contains("secret-order"), "{text}");
        assert!(!text.contains("roster-name"), "{text}");
        assert!(!text.contains("done-row"), "{text}");
        assert!(!text.contains("gamma"), "{text}");
        assert!(!text.contains("/does/not/open"), "{text}");
        assert!(!text.contains("second line"), "{text}");
        assert_eq!(
            fs::read_to_string(dir.join("ORDERS.tsv")).unwrap(),
            "secret-order\n"
        );

        fs::remove_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".git"), "gitdir: /does/not/open\n").unwrap();
        let detached = dashboard(&dir);
        assert!(!detached.contains("branch dash-proof"), "{detached}");
        assert!(!detached.contains("/does/not/open"), "{detached}");
        assert!(
            detached.contains("alpha branch feature/s5 off main\n"),
            "{detached}"
        );

        fs::write(dir.join("reviews").join("review.md"), [0xff, 0xfe]).unwrap();
        let bad = dashboard(&dir);
        assert!(bad.contains("reviews\nreview unreadable\n"), "{bad}");
        assert!(bad.contains("ada in progress\n"), "{bad}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dashboard_uses_one_guided_program_beside_the_served_directory() {
        let dir = scratch(line!());
        let program = dir.join(".crucible").join("work");
        fs::create_dir_all(&program).unwrap();
        fs::write(
            program.join("PROGRAM"),
            format!(
                "repo: {}\nprogram: work\nlifecycle: managed\ncycle: guided\n",
                dir.display()
            ),
        )
        .unwrap();
        let attempts = program.join("attempts").join("A1.1.1");
        fs::create_dir_all(&attempts).unwrap();
        fs::write(
            attempts.join("meta.tsv"),
            "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\nA1.1.1\titem\t-\tCLAIM\tmaker\tada\tkind\t-\tFOCUSED\tDISPATCHED\t1\t2\t-\n",
        )
        .unwrap();
        fs::write(
            attempts.join("events.tsv"),
            "state\tepoch\tpid\treason\nDISPATCHED\t1\t-\tfixture\n",
        )
        .unwrap();
        fs::create_dir_all(dir.join("reviews")).unwrap();
        fs::write(dir.join("reviews").join("review.md"), "served review\n").unwrap();

        let text = dashboard(&dir);
        assert!(text.contains("ada new\n"), "{text}");
        assert!(text.contains("served review\n"), "{text}");
        assert!(
            text.contains(&format!("repo\n{}\n", dir.display())),
            "{text}"
        );

        let other = dir.join(".crucible").join("also");
        fs::create_dir_all(&other).unwrap();
        fs::write(
            other.join("PROGRAM"),
            format!(
                "repo: {}\nlifecycle: managed\ncycle: guided\n",
                dir.display()
            ),
        )
        .unwrap();
        let bea = other.join("attempts").join("A1.1.2");
        fs::create_dir_all(&bea).unwrap();
        fs::write(
            bea.join("meta.tsv"),
            "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\nA1.1.2\titem\t-\tCLAIM\tmaker\tbea\tkind\t-\tFOCUSED\tDISPATCHED\t1\t2\t-\n",
        )
        .unwrap();
        fs::write(
            bea.join("events.tsv"),
            "state\tepoch\tpid\treason\nDISPATCHED\t2\t-\tfixture\n",
        )
        .unwrap();
        let both = dashboard(&dir);
        assert!(!both.contains("ada "), "{both}");
        assert!(!both.contains("bea "), "{both}");
        assert!(both.starts_with("agents\nrepo\n"), "{both}");
        assert!(both.contains("served review\n"), "{both}");

        let _ = fs::remove_dir_all(&dir);
    }
}
