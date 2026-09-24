use std::fs;
use std::path::Path;

use crate::{message, records, GuidedError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleMode {
    ItemFile,
    Managed,
}

impl LifecycleMode {
    pub fn as_str(self) -> &'static str {
        match self {
            LifecycleMode::ItemFile => "item-file",
            LifecycleMode::Managed => "managed",
        }
    }
}

impl std::fmt::Display for LifecycleMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// First `lifecycle: ` value. A missing file or line is `item-file`.
pub fn lifecycle_mode(root: &Path) -> Result<LifecycleMode, GuidedError> {
    let Ok(text) = fs::read_to_string(root.join("PROGRAM")) else {
        return Ok(LifecycleMode::ItemFile);
    };
    let Some(mode) = records(&text)
        .into_iter()
        .find_map(|line| line.strip_prefix("lifecycle: "))
    else {
        return Ok(LifecycleMode::ItemFile);
    };
    match mode {
        "" => Ok(LifecycleMode::ItemFile),
        "managed" => Ok(LifecycleMode::Managed),
        other => Err(message(format!("unsupported lifecycle behavior: {other}"))),
    }
}

pub fn uses_managed_lifecycle(root: &Path) -> Result<bool, GuidedError> {
    Ok(lifecycle_mode(root)? == LifecycleMode::Managed)
}

/// True only for an exact `cycle: guided` line. A missing file is false.
pub fn uses_guided_cycle(root: &Path) -> Result<bool, GuidedError> {
    let Ok(text) = fs::read_to_string(root.join("PROGRAM")) else {
        return Ok(false);
    };
    Ok(records(&text).contains(&"cycle: guided"))
}
