//! User preferences shared by graphical and headless sessions.
use crate::utils::user_config;
use std::{fs, io::ErrorKind, path::Path};

const FILENAME: &str = "network-settings.json";

#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)]
struct NetworkSettings {
    auto_accept_asset_transfers: bool,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

impl NetworkSettings {
    fn load(path: &Path) -> Result<Self, String> {
        match fs::read(path) {
            Ok(data) => serde_json::from_slice(&data)
                .map_err(|error| format!("invalid network settings {}: {error}", path.display())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("cannot read {}: {error}", path.display())),
        }
    }

    fn set_auto_accept(path: &Path, enabled: bool) -> Result<(), String> {
        let mut settings = Self::load(path)?;
        settings.auto_accept_asset_transfers = enabled;
        let mut data = serde_json::to_vec_pretty(&settings).map_err(|error| error.to_string())?;
        data.push(b'\n');
        user_config::write_atomic(path, &data)
    }
}

pub(crate) fn auto_accept_asset_transfers() -> bool {
    match user_config::directory().and_then(|dir| NetworkSettings::load(&dir.join(FILENAME))) {
        Ok(settings) => settings.auto_accept_asset_transfers,
        Err(error) => {
            eprintln!("[NetworkSettings] {error}");
            false
        }
    }
}

pub(crate) fn set_auto_accept_asset_transfers(enabled: bool) -> Result<(), String> {
    NetworkSettings::set_auto_accept(&user_config::directory()?.join(FILENAME), enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_settings_default_off_and_preserve_other_preferences_on_update() {
        let directory =
            std::env::temp_dir().join(format!("mittens-network-{}", uuid::Uuid::new_v4()));
        let path = directory.join(FILENAME);
        assert!(
            !NetworkSettings::load(&path)
                .unwrap()
                .auto_accept_asset_transfers
        );
        NetworkSettings::set_auto_accept(&path, true).unwrap();
        assert!(
            NetworkSettings::load(&path)
                .unwrap()
                .auto_accept_asset_transfers
        );
        fs::write(
            &path,
            r#"{"auto_accept_asset_transfers":true,"other_setting":42}"#,
        )
        .unwrap();
        NetworkSettings::set_auto_accept(&path, false).unwrap();
        let saved = NetworkSettings::load(&path).unwrap();
        assert!(!saved.auto_accept_asset_transfers);
        assert_eq!(saved.other["other_setting"], 42);
        fs::remove_dir_all(directory).unwrap();
    }
}
