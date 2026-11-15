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

    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/scripts/download-model-assets.sh");
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    fn collect_glbs(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                collect_glbs(root, &path, out);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"))
            {
                let relative = path.strip_prefix(root).unwrap();
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    /// The crates.io package omits every .glb, so the downloader must list them all.
    #[test]
    fn model_downloader_lists_every_bundled_glb() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let script =
            std::fs::read_to_string(root.join("examples/scripts/download-model-assets.sh"))
                .unwrap();
        let listed: Vec<_> = script.lines().map(str::trim).collect();
        let mut glbs = Vec::new();
        collect_glbs(root, &root.join("assets"), &mut glbs);
        // Untracked local and third-party models are not published with tags.
        glbs.retain(|path| !path.starts_with("assets/models/local/"));
        let missing: Vec<_> = glbs
            .iter()
            .filter(|path| !listed.contains(&path.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "add these to download-model-assets.sh: {missing:?}"
        );
    }
}
