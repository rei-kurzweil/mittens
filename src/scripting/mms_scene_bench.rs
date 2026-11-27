//! Headless MMS cost per scene: load time, then per-frame callback time with
//! host requests subtracted. Baseline for evaluator changes (closure capture,
//! heap arrays, slot resolution); see `docs/benchmarks/mms-interpreter.md`.
//!
//! cargo test --release --lib mms_scene_bench -- --ignored --nocapture
use crate::engine::{Universe, ecs::World, update_cpu_profile};
use std::time::Instant;

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
        let source = std::fs::read_to_string(path)
            .unwrap()
            .replace("AudioInput {}", "AudioInput { enabled(false) }");
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

        for _ in 0..warmup {
            scene.update(1.0 / 60.0, &Default::default());
        }

        let mut update_ms = 0.0;
        let mut invoke_ms = 0.0;
        let mut host_ms = 0.0;
        let mut frame_dispatch_ms = 0.0;
        let mut callbacks = 0u64;
        let mut worst_invoke_ms: f64 = 0.0;
        let mut per_callback = std::collections::BTreeMap::<String, (u64, f64)>::new();
        for _ in 0..frames {
            let started = Instant::now();
            scene.update(1.0 / 60.0, &Default::default());
            update_ms += started.elapsed().as_secs_f64() * 1000.0;
            let snapshot = update_cpu_profile::snapshot();
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
        println!("update_ms/frame         {:.3}", per_frame(update_ms));
        println!("callback_invoke/frame   {:.3}", per_frame(invoke_ms));
        println!("  host requests/frame   {:.3}", per_frame(host_ms));
        println!("  interpreter/frame     {:.3}", per_frame(invoke_ms - host_ms));
        println!("  worst frame invoke    {worst_invoke_ms:.3}");
        println!("frame_tick_dispatch     {:.3}", per_frame(frame_dispatch_ms));
        println!("callbacks/frame         {:.1}", callbacks as f64 / frames as f64);
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
            "update_ms_per_frame": per_frame(update_ms),
            "callback_invoke_ms_per_frame": per_frame(invoke_ms),
            "host_ms_per_frame": per_frame(host_ms),
            "interpreter_ms_per_frame": per_frame(invoke_ms - host_ms),
            "worst_frame_invoke_ms": worst_invoke_ms,
            "frame_tick_dispatch_ms_per_frame": per_frame(frame_dispatch_ms),
            "callbacks_per_frame": callbacks as f64 / frames as f64,
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
