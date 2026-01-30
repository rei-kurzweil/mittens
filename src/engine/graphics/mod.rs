pub mod mesh;
pub mod pipeline_descriptor_set_layouts;
pub mod primitives;
pub mod render_assets;
pub mod render_info;
pub mod visual_world;
pub mod vulkano_renderer;
pub(crate) mod vulkano_swapchain;
pub mod xr_swapchain;

pub use mesh::{CpuMesh, CpuVertex, MeshFactory};
#[allow(unused_imports)]
pub use primitives::{
    GpuRenderable, Material, MaterialHandle, MeshHandle, Renderable, TextureHandle, Transform,
    TransformMatrix,
};

pub use render_assets::{BuiltinMeshType, RenderAssets};
pub use visual_world::TextureFiltering;
pub use visual_world::VisualWorld;
pub use visual_world::{CameraData, CameraTarget, VisualCamera};
pub(crate) mod vulkano_texture_upload;
pub use vulkano_renderer::VulkanoRenderer;
pub use xr_swapchain::XRSwapchain;

/// Minimal Vulkan handle bundle for OpenXR session creation.
///
/// OpenXR wants raw Vulkan handles as opaque pointers.
#[derive(Debug, Clone, Copy)]
pub struct XrVulkanGraphics {
    pub vk_instance: openxr::sys::platform::VkInstance,
    pub vk_physical_device: openxr::sys::platform::VkPhysicalDevice,
    pub vk_device: openxr::sys::platform::VkDevice,
    pub queue_family_index: u32,
    pub queue_index: u32,
}

pub use render_info::RenderInfo;
/// Trait for uploading CPU meshes to GPU.
/// This abstraction allows different renderer implementations
/// to provide mesh uploading functionality without exposing renderer-specific details.
pub trait MeshUploader {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>>;
}

/// Trait for uploading decoded textures to the GPU.
///
/// Textures are provided as RGBA8 pixels.
pub trait TextureUploader {
    fn upload_texture_rgba8(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>>;

    fn upload_texture_bc7(
        &mut self,
        bc7_blocks: &[u8],
        width: u32,
        height: u32,
        srgb: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>>;
}

/// Convenience super-trait for types that can upload both meshes and textures.
pub trait RenderUploader: MeshUploader + TextureUploader {}

impl<T> RenderUploader for T where T: MeshUploader + TextureUploader {}

/// Graphics/Vulkan placeholder.
pub struct Graphics;

impl Graphics {
    pub fn new() -> Self {
        Self
    }
}
