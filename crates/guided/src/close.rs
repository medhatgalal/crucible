//! `check` and `close`.
//!
//! The lesson date is the UTC calendar date of [`Clock::now_unix`] (shell line 5974),
//! not `date +%Y-%m-%d`.

use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crucible_contract::{format_rfc3339_z, Clock};

use crate::attempt::result_field;
use crate::claims::{min_kinds_label, require_attempt_independence, rewrite_claim_line};
use crate::cycle::{
    attempt_child_dirs, claim_field, count_claim_headings, file_name, read_dir_paths,
    stale_evidence, workid, MARK,
};
use crate::dispatch::{is_maker, need, validate_managed_item};
use crate::panel::{
    h12, is_posint, is_regular, kind_of, min_kinds, panel_required_count, split_tabs,
};
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
/// The product `reviews/review.md` must contain `Judgment of SLUG: the judge is not the author.`
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
        let repo = crate::orchestrate::review_directory(root);
        // Held across recovery, the move, and the restore.
        // A second check must not observe a file already renamed away.
        let _probe_lock = acquire_probe_lock(&repo)?;
        // Before validation, so a killed probe is restored even when this check then refuses.
        recover_stranded(&repo)?;
        validate_managed_item(root, slug)?;
        if phase_of(root, slug)? == "REVIEW" {
            refuse_nondiscriminating_falsifier(root, slug)?;
        } else {
            say(&mut out, &mut bad, "FAIL managed item must be in REVIEW");
        }
    } else {
        check_item_falsifier(&dir, &mut out, &mut bad)?;
    }
    check_product_porcelain(root, &dir, &mut out, &mut bad)?;
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
    if !crate::orchestrate::visible_judgment(root, slug) {
        let path = crate::orchestrate::review_directory(root)
            .join("reviews")
            .join("review.md");
        say(
            &mut out,
            &mut bad,
            &format!(
                "FAIL no visible non-author judgment at {} (Judgment of {slug}: the judge is not the author.)",
                path.display()
            ),
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

fn check_product_porcelain(
    root: &Path,
    dir: &Path,
    out: &mut String,
    bad: &mut bool,
) -> Result<(), GuidedError> {
    let Some(repo) = crate::cycle::program_field(root, "repo") else {
        return Ok(());
    };
    let repo = Path::new(&repo);
    if !repo.is_dir() {
        return Ok(());
    }
    let item = fs::read_to_string(dir.join("ITEM.md")).unwrap_or_default();
    let owned = crate::harness::owned_file_tokens(&item);
    for path in crate::harness::porcelain_outside_owned(repo, &owned) {
        say(
            out,
            bad,
            &format!("FAIL product porcelain outside owned files: {path}"),
        );
    }
    Ok(())
}

const FALSIFIER_BOUND: Duration = Duration::from_secs(15);
static HIDE_SEQ: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
thread_local! {
    static FALSIFIER_BOUND_OVERRIDE: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
}

fn falsifier_bound() -> Duration {
    #[cfg(test)]
    {
        if let Some(bound) = FALSIFIER_BOUND_OVERRIDE.get() {
            return bound;
        }
    }
    FALSIFIER_BOUND
}

/// Move each existing owned file aside, run the one focused falsifier, and put the files back.
/// Exit 0 means the command still passes with the behavior gone.
fn refuse_nondiscriminating_falsifier(root: &Path, slug: &str) -> Result<(), GuidedError> {
    let text = fs::read_to_string(crate::dispatch::item_dir(root, slug).join("ITEM.md"))?;
    let command = crate::dispatch::section_lines(&text, "Focused falsifier")
        .into_iter()
        .next()
        .unwrap_or_default();
    let repo = crate::orchestrate::review_directory(root);
    let mut hidden = Hidden::prepare(&repo)?;
    let mut paths = Vec::new();
    for line in crate::dispatch::section_lines(&text, "Owned files") {
        let Some(rel) = line.strip_prefix("- ") else {
            continue;
        };
        if crate::dispatch::unsafe_owned(rel) || !owned_path_stays_in_repo(&repo, rel) {
            continue;
        }
        if rel
            .bytes()
            .any(|byte| matches!(byte, b'\t' | b'\n' | b'\r'))
        {
            hidden.restore()?;
            return Err(message("refused: owned path cannot be recorded"));
        }
        let path = repo.join(rel);
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                hidden.restore()?;
                return Err(err.into());
            }
        };
        if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
            continue;
        }
        paths.push(path);
    }
    // Held across the move, the run, and the restore. A signal outside this
    // function still kills the process; inside it, the files go back first.
    let _signals = StopSignals::install();
    paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in paths {
        if INTERRUPTED.swap(false, Ordering::SeqCst) {
            hidden.restore()?;
            return Err(message("refused: check interrupted"));
        }
        let backup = hidden.dir.join(hidden.pairs.len().to_string());
        // The manifest names the backup before the rename. A kill in that gap
        // leaves the file in place; a kill after the rename is recoverable.
        hidden.pairs.push((path.clone(), backup.clone()));
        if let Err(err) = hidden.write_manifest(&repo) {
            hidden.pairs.pop();
            hidden.restore()?;
            return Err(err);
        }
        if let Err(err) = fs::rename(&path, &backup) {
            hidden.pairs.pop();
            hidden.restore()?;
            return Err(err.into());
        }
    }
    if hidden.pairs.is_empty() {
        hidden.restore()?;
        if INTERRUPTED.swap(false, Ordering::SeqCst) {
            return Err(message("refused: check interrupted"));
        }
        return Ok(());
    }
    if let Err(err) = hidden.snapshot() {
        hidden.restore()?;
        return Err(err.into());
    }
    let ran = falsifier_passed(&repo, &command);
    hidden.restore()?;
    if INTERRUPTED.swap(false, Ordering::SeqCst) {
        return Err(message("refused: check interrupted"));
    }
    if ran? {
        return Err(message("refused: falsifier does not discriminate"));
    }
    Ok(())
}

struct Hidden {
    dir: PathBuf,
    repo: PathBuf,
    pairs: Vec<(PathBuf, PathBuf)>,
    /// Private copies of the owned bytes, taken before the falsifier runs.
    snaps: Vec<Vec<Snap>>,
    lock: Option<File>,
    /// A failed restore must not delete the manifest on a later `Drop`.
    pending_failure: bool,
}

impl Hidden {
    fn prepare(repo: &Path) -> Result<Self, GuidedError> {
        let canon = fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
        let parent = canon.parent().unwrap_or(&canon);
        let dir = parent.join(format!(
            ".crucible-falsifier-hide-{}-{}",
            std::process::id(),
            HIDE_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        if dir.starts_with(&canon) {
            return Err(message(
                "refused: cannot hold the owned files outside the product tree",
            ));
        }
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(&dir)?;
        let mut perms = fs::metadata(&dir)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&dir, perms)?;
        let lock = match OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(dir.join("LOCK"))
        {
            Ok(file) => file,
            Err(err) => {
                let _ = fs::remove_dir(&dir);
                return Err(err.into());
            }
        };
        if let Err(err) = lock_hide(&lock) {
            drop(lock);
            let _ = fs::remove_file(dir.join("LOCK"));
            let _ = fs::remove_dir(&dir);
            return Err(err.into());
        }
        let hidden = Self {
            dir,
            repo: canon,
            pairs: Vec::new(),
            snaps: Vec::new(),
            lock: Some(lock),
            pending_failure: false,
        };
        hidden.write_manifest(repo)?;
        Ok(hidden)
    }

    fn write_manifest(&self, repo: &Path) -> Result<(), GuidedError> {
        let canon = fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
        let mut body = format!("repo\t{}\n", canon.display());
        for (index, (original, _)) in self.pairs.iter().enumerate() {
            let stored = fs::canonicalize(original).unwrap_or_else(|_| original.clone());
            if !path_can_be_recorded(&stored) {
                return Err(message("refused: owned path cannot be recorded"));
            }
            body.push_str(&format!("{index}\t{}\n", stored.display()));
        }
        let tmp = self.dir.join("MANIFEST.tmp");
        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp)?;
            file.write_all(body.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&tmp, self.dir.join("MANIFEST"))?;
        Ok(())
    }

    fn snapshot(&mut self) -> std::io::Result<()> {
        let mut seq = 0u64;
        let mut all = Vec::with_capacity(self.pairs.len());
        for (_, backup) in &self.pairs {
            all.push(collect_snaps(&self.dir, backup, &mut seq)?);
        }
        self.snaps = all;
        Ok(())
    }

    fn restore(&mut self) -> Result<(), GuidedError> {
        if self.pending_failure {
            return Err(message("could not restore owned files"));
        }
        let mut first: Option<GuidedError> = None;
        let mut snap_rows = std::mem::take(&mut self.snaps);
        let mut snap_rows = snap_rows.drain(..).rev();
        let mut seq = 0u64;
        for (original, backup) in self.pairs.drain(..).rev() {
            if !restore_target_inside(&self.repo, &original) {
                self.pending_failure = true;
                return Err(message(
                    "refused: restore target is outside the product tree",
                ));
            }
            let row = snap_rows.next();
            let restored = if let Some(row) = row {
                restore_from_snaps(&self.repo, &original, &backup, row, &mut seq)
            } else {
                put_back(&original, &backup)
            };
            if let Err(err) = restored {
                if first.is_none() {
                    first = Some(message(format!(
                        "could not restore {}: {err}",
                        original.display()
                    )));
                }
            }
        }
        if let Some(err) = first {
            self.pending_failure = true;
            return Err(err);
        }
        self.remove_hide_dir()
    }

    fn remove_hide_dir(&mut self) -> Result<(), GuidedError> {
        if !self.dir.exists() {
            self.lock.take();
            return Ok(());
        }
        let _ = fs::remove_file(self.dir.join("MANIFEST"));
        let _ = fs::remove_file(self.dir.join("MANIFEST.tmp"));
        self.lock.take();
        let _ = fs::remove_file(self.dir.join("LOCK"));
        if fs::remove_dir(&self.dir).is_err() && self.dir.exists() {
            return Err(message(format!("could not remove {}", self.dir.display())));
        }
        Ok(())
    }
}

impl Drop for Hidden {
    fn drop(&mut self) {
        if self.restore().is_err() {
            // Unlock so the next check can restore from the manifest.
            self.lock.take();
        }
    }
}

fn lock_hide(file: &File) -> std::io::Result<()> {
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn try_lock_hide(file: &File) -> Result<bool, std::io::Error> {
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(true);
    }
    let err = std::io::Error::last_os_error();
    if err.kind() == std::io::ErrorKind::WouldBlock || err.raw_os_error() == Some(libc::EWOULDBLOCK)
    {
        return Ok(false);
    }
    Err(err)
}

fn path_can_be_recorded(path: &Path) -> bool {
    let bytes = path.as_os_str().as_bytes();
    std::str::from_utf8(bytes).is_ok()
        && !bytes
            .iter()
            .any(|byte| matches!(byte, b'\t' | b'\n' | b'\r'))
}

fn probe_lock_dir(canon: &Path) -> Result<PathBuf, GuidedError> {
    // Beside the product tree, on the same parent the hide directory uses.
    // A temp directory can be unlinked while the lock is held, which splits the inode.
    let Some(parent) = canon.parent() else {
        return Err(message("refused: product path cannot be recorded"));
    };
    let dir = parent.join(".crucible-falsifier-locks");
    if dir.starts_with(canon) {
        return Err(message(
            "refused: cannot hold the owned files outside the product tree",
        ));
    }
    Ok(dir)
}

fn probe_lock_name(canon: &Path) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in canon.as_os_str().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn ensure_probe_lock_dir(dir: &Path) -> Result<(), GuidedError> {
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            return Err(message("refused: cannot lock the falsifier probe"));
        }
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            if let Err(err) = builder.create(dir) {
                if err.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(err.into());
                }
            }
        }
        Err(err) => return Err(err.into()),
    }
    let mut perms = fs::metadata(dir)?.permissions();
    perms.set_mode(0o700);
    fs::set_permissions(dir, perms)?;
    Ok(())
}

/// Lock for one product tree. A second check fails until this file is dropped.
fn acquire_probe_lock(repo: &Path) -> Result<File, GuidedError> {
    let canon = match fs::canonicalize(repo) {
        Ok(path) => path,
        Err(err) => return Err(err.into()),
    };
    if !path_can_be_recorded(&canon) {
        return Err(message("refused: product path cannot be recorded"));
    }
    let dir = probe_lock_dir(&canon)?;
    ensure_probe_lock_dir(&dir)?;
    let path = dir.join(probe_lock_name(&canon));
    if fs::symlink_metadata(&path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(message("refused: cannot lock the falsifier probe"));
    }
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).mode(0o600);
    let file = options.open(&path)?;
    let mut perms = fs::metadata(&path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&path, perms)?;
    if !try_lock_hide(&file)? {
        return Err(message("refused: falsifier probe in progress"));
    }
    Ok(file)
}

struct Snap {
    rel: PathBuf,
    mode: u32,
    kind: SnapKind,
}

enum SnapKind {
    Dir,
    File(File),
    Link(PathBuf),
}

fn collect_snaps(hide: &Path, backup: &Path, seq: &mut u64) -> std::io::Result<Vec<Snap>> {
    let mut out = Vec::new();
    collect_one(hide, backup, Path::new(""), &mut out, seq)?;
    Ok(out)
}

fn collect_one(
    hide: &Path,
    root: &Path,
    rel: &Path,
    out: &mut Vec<Snap>,
    seq: &mut u64,
) -> std::io::Result<()> {
    let path = if rel.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };
    let meta = fs::symlink_metadata(&path)?;
    let mode = meta.permissions().mode() & 0o777;
    if meta.file_type().is_symlink() {
        out.push(Snap {
            rel: rel.to_path_buf(),
            mode,
            kind: SnapKind::Link(fs::read_link(&path)?),
        });
        return Ok(());
    }
    if meta.is_dir() {
        out.push(Snap {
            rel: rel.to_path_buf(),
            mode,
            kind: SnapKind::Dir,
        });
        let mut names = Vec::new();
        for entry in fs::read_dir(&path)? {
            names.push(entry?.file_name());
        }
        names.sort();
        for name in names {
            collect_one(hide, root, &rel.join(name), out, seq)?;
        }
        return Ok(());
    }
    if !meta.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "owned backup is not a file",
        ));
    }
    let tmp = hide.join(format!(".snap-{seq}"));
    *seq += 1;
    let mut input = File::open(&path)?;
    let mut body = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    std::io::copy(&mut input, &mut body)?;
    body.sync_all()?;
    fs::remove_file(&tmp)?;
    body.seek(SeekFrom::Start(0))?;
    out.push(Snap {
        rel: rel.to_path_buf(),
        mode,
        kind: SnapKind::File(body),
    });
    Ok(())
}

fn restore_from_snaps(
    repo: &Path,
    original: &Path,
    backup: &Path,
    mut row: Vec<Snap>,
    seq: &mut u64,
) -> std::io::Result<()> {
    for snap in &mut row {
        if snap.rel.is_absolute()
            || snap
                .rel
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "snapshot path leaves the owned file",
            ));
        }
        let dest = if snap.rel.as_os_str().is_empty() {
            original.to_path_buf()
        } else {
            original.join(&snap.rel)
        };
        if !restore_target_inside(repo, &dest) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "snapshot destination is outside the product tree",
            ));
        }
        place_snap(&dest, snap, seq)?;
    }
    for snap in &row {
        let SnapKind::Dir = snap.kind else {
            continue;
        };
        let dest = if snap.rel.as_os_str().is_empty() {
            original.to_path_buf()
        } else {
            original.join(&snap.rel)
        };
        let mut perms = fs::symlink_metadata(&dest)?.permissions();
        perms.set_mode(snap.mode);
        fs::set_permissions(&dest, perms)?;
    }
    discard_backup(backup)
}

fn clear_occupant(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn place_snap(dest: &Path, snap: &mut Snap, seq: &mut u64) -> std::io::Result<()> {
    if let SnapKind::Dir = snap.kind {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        // Replace a file or a symlink. `create_dir_all` would keep either.
        clear_occupant(dest)?;
        return fs::create_dir(dest);
    }
    let Some(parent) = dest.parent() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "owned path has no parent",
        ));
    };
    fs::create_dir_all(parent)?;
    clear_occupant(dest)?;
    match &mut snap.kind {
        SnapKind::Dir => Ok(()),
        SnapKind::Link(target) => std::os::unix::fs::symlink(target, dest),
        SnapKind::File(body) => {
            let tmp = parent.join(format!(".crucible-restore-{}-{}", std::process::id(), *seq));
            *seq += 1;
            // The falsifier can plant this name before it exits. Do not open it.
            clear_occupant(&tmp)?;
            body.seek(SeekFrom::Start(0))?;
            let mut out = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&tmp)?;
            std::io::copy(body, &mut out)?;
            out.sync_all()?;
            let mut perms = out.metadata()?.permissions();
            perms.set_mode(snap.mode);
            fs::set_permissions(&tmp, perms)?;
            fs::rename(&tmp, dest)
        }
    }
}

fn discard_backup(backup: &Path) -> std::io::Result<()> {
    let meta = match fs::symlink_metadata(backup) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(backup)
    } else {
        fs::remove_file(backup)
    }
}

fn put_back(original: &Path, backup: &Path) -> std::io::Result<()> {
    let backup_meta = match fs::symlink_metadata(backup) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // Another check already restored, or the rename never happened.
            // Do not unlink the live path.
            if fs::symlink_metadata(original).is_ok() {
                return Ok(());
            }
            return Err(err);
        }
        Err(err) => return Err(err),
    };
    if let Ok(meta) = fs::symlink_metadata(original) {
        let original_is_dir = meta.is_dir() && !meta.file_type().is_symlink();
        let backup_is_file = backup_meta.is_file() || backup_meta.file_type().is_symlink();
        // `rename` replaces a file. It does not replace a directory.
        if original_is_dir || !backup_is_file {
            if original_is_dir {
                fs::remove_dir_all(original)?;
            } else {
                fs::remove_file(original)?;
            }
        }
    }
    fs::rename(backup, original)
}

/// A symlinked ancestor would make `rename` leave the product tree.
fn owned_path_stays_in_repo(repo: &Path, rel: &str) -> bool {
    let mut cur = repo.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() {
            continue;
        }
        cur.push(part);
        match fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => return false,
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return true,
            Err(_) => return false,
        }
    }
    true
}

fn falsifier_passed(repo: &Path, line: &str) -> Result<bool, GuidedError> {
    let argv: Vec<&str> = line.split_whitespace().collect();
    let Some(program) = argv.first().copied() else {
        return Ok(false);
    };
    if program.is_empty() {
        return Ok(false);
    }
    let mut command = Command::new(program);
    command
        .args(&argv[1..])
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // The falsifier and anything it starts share one group, so the time bound stops all of them.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {
            return Err(message("refused: check interrupted"));
        }
        // NotFound, EACCES, and ENOEXEC all mean the command never ran.
        Err(_) => return Err(message("refused: falsifier did not run")),
    };
    let started = Instant::now();
    let bound = falsifier_bound();
    loop {
        if INTERRUPTED.swap(false, Ordering::SeqCst) {
            stop_falsifier_group(&mut child);
            return Err(message("refused: check interrupted"));
        }
        match child.try_wait() {
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {
                stop_falsifier_group(&mut child);
                return Err(message("refused: check interrupted"));
            }
            Err(err) => {
                stop_falsifier_group(&mut child);
                return Err(err.into());
            }
            Ok(Some(status)) => {
                let interrupted = INTERRUPTED.swap(false, Ordering::SeqCst);
                // The leader can exit while a background child is still in the group.
                stop_falsifier_group(&mut child);
                if interrupted {
                    return Err(message("refused: check interrupted"));
                }
                return Ok(status.success());
            }
            Ok(None) if started.elapsed() >= bound => {
                stop_falsifier_group(&mut child);
                return Ok(false);
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
        }
    }
}

static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static SIGNAL_LOCK: Mutex<()> = Mutex::new(());

extern "C" fn on_stop(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

struct StopSignals {
    old_int: libc::sigaction,
    old_term: libc::sigaction,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl StopSignals {
    fn install() -> Self {
        let lock = SIGNAL_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        INTERRUPTED.store(false, Ordering::SeqCst);
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_stop as *const () as usize;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            let mut old_int = std::mem::zeroed();
            let mut old_term = std::mem::zeroed();
            libc::sigaction(libc::SIGINT, &action, &mut old_int);
            libc::sigaction(libc::SIGTERM, &action, &mut old_term);
            Self {
                old_int,
                old_term,
                _lock: lock,
            }
        }
    }
}

impl Drop for StopSignals {
    fn drop(&mut self) {
        unsafe {
            libc::sigaction(libc::SIGINT, &self.old_int, std::ptr::null_mut());
            libc::sigaction(libc::SIGTERM, &self.old_term, std::ptr::null_mut());
        }
    }
}

fn recover_stranded(repo: &Path) -> Result<(), GuidedError> {
    let Ok(repo) = fs::canonicalize(repo) else {
        return Ok(());
    };
    let Some(parent) = repo.parent() else {
        return Ok(());
    };
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(".crucible-falsifier-hide-")
            && hide_dir_is_ours(&entry.path())
        {
            dirs.push(entry.path());
        }
    }
    dirs.sort();
    for dir in dirs {
        restore_one_hide(&dir, &repo)?;
    }
    Ok(())
}

fn hide_dir_is_ours(dir: &Path) -> bool {
    fs::symlink_metadata(dir)
        .map(|meta| meta.uid() == unsafe { libc::getuid() })
        .unwrap_or(false)
}

fn lexical_absolute(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::RootDir => out.push(comp),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            std::path::Component::Normal(part) => out.push(part),
            std::path::Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

fn restore_target_inside(repo: &Path, original: &Path) -> bool {
    let Some(normalized) = lexical_absolute(original) else {
        return false;
    };
    // Collapse `..` before looking at the filesystem, then canonicalize the
    // longest existing prefix. `/var` and a symlink to this same tree stay
    // inside. A prefix that canonicalizes outside the tree does not.
    let Some(lexical) = resolve_restore_path(&normalized) else {
        return false;
    };
    if !lexical.starts_with(repo) || lexical.as_path() == repo {
        return false;
    }
    // `rename` follows a symlink and only then applies `..`. The lexical path
    // can sit inside the tree while that rename lands outside it.
    if path_has_dot(original) {
        let Some(kernel) = kernel_restore_landing(original) else {
            return false;
        };
        if kernel.as_path() != lexical.as_path() || !kernel.starts_with(repo) {
            return false;
        }
    }
    true
}

fn path_has_dot(path: &Path) -> bool {
    path.components().any(|comp| {
        matches!(
            comp,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    })
}

#[derive(Clone)]
enum KernelStep {
    Root,
    Cur,
    Parent,
    Normal(std::ffi::OsString),
}

fn kernel_steps(path: &Path) -> Option<Vec<KernelStep>> {
    let mut out = Vec::new();
    for comp in path.components() {
        out.push(match comp {
            std::path::Component::Prefix(_) => return None,
            std::path::Component::RootDir => KernelStep::Root,
            std::path::Component::CurDir => KernelStep::Cur,
            std::path::Component::ParentDir => KernelStep::Parent,
            std::path::Component::Normal(part) => KernelStep::Normal(part.to_os_string()),
        });
    }
    Some(out)
}

/// Where `rename` creates the final component. `.` and `..` are applied after
/// symlink targets, which is the kernel order, not the lexical order.
fn kernel_restore_landing(original: &Path) -> Option<PathBuf> {
    if !original.is_absolute() {
        return None;
    }
    let steps = kernel_steps(original)?;
    let name = match steps.last()? {
        KernelStep::Normal(part) => part.clone(),
        _ => return None,
    };
    let mut resolved = kernel_resolve(&steps[..steps.len() - 1])?;
    resolved.push(name);
    Some(resolved)
}

fn kernel_resolve(initial: &[KernelStep]) -> Option<PathBuf> {
    use std::collections::VecDeque;
    let mut pending: VecDeque<KernelStep> = initial.iter().cloned().collect();
    let mut out = PathBuf::new();
    let mut followed = 0u32;
    let mut missing = false;
    while let Some(step) = pending.pop_front() {
        match step {
            KernelStep::Root => {
                out = PathBuf::from("/");
                missing = false;
            }
            KernelStep::Cur => {}
            KernelStep::Parent => {
                if !out.pop() {
                    return None;
                }
                missing = kernel_path_missing(&out)?;
            }
            KernelStep::Normal(part) => {
                out.push(&part);
                if missing {
                    continue;
                }
                match fs::symlink_metadata(&out) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        followed += 1;
                        if followed > 40 {
                            return None;
                        }
                        let target = fs::read_link(&out).ok()?;
                        out.pop();
                        let target_steps = kernel_steps(&target)?;
                        if target.is_absolute() {
                            out.clear();
                            missing = false;
                        }
                        let mut rest = VecDeque::new();
                        rest.extend(target_steps);
                        while let Some(next) = pending.pop_front() {
                            rest.push_back(next);
                        }
                        pending = rest;
                    }
                    Ok(_) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => missing = true,
                    Err(_) => return None,
                }
            }
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn kernel_path_missing(path: &Path) -> Option<bool> {
    if path == Path::new("/") {
        return Some(false);
    }
    match fs::symlink_metadata(path) {
        Ok(_) => Some(false),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Some(true),
        Err(_) => None,
    }
}

/// Parent's canonical location, plus the final component `rename` will create.
/// Canonicalize follows a symlink, so a parent that points outside the tree
/// resolves outside and the caller rejects it.
fn resolve_restore_path(normalized: &Path) -> Option<PathBuf> {
    let name = normalized.file_name()?;
    let mut resolved = resolve_existing_prefix(normalized.parent()?)?;
    resolved.push(name);
    Some(resolved)
}

fn resolve_existing_prefix(path: &Path) -> Option<PathBuf> {
    let mut missing = Vec::new();
    let mut cur = path.to_path_buf();
    loop {
        match fs::symlink_metadata(&cur) {
            Ok(_) => {
                let mut resolved = fs::canonicalize(&cur).ok()?;
                for part in missing.iter().rev() {
                    resolved.push(part);
                }
                return Some(resolved);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                missing.push(cur.file_name()?.to_os_string());
                if !cur.pop() {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
}

fn restore_one_hide(dir: &Path, repo: &Path) -> Result<(), GuidedError> {
    let lock = match OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.join("LOCK"))
    {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // A live probe writes LOCK before MANIFEST and holds it. No LOCK means no live owner.
            if dir.join("MANIFEST").is_file() {
                return restore_hide_manifest(dir, repo);
            }
            let _ = fs::remove_file(dir.join("MANIFEST.tmp"));
            let _ = fs::remove_dir(dir);
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    if !try_lock_hide(&lock)? {
        if hide_manifest_names(dir, repo)? {
            return Err(message("refused: falsifier probe in progress"));
        }
        return Ok(());
    }
    let restored = restore_hide_manifest(dir, repo);
    drop(lock);
    restored
}

fn hide_manifest_names(dir: &Path, repo: &Path) -> Result<bool, GuidedError> {
    let text = match fs::read_to_string(dir.join("MANIFEST")) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };
    for line in records(&text) {
        let mut fields = line.split('\t');
        if fields.next() != Some("repo") {
            continue;
        }
        let Ok(recorded) = fs::canonicalize(fields.next().unwrap_or("")) else {
            return Ok(false);
        };
        return Ok(recorded.as_path() == repo);
    }
    Ok(false)
}

fn restore_hide_manifest(dir: &Path, repo: &Path) -> Result<(), GuidedError> {
    let text = match fs::read_to_string(dir.join("MANIFEST")) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let _ = fs::remove_file(dir.join("MANIFEST.tmp"));
            let _ = fs::remove_file(dir.join("LOCK"));
            let _ = fs::remove_dir(dir);
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let mut recorded = None;
    let mut pairs = Vec::new();
    for line in records(&text) {
        let mut fields = line.split('\t');
        let key = fields.next().unwrap_or("");
        let value = fields.next().unwrap_or("");
        if key == "repo" {
            recorded = Some(PathBuf::from(value));
        } else if let Ok(index) = key.parse::<usize>() {
            pairs.push((PathBuf::from(value), dir.join(index.to_string())));
        }
    }
    let Some(recorded) = recorded else {
        return Ok(());
    };
    let Ok(recorded) = fs::canonicalize(&recorded) else {
        return Ok(());
    };
    if recorded != repo {
        return Ok(());
    }
    for (original, _) in &pairs {
        if !restore_target_inside(repo, original) {
            return Err(message(
                "refused: restore target is outside the product tree",
            ));
        }
    }
    pairs.sort_by_key(|(original, _)| original.components().count());
    for (original, backup) in pairs {
        put_back(&original, &backup)
            .map_err(|err| message(format!("could not restore {}: {err}", original.display())))?;
    }
    let _ = fs::remove_file(dir.join("MANIFEST"));
    let _ = fs::remove_file(dir.join("MANIFEST.tmp"));
    let _ = fs::remove_file(dir.join("LOCK"));
    let _ = fs::remove_dir(dir);
    Ok(())
}

fn stop_falsifier_group(child: &mut Child) {
    let pgid = child.id();
    let _ = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{pgid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
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
        // `head`/`sed` are byte tools. A non-UTF-8 output byte must not abort the gate.
        let bytes = fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes);
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
    // Same byte read as the main loop. `read_to_string` drops the attempt id when the output has a non-UTF-8 byte.
    let Ok(bytes) = fs::read(path) else {
        return Ok(false);
    };
    let text = String::from_utf8_lossy(&bytes);
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
        let bytes = fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes);
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
        let body = h12(tail_from_line(&bytes, 3));
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
    if !is_regular(&result_path) {
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

fn tail_from_line(bytes: &[u8], line_no: usize) -> &[u8] {
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
    let report = match check(root, &[slug]) {
        Ok(report) => report,
        Err(err) => {
            release_closing(&dir);
            return Err(err);
        }
    };
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
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
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
    use crate::state::STATE_HEADER;
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
            if let Ok(canon) = fs::canonicalize(&self.0) {
                if let Ok(dir) = super::probe_lock_dir(&canon) {
                    let _ = fs::remove_file(dir.join(super::probe_lock_name(&canon)));
                }
            }
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
        fs::create_dir_all(root.join("reviews")).unwrap();
        fs::write(
            root.join("reviews/review.md"),
            "Judgment of alpha: the judge is not the author.\n",
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
    fn harness_porcelain_does_not_fail_a_closeable_check() {
        let _lock = env_lock();
        let _judges = unset_env("CRUCIBLE_MIN_JUDGES");
        let _kinds = unset_env("CRUCIBLE_MIN_KINDS");
        let tmp = Tmp::new();
        let repo = tmp.0.join("repo");
        let root = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        let init = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .status()
            .unwrap();
        assert!(init.success());
        let dir = root.join("items/alpha");
        fs::create_dir_all(dir.join("work")).unwrap();
        fs::create_dir_all(dir.join("evidence")).unwrap();
        fs::create_dir_all(dir.join("verdicts")).unwrap();
        fs::create_dir_all(dir.join("briefs")).unwrap();
        fs::write(dir.join("work/note.txt"), "body\n").unwrap();
        fs::write(dir.join("MAKER"), "mk1\n").unwrap();
        fs::write(
            dir.join("ITEM.md"),
            format!("{ITEM}\n## Owned files\n\n- HELLO.txt\n"),
        )
        .unwrap();
        fs::write(root.join("PROGRAM"), format!("repo: {}\n", repo.display())).unwrap();
        fs::write(
            root.join("agents.tsv"),
            "mk1\tkindA\tm\thigh\ttrue\nj1\tkindB\tm\thigh\ttrue\nj2\tkindB\tm\thigh\ttrue\n",
        )
        .unwrap();
        fs::write(repo.join("HELLO.txt"), "hello\n").unwrap();
        fs::create_dir_all(repo.join(".wm")).unwrap();
        fs::write(repo.join(".wm/FLOOR.md"), "card\n").unwrap();
        fs::write(repo.join("START.md"), "coordinator card\n").unwrap();
        fs::create_dir_all(repo.join(".grok/rules")).unwrap();
        fs::write(repo.join(".grok/rules/loop-router.md"), "router\n").unwrap();
        fs::create_dir_all(repo.join("reviews")).unwrap();
        fs::write(
            repo.join("reviews/review.md"),
            "Judgment of alpha: the judge is not the author.\n",
        )
        .unwrap();
        git(&repo, &["add", "reviews/review.md"]);
        git(
            &repo,
            &[
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "user.name=test",
                "commit",
                "-qm",
                "judgment",
            ],
        );
        let clock = FixedClock::new(0);
        let wid = workid(&root, "alpha").unwrap();
        let o1 = run(&root, &["alpha", "j1", "--", "/bin/echo", "one"], &clock).unwrap();
        let o2 = run(&root, &["alpha", "j2", "--", "/bin/echo", "two"], &clock).unwrap();
        let n1 = Path::new(o1.split_whitespace().next().unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy();
        let n2 = Path::new(o2.split_whitespace().next().unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy();
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
        // `run` hides harness paths from git status. This check must still pass
        // when those paths are listed beside the owned file.
        let _ = fs::remove_file(repo.join(".git/info/exclude"));
        let report = check(&root, &["alpha"]).unwrap();
        assert_eq!(report.status, 0, "{}", report.text);
        assert!(
            report.text.ends_with(&format!("CLOSEABLE {wid}\n")),
            "{}",
            report.text
        );
        assert!(
            !report.text.contains("product porcelain"),
            "{}",
            report.text
        );
        fs::write(repo.join("OTHER.txt"), "not owned\n").unwrap();
        let dirty = check(&root, &["alpha"]).unwrap();
        assert_eq!(dirty.status, 1, "{}", dirty.text);
        assert!(
            dirty
                .text
                .contains("FAIL product porcelain outside owned files: OTHER.txt"),
            "{}",
            dirty.text
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

    #[test]
    fn non_utf8_evidence_does_not_abort_check_or_stick_closing() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let dir = root.join("items/alpha");
        fs::create_dir_all(dir.join("work")).unwrap();
        fs::create_dir_all(dir.join("evidence")).unwrap();
        fs::create_dir_all(dir.join("verdicts")).unwrap();
        fs::write(dir.join("work/note.txt"), "body\n").unwrap();
        fs::write(dir.join("MAKER"), "mk1\n").unwrap();
        fs::write(dir.join("ITEM.md"), ITEM).unwrap();
        fs::write(
            root.join("agents.tsv"),
            "mk1\tkindA\tm\thigh\ttrue\nj1\tkindB\tm\thigh\ttrue\n",
        )
        .unwrap();
        let wid = workid(root, "alpha").unwrap();
        let mut body = format!(
            "crucible-run/1\nagent: j1\nwork-id: {wid}\nwhen: 1970-01-01T00:00:00Z\ncommand: /bin/echo x\n--- output ---\n"
        )
        .into_bytes();
        body.push(0xff);
        body.extend(b"\n--- exit 0 ---\n");
        fs::write(dir.join(format!("evidence/j1.tok.{wid}.txt")), &body).unwrap();
        let report = check(root, &["alpha"]).expect("non-utf8 evidence must not abort check");
        assert_eq!(report.status, 1, "{}", report.text);
        assert!(report.text.contains("NOT CLOSEABLE\n"), "{}", report.text);
        assert_eq!(
            close(root, &["alpha", "nope"], &FixedClock::new(0))
                .unwrap_err()
                .to_string(),
            "refused: alpha is not closeable"
        );
        assert!(
            !dir.join(".closing").exists(),
            "a close the shell would continue must not leave .closing"
        );
    }

    #[test]
    fn dotfile_evidence_does_not_make_close_stale() {
        let _lock = env_lock();
        let _judges = unset_env("CRUCIBLE_MIN_JUDGES");
        let _kinds = unset_env("CRUCIBLE_MIN_KINDS");
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let dir = root.join("items/alpha");
        fs::create_dir_all(dir.join("work")).unwrap();
        fs::create_dir_all(dir.join("evidence")).unwrap();
        fs::create_dir_all(dir.join("verdicts")).unwrap();
        fs::write(dir.join("work/note.txt"), "body\n").unwrap();
        fs::write(dir.join("MAKER"), "mk1\n").unwrap();
        fs::write(dir.join("ITEM.md"), ITEM).unwrap();
        fs::write(
            root.join("agents.tsv"),
            "mk1\tkindA\tm\thigh\ttrue\nj1\tkindB\tm\thigh\ttrue\nj2\tkindB\tm\thigh\ttrue\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("reviews")).unwrap();
        fs::write(
            root.join("reviews/review.md"),
            "Judgment of alpha: the judge is not the author.\n",
        )
        .unwrap();
        let clock = FixedClock::new(0);
        let wid = workid(root, "alpha").unwrap();
        let o1 = run(root, &["alpha", "j1", "--", "/bin/echo", "one"], &clock).unwrap();
        let o2 = run(root, &["alpha", "j2", "--", "/bin/echo", "two"], &clock).unwrap();
        let n1 = Path::new(o1.split_whitespace().next().unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy();
        let n2 = Path::new(o2.split_whitespace().next().unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy();
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
        // Glob `evidence/*` never sees this. Its stem is not the current work id.
        fs::write(dir.join("evidence/.notes.txt"), "stale-looking\n").unwrap();
        let closed = close(root, &["alpha", "kept the dotfile"], &clock).unwrap();
        assert_eq!(closed, format!("closed alpha at {wid}\n"));
        assert!(dir.join("evidence/.notes.txt").is_file());
    }

    #[test]
    fn non_utf8_task_evidence_is_still_skipped() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tREVIEW\tw1\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        let dir = root.join("items/alpha");
        fs::create_dir_all(dir.join("work")).unwrap();
        fs::create_dir_all(dir.join("evidence")).unwrap();
        fs::create_dir_all(dir.join("verdicts")).unwrap();
        fs::write(dir.join("work/note.txt"), "body\n").unwrap();
        fs::write(dir.join("MAKER"), "mk1\n").unwrap();
        fs::write(
            dir.join("ITEM.md"),
            "\
# alpha — task evidence

## Goal

Keep task evidence off the item gate.

## Non-goals

None.

## Risk

LOW

## Owned files

- src/one

## Acceptance criteria

- [ ] A1

## Focused falsifier

scripts/verify-task-dag.sh

## Expensive evidence

NONE

## Stop conditions

Stop if task evidence closes the item.
",
        )
        .unwrap();
        fs::write(
            root.join("agents.tsv"),
            "mk1\tkindA\tm\thigh\ttrue\nj1\tkindB\tm\thigh\ttrue\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("attempts/A1700.1.1")).unwrap();
        fs::write(
            root.join("attempts/A1700.1.1/meta.tsv"),
            "\
attempt_id\titem\ttask_id\twork_id\trole\tagent\tkind\tcriterion\tevidence_class\tstate\tstarted_epoch\tdeadline_epoch\tretry_of
A1700.1.1\talpha\tT1\tw\tmaker\tj1\tkindB\tA1\tFOCUSED\tRUNNING\t1\t2\t-
",
        )
        .unwrap();
        let wid = workid(root, "alpha").unwrap();
        let mut body = format!(
            "crucible-run/1\nagent: j1\nwork-id: {wid}\nattempt-id: A1700.1.1\nwhen: 1970-01-01T00:00:00Z\ncommand: /bin/echo x\n--- output ---\n"
        )
        .into_bytes();
        body.push(0xff);
        body.extend(b"\n--- exit 0 ---\n");
        fs::write(dir.join(format!("evidence/j1.tok.{wid}.txt")), body).unwrap();
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            report
                .text
                .contains(&format!("no usable evidence for {wid}")),
            "task evidence must be skipped, got {}",
            report.text
        );
    }

    fn git(repo: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn plant_two_judges(root: &Path) -> (PathBuf, String) {
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
        let clock = FixedClock::new(0);
        let wid = workid(root, "alpha").unwrap();
        let o1 = run(root, &["alpha", "j1", "--", "/bin/echo", "one"], &clock).unwrap();
        let o2 = run(root, &["alpha", "j2", "--", "/bin/echo", "two"], &clock).unwrap();
        let n1 = Path::new(o1.split_whitespace().next().unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy();
        let n2 = Path::new(o2.split_whitespace().next().unwrap())
            .file_name()
            .unwrap()
            .to_string_lossy();
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
        (dir, wid)
    }

    fn assert_judgment_refused(root: &Path, repo: &Path) {
        let report = check(root, &["alpha"]).unwrap();
        assert_eq!(report.status, 1, "{}", report.text);
        assert!(report.text.contains("NOT CLOSEABLE\n"), "{}", report.text);
        let want = format!(
            "FAIL no visible non-author judgment at {} (Judgment of alpha: the judge is not the author.)",
            repo.join("reviews").join("review.md").display()
        );
        assert!(report.text.contains(&want), "{}", report.text);
    }

    #[test]
    fn product_review_gates_close() {
        let _lock = env_lock();
        let _judges = unset_env("CRUCIBLE_MIN_JUDGES");
        let _kinds = unset_env("CRUCIBLE_MIN_KINDS");
        let tmp = Tmp::new();
        let repo = tmp.0.join("repo");
        let root = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        fs::write(repo.join("keep.txt"), "keep\n").unwrap();
        git(&repo, &["add", "keep.txt"]);
        git(
            &repo,
            &[
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "user.name=test",
                "commit",
                "-qm",
                "baseline",
            ],
        );
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("PROGRAM"), format!("repo: {}\n", repo.display())).unwrap();
        let (dir, wid) = plant_two_judges(&root);
        let clock = FixedClock::new(0);

        assert_judgment_refused(&root, &repo);
        assert_eq!(
            close(&root, &["alpha", "should not land"], &clock)
                .unwrap_err()
                .to_string(),
            "refused: alpha is not closeable"
        );
        assert!(!root.join("LESSONS.md").exists());
        assert!(!fs::read_to_string(dir.join("ITEM.md"))
            .unwrap()
            .contains("STATUS: CLOSED"));
        assert!(!dir.join(".closing").exists());

        fs::create_dir_all(root.join("reviews")).unwrap();
        let sentence = "Judgment of alpha: the judge is not the author.\n";
        fs::write(root.join("reviews/review.md"), sentence).unwrap();
        assert_judgment_refused(&root, &repo);

        fs::create_dir_all(repo.join("reviews")).unwrap();
        std::os::unix::fs::symlink(
            root.join("reviews/review.md"),
            repo.join("reviews/review.md"),
        )
        .unwrap();
        assert_judgment_refused(&root, &repo);
        fs::remove_file(repo.join("reviews/review.md")).unwrap();

        fs::write(
            repo.join("reviews/review.md"),
            "Judgment of alpha: the judge is the author.\n",
        )
        .unwrap();
        git(&repo, &["add", "reviews/review.md"]);
        git(
            &repo,
            &[
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "user.name=test",
                "commit",
                "-qm",
                "wrong sentence",
            ],
        );
        assert_judgment_refused(&root, &repo);

        fs::write(repo.join("reviews/review.md"), sentence).unwrap();
        git(&repo, &["add", "reviews/review.md"]);
        git(
            &repo,
            &[
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "user.name=test",
                "commit",
                "-qm",
                "judgment",
            ],
        );
        let report = check(&root, &["alpha"]).unwrap();
        assert_eq!(report.status, 0, "{}", report.text);
        assert!(
            report.text.ends_with(&format!("CLOSEABLE {wid}\n")),
            "{}",
            report.text
        );

        fs::write(dir.join("verdicts/mk1.md"), "VERDICT: PASS\nself\n").unwrap();
        let maker = check(&root, &["alpha"]).unwrap();
        assert_eq!(maker.status, 1, "{}", maker.text);
        assert!(
            maker
                .text
                .contains("FAIL mk1 is a maker and may not judge its own work"),
            "{}",
            maker.text
        );
        fs::remove_file(dir.join("verdicts/mk1.md")).unwrap();

        let closed = close(&root, &["alpha", "learned it"], &clock).unwrap();
        assert_eq!(closed, format!("closed alpha at {wid}\n"));
        assert!(fs::read_to_string(root.join("LESSONS.md"))
            .unwrap()
            .contains(&format!("learned it (item alpha, work {wid})")));
        assert!(fs::read_to_string(dir.join("ITEM.md"))
            .unwrap()
            .contains(&format!("STATUS: CLOSED {wid}\n")));
    }

    fn commit_reviews_symlink(repo: &Path) {
        let rel = "reviews";
        let target = ".crucible/work/reviews";
        let path = repo.join(rel);
        std::os::unix::fs::symlink(target, &path).unwrap();
        let mut child = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["hash-object", "-w", "--stdin"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(target.as_bytes())
            .unwrap();
        let hashed = child.wait_with_output().unwrap();
        assert!(
            hashed.status.success(),
            "{}",
            String::from_utf8_lossy(&hashed.stderr)
        );
        let oid = String::from_utf8(hashed.stdout).unwrap();
        git(
            repo,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("120000,{},{rel}", oid.trim()),
            ],
        );
        git(
            repo,
            &[
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "user.name=test",
                "commit",
                "-qm",
                "symlink reviews",
            ],
        );
    }

    #[test]
    fn symlinked_reviews_directory_is_not_closeable() {
        let _lock = env_lock();
        let _judges = unset_env("CRUCIBLE_MIN_JUDGES");
        let _kinds = unset_env("CRUCIBLE_MIN_KINDS");
        let tmp = Tmp::new();
        let repo = tmp.0.join("repo");
        let root = repo.join(".crucible").join("work");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        fs::write(repo.join("keep.txt"), "keep\n").unwrap();
        git(&repo, &["add", "keep.txt"]);
        git(
            &repo,
            &[
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "user.name=test",
                "commit",
                "-qm",
                "baseline",
            ],
        );
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("PROGRAM"), format!("repo: {}\n", repo.display())).unwrap();
        let (dir, _wid) = plant_two_judges(&root);
        let clock = FixedClock::new(0);
        fs::create_dir_all(root.join("reviews")).unwrap();
        fs::write(
            root.join("reviews/review.md"),
            "Judgment of alpha: the judge is not the author.\n",
        )
        .unwrap();
        assert_judgment_refused(&root, &repo);

        commit_reviews_symlink(&repo);
        let porcelain = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["status", "--porcelain"])
            .output()
            .unwrap();
        let porcelain = String::from_utf8(porcelain.stdout).unwrap();
        assert!(
            porcelain.is_empty(),
            "fixture porcelain not clean:\n{porcelain}"
        );
        let report = check(&root, &["alpha"]).unwrap();
        assert_eq!(
            report.status, 1,
            "symlinked reviews directory was closeable:\n{}",
            report.text
        );
        assert!(report.text.contains("NOT CLOSEABLE\n"), "{}", report.text);
        let want = format!(
            "FAIL no visible non-author judgment at {} (Judgment of alpha: the judge is not the author.)",
            repo.join("reviews").join("review.md").display()
        );
        assert!(report.text.contains(&want), "{}", report.text);
        let refused = close(&root, &["alpha", "should not land"], &clock).unwrap_err();
        assert_eq!(refused.to_string(), "refused: alpha is not closeable");
        assert!(!root.join("LESSONS.md").exists());
        assert!(!fs::read_to_string(dir.join("ITEM.md"))
            .unwrap()
            .contains("STATUS: CLOSED"));
        assert!(!dir.join(".closing").exists());
    }

    fn discriminate_item(falsifier: &str) -> String {
        format!(
            "\
# alpha — discriminate

## Goal

Show the falsifier fails when the behavior is gone.

## Non-goals

No other gate.

## Risk

LOW

## Owned files

- src/kept.txt

## Acceptance criteria

- [ ] A1

## Focused falsifier

{falsifier}

## Expensive evidence

NONE

## Stop conditions

Stop if the owned file is not restored.
"
        )
    }

    fn plant_discriminate(root: &Path, falsifier: &str, create_file: bool) {
        fs::write(root.join("PROGRAM"), "lifecycle: managed\n").unwrap();
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tREVIEW\tw1\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        let dir = root.join("items/alpha");
        fs::create_dir_all(dir.join("work")).unwrap();
        fs::write(dir.join("work/note.txt"), "body\n").unwrap();
        fs::write(dir.join("ITEM.md"), discriminate_item(falsifier)).unwrap();
        if create_file {
            fs::create_dir_all(root.join("src")).unwrap();
            fs::write(root.join("src/kept.txt"), "live\n").unwrap();
        }
    }

    #[test]
    fn discriminate_refuses_when_the_falsifier_still_passes() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/usr/bin/true", true);
        let err = check(root, &["alpha"]).unwrap_err();
        assert_eq!(err.to_string(), "refused: falsifier does not discriminate");
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn discriminate_allows_a_falsifier_that_fails_after_removal() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/bin/test -f src/kept.txt", true);
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn discriminate_skips_when_the_owned_file_is_absent() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/usr/bin/true", false);
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
    }

    struct BoundGuard;
    impl BoundGuard {
        fn set(bound: std::time::Duration) -> Self {
            super::FALSIFIER_BOUND_OVERRIDE.set(Some(bound));
            Self
        }
    }
    impl Drop for BoundGuard {
        fn drop(&mut self) {
            super::FALSIFIER_BOUND_OVERRIDE.set(None);
        }
    }

    fn write_script(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    fn discriminate_restores_when_the_command_recreates_the_directory() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "tools/recreate.sh", false);
        fs::create_dir_all(root.join("src/widget")).unwrap();
        fs::write(root.join("src/widget/note"), "live\n").unwrap();
        let item = fs::read_to_string(root.join("items/alpha/ITEM.md")).unwrap();
        fs::write(
            root.join("items/alpha/ITEM.md"),
            item.replace("- src/kept.txt", "- src/widget"),
        )
        .unwrap();
        write_script(
            &root.join("tools/recreate.sh"),
            "#!/bin/sh\nmkdir -p src/widget\nprintf 'fake\\n' > src/widget/note\nprintf 'extra\\n' > src/widget/extra\nexit 1\n",
        );
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        assert_eq!(
            fs::read_to_string(root.join("src/widget/note")).unwrap(),
            "live\n"
        );
        assert!(!root.join("src/widget/extra").exists());
    }

    #[test]
    fn discriminate_stops_the_command_group_before_restore() {
        let _bound = BoundGuard::set(std::time::Duration::from_millis(400));
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "tools/linger.sh", true);
        write_script(
            &root.join("tools/linger.sh"),
            "#!/bin/sh\n(sleep 3; printf 'late\\n' > src/kept.txt) &\necho $! > tools/child.pid\nsleep 8\n",
        );
        let started = std::time::Instant::now();
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "check waited for the falsifier: {:?}",
            started.elapsed()
        );
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
        let pid = fs::read_to_string(root.join("tools/child.pid"))
            .unwrap()
            .trim()
            .to_string();
        let alive = std::process::Command::new("/bin/kill")
            .args(["-0", "--", &pid])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!alive.success(), "writer {pid} still alive");
    }

    #[test]
    fn discriminate_uses_the_product_repo() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let product = root.join("product");
        fs::create_dir_all(product.join("src")).unwrap();
        fs::write(product.join("src/kept.txt"), "live\n").unwrap();
        plant_discriminate(root, "/usr/bin/true", false);
        fs::write(
            root.join("PROGRAM"),
            format!("repo: {}\nlifecycle: managed\n", product.display()),
        )
        .unwrap();
        let err = check(root, &["alpha"]).unwrap_err();
        assert_eq!(err.to_string(), "refused: falsifier does not discriminate");
        assert_eq!(
            fs::read_to_string(product.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn discriminate_does_not_move_through_a_symlinked_parent() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let outside = tmp.0.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("kept.txt"), "live\n").unwrap();
        plant_discriminate(root, "/usr/bin/true", false);
        std::os::unix::fs::symlink(&outside, root.join("src")).unwrap();
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        assert_eq!(
            fs::read_to_string(outside.join("kept.txt")).unwrap(),
            "live\n"
        );
        assert!(root
            .join("src")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn discriminate_refuses_when_the_falsifier_does_not_run() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/no/such/falsifier", true);
        let err = check(root, &["alpha"]).unwrap_err();
        assert_eq!(err.to_string(), "refused: falsifier did not run");
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn discriminate_refuses_when_the_falsifier_is_not_executable() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        let script = root.join("tools/not-exec.sh");
        write_script(&script, "#!/bin/sh\nexit 0\n");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&script, perms).unwrap();
        plant_discriminate(root, script.to_str().unwrap(), true);
        let err = check(root, &["alpha"]).unwrap_err();
        assert_eq!(err.to_string(), "refused: falsifier did not run");
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn discriminate_skips_the_probe_before_review() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/usr/bin/true", true);
        fs::write(
            root.join("STATE.tsv"),
            format!("{STATE_HEADER}\nalpha\tACTIVE\tBUILD\tw1\tLOW\t-\t-\t1\n"),
        )
        .unwrap();
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            report.text.contains("FAIL managed item must be in REVIEW"),
            "{}",
            report.text
        );
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn discriminate_restores_a_hide_left_by_an_interrupted_check() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/bin/test -f src/kept.txt", true);
        let canon = fs::canonicalize(root).unwrap();
        let hide = canon.parent().unwrap().join(format!(
            ".crucible-falsifier-hide-interrupt-{}",
            std::process::id()
        ));
        fs::create_dir(&hide).unwrap();
        fs::rename(root.join("src/kept.txt"), hide.join("0")).unwrap();
        fs::write(
            hide.join("MANIFEST"),
            format!(
                "repo\t{}\n0\t{}\n",
                canon.display(),
                canon.join("src/kept.txt").display()
            ),
        )
        .unwrap();
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        assert!(
            !report.text.contains("falsifier did not run"),
            "{}",
            report.text
        );
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
        assert!(
            !hide.exists(),
            "stranded hide was left at {}",
            hide.display()
        );
    }

    #[test]
    fn discriminate_stops_when_the_process_receives_sigterm() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "tools/linger.sh", true);
        write_script(
            &root.join("tools/linger.sh"),
            "#!/bin/sh\nprintf ready\\n > tools/ready\nwhile true; do sleep 1; done\n",
        );
        let check_root = root.to_path_buf();
        let worker = thread::spawn(move || check(&check_root, &["alpha"]));
        let ready = root.join("tools/ready");
        let started = std::time::Instant::now();
        while !ready.is_file() {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(3),
                "falsifier did not start"
            );
            thread::sleep(std::time::Duration::from_millis(20));
        }
        unsafe {
            libc::kill(std::process::id() as libc::pid_t, libc::SIGTERM);
        }
        let err = worker.join().expect("check thread").unwrap_err();
        assert_eq!(err.to_string(), "refused: check interrupted");
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn put_back_leaves_the_live_file_when_the_backup_is_gone() {
        let tmp = Tmp::new();
        let original = tmp.0.join("kept.txt");
        fs::write(&original, "live\n").unwrap();
        super::put_back(&original, &tmp.0.join("missing-backup")).unwrap();
        assert_eq!(fs::read_to_string(&original).unwrap(), "live\n");
    }

    #[test]
    fn discriminate_a_second_check_does_not_take_the_hidden_file() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "tools/linger.sh", true);
        write_script(
            &root.join("tools/linger.sh"),
            "#!/bin/sh\nprintf ready\\n > tools/ready\nwhile true; do sleep 1; done\n",
        );
        let check_root = root.to_path_buf();
        let worker = thread::spawn(move || {
            let _bound = BoundGuard::set(std::time::Duration::from_secs(8));
            check(&check_root, &["alpha"])
        });
        let ready = root.join("tools/ready");
        let started = std::time::Instant::now();
        while !ready.is_file() {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(3),
                "falsifier did not start"
            );
            thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            !root.join("src/kept.txt").exists(),
            "probe did not hide the owned file"
        );
        let second = std::time::Instant::now();
        let err = close(
            root,
            &["alpha", "must not close during the probe"],
            &FixedClock::new(0),
        )
        .unwrap_err();
        assert!(
            second.elapsed() < std::time::Duration::from_secs(2),
            "close waited on the probe: {:?}",
            second.elapsed()
        );
        assert_eq!(err.to_string(), "refused: falsifier probe in progress");
        assert!(!root.join("items/alpha/.closing").exists());
        assert!(
            !root.join("src/kept.txt").exists(),
            "close moved the owned file back while the probe held it"
        );
        unsafe {
            libc::kill(std::process::id() as libc::pid_t, libc::SIGTERM);
        }
        let err = worker.join().expect("check thread").unwrap_err();
        assert_eq!(err.to_string(), "refused: check interrupted");
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn close_releases_the_lock_when_the_falsifier_does_not_discriminate() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/usr/bin/true", true);
        let err = close(
            root,
            &["alpha", "the falsifier still passed"],
            &FixedClock::new(0),
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "refused: falsifier does not discriminate");
        assert!(!root.join("items/alpha/.closing").exists());
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn discriminate_refuses_an_owned_path_that_cannot_be_recorded() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/usr/bin/true", false);
        fs::create_dir_all(root.join("src")).unwrap();
        let owned = root.join("src").join("a\tb.txt");
        fs::write(&owned, "live\n").unwrap();
        let item = fs::read_to_string(root.join("items/alpha/ITEM.md")).unwrap();
        fs::write(
            root.join("items/alpha/ITEM.md"),
            item.replace("- src/kept.txt", "- src/a\tb.txt"),
        )
        .unwrap();
        let err = check(root, &["alpha"]).unwrap_err();
        assert_eq!(err.to_string(), "refused: owned path cannot be recorded");
        assert_eq!(fs::read_to_string(&owned).unwrap(), "live\n");
    }

    #[test]
    fn discriminate_does_not_restore_a_path_outside_the_product_tree() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/bin/test -f src/kept.txt", true);
        let canon = fs::canonicalize(root).unwrap();
        let parent = canon.parent().unwrap();
        let outside = parent.join(format!("crucible-outside-target-{}", std::process::id()));
        let hide = parent.join(format!(
            ".crucible-falsifier-hide-outside-{}",
            std::process::id()
        ));
        let _cleanup = RemoveOnDrop(vec![outside.clone(), hide.clone()]);
        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(&hide);
        fs::write(&outside, "safe\n").unwrap();
        fs::create_dir(&hide).unwrap();
        fs::write(hide.join("0"), "pwned\n").unwrap();
        fs::write(
            hide.join("MANIFEST"),
            format!("repo\t{}\n0\t{}\n", canon.display(), outside.display()),
        )
        .unwrap();
        let result = check(root, &["alpha"]);
        let outside_body = fs::read_to_string(&outside).unwrap_or_default();
        let kept_body = fs::read_to_string(root.join("src/kept.txt")).unwrap_or_default();
        let hide_left = hide.exists();
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "refused: restore target is outside the product tree"
        );
        assert_eq!(outside_body, "safe\n");
        assert_eq!(kept_body, "live\n");
        assert!(hide_left, "outside refusal removed the hide directory");
        let _ = fs::remove_dir_all(&hide);
        let _ = fs::remove_file(&outside);
    }

    #[test]
    fn discriminate_restores_a_hide_named_through_a_symlink_of_the_same_tree() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/bin/test -f src/kept.txt", true);
        let canon = fs::canonicalize(root).unwrap();
        let parent = canon.parent().unwrap();
        let link = parent.join(format!("crucible-repo-link-{}", std::process::id()));
        let hide = parent.join(format!(
            ".crucible-falsifier-hide-alias-{}",
            std::process::id()
        ));
        let _cleanup = RemoveOnDrop(vec![link.clone(), hide.clone()]);
        let _ = fs::remove_file(&link);
        let _ = fs::remove_dir_all(&hide);
        std::os::unix::fs::symlink(&canon, &link).unwrap();
        fs::create_dir(&hide).unwrap();
        fs::rename(root.join("src/kept.txt"), hide.join("0")).unwrap();
        fs::write(
            hide.join("MANIFEST"),
            format!(
                "repo\t{}\n0\t{}\n",
                canon.display(),
                link.join("src/kept.txt").display()
            ),
        )
        .unwrap();
        let result = check(root, &["alpha"]);
        let kept_body = fs::read_to_string(root.join("src/kept.txt")).unwrap_or_default();
        let hide_left = hide.exists();
        let report = result.expect("same-tree alias must restore");
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        assert_eq!(kept_body, "live\n");
        assert!(!hide_left, "stranded hide was left at {}", hide.display());
        let _ = fs::remove_dir_all(&hide);
        let _ = fs::remove_file(&link);
    }

    #[test]
    fn discriminate_does_not_follow_a_symlink_outside_the_product_tree() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/bin/test -f src/kept.txt", true);
        let canon = fs::canonicalize(root).unwrap();
        let parent = canon.parent().unwrap();
        let outside = parent.join(format!("crucible-escape-dir-{}", std::process::id()));
        let link = canon.join("src/escape");
        let hide = parent.join(format!(
            ".crucible-falsifier-hide-escape-{}",
            std::process::id()
        ));
        let _cleanup = RemoveOnDrop(vec![outside.clone(), hide.clone()]);
        let _ = fs::remove_dir_all(&outside);
        let _ = fs::remove_dir_all(&hide);
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("victim"), "safe\n").unwrap();
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        fs::create_dir(&hide).unwrap();
        fs::write(hide.join("0"), "pwned\n").unwrap();
        fs::write(
            hide.join("MANIFEST"),
            format!(
                "repo\t{}\n0\t{}\n",
                canon.display(),
                link.join("victim").display()
            ),
        )
        .unwrap();
        let result = check(root, &["alpha"]);
        let victim_body = fs::read_to_string(outside.join("victim")).unwrap_or_default();
        let kept_body = fs::read_to_string(root.join("src/kept.txt")).unwrap_or_default();
        let hide_left = hide.exists();
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "refused: restore target is outside the product tree"
        );
        assert_eq!(victim_body, "safe\n");
        assert_eq!(kept_body, "live\n");
        assert!(hide_left, "outside refusal removed the hide directory");
        let _ = fs::remove_dir_all(&hide);
        let _ = fs::remove_file(&link);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn discriminate_does_not_let_dotdot_escape_through_a_symlink() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/bin/test -f src/kept.txt", true);
        let canon = fs::canonicalize(root).unwrap();
        let parent = canon.parent().unwrap();
        let outside = parent.join(format!("crucible-dotdot-dir-{}", std::process::id()));
        let victim = parent.join(format!("crucible-dotdot-victim-{}", std::process::id()));
        let hide = parent.join(format!(
            ".crucible-falsifier-hide-dotdot-{}",
            std::process::id()
        ));
        let link = canon.join("src/escape");
        let _cleanup = RemoveOnDrop(vec![outside.clone(), victim.clone(), hide.clone()]);
        let _ = fs::remove_dir_all(&outside);
        let _ = fs::remove_file(&victim);
        let _ = fs::remove_dir_all(&hide);
        fs::create_dir(&outside).unwrap();
        fs::write(&victim, "safe\n").unwrap();
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        fs::create_dir(&hide).unwrap();
        fs::write(hide.join("0"), "pwned\n").unwrap();
        let original = canon
            .join("src")
            .join("escape")
            .join("..")
            .join(victim.file_name().unwrap());
        fs::write(
            hide.join("MANIFEST"),
            format!("repo\t{}\n0\t{}\n", canon.display(), original.display()),
        )
        .unwrap();
        let result = check(root, &["alpha"]);
        let victim_body = fs::read_to_string(&victim).unwrap_or_default();
        let kept_body = fs::read_to_string(root.join("src/kept.txt")).unwrap_or_default();
        let hide_left = hide.exists();
        let _ = fs::remove_dir_all(&hide);
        let _ = fs::remove_file(&victim);
        let _ = fs::remove_dir_all(&outside);
        let _ = fs::remove_file(&link);
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "refused: restore target is outside the product tree"
        );
        assert_eq!(victim_body, "safe\n");
        assert_eq!(kept_body, "live\n");
        assert!(hide_left, "outside refusal removed the hide directory");
    }

    #[test]
    fn discriminate_restores_a_dotdot_that_stays_inside_the_tree() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/bin/test -f src/kept.txt", true);
        let canon = fs::canonicalize(root).unwrap();
        let parent = canon.parent().unwrap();
        let hide = parent.join(format!(
            ".crucible-falsifier-hide-inside-dot-{}",
            std::process::id()
        ));
        let _cleanup = RemoveOnDrop(vec![hide.clone()]);
        let _ = fs::remove_dir_all(&hide);
        fs::create_dir(&hide).unwrap();
        fs::rename(root.join("src/kept.txt"), hide.join("0")).unwrap();
        let original = canon.join("src").join("..").join("src").join("kept.txt");
        fs::write(
            hide.join("MANIFEST"),
            format!("repo\t{}\n0\t{}\n", canon.display(), original.display()),
        )
        .unwrap();
        let result = check(root, &["alpha"]);
        let kept_body = fs::read_to_string(root.join("src/kept.txt")).unwrap_or_default();
        let hide_left = hide.exists();
        let report = result.expect("a dotdot path inside the tree must restore");
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        assert_eq!(kept_body, "live\n");
        assert!(!hide_left, "stranded hide was left at {}", hide.display());
        let _ = fs::remove_dir_all(&hide);
    }

    #[test]
    fn discriminate_does_not_restore_through_a_parent_the_falsifier_replaced() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "tools/swap.sh", true);
        let canon = fs::canonicalize(root).unwrap();
        let parent = canon.parent().unwrap();
        let outside = parent.join(format!("crucible-swap-outside-{}", std::process::id()));
        let _cleanup = RemoveOnDrop(vec![outside.clone()]);
        let _hide = HidePrefixCleanup(parent.to_path_buf());
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir(&outside).unwrap();
        write_script(
            &root.join("tools/swap.sh"),
            &format!(
                "#!/bin/sh\nrmdir src\nln -s '{}' src\nexit 1\n",
                outside.display()
            ),
        );
        let result = check(root, &["alpha"]);
        let leaked = outside.join("kept.txt");
        let leaked_body = fs::read_to_string(&leaked).unwrap_or_default();
        let mut saved = String::new();
        if let Ok(entries) = fs::read_dir(parent) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name
                    .to_string_lossy()
                    .starts_with(".crucible-falsifier-hide-")
                {
                    if let Ok(text) = fs::read_to_string(entry.path().join("0")) {
                        saved = text;
                    }
                }
            }
        }
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "refused: restore target is outside the product tree"
        );
        assert!(
            leaked_body.is_empty() && !leaked.exists(),
            "restore followed the replaced parent: {leaked_body:?}"
        );
        assert_eq!(saved, "live\n");
    }

    #[test]
    fn discriminate_stops_a_child_that_outlives_the_falsifier() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "tools/bg.sh", true);
        write_script(
            &root.join("tools/bg.sh"),
            "#!/bin/sh\n(sleep 1; printf 'late\\n' > src/kept.txt) &\necho $! > tools/child.pid\nexit 1\n",
        );
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        std::thread::sleep(std::time::Duration::from_millis(1500));
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
        let pid = fs::read_to_string(root.join("tools/child.pid"))
            .unwrap()
            .trim()
            .to_string();
        let alive = std::process::Command::new("/bin/kill")
            .args(["-0", "--", &pid])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!alive.success(), "writer {pid} still alive");
    }

    #[test]
    fn discriminate_puts_back_bytes_the_falsifier_rewrote() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "tools/tamper.sh", true);
        write_script(
            &root.join("tools/tamper.sh"),
            "#!/bin/sh\nfor d in ../.crucible-falsifier-hide-*; do\n  if [ -f \"$d/0\" ]; then printf 'tampered\\n' > \"$d/0\"; fi\ndone\nexit 1\n",
        );
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    #[test]
    fn discriminate_does_not_write_the_snapshot_through_a_planted_name() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "tools/tamper.sh", true);
        let outside = std::env::temp_dir().join(format!(
            "crucible-restore-outside-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&outside).unwrap();
        let _cleanup = RemoveOnDrop(vec![outside.clone()]);
        let secret = outside.join("secret");
        fs::write(&secret, "secret\n").unwrap();
        std::os::unix::fs::symlink(
            &secret,
            root.join(format!("src/.crucible-restore-{}-0", std::process::id())),
        )
        .unwrap();
        write_script(
            &root.join("tools/tamper.sh"),
            "#!/bin/sh\nfor d in ../.crucible-falsifier-hide-*; do\n  if [ -f \"$d/0\" ]; then printf 'tampered\\n' > \"$d/0\"; fi\ndone\nexit 1\n",
        );
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        let meta = fs::symlink_metadata(root.join("src/kept.txt")).unwrap();
        assert!(meta.is_file() && !meta.file_type().is_symlink());
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
        assert_eq!(fs::read_to_string(&secret).unwrap(), "secret\n");
    }

    #[test]
    fn discriminate_replaces_a_directory_the_falsifier_swapped() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "tools/swapdir.sh", false);
        fs::create_dir_all(root.join("src/widget")).unwrap();
        fs::write(root.join("src/widget/note"), "live\n").unwrap();
        let item = fs::read_to_string(root.join("items/alpha/ITEM.md")).unwrap();
        fs::write(
            root.join("items/alpha/ITEM.md"),
            item.replace("- src/kept.txt", "- src/widget"),
        )
        .unwrap();
        let outside = std::env::temp_dir().join(format!(
            "crucible-dir-outside-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("marker"), "keep\n").unwrap();
        let _cleanup = RemoveOnDrop(vec![outside.clone()]);
        write_script(
            &root.join("tools/swapdir.sh"),
            &format!(
                "#!/bin/sh\nrm -rf src/widget\nln -s '{}' src/widget\nexit 1\n",
                outside.display()
            ),
        );
        let report = check(root, &["alpha"]).unwrap();
        assert!(
            !report.text.contains("falsifier does not discriminate"),
            "{}",
            report.text
        );
        let meta = fs::symlink_metadata(root.join("src/widget")).unwrap();
        assert!(meta.is_dir() && !meta.file_type().is_symlink());
        assert_eq!(
            fs::read_to_string(root.join("src/widget/note")).unwrap(),
            "live\n"
        );
        assert_eq!(
            fs::read_to_string(outside.join("marker")).unwrap(),
            "keep\n"
        );
        assert!(!outside.join("note").exists());
    }

    #[test]
    fn discriminate_a_second_check_refuses_before_either_moves_a_file() {
        let tmp = Tmp::new();
        let root = tmp.0.as_path();
        plant_discriminate(root, "/usr/bin/true", true);
        let held = super::acquire_probe_lock(root).unwrap();
        let check_root = root.to_path_buf();
        let err = thread::spawn(move || check(&check_root, &["alpha"]))
            .join()
            .expect("check thread")
            .unwrap_err();
        assert_eq!(err.to_string(), "refused: falsifier probe in progress");
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
        drop(held);
        let err = check(root, &["alpha"]).unwrap_err();
        assert_eq!(err.to_string(), "refused: falsifier does not discriminate");
    }

    #[test]
    fn discriminate_refuses_a_product_path_that_cannot_be_recorded() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let mut raw = b"/tmp/crucible".to_vec();
        raw.push(0xff);
        assert!(!super::path_can_be_recorded(&PathBuf::from(
            OsString::from_vec(raw)
        )));
        let root = std::env::temp_dir().join(format!(
            "crucible\ttab-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let _cleanup = RemoveOnDrop(vec![root.clone()]);
        plant_discriminate(&root, "/usr/bin/true", true);
        let err = check(&root, &["alpha"]).unwrap_err();
        assert_eq!(err.to_string(), "refused: product path cannot be recorded");
        assert_eq!(
            fs::read_to_string(root.join("src/kept.txt")).unwrap(),
            "live\n"
        );
    }

    struct HidePrefixCleanup(PathBuf);
    impl Drop for HidePrefixCleanup {
        fn drop(&mut self) {
            let Ok(entries) = fs::read_dir(&self.0) else {
                return;
            };
            let prefix = format!(".crucible-falsifier-hide-{}-", std::process::id());
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with(&prefix) {
                    let _ = fs::remove_dir_all(entry.path());
                }
            }
        }
    }

    struct RemoveOnDrop(Vec<PathBuf>);
    impl Drop for RemoveOnDrop {
        fn drop(&mut self) {
            for path in &self.0 {
                let _ = fs::remove_dir_all(path);
                let _ = fs::remove_file(path);
            }
        }
    }
}
