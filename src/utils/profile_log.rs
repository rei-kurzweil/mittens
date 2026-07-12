use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn run_id() -> &'static str {
    static RUN_ID: OnceLock<String> = OnceLock::new();
    RUN_ID
        .get_or_init(|| format!("{}-{}", unix_timestamp_ms(), std::process::id()))
        .as_str()
}

pub fn append(profile_name: &str, message: &str) {
    let directory = Path::new("docs/.debug");
    if create_dir_all(directory).is_err() {
        return;
    }

    let path = directory.join(format!("{}-{profile_name}.log", run_id()));
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let timestamp_ms = unix_timestamp_ms();
    let _ = writeln!(file, "{timestamp_ms} {message}");
}
