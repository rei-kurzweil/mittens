//! Small explicit demo bundle. Files retain their repository import layout;
//! resource URIs are made absolute without changing the process working dir.
use crate::xr_host::Result;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub(crate) enum DemoScene {
    Cubes,
    Shell,
    SakuraStage,
    SakuraLean,
    SakuraCollision,
    NoiseBench,
    /// Prepared by `launcher::Launcher` from the project root, not installed here.
    Launcher,
}
pub(crate) struct SceneSource {
    pub source: String,
    pub path: PathBuf,
    pub label: &'static str,
}

macro_rules! bundle {
    ($($path:literal),+ $(,)?) => { &[$(($path, include_bytes!(concat!("../../", $path)).as_slice())),+] };
}
const STAGE_FILES: &[(&str, &[u8])] = bundle![
    "android/examples/sakura-stage.mms",
    "android/examples/sakura-stage-lean.mms",
    "android/examples/sakura-stage-collision.mms",
    "android/shaders/sakura-sky.frag",
    "assets/components/studio_stage.mms",
    "assets/components/materials/studio_anime.mms",
    "assets/components/truss.mms",
    "assets/components/buildings/pool_building.mms",
    "assets/components/materials/pool_tiles.mms",
    "assets/components/materials/matcap_metal.mms",
    "assets/components/materials/anime_surface.mms",
    "assets/shaders/pool-tiles.frag",
    "assets/shaders/pool-metal-matcap.frag",
    "assets/shaders/anime-surface.frag",
    "assets/textures/pool/white-tiles-surface.png",
    "assets/textures/pool/chrome-matcap.png",
    "assets/models/sakura-machine-pistol.glb",
    "assets/components/internal/panels.mms",
    "assets/components/internal/panel_items.mms",
    "assets/components/internal/assets_content.mms",
    "assets/components/internal/asset_item.mms",
    "assets/components/internal/ui/accordion.mms",
    "assets/components/icons.mms",
    "assets/components/materials/ui_unlit.mms",
    "assets/shaders/ui-unlit.frag",
    "assets/shaders/bvh-snapshot.frag",
];
const SHELL_FILES: &[(&str, &[u8])] = bundle![
    "assets/components/backgrounds/sakura_sky.mms",
    "assets/components/mitten_droid/garden.mms",
    "assets/components/mitten_droid/checkerboard.mms",
    "assets/shaders/foveation-checkerboard.frag",
    "assets/components/platforms/terraced_garden.mms",
    "assets/components/plants/sakura_tree.mms",
    "assets/components/materials/pool_tiles.mms",
    "assets/components/materials/anime_surface.mms",
    "assets/shaders/pool-tiles.frag",
    "assets/shaders/anime-surface.frag",
    "assets/shaders/sakura-blossom.frag",
    "assets/textures/pool/white-tiles-surface.png",
    "assets/textures/cherry-blossoms/sakura-blossom-256.png",
    "android/shaders/sakura-sky.frag",
    "android/examples/shell-server.mms",
    "assets/components/mitten_droid/ui.mms",
    "assets/components/panels/graphics_settings_panel.mms",
    "assets/components/ui/info_panel.mms",
    "assets/components/internal/ui/accordion.mms",
    "assets/components/icons.mms",
    "assets/components/mitten_droid/protocol.mms",
    "assets/shaders/mitten-droid-gold.frag",
    "assets/shaders/mitten-droid-ui.frag",
    "assets/components/materials/ui_unlit.mms",
    "assets/shaders/ui-unlit.frag",
    "assets/shaders/bvh-snapshot.frag",
];
const NOISE_BENCH_FILES: &[(&str, &[u8])] = bundle![
    "android/examples/noise-bench.mms",
    "assets/images/noise/perlin-1024.png",
    "assets/textures/noise/perlin-1024-astc4x4.astc",
    "assets/textures/noise/perlin-1024-astc8x8.astc",
];
/// The launcher and its imports. Installed into the project root only where a
/// pushed project has not already supplied the file, so pushed files win.
#[cfg(feature = "launcher")]
pub(crate) const LAUNCHER_FILES: &[(&str, &[u8])] = bundle![
    "android/examples/launcher.mms",
    "assets/components/mitten_droid/ui.mms",
    "assets/components/button.mms",
    "assets/components/internal/ui/accordion.mms",
    "assets/components/icons.mms",
    "assets/components/materials/ui_unlit.mms",
    "assets/components/materials/anime_surface.mms",
    "assets/components/panels/graphics_settings_panel.mms",
    "assets/components/ui/info_panel.mms",
    "assets/components/ui/dock.mms",
    "assets/shaders/mitten-droid-ui.frag",
    "assets/shaders/mitten-droid-gold.frag",
    "assets/shaders/ui-unlit.frag",
    "assets/shaders/anime-surface.frag",
    "assets/shaders/bvh-snapshot.frag",
];
/// Writes bundled files that are missing under `root`, without rewriting
/// paths: launcher scenes run with `root` as the working directory.
#[cfg(feature = "launcher")]
pub(crate) fn install_missing(root: &Path, files: &[(&str, &[u8])]) -> Result<()> {
    install_files(root, files, false)
}
/// Writes every bundled file under `root`, replacing existing copies.
#[cfg(feature = "launcher")]
pub(crate) fn install_all(root: &Path, files: &[(&str, &[u8])]) -> Result<()> {
    install_files(root, files, true)
}
#[cfg(feature = "launcher")]
fn install_files(root: &Path, files: &[(&str, &[u8])], overwrite: bool) -> Result<()> {
    let font: (&str, &[u8]) = (
        "assets/textures/font_system.png",
        include_bytes!("../assets/font_system.png"),
    );
    for &(relative, bytes) in files.iter().chain(std::iter::once(&font)) {
        let target = root.join(relative);
        if !overwrite && target.exists() {
            continue;
        }
        std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&target, bytes).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    Ok(())
}
/// Shader NOISE_MODE variants written by the installer (no includes/defines in MMS).
const NOISE_BENCH_SHADER: &str = include_str!("../../assets/shaders/perlin-noise-benchmark.frag");
const NOISE_BENCH_MODES: [u8; 4] = [0, 1, 2, 3];

pub(crate) fn install(directory: &Path, scene: DemoScene) -> Result<SceneSource> {
    let root = directory.join("mittens-demo");
    std::fs::create_dir_all(&root).map_err(|e| format!("demo directory: {e}"))?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let escaped = root
        .to_str()
        .ok_or("non-UTF8 demo path")?
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    match scene {
        DemoScene::Launcher => Err("the launcher scene is prepared by launcher::Launcher".into()),
        DemoScene::Cubes => {
            let fragment = root.join("albedo.frag");
            std::fs::write(fragment, include_str!("../assets/albedo.frag"))
                .map_err(|e| e.to_string())?;
            let source = include_str!("../assets/scene.mms")
                .replace("__FRAGMENT_PATH__", &format!("{escaped}/albedo.frag"));
            Ok(SceneSource {
                source,
                path: root.join("cubes.mms"),
                label: "two MMS cubes",
            })
        }
        DemoScene::Shell
        | DemoScene::SakuraStage
        | DemoScene::SakuraLean
        | DemoScene::SakuraCollision
        | DemoScene::NoiseBench => {
            let files = match scene {
                DemoScene::Shell => SHELL_FILES,
                DemoScene::NoiseBench => NOISE_BENCH_FILES,
                _ => STAGE_FILES,
            };
            if matches!(scene, DemoScene::NoiseBench) {
                let shaders = root.join("assets/shaders");
                std::fs::create_dir_all(&shaders).map_err(|e| e.to_string())?;
                for mode in NOISE_BENCH_MODES {
                    let source = NOISE_BENCH_SHADER.replacen(
                        "#version 450",
                        &format!("#version 450\n#define NOISE_MODE {mode}\n#define NOISE_OCTAVES 6"),
                        1,
                    );
                    std::fs::write(
                        shaders.join(format!("perlin-noise-benchmark-mode{mode}.frag")),
                        source,
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            for &(relative, bytes) in files {
                let target = root.join(relative);
                std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                #[cfg(all(target_os = "android", feature = "foveation-visualization"))]
                if relative == "android/shaders/sakura-sky.frag" {
                    std::fs::write(target, include_str!("../shaders/foveation-sky.frag"))
                        .map_err(|e| e.to_string())?;
                    continue;
                }
                if relative.ends_with(".mms") {
                    let source = std::str::from_utf8(bytes)
                        .map_err(|e| e.to_string())?
                        .replace("\"assets/", &format!("\"{escaped}/assets/"))
                        .replace(
                            "\"android/shaders/",
                            &format!("\"{escaped}/android/shaders/"),
                        );
                    std::fs::write(target, source).map_err(|e| e.to_string())?;
                } else {
                    std::fs::write(target, bytes).map_err(|e| e.to_string())?;
                }
            }
            let font = root.join("assets/textures/font_system.png");
            std::fs::create_dir_all(font.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(font, include_bytes!("../assets/font_system.png"))
                .map_err(|e| e.to_string())?;
            let (relative, label) = match scene {
                DemoScene::Shell => (
                    "android/examples/shell-server.mms",
                    "mitten-droid shell browser",
                ),
                DemoScene::SakuraStage => (
                    "android/examples/sakura-stage.mms",
                    "Sakura rooftop stage: UI and collision",
                ),
                DemoScene::SakuraLean => (
                    "android/examples/sakura-stage-lean.mms",
                    "Sakura rooftop stage: no UI, no collision",
                ),
                DemoScene::SakuraCollision => (
                    "android/examples/sakura-stage-collision.mms",
                    "Sakura rooftop stage: collision, no UI",
                ),
                DemoScene::NoiseBench => (
                    "android/examples/noise-bench.mms",
                    "noise texture vs procedural GPU benchmark",
                ),
                DemoScene::Cubes | DemoScene::Launcher => unreachable!(),
            };
            let path = root.join(relative);
            let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            Ok(SceneSource {
                source,
                path,
                label,
            })
        }
    }
}
