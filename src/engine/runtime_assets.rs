//! Optional application-owned root for packaged engine assets.
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Configure the shared packaged asset root before constructing a Universe.
/// Repeated calls for the same root are harmless; changing it is an error.
pub fn set_root(root: &Path) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("asset root: {e}"))?;
    if let Some(existing) = ROOT.get() {
        return if existing == &root {
            Ok(())
        } else {
            Err("asset root already configured".into())
        };
    }
    ROOT.set(root)
        .map_err(|_| "asset root already configured".to_string())
}

pub fn path(relative: &str) -> PathBuf {
    ROOT.get().map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join(relative),
        |root| root.join(relative),
    )
}

pub fn configured() -> bool {
    ROOT.get().is_some()
}
