//! Helpers intended for `cargo run --example ...` binaries.

use std::path::Path;
use std::process::Command;

/// Ensures the optional example GLB bundle exists under `assets/models`.
///
/// Crates.io packages omit GLB files to keep the engine crate small. Examples
/// call this before scene construction so a crate checkout or installed package
/// can restore the model bundle from the repository. The check is intentionally
/// generic: if `assets/models` already contains at least one `.glb`, this does
/// nothing.
pub fn ensure_model_assets() {
    if models_dir_has_glb() {
        return;
    }

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/download-model-assets.sh");
    if !script.is_file() {
        eprintln!(
            "[example-assets] no .glb files found in assets/models, and downloader script is missing: {}",
            script.display()
        );
        return;
    }

    println!(
        "[example-assets] no .glb files found in assets/models; running {}",
        script.display()
    );

    let output = Command::new("sh")
        .arg(&script)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output();

    match output {
        Ok(output) => {
            if !output.stdout.is_empty() {
                print!("{}", String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }
            if !output.status.success() {
                eprintln!(
                    "[example-assets] model downloader exited with status {}",
                    output.status
                );
            }
        }
        Err(error) => {
            eprintln!(
                "[example-assets] failed to run model downloader '{}': {error}",
                script.display()
            );
        }
    }
}

fn models_dir_has_glb() -> bool {
    let models_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/models");
    let Ok(entries) = std::fs::read_dir(models_dir) else {
        return false;
    };

    entries.filter_map(Result::ok).any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("glb"))
            .unwrap_or(false)
    })
}
