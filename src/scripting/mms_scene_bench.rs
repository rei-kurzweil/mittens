//! Headless MMS cost per scene: load time, then per-frame callback time with
//! host requests subtracted. Baseline for evaluator changes (closure capture,
//! heap arrays, slot resolution); see `docs/benchmarks/mms-interpreter.md`.
//!
//! cargo test --release --lib mms_scene_bench -- --ignored --nocapture
use crate::engine::{Universe, ecs::World, update_cpu_profile};
use crate::engine::graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader};
use std::time::Instant;

#[derive(Default)]
struct CpuUploader { meshes: u32, textures: u32 }
impl MeshUploader for CpuUploader {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.meshes += 1;
        Ok(MeshHandle(1000 + self.meshes))
    }
}
impl TextureUploader for CpuUploader {
    fn upload_texture_rgba8(&mut self, _: &[u8], _: u32, _: u32) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.textures += 1;
        Ok(TextureHandle(1000 + self.textures))
    }
    fn upload_texture_bc7(&mut self, _: &[u8], _: u32, _: u32, _: bool) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.textures += 1;
        Ok(TextureHandle(1000 + self.textures))
    }
}
fn prepare(scene: &mut Universe, uploader: &mut CpuUploader) {
    scene.systems.prepare_render(&mut scene.world, &mut scene.visuals, &mut scene.render_assets, uploader, &mut scene.command_queue);
}

const DEFAULT_SCENES: [&str; 3] = [
    "examples/sakura-twilight-simpler.mms",
    "examples/truck-bed-demo.mms",
    "examples/interceptor.mms",
];
const HOST_STAGES: [&str; 4] = ["host_query", "host_validation", "host_method", "host_other"];

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn stage(snapshot: &serde_json::Value, name: &str) -> f64 {
    snapshot["stages_ms"][name].as_f64().unwrap_or(0.0)
}

#[test]
#[ignore = "benchmark; run with --release --ignored --nocapture"]
fn mms_scene_bench() {
    let scenes: Vec<String> = std::env::var("MMS_BENCH_SCENES")
        .map(|list| list.split(',').map(str::to_owned).collect())
        .unwrap_or_else(|_| DEFAULT_SCENES.map(str::to_owned).to_vec());
    let warmup = env_usize("MMS_BENCH_WARMUP", 120);
    let frames = env_usize("MMS_BENCH_FRAMES", 600);
    let top = env_usize("MMS_BENCH_TOP", 8);
    let mut report = Vec::new();

    for path in &scenes {
        let mut source = std::fs::read_to_string(path)
            .unwrap()
            .replace("AudioInput {}", "AudioInput { enabled(false) }");
        if path.ends_with("/interceptor.mms")
            && std::env::var("MMS_BENCH_INTERCEPTOR_NPCS").as_deref() == Ok("2")
        {
            if std::env::var("MMS_BENCH_INTERCEPTOR_NO_MOTION_LOD").as_deref() == Ok("1") {
                source.push_str("\nrange_gear.set_motion_visibility(false)\n");
            }
            source.push_str(r#"
                let profile_spawn = { elapsed = 0.0 done = false }
                on_global("FrameTick", fn(event) {
                    if profile_spawn.done { return }
                    profile_spawn.elapsed = profile_spawn.elapsed + event.dt_sec
                    if profile_spawn.elapsed >= 1.0 {
                        profile_spawn.done = true
                        range_gear.spawn_npcs()
                    }
                })
            "#);
        }
        // Enabled before load so handler registration records labels.
        update_cpu_profile::enable(true);
        let mut scene = Universe::new(World::default());
        scene
            .systems
            .xr
            .set_external_frame(Some(Default::default()));
        let load_started = Instant::now();
        scene.load_mms_source_at_path(&source, path).unwrap();
        let load_ms = load_started.elapsed().as_secs_f64() * 1000.0;
        let render_prep = std::env::var("MMS_BENCH_PREPARE_RENDER").as_deref() == Ok("1");
        let mut uploader = CpuUploader::default();

        for _ in 0..warmup {
            scene.update(1.0 / 60.0, &Default::default());
            if render_prep { prepare(&mut scene, &mut uploader); }
        }
        let mitten_models = scene.world.all_components().filter(|&id| {
            scene.world.get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(id)
                .is_some_and(|gltf| gltf.uri == "assets/models/mitten.glb")
        }).count();
        if path.ends_with("/interceptor.mms") && std::env::var("MMS_BENCH_INTERCEPTOR_NPCS").as_deref() == Ok("2") {
            assert!(mitten_models >= 3, "expected the player and two NPC Mitten models, found {mitten_models}");
        }

        let mut update_ms = 0.0;
        let mut invoke_ms = 0.0;
        let mut host_ms = 0.0;
        let mut frame_dispatch_ms = 0.0;
        let mut prepare_ms = 0.0;
        let mut callbacks = 0u64;
        let mut paused_motion_total = 0usize;
        let mut worst_invoke_ms: f64 = 0.0;
        let mut per_callback = std::collections::BTreeMap::<String, (u64, f64)>::new();
        let mut stages = std::collections::BTreeMap::<String, f64>::new();
        for _ in 0..frames {
            let started = Instant::now();
            scene.update(1.0 / 60.0, &Default::default());
            update_ms += started.elapsed().as_secs_f64() * 1000.0;
            if render_prep {
                let started = Instant::now();
                prepare(&mut scene, &mut uploader);
                prepare_ms += started.elapsed().as_secs_f64() * 1000.0;
            }
            let snapshot = update_cpu_profile::snapshot();
            paused_motion_total += scene.world.all_components().filter(|&id| {
                scene.world.get_component_by_id_as::<crate::engine::ecs::component::SecondaryMotionComponent>(id)
                    .is_some_and(|motion| motion.paused)
            }).count();
            if let Some(values) = snapshot["stages_ms"].as_object() {
                for (name, value) in values {
                    *stages.entry(name.clone()).or_default() += value.as_f64().unwrap_or(0.0);
                }
            }
            let invoke = stage(&snapshot, "callback_invoke");
            invoke_ms += invoke;
            worst_invoke_ms = worst_invoke_ms.max(invoke);
            host_ms += HOST_STAGES.iter().map(|name| stage(&snapshot, name)).sum::<f64>();
            frame_dispatch_ms += stage(&snapshot, "frame_tick_dispatch");
            callbacks += snapshot["host_counters"][0].as_u64().unwrap_or(0);
            for sample in snapshot["callbacks"].as_array().into_iter().flatten() {
                let label = sample["label"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("callback {}", sample["id"]));
                let entry = per_callback.entry(label).or_default();
                entry.0 += sample["count"].as_u64().unwrap_or(0);
                entry.1 += sample["ms"].as_f64().unwrap_or(0.0);
            }
        }
        update_cpu_profile::enable(false);

        let per_frame = |total: f64| total / frames as f64;
        let mut hottest: Vec<_> = per_callback.into_iter().collect();
        hottest.sort_by(|a, b| b.1.1.total_cmp(&a.1.1));
        hottest.truncate(top);

        println!("\n== {path}");
        println!("load_ms                 {load_ms:.1}");
        println!("mitten_models           {mitten_models}");
        println!("update_ms/frame         {:.3}", per_frame(update_ms));
        println!("prepare_ms/frame        {:.3}", per_frame(prepare_ms));
        println!("callback_invoke/frame   {:.3}", per_frame(invoke_ms));
        println!("  host requests/frame   {:.3}", per_frame(host_ms));
        println!("  interpreter/frame     {:.3}", per_frame(invoke_ms - host_ms));
        println!("  worst frame invoke    {worst_invoke_ms:.3}");
        println!("frame_tick_dispatch     {:.3}", per_frame(frame_dispatch_ms));
        println!("callbacks/frame         {:.1}", callbacks as f64 / frames as f64);
        println!("paused motion roots     {:.2}", paused_motion_total as f64 / frames as f64);
        for (name, ms) in &stages {
            println!("  stage {name}: {:.3} ms/frame", per_frame(*ms));
        }
        for (label, (count, ms)) in &hottest {
            println!(
                "  {:>8.3} ms/frame  {:>6.2} calls/frame  {label}",
                per_frame(*ms),
                *count as f64 / frames as f64
            );
        }
        report.push(serde_json::json!({
            "scene": path,
            "warmup_frames": warmup,
            "frames": frames,
            "load_ms": load_ms,
            "mitten_models": mitten_models,
            "update_ms_per_frame": per_frame(update_ms),
            "prepare_ms_per_frame": if render_prep { Some(per_frame(prepare_ms)) } else { None },
            "callback_invoke_ms_per_frame": per_frame(invoke_ms),
            "host_ms_per_frame": per_frame(host_ms),
            "interpreter_ms_per_frame": per_frame(invoke_ms - host_ms),
            "worst_frame_invoke_ms": worst_invoke_ms,
            "frame_tick_dispatch_ms_per_frame": per_frame(frame_dispatch_ms),
            "callbacks_per_frame": callbacks as f64 / frames as f64,
            "paused_motion_roots_mean": paused_motion_total as f64 / frames as f64,
            "stages_ms_per_frame": stages.iter().map(|(name, ms)| (name, per_frame(*ms))).collect::<std::collections::BTreeMap<_, _>>(),
            "hottest": hottest.iter().map(|(label, (count, ms))| serde_json::json!({
                "label": label,
                "calls_per_frame": *count as f64 / frames as f64,
                "ms_per_frame": per_frame(*ms),
            })).collect::<Vec<_>>(),
        }));
    }

    if let Some(output) = std::env::var_os("MMS_BENCH_OUTPUT") {
        std::fs::write(output, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
}
