//! Headset home screen: the launcher scene opens MMS files from a project tree
//! pushed to app storage, and the host returns to it on request or failure.
//!
//! The project root mirrors the repository layout (`examples/`, `assets/`,
//! `android/examples/`, ...) and becomes the process working directory, so
//! scripts resolve `assets/...` URIs exactly as they do on desktop.
use crate::xr_host::Result;
use mittens_engine::engine::app_launch::{self, LaunchRequest};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// Seconds the left menu button must be held to return home.
const HOME_HOLD_SECONDS: f32 = 0.6;
const HOME: &str = "android/examples/launcher.mms";

static EXTERNAL_FILES: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Records the activity's external files directory, where `adb push` can
/// write without `run-as`. Called once before the XR worker starts.
pub(crate) fn set_external_files_dir(directory: Option<PathBuf>) {
    let _ = EXTERNAL_FILES.set(directory);
}

pub(crate) struct Launcher {
    root: PathBuf,
    home: PathBuf,
    current: PathBuf,
    menu_held: f32,
    menu_latched: bool,
}

impl Launcher {
    /// Prepares the project root under external storage, falling back to the
    /// internal `directory`, and installs any missing launcher files.
    pub fn prepare(directory: &Path) -> Result<Self> {
        let root = EXTERNAL_FILES
            .get()
            .cloned()
            .flatten()
            .unwrap_or_else(|| directory.to_path_buf())
            .join("project");
        std::fs::create_dir_all(&root).map_err(|e| format!("project root: {e}"))?;
        crate::demo_assets::install_missing(&root, crate::demo_assets::LAUNCHER_FILES)?;
        let root = root.canonicalize().map_err(|e| format!("project root: {e}"))?;
        mittens_engine::engine::runtime_assets::set_root(&root)?;
        std::env::set_current_dir(&root).map_err(|e| format!("project working dir: {e}"))?;
        app_launch::set_enabled(true);
        log::info!("launcher project_root={}", root.display());
        let home = root.join(HOME);
        Ok(Self {
            root,
            current: home.clone(),
            home,
            menu_held: 0.0,
            menu_latched: false,
        })
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn at_home(&self) -> bool {
        self.current == self.home
    }

    pub fn set_current(&mut self, path: &Path) {
        self.current = path.to_path_buf();
    }

    /// The path shown to the person, relative to the project root when possible.
    pub fn display(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    /// The next scene to load: home after the menu button is held, otherwise
    /// the latest `App.launch`/`App.home` request.
    pub fn poll(&mut self, menu_down: bool, dt: f32) -> Option<PathBuf> {
        if menu_down {
            self.menu_held += dt;
        } else {
            self.menu_held = 0.0;
            self.menu_latched = false;
        }
        if self.menu_held >= HOME_HOLD_SECONDS && !self.menu_latched {
            self.menu_latched = true;
            app_launch::set_last_error("");
            return Some(self.home.clone());
        }
        match app_launch::take()? {
            LaunchRequest::Scene(path) => {
                let path = Path::new(&path);
                Some(if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    self.root.join(path)
                })
            }
            LaunchRequest::Home => Some(self.home.clone()),
        }
    }
}
