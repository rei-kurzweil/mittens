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
    user_input::InputState,
};
use std::{path::Path, sync::Arc};

pub(crate) struct SceneRuntime {
    universe: Universe,
    renderer: SceneRenderer,
    last_time: Option<i64>,
    announced: bool,
}
pub(crate) struct PreparedScene {
    universe: Universe,
    label: &'static str,
}
impl PreparedScene {
    pub fn load(directory: &Path, scene: DemoScene) -> Result<Self> {
        let source = demo_assets::install(directory, scene)?;
        let mut universe = Universe::new(World::default());
        universe
            .load_mms_source_at_path(&source.source, &source.path.to_string_lossy())
            .map_err(|e| format!("load mobile MMS scene: {e}"))?;
        Ok(Self {
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
            renderer,
            last_time: None,
            announced: false,
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
        let dt = self.last_time.map_or(1.0 / 90.0, |last| {
            ((now - last) as f64 * 1e-9).clamp(0.0, 0.1) as f32
        });
        self.last_time = Some(now);
        self.universe.update(dt, &InputState::default());
        self.universe.systems.prepare_render(
            &mut self.universe.world,
            &mut self.universe.visuals,
            &mut self.universe.render_assets,
            &mut self.renderer,
            &mut self.universe.command_queue,
        );
        self.universe.visuals.prepare_draw_cache();
        let cameras = [
            EyeCamera::from_xr(&views[0])?,
            EyeCamera::from_xr(&views[1])?,
        ];
        let stats: Stats = self
            .renderer
            .draw(index, &self.universe.visuals, &cameras)?;
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
