//! Decide whether a request is one part or a vehicle.
//!
//! The program record is `GRILL.md`. Section order is fixed.

use std::fs;
use std::path::{Path, PathBuf};

use crucible_contract::Clock;

use crate::dispatch::{section_lines, unsafe_owned};
use crate::task::validate_vehicle;
use crate::{message, records, GuidedError};

const SECTIONS: &[&str] = &[
    "vehicle",
    "source words",
    "scout",
    "frame",
    "size",
    "cut",
    "sign",
];

pub fn grill(root: &Path, args: &[&str], clock: &dyn Clock) -> Result<String, GuidedError> {
    let (sub, rest) = match args.split_first() {
        Some((sub, rest)) => (*sub, rest),
        None => ("", &[][..]),
    };
    if sub != "decide" {
        return Err(message("usage: crucible grill decide [REQUEST]"));
    }
    let request = request_path(root, rest)?;
    let text = fs::read_to_string(&request)?;
    let owned = owned_paths(&text)?;
    let checks = check_lines(&text)?;
    let claims = match fs::read_to_string(root.join("CLAIMS.md")) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    let (kept, dropped) = split_scout(&claims, &owned);
    if kept.is_empty() {
        let body = record(
            "dropped",
            &owned,
            &checks,
            &dropped,
            &kept,
            "dropped",
            &[],
            clock.now_unix(),
        );
        write_record(root, &body)?;
        return Ok(stdout_of(&dropped, "", &[]));
    }
    if kept.len() == 1 {
        if checks.len() != 1 {
            return Err(message("one owned path needs one check"));
        }
        let orders = vec![kept[0].clone()];
        let body = record(
            "part",
            &owned,
            &checks,
            &dropped,
            &kept,
            "size: part\nnote: one item",
            &orders,
            clock.now_unix(),
        );
        write_record(root, &body)?;
        return Ok(stdout_of(&dropped, "part", &orders));
    }
    if !root.join("ORDERS.tsv").is_file() {
        return Err(message("vehicle graph required"));
    }
    let orders = validate_vehicle(root, &kept)?;
    let body = record(
        "vehicle\nORDERS.tsv",
        &owned,
        &checks,
        &dropped,
        &kept,
        "size: vehicle",
        &orders,
        clock.now_unix(),
    );
    write_record(root, &body)?;
    Ok(stdout_of(&dropped, "vehicle", &orders))
}

fn request_path(root: &Path, args: &[&str]) -> Result<PathBuf, GuidedError> {
    match args {
        [] => {
            let path = root.join("GRILL.md");
            if path.is_file() {
                Ok(path)
            } else {
                Err(message("usage: crucible grill decide [REQUEST]"))
            }
        }
        [one] => {
            let path = Path::new(one);
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()?.join(path)
            };
            if !path.is_file() {
                return Err(message(format!("no such request: {one}")));
            }
            Ok(path)
        }
        _ => Err(message("usage: crucible grill decide [REQUEST]")),
    }
}

fn owned_paths(text: &str) -> Result<Vec<String>, GuidedError> {
    let lines = section_lines(text, "Owned files");
    if lines.is_empty() {
        return Err(message("request names no owned path"));
    }
    let mut paths = Vec::new();
    for line in lines {
        if !line.starts_with("- ") {
            return Err(message("owned paths must be list items"));
        }
        let path = &line[2..];
        if path.is_empty() || unsafe_owned(path) {
            return Err(message(format!("unsafe owned path {path}")));
        }
        if paths.iter().any(|seen| seen == path) {
            return Err(message(format!("request repeats owned path {path}")));
        }
        paths.push(path.to_string());
    }
    Ok(paths)
}

fn check_lines(text: &str) -> Result<Vec<String>, GuidedError> {
    let mut checks = Vec::new();
    for line in section_lines(text, "Checks") {
        if !line.starts_with("- ") {
            return Err(message("checks must be list items"));
        }
        let check = line[2..].trim();
        if check.is_empty() {
            return Err(message("checks must be list items"));
        }
        checks.push(check.to_string());
    }
    Ok(checks)
}

fn split_scout(claims: &str, owned: &[String]) -> (Vec<String>, Vec<String>) {
    let blocks = claim_blocks(claims);
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for path in owned {
        let fully = blocks
            .iter()
            .any(|block| scout_fully_exists(block) && names_path(block, path));
        if fully {
            dropped.push(path.clone());
        } else {
            kept.push(path.clone());
        }
    }
    (kept, dropped)
}

fn claim_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut cur = String::new();
    let mut on = false;
    for line in records(text) {
        if line.starts_with("### ") {
            if on {
                blocks.push(std::mem::take(&mut cur));
            }
            on = true;
        }
        if on {
            cur.push_str(line);
            cur.push('\n');
        }
    }
    if on {
        blocks.push(cur);
    }
    blocks
}

fn scout_fully_exists(block: &str) -> bool {
    records(block).iter().any(|line| {
        line.trim()
            .strip_prefix("scout:")
            .is_some_and(|rest| rest.trim() == "FULLY-EXISTS")
    })
}

fn names_path(block: &str, path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let bytes = block.as_bytes();
    let needle = path.as_bytes();
    if needle.len() > bytes.len() {
        return false;
    }
    for i in 0..=bytes.len() - needle.len() {
        if &bytes[i..i + needle.len()] != needle {
            continue;
        }
        let before_ok = i == 0 || !is_path_byte(bytes[i - 1]);
        let after = i + needle.len();
        let after_ok = after == bytes.len() || !is_path_byte(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/')
}

fn record(
    vehicle: &str,
    owned: &[String],
    checks: &[String],
    dropped: &[String],
    kept: &[String],
    size: &str,
    orders: &[String],
    epoch: i64,
) -> [(&'static str, String); 7] {
    let mut source = String::new();
    for path in owned {
        source.push_str(&format!("owned: {path}\n"));
    }
    for check in checks {
        source.push_str(&format!("check: {check}\n"));
    }
    let mut scout = String::new();
    for path in dropped {
        scout.push_str(&format!("dropped: {path}\n"));
    }
    for path in kept {
        scout.push_str(&format!("kept: {path}\n"));
    }
    let mut frame = String::new();
    for path in kept {
        frame.push_str(path);
        frame.push('\n');
    }
    for check in checks {
        frame.push_str(check);
        frame.push('\n');
    }
    let mut cut = String::new();
    for order in orders {
        cut.push_str(&format!("order: {order}\n"));
    }
    [
        ("vehicle", vehicle.to_string()),
        ("source words", source),
        ("scout", scout),
        ("frame", frame),
        ("size", size.to_string()),
        ("cut", cut),
        ("sign", format!("decided {epoch}")),
    ]
}

fn write_record(root: &Path, sections: &[(&str, String); 7]) -> Result<(), GuidedError> {
    let mut body = String::from("# GRILL\n");
    for (idx, (title, text)) in sections.iter().enumerate() {
        debug_assert_eq!(*title, SECTIONS[idx]);
        body.push_str(&format!("\n## {title}\n"));
        if !text.is_empty() {
            body.push_str(text);
            if !text.ends_with('\n') {
                body.push('\n');
            }
        }
    }
    let tmp = root.join(format!(".GRILL.md.{}.tmp", std::process::id()));
    let wrote = (|| -> Result<(), GuidedError> {
        fs::write(&tmp, &body)?;
        fs::rename(&tmp, root.join("GRILL.md"))?;
        Ok(())
    })();
    if wrote.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    wrote
}

fn stdout_of(dropped: &[String], size: &str, orders: &[String]) -> String {
    let mut out = String::new();
    for path in dropped {
        out.push_str(&format!("dropped: {path}\n"));
    }
    if orders.is_empty() {
        return out;
    }
    if size == "part" {
        out.push_str("size: part\nnote: one item\n");
    } else {
        out.push_str(&format!("size: {size}\n"));
    }
    for order in orders {
        out.push_str(&format!("order: {order}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FixedClock;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "crucible-grill-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn request(paths: &[&str], checks: &[&str]) -> String {
        let mut text = String::from("# Request\n\n## Owned files\n\n");
        for path in paths {
            text.push_str(&format!("- {path}\n"));
        }
        text.push_str("\n## Checks\n\n");
        for check in checks {
            text.push_str(&format!("- {check}\n"));
        }
        text
    }

    fn write_verify(dir: &Path, name: &str) {
        let path = dir.join(format!("orders/{name}.verify.sh"));
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut perm = fs::metadata(&path).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&path, perm).unwrap();
    }

    fn headings(text: &str) -> Vec<&str> {
        records(text)
            .into_iter()
            .filter_map(|line| line.strip_prefix("## "))
            .collect()
    }

    #[test]
    fn one_file_request_stays_one_order() {
        let dir = tmp();
        let clock = FixedClock::new(42);
        let path = dir.join("REQUEST.md");
        fs::write(
            &path,
            request(&["src/hello.txt"], &["hello is the greeting"]),
        )
        .unwrap();
        fs::write(
            dir.join("CLAIMS.md"),
            "\
# CLAIMS

### C1 greeting
    scout: ABSENT
    path: src/hello.txt
    status: NEW
",
        )
        .unwrap();
        let out = grill(&dir, &["decide", path.to_str().unwrap()], &clock).unwrap();
        assert_eq!(out, "size: part\nnote: one item\norder: src/hello.txt\n");
        let grill_md = fs::read_to_string(dir.join("GRILL.md")).unwrap();
        assert_eq!(headings(&grill_md), SECTIONS);
        assert!(
            grill_md.contains("## size\nsize: part\nnote: one item\n"),
            "{grill_md}"
        );
        assert_eq!(grill_md.matches("order:").count(), 1, "{grill_md}");
        assert!(!dir.join("items").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_paths_without_orders_do_not_create_an_item() {
        let dir = tmp();
        let clock = FixedClock::new(42);
        let path = dir.join("REQUEST.md");
        fs::write(
            &path,
            request(
                &["src/door.txt", "src/frame.txt"],
                &["door opens", "frame holds"],
            ),
        )
        .unwrap();
        let err = grill(&dir, &["decide", path.to_str().unwrap()], &clock).unwrap_err();
        assert_eq!(err.to_string(), "vehicle graph required");
        assert!(!dir.join("GRILL.md").exists());
        assert!(!dir.join("items").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn valid_orders_accept_a_vehicle() {
        let dir = tmp();
        let clock = FixedClock::new(42);
        let path = dir.join("REQUEST.md");
        fs::write(
            &path,
            request(&["src/door.txt", "src/frame.txt"], &["door opens"]),
        )
        .unwrap();
        fs::create_dir_all(dir.join("orders")).unwrap();
        fs::write(
            dir.join("ORDERS.tsv"),
            "\
order_id\tdepends_on\tpaths_file\tverify_script
door\t-\torders/door.paths\torders/door.verify.sh
frame\t-\torders/frame.paths\torders/frame.verify.sh
assembly\tdoor,frame\t-\torders/assembly.verify.sh
",
        )
        .unwrap();
        fs::write(dir.join("orders/door.paths"), "src/door.txt\n").unwrap();
        fs::write(dir.join("orders/frame.paths"), "src/frame.txt\n").unwrap();
        for name in ["door", "frame", "assembly"] {
            write_verify(&dir, name);
        }
        let out = grill(&dir, &["decide", path.to_str().unwrap()], &clock).unwrap();
        assert_eq!(
            out,
            "size: vehicle\norder: door\norder: frame\norder: assembly\n"
        );
        let grill_md = fs::read_to_string(dir.join("GRILL.md")).unwrap();
        assert_eq!(headings(&grill_md), SECTIONS);
        assert!(grill_md.contains("size: vehicle\n"), "{grill_md}");
        assert!(!dir.join("items").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fully_exists_path_is_dropped() {
        let dir = tmp();
        let clock = FixedClock::new(42);
        let path = dir.join("REQUEST.md");
        fs::write(
            &path,
            request(&["src/hello.txt"], &["hello is the greeting"]),
        )
        .unwrap();
        fs::write(
            dir.join("CLAIMS.md"),
            "\
# CLAIMS

### C1 greeting
    scout: FULLY-EXISTS
    path: src/hello.txt
    status: CLOSED
",
        )
        .unwrap();
        let out = grill(&dir, &["decide", path.to_str().unwrap()], &clock).unwrap();
        assert_eq!(out, "dropped: src/hello.txt\n");
        let grill_md = fs::read_to_string(dir.join("GRILL.md")).unwrap();
        assert_eq!(headings(&grill_md), SECTIONS);
        assert!(!grill_md.contains("order:"), "{grill_md}");
        assert!(!grill_md.contains("size: part"), "{grill_md}");
        assert!(!dir.join("items").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn absent_grill_md_is_the_request_when_no_path_is_given() {
        let dir = tmp();
        let clock = FixedClock::new(7);
        fs::write(
            dir.join("GRILL.md"),
            request(&["src/hello.txt"], &["hello is the greeting"]),
        )
        .unwrap();
        let out = grill(&dir, &["decide"], &clock).unwrap();
        assert!(out.contains("order: src/hello.txt\n"), "{out}");
        assert_eq!(
            headings(&fs::read_to_string(dir.join("GRILL.md")).unwrap()),
            SECTIONS
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cycle_unknown_id_overlap_and_missing_assembly_are_refused() {
        let clock = FixedClock::new(1);
        let cases = [
            (
                "\
order_id\tdepends_on\tpaths_file\tverify_script
door\tframe\torders/door.paths\torders/door.verify.sh
frame\tdoor\torders/frame.paths\torders/frame.verify.sh
assembly\tdoor,frame\t-\torders/assembly.verify.sh
",
                "invalid ORDERS.tsv: dependency cycle",
                false,
            ),
            (
                "\
order_id\tdepends_on\tpaths_file\tverify_script
door\tmissing\torders/door.paths\torders/door.verify.sh
frame\t-\torders/frame.paths\torders/frame.verify.sh
assembly\tdoor,frame\t-\torders/assembly.verify.sh
",
                "invalid ORDERS.tsv: unknown dependency missing for door",
                false,
            ),
            (
                "\
order_id\tdepends_on\tpaths_file\tverify_script
door\t-\torders/door.paths\torders/door.verify.sh
frame\t-\torders/frame.paths\torders/frame.verify.sh
assembly\tdoor,frame\t-\torders/assembly.verify.sh
",
                "invalid ORDERS.tsv: ownership overlap src/door.txt (door) and src/door.txt (frame)",
                true,
            ),
            (
                "\
order_id\tdepends_on\tpaths_file\tverify_script
door\t-\torders/door.paths\torders/door.verify.sh
frame\t-\torders/frame.paths\torders/frame.verify.sh
",
                "invalid ORDERS.tsv: missing assembly row",
                false,
            ),
        ];
        for (tsv, needle, overlap) in cases {
            let dir = tmp();
            let path = dir.join("REQUEST.md");
            fs::write(
                &path,
                request(&["src/door.txt", "src/frame.txt"], &["holds"]),
            )
            .unwrap();
            fs::create_dir_all(dir.join("orders")).unwrap();
            fs::write(dir.join("ORDERS.tsv"), tsv).unwrap();
            if overlap {
                fs::write(dir.join("orders/door.paths"), "src/door.txt\n").unwrap();
                fs::write(dir.join("orders/frame.paths"), "src/door.txt\n").unwrap();
            } else {
                fs::write(dir.join("orders/door.paths"), "src/door.txt\n").unwrap();
                fs::write(dir.join("orders/frame.paths"), "src/frame.txt\n").unwrap();
            }
            for name in ["door", "frame", "assembly"] {
                write_verify(&dir, name);
            }
            let err = grill(&dir, &["decide", path.to_str().unwrap()], &clock)
                .unwrap_err()
                .to_string();
            assert!(err.contains(needle), "{err} wanted {needle}");
            assert!(!dir.join("items").exists());
            assert!(!dir.join("GRILL.md").exists());
            let _ = fs::remove_dir_all(&dir);
        }
    }
}
