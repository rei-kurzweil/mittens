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
}
pub(crate) struct PreparedScene {
    universe: Universe,
    label: &'static str,
    terminal: Option<UdpTerminalHost>,
}
impl PreparedScene {
    pub fn load(directory: &Path, scene: DemoScene) -> Result<Self> {
        let source = demo_assets::install(directory, scene)?;
        #[cfg(target_os = "android")]
        mittens_engine::engine::runtime_assets::set_root(&directory.join("mittens-demo"))?;
        let mut universe = Universe::new(World::default());
        universe
            .systems
            .xr
            .set_external_frame(Some(Default::default()));
        universe
            .load_mms_source_at_path(&source.source, &source.path.to_string_lossy())
            .map_err(|e| format!("load mobile MMS scene: {e}"))?;
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
}
impl SceneRuntime {
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
        let renderer =
            SceneRenderer::with_samples(gpu, format, extent, images, vk::SampleCountFlags::TYPE_4)?;
        report.line(renderer.attachment_policy());
        report.line(format!("mobile scene loaded: {}; custom opaque materials, ambient/directional/point/spot lights, plain sky; no legacy pipelines", scene.label));
        Ok(Self {
            universe: scene.universe,
            terminal: scene.terminal,
            renderer,
            last_time: None,
            announced: false,
            profile: crate::frame_profile::FrameProfile::default(),
        })
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
        let stats: Stats = self
            .renderer
            .draw(index, &self.universe.visuals, &cameras)?;
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
