//! Timed confirmation in the real headset, using the existing XR perf collector.
//! cargo run --release --example profile-sakura-xr -- 0 /tmp/sakura-xr-0
//! cargo run --release --example profile-sakura-xr -- 2 /tmp/sakura-xr-2
//! cargo run --release --example profile-sakura-xr -- twilight /tmp/sakura-xr-client
use mittens_engine::engine::{
    self,
    ecs::component::MultiplayerSessionComponent,
    graphics::MsaaMode,
    vr_perf::{VrPerfCase, VrPerfConfig},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const USAGE: &str = "usage: profile-sakura-xr <0|2|twilight|twilight-simpler|interceptor-0|interceptor-2> <report-directory> [--wait-for-peer] [--warmup SECONDS] [--sample SECONDS] [--startup-timeout SECONDS]";

struct CaptureTiming {
    warmup: Duration,
    sample: Duration,
    startup_timeout: Duration,
    wait_for_peer: bool,
}

impl CaptureTiming {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut timing = Self {
            warmup: Duration::from_secs(10),
            sample: Duration::from_secs(15),
            startup_timeout: Duration::from_secs(120),
            wait_for_peer: false,
        };
        while let Some(flag) = args.next() {
            let target = match flag.as_str() {
                "--wait-for-peer" => {
                    timing.wait_for_peer = true;
                    continue;
                }
                "--warmup" => &mut timing.warmup,
                "--sample" => &mut timing.sample,
                "--startup-timeout" => &mut timing.startup_timeout,
                _ => return Err(format!("unknown option {flag}\n{USAGE}")),
            };
            let seconds = args
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| format!("{flag} requires a duration in whole seconds"))?;
            if seconds == 0 && flag != "--warmup" {
                return Err(format!("{flag} must be greater than zero"));
            }
            *target = Duration::from_secs(seconds);
        }
        Ok(timing)
    }

    fn deadline(&self) -> Duration {
        self.startup_timeout
            .saturating_add(self.warmup)
            .saturating_add(self.sample)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().ok_or(USAGE)?;
    let report_dir = PathBuf::from(args.next().ok_or("report directory is required")?);
    let timing = CaptureTiming::parse(args)?;
    mittens_engine::utils::logger::init();
    let mut scene = engine::Universe::new(Default::default());
    let mut config = VrPerfConfig::new(
        VrPerfCase::AvatarNoSpringNoMirror,
        timing.warmup,
        timing.sample,
    );
    config.report_dir = report_dir;
    match mode.as_str() {
        "0" | "2" => {
            let source = include_str!("sakura-drone-conversion.mms").replace(
                "let initial_lumae_count = 0",
                &format!("let initial_lumae_count = {mode}"),
            ) + "\nGraphicsSettings.secondary_motion.toggle(false)\n";
            scene.set_msaa_mode(MsaaMode::Msaa4x)?;
            scene.load_mms_source_at_path(&source, "examples/sakura-drone-conversion.mms")?;
            println!("Sakura XR profile: {mode} Lumae, springs off");
        }
        "twilight" | "twilight-simpler" => {
            let name = if mode == "twilight" {
                "sakura-twilight"
            } else {
                "sakura-twilight-simpler"
            };
            let path = format!("examples/{name}.mms");
            scene.load_mms_source_at_path(&std::fs::read_to_string(&path)?, &path)?;
            config.authored_scene = Some(name);
        }
        "interceptor-0" | "interceptor-2" => {
            let path = "examples/interceptor.mms";
            let mut source = std::fs::read_to_string(path)?;
            if mode == "interceptor-2" {
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
            scene.load_mms_source_at_path(&source, path)?;
            config.authored_scene = Some(mode.clone());
        }
        _ => return Err(USAGE.into()),
    }
    let wait_session = scene.world.all_components().find(|&id| {
        scene
            .world
            .get_component_by_id_as::<MultiplayerSessionComponent>(id)
            .is_some_and(|session| timing.wait_for_peer || !session.is_host())
    });
    if timing.wait_for_peer && wait_session.is_none() {
        return Err("--wait-for-peer requires a multiplayer scene".into());
    }
    let mut capture_peer = None;
    if wait_session.is_some() {
        println!(
            "Waiting for a multiplayer peer before the {} s warm-up",
            timing.warmup.as_secs()
        );
    }
    scene.init_renderer_for_xr()?;
    let started = Instant::now();
    let mut previous = started;
    loop {
        let now = Instant::now();
        scene.update_with_input_providers(now.saturating_duration_since(previous).as_secs_f32());
        previous = now;
        let peer = match wait_session {
            None => Some((0, 0)),
            Some(id) => scene
                .world
                .get_component_by_id_as::<MultiplayerSessionComponent>(id)
                .and_then(|session| {
                    session
                        .peer_ips
                        .keys()
                        .copied()
                        .min()
                        .map(|peer| (session.peer_id, peer))
                }),
        };
        if peer != capture_peer {
            if capture_peer.is_some() {
                scene.cancel_vr_perf();
                println!(
                    "[vr-perf] multiplayer peer disconnected or changed; restarting capture after reconnection"
                );
            }
            if peer.is_some() {
                scene.configure_vr_perf(config.clone());
            }
            capture_peer = peer;
        }
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
        if started.elapsed() > timing.deadline() {
            return Err(if capture_peer.is_none() {
                "headset capture timed out waiting for a multiplayer peer"
            } else {
                "headset capture timed out before a continuous sample finished"
            }
            .into());
        }
        if !scene.systems.xr.session_running() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_a_longer_capture_without_shortening_setup_time() {
        let timing = CaptureTiming::parse(
            [
                "--wait-for-peer",
                "--warmup",
                "30",
                "--sample",
                "60",
                "--startup-timeout",
                "300",
            ]
            .map(String::from)
            .into_iter(),
        )
        .unwrap();
        assert!(timing.wait_for_peer);
        assert_eq!(timing.warmup, Duration::from_secs(30));
        assert_eq!(timing.sample, Duration::from_secs(60));
        assert_eq!(timing.deadline(), Duration::from_secs(390));
        assert!(CaptureTiming::parse(["--sample", "0"].map(String::from).into_iter()).is_err());
    }
}
