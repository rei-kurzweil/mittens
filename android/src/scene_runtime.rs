//! The app's engine scene. The host owns XR; this scene creates no XR system.
use crate::{
    graphics_device::Gpu,
    renderer::{SceneRenderer, Stats, view::EyeCamera},
    xr_host::{Report, Result},
};
use ash::vk;
use mittens_engine::engine::{Universe, ecs::World, user_input::InputState};
use std::{path::Path, sync::Arc};

pub(crate) struct SceneRuntime {
    universe: Universe,
    renderer: SceneRenderer,
    last_time: Option<i64>,
    announced: bool,
}
impl SceneRuntime {
    pub fn new(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        images: Vec<u64>,
        directory: &Path,
        report: &mut Report,
    ) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|e| format!("scene files directory: {e}"))?;
        let fragment = directory.join("mobile-albedo.frag");
        std::fs::write(&fragment, include_str!("../assets/albedo.frag"))
            .map_err(|e| format!("demo fragment: {e}"))?;
        // Use an escaped absolute URI because the engine compiler reads source
        // files asynchronously. The app owns this private file for its lifetime.
        let uri = fragment
            .to_str()
            .ok_or("non-UTF8 scene path")?
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let source = include_str!("../assets/scene.mms").replace("__FRAGMENT_PATH__", &uri);
        let mut universe = Universe::new(World::default());
        universe
            .load_mms_source_at_path(
                &source,
                &directory.join("mobile-scene.mms").to_string_lossy(),
            )
            .map_err(|e| format!("load mobile MMS scene: {e}"))?;
        let renderer =
            SceneRenderer::with_samples(gpu, format, extent, images, vk::SampleCountFlags::TYPE_4)?;
        report.line(renderer.attachment_policy());
        report.line("mobile scene loaded: two MMS cubes; generic custom opaque/albedo material; no legacy pipelines");
        Ok(Self {
            universe,
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
