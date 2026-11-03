//! Fixed, projection-aware Vulkan density maps. No headset-vendor runtime API.
use super::{pass::EyePass, resources::upload_density, view::EyeCamera};
use crate::{graphics_device::Gpu, xr_host::Result};
use ash::vk;
use mittens_engine::engine::ecs::system::graphics_settings_system::{
    XrFoveationFeedback as Feedback, XrFoveationQuality as Quality, XrFoveationStatus as Status,
};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
struct Request {
    quality: Quality,
    projections: [[[f32; 4]; 4]; 2],
}
pub(super) struct FixedFoveation {
    gpu: Arc<Gpu>,
    format: vk::Format,
    extent: vk::Extent2D,
    images: Vec<u64>,
    samples: vk::SampleCountFlags,
    last_request: Option<Request>,
    cached: Option<Request>,
    pass: Option<EyePass>,
    feedback: Feedback,
}
impl FixedFoveation {
    pub fn new(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        images: Vec<u64>,
        samples: vk::SampleCountFlags,
    ) -> Self {
        Self {
            gpu,
            format,
            extent,
            images,
            samples,
            last_request: None,
            cached: None,
            pass: None,
            feedback: Feedback::default(),
        }
    }
    pub fn pass(&self) -> Option<&EyePass> {
        self.feedback
            .applied
            .filter(|q| *q != Quality::Off)
            .and(self.pass.as_ref())
    }
    /// Called between completed frames. An unchanged request does no GPU work,
    /// allocation, logging, or map generation. A failed change keeps the old pass.
    pub fn update(
        &mut self,
        quality: Quality,
        cameras: &[EyeCamera; 2],
    ) -> Option<(Feedback, Option<String>)> {
        let request = Request {
            quality,
            projections: if quality == Quality::Off {
                [[[0.0; 4]; 4]; 2]
            } else {
                cameras.map(|camera| camera.proj)
            },
        };
        if self.last_request == Some(request) {
            return None;
        }
        self.last_request = Some(request);
        if self.gpu.density_texel.is_none() {
            self.feedback = Feedback::default();
            return Some((self.feedback, None));
        }
        let result = if quality == Quality::Off || self.cached == Some(request) {
            Ok(())
        } else {
            self.build(request)
        };
        let error = match result {
            Ok(()) => {
                self.feedback = Feedback {
                    status: Status::Accepted,
                    applied: Some(quality),
                };
                None
            }
            Err(error) => {
                self.feedback.status = Status::Failed;
                Some(error)
            }
        };
        Some((self.feedback, error))
    }
    fn build(&mut self, request: Request) -> Result<()> {
        let texel = self.gpu.density_texel.ok_or("density maps unavailable")?;
        let extent = map_extent(self.extent, texel);
        let left = density_data(self.extent, texel, request.projections[0], request.quality)?;
        let right = density_data(self.extent, texel, request.projections[1], request.quality)?;
        let maps = [
            upload_density(self.gpu.clone(), extent, &left)?,
            upload_density(self.gpu.clone(), extent, &right)?,
        ];
        let pass = EyePass::with_density(
            self.gpu.clone(),
            self.format,
            self.extent,
            &self.images,
            self.samples,
            Some(maps),
        )?;
        self.pass = Some(pass);
        self.cached = Some(request);
        Ok(())
    }
}
fn map_extent(eye: vk::Extent2D, texel: vk::Extent2D) -> vk::Extent2D {
    vk::Extent2D {
        width: eye.width.div_ceil(texel.width),
        height: eye.height.div_ceil(texel.height),
    }
}
/// Amount increases by shrinking the full-density cone and coarsening the edge.
/// Values are rounded DOWN in UNORM so reciprocal density permits 2x2/4x4.
fn density(angle: f32, quality: Quality) -> u8 {
    let (full, half) = match quality {
        Quality::Off => return 255,
        Quality::Low => (35.0, f32::INFINITY),
        Quality::Medium => (30.0, 45.0),
        Quality::High => (20.0, 35.0),
    };
    if angle <= full {
        255
    } else if angle <= half {
        127
    } else {
        63
    }
}
fn density_data(
    eye: vk::Extent2D,
    texel: vk::Extent2D,
    proj: [[f32; 4]; 4],
    quality: Quality,
) -> Result<Vec<u8>> {
    if !proj.iter().flatten().all(|v| v.is_finite()) || proj[0][0] <= 0.0 || proj[1][1] == 0.0 {
        return Err("invalid projection for fixed foveation".into());
    }
    let extent = map_extent(eye, texel);
    let mut data = Vec::with_capacity(extent.width as usize * extent.height as usize * 2);
    for y in 0..extent.height {
        for x in 0..extent.width {
            // Unproject into an eye-local ray; this keeps each eye's full-density
            // cone on its forward axis even for asymmetric XR projections.
            let nx = 2.0 * ((x as f32 + 0.5) * texel.width as f32) / eye.width as f32 - 1.0;
            let ny = 2.0 * ((y as f32 + 0.5) * texel.height as f32) / eye.height as f32 - 1.0;
            let tx = (nx + proj[2][0]) / proj[0][0];
            let ty = (ny + proj[2][1]) / proj[1][1];
            let angle = tx.hypot(ty).atan().to_degrees();
            let value = density(angle, quality);
            data.extend_from_slice(&[value, value]);
        }
    }
    Ok(data)
}

#[cfg(test)]
#[path = "foveation_tests.rs"]
mod tests;
