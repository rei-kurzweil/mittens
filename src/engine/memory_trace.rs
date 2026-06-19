use crate::engine::ecs::World;
use crate::engine::ecs::system::GLTFSystem;
use crate::engine::graphics::{RenderAssets, VisualWorld};
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryCounters {
    pub world_components: usize,
    pub visual_instances: usize,
    pub visual_mirrors: usize,
    pub cpu_meshes: usize,
    pub imported_meshes: usize,
    pub gltf_tracked_components: usize,
    pub gltf_cached_resources: usize,
    pub gltf_cached_meshes: usize,
    pub gltf_cached_textures: usize,
    pub gltf_cached_cpu_bytes: usize,
}

#[derive(Debug)]
struct MemoryTraceState {
    previous_rss_bytes: Option<u64>,
    baseline_rss_bytes: Option<u64>,
    sample_index: u64,
}

fn enabled() -> bool {
    env::var("CAT_DEBUG_MEMORY")
        .ok()
        .map(|s| {
            let s = s.trim().to_ascii_lowercase();
            !(s.is_empty() || s == "0" || s == "false" || s == "off")
        })
        .unwrap_or(false)
}

fn log_path() -> PathBuf {
    env::var("CAT_DEBUG_MEMORY_FILE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/cat-engine-memory.log"))
}

fn append_line(path: &Path, line: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn state() -> &'static Mutex<MemoryTraceState> {
    static STATE: OnceLock<Mutex<MemoryTraceState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(MemoryTraceState {
            previous_rss_bytes: None,
            baseline_rss_bytes: None,
            sample_index: 0,
        })
    })
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.2} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.2} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.2} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn format_delta_signed(bytes: i64) -> String {
    let sign = if bytes >= 0 { "+" } else { "-" };
    format!("{sign}{}", format_bytes(bytes.unsigned_abs()))
}

fn parse_status_memory_kib(line: &str, field: &str) -> Option<u64> {
    let value = line.strip_prefix(field)?.split_whitespace().next()?;
    value.parse::<u64>().ok().map(|kib| kib * 1024)
}

fn read_rss_sample() -> Option<(u64, Option<u64>)> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let mut rss_bytes = None;
    let mut peak_rss_bytes = None;
    for line in status.lines() {
        if rss_bytes.is_none() {
            rss_bytes = parse_status_memory_kib(line, "VmRSS:");
        }
        if peak_rss_bytes.is_none() {
            peak_rss_bytes = parse_status_memory_kib(line, "VmHWM:");
        }
    }
    rss_bytes.map(|rss| (rss, peak_rss_bytes))
}

pub fn collect_counters(
    world: &World,
    visuals: &VisualWorld,
    render_assets: &RenderAssets,
    gltf: Option<&GLTFSystem>,
) -> MemoryCounters {
    MemoryCounters {
        world_components: world.component_count(),
        visual_instances: visuals.instance_count(),
        visual_mirrors: visuals.mirror_count(),
        cpu_meshes: render_assets.cpu_mesh_count(),
        imported_meshes: render_assets.imported_mesh_count(),
        gltf_tracked_components: gltf.map(GLTFSystem::tracked_component_count).unwrap_or(0),
        gltf_cached_resources: gltf.map(GLTFSystem::cached_resource_count).unwrap_or(0),
        gltf_cached_meshes: gltf.map(GLTFSystem::cached_mesh_count).unwrap_or(0),
        gltf_cached_textures: gltf.map(GLTFSystem::cached_texture_count).unwrap_or(0),
        gltf_cached_cpu_bytes: gltf.map(GLTFSystem::cached_cpu_bytes).unwrap_or(0),
    }
}

pub fn log_line(line: impl AsRef<str>) {
    if !enabled() {
        return;
    }
    append_line(&log_path(), line.as_ref());
}

pub fn sample(label: &str, counters: Option<MemoryCounters>) {
    if !enabled() {
        return;
    }
    let Some((rss_bytes, peak_rss_bytes)) = read_rss_sample() else {
        return;
    };

    let mut state = state().lock().expect("memory trace mutex poisoned");
    let baseline = *state.baseline_rss_bytes.get_or_insert(rss_bytes);
    let delta_prev = rss_bytes as i64 - state.previous_rss_bytes.unwrap_or(rss_bytes) as i64;
    let delta_base = rss_bytes as i64 - baseline as i64;
    let sample_index = state.sample_index;
    state.sample_index += 1;
    state.previous_rss_bytes = Some(rss_bytes);

    let line = if let Some(counters) = counters {
        format!(
            "[memory] #{sample_index:03} {label} rss={} delta_prev={} delta_base={} peak={} components={} instances={} mirrors={} cpu_meshes={} imported_meshes={} gltf_tracked={} gltf_cached_resources={} gltf_cached_meshes={} gltf_cached_textures={} gltf_cached_cpu={}",
            format_bytes(rss_bytes),
            format_delta_signed(delta_prev),
            format_delta_signed(delta_base),
            peak_rss_bytes
                .map(format_bytes)
                .unwrap_or_else(|| "n/a".to_string()),
            counters.world_components,
            counters.visual_instances,
            counters.visual_mirrors,
            counters.cpu_meshes,
            counters.imported_meshes,
            counters.gltf_tracked_components,
            counters.gltf_cached_resources,
            counters.gltf_cached_meshes,
            counters.gltf_cached_textures,
            format_bytes(counters.gltf_cached_cpu_bytes as u64),
        )
    } else {
        format!(
            "[memory] #{sample_index:03} {label} rss={} delta_prev={} delta_base={} peak={}",
            format_bytes(rss_bytes),
            format_delta_signed(delta_prev),
            format_delta_signed(delta_base),
            peak_rss_bytes
                .map(format_bytes)
                .unwrap_or_else(|| "n/a".to_string())
        )
    };
    append_line(&log_path(), &line);
}
