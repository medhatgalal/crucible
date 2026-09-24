//! Contract audit, plan audit, and the append-only ACP probe log.
//!
//! `probed-epoch` is [`Clock::now_unix`]. Not `SystemTime`.

use std::fs;
use std::path::Path;

use crucible_contract::Clock;

use crate::attempt::{enforce_transport_ladder, result_field};
use crate::claims::require_panel_approval;
use crate::cycle::{
    attempt_dir, attempt_event, attempt_meta, attempt_state, attempt_transport, is_claim_slug,
};
use crate::dispatch::{is_maker, need, require_registered, require_role_cast};
use crate::panel::{h12, is_regular, kind_of};
use crate::program::uses_managed_lifecycle;
use crate::state::{state_update_item, state_value};
use crate::{message, records, GuidedError};

/// `crucible contract-audit`, including `PASS --like`.
pub fn contract_audit(
    root: &Path,
    args: &[&str],
    clock: &dyn Clock,
) -> Result<String, GuidedError> {
    if !uses_managed_lifecycle(root)? {
        return Err(message(
            "contract-audit requires managed lifecycle behavior",
        ));
    }
    if args.len() < 3 {
        return Err(message(
            "usage: crucible contract-audit ATTEMPT AUDITOR PASS|FIX|STOP [note...] | PASS --like ATTEMPT2...",
        ));
    }
    let id = args[0];
    let auditor = args[1];
    let verdict = args[2];
    if !matches!(verdict, "PASS" | "FIX" | "STOP") {
        return Err(message("contract-audit verdict must be PASS, FIX, or STOP"));
    }
    require_registered(root, auditor)?;
    let rest = &args[3..];
    let (likes, note) = if rest.first().copied() == Some("--like") {
        if verdict != "PASS" {
            return Err(message("refused: --like requires PASS"));
        }
        let likes = like_ids(&rest[1..]);
        if likes.is_empty() {
            return Err(message(
                "usage: crucible contract-audit ATTEMPT AUDITOR PASS --like ATTEMPT2...",
            ));
        }
        (likes, "NONE".to_string())
    } else if rest.is_empty() {
        (Vec::new(), "NONE".to_string())
    } else {
        (Vec::new(), rest.join(" "))
    };
    let ad = attempt_dir(root, id)?;
    if !ad.join("contract.md").is_file() {
        return Err(message(format!("attempt has no contract.md: {id}")));
    }
    if attempt_state(root, id)? != "DISPATCHED" {
        return Err(message(
            "refused: contract-audit may only be recorded while DISPATCHED (before attempt start)",
        ));
    }
    require_panel_approval(root)?;
    let attempt_agent = attempt_meta(root, id, 6)?;
    let role = attempt_meta(root, id, 5)?;
    let slug = attempt_meta(root, id, 2)?;
    if auditor == attempt_agent {
        return Err(message(format!(
            "refused: contract-auditor {auditor} cannot audit its own attempt agent"
        )));
    }
    require_role_cast(root, "contract-auditor", auditor)?;
    if matches!(role.as_str(), "judge" | "adversary") && is_maker(root, &slug, auditor)? {
        return Err(message(format!(
            "refused: maker {auditor} cannot contract-audit review of {slug}"
        )));
    }
    let transport = attempt_transport(root, id)?.unwrap_or_else(|| "-".to_string());
    let mut independence = "ok".to_string();
    match verdict {
        "PASS" => {
            if transport == "-" {
                return Err(message(
                    "refused: record transport before contract-audit PASS",
                ));
            }
            enforce_transport_ladder(root, &transport)?;
            if !contract_structural_ok(&ad, &role)? {
                return Err(message(
                    "refused: contract.md fails structural checks (brief-only instruction / role sections)",
                ));
            }
            independence = match transport.as_str() {
                "multi-agent" => "ok".to_string(),
                "acp" | "subagent" => "weak".to_string(),
                _ => "ok".to_string(),
            };
        }
        "STOP" => independence = "unavailable".to_string(),
        "FIX" => independence = "weak".to_string(),
        _ => {}
    }
    let audit_path = ad.join("contract-audit.md");
    if audit_path.exists() {
        if !is_regular(&audit_path) {
            return Err(message(format!("contract-audit is immutable: {id}")));
        }
        let text = fs::read_to_string(&audit_path)?;
        let existing = result_field(&text, "VERDICT");
        let existing_auditor = result_field(&text, "AUDITOR");
        if existing == verdict && existing_auditor == auditor {
            let mut out = format!(
                "{}/contract-audit.md (already {verdict} by {auditor})\n",
                ad.display()
            );
            if !likes.is_empty() {
                let role = attempt_meta(root, id, 5)?;
                out.push_str(&copy_likes(root, id, &role, &likes)?);
            }
            return Ok(out);
        }
        return Err(message(format!(
            "contract-audit is immutable: {id} (existing {existing} by {existing_auditor})"
        )));
    }
    let (failures, required_fix) = match verdict {
        "PASS" => ("none", "none"),
        "FIX" => (note.as_str(), note.as_str()),
        _ => (note.as_str(), "none"),
    };
    let mut body = format!(
        "VERDICT: {verdict}\nAUDITOR: {auditor}\nAUDITOR-KIND: {}\nTRANSPORT: {transport}\nINDEPENDENCE: {independence}\nFAILURES: {failures}\nREQUIRED_FIX: {required_fix}\n\nFile-based contract audit for attempt {id} by {auditor}.\n",
        kind_of(root, auditor)?
    );
    if verdict == "PASS" && note != "NONE" {
        body.push_str(&format!("NOTE: {note}\n"));
    }
    let tmp = ad.join(format!(".contract-audit.{}.tmp", std::process::id()));
    fs::write(&tmp, body)?;
    fs::rename(&tmp, &audit_path)?;
    if verdict == "STOP" {
        if root.join("STATE.tsv").is_file() && !is_claim_slug(&slug) {
            let stage = state_value(root, &slug, 3)?.unwrap_or_default();
            let risk = state_value(root, &slug, 5)?.unwrap_or_default();
            let wid = state_value(root, &slug, 4)?.unwrap_or_default();
            state_update_item(
                root,
                clock,
                &slug,
                "BLOCKED",
                &stage,
                &wid,
                &risk,
                "-",
                "INDEPENDENCE_UNAVAILABLE",
            )?;
        }
        return Ok(format!(
            "{}/contract-audit.md\nESCALATE INDEPENDENCE_UNAVAILABLE — do not continue as solo theatre\n",
            ad.display()
        ));
    }
    if verdict == "FIX" {
        // A revised redispatch needs the live attempt out of the way. This one is not re-audited.
        attempt_event(root, clock, id, "SUPERSEDED", "-", "contract-audit-FIX")?;
        if root.join("STATE.tsv").is_file() && !is_claim_slug(&slug) {
            let stage = state_value(root, &slug, 3)?.unwrap_or_default();
            let risk = state_value(root, &slug, 5)?.unwrap_or_default();
            let wid = state_value(root, &slug, 4)?.unwrap_or_default();
            let inflight = state_value(root, &slug, 6)?.unwrap_or_default();
            if inflight == id {
                state_update_item(root, clock, &slug, "ACTIVE", &stage, &wid, &risk, "-", "-")?;
            }
        }
        return Ok(format!(
            "{}/contract-audit.md\nFIX: revise the contract and redispatch a new attempt (this attempt is SUPERSEDED)\n",
            ad.display()
        ));
    }
    let mut out = format!("{}/contract-audit.md\n", ad.display());
    if !likes.is_empty() {
        out.push_str(&copy_likes(root, id, &role, &likes)?);
    }
    Ok(out)
}

fn like_ids(args: &[&str]) -> Vec<String> {
    args.iter()
        .flat_map(|word| word.split_whitespace().map(str::to_string))
        .collect()
}

fn copy_likes(root: &Path, id: &str, role: &str, likes: &[String]) -> Result<String, GuidedError> {
    let mut out = String::new();
    for like_id in likes {
        match contract_audit_copy_like(root, id, like_id)? {
            Some(line) => out.push_str(&line),
            None => {
                return Err(message(format!(
                    "refused: {like_id} is not an isomorphic DISPATCHED {role} contract of {id}"
                )));
            }
        }
    }
    Ok(out)
}

fn contract_audit_copy_like(
    root: &Path,
    template: &str,
    target: &str,
) -> Result<Option<String>, GuidedError> {
    let tad = root.join("attempts").join(template);
    let audit = tad.join("contract-audit.md");
    if !audit.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&audit)?;
    if !records(&text)
        .iter()
        .any(|line| line.starts_with("VERDICT: PASS"))
    {
        return Ok(None);
    }
    let dad = attempt_dir(root, target)?;
    if dad.join("contract-audit.md").exists() {
        return Ok(None);
    }
    if attempt_state(root, target)? != "DISPATCHED" {
        return Ok(None);
    }
    if !tad.join("contract.md").is_file() || !dad.join("contract.md").is_file() {
        return Ok(None);
    }
    if attempt_meta(root, template, 5)? != attempt_meta(root, target, 5)? {
        return Ok(None);
    }
    if !contracts_isomorphic(&tad.join("contract.md"), &dad.join("contract.md"))? {
        return Ok(None);
    }
    let copied = sed_replace_dots(&text, template, target);
    let tmp = dad.join(format!(".contract-audit.{}.tmp", std::process::id()));
    fs::write(&tmp, copied)?;
    let dest = dad.join("contract-audit.md");
    fs::rename(&tmp, &dest)?;
    Ok(Some(format!(
        "{}/contract-audit.md (like {template})\n",
        dad.display()
    )))
}

fn contracts_isomorphic(a: &Path, b: &Path) -> Result<bool, GuidedError> {
    let left = h12(contract_normalize(&fs::read_to_string(a)?).as_bytes());
    let right = h12(contract_normalize(&fs::read_to_string(b)?).as_bytes());
    Ok(!left.is_empty() && left == right)
}

fn contract_normalize(text: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    for line in records(text) {
        if line.starts_with("## The claim") {
            skip = true;
            out.push_str("## The claim\nCN\n");
            continue;
        }
        if skip && line.starts_with("## ") {
            skip = false;
        }
        if skip || line.starts_with("attempt-id:") {
            continue;
        }
        let line = replace_attempt_tokens(&replace_c_ids(line));
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn replace_c_ids(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == 'C' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 {
                out.push_str("CN");
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn replace_attempt_tokens(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let digit_or_dot = |c: char| c.is_ascii_digit() || c == '.';
        if chars[i] == 'A'
            && i + 2 < chars.len()
            && chars[i + 1].is_ascii_digit()
            && digit_or_dot(chars[i + 2])
        {
            let mut j = i + 3;
            while j < chars.len() && digit_or_dot(chars[j]) {
                j += 1;
            }
            out.push_str("ATTEMPT");
            i = j;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// POSIX sed `s/pattern/repl/g` where `.` in `pattern` is any character. Attempt ids only use that.
fn sed_replace_dots(text: &str, pattern: &str, replacement: &str) -> String {
    let pat: Vec<char> = pattern.chars().collect();
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(len) = match_sed(&chars[i..], &pat) {
            out.push_str(replacement);
            i += len;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn match_sed(text: &[char], pat: &[char]) -> Option<usize> {
    if pat.is_empty() {
        return None;
    }
    let mut ti = 0;
    for &pc in pat {
        if pc == '.' {
            if ti >= text.len() {
                return None;
            }
            ti += 1;
        } else if ti >= text.len() || text[ti] != pc {
            return None;
        } else {
            ti += 1;
        }
    }
    Some(ti)
}

fn contract_structural_ok(ad: &Path, role: &str) -> Result<bool, GuidedError> {
    let path = ad.join("contract.md");
    if !path.is_file() {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    if !contains_ci(&text, "follow it exactly") {
        return Ok(false);
    }
    let lines = records(&text);
    match role {
        "judge" | "adversary" => {
            if !text.contains("This attempt is bound") {
                return Ok(false);
            }
            if lines.iter().any(|line| line.starts_with("## Maker report")) {
                return Ok(false);
            }
            Ok(lines
                .iter()
                .any(|line| line.starts_with("## Review independence")))
        }
        "maker" => Ok(text.contains("This attempt is bound")
            && lines
                .iter()
                .any(|line| line.starts_with("## Work location") || line.starts_with("## Item"))),
        "claim-auditor" | "scout" => Ok(text.contains("claim") || text.contains("Claim")),
        _ => Ok(true),
    }
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

/// `crucible plan-audit`.
pub fn plan_audit(root: &Path, args: &[&str], _clock: &dyn Clock) -> Result<String, GuidedError> {
    if !uses_managed_lifecycle(root)? {
        return Err(message("plan-audit requires managed lifecycle behavior"));
    }
    if args.len() < 3 {
        return Err(message(
            "usage: crucible plan-audit SLUG AUDITOR PASS|FIX|STOP",
        ));
    }
    let slug = args[0];
    let auditor = args[1];
    let verdict = args[2];
    if !matches!(verdict, "PASS" | "FIX" | "STOP") {
        return Err(message("plan-audit verdict must be PASS, FIX, or STOP"));
    }
    let d = need(root, slug)?;
    require_registered(root, auditor)?;
    if is_maker(root, slug, auditor)? {
        return Err(message(format!(
            "refused: {auditor} is a maker of {slug} — plan-audit must be independent"
        )));
    }
    let path = d.join("plan-audit.md");
    if path.exists() && !is_regular(&path) {
        return Err(message("plan-audit.md must be a regular file"));
    }
    if path.is_file() && is_regular(&path) {
        let existing = result_field(&fs::read_to_string(&path)?, "VERDICT");
        if existing == verdict {
            return Ok(format!(
                "{}/plan-audit.md (already {verdict})\n",
                d.display()
            ));
        }
        return Err(message(format!(
            "refused: plan-audit.md is immutable (existing {existing})"
        )));
    }
    let body = format!(
        "VERDICT: {verdict}\nAUDITOR: {auditor}\nAUDITOR-KIND: {}\nITEM: {slug}\n\nPlan-audit of ITEM.md for {slug} by {auditor}.\n",
        kind_of(root, auditor)?
    );
    fs::write(&path, body)?;
    Ok(format!("{}/plan-audit.md\n", d.display()))
}

/// `crucible probe-acp`. `probed-epoch` is the injected clock.
pub fn probe_acp(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    if args.is_empty() {
        return Err(message(
            "usage: crucible probe-acp ok|failed|unavailable [note...]",
        ));
    }
    let status = args[0];
    if !matches!(status, "ok" | "failed" | "unavailable") {
        return Err(message(
            "probe-acp status must be ok, failed, or unavailable",
        ));
    }
    let note = if args.len() == 1 {
        "none".to_string()
    } else {
        args[1..].join(" ")
    };
    let now = clock.now_unix();
    let dir = root.join("acp-probes");
    fs::create_dir_all(&dir)?;
    let summary = root.join("ACP-PROBE.md");
    if is_regular(&summary) {
        let prev = result_field(&fs::read_to_string(&summary)?, "status");
        if prev.eq_ignore_ascii_case("ok") && status != "ok" {
            return Err(message(format!(
                "refused: ACP probe already ok; cannot downgrade to {status} to unlock weaker isolation (record PANEL ACP-unavailable if needed)"
            )));
        }
    }
    let mut n = 1u64;
    while dir.join(format!("{n}.md")).exists() {
        n += 1;
    }
    fs::write(
        dir.join(format!("{n}.md")),
        format!("status: {status}\nprobed-epoch: {now}\nnote: {note}\n\nImmutable ACP probe event {n}.\n"),
    )?;
    let tmp = root.join(format!(".ACP-PROBE.md.{}.tmp", std::process::id()));
    fs::write(
        &tmp,
        format!(
            "status: {status}\nprobed-epoch: {now}\nnote: {note}\nevent: acp-probes/{n}.md\n\nLatest ACP probe summary. History under acp-probes/.\n"
        ),
    )?;
    fs::rename(&tmp, &summary)?;
    Ok(format!("{}/ACP-PROBE.md\n", root.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::STATE_HEADER;
    use crucible_contract::FixedClock;
    use std::sync::atomic::{AtomicU64, Ordering};

    const EPOCH: i64 = 1_700_000_042;

    struct Tmp(std::path::PathBuf);

    impl Tmp {
        fn new() -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "crucible-audit-{}-{}",
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
    fn fixed_clock_predicts_the_probe_epoch_and_refuses_downgrade() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let clock = FixedClock::new(EPOCH);
        let out = probe_acp(root, &["failed", "no", "broker"], &clock).unwrap();
        assert_eq!(out, format!("{}/ACP-PROBE.md\n", root.display()));
        let summary = fs::read_to_string(root.join("ACP-PROBE.md")).unwrap();
        assert!(summary.starts_with(&format!(
            "status: failed\nprobed-epoch: {EPOCH}\nnote: no broker\n"
        )));
        let event = fs::read_to_string(root.join("acp-probes/1.md")).unwrap();
        assert!(event.contains(&format!("probed-epoch: {EPOCH}\n")));
        assert!(event.contains("Immutable ACP probe event 1.\n"));
        probe_acp(root, &["ok"], &clock).unwrap();
        assert_eq!(
            probe_acp(root, &["failed"], &clock).unwrap_err().to_string(),
            "refused: ACP probe already ok; cannot downgrade to failed to unlock weaker isolation (record PANEL ACP-unavailable if needed)"
        );
        let kept = fs::read_to_string(root.join("acp-probes/2.md")).unwrap();
        assert!(kept.contains("status: ok\n"));
        assert!(!root.join("acp-probes/3.md").exists());
    }

    #[test]
    fn plan_audit_is_immutable_and_refuses_the_maker() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(root.join("STATE.tsv"), format!("{STATE_HEADER}\n")).unwrap();
        fs::write(root.join("agents.tsv"), "ada\tgrok\tm\th\techo {BRIEF}\n").unwrap();
        let item = root.join("items/alpha");
        fs::create_dir_all(&item).unwrap();
        fs::write(item.join("MAKER"), "ada\n").unwrap();
        assert_eq!(
            plan_audit(root, &["alpha", "ada", "PASS"], &FixedClock::new(1))
                .unwrap_err()
                .to_string(),
            "refused: ada is a maker of alpha — plan-audit must be independent"
        );
        fs::write(
            root.join("agents.tsv"),
            "ada\tgrok\nm\th\techo\nbea\tcodex\nm\th\techo\n",
        )
        .unwrap();
        let out = plan_audit(root, &["alpha", "bea", "PASS"], &FixedClock::new(1)).unwrap();
        assert_eq!(out, format!("{}/plan-audit.md\n", item.display()));
        assert_eq!(
            plan_audit(root, &["alpha", "bea", "PASS"], &FixedClock::new(1)).unwrap(),
            format!("{}/plan-audit.md (already PASS)\n", item.display())
        );
        assert_eq!(
            plan_audit(root, &["alpha", "bea", "FIX"], &FixedClock::new(1))
                .unwrap_err()
                .to_string(),
            "refused: plan-audit.md is immutable (existing PASS)"
        );
    }

    #[test]
    fn like_copies_an_isomorphic_dispatched_contract() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(root.join("STATE.tsv"), format!("{STATE_HEADER}\n")).unwrap();
        fs::write(
            root.join("agents.tsv"),
            "ada\tgrok\nm\th\techo {BRIEF}\nbea\tcodex\nm\th\techo {BRIEF}\n",
        )
        .unwrap();
        let template = "A1700000042.1.1";
        let other = "A1700000042.1.2";
        for (id, claim) in [(template, "C1"), (other, "C2")] {
            let ad = root.join("attempts").join(id);
            fs::create_dir_all(&ad).unwrap();
            fs::write(
                ad.join("meta.tsv"),
                format!(
                    "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\n{id}\t{claim}\t-\tCLAIM\tclaim-auditor\tada\tgrok\t-\tFOCUSED\tDISPATCHED\t1\t2\t-\n"
                ),
            )
            .unwrap();
            fs::write(
                ad.join("events.tsv"),
                "state\tepoch\tpid\treason\nDISPATCHED\t1\t-\tseed\n",
            )
            .unwrap();
            fs::write(ad.join("transport"), "multi-agent\n").unwrap();
            fs::write(
                ad.join("contract.md"),
                format!(
                    "Read this file and follow it exactly.\nattempt-id: {id}\n## The claim\n{claim} says hello\n## After\nbound claim check\n"
                ),
            )
            .unwrap();
        }
        let out = contract_audit(
            root,
            &[template, "bea", "PASS", "--like", other],
            &FixedClock::new(EPOCH),
        )
        .unwrap();
        let tad = root.join("attempts").join(template);
        let dad = root.join("attempts").join(other);
        assert_eq!(
            out,
            format!(
                "{}/contract-audit.md\n{}/contract-audit.md (like {template})\n",
                tad.display(),
                dad.display()
            )
        );
        let copied = fs::read_to_string(dad.join("contract-audit.md")).unwrap();
        assert!(copied.contains(&format!("attempt {other} by bea")));
        assert!(!copied.contains(template));
        assert!(
            copied.contains("INDEPENDENCE: weak\n") || copied.contains("TRANSPORT: multi-agent\n")
        );
    }
}
