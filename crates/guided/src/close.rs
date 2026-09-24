//! `check` and `close`.
//!
//! The lesson date is the UTC calendar date of [`Clock::now_unix`] (shell line 5974),
//! not `date +%Y-%m-%d`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crucible_contract::{format_rfc3339_z, Clock};

use crate::attempt::result_field;
use crate::claims::{min_kinds_label, require_attempt_independence, rewrite_claim_line};
use crate::cycle::{
    attempt_child_dirs, claim_field, count_claim_headings, file_name, read_dir_paths,
    stale_evidence, workid, MARK,
};
use crate::dispatch::{is_maker, need, validate_managed_item};
use crate::panel::{h12, is_posint, kind_of, min_kinds, panel_required_count};
use crate::phase::phase_of;
use crate::program::{uses_guided_cycle, uses_managed_lifecycle};
use crate::state::{state_update_item, state_validate_file, state_value};
use crate::{message, records, GuidedError};

/// Stdout of `crucible check`, plus the shell status (`1` when not closeable).
#[derive(Debug)]
pub struct CheckReport {
    pub text: String,
    pub status: i32,
}

/// `crucible check SLUG`. A die inside the gate is [`Err`]. `NOT CLOSEABLE` is status 1.
pub fn check(root: &Path, args: &[&str]) -> Result<CheckReport, GuidedError> {
    let slug = args.first().copied().unwrap_or("");
    let dir = need(root, slug)?;
    let wid = workid(root, slug)?;
    let mut out = format!("check {slug} (work id {wid})\n");
    let mut bad = false;
    let mut scan = vec![dir.join("evidence"), dir.join("verdicts")];
    if !dir.join("TARGET").is_file() {
        scan.insert(0, dir.join("work"));
    }
    for path in scan {
        for line in records(&faults(&path)) {
            say(&mut out, &mut bad, &format!("FAIL {line}"));
        }
    }
    if wid == "EMPTY" {
        say(
            &mut out,
            &mut bad,
            &format!("FAIL no work in {}/work/", dir.display()),
        );
    }
    if wid == "NOBRANCH" {
        let branch = crate::dispatch::tgt(root, slug, "branch")?;
        let repo = crate::dispatch::tgt(root, slug, "repo")?;
        say(
            &mut out,
            &mut bad,
            &format!("FAIL branch {branch} does not exist in {repo}"),
        );
    }
    if uses_managed_lifecycle(root)? {
        validate_managed_item(root, slug)?;
        if phase_of(root, slug)? != "REVIEW" {
            say(&mut out, &mut bad, "FAIL managed item must be in REVIEW");
        }
    } else {
        check_item_falsifier(&dir, &mut out, &mut bad)?;
    }
    if !maker_recorded(&dir) {
        say(
            &mut out,
            &mut bad,
            &format!(
                "FAIL no maker recorded — run: crucible brief {slug} maker NAME (or dispatch a maker)"
            ),
        );
    }
    if !uses_managed_lifecycle(root)? {
        check_phase_files(root, slug, &dir, &mut out, &mut bad)?;
    }
    let mut ok_agents = Vec::new();
    check_evidence(root, slug, &dir, &wid, &mut out, &mut bad, &mut ok_agents)?;
    check_verdicts(root, slug, &dir, &wid, &ok_agents, &mut out, &mut bad)?;
    let need_j = guided_min_judges(root)?;
    let vn = passing_count(&out);
    if vn < need_j {
        say(
            &mut out,
            &mut bad,
            &format!(
                "FAIL {vn} passing judges, need {}",
                guided_min_judges_label(root)?
            ),
        );
    }
    let kn = passing_kinds(&out);
    let need_k = min_kinds()?;
    if (kn as u64) < need_k {
        say(
            &mut out,
            &mut bad,
            &format!("FAIL {kn} distinct kinds, need {}", min_kinds_label()?),
        );
    }
    if bad {
        out.push_str("NOT CLOSEABLE\n");
        return Ok(CheckReport {
            text: out,
            status: 1,
        });
    }
    out.push_str(&format!("CLOSEABLE {wid}\n"));
    Ok(CheckReport {
        text: out,
        status: 0,
    })
}

fn say(out: &mut String, bad: &mut bool, msg: &str) {
    out.push_str("  ");
    out.push_str(msg);
    out.push('\n');
    *bad = true;
}

fn passing_count(text: &str) -> u64 {
    records(text)
        .iter()
        .filter(|line| line.starts_with("  ok   "))
        .count() as u64
}

fn passing_kinds(text: &str) -> usize {
    let mut kinds = Vec::new();
    for line in records(text) {
        let Some(rest) = line.strip_prefix("  ok   ") else {
            continue;
        };
        let Some(kind) = kind_in_ok(rest) else {
            continue;
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    kinds.len()
}

fn kind_in_ok(line: &str) -> Option<String> {
    let marker = "(kind ";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find(", recorded")?;
    Some(rest[..end].to_string())
}

fn faults(dir: &Path) -> String {
    if !dir.is_dir() {
        return format!("missing directory {}\n", dir.display());
    }
    let mut symlinks = Vec::new();
    let mut newline = false;
    walk_faults(dir, dir, &mut symlinks, &mut newline);
    symlinks.sort();
    let mut out = String::new();
    for rel in symlinks {
        out.push_str(&format!("symlink not allowed: {rel}\n"));
    }
    if newline {
        out.push_str(&format!(
            "a filename under {} contains a newline\n",
            dir.display()
        ));
    }
    out
}

fn walk_faults(dir: &Path, root: &Path, symlinks: &mut Vec<String>, newline: &mut bool) {
    let mut paths = read_dir_paths(dir);
    paths.sort();
    for path in paths {
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            let rel = path
                .strip_prefix(root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| file_name(&path));
            symlinks.push(rel);
            continue;
        }
        if meta.is_dir() {
            walk_faults(&path, root, symlinks, newline);
        } else if meta.is_file() && path.to_string_lossy().contains('\n') {
            *newline = true;
        }
    }
}

fn check_item_falsifier(dir: &Path, out: &mut String, bad: &mut bool) -> Result<(), GuidedError> {
    let text = fs::read_to_string(dir.join("ITEM.md")).unwrap_or_default();
    if text.contains("TEMPLATE-FALSIFIER-UNWRITTEN") {
        say(out, bad, "FAIL falsifier not written in ITEM.md");
        return Ok(());
    }
    if !records(&text)
        .iter()
        .any(|line| line.starts_with("## Falsifier"))
    {
        say(out, bad, "FAIL ITEM.md has no ## Falsifier section");
        return Ok(());
    }
    let body = falsifier_body(&text);
    let stripped: String = body
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{000b}' | '\u{000c}'))
        .collect();
    if stripped.chars().count() < 40 {
        say(
            out,
            bad,
            "FAIL the falsifier is empty or too short to name a change and a check",
        );
    }
    Ok(())
}

fn falsifier_body(text: &str) -> String {
    let mut on = false;
    let mut body = String::new();
    for line in records(text) {
        if !on {
            if line.starts_with("## Falsifier") {
                on = true;
            }
            continue;
        }
        if line.starts_with("## ") {
            break;
        }
        body.push_str(line);
        body.push('\n');
    }
    body
}

fn maker_recorded(dir: &Path) -> bool {
    let maker = dir.join("MAKER");
    let maker_ok = maker.is_file()
        && fs::metadata(&maker)
            .map(|meta| meta.len() > 0)
            .unwrap_or(false);
    if maker_ok {
        return true;
    }
    let makers = dir.join("MAKERS.tsv");
    if !makers.is_file() {
        return false;
    }
    fs::read_to_string(&makers)
        .map(|text| records(&text).len() >= 2)
        .unwrap_or(false)
}

fn check_phase_files(
    root: &Path,
    slug: &str,
    dir: &Path,
    out: &mut String,
    bad: &mut bool,
) -> Result<(), GuidedError> {
    let phase = phase_of(root, slug)?;
    let required: &[&str] = match phase.as_str() {
        "TASKS" => &["DESIGN.md"],
        "BUILD" | "VERIFY" | "ADVERSARY" => &["DESIGN.md", "TASKS.md"],
        "GRADUATE" => &["DESIGN.md", "TASKS.md", "ADVERSARY.md"],
        _ => &[],
    };
    for name in required {
        if !dir.join(name).is_file() {
            say(out, bad, &format!("FAIL phase {phase} requires {name}"));
        }
    }
    Ok(())
}

fn check_evidence(
    root: &Path,
    slug: &str,
    dir: &Path,
    wid: &str,
    out: &mut String,
    bad: &mut bool,
    ok_agents: &mut Vec<String>,
) -> Result<(), GuidedError> {
    let evidence = dir.join("evidence");
    if !any_regular_file(&evidence) {
        say(
            out,
            bad,
            &format!("FAIL no evidence — use: crucible run {slug} NAME -- CMD"),
        );
        return Ok(());
    }
    let mut names = read_dir_paths(&evidence);
    names.retain(|path| path.is_file() && !file_name(path).starts_with('.'));
    names.sort();
    let managed = uses_managed_lifecycle(root)?;
    let mut en = 0u64;
    for path in names {
        let name = file_name(&path);
        if name.starts_with(".partial.") {
            continue;
        }
        let Some(stem) = name.strip_suffix(".txt") else {
            say(out, bad, &format!("FAIL evidence {name} must end in .txt"));
            continue;
        };
        if managed && skip_task_evidence(root, &path)? {
            continue;
        }
        let id = stem.rsplit('.').next().unwrap_or(stem);
        if id != wid {
            say(
                out,
                bad,
                &format!("FAIL stale evidence {name} (not from {wid})"),
            );
            continue;
        }
        let rest = match stem.rsplit_once('.') {
            Some((rest, _)) => rest,
            None => stem,
        };
        let (an, seq) = match rest.rsplit_once('.') {
            Some((an, seq)) => (an, seq),
            None => (rest, rest),
        };
        if an.is_empty() || an.contains('.') || seq.is_empty() || !is_token(seq) {
            say(
                out,
                bad,
                &format!("FAIL evidence {name}: name must be AGENT.TOKEN.WORKID.txt"),
            );
            continue;
        }
        if kind_of(root, an)?.is_empty() {
            say(
                out,
                bad,
                &format!("FAIL evidence {name}: '{an}' is not a registered agent"),
            );
            continue;
        }
        let meta = fs::metadata(&path)?;
        if meta.len() == 0 {
            say(out, bad, &format!("FAIL empty evidence {name}"));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let first = records(&text).first().copied().unwrap_or("");
        if first != MARK {
            say(
                out,
                bad,
                &format!("FAIL evidence {name} was not produced by 'crucible run'"),
            );
            continue;
        }
        let iw = field_line(&text, "work-id");
        if iw != wid {
            say(
                out,
                bad,
                &format!("FAIL evidence {name} says work-id {iw} inside — renamed, not re-run"),
            );
            continue;
        }
        let ia = field_line(&text, "agent");
        if ia != an {
            say(
                out,
                bad,
                &format!("FAIL evidence {name} says agent {ia} inside — renamed"),
            );
            continue;
        }
        if !records(&text)
            .iter()
            .any(|line| line.starts_with("--- exit "))
        {
            say(
                out,
                bad,
                &format!("FAIL evidence {name} has no exit line (truncated)"),
            );
            continue;
        }
        if !ok_agents.iter().any(|seen| seen == an) {
            ok_agents.push(an.to_string());
        }
        en += 1;
    }
    if en == 0 {
        say(out, bad, &format!("FAIL no usable evidence for {wid}"));
    }
    Ok(())
}

fn skip_task_evidence(root: &Path, path: &Path) -> Result<bool, GuidedError> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let attempt = field_line(&text, "attempt-id");
    if attempt.is_empty() {
        return Ok(false);
    }
    let meta = root.join("attempts").join(&attempt).join("meta.tsv");
    if !meta.is_file() {
        return Ok(false);
    }
    let rows = fs::read_to_string(meta).unwrap_or_default();
    let row = records(&rows).get(1).copied().unwrap_or("");
    let task = split_tabs(row).get(2).copied().unwrap_or("");
    Ok(task != "-")
}

fn any_regular_file(dir: &Path) -> bool {
    fn walk(dir: &Path) -> bool {
        for path in read_dir_paths(dir) {
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_file() || (meta.is_dir() && walk(&path)) {
                return true;
            }
        }
        false
    }
    dir.is_dir() && walk(dir)
}

fn check_verdicts(
    root: &Path,
    slug: &str,
    dir: &Path,
    wid: &str,
    ok_agents: &[String],
    out: &mut String,
    bad: &mut bool,
) -> Result<(), GuidedError> {
    let verdicts = dir.join("verdicts");
    let mut paths = read_dir_paths(&verdicts);
    paths.retain(|path| !file_name(path).starts_with('.'));
    paths.sort();
    let mut bodies = Vec::new();
    let managed = uses_managed_lifecycle(root)?;
    let guided = uses_guided_cycle(root)?;
    for path in paths {
        let nm = file_name(&path);
        if nm == "history" {
            continue;
        }
        let Some(nm) = nm.strip_suffix(".md") else {
            say(
                out,
                bad,
                &format!("FAIL {nm}: only NAME.md belongs in verdicts/"),
            );
            continue;
        };
        let meta = fs::symlink_metadata(&path).ok();
        let Some(meta) = meta else {
            continue;
        };
        if meta.file_type().is_symlink() {
            say(out, bad, &format!("FAIL {nm} is a symlink"));
            continue;
        }
        if !meta.is_file() {
            say(out, bad, &format!("FAIL {nm} is not a regular file"));
            continue;
        }
        let kind = kind_of(root, nm)?;
        if kind.is_empty() {
            say(
                out,
                bad,
                &format!("FAIL {nm} is not registered in agents.tsv, so it is not a judge"),
            );
            continue;
        }
        if is_maker(root, slug, nm)? {
            say(
                out,
                bad,
                &format!("FAIL {nm} is a maker and may not judge its own work"),
            );
            continue;
        }
        if meta.len() == 0 {
            say(out, bad, &format!("FAIL empty verdict {nm}.md"));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let lines: Vec<&str> = records(&text);
        let l1 = lines.first().copied().unwrap_or("");
        if l1 != "VERDICT: PASS" {
            if matches!(
                l1,
                "VERDICT: REJECT" | "VERDICT: INSUFFICIENT_EVIDENCE" | "VERDICT: SCOPE_CONFLICT"
            ) {
                let word = l1.trim_start_matches("VERDICT: ");
                say(out, bad, &format!("FAIL {nm} returned {word}"));
            } else {
                say(
                    out,
                    bad,
                    &format!("FAIL {nm}: line 1 must be exactly 'VERDICT: <WORD>'"),
                );
            }
            continue;
        }
        let l2 = lines.get(1).copied().unwrap_or("");
        let vwid = l2.strip_prefix("WORK-ID: ").unwrap_or("");
        if !is_alnum(vwid) {
            say(
                out,
                bad,
                &format!("FAIL {nm}: line 2 must be exactly 'WORK-ID: <id>'"),
            );
            continue;
        }
        if vwid != wid {
            say(
                out,
                bad,
                &format!("FAIL {nm} judged {vwid} but work is {wid} (stale)"),
            );
            continue;
        }
        if managed
            && !managed_verdict_ok(
                root,
                slug,
                wid,
                nm,
                lines.get(2).copied().unwrap_or(""),
                out,
                bad,
                guided,
            )?
        {
            continue;
        }
        if !ok_agents.iter().any(|agent| agent == nm) {
            say(
                out,
                bad,
                &format!("FAIL {nm} passed but recorded no evidence of its own"),
            );
            continue;
        }
        if !cites_own_evidence(&dir.join("evidence"), nm, wid, &text) {
            say(
                out,
                bad,
                &format!("FAIL {nm} passed without naming any evidence file it recorded"),
            );
            continue;
        }
        let body = h12(tail_from_line(&text, 3));
        if bodies.iter().any(|seen| seen == &body) {
            say(
                out,
                bad,
                &format!("FAIL {nm} is byte-identical to another verdict"),
            );
            continue;
        }
        bodies.push(body);
        out.push_str(&format!(
            "  ok   {nm} PASS on {wid} (kind {kind}, recorded its own evidence)\n"
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn managed_verdict_ok(
    root: &Path,
    slug: &str,
    wid: &str,
    nm: &str,
    l3: &str,
    out: &mut String,
    bad: &mut bool,
    guided: bool,
) -> Result<bool, GuidedError> {
    if !(l3.starts_with("result A") && l3.contains("; see ")) {
        say(
            out,
            bad,
            &format!("FAIL {nm}: managed verdict is not bound to an attempt result"),
        );
        return Ok(false);
    }
    let after = l3.strip_prefix("result ").unwrap_or(l3);
    let result_id = after.split("; see ").next().unwrap_or("");
    let result_evidence = l3.split("; see ").nth(1).unwrap_or("");
    if result_id.is_empty() || !attempt_charset(result_id) {
        say(
            out,
            bad,
            &format!("FAIL {nm}: managed verdict has an invalid attempt id"),
        );
        return Ok(false);
    }
    let result_path = root.join("attempts").join(result_id).join("result.md");
    if !is_regular_file(&result_path) {
        say(
            out,
            bad,
            &format!("FAIL {nm}: managed attempt result {result_id} is missing"),
        );
        return Ok(false);
    }
    let result = fs::read_to_string(&result_path).unwrap_or_default();
    let agree = result_field(&result, "OUTCOME") == "PASS"
        && result_field(&result, "ITEM") == slug
        && result_field(&result, "WORK-ID") == wid
        && result_field(&result, "ATTEMPT-ID") == result_id
        && result_field(&result, "ROLE") == "judge"
        && result_field(&result, "AGENT") == nm
        && result_field(&result, "EVIDENCE") == result_evidence;
    if !agree {
        say(
            out,
            bad,
            &format!("FAIL {nm}: verdict and managed attempt result disagree"),
        );
        return Ok(false);
    }
    if guided && require_attempt_independence(root, result_id).is_err() {
        say(
            out,
            bad,
            &format!("FAIL {nm}: attempt {result_id} lacks independence seal under current panel"),
        );
        return Ok(false);
    }
    Ok(true)
}

fn cites_own_evidence(dir: &Path, nm: &str, wid: &str, verdict: &str) -> bool {
    let prefix = format!("{nm}.");
    let suffix = format!(".{wid}.txt");
    for path in read_dir_paths(dir) {
        if !path.is_file() {
            continue;
        }
        let name = file_name(&path);
        if name.starts_with(&prefix)
            && name.ends_with(&suffix)
            && name.len() >= prefix.len() + suffix.len()
            && verdict.contains(&name)
        {
            return true;
        }
    }
    false
}

fn tail_from_line(text: &str, line_no: usize) -> &[u8] {
    let bytes = text.as_bytes();
    if line_no <= 1 {
        return bytes;
    }
    let mut seen = 1usize;
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            seen += 1;
            if seen == line_no {
                return &bytes[i + 1..];
            }
        }
    }
    &[]
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_file())
        .unwrap_or(false)
}

fn is_alnum(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn is_token(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn attempt_charset(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'.' || b == b'A')
}

fn field_line(text: &str, key: &str) -> String {
    let prefix = format!("{key}: ");
    records(text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string()
}

fn split_tabs(rec: &str) -> Vec<&str> {
    if rec.is_empty() {
        Vec::new()
    } else {
        rec.split('\t').collect()
    }
}

fn guided_min_judges(root: &Path) -> Result<u64, GuidedError> {
    if let Some(raw) = nonempty_var("CRUCIBLE_MIN_JUDGES") {
        return parse_pos(&raw);
    }
    if uses_guided_cycle(root)? {
        let n = panel_required_count(root, "reviewer")?;
        if n >= 1 {
            return Ok(n as u64);
        }
    }
    Ok(2)
}

fn guided_min_judges_label(root: &Path) -> Result<String, GuidedError> {
    if let Some(raw) = nonempty_var("CRUCIBLE_MIN_JUDGES") {
        parse_pos(&raw)?;
        return Ok(raw);
    }
    Ok(guided_min_judges(root)?.to_string())
}

fn nonempty_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|raw| !raw.is_empty())
}

fn parse_pos(raw: &str) -> Result<u64, GuidedError> {
    if !is_posint(raw) {
        return Err(message("CRUCIBLE_MIN_JUDGES must be a positive integer"));
    }
    raw.parse::<u64>()
        .map_err(|_| message("CRUCIBLE_MIN_JUDGES must be a positive integer"))
}

/// `crucible close SLUG "lesson" [--successor SHA]`.
pub fn close(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    let slug = args.first().copied().unwrap_or("");
    let dir = need(root, slug)?;
    let (lesson, successor) = parse_close_args(&args[1..])?;
    if lesson.is_empty() {
        return Err(message(
            "usage: crucible close SLUG \"one-line lesson, or NONE\" [--successor SHA]",
        ));
    }
    if lesson.contains('\n') {
        return Err(message("lesson must be one line"));
    }
    if uses_managed_lifecycle(root)? {
        state_validate_file(&root.join("STATE.tsv"))?;
        if state_value(root, slug, 2)?.as_deref() == Some("CLOSED") {
            return Err(message(format!("{slug} is already closed")));
        }
    } else if item_status_closed(&dir) {
        return Err(message(format!("{slug} is already closed")));
    }
    let closing = dir.join(".closing");
    if fs::create_dir(&closing).is_err() {
        return Err(message(format!(
            "a close of {slug} is already in progress or finished"
        )));
    }
    let before = workid(root, slug)?;
    // `die` inside check is `exit`, so the shell never reaches the `||` rmdir.
    let report = check(root, &[slug])?;
    if report.status != 0 {
        release_closing(&dir);
        return Err(message(format!("refused: {slug} is not closeable")));
    }
    let after = workid(root, slug)?;
    if before != after {
        release_closing(&dir);
        return Err(message(format!(
            "refused: work changed during check ({before} -> {after})"
        )));
    }
    if dir.join("TARGET").is_file() {
        let repo = crate::dispatch::tgt(root, slug, "repo")?;
        let branch = crate::dispatch::tgt(root, slug, "branch")?;
        let head = git_rev12(&repo, &branch);
        if !head.is_empty() && after != head {
            let succ12 = first12(&successor);
            if successor.is_empty() || succ12 != head {
                release_closing(&dir);
                return Err(message(format!(
                    "refused: close work-id {after} is not {branch} HEAD {head} (new attempt or --successor SHA)"
                )));
            }
        }
    }
    let last_pass = last_pass_wid(&dir);
    if !last_pass.is_empty() && after != last_pass {
        let succ12 = first12(&successor);
        if successor.is_empty() || (succ12 != after && succ12 != last_pass) {
            release_closing(&dir);
            return Err(message(format!(
                "refused: close work-id {after} is not last PASS {last_pass} (record a successor review or --successor SHA)"
            )));
        }
    }
    let stale = stale_evidence(root, slug, &after);
    if !stale.is_empty() {
        release_closing(&dir);
        return Err(message(format!(
            "refused: stale evidence still in items/{slug}/evidence (run: evidence archive {slug}): {}",
            stale.join("\n")
        )));
    }
    if let Some(fp) = reject_fingerprint(root, slug) {
        if lesson == "NONE" {
            release_closing(&dir);
            return Err(message(format!(
                "refused: LESSONS.md cannot stay NONE when REJECT fingerprint {fp} exists"
            )));
        }
    }
    // Shell line 5974. UTC date of the clock, not the local `date` calendar.
    let stamped = format_rfc3339_z(clock.now_unix());
    let date = &stamped[..10];
    let mut lessons = OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("LESSONS.md"))?;
    writeln!(lessons, "- [{date}] {lesson} (item {slug}, work {after})")?;
    drop(lessons);
    if uses_managed_lifecycle(root)? {
        let risk = state_value(root, slug, 5)?.unwrap_or_default();
        state_update_item(
            root, clock, slug, "CLOSED", "REVIEW", &after, &risk, "-", "-",
        )?;
    } else {
        rewrite_status(&dir, &after)?;
    }
    close_claims(root, slug)?;
    Ok(format!("closed {slug} at {after}\n"))
}

fn parse_close_args(args: &[&str]) -> Result<(String, String), GuidedError> {
    let mut lesson = Vec::new();
    let mut successor = String::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--successor" {
            i += 1;
            if i >= args.len() || args[i].is_empty() {
                return Err(message(
                    "usage: crucible close SLUG \"lesson\" [--successor SHA]",
                ));
            }
            successor = args[i].to_string();
        } else {
            lesson.push(args[i]);
        }
        i += 1;
    }
    Ok((lesson.join(" "), successor))
}

fn item_status_closed(dir: &Path) -> bool {
    fs::read_to_string(dir.join("ITEM.md"))
        .map(|text| {
            records(&text)
                .iter()
                .any(|line| line.starts_with("STATUS: CLOSED"))
        })
        .unwrap_or(false)
}

fn release_closing(dir: &Path) {
    let _ = fs::remove_dir(dir.join(".closing"));
}

fn git_rev12(repo: &str, rev: &str) -> String {
    let out = std::process::Command::new("git")
        .args(["-C", repo, "rev-parse", "--verify", "--quiet", rev])
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(out) = out else {
        return String::new();
    };
    if !out.status.success() {
        return String::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(12)
        .collect()
}

fn first12(value: &str) -> String {
    value.chars().take(12).collect()
}

fn last_pass_wid(dir: &Path) -> String {
    let mut files = read_dir_paths(&dir.join("verdicts"));
    files.retain(|path| path.is_file() && file_name(path).ends_with(".md"));
    files.sort();
    let mut last = String::new();
    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if result_field(&text, "VERDICT") == "PASS" {
            last = result_field(&text, "WORK-ID");
        }
    }
    last
}

fn reject_fingerprint(root: &Path, slug: &str) -> Option<String> {
    let mut found = None;
    for path in attempt_child_dirs(root) {
        let result = path.join("result.md");
        if !result.is_file() {
            continue;
        }
        let Ok(text) = fs::read_to_string(&result) else {
            continue;
        };
        if result_field(&text, "ITEM") != slug || result_field(&text, "OUTCOME") != "REJECT" {
            continue;
        }
        let fp = result_field(&text, "FINDING-FINGERPRINT");
        if !fp.is_empty() && fp != "-" {
            found = Some(fp);
        }
    }
    found
}

fn rewrite_status(dir: &Path, wid: &str) -> Result<(), GuidedError> {
    let path = dir.join("ITEM.md");
    let text = fs::read_to_string(&path)?;
    let mut out = String::new();
    for line in records(&text) {
        if line.starts_with("STATUS: OPEN") {
            out.push_str(&format!("STATUS: CLOSED {wid}\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    let tmp = dir.join(format!("ITEM.{}.tmp", std::process::id()));
    fs::write(&tmp, out)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn close_claims(root: &Path, slug: &str) -> Result<(), GuidedError> {
    let path = root.join("CLAIMS.md");
    if !path.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&path)?;
    let tot = count_claim_headings(&text);
    for i in 1..=tot {
        if claim_field(&text, i, "item").as_deref() == Some(slug) {
            rewrite_claim_line(root, &format!("C{i}"), "status", "CLOSED")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::ENV_LOCK;
    use crate::inspect::run;
    use crucible_contract::FixedClock;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::MutexGuard;

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-close-{}-{}",
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

    struct Restore(&'static str, Option<std::ffi::OsString>);
    impl Drop for Restore {
        fn drop(&mut self) {
            // SAFETY: the test holds ENV_LOCK until this guard drops.
            unsafe {
                match self.1.take() {
                    Some(prev) => std::env::set_var(self.0, prev),
                    None => std::env::remove_var(self.0),
                }
            }
        }
    }

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn unset_env(key: &'static str) -> Restore {
        let prev = std::env::var_os(key);
        // SAFETY: caller holds ENV_LOCK.
        unsafe { std::env::remove_var(key) };
        Restore(key, prev)
    }

    const ITEM: &str = "\
# alpha — t

PHASE: SPEC
STATUS: OPEN

## The ask

Do the thing.

## Acceptance criteria

- [ ] A1

## Falsifier

Undo the change to src/widget.rs and confirm the focused check then fails loudly.
";

    #[test]
    fn close_lesson_date_is_utc_day_of_the_fixed_clock() {
        let _lock = env_lock();
        let _judges = unset_env("CRUCIBLE_MIN_JUDGES");
        let _kinds = unset_env("CRUCIBLE_MIN_KINDS");
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let dir = root.join("items/alpha");
        fs::create_dir_all(dir.join("work")).unwrap();
        fs::create_dir_all(dir.join("evidence")).unwrap();
        fs::create_dir_all(dir.join("verdicts")).unwrap();
        fs::create_dir_all(dir.join("briefs")).unwrap();
        fs::write(dir.join("work/note.txt"), "body\n").unwrap();
        fs::write(dir.join("MAKER"), "mk1\n").unwrap();
        fs::write(dir.join("ITEM.md"), ITEM).unwrap();
        fs::write(
            root.join("agents.tsv"),
            "mk1\tkindA\tm\thigh\ttrue\nj1\tkindB\tm\thigh\ttrue\nj2\tkindB\tm\thigh\ttrue\n",
        )
        .unwrap();
        // 0 is 1970-01-01 UTC and 1969-12-31 in US timezones. The pin is the UTC day.
        let clock = FixedClock::new(0);
        let wid = workid(root, "alpha").unwrap();
        let o1 = run(root, &["alpha", "j1", "--", "/bin/echo", "one"], &clock).unwrap();
        let o2 = run(root, &["alpha", "j2", "--", "/bin/echo", "two"], &clock).unwrap();
        let f1 = o1.split_whitespace().next().unwrap();
        let f2 = o2.split_whitespace().next().unwrap();
        let n1 = Path::new(f1).file_name().unwrap().to_string_lossy();
        let n2 = Path::new(f2).file_name().unwrap().to_string_lossy();
        fs::write(
            dir.join("verdicts/j1.md"),
            format!("VERDICT: PASS\nWORK-ID: {wid}\ncites {n1} from judge one\n"),
        )
        .unwrap();
        fs::write(
            dir.join("verdicts/j2.md"),
            format!("VERDICT: PASS\nWORK-ID: {wid}\ncites {n2} from judge two\n"),
        )
        .unwrap();
        let report = check(root, &["alpha"]).unwrap();
        assert_eq!(report.status, 0, "{}", report.text);
        assert!(
            report.text.ends_with(&format!("CLOSEABLE {wid}\n")),
            "{}",
            report.text
        );
        let closed = close(root, &["alpha", "learned it"], &clock).unwrap();
        assert_eq!(closed, format!("closed alpha at {wid}\n"));
        assert_eq!(
            fs::read_to_string(root.join("LESSONS.md")).unwrap(),
            format!("- [1970-01-01] learned it (item alpha, work {wid})\n")
        );
        assert!(fs::read_to_string(dir.join("ITEM.md"))
            .unwrap()
            .contains(&format!("STATUS: CLOSED {wid}\n")));
        assert_eq!(
            close(root, &["alpha", "again"], &clock)
                .unwrap_err()
                .to_string(),
            "alpha is already closed"
        );
    }

    #[test]
    fn empty_work_is_not_closeable_and_close_refuses() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::create_dir_all(root.join("items/alpha")).unwrap();
        fs::write(root.join("items/alpha/ITEM.md"), "# alpha\n").unwrap();
        let report = check(root, &["alpha"]).unwrap();
        assert_eq!(report.status, 1);
        assert!(report.text.contains("NOT CLOSEABLE\n"), "{}", report.text);
        let clock = FixedClock::new(0);
        assert_eq!(
            close(root, &["alpha", "nope"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: alpha is not closeable"
        );
        assert!(!root.join("items/alpha/.closing").exists());
    }
}
