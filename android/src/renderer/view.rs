//! OpenXR view conversion and the existing MeshSurfaceV1 camera ABI.
use crate::xr_host::Result;
use ash::vk;
use mittens_engine::{
    engine::{ecs::Transform, graphics::visual_world::VisualWorld},
    utils::math::mat4_inverse,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct EyeCamera {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
}
impl EyeCamera {
    pub fn from_xr(view: &openxr::View) -> Result<Self> {
        let p = view.pose.position;
        let q = view.pose.orientation;
        let mut transform = Transform::default();
        transform.translation = [p.x, p.y, p.z];
        transform.rotation = [q.x, q.y, q.z, q.w];
        transform.recompute_model();
        let view_matrix = mat4_inverse(transform.model).ok_or("invalid OpenXR eye pose")?;
        let fov = view.fov;
        let (l, r, d, u) = (
            fov.angle_left.tan(),
            fov.angle_right.tan(),
            fov.angle_down.tan(),
            fov.angle_up.tan(),
        );
        let (width, height) = (r - l, u - d);
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err("invalid OpenXR eye FOV".into());
        }
        let (near, far) = (0.05, 100.0);
        // Right handed, zero-to-one depth, Vulkan framebuffer Y direction.
        let proj = [
            [2.0 / width, 0.0, 0.0, 0.0],
            [0.0, -2.0 / height, 0.0, 0.0],
            [
                (r + l) / width,
                -(u + d) / height,
                -far / (far - near),
                -1.0,
            ],
            [0.0, 0.0, -(far * near) / (far - near), 0.0],
        ];
        Ok(Self {
            view: view_matrix,
            proj,
        })
    }
}

#[repr(C)]
pub(super) struct CameraUniform {
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    camera2d: [[f32; 4]; 3],
    viewport: [f32; 2],
    pad: [f32; 2],
    ambient: [f32; 3],
    flags: u32,
}
impl CameraUniform {
    pub fn new(camera: &EyeCamera, visuals: &VisualWorld, extent: vk::Extent2D) -> Self {
        Self {
            view: camera.view,
            proj: camera.proj,
            camera2d: visuals.camera_2d(),
            viewport: [extent.width as f32, extent.height as f32],
            pad: [0.0; 2],
            ambient: visuals.ambient_light(),
            flags: 0,
        }
    }
}
const _: () = assert!(std::mem::size_of::<CameraUniform>() == 208);
