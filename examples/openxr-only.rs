//! Run the real Mittens renderer without a Winit event loop or window:
//! cargo run --example openxr-only
//! cargo run --example openxr-only -- --seconds 30 --no-msaa
//!
//! A running OpenXR runtime and headset are required. The embedded scene uses
//! only built-in meshes; no model downloads, filesystem imports, or REPL.

use std::time::{Duration, Instant};

use mittens_engine::{engine, utils};

const SCENE: &str = r#"
BGC.rgba(0.035, 0.045, 0.085, 1.0)
AL.rgb(0.65, 0.65, 0.65)
T.position(0.0, 1.4, -2.0).scale(0.4, 0.4, 0.4) {
    R.cube() { C.rgba(0.2, 0.7, 1.0, 1.0) }
}
T.position(-0.7, 1.2, -2.4).scale(0.25, 0.25, 0.25) {
    R.cube() { C.rgba(1.0, 0.4, 0.2, 1.0) }
}
T.position(0.0, -0.05, -2.0).scale(4.0, 0.05, 4.0) {
    R.cube() { C.rgba(0.25, 0.3, 0.35, 1.0) }
}
InputXR.on() {
    T { CXR {} }
}
XR.on()
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    utils::logger::init();
    let mut max_duration = None;
    let mut no_msaa = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" => {
                let seconds = args
                    .next()
                    .ok_or("--seconds requires a positive integer")?
                    .parse::<u64>()?;
                if seconds == 0 {
                    return Err("--seconds must be positive".into());
                }
                max_duration = Some(Duration::from_secs(seconds));
            }
            "--no-msaa" => no_msaa = true,
            "--help" | "-h" => {
                println!("openxr-only [--seconds N] [--no-msaa]");
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    let mut universe = engine::Universe::new(engine::ecs::World::default());
    if no_msaa {
        universe.set_msaa_mode(engine::graphics::MsaaMode::Off)?;
    }
    universe.load_mms_source_at_path(SCENE, "examples/openxr-only.mms")?;
    universe.init_renderer_for_xr()?;

    let started = Instant::now();
    let mut previous = started;
    println!("[openxr-only] Driving Mittens from OpenXR; no desktop window");
    loop {
        let now = Instant::now();
        let dt = now.saturating_duration_since(previous).as_secs_f32();
        previous = now;
        universe.update_with_input_providers(dt);
        universe.render();

        if universe.systems.xr.render_failed() {
            return Err("OpenXR rendering failed; see the preceding XR diagnostics".into());
        }
        if universe.exit_requested()
            || universe.systems.xr.runtime_exit_requested()
            || max_duration.is_some_and(|limit| started.elapsed() >= limit)
        {
            break;
        }
        if !universe.systems.xr.session_running() {
            // No xrWaitFrame pacing while waiting for READY or after STOPPING.
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_scene_has_an_xr_camera_without_a_window_camera() {
        let mut universe = engine::Universe::new(engine::ecs::World::default());
        universe
            .load_mms_source_at_path(SCENE, "examples/openxr-only.mms")
            .unwrap();
        let components = &universe.world;
        assert!(components.all_components().any(|id| {
            components
                .get_component_by_id_as::<engine::ecs::component::CameraXRComponent>(id)
                .is_some()
        }));
        assert!(!components.all_components().any(|id| {
            components
                .get_component_by_id_as::<engine::ecs::component::Camera3DComponent>(id)
                .is_some()
        }));
    }
}
