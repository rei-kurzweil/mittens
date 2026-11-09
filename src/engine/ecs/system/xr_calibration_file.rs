//! Explicit, synchronous persistence for the shared user floor calibration.
use std::{fs, io::Write, path::PathBuf};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Calibration {
    version: u32,
    floor_correction_meters: f64,
}

pub(super) fn validate(meters: f64) -> Result<(), &'static str> {
    if !meters.is_finite() || !(-0.5..=0.5).contains(&meters) {
        return Err("floor correction must be finite and between -0.5 and +0.5 metres");
    }
    Ok(())
}

/// Shared across scenes; never depends on the process working directory.
pub fn default_file() -> Result<PathBuf, String> {
    let env_path = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    #[cfg(target_os = "windows")]
    let base = env_path("APPDATA");
    #[cfg(target_os = "macos")]
    let base = env_path("HOME").map(|p| p.join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = env_path("XDG_CONFIG_HOME")
        .filter(|p| p.is_absolute())
        .or_else(|| env_path("HOME").map(|p| p.join(".config")));
    let base = base
        .filter(|p| p.is_absolute())
        .ok_or("cannot determine an absolute user config directory")?;
    Ok(base.join("mittens/xr-calibration.json"))
}

/// Local filenames or local file URLs only. URL parsing handles percent escapes.
pub fn local_path(file: &str) -> Result<PathBuf, String> {
    if file.is_empty() {
        return Err("calibration filename must not be empty".into());
    }
    if file.starts_with("file:") {
        let url = reqwest::Url::parse(file).map_err(|e| e.to_string())?;
        if url.host_str().is_some_and(|host| host != "localhost") {
            return Err("expected a local file URL".into());
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err("calibration file URL must not contain a query or fragment".into());
        }
        return url
            .to_file_path()
            .map_err(|_| "expected a local file URL".into());
    }
    // Preserve Windows drive paths without accepting remote URL schemes.
    if file.contains(':') && !(cfg!(windows) && file.as_bytes().get(1) == Some(&b':')) {
        return Err("expected a local filename or file URL".into());
    }
    Ok(PathBuf::from(file))
}

/// Missing calibration means zero. Other read/format/version errors are reported.
pub fn load(file: &str) -> Result<f64, String> {
    let path = local_path(file)?;
    let data = match fs::read(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0.0),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    let record: Calibration = serde_json::from_slice(&data)
        .map_err(|e| format!("invalid calibration {}: {e}", path.display()))?;
    if record.version != 1 {
        return Err(format!(
            "unsupported calibration version {}",
            record.version
        ));
    }
    validate(record.floor_correction_meters)?;
    Ok(record.floor_correction_meters)
}

/// Validate first, then sync and atomically replace from the same directory.
/// An unsuccessful write leaves the previous destination intact.
pub fn save(file: &str, meters: f64) -> Result<(), String> {
    validate(meters)?;
    let path = local_path(file)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    let temporary = parent.join(format!(".xr-calibration-{}.tmp", uuid::Uuid::new_v4()));
    let data = serde_json::to_vec_pretty(&Calibration {
        version: 1,
        floor_correction_meters: meters,
    })
    .map_err(|e| e.to_string())?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| format!("cannot create calibration temporary file: {e}"))?;
    let result = (|| {
        output.write_all(&data)?;
        output.write_all(b"\n")?;
        output.sync_all()?;
        drop(output);
        fs::rename(&temporary, &path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&temporary);
        return Err(format!("cannot save {}: {e}", path.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("mittens-xr-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn file(&self) -> String {
            self.0
                .join("nested/floor calibration %.json")
                .to_str()
                .unwrap()
                .into()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn xr_calibration_roundtrip_missing_file_urls_and_atomic_replacement() {
        let fixture = Fixture::new();
        let file = fixture.file();
        assert_eq!(load(&file).unwrap(), 0.0);
        let url = reqwest::Url::from_file_path(&file).unwrap().to_string();
        assert!(url.contains("%20") && url.contains("%25"));
        for value in [-0.5, 0.15, 0.5, 0.0] {
            save(&url, value).unwrap();
            assert_eq!(load(&file).unwrap(), value);
            assert_eq!(load(&url).unwrap(), value);
        }
        let record: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(record["version"], 1);
        assert_eq!(
            fs::read_dir(PathBuf::from(&file).parent().unwrap())
                .unwrap()
                .count(),
            1
        );
        assert!(default_file().unwrap().is_absolute());
    }

    #[test]
    fn xr_calibration_rejects_invalid_records_paths_and_preserves_failed_writes() {
        let fixture = Fixture::new();
        let file = fixture.file();
        save(&file, 0.15).unwrap();
        let original = fs::read(&file).unwrap();
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -0.500001,
            0.500001,
        ] {
            assert!(save(&file, value).is_err());
            assert_eq!(fs::read(&file).unwrap(), original);
        }
        for path in [
            "",
            "https://example.com/floor.json",
            "file://remote/tmp/floor.json",
            "file:///tmp/floor.json?query",
            "file:///tmp/floor.json#fragment",
        ] {
            assert!(load(path).is_err(), "{path}");
            assert!(save(path, 0.0).is_err(), "{path}");
        }
        for record in [
            "bad JSON",
            r#"{"version":2,"floor_correction_meters":0}"#,
            r#"{"version":1,"floor_correction_meters":0.51}"#,
            r#"{"version":1,"floor_correction_meters":null}"#,
            r#"{"version":1}"#,
        ] {
            fs::write(&file, record).unwrap();
            assert!(load(&file).is_err(), "{record}");
        }
        // A non-directory ancestor fails before replacing the prior calibration.
        let blocked = format!("{file}/floor.json");
        assert!(save(&blocked, 0.0).is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), r#"{"version":1}"#);
        // Force rename failure after temporary creation and verify cleanup.
        let directory = fixture.0.join("destination-directory");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("keep"), "previous contents").unwrap();
        assert!(save(directory.to_str().unwrap(), 0.0).is_err());
        assert_eq!(
            fs::read_to_string(directory.join("keep")).unwrap(),
            "previous contents"
        );
        assert!(!fs::read_dir(&fixture.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        assert!(load(directory.to_str().unwrap()).is_err());
    }
}
