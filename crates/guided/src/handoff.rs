//! Record one git handoff of an order. Reads the product git directory.
//! Does not run git and does not write into the product directory.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::orchestrate::order_id_ok;
use crate::orchestrate::ORDER_HEADER;
use crate::{message, records, GuidedError};

const HEADER: &str = "order_id\tbranch\tcommit\n";

/// `crucible handoff ORDER`.
pub fn handoff(root: &Path, args: &[&str]) -> Result<String, GuidedError> {
    let [order] = args else {
        return Err(message("usage: crucible handoff ORDER"));
    };
    if !order_present(root, order)? {
        return Err(message(format!("no such order: {order}")));
    }
    let product = product_dir(root)?;
    let (branch, sha) = branch_commit(&product)?;
    append_row(root, order, &branch, &sha)?;
    Ok(format!("handoff {order} {branch} {sha}\n"))
}

fn order_present(root: &Path, id: &str) -> Result<bool, GuidedError> {
    if !order_id_ok(id) {
        return Ok(false);
    }
    let path = root.join("ORDERS.tsv");
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };
    if !meta.file_type().is_file() {
        return Err(message("invalid ORDERS.tsv: missing regular file"));
    }
    let text = fs::read_to_string(&path)?;
    let rows = records(&text);
    if rows.first().copied() != Some(ORDER_HEADER) {
        return Err(message("invalid ORDERS.tsv: header mismatch"));
    }
    Ok(rows
        .iter()
        .skip(1)
        .any(|rec| rec.split('\t').next() == Some(id)))
}

fn product_dir(root: &Path) -> Result<PathBuf, GuidedError> {
    let program = root.join("PROGRAM");
    let meta = fs::symlink_metadata(&program);
    if !matches!(meta, Ok(ref info) if info.file_type().is_file()) {
        return Ok(root.to_path_buf());
    }
    let text = fs::read_to_string(&program)?;
    let Some(repo) = records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix("repo: "))
    else {
        return Ok(root.to_path_buf());
    };
    let repo = repo.trim();
    if repo.is_empty() {
        return Err(message("handoff requires a git branch"));
    }
    let path = PathBuf::from(repo);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(root.join(path))
    }
}

fn branch_commit(product: &Path) -> Result<(String, String), GuidedError> {
    let git = product.join(".git");
    if !real_dir(&git) {
        return Err(message("handoff requires a git branch"));
    }
    let head = fs::read_to_string(git.join("HEAD")).unwrap_or_default();
    let line = head.lines().next().unwrap_or("").trim();
    let Some(branch) = line.strip_prefix("ref: refs/heads/") else {
        return Err(message("handoff requires a git branch"));
    };
    if !branch_ok(branch) {
        return Err(message("handoff requires a git branch"));
    }
    let sha = commit_file(&git, branch)?;
    Ok((branch.to_string(), sha))
}

fn branch_ok(branch: &str) -> bool {
    !branch.is_empty()
        && !branch.contains([' ', '\t', '\\'])
        && !branch.contains("..")
        && !branch.starts_with('/')
        && !branch.ends_with('/')
}

fn commit_file(git: &Path, branch: &str) -> Result<String, GuidedError> {
    let mut path = git.join("refs").join("heads");
    for part in branch.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(message("handoff requires a commit"));
        }
        path.push(part);
    }
    let text = fs::read_to_string(&path).unwrap_or_default();
    let sha = text.lines().next().unwrap_or("").trim();
    if sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(sha.to_ascii_lowercase())
    } else {
        Err(message("handoff requires a commit"))
    }
}

fn append_row(root: &Path, order: &str, branch: &str, sha: &str) -> Result<(), GuidedError> {
    let path = root.join("HANDOFF.tsv");
    let row = format!("{order}\t{branch}\t{sha}\n");
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_file() => {
            let mut file = OpenOptions::new().append(true).open(&path)?;
            file.write_all(row.as_bytes())?;
        }
        Ok(_) => return Err(message("handoff record is not a file")),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            fs::write(&path, format!("{HEADER}{row}"))?;
        }
        Err(err) => return Err(err.into()),
    }
    Ok(())
}

fn real_dir(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(meta) => meta.file_type().is_dir(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
    const DECOY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    struct Tmp(PathBuf);

    impl Tmp {
        fn new() -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "crucible-handoff-{}-{}",
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

    fn plant_decoy(root: &Path) {
        let git = root.join(".git");
        fs::create_dir_all(git.join("refs/heads")).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/decoy\n").unwrap();
        fs::write(git.join("refs/heads/decoy"), format!("{DECOY}\n")).unwrap();
    }

    #[test]
    fn handoff_records_the_order_commit_and_leaves_the_product() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let product = root.join("product");
        fs::create_dir_all(&product).unwrap();
        fs::write(product.join("PRODUCT.md"), "stay\n").unwrap();
        fs::write(product.join(".git"), "gitdir: /elsewhere\n").unwrap();
        fs::write(
            root.join("PROGRAM"),
            format!("repo: {}\n", product.display()),
        )
        .unwrap();
        fs::write(
            root.join("ORDERS.tsv"),
            "order_id\tdepends_on\tpaths_file\tverify_script\nA\t-\ta.paths\ta.sh\n",
        )
        .unwrap();
        let orders = fs::read(root.join("ORDERS.tsv")).unwrap();
        plant_decoy(root);
        assert_eq!(
            handoff(root, &[]).unwrap_err().to_string(),
            "usage: crucible handoff ORDER"
        );
        assert_eq!(
            handoff(root, &["B"]).unwrap_err().to_string(),
            "no such order: B"
        );
        assert!(!root.join("HANDOFF.tsv").exists());
        assert_eq!(
            handoff(root, &["A"]).unwrap_err().to_string(),
            "handoff requires a git branch"
        );
        assert!(!root.join("HANDOFF.tsv").exists());
        assert_eq!(fs::read(product.join("PRODUCT.md")).unwrap(), b"stay\n");
        fs::remove_file(product.join(".git")).unwrap();
        let git = product.join(".git");
        fs::create_dir_all(git.join("refs/heads")).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(git.join("refs/heads/main"), format!("{SHA}\n")).unwrap();
        assert_eq!(
            handoff(root, &["A"]).unwrap(),
            format!("handoff A main {SHA}\n")
        );
        assert_eq!(
            fs::read_to_string(root.join("HANDOFF.tsv")).unwrap(),
            format!("order_id\tbranch\tcommit\nA\tmain\t{SHA}\n")
        );
        assert!(!product.join("HANDOFF.tsv").exists());
        assert_eq!(fs::read(product.join("PRODUCT.md")).unwrap(), b"stay\n");
        assert_eq!(
            fs::read(git.join("HEAD")).unwrap(),
            b"ref: refs/heads/main\n"
        );
        assert_eq!(
            fs::read(git.join("refs/heads/main")).unwrap(),
            format!("{SHA}\n").as_bytes()
        );
        assert_eq!(
            fs::read(root.join(".git/refs/heads/decoy")).unwrap(),
            format!("{DECOY}\n").as_bytes()
        );
        assert!(!fs::read_to_string(root.join("HANDOFF.tsv"))
            .unwrap()
            .contains(DECOY));
        assert_eq!(fs::read(root.join("ORDERS.tsv")).unwrap(), orders);
        let recorded = fs::read(root.join("HANDOFF.tsv")).unwrap();
        assert_eq!(
            handoff(root, &["missing"]).unwrap_err().to_string(),
            "no such order: missing"
        );
        assert_eq!(fs::read(root.join("HANDOFF.tsv")).unwrap(), recorded);
    }
}
