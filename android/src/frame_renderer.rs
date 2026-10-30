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
pub(crate) struct FrameSetup {
    #[cfg(feature = "mobile-renderer")]
    scene: crate::scene_runtime::PreparedScene,
}
impl FrameSetup {
    pub fn new(directory: Option<&Path>) -> Result<Self> {
        #[cfg(feature = "mobile-renderer")]
        {
            let scene = if cfg!(feature = "cube-demo") {
                crate::demo_assets::DemoScene::Cubes
            } else {
                crate::demo_assets::DemoScene::SakuraStage
            };
            Ok(Self {
                scene: crate::scene_runtime::PreparedScene::load(
                    directory.ok_or("Android files directory is required for the mobile scene")?,
                    scene,
                )?,
            })
        }
        #[cfg(not(feature = "mobile-renderer"))]
        {
            let _ = directory;
            Ok(Self {})
        }
    }
    pub fn desired_xr_resolution(&self) -> Option<[u32; 2]> {
        #[cfg(feature = "mobile-renderer")]
        {
            self.scene.desired_xr_resolution()
        }
        #[cfg(not(feature = "mobile-renderer"))]
        {
            Some([1024, 1024])
        }
    }
}
impl FrameRenderer {
    pub fn new(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        images: Vec<u64>,
        setup: FrameSetup,
        report: &mut Report,
    ) -> Result<Self> {
        #[cfg(feature = "mobile-renderer")]
        {
            Ok(Self::Scene(crate::scene_runtime::SceneRuntime::new(
                gpu,
                format,
                extent,
                images,
                setup.scene,
                report,
            )?))
        }
        #[cfg(not(feature = "mobile-renderer"))]
        {
            let _ = (setup, report);
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
