use std::fs;
use std::path::Path;
use std::process::Command;

use crucible_contract::{resolve_wm, Clock};

use crate::error::KernelError;
use crate::floor::floor_write;
use crate::metrics::metrics_append;
use crate::paths::kv_get;

const INDEPENDENCE: &str = "SUBAGENT-ISOLATED";
const CARD: &str = "STOP-ASK MAP-HUMAN";

/// Result of the POSIX MAP-HUMAN gate (`map_needs_human_sign` / `human_sign_present`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopAskMapHuman {
    pub stop: bool,
    pub card: String,
    pub station: String,
    pub elapsed_s: i64,
    pub exit: i32,
}

/// Halt when POSIX would `STOP-ASK MAP-HUMAN` (HIGH/live unsigned).
///
/// POSIX: `map_needs_human_sign && ! human_sign_present` → FLOOR / metrics
/// `STOP-ASK MAP-HUMAN`, exit 1. A valid `MAP-HUMAN` sign is not a stop; this
/// function does not invent `MAP-HUMAN`. Does not `git worktree remove`.
/// Isolation is `SUBAGENT-ISOLATED`. Does not invoke grok.
pub fn stop_ask_map_human(
    dir: impl AsRef<Path>,
    clock: &dyn Clock,
) -> Result<StopAskMapHuman, KernelError> {
    let dir = dir.as_ref();
    let (repo, wm) = resolve_wm(dir);

    if !map_needs_human_sign(&repo, &wm) || human_sign_present(&repo, &wm) {
        return Ok(StopAskMapHuman {
            stop: false,
            card: String::new(),
            station: String::new(),
            elapsed_s: 0,
            exit: 0,
        });
    }

    let floor = floor_write(dir, CARD, INDEPENDENCE, clock)?;
    let bound = posix_loop_bound(&repo);
    metrics_append(dir, CARD, "-", bound, floor.elapsed_s, 0, clock)?;
    Ok(StopAskMapHuman {
        stop: true,
        card: floor.card,
        station: floor.station,
        elapsed_s: floor.elapsed_s,
        exit: 1,
    })
}

fn map_needs_human_sign(repo: &Path, wm: &Path) -> bool {
    let slices = parse_slices(repo);
    let sid = first_ready_slice(&slices).or_else(|| {
        let text = fs::read_to_string(wm.join("slice-in-flight")).ok()?;
        kv_get(&text, "id")
    });
    if let Some(sid) = sid {
        return slice_needs_human_sign(repo, &slices, &sid);
    }
    if slices.is_empty() {
        return false;
    }
    slices
        .iter()
        .any(|row| row.risk == "HIGH" || module_live_write(repo, &row.module) == "yes")
}

fn slice_needs_human_sign(repo: &Path, slices: &[SliceRow], id: &str) -> bool {
    let Some(row) = slices.iter().find(|r| r.id == id) else {
        return false;
    };
    row.risk == "HIGH" || module_live_write(repo, &row.module) == "yes"
}

fn first_ready_slice(slices: &[SliceRow]) -> Option<String> {
    for row in slices {
        if row.status == "READY" && deps_ready(slices, &row.depends_on) {
            return Some(row.id.clone());
        }
    }
    None
}

fn deps_ready(slices: &[SliceRow], dep: &str) -> bool {
    if dep.is_empty() || dep == "-" {
        return true;
    }
    dep.split(',').all(|part| {
        let t = part.trim();
        t.is_empty() || t == "-" || slices.iter().any(|r| r.id == t && r.status == "CLOSED")
    })
}

fn module_live_write(repo: &Path, module: &str) -> String {
    if module.is_empty() {
        return "no".to_string();
    }
    let Ok(text) = fs::read_to_string(repo.join("architecture").join("modules.md")) else {
        return "no".to_string();
    };
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = t.split('\t').map(str::trim).collect();
        if cols.len() < 2 || cols[0] == "module_id" {
            continue;
        }
        if cols[0] == module {
            if cols.len() >= 6 {
                return cols[5].to_string();
            }
            return "no".to_string();
        }
    }
    "no".to_string()
}

fn human_sign_present(repo: &Path, wm: &Path) -> bool {
    let path = repo.join("MAP-HUMAN");
    let Ok(text) = fs::read_to_string(&path) else {
        return false;
    };
    let Some(who) = kv_get(&text, "SIGNED") else {
        return false;
    };
    let who = who.trim();
    if who.is_empty() {
        return false;
    }
    match who {
        "parent" | "coordinator" | "loop" | "mapper" | "maker" | "reviewer" | "-" => {
            return false;
        }
        _ => {}
    }
    if let Some(mapper) = mapper_id(wm) {
        if who == mapper {
            return false;
        }
    }
    if let Some(maker) = panel_agent(wm, "maker") {
        if who == maker {
            return false;
        }
    }
    if let Some(rev) = panel_agent(wm, "reviewer") {
        if who == rev {
            return false;
        }
    }
    let Some(map_name) = kv_get(&text, "MAP") else {
        return false;
    };
    let map_name = map_name.trim();
    if map_name.is_empty() {
        return false;
    }
    let map_path = {
        let p = Path::new(map_name);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            repo.join(p)
        }
    };
    if !map_path.is_file() {
        return false;
    }
    let want = kv_get(&text, "SHA256")
        .or_else(|| kv_get(&text, "MAP-SHA256"))
        .map(|s| s.trim().to_ascii_lowercase());
    let Some(want) = want.filter(|s| !s.is_empty()) else {
        return false;
    };
    let Some(got) = file_sha256(&map_path) else {
        return false;
    };
    want == got
}

fn mapper_id(wm: &Path) -> Option<String> {
    let text = fs::read_to_string(wm.join("mapper")).ok()?;
    kv_get(&text, "id")
}

fn panel_agent(wm: &Path, role: &str) -> Option<String> {
    let text = fs::read_to_string(wm.join("PANEL.tsv")).ok()?;
    for line in text.lines() {
        let mut cols = line.split('\t');
        if cols.next()? != role {
            continue;
        }
        let agent = cols.next()?.trim();
        if agent.is_empty() || agent == "-" {
            return None;
        }
        return Some(agent.to_string());
    }
    None
}

fn file_sha256(path: &Path) -> Option<String> {
    for (bin, args) in [
        ("sha256sum", &[] as &[&str]),
        ("shasum", &["-a", "256"]),
        ("openssl", &["dgst", "-sha256"]),
    ] {
        let mut cmd = Command::new(bin);
        cmd.args(args).arg(path);
        let Ok(out) = cmd.output() else {
            continue;
        };
        if !out.status.success() {
            continue;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(hex) = s
            .split_whitespace()
            .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Some(hex.to_ascii_lowercase());
        }
    }
    None
}

struct SliceRow {
    id: String,
    module: String,
    risk: String,
    status: String,
    depends_on: String,
}

fn parse_slices(repo: &Path) -> Vec<SliceRow> {
    let Ok(text) = fs::read_to_string(repo.join("slices.tsv")) else {
        return Vec::new();
    };
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let heads: Vec<&str> = header.split('\t').map(str::trim).collect();
    let hid = match heads.iter().position(|c| *c == "id") {
        Some(i) => i,
        None => return Vec::new(),
    };
    let hs = match heads.iter().position(|c| *c == "status") {
        Some(i) => i,
        None => return Vec::new(),
    };
    let hm = heads.iter().position(|c| *c == "module");
    let hr = heads.iter().position(|c| *c == "risk");
    let hd = heads
        .iter()
        .position(|c| *c == "depends_on" || *c == "depends-on");
    let mut rows = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let id = cols.get(hid).map(|s| s.trim()).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        rows.push(SliceRow {
            id: id.to_string(),
            module: hm
                .and_then(|i| cols.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            risk: hr
                .and_then(|i| cols.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            status: cols
                .get(hs)
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            depends_on: hd
                .and_then(|i| cols.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "-".to_string()),
        });
    }
    rows
}

fn posix_loop_bound(repo: &Path) -> i64 {
    let n = count_slice_rows(repo);
    (16 + 12 * n).clamp(40, 240)
}

fn count_slice_rows(repo: &Path) -> i64 {
    let Ok(text) = fs::read_to_string(repo.join("slices.tsv")) else {
        return 0;
    };
    text.lines()
        .enumerate()
        .filter(|(i, line)| {
            if *i == 0 {
                return false;
            }
            let t = line.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .count() as i64
}
