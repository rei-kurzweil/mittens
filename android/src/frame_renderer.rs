//! Feature selection at the frame boundary, outside all draw loops.
use crate::{
    graphics_device::Gpu,
    xr_host::{Report, Result},
};
use ash::vk;
use std::{path::Path, sync::Arc};

pub(crate) enum FrameRenderer {
    #[cfg(not(feature = "mobile-renderer"))]
    Triangle(crate::triangle_probe::Renderer),
    #[cfg(feature = "mobile-renderer")]
    Scene(crate::scene_runtime::SceneRuntime),
}
impl FrameRenderer {
    pub fn new(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        images: Vec<u64>,
        directory: Option<&Path>,
        report: &mut Report,
    ) -> Result<Self> {
        #[cfg(feature = "mobile-renderer")]
        {
            Ok(Self::Scene(crate::scene_runtime::SceneRuntime::new(
                gpu,
                format,
                extent,
                images,
                directory.ok_or("Android files directory is required for the mobile scene")?,
                report,
                if cfg!(feature = "cube-demo") {
                    crate::demo_assets::DemoScene::Cubes
                } else {
                    crate::demo_assets::DemoScene::SakuraStage
                },
            )?))
        }
        #[cfg(not(feature = "mobile-renderer"))]
        {
            let _ = (directory, report);
            Ok(Self::Triangle(crate::triangle_probe::Renderer::new(
                gpu, format, extent, images,
            )?))
        }
    }
    pub fn draw(
        &mut self,
        index: usize,
        views: &[openxr::View],
        time: openxr::Time,
        triangle: bool,
        report: &mut Report,
    ) -> Result<&'static str> {
        match self {
            #[cfg(feature = "mobile-renderer")]
            Self::Scene(scene) => {
                let _ = triangle;
                scene.draw(index, views, time, report)
            }
            #[cfg(not(feature = "mobile-renderer"))]
            Self::Triangle(renderer) => {
                let _ = (views, time, report);
                renderer.draw(index, triangle)?;
                Ok(if triangle {
                    "stereo_triangle"
                } else {
                    "stereo_clear"
                })
            }
        }
    }
}
