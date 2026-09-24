//! Evidence-grounded claim dispositions. Output strings match the shell.

use std::path::Path;

use crate::claims::{claim_true_uncounted, glob_md, min_kinds_label};
use crate::cycle::{
    claim_field, claim_true_bar_met, claim_true_counts, claim_verdict_word, count_claim_headings,
};
use crate::panel::{guided_min_auditors, guided_min_auditors_label, min_kinds};
use crate::{message, records, GuidedError};

/// Stdout of `crucible triage`, plus the shell status (`1` when any claim is unaudited).
#[derive(Debug)]
pub struct TriageReport {
    pub text: String,
    pub status: i32,
}

pub fn triage(root: &Path) -> Result<TriageReport, GuidedError> {
    let path = root.join("CLAIMS.md");
    if !path.is_file() {
        return Err(message(
            "no CLAIMS.md — run: <engine>/crucible adopt <program>",
        ));
    }
    let text = std::fs::read_to_string(&path)?;
    let n = count_claim_headings(&text);
    if n == 0 {
        return Err(message(
            "no claims yet — crucible claim add \"<claim>\" \"<source sentence>\"",
        ));
    }
    let min_a = guided_min_auditors(root)?;
    let min_a_label = guided_min_auditors_label(root)?;
    let min_k = min_kinds()?;
    let min_k_label = min_kinds_label()?;
    let mut report = format!(
        "TRIAGE — {n} claim(s). Recommendations are derived from verdicts, not from opinion.\n\n"
    );
    let mut unaudited = 0u64;
    for i in 1..=n {
        let title = heading_title(&text, i);
        let title = if title.is_empty() {
            "(untitled)".to_string()
        } else {
            title
        };
        let cn = format!("C{i}");
        let mut true_n = 0u64;
        let mut false_n = 0u64;
        let mut stale_n = 0u64;
        let mut unver_n = 0u64;
        let mut kinds = Vec::new();
        let mut who = String::new();
        let verdicts = root.join("claims").join(&cn).join("verdicts");
        if verdicts.is_dir() {
            for path in glob_md(&verdicts) {
                let Some(word) = claim_verdict_word(&path) else {
                    continue;
                };
                let agent = line_value(&path, "AGENT");
                let kind = line_value(&path, "KIND");
                who.push(' ');
                who.push_str(&agent);
                who.push(':');
                who.push_str(word);
                if !kind.is_empty() && !kinds.iter().any(|seen| seen == &kind) {
                    kinds.push(kind);
                }
                match word {
                    "TRUE" => true_n += 1,
                    "FALSE" => false_n += 1,
                    "STALE" => stale_n += 1,
                    "UNVERIFIABLE" => unver_n += 1,
                    _ => {}
                }
            }
        }
        let kn = kinds.len() as u64;
        let scout = claim_field(&text, i, "scout").unwrap_or_default();
        let item = claim_field(&text, i, "item").unwrap_or_default();
        let total = true_n + false_n + stale_n + unver_n;
        let rec = if total == 0 {
            unaudited += 1;
            "NO RECOMMENDATION — unaudited. Dispatch claim-auditors before deciding this."
                .to_string()
        } else if !item.is_empty() {
            format!("ADMITTED as item {item}")
        } else if false_n > 0 && true_n == 0 {
            "DROP — auditors found it untrue. This is work you do not have to do.".to_string()
        } else if stale_n > 0 && true_n == 0 {
            "DROP — already fixed. Record the change that fixed it.".to_string()
        } else if unver_n > 0 && true_n == 0 {
            "ASK THE OPERATOR — unverifiable from the code. Do not resolve this yourself."
                .to_string()
        } else if true_n > 0 && false_n > 0 {
            format!(
                "AUDITORS DISAGREE — {true_n} TRUE, {false_n} FALSE. Dispatch a third of a different kind."
            )
        } else if scout == "FULLY-EXISTS" {
            "DROP — the scout found this already implemented. Cite where.".to_string()
        } else if scout.is_empty() {
            "SCOUT FIRST — audited TRUE but nobody checked whether it already exists.".to_string()
        } else {
            let (ct, ckn) = claim_true_counts(root, &cn)?;
            if claim_true_bar_met(root, &cn)? {
                if scout == "PARTLY-EXISTS" {
                    "ADMIT, NARROWED — only the gap the scout could not find.".to_string()
                } else {
                    format!(
                        "ADMIT — audited TRUE by {ct} agent(s) across {ckn} kind(s), and absent from the repo."
                    )
                }
            } else if true_n >= min_a && kn >= min_k {
                let why = claim_true_uncounted(root, &cn)?.unwrap_or_else(|| {
                    format!(
                        "the {ct} counted verdict(s) span only {ckn} model kind(s) — dispatch an auditor from another kind in agents.tsv"
                    )
                });
                format!(
                    "INDEPENDENCE INCOMPLETE — {true_n} TRUE on file, {ct} counted across {ckn} kind(s); need {min_a_label} across {min_k_label}. {why}."
                )
            } else {
                format!(
                    "MORE AUDIT — {true_n} TRUE across {kn} kind(s); need {min_a_label} across {min_k_label}."
                )
            }
        };
        let who_col = if who.is_empty() {
            " none".to_string()
        } else {
            who
        };
        let scout_col = if scout.is_empty() { "not run" } else { &scout };
        report.push_str(&format!("C{i}  {title}\n"));
        report.push_str(&format!(
            "     verdicts:{who_col}   kinds: {kn}   scout: {scout_col}\n"
        ));
        report.push_str(&format!("     -> {rec}\n\n"));
    }
    report.push_str(
        "\
Take this to the operator. Say what the evidence says, say what you would do, and let
them decide. Propose merges for overlapping claims and splits for oversized ones —
neither is visible from verdicts alone, so that part is your judgement and theirs.
Report anything you found that the document never mentioned; do not silently add it.
",
    );
    let status = if unaudited > 0 {
        report.push_str(&format!(
            "\nREFUSED: {unaudited} claim(s) have no verdicts, so no disposition was recommended for them.\n"
        ));
        1
    } else {
        0
    };
    Ok(TriageReport {
        text: report,
        status,
    })
}

fn heading_title(text: &str, i: usize) -> String {
    let prefix = format!("### C{i} — ");
    records(text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string()
}

fn line_value(path: &Path, key: &str) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let prefix = format!("{key}: ");
    records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::{claim, ENV_LOCK};
    use crucible_contract::FixedClock;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::MutexGuard;

    static SEQ: AtomicU64 = AtomicU64::new(0);

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "crucible-triage-{}-{}",
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
        // SAFETY: caller holds ENV_LOCK; Restore runs before that guard drops.
        unsafe { std::env::remove_var(key) };
        Restore(key, prev)
    }

    fn agents(root: &Path) {
        fs::write(
            root.join("agents.tsv"),
            "a1\tkindA\tm\thigh\ttrue\na2\tkindA\tm\thigh\ttrue\n",
        )
        .unwrap();
    }

    fn evidence(root: &Path, agent: &str) {
        let dir = root.join("claims/C1/evidence");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("{agent}.manual.txt")),
            "crucible-run/1\nok\n",
        )
        .unwrap();
    }

    fn verdict(root: &Path, agent: &str, word: &str) {
        let dir = root.join("claims/C1/verdicts");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(format!("{agent}.md")),
            format!("CLAIM-VERDICT: {word}\nAGENT: {agent}\nKIND: kindA\nCITATION: q\n"),
        )
        .unwrap();
    }

    #[test]
    fn unaudited_claim_refuses_after_the_report() {
        let _guard = env_lock();
        let _auditors = unset_env("CRUCIBLE_MIN_AUDITORS");
        let _kinds = unset_env("CRUCIBLE_MIN_KINDS");
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        assert_eq!(
            triage(root).unwrap_err().to_string(),
            "no CLAIMS.md — run: <engine>/crucible adopt <program>"
        );
        fs::write(root.join("CLAIMS.md"), "# CLAIMS\n").unwrap();
        assert_eq!(
            triage(root).unwrap_err().to_string(),
            "no claims yet — crucible claim add \"<claim>\" \"<source sentence>\""
        );
        claim(
            root,
            &["add", "the widget is not present", "src"],
            &FixedClock::new(1),
        )
        .unwrap();
        let report = triage(root).unwrap();
        assert_eq!(report.status, 1);
        assert_eq!(
            report.text,
            "\
TRIAGE — 1 claim(s). Recommendations are derived from verdicts, not from opinion.

C1  the widget is not present
     verdicts: none   kinds: 0   scout: not run
     -> NO RECOMMENDATION — unaudited. Dispatch claim-auditors before deciding this.

Take this to the operator. Say what the evidence says, say what you would do, and let
them decide. Propose merges for overlapping claims and splits for oversized ones —
neither is visible from verdicts alone, so that part is your judgement and theirs.
Report anything you found that the document never mentioned; do not silently add it.

REFUSED: 1 claim(s) have no verdicts, so no disposition was recommended for them.
"
        );
    }

    #[test]
    fn dispositions_follow_verdicts_and_the_counted_bar() {
        let _guard = env_lock();
        let _auditors = unset_env("CRUCIBLE_MIN_AUDITORS");
        let _kinds = unset_env("CRUCIBLE_MIN_KINDS");
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        agents(root);
        fs::write(
            root.join("CLAIMS.md"),
            "\
# CLAIMS

### C1 — gap

    polarity: ABSENT
    status: NEW
    scout: ABSENT
    item:
",
        )
        .unwrap();
        fs::create_dir_all(root.join("claims/C1/verdicts")).unwrap();
        verdict(root, "a1", "FALSE");
        let drop = triage(root).unwrap();
        assert_eq!(drop.status, 0);
        assert!(drop.text.contains(
            "     -> DROP — auditors found it untrue. This is work you do not have to do.\n"
        ));

        verdict(root, "a1", "TRUE");
        evidence(root, "a1");
        let more = triage(root).unwrap();
        assert!(
            more.text
                .contains("     -> MORE AUDIT — 1 TRUE across 1 kind(s); need 2 across 1.\n"),
            "{}",
            more.text
        );

        verdict(root, "a2", "TRUE");
        let incomplete = triage(root).unwrap();
        assert!(
            incomplete.text.contains(
                "     -> INDEPENDENCE INCOMPLETE — 2 TRUE on file, 1 counted across 1 kind(s); need 2 across 1. a2 recorded no usable evidence for C1 — run: ./crucible run-claim C1 a2 -- <command>.\n"
            ),
            "{}",
            incomplete.text
        );

        evidence(root, "a2");
        let admit = triage(root).unwrap();
        assert!(
            admit.text.contains(
                "     -> ADMIT — audited TRUE by 2 agent(s) across 1 kind(s), and absent from the repo.\n"
            ),
            "{}",
            admit.text
        );
    }
}
