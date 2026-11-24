//! Explicit, synchronous persistence for the user's input preferences.
use super::super::xr_settings_system::calibration_file::{config_file, local_path, write_atomically};
use crate::engine::ecs::component::XrStickMode;
use std::{fs, path::PathBuf};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InputSettings {
    version: u32,
    xr_stick_mode: String,
}

/// Shared across scenes; never depends on the process working directory.
pub fn default_file() -> Result<PathBuf, String> {
    config_file("input-settings.json")
}

/// A missing file means no saved preference. Other read/format/version
/// errors are reported.
pub fn load(file: &str) -> Result<Option<XrStickMode>, String> {
    let path = local_path(file)?;
    let data = match fs::read(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let record: InputSettings = serde_json::from_slice(&data)
        .map_err(|e| format!("invalid input settings {}: {e}", path.display()))?;
    if record.version != 1 {
        return Err(format!("unsupported input settings version {}", record.version));
    }
    XrStickMode::parse(&record.xr_stick_mode)
        .map(Some)
        .ok_or_else(|| format!("unknown stick mode {:?}", record.xr_stick_mode))
}

pub fn save(file: &str, mode: XrStickMode) -> Result<(), String> {
    let path = local_path(file)?;
    let data = serde_json::to_vec_pretty(&InputSettings {
        version: 1,
        xr_stick_mode: mode.as_str().into(),
    })
    .map_err(|e| e.to_string())?;
    write_atomically(&path, &data, "input-settings")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_settings_roundtrip_missing_file_and_invalid_records() {
        let dir = std::env::temp_dir().join(format!("mittens-input-{}", uuid::Uuid::new_v4()));
        let file = dir.join("nested/input settings.json");
        let file = file.to_str().unwrap();
        assert_eq!(load(file).unwrap(), None);
        for mode in [XrStickMode::Turn, XrStickMode::Strafe] {
            save(file, mode).unwrap();
            assert_eq!(load(file).unwrap(), Some(mode));
        }
        for record in [
            "bad JSON",
            r#"{"version":2,"xr_stick_mode":"turn"}"#,
            r#"{"version":1,"xr_stick_mode":"sideways"}"#,
            r#"{"version":1}"#,
        ] {
            fs::write(file, record).unwrap();
            assert!(load(file).is_err(), "{record}");
        }
        assert!(load("https://example.com/input.json").is_err());
        assert!(default_file().unwrap().ends_with("mittens/input-settings.json"));
        let _ = fs::remove_dir_all(&dir);
    }
}
