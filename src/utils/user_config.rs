//! Paths and atomic writes for local user preferences.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(crate) fn directory() -> Result<PathBuf, String> {
    let env_path = |name| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    #[cfg(target_os = "windows")]
    let base = env_path("APPDATA");
    #[cfg(target_os = "macos")]
    let base = env_path("HOME").map(|path| path.join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = env_path("XDG_CONFIG_HOME")
        .filter(|path| path.is_absolute())
        .or_else(|| env_path("HOME").map(|path| path.join(".config")));
    base.filter(|path| path.is_absolute())
        .map(|path| path.join("mittens"))
        .ok_or_else(|| "cannot determine an absolute user config directory".into())
}

pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let temporary = parent.join(format!(".mittens-{}.tmp", uuid::Uuid::new_v4()));
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| {
            format!(
                "cannot create temporary file in {}: {error}",
                parent.display()
            )
        })?;
    let result = (|| {
        output.write_all(data)?;
        output.sync_all()?;
        drop(output);
        fs::rename(&temporary, path)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(format!("cannot save {}: {error}", path.display()));
    }
    Ok(())
}
