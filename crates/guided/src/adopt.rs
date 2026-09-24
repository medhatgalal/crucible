//! Port of `adopt_install_engine` through `cmd_adopt`. Refusals are the shell `die` strings.
//! Stdout is the shell's printf text. This does not write FLOOR and does not bump VERSION.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use crate::state::{state_render_file, STATE_HEADER};
use crate::{message, records, GuidedError};

const SKILL_VIEWS: &[&str] = &[
    ".crucible/.grok/skills",
    ".crucible/.claude/skills",
    ".crucible/.agents/skills",
    ".crucible/.kiro/skills",
    ".grok/skills",
    ".claude/skills",
    ".agents/skills",
    ".kiro/skills",
];

const GITIGNORE_PATTERNS: &[&str] = &["*/agents.tsv", "*/PANEL.AGENTS.tsv", "*/worktrees/"];

const USAGE: &str = "usage: crucible adopt [PROGRAM] [--managed] [--refresh] [--working-mode] [--overwrite-batteries] [--panel-from PROGRAM]";

const LESSONS: &str = "- Conversational \"keep looping\" is not a waiver to implement. Drive ticks one legal orchestrator action; makers stay inside EXECUTE for the admitted item.\n";

const BACKLOG: &str = "- [ ] example — replace this line with real items as claims are admitted\n";

const PROBLEM: &str = "# Problem\n\nTEMPLATE-PROBLEM-NEEDS-INPUT\n";

const AGENTS_TEMPLATE: &str = "\
# name\tkind\tmodel\teffort\tcommand   ({BRIEF} {MODEL} {EFFORT} substituted)\n\
# Only names here may author a verdict. kind is the model family and is what\n\
# cross-model judging counts. Replace these with how YOUR agents are invoked.\n\
mk1\tkindA\tMODEL\thigh\tAGENT_CLI --model {MODEL} --effort {EFFORT} \"read {BRIEF} and follow it exactly\"\n\
j1\tkindA\tMODEL\thigh\tAGENT_CLI --model {MODEL} --effort {EFFORT} \"read {BRIEF} and follow it exactly\"\n\
j2\tkindB\tMODEL\thigh\tOTHER_CLI --model {MODEL} \"read {BRIEF} and follow it exactly\"\n";

struct AdoptArgs {
    program: String,
    managed: bool,
    refresh: bool,
    working_mode: bool,
    overwrite_batteries: bool,
    panel_from: Option<String>,
}

pub fn cmd_adopt(
    cwd: &Path,
    src: &Path,
    args: &[&str],
    stdout: &mut dyn Write,
) -> Result<(), GuidedError> {
    let opts = parse_adopt_args(args)?;
    let repo = repo_toplevel(cwd)?;
    let dst = repo.join(".crucible").join(&opts.program);
    if opts.refresh {
        refresh(src, &repo, &dst, &opts, stdout)
    } else {
        install_new(src, &repo, &dst, &opts, stdout)
    }
}

pub fn adopt_install_engine(src: &Path, dst: &Path) -> Result<(), GuidedError> {
    fs::create_dir_all(dst.join("items"))?;
    fs::create_dir_all(dst.join("scripts"))?;
    fs::create_dir_all(dst.join("docs"))?;
    fs::create_dir_all(dst.join("roles"))?;
    for name in [
        "BOOTSTRAP.md",
        "START.md",
        "RULES.md",
        "LOOP.md",
        "CONFIGURE.md",
        "VERSION",
    ] {
        let from = src.join(name);
        if is_file(&from) {
            copy_file(&from, &dst.join(name))?;
        }
    }
    // Still the shell entry. A later tag installs the binary here; this copy does not.
    if is_file(&src.join("crucible-guided")) {
        copy_file(&src.join("crucible-guided"), &dst.join("crucible"))?;
        copy_file(&src.join("crucible-guided"), &dst.join("crucible-guided"))?;
    } else if is_file(&src.join("crucible")) {
        copy_file(&src.join("crucible"), &dst.join("crucible"))?;
    }
    if is_dir(&src.join("roles")) {
        copy_matching(&src.join("roles"), &dst.join("roles"), ".md", &[])?;
    }
    copy_matching(
        &src.join("scripts"),
        &dst.join("scripts"),
        ".sh",
        &["package-release.sh", "verify-package.sh"],
    )?;
    if is_dir(&src.join("docs")) {
        copy_matching(&src.join("docs"), &dst.join("docs"), ".md", &[])?;
    }
    chmod_x(&dst.join("crucible"));
    chmod_x(&dst.join("crucible-guided"));
    chmod_scripts(&dst.join("scripts"));
    Ok(())
}

pub fn adopt_src_sha256(src: &Path) -> Result<String, GuidedError> {
    let mut hasher = Sha256::new();
    feed_file(&mut hasher, &src.join("VERSION"))?;
    feed_file(&mut hasher, &src.join("wm.sh"))?;
    let crucible = src.join("crucible");
    if is_file(&crucible) && !adopt_is_script(&crucible) {
        feed_file(&mut hasher, &crucible)?;
    } else {
        let release = src.join("target/release/crucible");
        if is_executable(&release) {
            feed_file(&mut hasher, &release)?;
        }
    }
    feed_file(&mut hasher, &src.join("ROUTING.tsv"))?;
    let skills = src.join("skills");
    if is_dir(&skills) {
        let mut files = Vec::new();
        collect_regular_files(&skills, &mut files)?;
        files.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
        for path in files {
            feed_file(&mut hasher, &path)?;
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn adopt_write_engine_source(src: &Path, dst: &Path) -> Result<(), GuidedError> {
    let ver = version_label(src);
    let hash = adopt_src_sha256(src)?;
    fs::write(
        dst.join("ENGINE-SOURCE"),
        format!("version: {ver}\nsha256: {hash}\n"),
    )?;
    Ok(())
}

pub fn adopt_check_required_batteries(routing: &Path, skills: &Path) -> Result<(), GuidedError> {
    if !is_file(routing) {
        return Err(message("refused: ROUTING.tsv missing"));
    }
    let text = fs::read_to_string(routing)?;
    let mut missing = String::new();
    for line in records(&text) {
        let fields: Vec<&str> = line.split('\t').collect();
        let phase = field(&fields, 0);
        if phase.is_empty() || phase == "phase" || phase.starts_with('#') {
            continue;
        }
        if field(&fields, 5) != "yes" {
            continue;
        }
        let bat = field(&fields, 2);
        if bat.is_empty() || bat == "-" {
            continue;
        }
        if !is_file(&skills.join(bat).join("SKILL.md")) {
            missing.push(' ');
            missing.push_str(bat);
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(message(format!(
            "refused: required battery missing:{missing}"
        )))
    }
}

pub fn adopt_battery_kept(repo: &Path, name: &str) -> bool {
    let skill = repo.join(".crucible/skills").join(name);
    if exists(&skill.join(".keep")) || exists(&skill.join("KEEP")) {
        return true;
    }
    for rel in [".crucible/skills/KEEP", ".crucible/KEEP"] {
        if keep_file_lists(&repo.join(rel), name) {
            return true;
        }
    }
    false
}

pub fn adopt_refresh_skill_views(repo: &Path, name: &str) -> Result<(), GuidedError> {
    let src = repo.join(".crucible/skills").join(name);
    // A symlink is not a canonical tree. Views are real directory copies (`cp -R`).
    if !is_real_dir(&src) {
        return Ok(());
    }
    for rel in SKILL_VIEWS {
        let dest = repo.join(rel).join(name);
        if !shell_child_of(&dest, repo) {
            return Err(message(format!(
                "refused: view path outside repo: {}",
                dest.display()
            )));
        }
        rm_rf(&dest)?;
        fs::create_dir_all(&dest)?;
        copy_children(&src, &dest).map_err(|_| {
            message(format!(
                "refused: could not copy skill view {}",
                dest.display()
            ))
        })?;
    }
    Ok(())
}

pub fn adopt_restore_kept_batteries(keep: &Path, repo: &Path) -> Result<(), GuidedError> {
    if keep.as_os_str().is_empty() || !is_dir(keep) {
        return Ok(());
    }
    if repo.as_os_str().is_empty() || repo == Path::new("/") {
        let _ = rm_rf(keep);
        return Err(message("refused: bad repository root"));
    }
    let skills_root = repo.join(".crucible/skills");
    let mut kept_dirs = Vec::new();
    for ent in fs::read_dir(keep)? {
        let ent = ent?;
        let path = ent.path();
        if !is_dir(&path) {
            continue;
        }
        kept_dirs.push((ent.file_name(), path));
    }
    kept_dirs.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, kept) in kept_dirs {
        let dest = skills_root.join(&name);
        if !shell_child_of(&dest, &skills_root) {
            let _ = rm_rf(keep);
            return Err(message(format!(
                "refused: KEEP restore path outside skills: {}",
                dest.display()
            )));
        }
        rm_rf(&dest)?;
        fs::create_dir_all(&dest)?;
        let name_str = name.to_string_lossy();
        if copy_children(&kept, &dest).is_err() {
            return Err(message(format!(
                "refused: KEEP restore failed for {name_str}"
            )));
        }
        adopt_refresh_skill_views(repo, name_str.as_ref())?;
    }
    rm_rf(keep)?;
    Ok(())
}

pub fn adopt_is_script(path: &Path) -> bool {
    if !is_file(path) {
        return false;
    }
    let mut buf = [0u8; 2];
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    match file.read(&mut buf) {
        Ok(2) => buf == *b"#!",
        _ => false,
    }
}

pub fn adopt_find_rust_bin(src: &Path) -> Result<PathBuf, GuidedError> {
    if let Some(value) = nonempty_var("CRUCIBLE_RUST_BIN") {
        let path = PathBuf::from(&value);
        if is_executable(&path) {
            return Ok(path);
        }
    }
    let local = src.join("crucible");
    if is_file(&local) && is_executable(&local) && !adopt_is_script(&local) {
        return Ok(local);
    }
    let release = src.join("target/release/crucible");
    if is_executable(&release) {
        return Ok(release);
    }
    if is_file(&src.join("Cargo.toml")) && cargo_release(src) && is_executable(&release) {
        return Ok(release);
    }
    Err(message(
        "refused: working-mode source is missing the crucible binary (cargo build --release)",
    ))
}

pub fn adopt_install_working_mode(
    src: &Path,
    dst: &Path,
    repo: &Path,
    overwrite_batteries: bool,
) -> Result<(), GuidedError> {
    if repo.as_os_str().is_empty() || repo == Path::new("/") {
        return Err(message("refused: bad repository root"));
    }
    if !is_file(&src.join("wm.sh")) {
        return Err(message("refused: working-mode source is missing wm.sh"));
    }
    if !is_dir(&src.join("skills")) {
        return Err(message("refused: working-mode source is missing skills/"));
    }
    if !is_file(&src.join("scripts/project-skills.sh")) {
        return Err(message(
            "refused: working-mode source is missing scripts/project-skills.sh",
        ));
    }
    if !is_file(&src.join("ROUTING.tsv")) {
        return Err(message("refused: ROUTING.tsv missing"));
    }
    adopt_check_required_batteries(&src.join("ROUTING.tsv"), &src.join("skills"))?;

    let bin = adopt_find_rust_bin(src)?;
    copy_file(&bin, &dst.join("crucible"))?;
    chmod_x(&dst.join("crucible"));
    if is_file(&src.join("crucible-guided")) {
        copy_file(&src.join("crucible-guided"), &dst.join("crucible-guided"))?;
        chmod_x(&dst.join("crucible-guided"));
    } else if is_file(&src.join("crucible")) && adopt_is_script(&src.join("crucible")) {
        copy_file(&src.join("crucible"), &dst.join("crucible-guided"))?;
        chmod_x(&dst.join("crucible-guided"));
    }
    copy_file(&src.join("wm.sh"), &dst.join("wm.sh"))?;
    chmod_x(&dst.join("wm.sh"));
    if is_file(&src.join("WORKING-MODE.md")) {
        copy_file(&src.join("WORKING-MODE.md"), &dst.join("WORKING-MODE.md"))?;
    }
    copy_file(&src.join("ROUTING.tsv"), &dst.join("ROUTING.tsv"))?;
    fs::create_dir_all(repo.join(".crucible"))?;
    copy_file(
        &src.join("ROUTING.tsv"),
        &repo.join(".crucible/ROUTING.tsv"),
    )?;
    if is_dir(&src.join("adapters")) {
        let adapters = dst.join("adapters");
        fs::create_dir_all(&adapters)?;
        copy_matching(&src.join("adapters"), &adapters, "", &[])?;
    }

    let mut keep = PathBuf::new();
    if !overwrite_batteries && is_dir(&repo.join(".crucible/skills")) {
        keep = dst.join(format!(".keep-snapshot.{}", std::process::id()));
        fs::create_dir_all(&keep)?;
        let mut names = dir_names(&repo.join(".crucible/skills"))?;
        names.sort();
        for name in names {
            let name_s = name.to_string_lossy();
            if name_s.is_empty() || name_s.starts_with('.') || name_s.contains('/') {
                continue;
            }
            let canon = repo.join(".crucible/skills").join(&name);
            if !is_dir(&canon) {
                continue;
            }
            if adopt_battery_kept(repo, name_s.as_ref()) {
                copy_dir(&canon, &keep.join(&name))?;
            }
        }
    }

    let proj_rc = run_project_skills(src, repo);
    // The projector can wipe canonical dirs and then exit non-zero. Restore first.
    adopt_restore_kept_batteries(&keep, repo)?;
    if proj_rc != 0 {
        return Err(message("refused: skill projection failed"));
    }
    adopt_check_required_batteries(&src.join("ROUTING.tsv"), &repo.join(".crucible/skills"))?;
    adopt_write_engine_source(src, dst)?;
    Ok(())
}

pub fn adopt_working_mode_installed(dst: &Path) -> bool {
    if is_file(&dst.join("wm.sh")) {
        return true;
    }
    let program = dst.join("PROGRAM");
    if !is_file(&program) {
        return false;
    }
    match fs::read_to_string(&program) {
        Ok(text) => records(&text).contains(&"working-mode: yes"),
        Err(_) => false,
    }
}

pub fn adopt_gitignore_reason(pattern: &str) -> String {
    match pattern {
        "*/agents.tsv" => {
            "agents.tsv names the agents on THIS machine, so it is not project state.".to_string()
        }
        "*/PANEL.AGENTS.tsv" => {
            "PANEL.AGENTS.tsv is the approve-panel copy of agents.tsv, so it carries the same machine-local invocations.".to_string()
        }
        "*/worktrees/" => {
            "isolated task worktrees are retained machine state, not program artifacts.".to_string()
        }
        _ => String::new(),
    }
}

pub fn adopt_sync_gitignore(repo: &Path, stdout: &mut dyn Write) -> Result<(), GuidedError> {
    let ignore = repo.join(".crucible/.gitignore");
    if !exists(&ignore) {
        fs::write(
            &ignore,
            "# Patterns here are relative to this .crucible/ directory.\n",
        )?;
    }
    for pattern in GITIGNORE_PATTERNS {
        let text = fs::read_to_string(&ignore)?;
        if records(&text).contains(pattern) {
            continue;
        }
        let mut file = OpenOptions::new().append(true).open(&ignore)?;
        writeln!(file, "# {}", adopt_gitignore_reason(pattern))?;
        writeln!(file, "{pattern}")?;
    }
    let tracked = git_stdout(
        repo,
        &[
            "ls-files",
            "--",
            ".crucible/*/agents.tsv",
            ".crucible/*/PANEL.AGENTS.tsv",
        ],
    );
    for stale in records(&tracked) {
        if stale.is_empty() {
            continue;
        }
        writeln!(
            stdout,
            "still tracked (machine-local): {stale} — untrack with: git rm --cached {stale}"
        )?;
    }
    Ok(())
}

pub fn adopt_copy_panel_from(srcprog: &str, dstdir: &Path, repo: &Path) -> Result<(), GuidedError> {
    let srcdir = panel_source(repo, srcprog)?;
    for name in [
        "agents.tsv",
        "PANEL.md",
        "PANEL.ASSIGN.tsv",
        "PANEL.APPROVAL",
    ] {
        copy_file(&srcdir.join(name), &dstdir.join(name))?;
    }
    if is_file(&srcdir.join("PANEL.AGENTS.tsv")) {
        copy_file(
            &srcdir.join("PANEL.AGENTS.tsv"),
            &dstdir.join("PANEL.AGENTS.tsv"),
        )?;
    }
    let approvals = srcdir.join("panel-approvals");
    if is_dir(&approvals) {
        let dest = dstdir.join("panel-approvals");
        fs::create_dir_all(&dest)?;
        copy_matching(&approvals, &dest, ".md", &[])?;
    }
    Ok(())
}

fn refresh(
    src: &Path,
    repo: &Path,
    dst: &Path,
    opts: &AdoptArgs,
    stdout: &mut dyn Write,
) -> Result<(), GuidedError> {
    if is_dir(dst) && !is_file(&dst.join("PROGRAM")) {
        return Err(message(format!(
            "refused: {} is a husk (no PROGRAM). Keep it, trash it, or adopt a different program name",
            dst.display()
        )));
    }
    if !(is_dir(dst) && is_file(&dst.join("PROGRAM"))) {
        return Err(message(format!(
            "program does not exist: {}",
            dst.display()
        )));
    }
    let src_abs = fs::canonicalize(src)?;
    let dst_abs = fs::canonicalize(dst)?;
    if src_abs == dst_abs {
        return Err(message(
            "refused: refresh source is the destination (src == dst); use a versioned tarball",
        ));
    }
    let old = version_label(dst);
    let new = version_label(src);
    adopt_install_engine(src, dst)?;
    if opts.working_mode || adopt_working_mode_installed(dst) {
        adopt_install_working_mode(src, dst, repo, opts.overwrite_batteries)?;
        append_working_mode_line(dst)?;
    }
    adopt_sync_gitignore(repo, stdout)?;
    writeln!(stdout, "refreshed engine {old} -> {new}")?;
    writeln!(stdout, "{}", dst.display())?;
    if is_file(&dst.join("scripts/acp-brief.py")) {
        writeln!(stdout, "kept local adapter: scripts/acp-brief.py")?;
    }
    Ok(())
}

fn install_new(
    src: &Path,
    repo: &Path,
    dst: &Path,
    opts: &AdoptArgs,
    stdout: &mut dyn Write,
) -> Result<(), GuidedError> {
    if let Some(panel) = &opts.panel_from {
        if panel == &opts.program {
            return Err(message(
                "refused: --panel-from cannot be the new program name",
            ));
        }
        panel_source(repo, panel)?;
    }
    if is_dir(dst) {
        return Err(message(format!(
            "program already exists: {}",
            dst.display()
        )));
    }
    if opts.working_mode {
        if !is_file(&src.join("wm.sh")) {
            return Err(message("refused: working-mode source is missing wm.sh"));
        }
        if !is_dir(&src.join("skills")) {
            return Err(message("refused: working-mode source is missing skills/"));
        }
        if !is_file(&src.join("ROUTING.tsv")) {
            return Err(message("refused: ROUTING.tsv missing"));
        }
        adopt_check_required_batteries(&src.join("ROUTING.tsv"), &src.join("skills"))?;
    }
    fs::create_dir_all(dst)?;
    adopt_install_engine(src, dst)?;
    let base = current_branch(repo);
    let prog = &opts.program;
    let repo_s = repo.display();
    let mut program = format!("repo: {repo_s}\nprogram: {prog}\nbase: {base}\n");
    if opts.managed {
        program.push_str("lifecycle: managed\ncycle: guided\n");
    }
    if opts.working_mode {
        program.push_str("working-mode: yes\n");
    }
    fs::write(dst.join("PROGRAM"), program)?;
    fs::write(dst.join("LESSONS.md"), LESSONS)?;
    fs::write(dst.join("BACKLOG.md"), BACKLOG)?;
    if opts.managed {
        fs::write(dst.join("STATE.tsv"), format!("{STATE_HEADER}\n"))?;
        state_render_file(dst, &dst.join("STATE.md"), None)?;
    } else {
        fs::write(
            dst.join("STATE.md"),
            item_file_state(prog, &repo_s.to_string()),
        )?;
    }
    fs::write(dst.join("CLAIMS.md"), claims_md(prog))?;
    fs::write(dst.join("PROBLEM.md"), PROBLEM)?;
    if opts.working_mode {
        adopt_install_working_mode(src, dst, repo, true)?;
    }
    adopt_sync_gitignore(repo, stdout)?;
    writeln!(stdout, "installed cycle \"{prog}\" into {}", dst.display())?;
    writeln!(stdout)?;
    writeln!(stdout, "Fresh-agent entrypoint:")?;
    writeln!(stdout, "  read .crucible/{prog}/START.md and execute it")?;
    writeln!(stdout)?;
    writeln!(
        stdout,
        "The agent will configure its panel, investigate the problem, ask for proposal approval,"
    )?;
    writeln!(
        stdout,
        "and coordinate build/review iterations. The operator does not drive protocol commands."
    )?;
    if let Some(panel) = &opts.panel_from {
        if panel == prog {
            return Err(message(
                "refused: --panel-from cannot be the new program name",
            ));
        }
        adopt_copy_panel_from(panel, dst, repo)?;
        writeln!(
            stdout,
            "copied approved panel from .crucible/{panel} (leftover PROBLEM stays on {panel})"
        )?;
    } else if !is_file(&dst.join("agents.tsv")) {
        fs::write(dst.join("agents.tsv"), AGENTS_TEMPLATE)?;
    }
    Ok(())
}

fn parse_adopt_args(args: &[&str]) -> Result<AdoptArgs, GuidedError> {
    let mut program = None;
    let mut managed = false;
    let mut refresh = false;
    let mut working_mode = false;
    let mut overwrite_batteries = false;
    let mut panel_from = None;
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--managed" => managed = true,
            "--refresh" => refresh = true,
            "--working-mode" => working_mode = true,
            "--overwrite-batteries" => overwrite_batteries = true,
            "--panel-from" => {
                i += 1;
                if i >= args.len() || args[i].is_empty() {
                    return Err(message(
                        "usage: crucible adopt [PROGRAM] --managed --panel-from PROGRAM",
                    ));
                }
                panel_from = Some(args[i].to_string());
            }
            other if other.starts_with('-') => return Err(message(USAGE)),
            other => {
                if program.is_some() {
                    return Err(message(USAGE));
                }
                program = Some(other.to_string());
            }
        }
        i += 1;
    }
    let program = match program {
        Some(name) if !name.is_empty() => name,
        _ => "main".to_string(),
    };
    if refresh && managed {
        return Err(message(
            "usage: crucible adopt [PROGRAM] --refresh  (--managed is install-only)",
        ));
    }
    if panel_from.is_some() && refresh {
        return Err(message(
            "usage: crucible adopt [PROGRAM] --managed --panel-from PROGRAM  (--refresh does not copy a panel)",
        ));
    }
    if panel_from.is_some() && !managed {
        return Err(message(
            "usage: crucible adopt [PROGRAM] --managed --panel-from PROGRAM",
        ));
    }
    if working_mode && !managed && !refresh {
        return Err(message(
            "usage: crucible adopt [PROGRAM] --managed --working-mode",
        ));
    }
    if program
        .bytes()
        .any(|b| !b.is_ascii_alphanumeric() && !matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(message("program name may only contain A-Za-z0-9._-"));
    }
    Ok(AdoptArgs {
        program,
        managed,
        refresh,
        working_mode,
        overwrite_batteries,
        panel_from,
    })
}

fn panel_source(repo: &Path, srcprog: &str) -> Result<PathBuf, GuidedError> {
    let srcdir = repo.join(".crucible").join(srcprog);
    if !(is_dir(&srcdir) && is_file(&srcdir.join("PROGRAM"))) {
        return Err(message(format!(
            "refused: --panel-from program does not exist: {}",
            srcdir.display()
        )));
    }
    if !(is_nonempty_file(&srcdir.join("PANEL.md"))
        && is_nonempty_file(&srcdir.join("PANEL.ASSIGN.tsv"))
        && is_nonempty_file(&srcdir.join("agents.tsv")))
    {
        return Err(message(format!(
            "refused: --panel-from {srcprog} has no panel to copy"
        )));
    }
    if !is_file(&srcdir.join("PANEL.APPROVAL")) {
        return Err(message(format!(
            "refused: --panel-from {srcprog} panel is not approved"
        )));
    }
    Ok(srcdir)
}

fn append_working_mode_line(dst: &Path) -> Result<(), GuidedError> {
    let path = dst.join("PROGRAM");
    if !is_file(&path) {
        return Ok(());
    }
    let text = fs::read_to_string(&path)?;
    if records(&text).contains(&"working-mode: yes") {
        return Ok(());
    }
    let mut file = OpenOptions::new().append(true).open(&path)?;
    writeln!(file, "working-mode: yes")?;
    Ok(())
}

fn version_label(dir: &Path) -> String {
    let path = dir.join("VERSION");
    let Ok(meta) = fs::metadata(&path) else {
        return "unknown".to_string();
    };
    if !meta.is_file() || meta.len() == 0 {
        return "unknown".to_string();
    }
    let Ok(bytes) = fs::read(&path) else {
        return "unknown".to_string();
    };
    let end = bytes
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn claims_md(prog: &str) -> String {
    format!(
        "# CLAIMS — {prog}\n\nOne heading per finding from the problem document, quoting its source sentence\nverbatim. Nothing becomes an item until it survives audit.\n"
    )
}

fn item_file_state(prog: &str, repo: &str) -> String {
    format!(
        "\
# STATE — {prog}

The resume point. Trust this over your memory, always, and especially after a restart or a
context compaction. The orchestrator writes it after every transition; nobody else writes it.

program: {prog}
repo: {repo}
phase: INTAKE

## In flight

item: (none)
dispatch: (none — the path of the contract file)
awaiting: (none — the output file that must appear)

## Escalations awaiting the human

(none)

---

Recover with: cat STATE.md; .crucible/{prog}/crucible next; git log --oneline -10
Never re-dispatch a phase this file marks complete.
"
    )
}

fn keep_file_lists(path: &Path, name: &str) -> bool {
    if !is_file(path) {
        return false;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    for line in records(&text) {
        if line.trim_start_matches([' ', '\t']).starts_with('#') {
            continue;
        }
        if line.split_whitespace().next() == Some(name) {
            return true;
        }
    }
    false
}

fn run_project_skills(src: &Path, repo: &Path) -> i32 {
    let script = src.join("scripts/project-skills.sh");
    match Command::new(&script)
        .arg(src.join("skills"))
        .arg(repo)
        .stdin(Stdio::null())
        .status()
    {
        Ok(status) => status.code().unwrap_or(1),
        Err(_) => 1,
    }
}

fn cargo_release(src: &Path) -> bool {
    cargo_build(src, true) || cargo_build(src, false)
}

fn cargo_build(src: &Path, locked: bool) -> bool {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--release");
    if locked {
        cmd.arg("--locked");
    }
    let Ok(out) = cmd
        .current_dir(src)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    else {
        return false;
    };
    let mut err = io::stderr().lock();
    let _ = err.write_all(&out.stdout);
    let _ = err.write_all(&out.stderr);
    out.status.success()
}

fn feed_file(hasher: &mut Sha256, path: &Path) -> Result<(), GuidedError> {
    if !is_file(path) {
        return Ok(());
    }
    let mut file = File::open(path)?;
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(())
}

fn collect_regular_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), GuidedError> {
    for ent in fs::read_dir(dir)? {
        let ent = ent?;
        let path = ent.path();
        let ft = ent.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            collect_regular_files(&path, out)?;
        } else if ft.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn copy_matching(
    src_dir: &Path,
    dst_dir: &Path,
    ext: &str,
    skip: &[&str],
) -> Result<(), GuidedError> {
    let rd = match fs::read_dir(src_dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for ent in rd {
        let ent = ent?;
        let name = ent.file_name();
        let name_s = name.to_string_lossy();
        if name_s.starts_with('.') {
            continue;
        }
        if !ext.is_empty() && !name_s.ends_with(ext) {
            continue;
        }
        if skip.iter().any(|s| *s == name_s.as_ref()) {
            continue;
        }
        let path = ent.path();
        if is_file(&path) {
            copy_file(&path, &dst_dir.join(name))?;
        }
    }
    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> Result<(), GuidedError> {
    fs::copy(src, dst)?;
    Ok(())
}

fn copy_dir(src: &Path, dest: &Path) -> Result<(), GuidedError> {
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let target = fs::read_link(src)?;
        std::os::unix::fs::symlink(target, dest)?;
        return Ok(());
    }
    fs::create_dir_all(dest)?;
    copy_children(src, dest)?;
    Ok(())
}

fn copy_children(from: &Path, dest: &Path) -> io::Result<()> {
    for ent in fs::read_dir(from)? {
        let ent = ent?;
        let to = dest.join(ent.file_name());
        let ft = ent.file_type()?;
        if ft.is_symlink() {
            if fs::symlink_metadata(&to).is_ok() {
                rm_rf(&to)?;
            }
            let target = fs::read_link(ent.path())?;
            std::os::unix::fs::symlink(target, &to)?;
        } else if ft.is_dir() {
            fs::create_dir_all(&to)?;
            copy_children(&ent.path(), &to)?;
        } else if ft.is_file() {
            fs::copy(ent.path(), &to)?;
        } else {
            return Err(io::Error::other("unsupported file type in skill tree"));
        }
    }
    Ok(())
}

fn chmod_scripts(dir: &Path) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || !name.ends_with(".sh") {
            continue;
        }
        if is_file(&ent.path()) {
            chmod_x(&ent.path());
        }
    }
}

fn chmod_x(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    let mut perm = meta.permissions();
    perm.set_mode(perm.mode() | 0o111);
    let _ = fs::set_permissions(path, perm);
}

fn rm_rf(path: &Path) -> io::Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if meta.file_type().is_symlink() || meta.is_file() {
        fs::remove_file(path)
    } else {
        fs::remove_dir_all(path)
    }
}

fn dir_names(dir: &Path) -> Result<Vec<OsString>, GuidedError> {
    let mut names = Vec::new();
    for ent in fs::read_dir(dir)? {
        names.push(ent?.file_name());
    }
    Ok(names)
}

fn shell_child_of(path: &Path, parent: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let path = path.as_os_str().as_bytes();
    let parent = parent.as_os_str().as_bytes();
    path.len() > parent.len() && path.starts_with(parent) && path[parent.len()] == b'/'
}

fn is_file(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

fn is_dir(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_dir())
        .unwrap_or(false)
}

fn is_nonempty_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

fn exists(path: &Path) -> bool {
    fs::metadata(path).is_ok()
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn nonempty_var(key: &str) -> Option<OsString> {
    let value = std::env::var_os(key)?;
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn field<'a>(fields: &[&'a str], index: usize) -> &'a str {
    fields.get(index).copied().unwrap_or("")
}

fn repo_toplevel(cwd: &Path) -> Result<PathBuf, GuidedError> {
    let not_repo = "not inside a git repository — cd into the repo you want to work on";
    match git_stdout_line(cwd, &["rev-parse", "--show-toplevel"]) {
        Some(top) if !top.is_empty() => Ok(PathBuf::from(top)),
        _ => Err(message(not_repo)),
    }
}

fn current_branch(repo: &Path) -> String {
    git_stdout_line(repo, &["symbolic-ref", "--short", "HEAD"])
        .unwrap_or_else(|| "main".to_string())
}

fn git_capture(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
    git_capture(dir, args).unwrap_or_default()
}

fn git_stdout_line(dir: &Path, args: &[&str]) -> Option<String> {
    let mut line = git_capture(dir, args)?;
    if line.ends_with('\n') {
        line.pop();
    }
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, MutexGuard};

    static RUST_BIN_ENV: Mutex<()> = Mutex::new(());
    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp {
        root: PathBuf,
    }

    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-adopt-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    struct RestoreRustBin {
        prev: Option<OsString>,
        _lock: MutexGuard<'static, ()>,
    }

    impl Drop for RestoreRustBin {
        fn drop(&mut self) {
            // SAFETY: dropped while RUST_BIN_ENV is still held.
            unsafe {
                match self.prev.take() {
                    Some(v) => std::env::set_var("CRUCIBLE_RUST_BIN", v),
                    None => std::env::remove_var("CRUCIBLE_RUST_BIN"),
                }
            }
        }
    }

    fn lock_rust_bin(value: Option<&OsStr>) -> RestoreRustBin {
        let lock = RUST_BIN_ENV.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var_os("CRUCIBLE_RUST_BIN");
        // SAFETY: callers hold RUST_BIN_ENV for the whole adopt that reads this key.
        unsafe {
            match value {
                Some(v) => std::env::set_var("CRUCIBLE_RUST_BIN", v),
                None => std::env::remove_var("CRUCIBLE_RUST_BIN"),
            }
        }
        RestoreRustBin { prev, _lock: lock }
    }

    fn chmod_x(path: &Path) {
        let mut perm = fs::metadata(path).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(path, perm).unwrap();
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .arg("-c")
            .arg("user.email=t@local")
            .arg("-c")
            .arg("user.name=t")
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@local")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@local")
            .stdin(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} in {}", dir.display());
    }

    fn init_git(dir: &Path) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        fs::write(dir.join("README"), "tracked\n").unwrap();
        git(dir, &["add", "README"]);
        git(dir, &["commit", "-qm", "baseline"]);
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .unwrap();
        assert!(out.status.success());
        PathBuf::from(
            String::from_utf8_lossy(&out.stdout)
                .trim_end_matches('\n')
                .to_string(),
        )
    }

    fn run(cwd: &Path, src: &Path, args: &[&str]) -> Result<String, String> {
        let mut out = Vec::new();
        match cmd_adopt(cwd, src, args, &mut out) {
            Ok(()) => Ok(String::from_utf8(out).unwrap()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn write_guided_src(src: &Path) {
        fs::create_dir_all(src.join("scripts")).unwrap();
        fs::create_dir_all(src.join("roles")).unwrap();
        fs::create_dir_all(src.join("docs")).unwrap();
        fs::write(src.join("VERSION"), "1.17.0\n").unwrap();
        fs::write(src.join("START.md"), "start\n").unwrap();
        fs::write(src.join("BOOTSTRAP.md"), "boot\n").unwrap();
        fs::write(src.join("RULES.md"), "rules\n").unwrap();
        fs::write(src.join("LOOP.md"), "loop\n").unwrap();
        fs::write(src.join("CONFIGURE.md"), "cfg\n").unwrap();
        fs::write(src.join("crucible-guided"), "#!/bin/sh\necho guided\n").unwrap();
        fs::write(src.join("scripts/keep.sh"), "#!/bin/sh\n").unwrap();
        fs::write(src.join("scripts/package-release.sh"), "#!/bin/sh\nno\n").unwrap();
        fs::write(src.join("scripts/verify-package.sh"), "#!/bin/sh\nno\n").unwrap();
        fs::write(src.join("roles/maker.md"), "maker\n").unwrap();
        fs::write(src.join("roles/skip.txt"), "no\n").unwrap();
        fs::write(src.join("docs/install.md"), "install\n").unwrap();
        fs::write(src.join("docs/skip.txt"), "no\n").unwrap();
    }

    fn real_projector() -> String {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/project-skills.sh");
        fs::read_to_string(path).unwrap()
    }

    fn routing(rows: &str) -> String {
        format!("phase\tjob\tbattery\trole\tstake\trequired\n{rows}")
    }

    fn write_wm_src(src: &Path, rows: &str, projector: &str) {
        write_guided_src(src);
        fs::remove_file(src.join("crucible-guided")).unwrap();
        fs::create_dir_all(src.join("skills/architecture/nested")).unwrap();
        fs::write(src.join("skills/architecture/SKILL.md"), "ARCH-BODY\n").unwrap();
        fs::write(src.join("skills/architecture/nested/note.txt"), "nest\n").unwrap();
        fs::create_dir_all(src.join("skills/critique")).unwrap();
        fs::write(src.join("skills/critique/SKILL.md"), "CRIT\n").unwrap();
        fs::write(src.join("wm.sh"), "#!/bin/sh\nprintf 'wm\\n'\n").unwrap();
        fs::write(src.join("WORKING-MODE.md"), "wm-doc\n").unwrap();
        fs::write(src.join("ROUTING.tsv"), routing(rows)).unwrap();
        fs::write(src.join("crucible"), "#!/bin/sh\necho shell\n").unwrap();
        fs::create_dir_all(src.join("target/release")).unwrap();
        fs::write(src.join("target/release/crucible"), b"rust-bin-bytes").unwrap();
        chmod_x(&src.join("target/release/crucible"));
        fs::create_dir_all(src.join("adapters")).unwrap();
        fs::write(src.join("adapters/local.txt"), "adapt\n").unwrap();
        fs::write(src.join("adapters/.secret"), "no\n").unwrap();
        fs::write(src.join("scripts/project-skills.sh"), projector).unwrap();
        chmod_x(&src.join("scripts/project-skills.sh"));
        chmod_x(&src.join("wm.sh"));
    }

    fn assert_real_view(path: &Path, body: &str) {
        let meta = fs::symlink_metadata(path).unwrap();
        assert!(meta.is_dir(), "{}", path.display());
        assert!(!meta.file_type().is_symlink(), "{}", path.display());
        assert_eq!(
            fs::read_to_string(path.join("SKILL.md")).unwrap(),
            body,
            "{}",
            path.display()
        );
        assert_eq!(
            fs::read_to_string(path.join("nested/note.txt")).unwrap(),
            "nest\n"
        );
    }

    const VIEWS: &[&str] = &[
        ".crucible/.grok/skills/architecture",
        ".crucible/.claude/skills/architecture",
        ".crucible/.agents/skills/architecture",
        ".crucible/.kiro/skills/architecture",
        ".grok/skills/architecture",
        ".claude/skills/architecture",
        ".agents/skills/architecture",
        ".kiro/skills/architecture",
    ];

    #[test]
    fn usage_name_and_repo_refusals() {
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        fs::create_dir_all(&src).unwrap();
        let bare = tmp.path().join("bare");
        fs::create_dir_all(&bare).unwrap();
        assert_eq!(
            run(&bare, &src, &["--refresh", "--managed"]).unwrap_err(),
            "usage: crucible adopt [PROGRAM] --refresh  (--managed is install-only)"
        );
        assert_eq!(
            run(&bare, &src, &["--refresh", "--panel-from", "old"]).unwrap_err(),
            "usage: crucible adopt [PROGRAM] --managed --panel-from PROGRAM  (--refresh does not copy a panel)"
        );
        assert_eq!(
            run(&bare, &src, &["--panel-from", "old"]).unwrap_err(),
            "usage: crucible adopt [PROGRAM] --managed --panel-from PROGRAM"
        );
        assert_eq!(
            run(&bare, &src, &["--panel-from"]).unwrap_err(),
            "usage: crucible adopt [PROGRAM] --managed --panel-from PROGRAM"
        );
        assert_eq!(
            run(&bare, &src, &["--working-mode"]).unwrap_err(),
            "usage: crucible adopt [PROGRAM] --managed --working-mode"
        );
        assert_eq!(run(&bare, &src, &["--nope"]).unwrap_err(), USAGE);
        assert_eq!(run(&bare, &src, &["a", "b"]).unwrap_err(), USAGE);
        assert_eq!(
            run(&bare, &src, &["bad name", "--managed"]).unwrap_err(),
            "program name may only contain A-Za-z0-9._-"
        );
        assert_eq!(
            run(&bare, &src, &["work", "--managed"]).unwrap_err(),
            "not inside a git repository — cd into the repo you want to work on"
        );
    }

    #[test]
    fn managed_adopt_copies_shell_entry_and_not_floor() {
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        write_guided_src(&src);
        let repo = init_git(&tmp.path().join("repo"));
        let sub = repo.join("nested");
        fs::create_dir(&sub).unwrap();
        fs::create_dir_all(repo.join(".crucible")).unwrap();
        fs::write(
            repo.join(".crucible/.gitignore"),
            "# custom\n*/agents.tsv\n",
        )
        .unwrap();
        let out = run(&sub, &src, &["work", "--managed"]).unwrap();
        let dst = repo.join(".crucible/work");
        let installed = format!("installed cycle \"work\" into {}\n\nFresh-agent entrypoint:\n  read .crucible/work/START.md and execute it\n\nThe agent will configure its panel, investigate the problem, ask for proposal approval,\nand coordinate build/review iterations. The operator does not drive protocol commands.\n", dst.display());
        assert_eq!(out, installed);
        let top = repo.display();
        assert_eq!(
            fs::read_to_string(dst.join("PROGRAM")).unwrap(),
            format!("repo: {top}\nprogram: work\nbase: main\nlifecycle: managed\ncycle: guided\n")
        );
        assert_eq!(
            fs::read_to_string(dst.join("STATE.tsv")).unwrap(),
            format!("{STATE_HEADER}\n")
        );
        assert_eq!(
            fs::read_to_string(dst.join("STATE.md")).unwrap(),
            "\
# STATE — work

Generated from `STATE.tsv`; do not edit this file by hand.

| Item | Status | Stage | Work ID | Risk | In-flight attempt | Block code | Updated epoch |
| --- | --- | --- | --- | --- | --- | --- | --- |
"
        );
        assert_eq!(fs::read_to_string(dst.join("LESSONS.md")).unwrap(), LESSONS);
        assert_eq!(fs::read_to_string(dst.join("BACKLOG.md")).unwrap(), BACKLOG);
        assert_eq!(fs::read_to_string(dst.join("PROBLEM.md")).unwrap(), PROBLEM);
        assert!(fs::read_to_string(dst.join("CLAIMS.md"))
            .unwrap()
            .starts_with("# CLAIMS — work\n"));
        assert_eq!(
            fs::read_to_string(dst.join("agents.tsv")).unwrap(),
            AGENTS_TEMPLATE
        );
        assert_eq!(
            fs::read(dst.join("crucible")).unwrap(),
            b"#!/bin/sh\necho guided\n"
        );
        assert_eq!(
            fs::read(dst.join("crucible-guided")).unwrap(),
            b"#!/bin/sh\necho guided\n"
        );
        assert_ne!(
            fs::metadata(dst.join("crucible"))
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0
        );
        assert_eq!(fs::read_to_string(dst.join("START.md")).unwrap(), "start\n");
        assert_eq!(fs::read_to_string(dst.join("VERSION")).unwrap(), "1.17.0\n");
        assert_eq!(
            fs::read_to_string(dst.join("scripts/keep.sh")).unwrap(),
            "#!/bin/sh\n"
        );
        assert!(!dst.join("scripts/package-release.sh").exists());
        assert!(!dst.join("scripts/verify-package.sh").exists());
        assert_eq!(
            fs::read_to_string(dst.join("roles/maker.md")).unwrap(),
            "maker\n"
        );
        assert!(!dst.join("roles/skip.txt").exists());
        assert_eq!(
            fs::read_to_string(dst.join("docs/install.md")).unwrap(),
            "install\n"
        );
        assert!(!dst.join("docs/skip.txt").exists());
        assert!(!dst.join("wm.sh").exists());
        assert!(!dst.join("ENGINE-SOURCE").exists());
        assert!(!repo.join(".crucible/skills").exists());
        assert!(!repo.join(".grok").exists());
        assert!(!repo.join(".wm").exists());
        let ignore = fs::read_to_string(repo.join(".crucible/.gitignore")).unwrap();
        assert!(ignore.starts_with("# custom\n*/agents.tsv\n"));
        assert_eq!(ignore.matches("*/agents.tsv").count(), 1);
        assert!(ignore.contains("*/PANEL.AGENTS.tsv\n"));
        assert!(ignore.contains("*/worktrees/\n"));
        assert!(!ignore.contains(".wm"));
        assert_eq!(
            run(&repo, &src, &["work", "--managed"]).unwrap_err(),
            format!("program already exists: {}", dst.display())
        );
    }

    #[test]
    fn item_file_adopt_stdout_and_state() {
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        write_guided_src(&src);
        let repo = init_git(&tmp.path().join("repo"));
        run(&repo, &src, &["plain"]).unwrap();
        let dst = repo.join(".crucible/plain");
        assert_eq!(
            fs::read_to_string(dst.join("PROGRAM")).unwrap(),
            format!("repo: {}\nprogram: plain\nbase: main\n", repo.display())
        );
        assert!(!dst.join("STATE.tsv").exists());
        assert_eq!(
            fs::read_to_string(dst.join("STATE.md")).unwrap(),
            item_file_state("plain", &repo.display().to_string())
        );
    }

    #[test]
    fn refresh_keeps_local_adapter_and_refuses_husk_and_same_tree() {
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        write_guided_src(&src);
        let repo = init_git(&tmp.path().join("repo"));
        run(&repo, &src, &["work", "--managed"]).unwrap();
        let dst = repo.join(".crucible/work");
        fs::write(dst.join("VERSION"), "1.0.0\n").unwrap();
        fs::write(dst.join("PROBLEM.md"), "KEEP-PROBLEM\n").unwrap();
        fs::create_dir_all(dst.join("scripts")).unwrap();
        fs::write(dst.join("scripts/acp-brief.py"), "KEEP-ACP\n").unwrap();
        fs::write(src.join("VERSION"), "9.9.9\n").unwrap();
        fs::write(src.join("START.md"), "new-start\n").unwrap();
        let out = run(&repo, &src, &["work", "--refresh"]).unwrap();
        assert_eq!(
            out,
            format!(
                "refreshed engine 1.0.0 -> 9.9.9\n{}\nkept local adapter: scripts/acp-brief.py\n",
                dst.display()
            )
        );
        assert_eq!(
            fs::read_to_string(dst.join("scripts/acp-brief.py")).unwrap(),
            "KEEP-ACP\n"
        );
        assert_eq!(
            fs::read_to_string(dst.join("PROBLEM.md")).unwrap(),
            "KEEP-PROBLEM\n"
        );
        assert_eq!(fs::read_to_string(dst.join("VERSION")).unwrap(), "9.9.9\n");
        assert_eq!(
            fs::read_to_string(dst.join("START.md")).unwrap(),
            "new-start\n"
        );
        assert!(!repo.join(".wm").exists());

        fs::remove_file(dst.join("PROGRAM")).unwrap();
        assert_eq!(
            run(&repo, &src, &["work", "--refresh"]).unwrap_err(),
            format!(
                "refused: {} is a husk (no PROGRAM). Keep it, trash it, or adopt a different program name",
                dst.display()
            )
        );
        let missing = repo.join(".crucible/other");
        assert_eq!(
            run(&repo, &src, &["other", "--refresh"]).unwrap_err(),
            format!("program does not exist: {}", missing.display())
        );

        fs::write(dst.join("PROGRAM"), "program: work\n").unwrap();
        assert_eq!(
            run(&repo, &dst, &["work", "--refresh"]).unwrap_err(),
            "refused: refresh source is the destination (src == dst); use a versioned tarball"
        );
        let link = tmp.path().join("same");
        std::os::unix::fs::symlink(&dst, &link).unwrap();
        assert_eq!(
            run(&repo, &link, &["work", "--refresh"]).unwrap_err(),
            "refused: refresh source is the destination (src == dst); use a versioned tarball"
        );
    }

    #[test]
    fn panel_from_copies_an_approved_panel_and_refuses_the_rest() {
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        write_guided_src(&src);
        let repo = init_git(&tmp.path().join("repo"));
        run(&repo, &src, &["old", "--managed"]).unwrap();
        let old = repo.join(".crucible/old");
        fs::write(old.join("PANEL.md"), "panel\n").unwrap();
        fs::write(old.join("PANEL.ASSIGN.tsv"), "assign\n").unwrap();
        fs::write(old.join("agents.tsv"), "mk\tkind\n").unwrap();
        fs::write(old.join("PANEL.APPROVAL"), "").unwrap();
        fs::write(old.join("PANEL.AGENTS.tsv"), "copy\n").unwrap();
        fs::create_dir(old.join("panel-approvals")).unwrap();
        fs::write(old.join("panel-approvals/a.md"), "rec\n").unwrap();
        fs::write(old.join("panel-approvals/skip.txt"), "no\n").unwrap();
        let out = run(&repo, &src, &["new", "--managed", "--panel-from", "old"]).unwrap();
        assert!(out.contains(
            "copied approved panel from .crucible/old (leftover PROBLEM stays on old)\n"
        ));
        let dst = repo.join(".crucible/new");
        assert_eq!(
            fs::read_to_string(dst.join("agents.tsv")).unwrap(),
            "mk\tkind\n"
        );
        assert_eq!(fs::read_to_string(dst.join("PANEL.md")).unwrap(), "panel\n");
        assert_eq!(
            fs::read_to_string(dst.join("PANEL.ASSIGN.tsv")).unwrap(),
            "assign\n"
        );
        assert_eq!(fs::read_to_string(dst.join("PANEL.APPROVAL")).unwrap(), "");
        assert_eq!(
            fs::read_to_string(dst.join("PANEL.AGENTS.tsv")).unwrap(),
            "copy\n"
        );
        assert_eq!(
            fs::read_to_string(dst.join("panel-approvals/a.md")).unwrap(),
            "rec\n"
        );
        assert!(!dst.join("panel-approvals/skip.txt").exists());
        assert_eq!(fs::read_to_string(dst.join("PROBLEM.md")).unwrap(), PROBLEM);
        assert_eq!(fs::read_to_string(old.join("PROBLEM.md")).unwrap(), PROBLEM);

        assert_eq!(
            run(
                &repo,
                &src,
                &["newer", "--managed", "--panel-from", "newer"]
            )
            .unwrap_err(),
            "refused: --panel-from cannot be the new program name"
        );
        assert_eq!(
            run(
                &repo,
                &src,
                &["newer", "--managed", "--panel-from", "missing"]
            )
            .unwrap_err(),
            format!(
                "refused: --panel-from program does not exist: {}",
                repo.join(".crucible/missing").display()
            )
        );
        fs::remove_file(old.join("PANEL.APPROVAL")).unwrap();
        assert_eq!(
            run(&repo, &src, &["newer", "--managed", "--panel-from", "old"]).unwrap_err(),
            "refused: --panel-from old panel is not approved"
        );
        assert!(!repo.join(".crucible/newer").exists());
        fs::write(old.join("PANEL.APPROVAL"), "ok\n").unwrap();
        fs::write(old.join("PANEL.md"), "").unwrap();
        assert_eq!(
            run(&repo, &src, &["newer", "--managed", "--panel-from", "old"]).unwrap_err(),
            "refused: --panel-from old has no panel to copy"
        );
    }

    #[test]
    fn gitignore_warns_when_machine_local_rows_are_tracked() {
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        write_guided_src(&src);
        let repo = init_git(&tmp.path().join("repo"));
        run(&repo, &src, &["work", "--managed"]).unwrap();
        git(&repo, &["add", "-f", ".crucible/work/agents.tsv"]);
        git(&repo, &["commit", "-qm", "track agents"]);
        let out = run(&repo, &src, &["work", "--refresh"]).unwrap();
        assert!(out.contains(
            "still tracked (machine-local): .crucible/work/agents.tsv — untrack with: git rm --cached .crucible/work/agents.tsv\n"
        ));
        assert!(out.contains("refreshed engine "));
    }

    #[test]
    fn sha_script_battery_and_view_rules() {
        let tmp = Tmp::new();
        fs::write(tmp.path().join("VERSION"), b"abc").unwrap();
        assert_eq!(
            adopt_src_sha256(tmp.path()).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        fs::write(tmp.path().join("crucible"), b"#!/bin/sh\n").unwrap();
        fs::create_dir_all(tmp.path().join("target/release")).unwrap();
        fs::write(tmp.path().join("target/release/crucible"), b"abc").unwrap();
        chmod_x(&tmp.path().join("target/release/crucible"));
        fs::remove_file(tmp.path().join("VERSION")).unwrap();
        assert!(adopt_is_script(&tmp.path().join("crucible")));
        assert!(!adopt_is_script(&tmp.path().join("missing")));
        assert!(!adopt_is_script(
            &tmp.path().join("target/release/crucible")
        ));
        assert_eq!(
            adopt_src_sha256(tmp.path()).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        fs::write(tmp.path().join("crucible"), b"abc").unwrap();
        assert_eq!(
            adopt_src_sha256(tmp.path()).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        let routing = tmp.path().join("ROUTING.tsv");
        fs::write(
            &routing,
            "phase\tjob\tbattery\trole\tstake\trequired\nA\tx\tfirst\tr\ts\tyes\nB\tx\tsecond\tr\ts\tyes\nC\tx\topt\tr\ts\tno\nD\tx\t-\tr\ts\tyes\n# skip\n",
        )
        .unwrap();
        let skills = tmp.path().join("skills");
        fs::create_dir_all(&skills).unwrap();
        assert_eq!(
            adopt_check_required_batteries(&routing, &skills)
                .unwrap_err()
                .to_string(),
            "refused: required battery missing: first second"
        );
        fs::create_dir_all(skills.join("first")).unwrap();
        fs::write(skills.join("first/SKILL.md"), "x\n").unwrap();
        fs::create_dir_all(skills.join("second")).unwrap();
        fs::write(skills.join("second/SKILL.md"), "x\n").unwrap();
        adopt_check_required_batteries(&routing, &skills).unwrap();
        assert_eq!(
            adopt_check_required_batteries(&tmp.path().join("nope"), &skills)
                .unwrap_err()
                .to_string(),
            "refused: ROUTING.tsv missing"
        );

        let repo = tmp.path().join("kept");
        fs::create_dir_all(repo.join(".crucible/skills/arch")).unwrap();
        fs::write(repo.join(".crucible/skills/arch/.keep"), "").unwrap();
        assert!(adopt_battery_kept(&repo, "arch"));
        fs::remove_file(repo.join(".crucible/skills/arch/.keep")).unwrap();
        fs::write(repo.join(".crucible/skills/arch/KEEP"), "").unwrap();
        assert!(adopt_battery_kept(&repo, "arch"));
        fs::remove_file(repo.join(".crucible/skills/arch/KEEP")).unwrap();
        fs::write(repo.join(".crucible/KEEP"), "# no\narch extra\n").unwrap();
        assert!(adopt_battery_kept(&repo, "arch"));
        fs::write(repo.join(".crucible/KEEP"), "  # arch\nnope\n").unwrap();
        assert!(!adopt_battery_kept(&repo, "arch"));
        assert_eq!(adopt_gitignore_reason("nope"), "");

        fs::create_dir_all(repo.join(".crucible/skills/architecture")).unwrap();
        fs::write(
            repo.join(".crucible/skills/architecture/SKILL.md"),
            "BODY\n",
        )
        .unwrap();
        fs::create_dir_all(repo.join(".crucible/skills/architecture/nested")).unwrap();
        fs::write(
            repo.join(".crucible/skills/architecture/nested/note.txt"),
            "nest\n",
        )
        .unwrap();
        let view = repo.join(".grok/skills/architecture");
        fs::create_dir_all(view.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(repo.join(".crucible/skills/architecture"), &view).unwrap();
        adopt_refresh_skill_views(&repo, "architecture").unwrap();
        assert_real_view(&view, "BODY\n");
        let link = repo.join(".crucible/skills/linked");
        std::os::unix::fs::symlink(repo.join(".crucible/skills/architecture"), &link).unwrap();
        adopt_refresh_skill_views(&repo, "linked").unwrap();
        assert!(!repo.join(".grok/skills/linked").exists());
    }

    #[test]
    fn find_rust_bin_prefers_env_then_nonscript_then_release() {
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        fs::create_dir_all(src.join("target/release")).unwrap();
        fs::write(src.join("crucible"), b"#!/bin/sh\n").unwrap();
        chmod_x(&src.join("crucible"));
        fs::write(src.join("target/release/crucible"), b"rel").unwrap();
        chmod_x(&src.join("target/release/crucible"));
        let _env = lock_rust_bin(None);
        assert_eq!(
            adopt_find_rust_bin(&src).unwrap(),
            src.join("target/release/crucible")
        );
        fs::write(src.join("crucible"), b"bin").unwrap();
        chmod_x(&src.join("crucible"));
        assert_eq!(adopt_find_rust_bin(&src).unwrap(), src.join("crucible"));
        let other = tmp.path().join("other-bin");
        fs::write(&other, b"env").unwrap();
        chmod_x(&other);
        drop(_env);
        let _env = lock_rust_bin(Some(other.as_os_str()));
        assert_eq!(adopt_find_rust_bin(&src).unwrap(), other);
        drop(_env);
        let plain = tmp.path().join("plain");
        fs::write(&plain, b"noexec").unwrap();
        let _env = lock_rust_bin(Some(plain.as_os_str()));
        assert_eq!(adopt_find_rust_bin(&src).unwrap(), src.join("crucible"));
        drop(_env);
        let _env = lock_rust_bin(None);
        let empty = tmp.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        assert_eq!(
            adopt_find_rust_bin(&empty).unwrap_err().to_string(),
            "refused: working-mode source is missing the crucible binary (cargo build --release)"
        );
    }

    #[test]
    fn working_mode_projects_real_directories_and_restores_keep() {
        let _env = lock_rust_bin(None);
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        write_wm_src(
            &src,
            "MAP\tdecompose\tarchitecture\tplanner\tspec\tyes\nRESEARCH\tsurvey\tresearch\tspecifier\tspec\tno\n",
            &real_projector(),
        );
        let repo = init_git(&tmp.path().join("repo"));
        fs::create_dir_all(repo.join(".crucible/skills/architecture")).unwrap();
        fs::write(
            repo.join(".crucible/skills/architecture/SKILL.md"),
            "PATCHED\n",
        )
        .unwrap();
        fs::write(repo.join(".crucible/skills/architecture/.keep"), "").unwrap();
        let out = run(&repo, &src, &["work", "--managed", "--working-mode"]).unwrap();
        assert!(out.starts_with("installed cycle \"work\" into "));
        let dst = repo.join(".crucible/work");
        assert!(out.contains(&format!("{}\n", dst.display())));
        assert_eq!(fs::read(dst.join("crucible")).unwrap(), b"rust-bin-bytes");
        assert_ne!(
            fs::metadata(dst.join("crucible"))
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0
        );
        assert_eq!(
            fs::read(dst.join("crucible-guided")).unwrap(),
            b"#!/bin/sh\necho shell\n"
        );
        assert_eq!(
            fs::read_to_string(dst.join("wm.sh")).unwrap(),
            "#!/bin/sh\nprintf 'wm\\n'\n"
        );
        assert_eq!(
            fs::read_to_string(dst.join("WORKING-MODE.md")).unwrap(),
            "wm-doc\n"
        );
        assert_eq!(
            fs::read_to_string(dst.join("adapters/local.txt")).unwrap(),
            "adapt\n"
        );
        assert!(!dst.join("adapters/.secret").exists());
        assert_eq!(
            fs::read_to_string(repo.join(".crucible/ROUTING.tsv")).unwrap(),
            fs::read_to_string(dst.join("ROUTING.tsv")).unwrap()
        );
        let source = fs::read_to_string(dst.join("ENGINE-SOURCE")).unwrap();
        assert!(source.starts_with("version: 1.17.0\nsha256: "));
        let hash = source
            .lines()
            .nth(1)
            .unwrap()
            .strip_prefix("sha256: ")
            .unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(adopt_src_sha256(&src).unwrap(), hash);
        assert!(fs::read_to_string(dst.join("PROGRAM"))
            .unwrap()
            .contains("working-mode: yes\n"));
        assert_eq!(
            fs::read_to_string(repo.join(".crucible/skills/architecture/SKILL.md")).unwrap(),
            "ARCH-BODY\n"
        );
        assert!(!repo.join(".crucible/skills/architecture/.keep").exists());
        assert!(!repo.join(".crucible/skills/research").exists());
        for rel in VIEWS {
            assert_real_view(&repo.join(rel), "ARCH-BODY\n");
        }
        assert!(!repo.join(".codex").exists());
        assert!(!repo.join(".wm").exists());
        let ignore = fs::read_to_string(repo.join(".crucible/.gitignore")).unwrap();
        assert!(ignore.contains("*/agents.tsv\n"));
        assert!(ignore.contains("*/worktrees/\n"));
        assert!(!ignore.contains(".wm"));

        fs::write(
            repo.join(".crucible/skills/architecture/SKILL.md"),
            "PATCHED-ARCH\n",
        )
        .unwrap();
        fs::write(repo.join(".crucible/skills/architecture/.keep"), "").unwrap();
        fs::write(
            repo.join(".crucible/skills/critique/SKILL.md"),
            "PATCHED-CRIT\n",
        )
        .unwrap();
        fs::create_dir_all(repo.join(".crucible/skills/review")).unwrap();
        fs::write(
            repo.join(".crucible/skills/review/SKILL.md"),
            "PATCHED-REVIEW\n",
        )
        .unwrap();
        fs::write(repo.join(".crucible/skills/KEEP"), "review\n").unwrap();
        run(&repo, &src, &["work", "--refresh"]).unwrap();
        assert_eq!(
            fs::read_to_string(repo.join(".crucible/skills/architecture/SKILL.md")).unwrap(),
            "PATCHED-ARCH\n"
        );
        assert!(repo.join(".crucible/skills/architecture/.keep").exists());
        assert_eq!(
            fs::read_to_string(repo.join(".crucible/skills/critique/SKILL.md")).unwrap(),
            "CRIT\n"
        );
        assert_eq!(
            fs::read_to_string(repo.join(".crucible/skills/review/SKILL.md")).unwrap(),
            "PATCHED-REVIEW\n"
        );
        assert_real_view(&repo.join(".grok/skills/architecture"), "PATCHED-ARCH\n");
        run(&repo, &src, &["work", "--refresh", "--overwrite-batteries"]).unwrap();
        assert_eq!(
            fs::read_to_string(repo.join(".crucible/skills/architecture/SKILL.md")).unwrap(),
            "ARCH-BODY\n"
        );
        assert!(!repo.join(".crucible/skills/architecture/.keep").exists());
    }

    #[test]
    fn keep_restore_runs_when_skill_projection_fails() {
        let _env = lock_rust_bin(None);
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        write_wm_src(
            &src,
            "MAP\tdecompose\tarchitecture\tplanner\tspec\tyes\n",
            &real_projector(),
        );
        let repo = init_git(&tmp.path().join("repo"));
        run(&repo, &src, &["work", "--managed", "--working-mode"]).unwrap();
        fs::write(
            repo.join(".crucible/skills/architecture/SKILL.md"),
            "PATCHED-KEEP-FAIL\n",
        )
        .unwrap();
        fs::write(repo.join(".crucible/skills/architecture/.keep"), "m\n").unwrap();
        fs::write(
            repo.join(".crucible/skills/critique/SKILL.md"),
            "UNKEPT-CRITIQUE\n",
        )
        .unwrap();
        let bad = tmp.path().join("bad");
        write_wm_src(
            &bad,
            "MAP\tdecompose\tarchitecture\tplanner\tspec\tyes\n",
            "#!/bin/sh\nset -eu\nDST=$2\nskills=\"$DST/.crucible/skills\"\nif [ -d \"$skills\" ]; then\n  for d in \"$skills\"/*; do\n    [ -d \"$d\" ] || continue\n    rm -rf \"$d\"\n  done\nfi\nprintf 'project-skills: fixture fail after wipe\\n' >&2\nexit 1\n",
        );
        let err = run(&repo, &bad, &["work", "--refresh"]).unwrap_err();
        assert_eq!(err, "refused: skill projection failed");
        assert_eq!(
            fs::read_to_string(repo.join(".crucible/skills/architecture/SKILL.md")).unwrap(),
            "PATCHED-KEEP-FAIL\n"
        );
        assert_eq!(
            fs::read_to_string(repo.join(".crucible/skills/architecture/.keep")).unwrap(),
            "m\n"
        );
        assert!(!repo.join(".crucible/skills/critique/SKILL.md").exists());
        assert_eq!(
            fs::read_to_string(repo.join(".grok/skills/architecture/SKILL.md")).unwrap(),
            "PATCHED-KEEP-FAIL\n"
        );
        let dst = repo.join(".crucible/work");
        let leftover = fs::read_dir(&dst).unwrap().any(|ent| {
            ent.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".keep-snapshot.")
        });
        assert!(!leftover);
        let meta = fs::symlink_metadata(repo.join(".grok/skills/architecture")).unwrap();
        assert!(meta.is_dir());
        assert!(!meta.file_type().is_symlink());
    }

    #[test]
    fn missing_required_battery_does_not_install() {
        let _env = lock_rust_bin(None);
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        write_wm_src(
            &src,
            "MAP\tdecompose\tarchitecture\tplanner\tspec\tyes\nX\tmissing\tno-such-battery\toperator\tspec\tyes\n",
            &real_projector(),
        );
        let repo = init_git(&tmp.path().join("repo"));
        let err = run(&repo, &src, &["work", "--managed", "--working-mode"]).unwrap_err();
        assert_eq!(err, "refused: required battery missing: no-such-battery");
        assert!(!repo.join(".crucible/work").exists());
        assert!(!repo.join(".crucible/work/wm.sh").exists());
    }

    #[test]
    fn refresh_working_mode_appends_program_line_once() {
        let _env = lock_rust_bin(None);
        let tmp = Tmp::new();
        let src = tmp.path().join("src");
        write_wm_src(
            &src,
            "MAP\tdecompose\tarchitecture\tplanner\tspec\tyes\n",
            &real_projector(),
        );
        let repo = init_git(&tmp.path().join("repo"));
        run(&repo, &src, &["work", "--managed"]).unwrap();
        let program = repo.join(".crucible/work/PROGRAM");
        assert!(!fs::read_to_string(&program)
            .unwrap()
            .contains("working-mode:"));
        run(&repo, &src, &["work", "--refresh", "--working-mode"]).unwrap();
        let text = fs::read_to_string(&program).unwrap();
        assert!(text.contains("\nworking-mode: yes\n"));
        assert_eq!(text.matches("working-mode: yes").count(), 1);
        assert!(repo.join(".crucible/work/wm.sh").is_file());
        run(&repo, &src, &["work", "--refresh"]).unwrap();
        assert_eq!(
            fs::read_to_string(&program)
                .unwrap()
                .matches("working-mode: yes")
                .count(),
            1
        );
        assert_eq!(
            adopt_install_working_mode(&src, &repo.join(".crucible/work"), Path::new("/"), false)
                .unwrap_err()
                .to_string(),
            "refused: bad repository root"
        );
    }
}
