const SOURCE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/text-animation.mms");
use crate::engine::{
    Universe,
    ecs::{EventSignal, World, component::TextComponent},
};

fn text(scene: &Universe, label: &str) -> String {
    let id = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap();
    scene
        .world
        .get_component_by_id_as::<TextComponent>(id)
        .unwrap()
        .text
        .clone()
}
#[test]
fn benchmark_mms_returns_results_and_callbacks_use_the_same_collector() {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        Benchmark.start("off")
        let button = R.cube() { name = "stop" }
        let result = Text { name = "result" "waiting" }
        on(button, "Click", fn(event) {
            let measured = Benchmark.stop("off")
            result.set_text(measured.label + ":" + measured.frame_count + ":" + measured.gpu_frame_ms.mean_ms)
        })
    "#, SOURCE_PATH).unwrap();
    let ticket = scene.systems.benchmarks.frame_ticket();
    scene.systems.benchmarks.record_frame(ticket, Some(4.0));
    scene.systems.benchmarks.record_frame(ticket, Some(2.0));
    let button = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("stop"))
        .unwrap();
    scene.systems.rx.push_event(
        button,
        EventSignal::Click {
            raycaster: button,
            renderable: button,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    for _ in 0..3 {
        scene.update(0.016, &Default::default());
    }
    assert_eq!(text(&scene, "result"), "off:2:3");
    assert!(scene.systems.benchmarks.stop("off").is_err());
}
#[test]
fn benchmark_mms_validates_labels_and_arguments() {
    for source in [
        "Benchmark.start()",
        "Benchmark.start(1)",
        "Benchmark.start(\"\")",
        "Benchmark.stop(\"missing\")",
        "Benchmark.start(\"a\") Benchmark.start(\"a\")",
    ] {
        let mut scene = Universe::new(World::default());
        assert!(
            scene.load_mms_source_at_path(source, SOURCE_PATH).is_err(),
            "{source}"
        );
        assert!(
            !scene.systems.benchmarks.has_active(),
            "failed load leaked its measurements"
        );
    }
}
#[test]
fn benchmark_missing_host_service_is_unavailable() {
    let configured = crate::scripting::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let host = crate::scripting::host::MittensHost::new(&mut world, &mut queue, &mut intents)
        .with_bindings(configured.bindings());
    let mut session = configured.runtime().session(host);
    let error = session.eval("Benchmark.start(\"a\")").unwrap_err();
    assert!(error.to_string().contains("unavailable"), "{error}");
}
#[test]
fn benchmark_scene_replacement_and_close_reset_active_intervals() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path("Benchmark.start(\"original\")", SOURCE_PATH)
        .unwrap();
    assert!(
        scene
            .load_mms_source_at_path(
                "Benchmark.start(\"temporary\") Benchmark.stop(\"missing\")",
                SOURCE_PATH
            )
            .is_err()
    );
    assert!(scene.systems.benchmarks.stop("original").is_ok());
    assert!(scene.systems.benchmarks.stop("temporary").is_err());
    scene.systems.benchmarks.start("old").unwrap();
    scene
        .load_mms_source_at_path("Benchmark.start(\"new\")", SOURCE_PATH)
        .unwrap();
    assert!(scene.systems.benchmarks.stop("old").is_err());
    scene.close_mms_session();
    assert!(!scene.systems.benchmarks.has_active());
}
#[test]
fn benchmark_keyframes_change_settings_and_measure_only_the_visual_interval() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r#"
        Clock.bpm(60)
        let result = Text { name = "keyframe_result" "waiting" }
        Animation.length(1) {
            Keyframe.at(0) { GraphicsSettings.foveation.quality("high") Benchmark.start("clip") }
            Keyframe.at(0.2) {
                let measured = Benchmark.stop("clip")
                result.set_text(measured.label + ":" + measured.gpu_frame_ms.mean_ms)
                GraphicsSettings.foveation.quality("off")
            }
        }
    "#,
            SOURCE_PATH,
        )
        .unwrap();
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    #[derive(Default)]
    struct ManualClock(AtomicU64);
    impl crate::engine::ecs::system::clock_system::ClockDriver for ManualClock {
        fn name(&self) -> &'static str {
            "benchmark-test"
        }
        fn time_now_sec(&self) -> f64 {
            f64::from_bits(self.0.load(Ordering::Relaxed))
        }
    }
    let clock = Arc::new(ManualClock::default());
    scene.systems.clock.set_driver(clock.clone());
    for step in 0..30 {
        clock
            .0
            .store((f64::from(step) * 0.02).to_bits(), Ordering::Relaxed);
        scene.update(0.02, &Default::default());
        let ticket = scene.systems.benchmarks.frame_ticket();
        scene.systems.benchmarks.record_frame(ticket, Some(7.0));
        if text(&scene, "keyframe_result") != "waiting" {
            break;
        }
    }
    assert_eq!(text(&scene, "keyframe_result"), "clip:7");
    assert_eq!(
        scene.systems.graphics_settings.foveation_quality().as_str(),
        "off"
    );
    assert!(!scene.systems.benchmarks.has_active());
}
#[test]
fn benchmark_immediate_stop_exposes_missing_metrics_and_wall_duration() {
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        Benchmark.start("duration")
        let result = Benchmark.stop("duration")
        Text { name = "duration_result" "" + (result.duration_ms >= 0) + ":" + result.cpu_update_ms + ":" + result.gpu_frame_ms }
    "#, SOURCE_PATH).unwrap();
    assert_eq!(text(&scene, "duration_result"), "true:null:null");
}
