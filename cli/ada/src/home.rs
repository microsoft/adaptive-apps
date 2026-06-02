//! Resolution of `$ADA_HOME` — the on-disk root for `ada`'s bundled
//! artifacts.
//!
//! The installation scripts (`cli/install.sh`, `cli/install.ps1`) lay
//! the release archive out as:
//!
//! ```text
//! $ADA_HOME/                 # default: ~/.adaptive
//!   bin/
//!     ada                    # this binary (ada.exe on windows)
//!   radius/
//!     app.bicep
//!     local-env.bicep
//!     aks-env.bicep
//!     bicepconfig.json
//!     types.tgz
//!     resource-types/types.yaml
//!     recipes/...
//! ```
//!
//! Resolution order:
//! 1. `$ADA_HOME` if set and non-empty.
//! 2. `$HOME/.adaptive` (or `%USERPROFILE%\.adaptive` on Windows).
//!
//! Nothing here creates directories — callers that need a guaranteed
//! path (e.g. `ada init`) call [`ensure`] explicitly.

use std::path::PathBuf;

use anyhow::{anyhow, Result};

/// Environment variable consulted before falling back to `~/.adaptive`.
pub const ENV_VAR: &str = "ADA_HOME";

/// Default directory name created under the user's home directory.
pub const DEFAULT_DIR: &str = ".adaptive";

/// Resolve `$ADA_HOME` without touching the filesystem.
pub fn root() -> Result<PathBuf> {
    if let Ok(explicit) = std::env::var(ENV_VAR) {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let home = home_dir()
        .ok_or_else(|| anyhow!("could not resolve user home directory; set {ENV_VAR} explicitly"))?;
    Ok(home.join(DEFAULT_DIR))
}

/// Path to the bundled Radius asset tree (`$ADA_HOME/radius`).
pub fn radius_root() -> Result<PathBuf> {
    Ok(root()?.join("radius"))
}

/// Path to the bundled binaries directory (`$ADA_HOME/bin`).
#[allow(dead_code)]
pub fn bin_dir() -> Result<PathBuf> {
    Ok(root()?.join("bin"))
}

/// Ensure `$ADA_HOME`, `$ADA_HOME/bin`, and `$ADA_HOME/radius` exist.
/// Returns the resolved `$ADA_HOME`.
pub fn ensure() -> Result<PathBuf> {
    let root = root()?;
    std::fs::create_dir_all(root.join("bin"))?;
    std::fs::create_dir_all(root.join("radius"))?;
    Ok(root)
}

fn home_dir() -> Option<PathBuf> {
    if let Ok(h) = std::env::var("HOME") {
        if !h.is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    if let Ok(h) = std::env::var("USERPROFILE") {
        if !h.is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    None
}
