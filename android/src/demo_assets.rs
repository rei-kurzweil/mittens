//! Small explicit demo bundle. Files retain their repository import layout;
//! resource URIs are made absolute without changing the process working dir.
use crate::xr_host::Result;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub(crate) enum DemoScene {
    Cubes,
    SakuraStage,
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
    "android/shaders/sakura-sky.frag",
    "assets/components/studio_stage.mms",
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
];
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
        DemoScene::SakuraStage => {
            for &(relative, bytes) in STAGE_FILES {
                let target = root.join(relative);
                std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
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
            let path = root.join("android/examples/sakura-stage.mms");
            let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            Ok(SceneSource {
                source,
                path,
                label: "Sakura rooftop sound stage",
            })
        }
    }
}
