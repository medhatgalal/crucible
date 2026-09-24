//! Panel, proposal, and agent-registry checks. Panel ids are SHA-256 (no `cksum` fallback).

use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::program::uses_guided_cycle;
use crate::{message, records, GuidedError};

const PROPOSAL_HEADINGS: [&str; 5] = [
    "## Verified problem",
    "## Proposed outcome",
    "## Non-goals",
    "## Backlog",
    "## Verification",
];

const PANEL_HEADINGS: [&str; 6] = [
    "## Agents",
    "## Roles",
    "## Risk posture",
    "## Isolation transport",
    "## Independence ladder",
    "## Waivers",
];

const KNOWN_ROLES: [&str; 12] = [
    "coordinator",
    "claim-auditor",
    "contract-auditor",
    "maker",
    "reviewer",
    "judge",
    "adversary",
    "scout",
    "specifier",
    "architect",
    "planner",
    "integrator",
];

pub(crate) fn is_regular(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_file())
        .unwrap_or(false)
}

pub(crate) fn is_nonempty_regular(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_file() && meta.len() > 0)
        .unwrap_or(false)
}

pub(crate) fn split_tabs(rec: &str) -> Vec<&str> {
    if rec.is_empty() {
        Vec::new()
    } else {
        rec.split('\t').collect()
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let dig = Sha256::digest(bytes);
    let mut s = String::with_capacity(dig.len() * 2);
    for byte in dig {
        s.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        s.push(char::from(b"0123456789abcdef"[usize::from(byte & 0xf)]));
    }
    s
}

pub(crate) fn hash_file(path: &Path) -> Result<String, GuidedError> {
    Ok(sha256_hex(&fs::read(path)?))
}

/// First 12 hex chars of SHA-256 over `bytes`, matching `h12`.
pub(crate) fn h12(bytes: &[u8]) -> String {
    sha256_hex(bytes).chars().take(12).collect()
}

pub(crate) fn is_posint(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|b| b.is_ascii_digit())
        && value != "0"
        && !value.bytes().all(|b| b == b'0')
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

pub(crate) fn contains_word_ci(hay: &str, word: &str) -> bool {
    let h = hay.as_bytes();
    let w = word.as_bytes();
    if w.is_empty() || h.len() < w.len() {
        return false;
    }
    for i in 0..=h.len() - w.len() {
        if !h[i..i + w.len()].eq_ignore_ascii_case(w) {
            continue;
        }
        let before_ok = i == 0 || !is_word_byte(h[i - 1]);
        let after = i + w.len();
        let after_ok = after == h.len() || !is_word_byte(h[after]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

pub(crate) fn contains_ci(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let h = hay.as_bytes();
    let n = needle.as_bytes();
    h.len() >= n.len() && (0..=h.len() - n.len()).any(|i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

fn is_blank(line: &str) -> bool {
    line.bytes().all(|b| b.is_ascii_whitespace())
}

/// `agents.tsv` column `c` (1-based) for `name`. Missing file or row is empty.
pub(crate) fn agent_col(root: &Path, name: &str, column: usize) -> Result<String, GuidedError> {
    let path = root.join("agents.tsv");
    if !path.is_file() {
        return Ok(String::new());
    }
    let text = fs::read_to_string(path)?;
    for line in records(&text) {
        if line.starts_with('#') {
            continue;
        }
        let fields = split_tabs(line);
        if fields.first().copied() == Some(name) {
            return Ok(fields.get(column - 1).copied().unwrap_or("").to_string());
        }
    }
    Ok(String::new())
}

pub(crate) fn kind_of(root: &Path, name: &str) -> Result<String, GuidedError> {
    agent_col(root, name, 2)
}

pub(crate) fn agents_registry_ready(root: &Path) -> Result<bool, GuidedError> {
    let path = root.join("agents.tsv");
    if !is_regular(&path) {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    let mut rows = 0usize;
    let mut placeholder = false;
    for line in records(&text) {
        if line.starts_with('#') || is_blank(line) {
            continue;
        }
        rows += 1;
        let fields = split_tabs(line);
        if fields.get(2).copied() == Some("MODEL") {
            placeholder = true;
        }
        if fields
            .get(4)
            .is_some_and(|cmd| cmd.contains("AGENT_CLI") || cmd.contains("OTHER_CLI"))
        {
            placeholder = true;
        }
    }
    Ok(rows >= 2 && !placeholder)
}

fn assign_role_known(role: &str) -> bool {
    KNOWN_ROLES.contains(&role)
}

fn assign_role_normalize(role: &str) -> &str {
    if role == "judge" {
        "reviewer"
    } else {
        role
    }
}

fn listed(names: &[String], agent: &str) -> bool {
    names.iter().any(|n| n == agent)
}

/// `required=yes` rows for `want`, after `judge` → `reviewer`. Missing file is 0.
pub(crate) fn panel_required_count(root: &Path, want: &str) -> Result<usize, GuidedError> {
    let path = root.join("PANEL.ASSIGN.tsv");
    if !path.is_file() {
        return Ok(0);
    }
    let text = fs::read_to_string(path)?;
    let mut n = 0usize;
    for line in records(&text) {
        let fields = split_tabs(line);
        let raw = fields.first().copied().unwrap_or("");
        if raw.starts_with('#') || raw == "role" {
            continue;
        }
        let role = if raw == "judge" { "reviewer" } else { raw };
        let req = fields.get(2).copied().unwrap_or("").to_ascii_lowercase();
        if role == want && (req == "yes" || req == "required") {
            n += 1;
        }
    }
    Ok(n)
}

const AUDITORS_DIE: &str = "CRUCIBLE_MIN_AUDITORS must be a positive integer";
const KINDS_DIE: &str = "CRUCIBLE_MIN_KINDS must be a positive integer";

/// Admit-bar floor: env override, else max(2, required claim-auditor rows) on a guided cycle.
pub(crate) fn guided_min_auditors(root: &Path) -> Result<u64, GuidedError> {
    if let Some(raw) = nonempty_var("CRUCIBLE_MIN_AUDITORS") {
        return parse_pos_override(&raw, AUDITORS_DIE);
    }
    if uses_guided_cycle(root)? {
        let n = panel_required_count(root, "claim-auditor")?;
        if n > 2 {
            return Ok(n as u64);
        }
    }
    Ok(2)
}

/// Shell prints the raw override (`03` stays `03`). The numeric bar still uses [`guided_min_auditors`].
pub(crate) fn guided_min_auditors_label(root: &Path) -> Result<String, GuidedError> {
    if let Some(raw) = nonempty_var("CRUCIBLE_MIN_AUDITORS") {
        parse_pos_override(&raw, AUDITORS_DIE)?;
        return Ok(raw);
    }
    Ok(guided_min_auditors(root)?.to_string())
}

pub(crate) fn min_kinds() -> Result<u64, GuidedError> {
    if let Some(raw) = nonempty_var("CRUCIBLE_MIN_KINDS") {
        return parse_pos_override(&raw, KINDS_DIE);
    }
    Ok(1)
}

fn nonempty_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|raw| !raw.is_empty())
}

/// `posint` plus a parse into the integer the bar compares. Too big is a refusal, not a default.
fn parse_pos_override(raw: &str, die: &str) -> Result<u64, GuidedError> {
    if !is_posint(raw) {
        return Err(message(die));
    }
    raw.parse::<u64>().map_err(|_| message(die))
}

fn maker_reviewer_waiver(root: &Path) -> Result<bool, GuidedError> {
    let path = root.join("PANEL.md");
    if !path.is_file() {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    Ok(records(&text).into_iter().any(waiver_line))
}

fn waiver_line(line: &str) -> bool {
    let trimmed = line.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let lower = trimmed.to_ascii_lowercase();
    let rest = if let Some(rest) = lower.strip_prefix("ladder_waiver:") {
        rest
    } else if let Some(rest) = lower.strip_prefix("waiver:") {
        rest
    } else {
        return false;
    };
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let token = "maker-reviewer-same-agent";
    if !rest.starts_with(token) {
        return false;
    }
    rest.as_bytes()
        .get(token.len())
        .is_none_or(|b| !is_word_byte(*b))
}

fn panel_mentions_risk(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("risk posture") || lower.contains("## risk")
}

fn risk_posture_window_has_high(text: &str) -> bool {
    let lines = records(text);
    for (i, line) in lines.iter().enumerate() {
        if !line.starts_with("## Risk posture") {
            continue;
        }
        let end = (i + 6).min(lines.len());
        let window = lines[i..end].join("\n");
        if contains_word_ci(&window, "HIGH") {
            return true;
        }
    }
    false
}

fn assign_has_role(text: &str, role: &str) -> bool {
    records(text).into_iter().any(|line| {
        if line.starts_with('#') {
            return false;
        }
        split_tabs(line).first().copied() == Some(role)
    })
}

pub(crate) fn assign_valid(root: &Path) -> Result<bool, GuidedError> {
    let path = root.join("PANEL.ASSIGN.tsv");
    if !is_nonempty_regular(&path) || !agents_registry_ready(root)? {
        return Ok(false);
    }
    let text = fs::read_to_string(&path)?;
    let mut has_coord = false;
    let mut has_claim = false;
    let mut has_maker = false;
    let mut has_reviewer = false;
    let mut has_caudit = false;
    let mut makers = Vec::new();
    let mut reviewers = Vec::new();
    let mut coords = Vec::new();
    let mut claim_auditors = Vec::new();
    let mut contract_auditors = Vec::new();
    for line in records(&text) {
        let fields = split_tabs(line);
        let role = fields.first().copied().unwrap_or("");
        if role.is_empty() || role.starts_with('#') || role == "role" {
            continue;
        }
        if !assign_role_known(role) {
            return Ok(false);
        }
        let agent = fields.get(1).copied().unwrap_or("");
        if agent.is_empty() || kind_of(root, agent)?.is_empty() {
            return Ok(false);
        }
        let required = fields.get(2).copied().unwrap_or("");
        if !matches!(required, "yes" | "no" | "") {
            return Ok(false);
        }
        match assign_role_normalize(role) {
            "coordinator" => {
                has_coord = true;
                coords.push(agent.to_string());
            }
            "claim-auditor" => {
                has_claim = true;
                claim_auditors.push(agent.to_string());
            }
            "maker" => {
                has_maker = true;
                makers.push(agent.to_string());
            }
            "reviewer" => {
                has_reviewer = true;
                reviewers.push(agent.to_string());
            }
            "contract-auditor" => {
                has_caudit = true;
                contract_auditors.push(agent.to_string());
            }
            _ => {}
        }
    }
    if !(has_coord && has_claim && has_maker && has_reviewer && has_caudit) {
        return Ok(false);
    }
    let waived = maker_reviewer_waiver(root)?;
    if !waived {
        for maker in &makers {
            if listed(&reviewers, maker) {
                return Ok(false);
            }
        }
        for coord in &coords {
            if listed(&makers, coord) || listed(&reviewers, coord) {
                return Ok(false);
            }
        }
    }
    for coord in &coords {
        if listed(&claim_auditors, coord) || listed(&contract_auditors, coord) {
            return Ok(false);
        }
    }
    for auditor in &contract_auditors {
        if listed(&makers, auditor) {
            return Ok(false);
        }
    }
    let panel = root.join("PANEL.md");
    if panel.is_file() {
        let body = fs::read_to_string(&panel)?;
        if panel_mentions_risk(&body)
            && risk_posture_window_has_high(&body)
            && !assign_has_role(&text, "adversary")
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn proposal_valid(root: &Path) -> Result<bool, GuidedError> {
    let path = root.join("PROPOSAL.md");
    if !is_nonempty_regular(&path) {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    Ok(PROPOSAL_HEADINGS.iter().all(|h| text.contains(h))
        && !text.contains("TEMPLATE-")
        && !text.contains("TODO-REPLACE"))
}

pub fn proposal_id(root: &Path) -> Result<Option<String>, GuidedError> {
    let path = root.join("PROPOSAL.md");
    if !is_nonempty_regular(&path) {
        return Ok(None);
    }
    let hex = hash_file(&path)?;
    Ok(Some(hex.chars().take(12).collect()))
}

pub(crate) fn approval_current(root: &Path) -> Result<bool, GuidedError> {
    let path = root.join("APPROVAL");
    if !is_regular(&path) {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    let approved = records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix("proposal-id: "))
        .unwrap_or("");
    if approved.is_empty() {
        return Ok(false);
    }
    Ok(proposal_id(root)?.as_deref() == Some(approved))
}

/// Content-bind PANEL.md, PANEL.ASSIGN.tsv, and agents.tsv. `None` when any input is unusable.
pub fn panel_id(root: &Path) -> Result<Option<String>, GuidedError> {
    let paths = [
        root.join("PANEL.md"),
        root.join("PANEL.ASSIGN.tsv"),
        root.join("agents.tsv"),
    ];
    if paths.iter().any(|path| !is_nonempty_regular(path)) {
        return Ok(None);
    }
    let mut blob = String::new();
    for path in &paths {
        blob.push_str(&hash_file(path)?);
        blob.push('\n');
    }
    Ok(Some(h12(blob.as_bytes())))
}

pub fn panel_valid(root: &Path) -> Result<bool, GuidedError> {
    let path = root.join("PANEL.md");
    if !is_nonempty_regular(&path) {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    if PANEL_HEADINGS.iter().any(|h| !text.contains(h)) {
        return Ok(false);
    }
    if text.contains("TEMPLATE-") || text.contains("TODO-REPLACE") {
        return Ok(false);
    }
    if !contains_ci(&text, "multi-agent")
        && !contains_ci(&text, "acp")
        && !contains_ci(&text, "subagent")
    {
        return Ok(false);
    }
    if !agents_registry_ready(root)? || !assign_valid(root)? {
        return Ok(false);
    }
    Ok(true)
}

pub fn panel_approval_current(root: &Path) -> Result<bool, GuidedError> {
    let path = root.join("PANEL.APPROVAL");
    if !is_regular(&path) {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    let approved = records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix("panel-id: "))
        .unwrap_or("");
    if approved.is_empty() {
        return Ok(false);
    }
    Ok(panel_id(root)?.as_deref() == Some(approved))
}

/// First recorded transport token in `text` (`multi-agent`, `acp`, or `subagent`).
pub(crate) fn first_transport_token(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    let mut best: Option<(usize, &'static str)> = None;
    for token in ["multi-agent", "acp", "subagent"] {
        if let Some(at) = lower.find(token) {
            if best.map(|(prev, _)| at < prev).unwrap_or(true) {
                best = Some((at, token));
            }
        }
    }
    best.map(|(_, token)| token)
}

/// `true` when subagent is still honest under the recorded ACP probe.
pub(crate) fn acp_probe_failed(root: &Path) -> Result<bool, GuidedError> {
    let probes = root.join("acp-probes");
    if probes.is_dir() {
        if let Ok(rd) = fs::read_dir(&probes) {
            for ent in rd.flatten() {
                let path = ent.path();
                if path.extension().is_none_or(|ext| ext != "md") || !path.is_file() {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                if records(&text).contains(&"status: ok") {
                    return Ok(false);
                }
            }
        }
    }
    let probe = root.join("ACP-PROBE.md");
    if is_regular(&probe) {
        let text = fs::read_to_string(&probe)?;
        if let Some(prev) = records(&text)
            .into_iter()
            .find_map(|line| line.strip_prefix("status: "))
        {
            match prev.to_ascii_lowercase().as_str() {
                "ok" => return Ok(false),
                "failed" | "unavailable" | "error" => return Ok(true),
                _ => {}
            }
        }
    }
    let panel = root.join("PANEL.md");
    if panel.is_file() {
        let text = fs::read_to_string(panel)?;
        if records(&text).into_iter().any(panel_acp_down) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn panel_acp_down(line: &str) -> bool {
    let trimmed = line.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let lower = trimmed.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("acp:") else {
        return false;
    };
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
    for word in ["failed", "unavailable"] {
        if rest.starts_with(word)
            && rest
                .as_bytes()
                .get(word.len())
                .is_none_or(|b| !is_word_byte(*b))
        {
            return true;
        }
    }
    false
}

/// 0 honest, 1 subagent without a recorded ACP failure, 2 not a transport.
pub(crate) fn transport_ladder_ok(root: &Path, transport: &str) -> Result<u8, GuidedError> {
    match transport {
        "multi-agent" | "acp" => Ok(0),
        "subagent" => Ok(if acp_probe_failed(root)? { 0 } else { 1 }),
        _ => Ok(2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_well_known_digests() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(h12(b""), "e3b0c44298fc");
        assert!(is_posint("1"));
        assert!(is_posint("03"));
        assert!(!is_posint("0"));
        assert!(!is_posint(""));
        assert!(!is_posint("12a"));
    }
}
