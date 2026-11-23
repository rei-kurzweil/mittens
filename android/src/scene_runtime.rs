//! The app's engine scene. The host owns XR; this scene creates no XR system.
use crate::{
    demo_assets::{self, DemoScene},
    graphics_device::Gpu,
    renderer::{SceneRenderer, Stats, view::EyeCamera},
    xr_host::{Report, Result},
};
use ash::vk;
use mittens_engine::engine::{
    Universe,
    ecs::{World, component::RendererSettingsComponent},
    user_input::udp_text::UdpTerminalHost,
};
use std::{path::Path, sync::Arc};

pub(crate) struct SceneRuntime {
    universe: Universe,
    renderer: SceneRenderer,
    last_time: Option<i64>,
    announced: bool,
    profile: crate::frame_profile::FrameProfile,
    terminal: Option<UdpTerminalHost>,
    /// Kept to rebuild the renderer, and so drop the previous scene's GPU
    /// resources, whenever the launcher switches scenes.
    target: RenderTarget,
    #[cfg(feature = "launcher")]
    launcher: Option<crate::launcher::Launcher>,
    #[cfg_attr(not(feature = "launcher"), allow(dead_code))]
    menu_down: bool,
    /// The current scene was rejected; return home at the next frame boundary.
    #[cfg_attr(not(feature = "launcher"), allow(dead_code))]
    failed: bool,
}
struct RenderTarget {
    gpu: Arc<Gpu>,
    format: vk::Format,
    extent: vk::Extent2D,
    images: Vec<u64>,
}
impl RenderTarget {
    fn renderer(&self) -> Result<SceneRenderer> {
        SceneRenderer::with_samples(
            self.gpu.clone(),
            self.format,
            self.extent,
            self.images.clone(),
            vk::SampleCountFlags::TYPE_4,
        )
    }
}
pub(crate) struct PreparedScene {
    universe: Universe,
    label: &'static str,
    terminal: Option<UdpTerminalHost>,
    #[cfg(feature = "launcher")]
    launcher: Option<crate::launcher::Launcher>,
}
/// A scene whose XR input comes from the host; the scene creates no XR session.
fn new_universe(source: &str, path: &Path) -> Result<Universe> {
    let mut universe = Universe::new(World::default());
    universe
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    universe
        .load_mms_source_at_path(source, &path.to_string_lossy())
        .map_err(|e| format!("load mobile MMS scene: {e}"))?;
    Ok(universe)
}
#[cfg(feature = "launcher")]
fn load_universe(path: &Path) -> Result<Universe> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    new_universe(&source, path)
}
impl PreparedScene {
    pub fn load(directory: &Path, scene: DemoScene) -> Result<Self> {
        #[cfg(feature = "launcher")]
        if matches!(scene, DemoScene::Launcher) {
            let launcher = crate::launcher::Launcher::prepare(directory)?;
            return Ok(Self {
                universe: load_universe(launcher.home())?,
                label: "launcher",
                terminal: None,
                launcher: Some(launcher),
            });
        }
        let source = demo_assets::install(directory, scene)?;
        #[cfg(target_os = "android")]
        mittens_engine::engine::runtime_assets::set_root(&directory.join("mittens-demo"))?;
        let mut universe = new_universe(&source.source, &source.path)?;
        if matches!(scene, DemoScene::SakuraLean) {
            let colliders: Vec<_> = universe.world.all_components().filter(|&id| universe.world
                .get_component_by_id_as::<mittens_engine::engine::ecs::component::CollidableComponent>(id).is_some()).collect();
            for id in colliders {
                universe.world.get_component_by_id_as_mut::<mittens_engine::engine::ecs::component::CollidableComponent>(id).unwrap().enabled = false;
            }
        }
        let terminal = if matches!(scene, DemoScene::Shell) {
            Some(
                UdpTerminalHost::install(
                    &mut universe,
                    "0.0.0.0:7001".parse().unwrap(),
                    "shell_terminal_input",
                )
                .map_err(|error| format!("UDP terminal: {error}"))?,
            )
        } else {
            None
        };
        Ok(Self {
            terminal,
            universe,
            label: source.label,
            #[cfg(feature = "launcher")]
            launcher: None,
        })
    }

    pub fn desired_xr_resolution(&self) -> Option<[u32; 2]> {
        self.universe
            .world
            .all_components()
            .filter_map(|id| {
                self.universe
                    .world
                    .get_component_by_id_as::<RendererSettingsComponent>(id)
            })
            .last()
            .and_then(|settings| settings.desired_xr_resolution)
    }

    pub fn preferred_xr_refresh_rate(&self) -> Option<f32> {
        self.universe
            .world
            .all_components()
            .filter_map(|id| {
                self.universe
                    .world
                    .get_component_by_id_as::<RendererSettingsComponent>(id)
            })
            .last()
            .and_then(|settings| settings.preferred_xr_refresh_rate)
    }
}
impl SceneRuntime {
    pub fn foveation_quality(
        &self,
    ) -> mittens_engine::engine::ecs::system::graphics_settings_system::XrFoveationQuality {
        self.universe.systems.graphics_settings.foveation_quality()
    }

    pub fn set_foveation_feedback(
        &mut self,
        feedback: mittens_engine::engine::ecs::system::graphics_settings_system::XrFoveationFeedback,
    ) {
        if self
            .universe
            .systems
            .graphics_settings
            .foveation_feedback()
            .applied
            != feedback.applied
        {
            self.profile.reset(feedback.applied);
        }
        self.universe
            .systems
            .graphics_settings
            .set_foveation_feedback(feedback, &mut self.universe.command_queue);
    }

    pub fn set_external_frame(
        &mut self,
        frame: mittens_engine::engine::ecs::system::external_xr_frame::ExternalXrFrame,
    ) {
        self.universe.systems.xr.set_external_frame(Some(frame));
    }
    pub fn new(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        images: Vec<u64>,
        scene: PreparedScene,
        report: &mut Report,
    ) -> Result<Self> {
        let target = RenderTarget {
            gpu,
            format,
            extent,
            images,
        };
        let renderer = target.renderer()?;
        report.line(renderer.attachment_policy());
        report.line(format!("mobile scene loaded: {}; custom opaque materials, ambient/directional/point/spot lights, plain sky; no legacy pipelines", scene.label));
        Ok(Self {
            universe: scene.universe,
            terminal: scene.terminal,
            renderer,
            last_time: None,
            announced: false,
            profile: crate::frame_profile::FrameProfile::default(),
            target,
            #[cfg(feature = "launcher")]
            launcher: scene.launcher,
            menu_down: false,
            failed: false,
        })
    }

    /// Left menu button state from the host's controller sample.
    pub fn set_menu_down(&mut self, down: bool) {
        self.menu_down = down;
    }

    /// Replaces the scene at a frame boundary: home after a failure, a held
    /// menu button, or an `App` request. A scene that fails to load returns
    /// home with its error; a launcher that fails to load is fatal.
    #[cfg(feature = "launcher")]
    fn switch_scene(&mut self, dt: f32, report: &mut Report) -> Result<()> {
        use mittens_engine::engine::app_launch;
        let Some(launcher) = self.launcher.as_mut() else {
            return Ok(());
        };
        let next = if std::mem::take(&mut self.failed) {
            Some(launcher.home().to_path_buf())
        } else {
            launcher.poll(self.menu_down, dt)
        };
        let Some(mut path) = next else {
            return Ok(());
        };
        let universe = match load_universe(&path) {
            Ok(universe) => {
                if path != launcher.home() {
                    app_launch::set_last_error("");
                }
                universe
            }
            Err(error) if path != launcher.home() => {
                let shown = launcher.display(&path);
                report.line(format!("LAUNCH failed scene={shown} error={error}"));
                app_launch::set_last_error(format!("{shown}: {error}"));
                path = launcher.home().to_path_buf();
                load_universe(&path)?
            }
            Err(error) => return Err(error),
        };
        // Engine mesh/texture handles belong to one renderer instance, so the
        // previous scene's GPU resources go with it.
        self.renderer = self.target.renderer()?;
        self.universe = universe;
        self.terminal = None;
        self.announced = false;
        self.profile = crate::frame_profile::FrameProfile::default();
        launcher.set_current(&path);
        report.line(format!("LAUNCH scene={}", launcher.display(&path)));
        Ok(())
    }
    pub fn draw(
        &mut self,
        index: usize,
        views: &[openxr::View],
        time: openxr::Time,
        report: &mut Report,
    ) -> Result<&'static str> {
        let now = time.as_nanos();
        let interval = self.last_time.map(|last| (now - last) as f64 * 1e-6);
        let dt = self.last_time.map_or(1.0 / 90.0, |last| {
            ((now - last) as f64 * 1e-9).clamp(0.0, 0.1) as f32
        });
        self.last_time = Some(now);
        #[cfg(feature = "launcher")]
        self.switch_scene(dt, report)?;
        let start = std::time::Instant::now();
        let input = self.universe.input_providers.poll();
        if let Some(terminal) = &self.terminal {
            terminal.prepare(&mut self.universe, &input);
        }
        self.universe.update(dt, &input);
        let update_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = std::time::Instant::now();
        self.universe.systems.prepare_render(
            &mut self.universe.world,
            &mut self.universe.visuals,
            &mut self.universe.render_assets,
            &mut self.renderer,
            &mut self.universe.command_queue,
        );
        self.universe.visuals.prepare_draw_cache();
        let assets_ms = start.elapsed().as_secs_f64() * 1000.0;
        let cameras = if let Some(eyes) = self
            .universe
            .visuals
            .visual_camera(mittens_engine::engine::graphics::CameraTarget::Xr)
            .filter(|camera| camera.eyes.len() == 2)
        {
            [
                EyeCamera {
                    view: eyes.eyes[0].view,
                    proj: EyeCamera::from_xr(&views[0])?.proj,
                },
                EyeCamera {
                    view: eyes.eyes[1].view,
                    proj: EyeCamera::from_xr(&views[1])?.proj,
                },
            ]
        } else {
            [
                EyeCamera::from_xr(&views[0])?,
                EyeCamera::from_xr(&views[1])?,
            ]
        };
        let requested = self.foveation_quality();
        if let Some((feedback, error)) = self.renderer.set_foveation(requested, &cameras) {
            self.set_foveation_feedback(feedback);
            report.line(format!("XR_FOVEATION backend=Vulkan_fragment_density_map requested={} status={} applied={:?} eye_tracking=false error={error:?}",
                requested.as_str(), feedback.status.as_str(), feedback.applied));
        }
        let benchmark_ticket = self.universe.systems.benchmarks.frame_ticket();
        let stats: Stats = match self.renderer.draw(index, &self.universe.visuals, &cameras) {
            Ok(stats) => stats,
            #[cfg(feature = "launcher")]
            Err(error) if self.launcher.as_ref().is_some_and(|l| !l.at_home()) => {
                // Rejected before recording: the image is released unrendered
                // for one frame while the host returns home.
                report.line(format!("LAUNCH render rejected: {error}"));
                mittens_engine::engine::app_launch::set_last_error(error);
                self.failed = true;
                return Ok("scene_rejected");
            }
            Err(error) => return Err(error),
        };
        self.universe
            .systems
            .benchmarks
            .record_frame(benchmark_ticket, stats.gpu_ms);
        self.profile
            .record(interval, update_ms, assets_ms, stats, report);
        if stats.draws > 0 && !self.announced {
            report.line(format!(
                "custom scene GPU complete: {stats:?}; draws include both eyes"
            ));
            self.announced = true;
        }
        Ok(if stats.draws > 0 {
            "custom_opaque_scene"
        } else {
            "custom_material_pending"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mittens_engine::engine::ecs::component::{
        CollidableComponent, EditorUIComponent, GravityComponent, InputXRGamepadComponent,
        XrHandPreference,
    };

    #[test]
    fn lean_scene_disables_all_colliders_and_uses_left_stick() {
        let dir = std::env::temp_dir().join(format!("mittens-lean-test-{}", std::process::id()));
        let scene = PreparedScene::load(&dir, DemoScene::SakuraLean).unwrap();
        let components = scene.universe.world.all_components().collect::<Vec<_>>();
        let colliders: Vec<_> = components
            .iter()
            .filter_map(|&id| {
                scene
                    .universe
                    .world
                    .get_component_by_id_as::<CollidableComponent>(id)
            })
            .collect();
        assert!(
            !colliders.is_empty(),
            "building should have authored collision components"
        );
        assert!(colliders.iter().all(|collider| !collider.enabled));
        assert!(components.iter().all(|&id| {
            scene
                .universe
                .world
                .get_component_by_id_as::<EditorUIComponent>(id)
                .is_none()
        }));
        assert!(components.iter().all(|&id| {
            scene
                .universe
                .world
                .get_component_by_id_as::<GravityComponent>(id)
                .is_none()
        }));
        assert!(
            components
                .iter()
                .filter_map(|&id| scene
                    .universe
                    .world
                    .get_component_by_id_as::<InputXRGamepadComponent>(id))
                .any(|gamepad| gamepad.locomotion && gamepad.hand == XrHandPreference::Left)
        );
    }
}
