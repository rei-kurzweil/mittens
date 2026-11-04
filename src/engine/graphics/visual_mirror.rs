//! Retained mirror identities and per-view capture state.
use super::primitives::InstanceHandle;
use super::visual_world::CameraData;
use crate::engine::ecs::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirrorViewerFamily {
    Monoscopic,
    Stereoscopic,
}

impl MirrorViewerFamily {
    pub fn key_segment(self) -> &'static str {
        match self {
            Self::Monoscopic => "mono",
            Self::Stereoscopic => "stereo",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MirrorCaptureRequest {
    pub family: MirrorViewerFamily,
    pub enabled: bool,
    pub view_index: usize,
    pub camera: CameraData,
    pub target_key: String,
}

#[derive(Debug, Clone)]
pub struct VisualMirror {
    pub mirror_component: ComponentId,
    /// Producer lifetime is independent of transient tracking/view availability.
    pub capture_prefix: String,
    pub captures: Vec<MirrorCaptureRequest>,
    pub plane_origin: [f32; 3],
    pub plane_normal: [f32; 3],
    pub aspect_ratio: f32,
    pub source_instance: InstanceHandle,
    pub resolution_scale: f32,
}

impl MirrorCaptureRequest {
    #[inline]
    pub(crate) fn enabled_for(&self, family: MirrorViewerFamily) -> bool {
        self.enabled && self.family == family
    }
}
