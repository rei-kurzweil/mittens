//! Filesystem ownership, opaque entry IDs, and wire listings. No scene work.
use meow_meow_script::TransportValue as V;
use std::path::{Path, PathBuf};

pub const PAGE_SIZE: usize = 20;
pub const MAX_ENTRIES: usize = 4096;
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub path: Option<PathBuf>,
}
#[derive(Default)]
pub struct Model {
    pub root: Option<PathBuf>,
    pub relative: PathBuf,
    pub path: String,
    pub revision: String,
    pub generation: u64,
    pub entries: Vec<Entry>,
    pub page: usize,
    pub selected_id: String,
    pub can_up: bool,
    pub selected_name: String,
    pub preview: String,
    pub action: Option<V>,
    pub action_count: u64,
    pub error: String,
    pub demo: bool,
}
fn table(values: impl IntoIterator<Item = (&'static str, V)>) -> V {
    V::Table(
        values
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect(),
    )
}
impl Model {
    pub fn demo() -> Self {
        let mut model = Self {
            demo: true,
            ..Self::default()
        };
        model.demo_directory(true);
        model
    }
    fn demo_directory(&mut self, inside: bool) {
        let names: &[&str] = if inside {
            &[
                "hello-headset.mms",
                "golden-hour.mms",
                "sakura-stage.mms",
                "floating-notes.mms",
                "keyboard-playground.mms",
                "material-study.mms",
                "tiny-planets.mms",
                "room-controls.mms",
                "spatial-terminal.mms",
                "animated-type.mms",
                "paper-lanterns.mms",
                "hand-tracking.mms",
                "desktop-bridge.mms",
                "amber-garden.mms",
                "music-room.mms",
                "soft-shadows.mms",
                "constellations.mms",
                "camera-tests.mms",
                "network-sandbox.mms",
                "welcome-home.mms",
                "moonlit-water.mms",
                "folded-paper.mms",
                "window-colors.mms",
                "sakura-petals.mms",
                "tiny-robots.mms",
                "quiet-morning.mms",
                "soft-lanterns.mms",
                "rainy-window.mms",
                "gold-material.mms",
                "cloud-studies.mms",
                "controller-lab.mms",
                "light-garden.mms",
                "scene-sketch.mms",
                "music-box.mms",
                "good-night.mms",
            ]
        } else {
            &[
                "mitten-droid",
                "hello-world.mms",
                "text-editor.mms",
                "ui-layout.mms",
                "keyboard-events.mms",
            ]
        };
        self.generation += 1;
        self.revision = format!("demo-{}", self.generation);
        self.path = if inside {
            "examples/mitten-droid"
        } else {
            "examples"
        }
        .into();
        self.can_up = inside;
        self.page = 0;
        self.entries = names
            .iter()
            .enumerate()
            .map(|(index, name)| Entry {
                id: format!("{}:{index}", self.revision),
                name: (*name).into(),
                kind: if *name == "mitten-droid" {
                    "directory"
                } else {
                    "mms"
                }
                .into(),
                path: None,
            })
            .collect();
    }
    pub fn local(root: &str, relative: &str) -> Result<Self, String> {
        let root = Path::new(root).canonicalize().map_err(|e| e.to_string())?;
        let mut model = Self {
            root: Some(root),
            ..Self::default()
        };
        model.navigate(Path::new(relative))?;
        Ok(model)
    }
    pub fn navigate(&mut self, relative: &Path) -> Result<(), String> {
        let root = self
            .root
            .as_ref()
            .ok_or("remote browser has no filesystem")?;
        let directory = root
            .join(relative)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !directory.starts_with(root) {
            return Err("directory is outside the browser root".into());
        }
        let relative = directory.strip_prefix(root).unwrap().to_path_buf();
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&directory).map_err(|e| e.to_string())? {
            if entries.len() == MAX_ENTRIES {
                return Err("directory exceeds 4096 entries".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let ty = entry.file_type().map_err(|e| e.to_string())?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "non-UTF8 filename")?;
            // Symlinks stay visible but cannot grant filesystem access.
            let kind = if ty.is_symlink() {
                "link"
            } else if ty.is_dir() {
                "directory"
            } else if ty.is_file() && entry.path().extension().is_some_and(|s| s == "mms") {
                "mms"
            } else {
                "file"
            };
            entries.push(Entry {
                id: String::new(),
                name,
                kind: kind.into(),
                path: Some(entry.path()),
            });
        }
        entries.sort_by(|a, b| {
            (a.kind != "directory", &a.name).cmp(&(b.kind != "directory", &b.name))
        });
        self.generation += 1;
        for (index, entry) in entries.iter_mut().enumerate() {
            entry.id = format!("{}:{index}", self.generation);
        }
        self.revision = self.generation.to_string();
        self.can_up = !relative.as_os_str().is_empty();
        self.path = root
            .file_name()
            .unwrap_or(root.as_os_str())
            .to_string_lossy()
            .into_owned();
        if self.can_up {
            self.path.push('/');
            self.path.push_str(&relative.to_string_lossy());
        }
        self.relative = relative;
        self.entries = entries;
        self.selected_id.clear();
        self.page = 0;
        self.error.clear();
        Ok(())
    }
    pub fn snapshot(&self) -> V {
        table([
            ("path", V::String(self.path.clone())),
            ("revision", V::String(self.revision.clone())),
            ("can_up", V::Bool(self.can_up)),
            (
                "entries",
                V::Array(
                    self.entries
                        .iter()
                        .map(|entry| {
                            table([
                                ("id", V::String(entry.id.clone())),
                                ("name", V::String(entry.name.clone())),
                                ("kind", V::String(entry.kind.clone())),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
    pub fn set_listing(&mut self, value: &V) -> Result<bool, String> {
        let V::Table(fields) = value else {
            return Err("listing must be an object".into());
        };
        let path = string(fields, "path")?;
        let revision = string(fields, "revision")?;
        let Some(V::Bool(can_up)) = field(fields, "can_up") else {
            return Err("can_up must be boolean".into());
        };
        let Some(V::Array(values)) = field(fields, "entries") else {
            return Err("entries must be an array".into());
        };
        if values.len() > MAX_ENTRIES || path.len() > 4096 || revision.len() > 128 {
            return Err("listing limit exceeded".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut entries = Vec::with_capacity(values.len());
        for value in values {
            let V::Table(entry) = value else {
                return Err("entry must be an object".into());
            };
            let id = string(entry, "id")?;
            let name = string(entry, "name")?;
            let kind = string(entry, "kind")?;
            if id.is_empty()
                || id.len() > 128
                || name.len() > 1024
                || !ids.insert(id.clone())
                || !["directory", "mms", "file", "link"].contains(&kind.as_str())
            {
                return Err("invalid entry".into());
            }
            entries.push(Entry {
                id,
                name,
                kind,
                path: None,
            });
        }
        if self.revision == revision && self.path == path {
            return Ok(false);
        }
        self.demo = false;
        self.path = path;
        self.revision = revision;
        self.can_up = *can_up;
        self.entries = entries;
        self.selected_id.clear();
        self.page = 0;
        self.error.clear();
        Ok(true)
    }
    pub fn activate(&mut self, kind: &str, id: &str) -> Result<(), String> {
        if !["up", "open"].contains(&kind) {
            return Err("unknown browser action".into());
        }
        if self.demo {
            if kind == "up" {
                self.demo_directory(false);
                return Ok(());
            }
            let entry = self
                .entries
                .iter()
                .find(|entry| entry.id == id)
                .ok_or("obsolete demo entry")?;
            if entry.kind == "directory" {
                self.demo_directory(true);
                return Ok(());
            }
            self.selected_id = entry.id.clone();
            self.selected_name = entry.name.clone();
            self.preview = format!(
                "// {}\n// Preview data / no code is running.\n\nlet message = \"Hello, headset.\"\nlet color = [1.0, 0.72, 0.16]\n\n// Your next idea goes here.",
                entry.name
            );
            return Ok(());
        }
        if self.root.is_none() {
            if kind == "open" {
                let entry = self
                    .entries
                    .iter()
                    .find(|entry| entry.id == id)
                    .ok_or("obsolete entry")?;
                self.selected_id = entry.id.clone();
                self.selected_name = entry.name.clone();
            }
            self.action_count += 1;
            self.action = Some(table([
                ("action_id", V::String(self.action_count.to_string())),
                ("kind", V::String(kind.into())),
                ("entry_id", V::String(id.into())),
                ("revision", V::String(self.revision.clone())),
            ]));
            return Ok(());
        }
        if kind == "up" {
            if self.can_up {
                let parent = self
                    .relative
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_path_buf();
                self.navigate(&parent)?;
            }
            return Ok(());
        }
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or("obsolete entry")?
            .clone();
        let path = entry.path.ok_or("missing local path")?;
        if entry.kind == "directory" {
            let relative = path
                .strip_prefix(self.root.as_ref().unwrap())
                .map_err(|e| e.to_string())?;
            return self.navigate(relative);
        }
        if entry.kind == "link" {
            return Err("symlinks are not opened by this browser".into());
        }
        let preview = if entry.kind == "mms" {
            read_preview(&path, self.root.as_ref().unwrap())?
        } else {
            "Source preview is available for MMS scripts.".into()
        };
        self.selected_id = entry.id;
        self.selected_name = entry.name;
        self.preview = preview;
        Ok(())
    }
}
fn string(fields: &[(String, V)], name: &str) -> Result<String, String> {
    match field(fields, name) {
        Some(V::String(value)) => Ok(value.clone()),
        _ => Err(format!("{name} must be a string")),
    }
}
fn read_preview(path: &Path, root: &Path) -> Result<String, String> {
    use std::io::Read;
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(root) {
        return Err("file is outside the browser root".into());
    }
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(65537)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let mut preview = String::new();
    for (index, line) in text.lines().take(12).enumerate() {
        if index > 0 {
            preview.push('\n');
        }
        preview.extend(line.chars().take(50));
    }
    if text.lines().count() > 12 || text.chars().count() > preview.chars().count() + 1 {
        preview.push_str("\n... preview");
    }
    Ok(preview)
}

fn field<'a>(fields: &'a [(String, V)], name: &str) -> Option<&'a V> {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}
