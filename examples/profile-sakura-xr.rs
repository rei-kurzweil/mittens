//! Timed confirmation in the real headset, using the existing XR perf collector.
//! cargo run --release --example profile-sakura-xr -- 0 /tmp/sakura-xr-0
//! cargo run --release --example profile-sakura-xr -- 2 /tmp/sakura-xr-2
use mittens_engine::engine::{
    self,
    graphics::MsaaMode,
    vr_perf::{VrPerfCase, VrPerfConfig},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let count: usize = args
        .next()
        .ok_or("usage: profile-sakura-xr <0|2> <report-directory>")?
        .parse()?;
    if ![0, 2].contains(&count) {
        return Err("choose 0 or 2 Lumae".into());
    }
    let report_dir = PathBuf::from(args.next().ok_or("report directory is required")?);
    if args.next().is_some() {
        return Err("unexpected additional arguments".into());
    }
    mittens_engine::utils::logger::init();
    let source = include_str!("sakura-drone-conversion.mms").replace(
        "let initial_lumae_count = 0",
        &format!("let initial_lumae_count = {count}"),
    );
    let source = source + "\nGraphicsSettings.secondary_motion.toggle(false)\n";
    let mut scene = engine::Universe::new(Default::default());
    scene.set_msaa_mode(MsaaMode::Msaa4x)?;
    scene.load_mms_source_at_path(&source, "examples/sakura-drone-conversion.mms")?;
    let mut config = VrPerfConfig::new(
        VrPerfCase::AvatarNoSpringNoMirror,
        Duration::from_secs(10),
        Duration::from_secs(15),
    );
    config.report_dir = report_dir;
    scene.configure_vr_perf(config);
    scene.init_renderer_for_xr()?;
    println!("Sakura XR profile: {count} Lumae, springs off, 10 s warmup + 15 s capture");
    let started = Instant::now();
    let mut previous = started;
    loop {
        let now = Instant::now();
        scene.update_with_input_providers(now.saturating_duration_since(previous).as_secs_f32());
        previous = now;
        scene.render();
        if scene.systems.xr.render_failed() {
            return Err("OpenXR rendering failed".into());
        }
        if scene.exit_requested() {
            break;
        }
        if scene.systems.xr.runtime_exit_requested() {
            return Err("runtime exited before timed capture completed".into());
        }
        if started.elapsed() > Duration::from_secs(120) {
            return Err("headset capture timed out waiting for presentation".into());
        }
        if !scene.systems.xr.session_running() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}
