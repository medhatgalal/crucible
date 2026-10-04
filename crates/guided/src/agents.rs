//! One line per agent, from attempt files only.

use std::fs;
use std::path::Path;

use crate::{records, GuidedError};

/// `agents` and one `name state` line per agent. State is the last
/// `events.tsv` row of that agent's latest attempt. No process list.
pub fn agent_records(root: &Path) -> Result<String, GuidedError> {
    let dir = root.join("attempts");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok("agents\n".to_string());
        }
        Err(err) => return Err(err.into()),
    };
    let mut best: Vec<(String, u64, String, String)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let Some(agent) = agent_name(&path.join("meta.tsv")) else {
            continue;
        };
        let Some((state, epoch)) = last_event(&path.join("events.tsv")) else {
            continue;
        };
        let Some(word) = state_word(&state) else {
            continue;
        };
        if let Some(row) = best.iter_mut().find(|row| row.0 == agent) {
            if (epoch, id.as_str()) >= (row.1, row.2.as_str()) {
                *row = (agent, epoch, id, word.to_string());
            }
        } else {
            best.push((agent, epoch, id, word.to_string()));
        }
    }
    best.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::from("agents\n");
    for (agent, _, _, word) in best {
        out.push_str(&agent);
        out.push(' ');
        out.push_str(&word);
        out.push('\n');
    }
    Ok(out)
}

fn agent_name(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 || rec.is_empty() {
            continue;
        }
        let fields: Vec<&str> = rec.split('\t').collect();
        let name = fields.get(5).copied().unwrap_or("").trim();
        if name.is_empty() || name == "agent" {
            continue;
        }
        return Some(name.to_string());
    }
    None
}

fn last_event(path: &Path) -> Option<(String, u64)> {
    let text = fs::read_to_string(path).ok()?;
    let mut found = None;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 || rec.is_empty() {
            continue;
        }
        let fields: Vec<&str> = rec.split('\t').collect();
        let state = fields.first().copied().unwrap_or("").trim();
        if state.is_empty() || state == "state" {
            continue;
        }
        let epoch = fields.get(1).copied().unwrap_or("0").parse().unwrap_or(0);
        found = Some((state.to_string(), epoch));
    }
    found
}

/// One line for the attempt the factory recorded: `name state running`
/// or `name state stopped`. `kill -0` is asked only about that pid.
pub fn agent_process(root: &Path, attempt_id: &str) -> Result<String, GuidedError> {
    let path = root.join("attempts").join(attempt_id);
    let agent = agent_name(&path.join("meta.tsv"))
        .ok_or_else(|| crate::message(format!("no attempt {attempt_id}")))?;
    let (state, _) = last_event(&path.join("events.tsv"))
        .ok_or_else(|| crate::message(format!("no attempt {attempt_id}")))?;
    let word =
        state_word(&state).ok_or_else(|| crate::message(format!("no attempt {attempt_id}")))?;
    let motion = match last_pid(&path.join("events.tsv")) {
        Some(pid) if process_alive(pid) => "running",
        _ => "stopped",
    };
    Ok(format!("{agent} {word} {motion}\n"))
}

fn last_pid(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let mut pid = None;
    for (idx, rec) in records(&text).into_iter().enumerate() {
        if idx == 0 || rec.is_empty() {
            continue;
        }
        let fields: Vec<&str> = rec.split('\t').collect();
        if let Ok(value) = fields.get(2).copied().unwrap_or("").trim().parse::<u32>() {
            if value >= 2 {
                pid = Some(value);
            }
        }
    }
    pid
}

fn process_alive(pid: u32) -> bool {
    if pid < 2 {
        return false;
    }
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn state_word(state: &str) -> Option<&'static str> {
    match state {
        "DISPATCHED" => Some("new"),
        "RUNNING" | "OVERDUE" => Some("in progress"),
        "RETURNED" | "TIMEOUT" | "STOPPED" | "ABANDONED" => Some("finished"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plant(root: &Path, id: &str, agent: &str, state: &str, epoch: u64) {
        let dir = root.join("attempts").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("meta.tsv"),
            format!(
                "attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of\n{id}\titem\t-\tCLAIM\tmaker\t{agent}\tkind\t-\tFOCUSED\tDISPATCHED\t1\t2\t-\n"
            ),
        )
        .unwrap();
        fs::write(
            dir.join("events.tsv"),
            format!("state\tepoch\tpid\treason\n{state}\t{epoch}\t-\tfixture\n"),
        )
        .unwrap();
    }

    #[test]
    fn agent_records_names_new_in_progress_and_finished() {
        let dir = std::env::temp_dir().join(format!(
            "crucible-agents-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(agent_records(&dir).unwrap(), "agents\n");
        plant(&dir, "A1.1.1", "ada", "DISPATCHED", 1);
        plant(&dir, "A1.1.2", "ada", "RUNNING", 9);
        plant(&dir, "A1.1.3", "bea", "DISPATCHED", 2);
        plant(&dir, "A1.1.4", "cy", "STOPPED", 3);
        assert_eq!(
            agent_records(&dir).unwrap(),
            "agents\nada in progress\nbea new\ncy finished\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    struct Reap(std::process::Child);

    #[cfg(unix)]
    impl Drop for Reap {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[cfg(unix)]
    #[test]
    fn agent_process_reports_the_recorded_pid_only() {
        let dir = std::env::temp_dir().join(format!(
            "crucible-agent-process-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let missing = agent_process(&dir, "A9.9.9").unwrap_err();
        assert_eq!(missing.to_string(), "no attempt A9.9.9");

        plant(&dir, "A1.2.3", "ada", "DISPATCHED", 1);
        let decoy = Reap(
            std::process::Command::new("sleep")
                .arg("30")
                .spawn()
                .unwrap(),
        );
        let decoy_pid = decoy.0.id();
        assert!(decoy_pid >= 2);
        let dashed = agent_process(&dir, "A1.2.3").unwrap();
        assert_eq!(dashed, "ada new stopped\n");
        assert!(
            !dashed.contains(&decoy_pid.to_string()),
            "unrecorded pid {decoy_pid} appeared in {dashed}"
        );

        let mut child = Reap(
            std::process::Command::new("sleep")
                .arg("30")
                .spawn()
                .unwrap(),
        );
        let pid = child.0.id();
        assert!(pid >= 2 && pid != decoy_pid);
        fs::write(
            dir.join("attempts").join("A1.2.3").join("events.tsv"),
            format!(
                "state\tepoch\tpid\treason\nDISPATCHED\t1\t-\tfixture\nRUNNING\t9\t{pid}\tobserved-start\n"
            ),
        )
        .unwrap();
        assert_eq!(
            agent_process(&dir, "A1.2.3").unwrap(),
            "ada in progress running\n"
        );

        let _ = child.0.kill();
        let _ = child.0.wait();
        let stopped = agent_process(&dir, "A1.2.3").unwrap();
        assert_eq!(stopped, "ada in progress stopped\n");
        assert!(
            !stopped.contains(&decoy_pid.to_string()),
            "unrecorded pid {decoy_pid} appeared in {stopped}"
        );
        drop(decoy);
        let _ = fs::remove_dir_all(&dir);
    }
}
