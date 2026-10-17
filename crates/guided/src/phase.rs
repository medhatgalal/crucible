//! `crucible phase`. Managed transitions read the attempt ledger; item-file mode rewrites PHASE.

use std::fs;
use std::path::Path;

use crucible_contract::Clock;

use crate::attempt::{maker_pass_exists, result_field, result_files};
use crate::cycle::{attempt_state, self_path, workid};
use crate::dispatch::{
    item_dir, need, task_assert_frozen, task_integration_complete, validate_task_dag,
};
use crate::program::uses_managed_lifecycle;
use crate::state::{state_update_item, state_validate_file, state_value};
use crate::{message, records, GuidedError};

/// `crucible phase`.
pub fn phase(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    let slug = args.first().copied().unwrap_or("");
    let dir = need(root, slug)?;
    let new = args.get(1).copied().unwrap_or("");
    if new.is_empty() {
        return Ok(format!("{}\n", phase_of(root, slug)?));
    }
    if uses_managed_lifecycle(root)? {
        return phase_managed(root, clock, slug, &dir, new);
    }
    phase_item_file(&dir, slug, new)
}

pub(crate) fn phase_of(root: &Path, slug: &str) -> Result<String, GuidedError> {
    if uses_managed_lifecycle(root)? {
        state_validate_file(&root.join("STATE.tsv"))?;
        return Ok(state_value(root, slug, 3)?.unwrap_or_default());
    }
    let path = item_dir(root, slug).join("ITEM.md");
    let Ok(text) = fs::read_to_string(path) else {
        return Ok("SPEC".to_string());
    };
    let phase = records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix("PHASE: "))
        .unwrap_or("");
    if phase.is_empty() {
        Ok("SPEC".to_string())
    } else {
        Ok(phase.to_string())
    }
}

fn phase_managed(
    root: &Path,
    clock: &dyn Clock,
    slug: &str,
    dir: &Path,
    new: &str,
) -> Result<String, GuidedError> {
    let old = phase_of(root, slug)?;
    if dir.join("TASKS.tsv").is_file() {
        validate_task_dag(root, slug)?;
        task_assert_frozen(root, slug)?;
    }
    let inflight = state_value(root, slug, 6)?.unwrap_or_default();
    if inflight.is_empty() {
        return Err(message(format!(
            "refused: {}/STATE.tsv has no row for {slug}, so nothing here can say whether work is in flight — inspect {}/STATE.tsv",
            root.display(),
            root.display()
        )));
    }
    if inflight != "-" {
        let ast = attempt_state(root, &inflight).unwrap_or_default();
        if old == "REVIEW" && new == "BUILD" && ast == "RETURNED" && judge_requested_fix(root, slug)
        {
            // A returned judge that asked for FIX may move back to BUILD.
        } else if ast.is_empty() {
            return Err(message(format!(
                "refused: {}/STATE.tsv names in-flight attempt {inflight} for {slug} and {}/attempts/{inflight} has no readable ledger — inspect {}/STATE.tsv",
                root.display(),
                root.display(),
                root.display()
            )));
        } else if ast == "DISPATCHED" {
            return Err(message(format!(
                "refused: attempt {inflight} is DISPATCHED and still in flight — it never started, so end it with: {} attempt finish {inflight} ABANDONED \"<what you observed>\"",
                self_path(root)
            )));
        } else {
            return Err(message(format!(
                "refused: attempt {inflight} is {ast} and still in flight"
            )));
        }
    }
    match (old.as_str(), new) {
        ("READY", "BUILD") | ("BUILD", "REVIEW") => {}
        ("REVIEW", "BUILD") => {
            if !judge_requested_fix(root, slug) {
                return Err(message(
                    "refused: REVIEW -> BUILD requires a judge result NEXT:FIX",
                ));
            }
        }
        _ => {
            return Err(message(format!(
                "refused: invalid transition {old} -> {new}"
            )));
        }
    }
    if old == "BUILD" && new == "REVIEW" {
        let current_wid = workid(root, slug)?;
        if dir.join("TASKS.tsv").is_file() {
            if !task_integration_complete(root, slug)? {
                return Err(message(
                    "refused: REVIEW requires current-work task integration",
                ));
            }
        } else if !maker_pass_exists(root, slug, &current_wid) {
            return Err(message(
                "refused: REVIEW requires a current-work maker PASS",
            ));
        }
        gate_before_review(root)?;
    }
    let status = state_value(root, slug, 2)?.unwrap_or_default();
    let risk = state_value(root, slug, 5)?.unwrap_or_default();
    let wid = workid(root, slug)?;
    state_update_item(root, clock, slug, &status, new, &wid, &risk, "-", "-")?;
    Ok(format!("{slug} is now in {new}\n"))
}

/// Folder boundaries live in `architecture/modules.md`. A missing map does not invent one.
/// An import that names another module root is illegal. `..` or an absolute path is banned slop.
fn gate_before_review(root: &Path) -> Result<(), GuidedError> {
    let repo = crate::orchestrate::review_directory(root);
    let map = repo.join("architecture").join("modules.md");
    let meta = match fs::symlink_metadata(&map) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    if meta.file_type().is_symlink() {
        return Err(message("refused: architecture/modules.md is a symlink"));
    }
    if !meta.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&map)?;
    let roots = module_roots(&text)?;
    if roots.is_empty() {
        return Err(message(
            "refused: architecture/modules.md has no module roots",
        ));
    }
    if let Some(hit) = first_import_violation(&repo, &roots)? {
        return Err(message(hit));
    }
    Ok(())
}

fn module_roots(text: &str) -> Result<Vec<String>, GuidedError> {
    let mut roots = Vec::new();
    let mut saw_header = false;
    for line in crate::records(text) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let first = fields.next().unwrap_or("");
        let second = fields.next().unwrap_or("");
        if !saw_header {
            saw_header = true;
            if first != "module_id" || second != "root_path" {
                return Err(message(
                    "refused: architecture/modules.md has no module roots",
                ));
            }
            continue;
        }
        if second.is_empty() {
            continue;
        }
        roots.push(normalize_root(second)?);
    }
    Ok(roots)
}

fn normalize_root(raw: &str) -> Result<String, GuidedError> {
    let raw = raw.trim();
    if raw.starts_with('/') || raw.contains('\\') {
        return Err(message(format!(
            "refused: banned slop in architecture/modules.md: {raw}"
        )));
    }
    let mut parts = Vec::new();
    for part in raw.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(message(format!(
                "refused: banned slop in architecture/modules.md: {raw}"
            )));
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(message(format!(
            "refused: banned slop in architecture/modules.md: {raw}"
        )));
    }
    Ok(format!("{}/", parts.join("/")))
}

fn first_import_violation(repo: &Path, roots: &[String]) -> Result<Option<String>, GuidedError> {
    for root in roots {
        let mut files = Vec::new();
        collect_sources(&repo.join(root), &mut files)?;
        for path in files {
            let rel = rel_slash(repo, &path);
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                if let Some(hit) = import_violation(line, &rel, roots) {
                    return Ok(Some(hit));
                }
            }
        }
    }
    Ok(None)
}

fn collect_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), GuidedError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let mut paths = Vec::new();
    for entry in entries {
        paths.push(entry?.path());
    }
    paths.sort();
    for path in paths {
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            collect_sources(&path, out)?;
        } else if meta.is_file() && is_source(&path) && meta.len() <= 262_144 {
            out.push(path);
        }
    }
    Ok(())
}

fn is_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("rs" | "py" | "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "go")
    )
}

fn rel_slash(repo: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(repo).unwrap_or(path);
    rel.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn import_violation(line: &str, file_rel: &str, roots: &[String]) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("//") || trimmed.starts_with('#') || !is_import(trimmed) {
        return None;
    }
    if trimmed.contains("..") || trimmed.contains("\"/") || trimmed.contains("'/") {
        return Some(format!("refused: banned slop in {file_rel}: {trimmed}"));
    }
    let own = roots
        .iter()
        .filter(|root| file_rel.starts_with(root.as_str()))
        .max_by_key(|root| root.len());
    for root in roots {
        if own.is_some_and(|own| own == root) {
            continue;
        }
        if trimmed.contains(root.as_str()) {
            return Some(format!("refused: illegal import in {file_rel}: {trimmed}"));
        }
    }
    None
}

fn is_import(trimmed: &str) -> bool {
    trimmed.starts_with("use ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("include ")
        || trimmed.starts_with("include!")
        || trimmed.contains("require(")
}

pub(crate) fn judge_requested_fix(root: &Path, slug: &str) -> bool {
    let mut latest: Option<String> = None;
    for path in result_files(root) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if result_field(&text, "ITEM") == slug && result_field(&text, "ROLE") == "judge" {
            latest = Some(text);
        }
    }
    latest.is_some_and(|text| result_field(&text, "NEXT") == "FIX")
}

fn phase_item_file(dir: &Path, slug: &str, new: &str) -> Result<String, GuidedError> {
    if !matches!(
        new,
        "SPEC" | "DESIGN" | "TASKS" | "BUILD" | "VERIFY" | "ADVERSARY" | "GRADUATE"
    ) {
        return Err(message(
            "phase must be one of SPEC DESIGN TASKS BUILD VERIFY ADVERSARY GRADUATE",
        ));
    }
    let req = match new {
        "TASKS" => Some("DESIGN.md"),
        "BUILD" => Some("TASKS.md"),
        "GRADUATE" => Some("ADVERSARY.md"),
        _ => None,
    };
    if let Some(req) = req {
        if !dir.join(req).is_file() {
            return Err(message(format!(
                "refused: {new} requires {req} and it does not exist"
            )));
        }
    }
    let item = dir.join("ITEM.md");
    let text = fs::read_to_string(&item)?;
    let has_phase = records(&text)
        .iter()
        .any(|line| line.starts_with("PHASE: "));
    let mut out = String::new();
    for line in records(&text) {
        if has_phase && line.starts_with("PHASE: ") {
            out.push_str(&format!("PHASE: {new}\n"));
        } else if !has_phase {
            if let Some(rest) = line.strip_prefix("STATUS: ") {
                out.push_str(&format!("PHASE: {new}\nSTATUS: {rest}\n"));
            } else {
                out.push_str(line);
                out.push('\n');
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    let tmp = dir.join(format!("ITEM.{}.tmp", std::process::id()));
    fs::write(&tmp, out)?;
    fs::rename(&tmp, &item)?;
    Ok(format!("{slug} is now in {new}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::STATE_HEADER;
    use crucible_contract::FixedClock;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Tmp(std::path::PathBuf);

    impl Tmp {
        fn new() -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "crucible-phase-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn managed_ready_moves_to_build_and_refuses_a_skip() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::create_dir_all(root.join("items/alpha/work")).unwrap();
        fs::write(root.join("items/alpha/ITEM.md"), "# alpha\n").unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tREADY\tEMPTY\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        let clock = FixedClock::new(1_700_000_070);
        assert_eq!(phase(root, &["alpha"], &clock).unwrap(), "READY\n");
        assert_eq!(
            phase(root, &["alpha", "REVIEW"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: invalid transition READY -> REVIEW"
        );
        assert_eq!(
            phase(root, &["alpha", "BUILD"], &clock).unwrap(),
            "alpha is now in BUILD\n"
        );
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(state.contains("alpha\tACTIVE\tBUILD\t"));
        assert!(state.contains("\t-\t-\t1700000070\n"));
    }

    #[test]
    fn item_file_phase_inserts_before_status() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let item = root.join("items/alpha");
        fs::create_dir_all(&item).unwrap();
        fs::write(item.join("ITEM.md"), "# alpha\nSTATUS: OPEN\n").unwrap();
        fs::write(item.join("DESIGN.md"), "design\n").unwrap();
        let clock = FixedClock::new(1);
        assert_eq!(phase(root, &["alpha"], &clock).unwrap(), "SPEC\n");
        assert_eq!(
            phase(root, &["alpha", "TASKS"], &clock).unwrap(),
            "alpha is now in TASKS\n"
        );
        assert_eq!(
            fs::read_to_string(item.join("ITEM.md")).unwrap(),
            "# alpha\nPHASE: TASKS\nSTATUS: OPEN\n"
        );
        assert_eq!(
            phase(root, &["alpha", "NOPE"], &clock)
                .unwrap_err()
                .to_string(),
            "phase must be one of SPEC DESIGN TASKS BUILD VERIFY ADVERSARY GRADUATE"
        );
    }

    fn ready_for_review(root: &Path) {
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::create_dir_all(root.join("items/alpha/work")).unwrap();
        fs::write(root.join("items/alpha/ITEM.md"), "# alpha\n").unwrap();
        fs::write(root.join("items/alpha/work/note"), "work\n").unwrap();
        let wid = workid(root, "alpha").unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tBUILD\t{wid}\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        fs::create_dir_all(root.join("attempts/a1")).unwrap();
        fs::write(
            root.join("attempts/a1/result.md"),
            format!("OUTCOME: PASS\nITEM: alpha\nWORK-ID: {wid}\nROLE: maker\n"),
        )
        .unwrap();
    }

    fn write_modules(root: &Path) {
        fs::create_dir_all(root.join("architecture")).unwrap();
        fs::write(
            root.join("architecture/modules.md"),
            "module_id\troot_path\tpublic_contracts\ttest_entrypoint\tpattern_instance\tlive_write\n\
alpha\tsrc/alpha/\t-\t-\tsrc/alpha/a.py\tno\n\
beta\tsrc/beta/\t-\t-\tsrc/beta/b.py\tno\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("src/alpha")).unwrap();
        fs::create_dir_all(root.join("src/beta")).unwrap();
        fs::write(root.join("src/beta/b.py"), "VALUE = 1\n").unwrap();
    }

    #[test]
    fn review_refuses_an_illegal_import() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        ready_for_review(root);
        write_modules(root);
        fs::write(root.join("src/alpha/a.py"), "import src/beta/b.py\n").unwrap();
        let before = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        let clock = FixedClock::new(1_700_000_080);
        assert_eq!(
            phase(root, &["alpha", "REVIEW"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: illegal import in src/alpha/a.py: import src/beta/b.py"
        );
        assert_eq!(fs::read_to_string(root.join("STATE.tsv")).unwrap(), before);
    }

    #[test]
    fn review_refuses_banned_slop() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        ready_for_review(root);
        write_modules(root);
        fs::write(root.join("src/alpha/a.py"), "import ../escape.py\n").unwrap();
        let before = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        let clock = FixedClock::new(1_700_000_081);
        assert_eq!(
            phase(root, &["alpha", "REVIEW"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: banned slop in src/alpha/a.py: import ../escape.py"
        );
        assert_eq!(fs::read_to_string(root.join("STATE.tsv")).unwrap(), before);
    }

    #[test]
    fn review_enters_when_imports_stay_inside_the_module() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        ready_for_review(root);
        write_modules(root);
        fs::write(root.join("src/alpha/a.py"), "import os\n").unwrap();
        let clock = FixedClock::new(1_700_000_082);
        assert_eq!(
            phase(root, &["alpha", "REVIEW"], &clock).unwrap(),
            "alpha is now in REVIEW\n"
        );
        let state = fs::read_to_string(root.join("STATE.tsv")).unwrap();
        assert!(state.contains("alpha\tACTIVE\tREVIEW\t"));
    }
}
