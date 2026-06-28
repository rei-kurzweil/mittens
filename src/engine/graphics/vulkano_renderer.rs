use crate::engine::graphics::MeshUploader;
use crate::engine::graphics::MsaaMode;
use crate::engine::graphics::TextureUploader;
use crate::engine::graphics::mesh::CpuMesh;
use crate::engine::graphics::primitives::MeshHandle;
use crate::engine::graphics::primitives::TextureHandle;
use crate::engine::graphics::visual_world::VisualWorld;
use std::sync::Arc;
use winit::window::Window;

mod vulkano_backend {
    use std::collections::HashMap;
    use std::mem::size_of;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use crate::engine::ecs::ComponentId;
    use crate::engine::ecs::system::render_to_texture_system::INTERNAL_RENDERER_STENCIL_CLIP_DEBUG_SELECTOR;
    use crate::engine::graphics::MsaaMode;
    use crate::engine::graphics::mesh::{CpuMesh, CpuVertex};
    use crate::engine::graphics::pipeline_descriptor_set_layouts::PipelineDescriptorSetLayouts;
    use crate::engine::graphics::post_processing::{
        PostProcessFrameTargets, PostProcessingConfig, PostProcessingRenderer,
    };
    use crate::engine::graphics::primitives::MeshHandle;
    use crate::engine::graphics::primitives::TextureHandle;
    use crate::engine::graphics::visual_world::{TextureFiltering, VisualWorld};
    use crate::engine::graphics::vulkano_swapchain::VulkanoSwapchainState;
    use crate::engine::graphics::vulkano_texture_upload;
    use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
    use vulkano::command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo, CopyImageInfo,
        PrimaryCommandBufferAbstract, allocator::StandardCommandBufferAllocator,
    };
    use vulkano::command_buffer::{
        ClearAttachment, ClearRect, RenderingAttachmentInfo, RenderingAttachmentResolveInfo,
        RenderingInfo,
    };
    use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
    use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
    use vulkano::format::ClearValue;
    use vulkano::image::view::{ImageView, ImageViewCreateInfo};
    use vulkano::image::{
        Image, ImageAspects, ImageCreateInfo, ImageSubresourceRange, ImageType, ImageUsage,
        SampleCount, SampleCounts,
    };
    use vulkano::memory::allocator::{
        AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator,
    };
    use vulkano::pipeline::graphics::color_blend::{
        AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState,
        ColorComponents,
    };
    use vulkano::pipeline::graphics::depth_stencil::{
        CompareOp, DepthState, DepthStencilState, StencilOp, StencilOpState, StencilOps,
        StencilState,
    };
    use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
    use vulkano::pipeline::graphics::multisample::MultisampleState;
    use vulkano::pipeline::graphics::rasterization::RasterizationState;
    use vulkano::pipeline::graphics::subpass::PipelineRenderingCreateInfo;
    use vulkano::pipeline::graphics::subpass::PipelineSubpassType;
    use vulkano::pipeline::graphics::vertex_input::{
        VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
        VertexInputState,
    };
    use vulkano::pipeline::graphics::viewport::{Scissor, Viewport, ViewportState};
    use vulkano::pipeline::layout::{PipelineLayout, PipelineLayoutCreateInfo};

    use vulkano::DeviceSize;
    use vulkano::Version;
    use vulkano::VulkanObject;
    use vulkano::format::Format;
    use vulkano::image::sampler::{
        Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode,
    };
    use vulkano::pipeline::{
        DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineShaderStageCreateInfo,
    };
    use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
    use vulkano::swapchain::{self, SwapchainPresentInfo};
    use vulkano::sync::{self, GpuFuture};
    use vulkano::{Validated, VulkanError};
    use vulkano_util::context::{VulkanoConfig, VulkanoContext};
    use winit::window::Window;

    fn env_flag(name: &str) -> bool {
        std::env::var(name)
            .ok()
            .map(|s| {
                let s = s.trim().to_ascii_lowercase();
                s == "1" || s == "true" || s == "on" || s == "yes"
            })
            .unwrap_or(false)
    }

    fn env_usize(name: &str) -> Option<usize> {
        std::env::var(name)
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
    }

    use vulkano::device::DeviceExtensions;

    // Split out command-buffer recording helpers to keep this file manageable.
    //
    // `vulkano_backend` is an inline module, so `#[path = "..."]` would be resolved relative to
    // a *virtual* module directory (`.../vulkano_renderer/vulkano_backend/`) that doesn't exist
    // on disk. Using `include!` lets us keep the helpers in a normal file next to the renderer.
    mod vulkano_cbb {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/engine/graphics/vulkano_cbb.rs"
        ));
    }

    mod toon_mesh_vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            path: "assets/shaders/toon-mesh.vert",
        }
    }

    mod mirror_mesh_vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            path: "assets/shaders/mirror-mesh.vert",
        }
    }

    mod toon_mesh_fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            path: "assets/shaders/toon-mesh.frag",
        }
    }

    mod emissive_toon_mesh_fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            path: "assets/shaders/emissive-toon-mesh.frag",
        }
    }

    mod mirror_mesh_fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            path: "assets/shaders/mirror-mesh.frag",
        }
    }

    mod skinned_toon_mesh_vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            path: "assets/shaders/skinned-toon-mesh.vert",
        }
    }

    mod grid_mesh_vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            path: "assets/shaders/grid.vert",
        }
    }

    mod grid_square_mesh_fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            path: "assets/shaders/grid-square.frag",
        }
    }

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    pub struct CameraUBO {
        pub view: [[f32; 4]; 4],
        pub proj: [[f32; 4]; 4],
        // std140 mat3 = 3x vec4 columns.
        pub camera2d: [[f32; 4]; 3],
        // Swapchain size in pixels (width, height). Used for aspect correction in 2D.
        pub viewport: [f32; 2],
        pub _pad0: [f32; 2],

        // Linear RGB ambient light in 0..1.
        pub ambient_light: [f32; 3],
        pub _pad1: f32,
    }

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    struct MaterialUBO {
        base_color: [f32; 4],
        quant_steps: f32,
        emissive: u32,
        _pad0: u32,
        _pad1: u32,
    }

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    struct GpuMat4 {
        cols: [[f32; 4]; 4],
    }

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    struct DummyPerInstanceLightingSSBO {
        _pad0: [u32; 4],
    }

    #[derive(
        BufferContents,
        vulkano::pipeline::graphics::vertex_input::Vertex,
        Clone,
        Copy,
        Debug,
        Default,
    )]
    #[repr(C)]
    pub struct InstanceData {
        #[format(R32G32B32A32_SFLOAT)]
        pub i_model_c0: [f32; 4],
        #[format(R32G32B32A32_SFLOAT)]
        pub i_model_c1: [f32; 4],
        #[format(R32G32B32A32_SFLOAT)]
        pub i_model_c2: [f32; 4],
        #[format(R32G32B32A32_SFLOAT)]
        pub i_model_c3: [f32; 4],

        #[format(R32G32B32A32_SFLOAT)]
        pub i_color: [f32; 4],
        #[format(R32_SFLOAT)]
        pub i_emissive: f32,

        #[format(R32_SFLOAT)]
        pub i_opacity: f32,

        // For skinned meshes: base index/count into the shared bones palette SSBO.
        // For non-skinned instances these are 0.
        #[format(R32_UINT)]
        pub i_bones_base: u32,
        #[format(R32_UINT)]
        pub i_bones_count: u32,
    }

    /// GPU-uploadable per-vertex skinning attributes (separate vertex buffer).
    #[derive(BufferContents, Debug, Clone, Copy, Default)]
    #[repr(C)]
    pub struct GpuSkinVertex {
        pub joints0: [u16; 4],
        pub weights0: [f32; 4],
    }

    #[derive(Debug, Clone)]
    pub enum RenderViewKind {
        Window,
        XrEye {
            eye: usize,
        },
        Mirror {
            mirror_component: ComponentId,
            family: crate::engine::graphics::visual_world::MirrorViewerFamily,
            view_index: usize,
            plane_origin: [f32; 3],
            plane_normal: [f32; 3],
            excluded_instance: Option<crate::engine::graphics::primitives::InstanceHandle>,
        },
    }

    #[derive(Debug, Clone)]
    pub struct RenderView {
        pub view: [[f32; 4]; 4],
        pub proj: [[f32; 4]; 4],
        pub viewport: [f32; 2],
        pub kind: RenderViewKind,
    }

    pub struct VulkanoGpuMesh {
        #[allow(dead_code)]
        pub vertices: Subbuffer<[CpuVertex]>,
        #[allow(dead_code)]
        pub skin_vertices: Option<Subbuffer<[GpuSkinVertex]>>,
        #[allow(dead_code)]
        pub indices: Subbuffer<[u32]>,
        #[allow(dead_code)]
        pub index_count: u32,
    }

    pub struct VulkanoGpuTexture {
        pub view: Arc<ImageView>,
        pub extent: [u32; 2],
        pub format: Format,
    }

    pub struct VulkanoState {
        #[allow(dead_code)]
        pub context: VulkanoContext,
        #[allow(dead_code)]
        pub window: Arc<Window>,

        #[allow(dead_code)]
        pub swapchain_state: VulkanoSwapchainState,

        #[allow(dead_code)]
        pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,

        #[allow(dead_code)]
        pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,

        pub post_processing_renderer: PostProcessingRenderer,

        #[allow(dead_code)]
        pub set_layouts: PipelineDescriptorSetLayouts,

        #[allow(dead_code)]
        pub meshes: HashMap<MeshHandle, VulkanoGpuMesh>,

        pub textures: HashMap<TextureHandle, VulkanoGpuTexture>,
        pub sampler_linear: Arc<Sampler>,
        pub sampler_nearest: Arc<Sampler>,
        pub sampler_nearest_mag: Arc<Sampler>,
        pub default_white_texture: TextureHandle,

        pub pipeline_toon_mesh: Arc<GraphicsPipeline>,
        pub pipeline_toon_mesh_transparent: Arc<GraphicsPipeline>,
        pub pipeline_toon_mesh_cutout: Arc<GraphicsPipeline>,
        pub pipeline_toon_mesh_transparent_clipped: Arc<GraphicsPipeline>,
        pub pipeline_toon_mesh_cutout_clipped: Arc<GraphicsPipeline>,

        pub pipeline_mirror_mesh: Arc<GraphicsPipeline>,
        pub pipeline_mirror_mesh_transparent: Arc<GraphicsPipeline>,
        pub pipeline_mirror_mesh_cutout: Arc<GraphicsPipeline>,
        pub pipeline_mirror_mesh_clipped: Arc<GraphicsPipeline>,
        pub pipeline_mirror_mesh_transparent_clipped: Arc<GraphicsPipeline>,
        pub pipeline_mirror_mesh_cutout_clipped: Arc<GraphicsPipeline>,

        pub pipeline_grid_mesh: Arc<GraphicsPipeline>,
        pub pipeline_grid_mesh_transparent: Arc<GraphicsPipeline>,
        pub pipeline_grid_mesh_transparent_clipped: Arc<GraphicsPipeline>,

        pub pipeline_emissive_toon_mesh: Arc<GraphicsPipeline>,
        pub pipeline_emissive_toon_mesh_transparent: Arc<GraphicsPipeline>,
        pub pipeline_emissive_toon_mesh_cutout: Arc<GraphicsPipeline>,
        pub pipeline_emissive_toon_mesh_transparent_clipped: Arc<GraphicsPipeline>,
        pub pipeline_emissive_toon_mesh_cutout_clipped: Arc<GraphicsPipeline>,
        pub pipeline_emissive_prepass_toon_mesh: Arc<GraphicsPipeline>,
        pub pipeline_emissive_prepass_toon_mesh_cutout: Arc<GraphicsPipeline>,

        pub pipeline_skinned_toon_mesh: Arc<GraphicsPipeline>,
        pub pipeline_skinned_toon_mesh_transparent: Arc<GraphicsPipeline>,
        pub pipeline_skinned_toon_mesh_cutout: Arc<GraphicsPipeline>,

        pub pipeline_skinned_emissive_toon_mesh: Arc<GraphicsPipeline>,
        pub pipeline_skinned_emissive_toon_mesh_transparent: Arc<GraphicsPipeline>,
        pub pipeline_skinned_emissive_toon_mesh_cutout: Arc<GraphicsPipeline>,
        pub pipeline_skinned_emissive_prepass_toon_mesh: Arc<GraphicsPipeline>,
        pub pipeline_skinned_emissive_prepass_toon_mesh_cutout: Arc<GraphicsPipeline>,

        /// Writes stencil INCR (enter clip region). Color write off, depth test off.
        pub pipeline_stencil_incr: Arc<GraphicsPipeline>,
        /// Writes stencil DECR (exit clip region). Color write off, depth test off.
        pub pipeline_stencil_decr: Arc<GraphicsPipeline>,
        /// Overlay draw gated by stencil EQUAL. For non-emissive materials.
        pub pipeline_overlay_clipped: Arc<GraphicsPipeline>,
        /// Overlay draw gated by stencil EQUAL. For emissive materials.
        pub pipeline_emissive_overlay_clipped: Arc<GraphicsPipeline>,
        /// Opaque draw gated by stencil EQUAL. Depth write ON. For non-emissive materials.
        pub pipeline_opaque_clipped: Arc<GraphicsPipeline>,
        /// Opaque draw gated by stencil EQUAL. Depth write ON. For emissive materials.
        pub pipeline_emissive_opaque_clipped: Arc<GraphicsPipeline>,

        pub msaa_samples: SampleCount,

        // --- Per-frame CPU work reduction ---
        cached_instance_buffer: Option<Subbuffer<[InstanceData]>>,
        cached_instance_count: usize,

        cached_background_instance_buffer: Option<Subbuffer<[InstanceData]>>,
        cached_background_instance_count: usize,

        cached_background_occluded_lit_instance_buffer: Option<Subbuffer<[InstanceData]>>,
        cached_background_occluded_lit_instance_count: usize,

        cached_cutout_instance_buffer: Option<Subbuffer<[InstanceData]>>,
        cached_cutout_instance_count: usize,

        cached_overlay_instance_buffer: Option<Subbuffer<[InstanceData]>>,
        cached_overlay_instance_count: usize,
        cached_material_sets: HashMap<
            (
                crate::engine::graphics::MaterialHandle,
                TextureHandle,
                TextureFiltering,
                u32,
            ),
            Arc<DescriptorSet>,
        >,
        pending_runtime_texture_updates: HashMap<TextureHandle, VulkanoGpuTexture>,
        window_runtime_debug_targets: Option<WindowRuntimeDebugTargets>,

        // Cached bones palette SSBOs (set=2 binding=1).
        //
        // These are per-frame slots (swapchain image index + optional XR eye slots) to avoid
        // writing a buffer while the GPU is still reading it from a previous frame.
        cached_bones_buffers: Vec<Subbuffer<[GpuMat4]>>,
        cached_bones_slot_valid: Vec<bool>,
        cached_bones_capacity: usize,

        xr_offscreen: Option<XrOffscreenTargets>,
        mirror_offscreen: std::collections::HashMap<String, MirrorOffscreenTargets>,

        pub window_resized: bool,
        pub recreate_swapchain: bool,
        pub images_in_flight: Vec<Option<Box<dyn GpuFuture>>>,
    }

    struct XrOffscreenTargets {
        extent: [u32; 2],
        color_format: Format,
        color_images: Vec<Arc<vulkano::image::Image>>,
        msaa_color_views: Vec<Arc<ImageView>>,
        color_views: Vec<Arc<ImageView>>,
        /// Combined depth+stencil views (same view used for both attachments).
        depth_views: Vec<Arc<ImageView>>,
    }

    struct MirrorOffscreenTargets {
        extent: [u32; 2],
        color_format: Format,
        color_images: Vec<Arc<vulkano::image::Image>>,
        msaa_color_views: Vec<Arc<ImageView>>,
        color_views: Vec<Arc<ImageView>>,
        /// Combined depth+stencil views (same view used for both attachments).
        depth_views: Vec<Arc<ImageView>>,
    }

    struct WindowRuntimeDebugTargets {
        extent: [u32; 2],
        color_format: Format,
        msaa_color_views: Vec<Arc<ImageView>>,
        color_views: Vec<Arc<ImageView>>,
    }

    #[derive(Clone)]
    struct PostProcessInvocation {
        final_output_view: Arc<ImageView>,
        final_color_format: Format,
        config: PostProcessingConfig,
        targets: PostProcessFrameTargets,
    }

    const MAX_LIGHTS: usize = 64;

    const LIGHT_TYPE_POINT: u32 = 1;
    const LIGHT_TYPE_DIRECTIONAL: u32 = 2;

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    struct GpuLight {
        // xyz position (world), w intensity
        pos_intensity: [f32; 4],
        // rgb color, w distance
        color_distance: [f32; 4],
        // Light metadata (matches `uvec4 meta` on the shader side).
        // meta.x = light_type (1=point, 2=directional)
        meta: [u32; 4],
    }

    #[derive(BufferContents, Clone, Copy, Debug)]
    #[repr(C, align(16))]
    struct LightsSSBO {
        count: u32,
        _pad0: [u32; 3],
        lights: [GpuLight; MAX_LIGHTS],
    }

    impl Default for LightsSSBO {
        fn default() -> Self {
            Self {
                count: 0,
                _pad0: [0, 0, 0],
                lights: [GpuLight::default(); MAX_LIGHTS],
            }
        }
    }

    impl VulkanoState {
        fn sampler_for(&self, filtering: TextureFiltering) -> &Arc<Sampler> {
            match filtering {
                TextureFiltering::Linear => &self.sampler_linear,
                TextureFiltering::Nearest => &self.sampler_nearest,
                TextureFiltering::NearestMagnification => &self.sampler_nearest_mag,
            }
        }

        fn create_material_ubo(
            material: crate::engine::graphics::MaterialHandle,
            quant_steps: f32,
        ) -> MaterialUBO {
            let quant_steps = if quant_steps.is_finite() {
                quant_steps.clamp(1.0, 64.0)
            } else {
                3.0
            };

            match material {
                crate::engine::graphics::MaterialHandle::TOON_MESH => MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps,
                    emissive: 0,
                    _pad0: 0,
                    _pad1: 0,
                },
                crate::engine::graphics::MaterialHandle::SKINNED_TOON_MESH => MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps,
                    emissive: 0,
                    _pad0: 0,
                    _pad1: 0,
                },
                crate::engine::graphics::MaterialHandle::EMISSIVE_TOON_MESH => MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps,
                    emissive: 0,
                    _pad0: 0,
                    _pad1: 0,
                },
                crate::engine::graphics::MaterialHandle::SKINNED_EMISSIVE_TOON_MESH => {
                    MaterialUBO {
                        base_color: [1.0, 1.0, 1.0, 1.0],
                        quant_steps,
                        emissive: 0,
                        _pad0: 0,
                        _pad1: 0,
                    }
                }
                crate::engine::graphics::MaterialHandle::GRID_MESH => MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps: 1.0,
                    emissive: 1,
                    _pad0: 0,
                    _pad1: 0,
                },
                // While migrating, treat UNLIT as a simple toon material too.
                crate::engine::graphics::MaterialHandle::UNLIT_MESH => MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps,
                    emissive: 1,
                    _pad0: 0,
                    _pad1: 0,
                },
                crate::engine::graphics::MaterialHandle::MIRROR => MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps: 1.0,
                    emissive: 1,
                    _pad0: 0,
                    _pad1: 0,
                },
                _ => MaterialUBO::default(),
            }
        }

        pub fn new(
            window: Arc<Window>,
            xr_required: Option<(&[String], &[String])>,
            msaa_mode: MsaaMode,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            // Prefer the helper context while we're migrating: it enables surface extensions
            // and sets up graphics/compute queues and allocators.
            let context = {
                let mut config = VulkanoConfig::default();

                // SteamVR's OpenXR Vulkan requirements commonly report a max API of 1.2.0.
                // Some runtimes appear to validate the VkInstance API version against this.
                config.instance_create_info.max_api_version = Some(Version::V1_2);

                // Dynamic rendering: required so we can record the same draw-batch code against
                // non-swapchain targets (e.g. OpenXR swapchain images) without per-target
                // RenderPass/Framebuffer objects.
                //
                // On Vulkan 1.2 this is provided via VK_KHR_dynamic_rendering.
                config.device_extensions.khr_dynamic_rendering = true;
                config.device_features.dynamic_rendering = true;

                if let Some((instance_exts, device_exts)) = xr_required {
                    let mut enabled_instance_exts = config.instance_create_info.enabled_extensions;
                    let mut enabled_device_exts = config.device_extensions;

                    let mut unknown_instance_exts: Vec<&str> = Vec::new();
                    for name in instance_exts {
                        let ok = match name.as_str() {
                            "VK_KHR_get_physical_device_properties2" => {
                                enabled_instance_exts.khr_get_physical_device_properties2 = true;
                                true
                            }
                            "VK_KHR_external_memory_capabilities" => {
                                enabled_instance_exts.khr_external_memory_capabilities = true;
                                true
                            }
                            "VK_KHR_external_fence_capabilities" => {
                                enabled_instance_exts.khr_external_fence_capabilities = true;
                                true
                            }
                            "VK_KHR_external_semaphore_capabilities" => {
                                enabled_instance_exts.khr_external_semaphore_capabilities = true;
                                true
                            }
                            "VK_KHR_surface" => {
                                // Needed by winit surface creation anyway, but we mark it explicitly.
                                enabled_instance_exts.khr_surface = true;
                                true
                            }
                            _ => false,
                        };
                        if !ok {
                            unknown_instance_exts.push(name);
                        }
                    }

                    let mut unknown_device_exts: Vec<&str> = Vec::new();
                    for name in device_exts {
                        let ok = match name.as_str() {
                            "VK_KHR_external_memory" => {
                                enabled_device_exts.khr_external_memory = true;
                                true
                            }
                            "VK_KHR_external_memory_fd" => {
                                enabled_device_exts.khr_external_memory_fd = true;
                                true
                            }
                            "VK_KHR_external_fence" => {
                                enabled_device_exts.khr_external_fence = true;
                                true
                            }
                            "VK_KHR_external_fence_fd" => {
                                enabled_device_exts.khr_external_fence_fd = true;
                                true
                            }
                            "VK_KHR_external_semaphore" => {
                                enabled_device_exts.khr_external_semaphore = true;
                                true
                            }
                            "VK_KHR_external_semaphore_fd" => {
                                enabled_device_exts.khr_external_semaphore_fd = true;
                                true
                            }
                            "VK_KHR_get_memory_requirements2" => {
                                enabled_device_exts.khr_get_memory_requirements2 = true;
                                true
                            }
                            "VK_KHR_dedicated_allocation" => {
                                enabled_device_exts.khr_dedicated_allocation = true;
                                true
                            }
                            "VK_KHR_bind_memory2" => {
                                enabled_device_exts.khr_bind_memory2 = true;
                                true
                            }
                            "VK_KHR_timeline_semaphore" => {
                                enabled_device_exts.khr_timeline_semaphore = true;
                                true
                            }
                            "VK_KHR_image_format_list" => {
                                enabled_device_exts.khr_image_format_list = true;
                                true
                            }
                            _ => false,
                        };
                        if !ok {
                            unknown_device_exts.push(name);
                        }
                    }

                    config.instance_create_info.enabled_extensions = enabled_instance_exts;
                    config.device_extensions = enabled_device_exts;

                    // Keep the device selection filter in sync with the extensions we require.
                    let required_dev_exts: DeviceExtensions = enabled_device_exts;
                    config.device_filter_fn =
                        Arc::new(move |p| p.supported_extensions().contains(&required_dev_exts));

                    if !unknown_instance_exts.is_empty() || !unknown_device_exts.is_empty() {
                        // These might still be satisfied by Vulkan API version or be irrelevant to Vulkano;
                        // we log them so we can extend the mapping as needed.
                        eprintln!(
                            "[VulkanoRenderer] Note: some OpenXR-required Vulkan extensions were not mapped: instance={:?} device={:?}",
                            unknown_instance_exts, unknown_device_exts
                        );
                    }
                }

                VulkanoContext::new(config)
            };
            let device = context.device().clone();

            // Global toggle: either 4x MSAA (if supported) or no multisampling.
            let msaa_samples = match msaa_mode {
                MsaaMode::Off => SampleCount::Sample1,
                MsaaMode::Msaa4x => {
                    let props = device.physical_device().properties();
                    let counts = props.framebuffer_color_sample_counts
                        & props.framebuffer_depth_sample_counts;
                    if counts.intersects(SampleCounts::SAMPLE_4) {
                        SampleCount::Sample4
                    } else {
                        SampleCount::Sample1
                    }
                }
            };

            match msaa_samples {
                SampleCount::Sample4 => println!("[VulkanoRenderer] MSAA enabled: 4x"),
                _ => println!("[VulkanoRenderer] MSAA disabled"),
            }

            let swapchain_state =
                VulkanoSwapchainState::new(&context, window.clone(), msaa_samples)?;
            let framebuffer_count = swapchain_state.swapchain_views.len();

            let set_layouts = PipelineDescriptorSetLayouts::new(device.clone())?;

            let vs = toon_mesh_vs::load(device.clone())?;
            let mirror_vs = mirror_mesh_vs::load(device.clone())?;
            let fs = toon_mesh_fs::load(device.clone())?;
            let emissive_fs = emissive_toon_mesh_fs::load(device.clone())?;
            let mirror_fs = mirror_mesh_fs::load(device.clone())?;
            let grid_vs = grid_mesh_vs::load(device.clone())?;
            let grid_fs = grid_square_mesh_fs::load(device.clone())?;

            let skinned_vs = skinned_toon_mesh_vs::load(device.clone())?;

            let stages = vec![
                PipelineShaderStageCreateInfo::new(
                    vs.entry_point("main")
                        .ok_or("missing toon-mesh.vert entry point")?,
                ),
                PipelineShaderStageCreateInfo::new(
                    fs.entry_point("main")
                        .ok_or("missing toon-mesh.frag entry point")?,
                ),
            ];

            let grid_stages = vec![
                PipelineShaderStageCreateInfo::new(
                    grid_vs
                        .entry_point("main")
                        .ok_or("missing grid.vert entry point")?,
                ),
                PipelineShaderStageCreateInfo::new(
                    grid_fs
                        .entry_point("main")
                        .ok_or("missing grid-square.frag entry point")?,
                ),
            ];

            let skinned_stages = vec![
                PipelineShaderStageCreateInfo::new(
                    skinned_vs
                        .entry_point("main")
                        .ok_or("missing skinned-toon-mesh.vert entry point")?,
                ),
                PipelineShaderStageCreateInfo::new(
                    fs.entry_point("main")
                        .ok_or("missing toon-mesh.frag entry point")?,
                ),
            ];

            let emissive_stages = vec![
                PipelineShaderStageCreateInfo::new(
                    vs.entry_point("main")
                        .ok_or("missing toon-mesh.vert entry point")?,
                ),
                PipelineShaderStageCreateInfo::new(
                    emissive_fs
                        .entry_point("main")
                        .ok_or("missing emissive-toon-mesh.frag entry point")?,
                ),
            ];

            let mirror_stages = vec![
                PipelineShaderStageCreateInfo::new(
                    mirror_vs
                        .entry_point("main")
                        .ok_or("missing mirror-mesh.vert entry point")?,
                ),
                PipelineShaderStageCreateInfo::new(
                    mirror_fs
                        .entry_point("main")
                        .ok_or("missing mirror-mesh.frag entry point")?,
                ),
            ];

            let skinned_emissive_stages = vec![
                PipelineShaderStageCreateInfo::new(
                    skinned_vs
                        .entry_point("main")
                        .ok_or("missing skinned-toon-mesh.vert entry point")?,
                ),
                PipelineShaderStageCreateInfo::new(
                    emissive_fs
                        .entry_point("main")
                        .ok_or("missing emissive-toon-mesh.frag entry point")?,
                ),
            ];

            let layout = PipelineLayout::new(
                device.clone(),
                PipelineLayoutCreateInfo {
                    set_layouts: vec![
                        set_layouts.global.clone(),
                        set_layouts.material.clone(),
                        set_layouts.rig.clone(),
                    ],
                    ..Default::default()
                },
            )?;

            // Important: `CpuVertex` contains more than just position (e.g. UV).
            // We explicitly declare which attributes are consumed by the shader.
            // Instance data occupies locations 1-4 (+ per-instance color/emissive).
            let vertex_input_state_static = VertexInputState::new()
                .binding(
                    0,
                    VertexInputBindingDescription {
                        stride: size_of::<CpuVertex>() as u32,
                        input_rate: VertexInputRate::Vertex,
                        ..Default::default()
                    },
                )
                .binding(
                    1,
                    VertexInputBindingDescription {
                        stride: size_of::<InstanceData>() as u32,
                        input_rate: VertexInputRate::Instance { divisor: 1 },
                        ..Default::default()
                    },
                )
                .attribute(
                    0,
                    VertexInputAttributeDescription {
                        binding: 0,
                        format: Format::R32G32B32_SFLOAT,
                        offset: 0,
                        ..Default::default()
                    },
                )
                .attribute(
                    5,
                    VertexInputAttributeDescription {
                        binding: 0,
                        format: Format::R32G32_SFLOAT,
                        offset: 12,
                        ..Default::default()
                    },
                )
                .attribute(
                    8,
                    VertexInputAttributeDescription {
                        binding: 0,
                        format: Format::R32G32B32_SFLOAT,
                        offset: 20,
                        ..Default::default()
                    },
                )
                .attribute(
                    1,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 0,
                        ..Default::default()
                    },
                )
                .attribute(
                    2,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 16,
                        ..Default::default()
                    },
                )
                .attribute(
                    3,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 32,
                        ..Default::default()
                    },
                )
                .attribute(
                    4,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 48,
                        ..Default::default()
                    },
                )
                .attribute(
                    6,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 64,
                        ..Default::default()
                    },
                )
                .attribute(
                    7,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32_SFLOAT,
                        offset: 80,
                        ..Default::default()
                    },
                )
                .attribute(
                    9,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32_SFLOAT,
                        offset: 84,
                        ..Default::default()
                    },
                )
                .attribute(
                    10,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32_UINT,
                        offset: 88,
                        ..Default::default()
                    },
                )
                .attribute(
                    11,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32_UINT,
                        offset: 92,
                        ..Default::default()
                    },
                );

            // Skinned pipeline: add a separate per-vertex skinning buffer (binding=1),
            // and move per-instance data to binding=2.
            let vertex_input_state_skinned = VertexInputState::new()
                .binding(
                    0,
                    VertexInputBindingDescription {
                        stride: size_of::<CpuVertex>() as u32,
                        input_rate: VertexInputRate::Vertex,
                        ..Default::default()
                    },
                )
                .binding(
                    1,
                    VertexInputBindingDescription {
                        stride: size_of::<GpuSkinVertex>() as u32,
                        input_rate: VertexInputRate::Vertex,
                        ..Default::default()
                    },
                )
                .binding(
                    2,
                    VertexInputBindingDescription {
                        stride: size_of::<InstanceData>() as u32,
                        input_rate: VertexInputRate::Instance { divisor: 1 },
                        ..Default::default()
                    },
                )
                // Base vertex attributes.
                .attribute(
                    0,
                    VertexInputAttributeDescription {
                        binding: 0,
                        format: Format::R32G32B32_SFLOAT,
                        offset: 0,
                        ..Default::default()
                    },
                )
                .attribute(
                    5,
                    VertexInputAttributeDescription {
                        binding: 0,
                        format: Format::R32G32_SFLOAT,
                        offset: 12,
                        ..Default::default()
                    },
                )
                .attribute(
                    8,
                    VertexInputAttributeDescription {
                        binding: 0,
                        format: Format::R32G32B32_SFLOAT,
                        offset: 20,
                        ..Default::default()
                    },
                )
                // Skinning attributes.
                .attribute(
                    12,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R16G16B16A16_UINT,
                        offset: 0,
                        ..Default::default()
                    },
                )
                .attribute(
                    13,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 8,
                        ..Default::default()
                    },
                )
                // Per-instance attributes (binding=2).
                .attribute(
                    1,
                    VertexInputAttributeDescription {
                        binding: 2,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 0,
                        ..Default::default()
                    },
                )
                .attribute(
                    2,
                    VertexInputAttributeDescription {
                        binding: 2,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 16,
                        ..Default::default()
                    },
                )
                .attribute(
                    3,
                    VertexInputAttributeDescription {
                        binding: 2,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 32,
                        ..Default::default()
                    },
                )
                .attribute(
                    4,
                    VertexInputAttributeDescription {
                        binding: 2,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 48,
                        ..Default::default()
                    },
                )
                .attribute(
                    6,
                    VertexInputAttributeDescription {
                        binding: 2,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 64,
                        ..Default::default()
                    },
                )
                .attribute(
                    7,
                    VertexInputAttributeDescription {
                        binding: 2,
                        format: Format::R32_SFLOAT,
                        offset: 80,
                        ..Default::default()
                    },
                )
                .attribute(
                    9,
                    VertexInputAttributeDescription {
                        binding: 2,
                        format: Format::R32_SFLOAT,
                        offset: 84,
                        ..Default::default()
                    },
                )
                .attribute(
                    10,
                    VertexInputAttributeDescription {
                        binding: 2,
                        format: Format::R32_UINT,
                        offset: 88,
                        ..Default::default()
                    },
                )
                .attribute(
                    11,
                    VertexInputAttributeDescription {
                        binding: 2,
                        format: Format::R32_UINT,
                        offset: 92,
                        ..Default::default()
                    },
                );

            let color_format = swapchain_state.swapchain.image_format();
            let mut pipeline_ci =
                vulkano::pipeline::graphics::GraphicsPipelineCreateInfo::layout(layout);
            pipeline_ci.stages = stages.into();
            pipeline_ci.vertex_input_state = Some(vertex_input_state_static);
            pipeline_ci.input_assembly_state = Some(InputAssemblyState::default());
            pipeline_ci.viewport_state = Some(ViewportState::default());
            pipeline_ci.rasterization_state = Some(RasterizationState::default());
            pipeline_ci.multisample_state = Some(MultisampleState {
                rasterization_samples: msaa_samples,
                ..Default::default()
            });
            // Enable depth testing so 3D geometry occludes correctly.
            pipeline_ci.depth_stencil_state = Some(DepthStencilState {
                depth: Some(DepthState::simple()),
                ..Default::default()
            });
            // Enable alpha blending so textures with transparency (e.g. PNG alpha) render correctly.
            // Uses straight alpha: out.rgb = src.rgb * src.a + dst.rgb * (1-src.a)
            pipeline_ci.color_blend_state = Some(ColorBlendState::with_attachment_states(
                1,
                ColorBlendAttachmentState {
                    blend: Some(AttachmentBlend {
                        src_color_blend_factor: BlendFactor::SrcAlpha,
                        dst_color_blend_factor: BlendFactor::OneMinusSrcAlpha,
                        color_blend_op: BlendOp::Add,
                        src_alpha_blend_factor: BlendFactor::One,
                        dst_alpha_blend_factor: BlendFactor::OneMinusSrcAlpha,
                        alpha_blend_op: BlendOp::Add,
                    }),
                    color_write_enable: true,
                    color_write_mask: ColorComponents::all(),
                },
            ));
            pipeline_ci.dynamic_state = [DynamicState::Viewport, DynamicState::Scissor]
                .into_iter()
                .collect();
            // Dynamic rendering so we can reuse the same draw code for non-swapchain targets (OpenXR).
            // The pipeline is keyed by attachment formats rather than a specific RenderPass.
            let mut pipeline_rendering = PipelineRenderingCreateInfo::default();
            pipeline_rendering.color_attachment_formats = vec![Some(color_format)];
            pipeline_rendering.depth_attachment_format = Some(VulkanoSwapchainState::DEPTH_FORMAT);
            pipeline_rendering.stencil_attachment_format =
                Some(VulkanoSwapchainState::DEPTH_FORMAT);

            pipeline_ci.subpass = Some(PipelineSubpassType::BeginRendering(pipeline_rendering));

            let pipeline_toon_mesh =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci.clone())?;
            let mut pipeline_ci_mirror = pipeline_ci.clone();
            pipeline_ci_mirror.stages = mirror_stages.clone().into();
            let pipeline_mirror_mesh =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_mirror.clone())?;
            let mut pipeline_ci_grid = pipeline_ci.clone();
            pipeline_ci_grid.stages = grid_stages.clone().into();
            let pipeline_grid_mesh =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_grid.clone())?;

            let mut pipeline_ci_emissive = pipeline_ci.clone();
            pipeline_ci_emissive.stages = emissive_stages.clone().into();
            let pipeline_emissive_toon_mesh =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_emissive.clone())?;

            let mut pipeline_ci_emissive_prepass = pipeline_ci_emissive.clone();
            pipeline_ci_emissive_prepass.depth_stencil_state = Some(DepthStencilState {
                depth: Some(DepthState {
                    write_enable: false,
                    compare_op: CompareOp::LessOrEqual,
                    ..DepthState::simple()
                }),
                ..Default::default()
            });
            let pipeline_emissive_prepass_toon_mesh =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_emissive_prepass)?;

            // Transparent variant: depth test ON, depth write OFF.
            let mut pipeline_ci_transparent = pipeline_ci.clone();
            pipeline_ci_transparent.depth_stencil_state = Some(DepthStencilState {
                depth: Some(DepthState {
                    write_enable: false,
                    ..DepthState::simple()
                }),
                ..Default::default()
            });
            let pipeline_toon_mesh_transparent =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_transparent.clone())?;
            let mut pipeline_ci_mirror_transparent = pipeline_ci_transparent.clone();
            pipeline_ci_mirror_transparent.stages = mirror_stages.clone().into();
            let pipeline_mirror_mesh_transparent =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_mirror_transparent)?;
            let mut pipeline_ci_grid_transparent = pipeline_ci_transparent.clone();
            pipeline_ci_grid_transparent.stages = grid_stages.clone().into();
            let pipeline_grid_mesh_transparent =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_grid_transparent.clone())?;

            let mut pipeline_ci_emissive_transparent = pipeline_ci_transparent.clone();
            pipeline_ci_emissive_transparent.stages = emissive_stages.clone().into();
            let pipeline_emissive_toon_mesh_transparent = GraphicsPipeline::new(
                device.clone(),
                None,
                pipeline_ci_emissive_transparent.clone(),
            )?;

            // Transparent cutout variant:
            // - depth test/write ON
            // - alpha-to-coverage enabled (requires MSAA)
            // - blending disabled (coverage handles edges)
            let mut pipeline_ci_cutout = pipeline_ci.clone();
            pipeline_ci_cutout.multisample_state = Some(MultisampleState {
                rasterization_samples: msaa_samples,
                alpha_to_coverage_enable: msaa_samples != SampleCount::Sample1,
                ..Default::default()
            });
            pipeline_ci_cutout.color_blend_state = Some(ColorBlendState::with_attachment_states(
                1,
                ColorBlendAttachmentState {
                    blend: None,
                    color_write_enable: true,
                    color_write_mask: ColorComponents::all(),
                },
            ));
            let pipeline_toon_mesh_cutout =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_cutout.clone())?;
            let mut pipeline_ci_mirror_cutout = pipeline_ci_cutout.clone();
            pipeline_ci_mirror_cutout.stages = mirror_stages.clone().into();
            let pipeline_mirror_mesh_cutout =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_mirror_cutout)?;

            let mut pipeline_ci_emissive_cutout = pipeline_ci_cutout.clone();
            pipeline_ci_emissive_cutout.stages = emissive_stages.clone().into();
            let pipeline_emissive_toon_mesh_cutout =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_emissive_cutout.clone())?;

            let mut pipeline_ci_emissive_prepass_cutout = pipeline_ci_emissive_cutout.clone();
            pipeline_ci_emissive_prepass_cutout.depth_stencil_state = Some(DepthStencilState {
                depth: Some(DepthState {
                    write_enable: false,
                    compare_op: CompareOp::LessOrEqual,
                    ..DepthState::simple()
                }),
                ..Default::default()
            });
            let pipeline_emissive_prepass_toon_mesh_cutout =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_emissive_prepass_cutout)?;

            // Skinned variants: same state, different vertex shader.
            let mut pipeline_ci_skinned = pipeline_ci.clone();
            pipeline_ci_skinned.stages = skinned_stages.clone().into();
            pipeline_ci_skinned.vertex_input_state = Some(vertex_input_state_skinned.clone());
            let pipeline_skinned_toon_mesh =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_skinned.clone())?;

            let mut pipeline_ci_skinned_emissive = pipeline_ci.clone();
            pipeline_ci_skinned_emissive.stages = skinned_emissive_stages.clone().into();
            pipeline_ci_skinned_emissive.vertex_input_state =
                Some(vertex_input_state_skinned.clone());
            let pipeline_skinned_emissive_toon_mesh =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_skinned_emissive.clone())?;

            let mut pipeline_ci_skinned_emissive_prepass = pipeline_ci_skinned_emissive.clone();
            pipeline_ci_skinned_emissive_prepass.depth_stencil_state = Some(DepthStencilState {
                depth: Some(DepthState {
                    write_enable: false,
                    compare_op: CompareOp::LessOrEqual,
                    ..DepthState::simple()
                }),
                ..Default::default()
            });
            let pipeline_skinned_emissive_prepass_toon_mesh =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_skinned_emissive_prepass)?;

            let mut pipeline_ci_skinned_transparent = pipeline_ci_transparent.clone();
            pipeline_ci_skinned_transparent.stages = skinned_stages.clone().into();
            pipeline_ci_skinned_transparent.vertex_input_state =
                Some(vertex_input_state_skinned.clone());
            let pipeline_skinned_toon_mesh_transparent =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_skinned_transparent)?;

            let mut pipeline_ci_skinned_emissive_transparent = pipeline_ci_transparent.clone();
            pipeline_ci_skinned_emissive_transparent.stages =
                skinned_emissive_stages.clone().into();
            pipeline_ci_skinned_emissive_transparent.vertex_input_state =
                Some(vertex_input_state_skinned.clone());
            let pipeline_skinned_emissive_toon_mesh_transparent = GraphicsPipeline::new(
                device.clone(),
                None,
                pipeline_ci_skinned_emissive_transparent,
            )?;

            let mut pipeline_ci_skinned_cutout = pipeline_ci_cutout.clone();
            pipeline_ci_skinned_cutout.stages = skinned_stages.into();
            pipeline_ci_skinned_cutout.vertex_input_state =
                Some(vertex_input_state_skinned.clone());
            let pipeline_skinned_toon_mesh_cutout =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_skinned_cutout)?;

            let mut pipeline_ci_skinned_emissive_cutout = pipeline_ci_cutout.clone();
            pipeline_ci_skinned_emissive_cutout.stages = skinned_emissive_stages.clone().into();
            pipeline_ci_skinned_emissive_cutout.vertex_input_state =
                Some(vertex_input_state_skinned.clone());
            let pipeline_skinned_emissive_toon_mesh_cutout = GraphicsPipeline::new(
                device.clone(),
                None,
                pipeline_ci_skinned_emissive_cutout.clone(),
            )?;

            let mut pipeline_ci_skinned_emissive_prepass_cutout =
                pipeline_ci_skinned_emissive_cutout.clone();
            pipeline_ci_skinned_emissive_prepass_cutout.depth_stencil_state =
                Some(DepthStencilState {
                    depth: Some(DepthState {
                        write_enable: false,
                        compare_op: CompareOp::LessOrEqual,
                        ..DepthState::simple()
                    }),
                    ..Default::default()
                });
            let pipeline_skinned_emissive_prepass_toon_mesh_cutout = GraphicsPipeline::new(
                device.clone(),
                None,
                pipeline_ci_skinned_emissive_prepass_cutout,
            )?;

            // Stencil clip pipelines — color write off, no depth test.
            // Shared helper: stencil ops for enter/exit clip.
            let stencil_ops_incr = StencilOps {
                compare_op: CompareOp::Equal,
                pass_op: StencilOp::IncrementAndClamp,
                fail_op: StencilOp::Keep,
                depth_fail_op: StencilOp::Keep,
            };
            let stencil_ops_decr = StencilOps {
                compare_op: CompareOp::Equal,
                pass_op: StencilOp::DecrementAndClamp,
                fail_op: StencilOp::Keep,
                depth_fail_op: StencilOp::Keep,
            };
            let stencil_ops_test = StencilOps {
                compare_op: CompareOp::Equal,
                pass_op: StencilOp::Keep,
                fail_op: StencilOp::Keep,
                depth_fail_op: StencilOp::Keep,
            };
            let stencil_write_color_blend = ColorBlendState::with_attachment_states(
                1,
                ColorBlendAttachmentState {
                    blend: None,
                    color_write_enable: true,
                    color_write_mask: ColorComponents::empty(),
                },
            );
            let mut stencil_dynamic_state = pipeline_ci.dynamic_state.clone();
            stencil_dynamic_state.insert(DynamicState::StencilReference);

            let mut pipeline_ci_stencil_incr = pipeline_ci.clone();
            pipeline_ci_stencil_incr.color_blend_state = Some(stencil_write_color_blend.clone());
            pipeline_ci_stencil_incr.depth_stencil_state = Some(DepthStencilState {
                depth: None,
                stencil: Some(StencilState {
                    front: StencilOpState {
                        ops: stencil_ops_incr,
                        ..Default::default()
                    },
                    back: StencilOpState {
                        ops: stencil_ops_incr,
                        ..Default::default()
                    },
                }),
                ..Default::default()
            });
            pipeline_ci_stencil_incr.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_stencil_incr =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_stencil_incr)?;

            let mut pipeline_ci_stencil_decr = pipeline_ci.clone();
            pipeline_ci_stencil_decr.color_blend_state = Some(stencil_write_color_blend);
            pipeline_ci_stencil_decr.depth_stencil_state = Some(DepthStencilState {
                depth: None,
                stencil: Some(StencilState {
                    front: StencilOpState {
                        ops: stencil_ops_decr,
                        ..Default::default()
                    },
                    back: StencilOpState {
                        ops: stencil_ops_decr,
                        ..Default::default()
                    },
                }),
                ..Default::default()
            });
            pipeline_ci_stencil_decr.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_stencil_decr =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_stencil_decr)?;

            // Overlay variants that require stencil == reference to pass.
            let clipped_depth_stencil = DepthStencilState {
                depth: Some(DepthState::simple()),
                stencil: Some(StencilState {
                    front: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                    back: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                }),
                ..Default::default()
            };

            let mut pipeline_ci_overlay_clipped = pipeline_ci.clone();
            pipeline_ci_overlay_clipped.depth_stencil_state = Some(clipped_depth_stencil.clone());
            pipeline_ci_overlay_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_overlay_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_overlay_clipped)?;

            let mut pipeline_ci_emissive_overlay_clipped = pipeline_ci_emissive.clone();
            pipeline_ci_emissive_overlay_clipped.depth_stencil_state =
                Some(clipped_depth_stencil.clone());
            pipeline_ci_emissive_overlay_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_emissive_overlay_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_emissive_overlay_clipped)?;

            // Opaque clipped: identical depth_stencil (depth write ON + stencil EQUAL),
            // based on the opaque pipeline_ci so blend state matches opaque phase.
            let mut pipeline_ci_opaque_clipped = pipeline_ci.clone();
            pipeline_ci_opaque_clipped.depth_stencil_state = Some(clipped_depth_stencil.clone());
            pipeline_ci_opaque_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_opaque_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_opaque_clipped)?;

            let mut pipeline_ci_mirror_clipped = pipeline_ci_mirror.clone();
            pipeline_ci_mirror_clipped.depth_stencil_state = Some(clipped_depth_stencil.clone());
            pipeline_ci_mirror_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_mirror_mesh_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_mirror_clipped)?;

            let mut pipeline_ci_emissive_opaque_clipped = pipeline_ci_emissive.clone();
            pipeline_ci_emissive_opaque_clipped.depth_stencil_state = Some(clipped_depth_stencil);
            pipeline_ci_emissive_opaque_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_emissive_opaque_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_emissive_opaque_clipped)?;

            let transparent_clipped_depth_stencil = DepthStencilState {
                depth: Some(DepthState {
                    write_enable: false,
                    ..DepthState::simple()
                }),
                stencil: Some(StencilState {
                    front: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                    back: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                }),
                ..Default::default()
            };

            let mut pipeline_ci_transparent_clipped = pipeline_ci_transparent.clone();
            pipeline_ci_transparent_clipped.depth_stencil_state =
                Some(transparent_clipped_depth_stencil.clone());
            pipeline_ci_transparent_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_toon_mesh_transparent_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_transparent_clipped)?;
            let mut pipeline_ci_mirror_transparent_clipped = pipeline_ci_transparent.clone();
            pipeline_ci_mirror_transparent_clipped.stages = mirror_stages.clone().into();
            pipeline_ci_mirror_transparent_clipped.depth_stencil_state =
                Some(transparent_clipped_depth_stencil.clone());
            pipeline_ci_mirror_transparent_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_mirror_mesh_transparent_clipped = GraphicsPipeline::new(
                device.clone(),
                None,
                pipeline_ci_mirror_transparent_clipped,
            )?;
            let mut pipeline_ci_grid_transparent_clipped = pipeline_ci_grid_transparent.clone();
            pipeline_ci_grid_transparent_clipped.depth_stencil_state =
                Some(transparent_clipped_depth_stencil.clone());
            pipeline_ci_grid_transparent_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_grid_mesh_transparent_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_grid_transparent_clipped)?;

            let mut pipeline_ci_emissive_transparent_clipped =
                pipeline_ci_emissive_transparent.clone();
            pipeline_ci_emissive_transparent_clipped.depth_stencil_state =
                Some(transparent_clipped_depth_stencil);
            pipeline_ci_emissive_transparent_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_emissive_toon_mesh_transparent_clipped = GraphicsPipeline::new(
                device.clone(),
                None,
                pipeline_ci_emissive_transparent_clipped,
            )?;

            let mut pipeline_ci_cutout_clipped = pipeline_ci_cutout.clone();
            pipeline_ci_cutout_clipped.depth_stencil_state = Some(DepthStencilState {
                depth: Some(DepthState::simple()),
                stencil: Some(StencilState {
                    front: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                    back: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                }),
                ..Default::default()
            });
            pipeline_ci_cutout_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_toon_mesh_cutout_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_cutout_clipped)?;
            let mut pipeline_ci_mirror_cutout_clipped = pipeline_ci_cutout.clone();
            pipeline_ci_mirror_cutout_clipped.stages = mirror_stages.into();
            pipeline_ci_mirror_cutout_clipped.depth_stencil_state = Some(DepthStencilState {
                depth: Some(DepthState::simple()),
                stencil: Some(StencilState {
                    front: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                    back: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                }),
                ..Default::default()
            });
            pipeline_ci_mirror_cutout_clipped.dynamic_state = stencil_dynamic_state.clone();
            let pipeline_mirror_mesh_cutout_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_mirror_cutout_clipped)?;

            let mut pipeline_ci_emissive_cutout_clipped = pipeline_ci_emissive_cutout.clone();
            pipeline_ci_emissive_cutout_clipped.depth_stencil_state = Some(DepthStencilState {
                depth: Some(DepthState::simple()),
                stencil: Some(StencilState {
                    front: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                    back: StencilOpState {
                        ops: stencil_ops_test,
                        ..Default::default()
                    },
                }),
                ..Default::default()
            });
            pipeline_ci_emissive_cutout_clipped.dynamic_state = stencil_dynamic_state;
            let pipeline_emissive_toon_mesh_cutout_clipped =
                GraphicsPipeline::new(device.clone(), None, pipeline_ci_emissive_cutout_clipped)?;

            let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
                device.clone(),
                Default::default(),
            ));

            let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
                device.clone(),
                Default::default(),
            ));

            let post_processing_renderer = PostProcessingRenderer::new(
                device.clone(),
                context.memory_allocator().clone(),
                descriptor_set_allocator.clone(),
            )?;

            let sampler_linear =
                Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear())?;

            let sampler_nearest = Sampler::new(
                device.clone(),
                SamplerCreateInfo {
                    mag_filter: Filter::Nearest,
                    min_filter: Filter::Nearest,
                    mipmap_mode: SamplerMipmapMode::Nearest,
                    address_mode: [SamplerAddressMode::Repeat; 3],
                    ..Default::default()
                },
            )?;

            let sampler_nearest_mag = Sampler::new(
                device.clone(),
                SamplerCreateInfo {
                    mag_filter: Filter::Nearest,
                    min_filter: Filter::Linear,
                    mipmap_mode: SamplerMipmapMode::Nearest,
                    address_mode: [SamplerAddressMode::Repeat; 3],
                    ..Default::default()
                },
            )?;

            let mut state = Self {
                context,
                window,

                swapchain_state,

                command_buffer_allocator,
                descriptor_set_allocator,
                post_processing_renderer,
                meshes: HashMap::new(),

                textures: HashMap::new(),
                sampler_linear,
                sampler_nearest,
                sampler_nearest_mag,
                default_white_texture: TextureHandle(0),

                set_layouts,

                pipeline_toon_mesh,
                pipeline_toon_mesh_transparent,
                pipeline_toon_mesh_cutout,
                pipeline_toon_mesh_transparent_clipped,
                pipeline_toon_mesh_cutout_clipped,
                pipeline_mirror_mesh,
                pipeline_mirror_mesh_transparent,
                pipeline_mirror_mesh_cutout,
                pipeline_mirror_mesh_clipped,
                pipeline_mirror_mesh_transparent_clipped,
                pipeline_mirror_mesh_cutout_clipped,

                pipeline_grid_mesh,
                pipeline_grid_mesh_transparent,
                pipeline_grid_mesh_transparent_clipped,

                pipeline_emissive_toon_mesh,
                pipeline_emissive_toon_mesh_transparent,
                pipeline_emissive_toon_mesh_cutout,
                pipeline_emissive_toon_mesh_transparent_clipped,
                pipeline_emissive_toon_mesh_cutout_clipped,
                pipeline_emissive_prepass_toon_mesh,
                pipeline_emissive_prepass_toon_mesh_cutout,

                pipeline_skinned_toon_mesh,
                pipeline_skinned_toon_mesh_transparent,
                pipeline_skinned_toon_mesh_cutout,

                pipeline_skinned_emissive_toon_mesh,
                pipeline_skinned_emissive_toon_mesh_transparent,
                pipeline_skinned_emissive_toon_mesh_cutout,
                pipeline_skinned_emissive_prepass_toon_mesh,
                pipeline_skinned_emissive_prepass_toon_mesh_cutout,

                pipeline_stencil_incr,
                pipeline_stencil_decr,
                pipeline_overlay_clipped,
                pipeline_emissive_overlay_clipped,
                pipeline_opaque_clipped,
                pipeline_emissive_opaque_clipped,

                msaa_samples,

                cached_instance_buffer: None,
                cached_instance_count: 0,

                cached_background_instance_buffer: None,
                cached_background_instance_count: 0,

                cached_background_occluded_lit_instance_buffer: None,
                cached_background_occluded_lit_instance_count: 0,

                cached_cutout_instance_buffer: None,
                cached_cutout_instance_count: 0,

                cached_overlay_instance_buffer: None,
                cached_overlay_instance_count: 0,
                cached_material_sets: HashMap::new(),
                pending_runtime_texture_updates: HashMap::new(),
                window_runtime_debug_targets: None,

                cached_bones_buffers: Vec::new(),
                cached_bones_slot_valid: Vec::new(),
                cached_bones_capacity: 0,

                xr_offscreen: None,
                mirror_offscreen: HashMap::new(),

                window_resized: false,
                recreate_swapchain: false,
                images_in_flight: (0..framebuffer_count).map(|_| None).collect(),
            };

            // Default texture: 1x1 white so untextured materials can still bind a sampler.
            state.upload_texture_rgba8(TextureHandle(0), &[255, 255, 255, 255], 1, 1)?;

            Ok(state)
        }

        pub fn window_color_format(&self) -> Format {
            self.swapchain_state.swapchain.image_format()
        }

        fn ensure_xr_offscreen_targets(
            &mut self,
            view_count: usize,
            extent: [u32; 2],
        ) -> Result<(), Box<dyn std::error::Error>> {
            let color_format = self.swapchain_state.swapchain.image_format();

            let needs_recreate = self.xr_offscreen.as_ref().is_none_or(|t| {
                t.extent != extent
                    || t.color_format != color_format
                    || t.color_views.len() != view_count
                    || (self.msaa_samples == SampleCount::Sample1) != t.msaa_color_views.is_empty()
                    || (self.msaa_samples != SampleCount::Sample1
                        && t.msaa_color_views.len() != view_count)
            });

            if !needs_recreate {
                return Ok(());
            }

            let memory_allocator = self.context.memory_allocator().clone();

            let mut color_images = Vec::with_capacity(view_count);
            let mut msaa_color_views = Vec::with_capacity(view_count);
            let mut color_views = Vec::with_capacity(view_count);
            let mut depth_views = Vec::with_capacity(view_count);

            for _ in 0..view_count {
                // Resolve target (single-sampled): used for transfer/copy out.
                let color_image = vulkano::image::Image::new(
                    memory_allocator.clone(),
                    vulkano::image::ImageCreateInfo {
                        image_type: vulkano::image::ImageType::Dim2d,
                        format: color_format,
                        extent: [extent[0], extent[1], 1],
                        samples: SampleCount::Sample1,
                        // OpenXR copies from these images into the XR swapchain target, so keep
                        // them usable for both transfer and sampling.
                        usage: vulkano::image::ImageUsage::COLOR_ATTACHMENT
                            | vulkano::image::ImageUsage::SAMPLED
                            | vulkano::image::ImageUsage::TRANSFER_SRC,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                        ..Default::default()
                    },
                )?;

                let color_view = ImageView::new_default(color_image.clone())
                    .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;

                // Multisampled color attachment (optional): resolved into `color_view`.
                if self.msaa_samples != SampleCount::Sample1 {
                    let msaa_color_image = vulkano::image::Image::new(
                        memory_allocator.clone(),
                        vulkano::image::ImageCreateInfo {
                            image_type: vulkano::image::ImageType::Dim2d,
                            format: color_format,
                            extent: [extent[0], extent[1], 1],
                            samples: self.msaa_samples,
                            usage: vulkano::image::ImageUsage::COLOR_ATTACHMENT
                                | vulkano::image::ImageUsage::TRANSIENT_ATTACHMENT,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                            ..Default::default()
                        },
                    )?;

                    let msaa_color_view = ImageView::new_default(msaa_color_image)
                        .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;
                    msaa_color_views.push(msaa_color_view);
                }

                let depth_image = vulkano::image::Image::new(
                    memory_allocator.clone(),
                    vulkano::image::ImageCreateInfo {
                        image_type: vulkano::image::ImageType::Dim2d,
                        format: VulkanoSwapchainState::DEPTH_FORMAT,
                        extent: [extent[0], extent[1], 1],
                        samples: self.msaa_samples,
                        usage: vulkano::image::ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                        ..Default::default()
                    },
                )?;

                let depth_view = ImageView::new(
                    depth_image.clone(),
                    ImageViewCreateInfo {
                        subresource_range: ImageSubresourceRange {
                            aspects: ImageAspects::DEPTH | ImageAspects::STENCIL,
                            ..depth_image.subresource_range()
                        },
                        ..ImageViewCreateInfo::from_image(&depth_image)
                    },
                )
                .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;

                color_images.push(color_image);
                color_views.push(color_view);
                depth_views.push(depth_view);
            }

            self.xr_offscreen = Some(XrOffscreenTargets {
                extent,
                color_format,
                color_images,
                msaa_color_views,
                color_views,
                depth_views,
            });

            Ok(())
        }

        pub fn ensure_mirror_offscreen_targets(
            &mut self,
            mirror_key: &str,
            view_count: usize,
            extent: [u32; 2],
            color_format: Format,
        ) -> Result<&MirrorOffscreenTargets, Box<dyn std::error::Error>> {
            let needs_recreate = self
                .mirror_offscreen
                .get(mirror_key)
                .map_or(true, |targets| {
                    targets.extent != extent
                        || targets.color_format != color_format
                        || targets.color_images.len() != view_count
                });

            if needs_recreate {
                let mut color_images = Vec::new();
                let mut msaa_color_views = Vec::new();
                let mut color_views = Vec::new();
                let mut depth_views = Vec::new();

                let memory_allocator = self.context.memory_allocator().clone();
                for _ in 0..view_count {
                    let color_image = Image::new(
                        memory_allocator.clone(),
                        ImageCreateInfo {
                            image_type: ImageType::Dim2d,
                            format: color_format,
                            extent: [extent[0], extent[1], 1],
                            samples: SampleCount::Sample1,
                            usage: ImageUsage::COLOR_ATTACHMENT
                                | ImageUsage::SAMPLED
                                | ImageUsage::TRANSFER_SRC,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                            ..Default::default()
                        },
                    )?;

                    let color_view = ImageView::new_default(color_image.clone())
                        .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;

                    if self.msaa_samples != SampleCount::Sample1 {
                        let msaa_color_image = Image::new(
                            memory_allocator.clone(),
                            ImageCreateInfo {
                                image_type: ImageType::Dim2d,
                                format: color_format,
                                extent: [extent[0], extent[1], 1],
                                samples: self.msaa_samples,
                                usage: ImageUsage::COLOR_ATTACHMENT
                                    | ImageUsage::TRANSIENT_ATTACHMENT,
                                ..Default::default()
                            },
                            AllocationCreateInfo {
                                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                                ..Default::default()
                            },
                        )?;

                        let msaa_color_view = ImageView::new_default(msaa_color_image).map_err(
                            |e| -> Box<dyn std::error::Error> { format!("{e:?}").into() },
                        )?;
                        msaa_color_views.push(msaa_color_view);
                    }

                    let depth_image = Image::new(
                        memory_allocator.clone(),
                        ImageCreateInfo {
                            image_type: ImageType::Dim2d,
                            format: VulkanoSwapchainState::DEPTH_FORMAT,
                            extent: [extent[0], extent[1], 1],
                            samples: self.msaa_samples,
                            usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                            ..Default::default()
                        },
                    )?;

                    let depth_view = ImageView::new(
                        depth_image.clone(),
                        ImageViewCreateInfo {
                            subresource_range: ImageSubresourceRange {
                                aspects: ImageAspects::DEPTH | ImageAspects::STENCIL,
                                ..depth_image.subresource_range()
                            },
                            ..ImageViewCreateInfo::from_image(&depth_image)
                        },
                    )
                    .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;

                    color_images.push(color_image);
                    color_views.push(color_view);
                    depth_views.push(depth_view);
                }

                self.mirror_offscreen.insert(
                    mirror_key.to_string(),
                    MirrorOffscreenTargets {
                        extent,
                        color_format,
                        color_images,
                        msaa_color_views,
                        color_views,
                        depth_views,
                    },
                );
            }

            Ok(self.mirror_offscreen.get(mirror_key).unwrap())
        }

        pub fn render_xr_eye_offscreen(
            &mut self,
            visual_world: &mut VisualWorld,
            eye: usize,
            extent: [u32; 2],
        ) -> Result<(), Box<dyn std::error::Error>> {
            if !self.pending_runtime_texture_updates.is_empty() {
                unsafe {
                    self.context.device().wait_idle().map_err(
                        |e| -> Box<dyn std::error::Error> {
                            format!("wait_idle failed before runtime texture swap: {e}").into()
                        },
                    )?;
                }
            }
            self.apply_pending_runtime_texture_updates();

            // MVP: assume PRIMARY_STEREO (2 eyes).
            let view_count = 2;
            self.ensure_xr_offscreen_targets(view_count, extent)?;
            let color_format = self
                .xr_offscreen
                .as_ref()
                .ok_or("XR offscreen targets missing")?
                .color_format;

            let post_process_config = visual_world.post_processing().clone();
            let post_process_active = post_process_config.is_active();
            if post_process_active {
                self.post_processing_renderer.ensure_xr_targets(
                    view_count,
                    extent,
                    color_format,
                    self.msaa_samples,
                    &post_process_config,
                )?;
            }

            self.render_mirror_captures(visual_world, color_format)?;

            let Some(targets) = self.xr_offscreen.as_ref() else {
                return Err("XR offscreen targets missing".into());
            };

            let resolve_view = targets
                .color_views
                .get(eye)
                .ok_or("XR offscreen eye out of range")?
                .clone();

            let post_process = if post_process_active {
                let pp_targets = self
                    .post_processing_renderer
                    .xr_frame_targets(eye)
                    .ok_or("missing XR post-processing targets")?
                    .clone();
                Some(PostProcessInvocation {
                    final_output_view: resolve_view.clone(),
                    final_color_format: targets.color_format,
                    config: post_process_config,
                    targets: pp_targets,
                })
            } else {
                None
            };

            let (color_attachment_view, color_resolve_view, depth_view) =
                if let Some(post) = post_process.as_ref() {
                    (
                        post.targets
                            .main_msaa_color
                            .clone()
                            .unwrap_or_else(|| post.targets.main_color.clone()),
                        if post.targets.main_msaa_color.is_some() {
                            Some(post.targets.main_color.clone())
                        } else {
                            None
                        },
                        post.targets.depth.clone(),
                    )
                } else if self.msaa_samples != SampleCount::Sample1 {
                    let msaa_view = targets
                        .msaa_color_views
                        .get(eye)
                        .ok_or("XR MSAA color eye out of range")?
                        .clone();
                    let depth_view = targets
                        .depth_views
                        .get(eye)
                        .ok_or("XR depth eye out of range")?
                        .clone();
                    (msaa_view, Some(resolve_view.clone()), depth_view)
                } else {
                    let depth_view = targets
                        .depth_views
                        .get(eye)
                        .ok_or("XR depth eye out of range")?
                        .clone();
                    (resolve_view.clone(), None, depth_view)
                };

            let xr_camera = visual_world
                .visual_camera(crate::engine::graphics::CameraTarget::Xr)
                .ok_or("missing XR camera")?;
            let eye_data = xr_camera
                .eyes
                .get(eye)
                .ok_or("XR camera eye out of range")?;
            let render_view = RenderView {
                view: eye_data.view,
                proj: eye_data.proj,
                viewport: [extent[0] as f32, extent[1] as f32],
                kind: RenderViewKind::XrEye { eye },
            };

            let window_slots = self.swapchain_state.swapchain_views.len().max(1);
            let bones_slots_total = window_slots + view_count;
            let bones_slot = window_slots + eye;

            let cb = self.build_draw_batches_command_buffer(
                visual_world,
                &render_view,
                bones_slot,
                bones_slots_total,
                color_attachment_view,
                color_resolve_view,
                depth_view,
                extent,
                post_process,
                None,
            )?;

            let device = self.context.device().clone();
            let queue = self.context.graphics_queue().clone();

            sync::now(device)
                .then_execute(queue, cb)?
                .then_signal_fence_and_flush()?
                .wait(None)?;

            Ok(())
        }

        fn render_mirror_captures(
            &mut self,
            visual_world: &mut VisualWorld,
            color_format: Format,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let device = self.context.device().clone();
            let queue = self.context.graphics_queue().clone();
            let mirrors = visual_world.mirrors().to_vec();
            let msaa_samples = self.msaa_samples;

            for mirror in mirrors {
                if mirror.captures.is_empty() {
                    continue;
                }
                let capture_count = mirror.captures.len();

                let aspect = mirror.aspect_ratio.max(1e-6);
                let base_extent = (1024.0 * mirror.resolution_scale).max(1.0).floor() as u32;
                let mirror_extent = if aspect >= 1.0 {
                    [
                        ((base_extent as f32) * aspect).max(1.0).floor() as u32,
                        base_extent.max(1),
                    ]
                } else {
                    [
                        base_extent.max(1),
                        ((base_extent as f32) / aspect).max(1.0).floor() as u32,
                    ]
                };
                if mirror_extent[0] == 0 || mirror_extent[1] == 0 {
                    continue;
                }

                let (color_views, msaa_color_views, depth_views) = {
                    let mirror_offscreen_key = mirror
                        .captures
                        .first()
                        .map(|capture| capture.target_key.as_str())
                        .ok_or("mirror capture key missing")?;
                    let targets = self.ensure_mirror_offscreen_targets(
                        mirror_offscreen_key,
                        capture_count,
                        mirror_extent,
                        color_format,
                    )?;
                    (
                        targets.color_views.clone(),
                        targets.msaa_color_views.clone(),
                        targets.depth_views.clone(),
                    )
                };

                for (capture_slot, capture) in mirror.captures.iter().enumerate() {
                    let eye_data = &capture.camera;

                    let render_view = RenderView {
                        view: eye_data.view,
                        proj: eye_data.proj,
                        viewport: [mirror_extent[0] as f32, mirror_extent[1] as f32],
                        kind: RenderViewKind::Mirror {
                            mirror_component: mirror.mirror_component,
                            family: capture.family,
                            view_index: capture.view_index,
                            plane_origin: mirror.plane_origin,
                            plane_normal: mirror.plane_normal,
                            excluded_instance: Some(mirror.source_instance),
                        },
                    };

                    let (color_attachment_view, color_resolve_view) =
                        if msaa_samples != SampleCount::Sample1 {
                            (
                                msaa_color_views[capture_slot].clone(),
                                Some(color_views[capture_slot].clone()),
                            )
                        } else {
                            (color_views[capture_slot].clone(), None)
                        };
                    let depth_view = depth_views[capture_slot].clone();

                    let runtime_texture_publication = visual_world
                        .runtime_texture_handle(&capture.target_key)
                        .map(|handle| {
                            let src_view = color_resolve_view
                                .clone()
                                .unwrap_or_else(|| color_attachment_view.clone());
                            (handle, src_view)
                        });

                    let cb = self.build_draw_batches_command_buffer(
                        visual_world,
                        &render_view,
                        0,
                        1,
                        color_attachment_view,
                        color_resolve_view,
                        depth_view,
                        mirror_extent,
                        None,
                        runtime_texture_publication,
                    )?;

                    sync::now(device.clone())
                        .then_execute(queue.clone(), cb)?
                        .then_signal_fence_and_flush()?
                        .wait(None)?;
                }
            }

            Ok(())
        }

        pub fn xr_offscreen_vk_image(&self, eye: usize) -> Option<ash::vk::Image> {
            let targets = self.xr_offscreen.as_ref()?;
            let img = targets.color_images.get(eye)?;
            Some(img.handle())
        }

        pub fn upload_texture_rgba8(
            &mut self,
            handle: TextureHandle,
            rgba: &[u8],
            width: u32,
            height: u32,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if self.textures.contains_key(&handle) {
                return Ok(());
            }

            let view = vulkano_texture_upload::upload_texture_rgba8(
                &self.context,
                &self.command_buffer_allocator,
                rgba,
                width,
                height,
            )?;

            self.textures.insert(
                handle,
                VulkanoGpuTexture {
                    view,
                    extent: [width, height],
                    format: Format::R8G8B8A8_UNORM,
                },
            );
            Ok(())
        }

        pub fn upload_texture_bc7(
            &mut self,
            handle: TextureHandle,
            bc7_blocks: &[u8],
            width: u32,
            height: u32,
            srgb: bool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if self.textures.contains_key(&handle) {
                return Ok(());
            }

            let view = vulkano_texture_upload::upload_texture_bc7(
                &self.context,
                &self.command_buffer_allocator,
                bc7_blocks,
                width,
                height,
                srgb,
            )?;

            self.textures.insert(
                handle,
                VulkanoGpuTexture {
                    view,
                    extent: [width, height],
                    format: if srgb {
                        Format::BC7_SRGB_BLOCK
                    } else {
                        Format::BC7_UNORM_BLOCK
                    },
                },
            );
            Ok(())
        }

        fn ensure_runtime_texture_target(
            &mut self,
            handle: TextureHandle,
            src_view: &Arc<ImageView>,
        ) -> Result<Arc<ImageView>, Box<dyn std::error::Error>> {
            let src_image = src_view.image().clone();
            let src_extent = src_image.extent();
            let extent = [src_extent[0], src_extent[1]];
            let format = src_image.format();

            let image = Image::new(
                self.context.memory_allocator().clone(),
                ImageCreateInfo {
                    image_type: ImageType::Dim2d,
                    format,
                    extent: [extent[0], extent[1], 1],
                    usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
            )?;

            let view = ImageView::new_default(image)
                .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;

            self.pending_runtime_texture_updates.insert(
                handle,
                VulkanoGpuTexture {
                    view: view.clone(),
                    extent,
                    format,
                },
            );

            Ok(view)
        }

        fn create_color_target_view(
            memory_allocator: Arc<StandardMemoryAllocator>,
            format: Format,
            extent: [u32; 2],
            samples: SampleCount,
            usage: ImageUsage,
        ) -> Result<Arc<ImageView>, Box<dyn std::error::Error>> {
            let image = Image::new(
                memory_allocator,
                ImageCreateInfo {
                    image_type: ImageType::Dim2d,
                    format,
                    extent: [extent[0], extent[1], 1],
                    samples,
                    usage,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
            )?;

            Ok(ImageView::new_default(image)?)
        }

        fn ensure_window_runtime_debug_targets(
            &mut self,
            frame_count: usize,
            extent: [u32; 2],
            color_format: Format,
        ) -> Result<(), Box<dyn std::error::Error>> {
            let needs_recreate = self
                .window_runtime_debug_targets
                .as_ref()
                .is_none_or(|targets| {
                    targets.extent != extent
                        || targets.color_format != color_format
                        || targets.color_views.len() != frame_count
                        || (self.msaa_samples == SampleCount::Sample1)
                            != targets.msaa_color_views.is_empty()
                        || (self.msaa_samples != SampleCount::Sample1
                            && targets.msaa_color_views.len() != frame_count)
                });

            if !needs_recreate {
                return Ok(());
            }

            let mut msaa_color_views = Vec::with_capacity(frame_count);
            let mut color_views = Vec::with_capacity(frame_count);
            for _ in 0..frame_count {
                if self.msaa_samples != SampleCount::Sample1 {
                    msaa_color_views.push(Self::create_color_target_view(
                        self.context.memory_allocator().clone(),
                        color_format,
                        extent,
                        self.msaa_samples,
                        ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
                    )?);
                }

                color_views.push(Self::create_color_target_view(
                    self.context.memory_allocator().clone(),
                    color_format,
                    extent,
                    SampleCount::Sample1,
                    ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC,
                )?);
            }

            self.window_runtime_debug_targets = Some(WindowRuntimeDebugTargets {
                extent,
                color_format,
                msaa_color_views,
                color_views,
            });

            Ok(())
        }

        fn build_stencil_clip_debug_batches(
            visual_world: &VisualWorld,
        ) -> Vec<crate::engine::graphics::visual_world::DrawBatch> {
            visual_world
                .stencil_clip_order()
                .iter()
                .enumerate()
                .filter_map(|(slot, &instance_index)| {
                    let instance = visual_world.instances().get(instance_index as usize)?;
                    Some(crate::engine::graphics::visual_world::DrawBatch {
                        material: crate::engine::graphics::MaterialHandle::UNLIT_MESH,
                        mesh: instance.renderable.mesh,
                        texture: None,
                        texture_filtering: TextureFiltering::Nearest,
                        quant_steps: 1.0,
                        stencil_ref: 0,
                        start: slot,
                        count: 1,
                    })
                })
                .collect()
        }

        fn retarget_mirror_surface_textures_for_render_view(
            visual_world: &mut VisualWorld,
            kind: &RenderViewKind,
        ) {
            let (family, view_index) = match *kind {
                RenderViewKind::Window => (
                    crate::engine::graphics::visual_world::MirrorViewerFamily::Monoscopic,
                    0,
                ),
                RenderViewKind::XrEye { eye } => (
                    crate::engine::graphics::visual_world::MirrorViewerFamily::Stereoscopic,
                    eye,
                ),
                RenderViewKind::Mirror {
                    family, view_index, ..
                } => (family, view_index),
            };

            let updates: Vec<_> = visual_world
                .mirrors()
                .iter()
                .filter_map(|mirror| {
                    let key = visual_world.mirror_texture_key_for_view(
                        mirror.mirror_component,
                        family,
                        view_index,
                    )?;
                    let handle = visual_world.runtime_texture_handle(key)?;
                    Some((mirror.source_instance, handle))
                })
                .collect();

            for (instance, handle) in updates {
                let _ = visual_world.update_texture(instance, Some(handle));
            }
        }

        fn hsv_debug_color_for_stencil_ref(stencil_ref: u8) -> [f32; 4] {
            if stencil_ref == 0 {
                return [0.08, 0.08, 0.08, 1.0];
            }

            let hue_deg = (((stencil_ref - 1) as f32) * 100.0) % 360.0;
            let saturation = 0.9;
            let value = 1.0;
            let chroma = value * saturation;
            let hue_sector = hue_deg / 60.0;
            let x = chroma * (1.0 - ((hue_sector % 2.0) - 1.0).abs());

            let (r1, g1, b1) = if hue_sector < 1.0 {
                (chroma, x, 0.0)
            } else if hue_sector < 2.0 {
                (x, chroma, 0.0)
            } else if hue_sector < 3.0 {
                (0.0, chroma, x)
            } else if hue_sector < 4.0 {
                (0.0, x, chroma)
            } else if hue_sector < 5.0 {
                (x, 0.0, chroma)
            } else {
                (chroma, 0.0, x)
            };

            let match_value = value - chroma;
            [r1 + match_value, g1 + match_value, b1 + match_value, 1.0]
        }

        fn build_stencil_clip_debug_instance_buffer(
            &self,
            visual_world: &VisualWorld,
            order: &[u32],
        ) -> Result<Option<Subbuffer<[InstanceData]>>, Box<dyn std::error::Error>> {
            if order.is_empty() {
                return Ok(None);
            }

            let instances_ref = visual_world.instances();
            let instance_data_iter = order.iter().map(|&idx| {
                let inst = instances_ref[idx as usize];
                let m = inst.transform.model;
                InstanceData {
                    i_model_c0: m[0],
                    i_model_c1: m[1],
                    i_model_c2: m[2],
                    i_model_c3: m[3],
                    i_color: Self::hsv_debug_color_for_stencil_ref(
                        inst.stencil_ref.saturating_add(1),
                    ),
                    i_emissive: 1.0,
                    i_opacity: 1.0,
                    i_bones_base: inst.bones_base,
                    i_bones_count: inst.bones_count,
                }
            });

            let buf: Subbuffer<[InstanceData]> = Buffer::from_iter(
                self.context.memory_allocator().clone(),
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                instance_data_iter,
            )?;

            Ok(Some(buf))
        }

        fn apply_pending_runtime_texture_updates(&mut self) {
            if self.pending_runtime_texture_updates.is_empty() {
                return;
            }

            for (handle, texture) in self.pending_runtime_texture_updates.drain() {
                self.textures.insert(handle, texture);
                self.cached_material_sets
                    .retain(|(_, texture_handle, _, _), _| *texture_handle != handle);
            }
        }

        fn collect_runtime_texture_publications(
            &self,
            visual_world: &VisualWorld,
            post_process: &PostProcessInvocation,
        ) -> Vec<(TextureHandle, Arc<ImageView>)> {
            let mut publications = Vec::new();

            if let (Some(emissive_pass), Some(view)) = (
                post_process.config.emissive_pass.as_ref(),
                post_process.targets.bloom_source.clone(),
            ) {
                if let Some(key) = emissive_pass.output_texture.as_deref() {
                    if let Some(handle) = visual_world.runtime_texture_handle(key) {
                        publications.push((handle, view));
                    }
                }
            }

            if let Some(bloom) = post_process.config.bloom.as_ref() {
                if let (Some(key), Some(view)) = (
                    bloom.output_texture.as_deref(),
                    post_process.targets.bloom_a.clone(),
                ) {
                    if let Some(handle) = visual_world.runtime_texture_handle(key) {
                        publications.push((handle, view));
                    }
                }
            }

            publications
        }

        fn recreate_swapchain_if_needed(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            if !(self.window_resized || self.recreate_swapchain) {
                return Ok(());
            }

            // Swapchain recreation can race with in-flight frames during rapid resize/fullscreen
            // transitions. Ensure the GPU is idle before we rebuild swapchain-dependent
            // resources (framebuffers/depth images).
            unsafe {
                self.context
                    .device()
                    .wait_idle()
                    .map_err(|e| -> Box<dyn std::error::Error> {
                        format!("wait_idle failed: {e}").into()
                    })?;

                // IMPORTANT: Vulkano's internal resource tracking is tied to futures. If we drop
                // futures without telling Vulkano they've finished, it can permanently believe a
                // resource is still in use (even if the GPU is idle).
                for slot in self.images_in_flight.iter_mut() {
                    if let Some(mut fut) = slot.take() {
                        fut.signal_finished();
                        fut.cleanup_finished();
                    }
                }
            }

            self.recreate_swapchain = false;

            if let Err(e) = self.swapchain_state.recreate(&self.context, &self.window) {
                self.recreate_swapchain = true;
                println!("[VulkanoRenderer] failed to recreate swapchain: {}", e);
                return Ok(());
            }

            // After swapchain recreation, all old swapchain images/depth attachments are gone.
            // Reset per-image in-flight tracking.
            self.images_in_flight = (0..self.swapchain_state.swapchain_views.len())
                .map(|_| None)
                .collect();

            // Swapchain image count may have changed; rebuild per-slot bones buffers lazily.
            self.cached_bones_buffers.clear();
            self.cached_bones_slot_valid.clear();
            self.cached_bones_capacity = 0;

            self.window_resized = false;
            Ok(())
        }

        fn build_instance_buffer_for_order_or_dummy(
            &self,
            visual_world: &VisualWorld,
            order: &[u32],
        ) -> Result<Subbuffer<[InstanceData]>, Box<dyn std::error::Error>> {
            static DID_LOG_SKIN_INSTANCE_RANGES: AtomicBool = AtomicBool::new(false);

            if !order.is_empty() && env_flag("CAT_DEBUG_SKIN_INSTANCE_RANGES") {
                let instances_ref = visual_world.instances();
                let skinned_count = order
                    .iter()
                    .filter(|&&idx| instances_ref[idx as usize].bones_count > 0)
                    .count();

                if skinned_count > 0 && !DID_LOG_SKIN_INSTANCE_RANGES.swap(true, Ordering::Relaxed)
                {
                    let mut skinned = Vec::new();
                    for &idx in order.iter() {
                        let inst = instances_ref[idx as usize];
                        if inst.bones_count > 0 {
                            skinned.push((idx, inst.renderable, inst.bones_base, inst.bones_count));
                            if skinned.len() >= 24 {
                                break;
                            }
                        }
                    }

                    let total = order.len();
                    println!(
                        "[VulkanoRenderer] instances: total={} with_bones={} (showing up to {})",
                        total,
                        skinned_count,
                        skinned.len()
                    );
                    for (i, (idx, renderable, base, count)) in skinned.iter().enumerate() {
                        println!(
                            "  skinned[{i:02}] instance_index={idx} renderable={renderable:?} bones_base={base} bones_count={count}"
                        );
                    }
                }
            }

            // `Buffer::from_iter` with an empty iterator can panic inside Vulkano.
            let buf: Subbuffer<[InstanceData]> = if order.is_empty() {
                Buffer::from_iter(
                    self.context.memory_allocator().clone(),
                    BufferCreateInfo {
                        usage: BufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_HOST
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    std::iter::once(InstanceData::default()),
                )?
            } else {
                let instances_ref = visual_world.instances();
                let instance_data_iter = order.iter().map(|&idx| {
                    let inst = instances_ref[idx as usize];
                    let m = inst.transform.model;
                    InstanceData {
                        i_model_c0: m[0],
                        i_model_c1: m[1],
                        i_model_c2: m[2],
                        i_model_c3: m[3],
                        i_color: inst.color,
                        i_emissive: inst.emissive,
                        i_opacity: inst.opacity,
                        i_bones_base: inst.bones_base,
                        i_bones_count: inst.bones_count,
                    }
                });

                Buffer::from_iter(
                    self.context.memory_allocator().clone(),
                    BufferCreateInfo {
                        usage: BufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_HOST
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    instance_data_iter,
                )?
            };

            Ok(buf)
        }

        fn build_instance_buffer_for_order_opt(
            &self,
            visual_world: &VisualWorld,
            order: &[u32],
        ) -> Result<Option<Subbuffer<[InstanceData]>>, Box<dyn std::error::Error>> {
            if order.is_empty() {
                return Ok(None);
            }

            let instances_ref = visual_world.instances();
            let instance_data_iter = order.iter().map(|&idx| {
                let inst = instances_ref[idx as usize];
                let m = inst.transform.model;
                InstanceData {
                    i_model_c0: m[0],
                    i_model_c1: m[1],
                    i_model_c2: m[2],
                    i_model_c3: m[3],
                    i_color: inst.color,
                    i_emissive: inst.emissive,
                    i_opacity: inst.opacity,
                    i_bones_base: inst.bones_base,
                    i_bones_count: inst.bones_count,
                }
            });

            let buf: Subbuffer<[InstanceData]> = Buffer::from_iter(
                self.context.memory_allocator().clone(),
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                instance_data_iter,
            )?;

            Ok(Some(buf))
        }

        fn get_or_create_material_set(
            &mut self,
            material: crate::engine::graphics::MaterialHandle,
            texture_handle: TextureHandle,
            filtering: TextureFiltering,
            quant_steps: f32,
        ) -> Result<Option<Arc<DescriptorSet>>, Box<dyn std::error::Error>> {
            match material {
                crate::engine::graphics::MaterialHandle::TOON_MESH
                | crate::engine::graphics::MaterialHandle::UNLIT_MESH
                | crate::engine::graphics::MaterialHandle::SKINNED_TOON_MESH
                | crate::engine::graphics::MaterialHandle::EMISSIVE_TOON_MESH
                | crate::engine::graphics::MaterialHandle::SKINNED_EMISSIVE_TOON_MESH
                | crate::engine::graphics::MaterialHandle::GRID_MESH
                | crate::engine::graphics::MaterialHandle::MIRROR => {}
                _ => return Ok(None),
            }

            let Some(tex) = self.textures.get(&texture_handle) else {
                return Ok(None);
            };

            let quant_bits = quant_steps.to_bits();
            let material_key = (material, texture_handle, filtering, quant_bits);
            if let Some(set) = self.cached_material_sets.get(&material_key) {
                return Ok(Some(set.clone()));
            }

            let material_ubo = Self::create_material_ubo(material, quant_steps);
            let material_buffer: Subbuffer<MaterialUBO> = Buffer::from_data(
                self.context.memory_allocator().clone(),
                BufferCreateInfo {
                    usage: BufferUsage::UNIFORM_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                material_ubo,
            )?;

            let sampler = self.sampler_for(filtering).clone();
            let set = DescriptorSet::new(
                self.descriptor_set_allocator.clone(),
                self.set_layouts.material.clone(),
                [
                    WriteDescriptorSet::buffer(0, material_buffer),
                    WriteDescriptorSet::image_view_sampler(1, tex.view.clone(), sampler),
                ],
                [],
            )?;

            self.cached_material_sets.insert(material_key, set.clone());
            Ok(Some(set))
        }

        fn build_draw_batches_command_buffer(
            &mut self,
            visual_world: &mut VisualWorld,
            render_view: &RenderView,
            bones_slot: usize,
            bones_slots_total: usize,
            color_attachment_view: Arc<ImageView>,
            color_resolve_view: Option<Arc<ImageView>>,
            depth_view: Arc<ImageView>,
            extent: [u32; 2],
            post_process: Option<PostProcessInvocation>,
            runtime_texture_publication: Option<(TextureHandle, Arc<ImageView>)>,
        ) -> Result<
            Arc<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
            Box<dyn std::error::Error>,
        > {
            Self::retarget_mirror_surface_textures_for_render_view(visual_world, &render_view.kind);

            let (camera_target, eye) = match render_view.kind {
                RenderViewKind::Window => (crate::engine::graphics::CameraTarget::Window, 0),
                RenderViewKind::XrEye { eye } => (crate::engine::graphics::CameraTarget::Xr, eye),
                RenderViewKind::Mirror {
                    family, view_index, ..
                } => match family {
                    crate::engine::graphics::visual_world::MirrorViewerFamily::Monoscopic => {
                        (crate::engine::graphics::CameraTarget::Window, view_index)
                    }
                    crate::engine::graphics::visual_world::MirrorViewerFamily::Stereoscopic => {
                        (crate::engine::graphics::CameraTarget::Xr, view_index)
                    }
                },
            };
            let excluded_instance = match &render_view.kind {
                RenderViewKind::Mirror {
                    excluded_instance, ..
                } => *excluded_instance,
                _ => None,
            };

            let queue = self.context.graphics_queue().clone();

            // Always rebuild draw cache cheaply.
            let draw_cache_rebuilt = visual_world.prepare_draw_cache();

            // Multi-layer transparency sort is always per-view.
            visual_world.prepare_transparent_multi_draw_cache_for_view(render_view.view);

            // Consume dirty flags so they reflect "changed since last render".
            // For multi-eye (XR) rendering, only consume on the first eye.
            let instance_data_dirty = if eye == 0 {
                visual_world.take_instance_data_dirty()
            } else {
                visual_world.instance_data_dirty()
            };

            // Only consume the bones palette dirty flag on the first eye.
            let bones_palette_dirty = if eye == 0 {
                visual_world.take_bones_palette_dirty()
            } else {
                false
            };

            // --- Opaque pass ---
            // Buffer indexed by opaque_stream().1; use its length for cache invalidation.
            let (opaque_ops, opaque_instances) = if excluded_instance.is_some() {
                visual_world.opaque_stream_excluding(excluded_instance)
            } else {
                let (ops, instances) = visual_world.opaque_stream();
                (ops.to_vec(), instances.to_vec())
            };
            let instance_count = opaque_instances.len();

            // --- Background pass ---
            // Background instances are stored in their own draw order/batches.
            let background_instance_count = visual_world.background_order().len();
            let background_occluded_lit_instance_count =
                visual_world.background_occluded_lit_order().len();
            let any_background =
                background_instance_count > 0 || background_occluded_lit_instance_count > 0;

            // --- Cutout pass ---
            let (cutout_ops, cutout_instances) = if excluded_instance.is_some() {
                visual_world.cutout_stream_excluding(excluded_instance)
            } else {
                let (ops, instances) = visual_world.cutout_stream();
                (ops.to_vec(), instances.to_vec())
            };
            let cutout_instance_count = cutout_instances.len();

            // --- Overlay pass ---
            // Buffer indexed by overlay_stream().1; use its length for cache invalidation.
            let (overlay_ops, overlay_instances) = if excluded_instance.is_some() {
                visual_world.overlay_stream_excluding(excluded_instance)
            } else {
                let (ops, instances) = visual_world.overlay_stream();
                (ops.to_vec(), instances.to_vec())
            };
            let overlay_instance_count = overlay_instances.len();

            let stencil_clip_debug_requested = camera_target
                == crate::engine::graphics::CameraTarget::Window
                && eye == 0
                && visual_world.stencil_clip_debug_requested();
            let stencil_clip_debug_handle = if stencil_clip_debug_requested {
                visual_world.runtime_texture_handle(INTERNAL_RENDERER_STENCIL_CLIP_DEBUG_SELECTOR)
            } else {
                None
            };
            let stencil_clip_debug_enabled =
                stencil_clip_debug_requested && stencil_clip_debug_handle.is_some();
            let stencil_clip_debug_batches = if stencil_clip_debug_enabled {
                Self::build_stencil_clip_debug_batches(visual_world)
            } else {
                Vec::new()
            };
            let stencil_clip_debug_instance_count = stencil_clip_debug_batches.len();

            // --- Emissive-only post-process source passes ---
            let emissive_instance_count = visual_world.emissive_draw_order().len();
            let emissive_cutout_instance_count = visual_world.emissive_cutout_order().len();

            let need_instance_buffer = instance_data_dirty
                || draw_cache_rebuilt
                || self.cached_instance_buffer.is_none()
                || self.cached_instance_count != instance_count;

            // Opaque instance buffer indexed by opaque_stream().1 (not draw_order) so the
            // stream's batch.start offsets address the correct slot in the buffer.
            let instance_buffer: Subbuffer<[InstanceData]> = if !need_instance_buffer {
                self.cached_instance_buffer
                    .as_ref()
                    .expect("cached_instance_buffer")
                    .clone()
            } else {
                let buf = self
                    .build_instance_buffer_for_order_or_dummy(&*visual_world, &opaque_instances)?;

                self.cached_instance_count = instance_count;
                self.cached_instance_buffer = Some(buf.clone());
                buf
            };

            let need_background_instance_buffer = instance_data_dirty
                || draw_cache_rebuilt
                || self.cached_background_instance_count != background_instance_count;
            let background_instance_buffer = if !need_background_instance_buffer {
                self.cached_background_instance_buffer.clone()
            } else {
                let buf = self.build_instance_buffer_for_order_opt(
                    &*visual_world,
                    visual_world.background_order(),
                )?;
                self.cached_background_instance_count = background_instance_count;
                self.cached_background_instance_buffer = buf.clone();
                buf
            };

            let need_background_occluded_lit_instance_buffer = instance_data_dirty
                || draw_cache_rebuilt
                || self.cached_background_occluded_lit_instance_count
                    != background_occluded_lit_instance_count;
            let background_occluded_lit_instance_buffer =
                if !need_background_occluded_lit_instance_buffer {
                    self.cached_background_occluded_lit_instance_buffer.clone()
                } else {
                    let buf = self.build_instance_buffer_for_order_opt(
                        &*visual_world,
                        visual_world.background_occluded_lit_order(),
                    )?;
                    self.cached_background_occluded_lit_instance_count =
                        background_occluded_lit_instance_count;
                    self.cached_background_occluded_lit_instance_buffer = buf.clone();
                    buf
                };

            let need_cutout_instance_buffer = instance_data_dirty
                || draw_cache_rebuilt
                || self.cached_cutout_instance_count != cutout_instance_count;
            let cutout_instance_buffer = if !need_cutout_instance_buffer {
                self.cached_cutout_instance_buffer.clone()
            } else {
                let buf =
                    self.build_instance_buffer_for_order_opt(&*visual_world, &cutout_instances)?;
                self.cached_cutout_instance_count = cutout_instance_count;
                self.cached_cutout_instance_buffer = buf.clone();
                buf
            };

            let need_overlay_instance_buffer = instance_data_dirty
                || draw_cache_rebuilt
                || self.cached_overlay_instance_count != overlay_instance_count;
            // Overlay buffer indexed by overlay_stream().1 so batch.start offsets are correct.
            let overlay_instance_buffer = if !need_overlay_instance_buffer {
                self.cached_overlay_instance_buffer.clone()
            } else {
                let buf =
                    self.build_instance_buffer_for_order_opt(&*visual_world, &overlay_instances)?;
                self.cached_overlay_instance_count = overlay_instance_count;
                self.cached_overlay_instance_buffer = buf.clone();
                buf
            };

            let stencil_clip_debug_instance_buffer = if stencil_clip_debug_enabled {
                self.build_stencil_clip_debug_instance_buffer(
                    &*visual_world,
                    visual_world.stencil_clip_order(),
                )?
            } else {
                None
            };
            let emissive_instance_buffer = self.build_instance_buffer_for_order_opt(
                &*visual_world,
                visual_world.emissive_draw_order(),
            )?;

            let emissive_cutout_instance_buffer = self.build_instance_buffer_for_order_opt(
                &*visual_world,
                visual_world.emissive_cutout_order(),
            )?;

            let clear_color = visual_world.clear_color();
            let defer_overlay_until_before_final_composite =
                post_process.is_some() && overlay_instance_count > 0;

            let mut color_attachment_clear = RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::Store,
                clear_value: Some(ClearValue::from(clear_color)),
                ..RenderingAttachmentInfo::image_view(color_attachment_view.clone())
            };

            if let Some(resolve_view) = color_resolve_view.clone() {
                color_attachment_clear.resolve_info =
                    Some(RenderingAttachmentResolveInfo::image_view(resolve_view));
                // The multisampled attachment doesn't need to be stored when resolve is used,
                // except when post-process is active and we plan to reopen the scene color
                // attachment for a deferred overlay pass before final composite.
                color_attachment_clear.store_op = if defer_overlay_until_before_final_composite {
                    AttachmentStoreOp::Store
                } else {
                    AttachmentStoreOp::DontCare
                };
            }

            let depth_attachment_clear = RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: if post_process.is_some() {
                    AttachmentStoreOp::Store
                } else {
                    AttachmentStoreOp::DontCare
                },
                clear_value: Some(ClearValue::Depth(1.0)),
                ..RenderingAttachmentInfo::image_view(depth_view.clone())
            };

            let rendering_info_clear_color_and_depth = RenderingInfo {
                render_area_offset: [0, 0],
                render_area_extent: [extent[0], extent[1]],
                layer_count: 1,
                color_attachments: vec![Some(color_attachment_clear)],
                depth_attachment: Some(depth_attachment_clear.clone()),
                stencil_attachment: Some(RenderingAttachmentInfo {
                    load_op: AttachmentLoadOp::Clear,
                    store_op: if defer_overlay_until_before_final_composite {
                        AttachmentStoreOp::Store
                    } else {
                        AttachmentStoreOp::DontCare
                    },
                    clear_value: Some(ClearValue::Stencil(0)),
                    ..RenderingAttachmentInfo::image_view(depth_view.clone())
                }),
                ..Default::default()
            };

            // Engine convention: +Y is up in clip space.
            // Vulkan's default viewport maps NDC Y with opposite direction, so we flip the
            // viewport by using a negative height.
            let viewport = Viewport {
                offset: [0.0, extent[1] as f32],
                extent: [extent[0] as f32, -(extent[1] as f32)],
                depth_range: 0.0..=1.0,
                ..Default::default()
            };

            // Camera uniform buffer (set=0, binding=0) for foreground.
            let camera_ubo_fg = CameraUBO {
                view: render_view.view,
                proj: render_view.proj,
                camera2d: visual_world.camera_2d(),
                viewport: [extent[0] as f32, extent[1] as f32],
                _pad0: [0.0, 0.0],

                ambient_light: visual_world.ambient_light(),
                _pad1: 0.0,
            };

            let camera_buffer_fg: Subbuffer<CameraUBO> = Buffer::from_data(
                self.context.memory_allocator().clone(),
                BufferCreateInfo {
                    usage: BufferUsage::UNIFORM_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                camera_ubo_fg,
            )?;

            // Lights storage buffer (set=0, binding=1).
            let mut lights_ssbo = LightsSSBO::default();
            let lights = visual_world.point_lights();
            let count = (lights.len()).min(MAX_LIGHTS);
            lights_ssbo.count = count as u32;
            for (i, l) in lights.iter().take(count).enumerate() {
                let light_type = match l.light_type {
                    LIGHT_TYPE_POINT => LIGHT_TYPE_POINT,
                    LIGHT_TYPE_DIRECTIONAL => LIGHT_TYPE_DIRECTIONAL,
                    // Default to point for legacy/unknown values.
                    _ => LIGHT_TYPE_POINT,
                };

                lights_ssbo.lights[i] = GpuLight {
                    pos_intensity: [
                        l.position_ws[0],
                        l.position_ws[1],
                        l.position_ws[2],
                        l.intensity,
                    ],
                    color_distance: [l.color[0], l.color[1], l.color[2], l.distance],
                    meta: [light_type, 0, 0, 0],
                };
            }

            let lights_buffer: Subbuffer<LightsSSBO> = Buffer::from_data(
                self.context.memory_allocator().clone(),
                BufferCreateInfo {
                    usage: BufferUsage::STORAGE_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                lights_ssbo,
            )?;

            // Global descriptor set (set=0): per-frame camera + lights.
            //
            // `global_set_fg` is the *foreground* variant used for normal scene rendering
            // (opaque + transparent passes). Its camera UBO uses the full view matrix,
            // so camera translation causes normal parallax.
            let global_set_fg = DescriptorSet::new(
                self.descriptor_set_allocator.clone(),
                self.set_layouts.global.clone(),
                [
                    WriteDescriptorSet::buffer(0, camera_buffer_fg),
                    WriteDescriptorSet::buffer(1, lights_buffer.clone()),
                ],
                [],
            )?;

            // Background global set: same layout + lights, but view translation removed.
            //
            // This makes backgrounds behave like a skybox: they rotate with the camera but
            // do not appear to move when the camera translates.
            let global_set_bg: Option<Arc<DescriptorSet>> = if !any_background {
                None
            } else {
                let mut view_bg = render_view.view;
                view_bg[3] = [0.0, 0.0, 0.0, 1.0];

                let camera_ubo_bg = CameraUBO {
                    view: view_bg,
                    proj: render_view.proj,
                    camera2d: visual_world.camera_2d(),
                    viewport: [extent[0] as f32, extent[1] as f32],
                    _pad0: [0.0, 0.0],

                    ambient_light: visual_world.ambient_light(),
                    _pad1: 0.0,
                };

                let camera_buffer_bg: Subbuffer<CameraUBO> = Buffer::from_data(
                    self.context.memory_allocator().clone(),
                    BufferCreateInfo {
                        usage: BufferUsage::UNIFORM_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_HOST
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    camera_ubo_bg,
                )?;

                let set = DescriptorSet::new(
                    self.descriptor_set_allocator.clone(),
                    self.set_layouts.global.clone(),
                    [
                        WriteDescriptorSet::buffer(0, camera_buffer_bg),
                        WriteDescriptorSet::buffer(1, lights_buffer.clone()),
                    ],
                    [],
                )?;
                Some(set)
            };

            // Rig descriptor set (set=2): shared bones palette + placeholder per-instance lighting.
            // Layout is defined in `PipelineDescriptorSetLayouts::rig`.
            let rig_set: Arc<DescriptorSet> = {
                static DID_LOG_BONES_PALETTE_UPLOAD: AtomicBool = AtomicBool::new(false);

                let want_len = visual_world.bones_palette().len().max(1);

                let want_slots = bones_slots_total.max(1);
                let slot = bones_slot.min(want_slots - 1);

                let needs_realloc = self.cached_bones_buffers.len() != want_slots
                    || self.cached_bones_capacity < want_len;

                if needs_realloc {
                    let new_cap = want_len.next_power_of_two().max(1);

                    let mut buffers = Vec::with_capacity(want_slots);
                    for _ in 0..want_slots {
                        let buffer: Subbuffer<[GpuMat4]> = Buffer::new_slice(
                            self.context.memory_allocator().clone(),
                            BufferCreateInfo {
                                usage: BufferUsage::STORAGE_BUFFER,
                                ..Default::default()
                            },
                            AllocationCreateInfo {
                                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                                ..Default::default()
                            },
                            new_cap as DeviceSize,
                        )?;
                        buffers.push(buffer);
                    }

                    self.cached_bones_buffers = buffers;
                    self.cached_bones_slot_valid = vec![false; want_slots];
                    self.cached_bones_capacity = new_cap;
                }

                if bones_palette_dirty {
                    for v in self.cached_bones_slot_valid.iter_mut() {
                        *v = false;
                    }
                }

                let slot_needs_upload = !self
                    .cached_bones_slot_valid
                    .get(slot)
                    .copied()
                    .unwrap_or(false);

                if slot_needs_upload {
                    let bones_src = visual_world.bones_palette();

                    if env_flag("CAT_DEBUG_BONES_PALETTE")
                        && bones_src.len() > 1
                        && !DID_LOG_BONES_PALETTE_UPLOAD.swap(true, Ordering::Relaxed)
                    {
                        println!(
                            "[VulkanoRenderer] bones palette upload: dirty={} realloc={} want_len={} cached_cap={} src_len={}",
                            bones_palette_dirty,
                            needs_realloc,
                            want_len,
                            self.cached_bones_capacity,
                            bones_src.len()
                        );

                        for (i, m) in bones_src.iter().take(3).enumerate() {
                            println!("  bone[{i:03}]={m:?}");
                        }
                    }

                    let identity = [
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ];

                    let mut dst = self.cached_bones_buffers[slot].write()?;

                    // Write current palette, then fill remainder with identity.
                    if bones_src.is_empty() {
                        dst[0] = GpuMat4 { cols: identity };
                        for slot in dst.iter_mut().skip(1) {
                            *slot = GpuMat4 { cols: identity };
                        }
                    } else {
                        for (i, m) in bones_src.iter().copied().enumerate() {
                            dst[i] = GpuMat4 { cols: m };
                        }
                        for slot in dst.iter_mut().skip(bones_src.len()) {
                            *slot = GpuMat4 { cols: identity };
                        }
                    }

                    if let Some(v) = self.cached_bones_slot_valid.get_mut(slot) {
                        *v = true;
                    }
                }

                let bones_buffer = self.cached_bones_buffers[slot].clone();

                let per_instance_lighting_buffer: Subbuffer<DummyPerInstanceLightingSSBO> =
                    Buffer::from_data(
                        self.context.memory_allocator().clone(),
                        BufferCreateInfo {
                            usage: BufferUsage::STORAGE_BUFFER,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        DummyPerInstanceLightingSSBO::default(),
                    )?;

                DescriptorSet::new(
                    self.descriptor_set_allocator.clone(),
                    self.set_layouts.rig.clone(),
                    [
                        WriteDescriptorSet::buffer(0, per_instance_lighting_buffer),
                        WriteDescriptorSet::buffer(1, bones_buffer),
                    ],
                    [],
                )?
            };

            let mut cbb = AutoCommandBufferBuilder::primary(
                self.command_buffer_allocator.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )?;

            // Single dynamic-rendering scope. This keeps MSAA resolve straightforward.
            cbb.begin_rendering(rendering_info_clear_color_and_depth)?;

            cbb.set_viewport(0, vec![viewport.clone()].into())?;
            cbb.set_scissor(
                0,
                vec![Scissor {
                    offset: [0, 0],
                    extent: [extent[0], extent[1]],
                    ..Default::default()
                }]
                .into(),
            )?;

            if any_background {
                // Background phase: draw:
                // 1) plain background (no depth write)
                // 2) occluded+lit background (depth write ON for self-occlusion)
                if let Some(global_set_bg) = global_set_bg.as_ref() {
                    if let Some(background_instance_buffer) = background_instance_buffer.as_ref() {
                        self.record_background_draws(
                            &mut cbb,
                            visual_world,
                            global_set_bg,
                            &rig_set,
                            background_instance_buffer,
                            background_instance_count,
                        )?;
                    }

                    if let Some(background_occluded_lit_instance_buffer) =
                        background_occluded_lit_instance_buffer.as_ref()
                    {
                        self.record_background_occluded_lit_draws(
                            &mut cbb,
                            visual_world,
                            global_set_bg,
                            &rig_set,
                            background_occluded_lit_instance_buffer,
                            background_occluded_lit_instance_count,
                        )?;
                    }
                }

                // Foreground phase: clear depth so background doesn't occlude.
                // NOTE: `clear_attachments` requires a bound graphics pipeline.
                cbb.bind_pipeline_graphics(self.pipeline_toon_mesh.clone())?;
                cbb.clear_attachments(
                    smallvec::smallvec![ClearAttachment::Depth(1.0)],
                    smallvec::smallvec![ClearRect {
                        offset: [0, 0],
                        extent: [extent[0], extent[1]],
                        array_layers: 0..1,
                    }],
                )?;
            }

            self.record_opaque_draws(
                &mut cbb,
                visual_world,
                &global_set_fg,
                &rig_set,
                &instance_buffer,
                instance_count,
                Some((&opaque_ops, &opaque_instances)),
            )?;

            if let Some(cutout_instance_buffer) = cutout_instance_buffer.as_ref() {
                self.record_cutout_draws(
                    &mut cbb,
                    visual_world,
                    &global_set_fg,
                    &rig_set,
                    cutout_instance_buffer,
                    cutout_instance_count,
                    Some((&cutout_ops, &cutout_instances)),
                )?;
            }

            self.record_transparent_single_draws(
                &mut cbb,
                visual_world,
                &global_set_fg,
                &rig_set,
                eye,
                excluded_instance,
            )?;

            self.record_transparent_multi_draws(
                &mut cbb,
                visual_world,
                &global_set_fg,
                &rig_set,
                camera_target,
                eye,
                excluded_instance,
            )?;

            // Overlay phase: when post-process is disabled, clear depth here so overlay draws on
            // top of the scene immediately. When post-process is enabled, defer overlay until
            // after emissive extraction so opaque/cutout depth can occlude the emissive pass.
            if overlay_instance_count > 0 && !defer_overlay_until_before_final_composite {
                // NOTE: `clear_attachments` requires a bound graphics pipeline.
                cbb.bind_pipeline_graphics(self.pipeline_toon_mesh.clone())?;
                cbb.clear_attachments(
                    smallvec::smallvec![ClearAttachment::Depth(1.0)],
                    smallvec::smallvec![ClearRect {
                        offset: [0, 0],
                        extent: [extent[0], extent[1]],
                        array_layers: 0..1,
                    }],
                )?;

                if let Some(overlay_instance_buffer) = overlay_instance_buffer.as_ref() {
                    self.record_overlay_draws(
                        &mut cbb,
                        visual_world,
                        &global_set_fg,
                        &rig_set,
                        overlay_instance_buffer,
                        overlay_instance_count,
                        Some((&overlay_ops, &overlay_instances)),
                    )?;
                }
            }

            cbb.end_rendering()?;

            if let Some((handle, src_view)) = runtime_texture_publication {
                let dst_view = self.ensure_runtime_texture_target(handle, &src_view)?;
                cbb.copy_image(CopyImageInfo::images(
                    src_view.image().clone(),
                    dst_view.image().clone(),
                ))?;
            }

            if let Some(post_process) = post_process {
                let bloom_radius_pixels = post_process
                    .config
                    .effective_blur_radius_pixels(post_process.targets.bloom_extent[0]);

                let mut blurred_bloom: Option<Arc<ImageView>> = None;
                if let (
                    Some(bloom_cfg),
                    Some(bloom_source),
                    Some(bloom_a),
                    Some(bloom_b),
                    Some(radius_pixels),
                ) = (
                    post_process.config.bloom.as_ref(),
                    post_process.targets.bloom_source.clone(),
                    post_process.targets.bloom_a.clone(),
                    post_process.targets.bloom_b.clone(),
                    bloom_radius_pixels,
                ) {
                    let has_emissive_content =
                        emissive_instance_count > 0 || emissive_cutout_instance_count > 0;

                    if has_emissive_content {
                        let mut bloom_attachment = RenderingAttachmentInfo {
                            load_op: AttachmentLoadOp::Clear,
                            store_op: AttachmentStoreOp::Store,
                            clear_value: Some(ClearValue::from([0.0, 0.0, 0.0, 0.0])),
                            ..RenderingAttachmentInfo::image_view(
                                post_process
                                    .targets
                                    .bloom_source_msaa
                                    .clone()
                                    .unwrap_or_else(|| bloom_source.clone()),
                            )
                        };

                        if post_process.targets.bloom_source_msaa.is_some() {
                            bloom_attachment.resolve_info = Some(
                                RenderingAttachmentResolveInfo::image_view(bloom_source.clone()),
                            );
                            bloom_attachment.store_op = AttachmentStoreOp::DontCare;
                        }

                        cbb.begin_rendering(RenderingInfo {
                            render_area_offset: [0, 0],
                            render_area_extent: [extent[0], extent[1]],
                            layer_count: 1,
                            color_attachments: vec![Some(bloom_attachment)],
                            depth_attachment: Some(RenderingAttachmentInfo {
                                load_op: AttachmentLoadOp::Load,
                                store_op: AttachmentStoreOp::DontCare,
                                ..RenderingAttachmentInfo::image_view(
                                    post_process.targets.depth.clone(),
                                )
                            }),
                            ..Default::default()
                        })?;

                        let bloom_viewport = Viewport {
                            offset: [0.0, extent[1] as f32],
                            extent: [extent[0] as f32, -(extent[1] as f32)],
                            depth_range: 0.0..=1.0,
                            ..Default::default()
                        };

                        cbb.set_viewport(0, vec![bloom_viewport].into())?;
                        cbb.set_scissor(
                            0,
                            vec![Scissor {
                                offset: [0, 0],
                                extent: [extent[0], extent[1]],
                                ..Default::default()
                            }]
                            .into(),
                        )?;

                        if let Some(emissive_instance_buffer) = emissive_instance_buffer.as_ref() {
                            self.record_instanced_draws_for_batches(
                                &mut cbb,
                                &global_set_fg,
                                &rig_set,
                                emissive_instance_buffer,
                                emissive_instance_count,
                                visual_world.emissive_draw_batches(),
                                self.pipeline_emissive_prepass_toon_mesh.clone(),
                                self.pipeline_emissive_prepass_toon_mesh.clone(),
                                self.pipeline_emissive_prepass_toon_mesh.clone(),
                                self.pipeline_emissive_prepass_toon_mesh.clone(),
                                self.pipeline_skinned_emissive_prepass_toon_mesh.clone(),
                                self.pipeline_skinned_emissive_prepass_toon_mesh.clone(),
                            )?;
                        }

                        if let Some(emissive_cutout_instance_buffer) =
                            emissive_cutout_instance_buffer.as_ref()
                        {
                            self.record_instanced_draws_for_batches(
                                &mut cbb,
                                &global_set_fg,
                                &rig_set,
                                emissive_cutout_instance_buffer,
                                emissive_cutout_instance_count,
                                visual_world.emissive_cutout_batches(),
                                self.pipeline_emissive_prepass_toon_mesh_cutout.clone(),
                                self.pipeline_emissive_prepass_toon_mesh_cutout.clone(),
                                self.pipeline_emissive_prepass_toon_mesh_cutout.clone(),
                                self.pipeline_emissive_prepass_toon_mesh_cutout.clone(),
                                self.pipeline_skinned_emissive_prepass_toon_mesh_cutout
                                    .clone(),
                                self.pipeline_skinned_emissive_prepass_toon_mesh_cutout
                                    .clone(),
                            )?;
                        }

                        cbb.end_rendering()?;

                        let bloom_format = post_process.final_color_format;
                        let blur_h_dir = [1.0 / post_process.targets.bloom_extent[0] as f32, 0.0];
                        let blur_v_dir = [0.0, 1.0 / post_process.targets.bloom_extent[1] as f32];

                        self.post_processing_renderer.record_final_pass(
                            &mut cbb,
                            bloom_format,
                            bloom_a.clone(),
                            post_process.targets.bloom_extent,
                            bloom_source,
                            None,
                            &post_process.config,
                        )?;

                        self.post_processing_renderer.record_blur_pass(
                            &mut cbb,
                            bloom_format,
                            bloom_a.clone(),
                            bloom_b.clone(),
                            post_process.targets.bloom_extent,
                            blur_h_dir,
                            radius_pixels,
                        )?;
                        self.post_processing_renderer.record_blur_pass(
                            &mut cbb,
                            bloom_format,
                            bloom_b,
                            bloom_a.clone(),
                            post_process.targets.bloom_extent,
                            blur_v_dir,
                            radius_pixels,
                        )?;

                        if bloom_cfg.intensity > 0.0 {
                            blurred_bloom = Some(bloom_a);
                        }
                    }
                }

                let final_output_view = post_process.final_output_view.clone();

                if overlay_instance_count > 0 {
                    cbb.begin_rendering(RenderingInfo {
                        render_area_offset: [0, 0],
                        render_area_extent: [extent[0], extent[1]],
                        layer_count: 1,
                        color_attachments: vec![Some({
                            let mut color_attachment_load = RenderingAttachmentInfo {
                                load_op: AttachmentLoadOp::Load,
                                store_op: AttachmentStoreOp::Store,
                                ..RenderingAttachmentInfo::image_view(color_attachment_view.clone())
                            };
                            if let Some(resolve_view) = color_resolve_view.clone() {
                                color_attachment_load.resolve_info =
                                    Some(RenderingAttachmentResolveInfo::image_view(resolve_view));
                                color_attachment_load.store_op = AttachmentStoreOp::DontCare;
                            }
                            color_attachment_load
                        })],
                        depth_attachment: Some(RenderingAttachmentInfo {
                            load_op: AttachmentLoadOp::Clear,
                            store_op: AttachmentStoreOp::DontCare,
                            clear_value: Some(ClearValue::Depth(1.0)),
                            ..RenderingAttachmentInfo::image_view(depth_view.clone())
                        }),
                        stencil_attachment: Some(RenderingAttachmentInfo {
                            load_op: AttachmentLoadOp::Load,
                            store_op: AttachmentStoreOp::DontCare,
                            ..RenderingAttachmentInfo::image_view(depth_view.clone())
                        }),
                        ..Default::default()
                    })?;

                    cbb.set_viewport(0, vec![viewport.clone()].into())?;
                    cbb.set_scissor(
                        0,
                        vec![Scissor {
                            offset: [0, 0],
                            extent: [extent[0], extent[1]],
                            ..Default::default()
                        }]
                        .into(),
                    )?;

                    if let Some(overlay_instance_buffer) = overlay_instance_buffer.as_ref() {
                        self.record_overlay_draws(
                            &mut cbb,
                            visual_world,
                            &global_set_fg,
                            &rig_set,
                            overlay_instance_buffer,
                            overlay_instance_count,
                            Some((&overlay_ops, &overlay_instances)),
                        )?;
                    }
                    cbb.end_rendering()?;
                }

                self.post_processing_renderer.record_final_pass(
                    &mut cbb,
                    post_process.final_color_format,
                    final_output_view.clone(),
                    extent,
                    post_process.targets.main_color.clone(),
                    blurred_bloom,
                    &post_process.config,
                )?;

                for (handle, src_view) in
                    self.collect_runtime_texture_publications(visual_world, &post_process)
                {
                    let dst_view = self.ensure_runtime_texture_target(handle, &src_view)?;
                    cbb.copy_image(CopyImageInfo::images(
                        src_view.image().clone(),
                        dst_view.image().clone(),
                    ))?;
                }
            }

            if let (Some(handle), Some(debug_instance_buffer)) = (
                stencil_clip_debug_handle,
                stencil_clip_debug_instance_buffer.as_ref(),
            ) {
                self.ensure_window_runtime_debug_targets(
                    self.swapchain_state.swapchain_views.len(),
                    extent,
                    color_attachment_view.image().format(),
                )?;

                let debug_view = self
                    .window_runtime_debug_targets
                    .as_ref()
                    .and_then(|targets| targets.color_views.get(bones_slot))
                    .cloned()
                    .ok_or("missing window runtime debug target")?;

                let debug_msaa_view = self
                    .window_runtime_debug_targets
                    .as_ref()
                    .and_then(|targets| targets.msaa_color_views.get(bones_slot))
                    .cloned();

                let mut debug_color_attachment = RenderingAttachmentInfo {
                    load_op: AttachmentLoadOp::Clear,
                    store_op: AttachmentStoreOp::Store,
                    clear_value: Some(ClearValue::from([1.0, 0.0, 1.0, 1.0])),
                    ..RenderingAttachmentInfo::image_view(
                        debug_msaa_view
                            .clone()
                            .unwrap_or_else(|| debug_view.clone()),
                    )
                };
                if debug_msaa_view.is_some() {
                    debug_color_attachment.resolve_info = Some(
                        RenderingAttachmentResolveInfo::image_view(debug_view.clone()),
                    );
                    debug_color_attachment.store_op = AttachmentStoreOp::DontCare;
                }

                cbb.begin_rendering(RenderingInfo {
                    render_area_offset: [0, 0],
                    render_area_extent: [extent[0], extent[1]],
                    layer_count: 1,
                    color_attachments: vec![Some(debug_color_attachment)],
                    depth_attachment: Some(RenderingAttachmentInfo {
                        load_op: AttachmentLoadOp::Clear,
                        store_op: AttachmentStoreOp::DontCare,
                        clear_value: Some(ClearValue::Depth(1.0)),
                        ..RenderingAttachmentInfo::image_view(depth_view.clone())
                    }),
                    stencil_attachment: Some(RenderingAttachmentInfo {
                        load_op: AttachmentLoadOp::Clear,
                        store_op: AttachmentStoreOp::DontCare,
                        clear_value: Some(ClearValue::Stencil(0)),
                        ..RenderingAttachmentInfo::image_view(depth_view.clone())
                    }),
                    ..Default::default()
                })?;

                cbb.set_viewport(0, vec![viewport.clone()].into())?;
                cbb.set_scissor(
                    0,
                    vec![Scissor {
                        offset: [0, 0],
                        extent: [extent[0], extent[1]],
                        ..Default::default()
                    }]
                    .into(),
                )?;

                self.record_instanced_draws_for_batches(
                    &mut cbb,
                    &global_set_fg,
                    &rig_set,
                    debug_instance_buffer,
                    stencil_clip_debug_instance_count,
                    &stencil_clip_debug_batches,
                    self.pipeline_toon_mesh.clone(),
                    self.pipeline_mirror_mesh.clone(),
                    self.pipeline_grid_mesh.clone(),
                    self.pipeline_emissive_toon_mesh.clone(),
                    self.pipeline_skinned_toon_mesh.clone(),
                    self.pipeline_skinned_emissive_toon_mesh.clone(),
                )?;

                cbb.end_rendering()?;

                let dst_view = self.ensure_runtime_texture_target(handle, &debug_view)?;
                cbb.copy_image(CopyImageInfo::images(
                    debug_view.image().clone(),
                    dst_view.image().clone(),
                ))?;
            }

            let cb = cbb.build()?;

            Ok(cb)
        }

        pub fn render_visual_world(
            &mut self,
            visual_world: &mut VisualWorld,
        ) -> Result<(), Box<dyn std::error::Error>> {
            self.recreate_swapchain_if_needed()?;
            if !self.pending_runtime_texture_updates.is_empty() {
                unsafe {
                    self.context.device().wait_idle().map_err(
                        |e| -> Box<dyn std::error::Error> {
                            format!("wait_idle failed before runtime texture swap: {e}").into()
                        },
                    )?;
                }
            }
            self.apply_pending_runtime_texture_updates();

            // Let Vulkano drop finished per-frame resources incrementally.
            for fut in self.images_in_flight.iter_mut() {
                if let Some(fut) = fut.as_mut() {
                    fut.cleanup_finished();
                }
            }

            let (image_i, suboptimal, acquire_future) =
                match swapchain::acquire_next_image(self.swapchain_state.swapchain.clone(), None)
                    .map_err(Validated::unwrap)
                {
                    Ok(r) => r,
                    Err(VulkanError::OutOfDate) => {
                        self.recreate_swapchain = true;
                        return Ok(());
                    }
                    Err(e) => return Err(Box::new(e)),
                };

            if suboptimal {
                self.recreate_swapchain = true;
            }

            let extent = self.swapchain_state.swapchain.image_extent();

            // Keep VisualWorld informed of the current output size so camera systems can
            // build aspect-correct projection matrices.
            visual_world.set_viewport([extent[0] as f32, extent[1] as f32]);

            let post_process_config = visual_world.post_processing().clone();
            let post_process_active = post_process_config.is_active();

            if post_process_active {
                self.post_processing_renderer.ensure_window_targets(
                    self.swapchain_state.swapchain_views.len(),
                    extent,
                    self.swapchain_state.swapchain.image_format(),
                    self.swapchain_state.msaa_samples,
                    &post_process_config,
                )?;
            }

            let resolve_view = self.swapchain_state.swapchain_views[image_i as usize].clone();
            let post_process = if post_process_active {
                let targets = self
                    .post_processing_renderer
                    .window_frame_targets(image_i as usize)
                    .ok_or("missing window post-processing targets")?
                    .clone();

                Some(PostProcessInvocation {
                    final_output_view: resolve_view.clone(),
                    final_color_format: self.swapchain_state.swapchain.image_format(),
                    config: post_process_config.clone(),
                    targets,
                })
            } else {
                None
            };

            let device = self.context.device().clone();
            let queue = self.context.graphics_queue().clone();

            self.render_mirror_captures(
                visual_world,
                self.swapchain_state.swapchain.image_format(),
            )?;

            let (color_attachment_view, color_resolve_view, depth_view) =
                if let Some(post) = post_process.as_ref() {
                    (
                        post.targets
                            .main_msaa_color
                            .clone()
                            .unwrap_or_else(|| post.targets.main_color.clone()),
                        if post.targets.main_msaa_color.is_some() {
                            Some(post.targets.main_color.clone())
                        } else {
                            None
                        },
                        post.targets.depth.clone(),
                    )
                } else if self.swapchain_state.msaa_samples != SampleCount::Sample1 {
                    (
                        self.swapchain_state.msaa_color_views[image_i as usize].clone(),
                        Some(resolve_view.clone()),
                        self.swapchain_state.depth_views[image_i as usize].clone(),
                    )
                } else {
                    (
                        resolve_view.clone(),
                        None,
                        self.swapchain_state.depth_views[image_i as usize].clone(),
                    )
                };

            let window_camera = visual_world
                .visual_camera(crate::engine::graphics::CameraTarget::Window)
                .ok_or("missing window camera")?;
            let window_eye = window_camera
                .eyes
                .get(0)
                .ok_or("window camera has no eyes")?;
            let render_view = RenderView {
                view: window_eye.view,
                proj: window_eye.proj,
                viewport: [extent[0] as f32, extent[1] as f32],
                kind: RenderViewKind::Window,
            };

            let cb = self.build_draw_batches_command_buffer(
                visual_world,
                &render_view,
                image_i as usize,
                self.swapchain_state.swapchain_views.len().max(1),
                color_attachment_view,
                color_resolve_view,
                depth_view,
                extent,
                post_process,
                None,
            )?;

            // Ensure we never render into a swapchain image (and its paired depth attachment)
            // while a previous frame that used that image is still in flight.
            let image_i_usize = image_i as usize;
            if self.images_in_flight.len() != self.swapchain_state.swapchain_views.len() {
                // Defensive: should only happen if swapchain recreation failed partially.
                self.images_in_flight = (0..self.swapchain_state.swapchain_views.len())
                    .map(|_| None)
                    .collect();
            }

            let image_future: Box<dyn GpuFuture> = self.images_in_flight[image_i_usize]
                .take()
                .unwrap_or_else(|| sync::now(device.clone()).boxed());

            // Wayland windows don't participate correctly in frame-callback scheduling unless
            // winit is notified just before we submit/present the rendered buffer.
            self.window.pre_present_notify();

            let execution = image_future
                .join(acquire_future)
                .then_execute(queue.clone(), cb)?
                .then_swapchain_present(
                    queue.clone(),
                    SwapchainPresentInfo::swapchain_image_index(
                        self.swapchain_state.swapchain.clone(),
                        image_i,
                    ),
                )
                .then_signal_fence_and_flush();

            match execution.map_err(Validated::unwrap) {
                Ok(future) => {
                    // Track this swapchain image as in flight; this also keeps per-frame
                    // resources alive until the GPU is done.
                    self.images_in_flight[image_i_usize] = Some(future.boxed());
                }
                Err(VulkanError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    // During resize/out-of-date thrash, the command buffer may have been
                    // submitted but we might not get a usable future back. Ensure the GPU
                    // is idle before we drop tracking; otherwise resources (e.g. depth
                    // attachments) can appear "already in use" next frame.
                    unsafe {
                        println!("[VulkanoRenderer] swapchain out of date during flush");
                        let _ = device.wait_idle();
                        for slot in self.images_in_flight.iter_mut() {
                            if let Some(mut fut) = slot.take() {
                                fut.signal_finished();
                                fut.cleanup_finished();
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("[VulkanoRenderer] failed to flush future: {e}");

                    unsafe {
                        println!("[VulkanoRenderer] waiting for device idle after flush failure");
                        let _ = device.wait_idle();
                        for slot in self.images_in_flight.iter_mut() {
                            if let Some(mut fut) = slot.take() {
                                fut.signal_finished();
                                fut.cleanup_finished();
                            }
                        }
                    }
                }
            }

            Ok(())
        }

        pub fn upload_mesh(
            &mut self,
            handle: MeshHandle,
            mesh: &CpuMesh,
        ) -> Result<(), Box<dyn std::error::Error>> {
            static SKIN_UPLOAD_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);

            if self.meshes.contains_key(&handle) {
                return Ok(());
            }

            if mesh.vertices.is_empty() {
                return Err("mesh has no vertices".into());
            }
            if mesh.indices_u32.is_empty() {
                return Err("mesh has no indices".into());
            }

            let memory_allocator = self.context.memory_allocator().clone();
            let queue = self.context.graphics_queue().clone();

            // Host-visible staging buffers.
            let vertices_src = Buffer::from_iter(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                mesh.vertices.iter().copied(),
            )?;

            let skin_src: Option<Subbuffer<[GpuSkinVertex]>> = match (&mesh.joints0, &mesh.weights0)
            {
                (Some(joints0), Some(weights0))
                    if joints0.len() == mesh.vertices.len()
                        && weights0.len() == mesh.vertices.len() =>
                {
                    if env_flag("CAT_DEBUG_SKIN_UPLOAD") {
                        let limit = env_usize("CAT_DEBUG_SKIN_UPLOAD_LIMIT").unwrap_or(3);
                        let n = SKIN_UPLOAD_LOG_COUNT.fetch_add(1, Ordering::Relaxed);
                        if n < limit {
                            println!(
                                "[VulkanoRenderer] skin upload: mesh={handle:?} verts={} indices={} joints0_verts={} weights0_verts={}",
                                mesh.vertices.len(),
                                mesh.indices_u32.len(),
                                joints0.len(),
                                weights0.len()
                            );
                            for vi in 0..mesh.vertices.len().min(8) {
                                let j = joints0[vi];
                                let w = weights0[vi];
                                let sum = w[0] + w[1] + w[2] + w[3];
                                println!("  v[{vi:04}] joints={j:?} weights={w:?} sum={sum:.6}",);
                            }

                            if env_flag("CAT_DEBUG_SKIN_HIST") {
                                let mut joint_weight: HashMap<u16, f32> = HashMap::new();
                                for (j, w) in joints0.iter().copied().zip(weights0.iter().copied())
                                {
                                    for lane in 0..4 {
                                        let jw = w[lane];
                                        if jw > 0.0 {
                                            *joint_weight.entry(j[lane]).or_insert(0.0) += jw;
                                        }
                                    }
                                }

                                let mut entries: Vec<(u16, f32)> =
                                    joint_weight.into_iter().collect();
                                entries.sort_by(|a, b| b.1.total_cmp(&a.1));
                                println!(
                                    "[VulkanoRenderer] skin joint histogram (top 12 by total weight):"
                                );
                                for (rank, (joint, total)) in
                                    entries.into_iter().take(12).enumerate()
                                {
                                    println!("  #{rank:02} joint={joint} total_weight={total:.3}");
                                }
                            }
                        }
                    }

                    let skin_iter =
                        joints0
                            .iter()
                            .copied()
                            .zip(weights0.iter().copied())
                            .map(|(j, w)| GpuSkinVertex {
                                joints0: j,
                                weights0: w,
                            });

                    Some(Buffer::from_iter(
                        memory_allocator.clone(),
                        BufferCreateInfo {
                            usage: BufferUsage::TRANSFER_SRC,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        skin_iter,
                    )?)
                }
                (Some(joints0), Some(weights0)) => {
                    if env_flag("CAT_DEBUG_SKIN_UPLOAD") {
                        println!(
                            "[VulkanoRenderer] skin upload skipped (len mismatch): mesh={handle:?} verts={} joints0_len={} weights0_len={}",
                            mesh.vertices.len(),
                            joints0.len(),
                            weights0.len()
                        );
                    }
                    None
                }
                (Some(joints0), None) => {
                    if env_flag("CAT_DEBUG_SKIN_UPLOAD") {
                        println!(
                            "[VulkanoRenderer] skin upload skipped (missing weights0): mesh={handle:?} verts={} joints0_len={}",
                            mesh.vertices.len(),
                            joints0.len(),
                        );
                    }
                    None
                }
                (None, Some(weights0)) => {
                    if env_flag("CAT_DEBUG_SKIN_UPLOAD") {
                        println!(
                            "[VulkanoRenderer] skin upload skipped (missing joints0): mesh={handle:?} verts={} weights0_len={}",
                            mesh.vertices.len(),
                            weights0.len(),
                        );
                    }
                    None
                }
                (None, None) => None,
            };

            let indices_src = Buffer::from_iter(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                mesh.indices_u32.iter().copied(),
            )?;

            // Device-local destination buffers.
            let vertices_dst = Buffer::new_slice::<CpuVertex>(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER | BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                mesh.vertices.len() as DeviceSize,
            )?;

            let skin_dst: Option<Subbuffer<[GpuSkinVertex]>> = skin_src
                .as_ref()
                .map(|_| {
                    Buffer::new_slice::<GpuSkinVertex>(
                        memory_allocator.clone(),
                        BufferCreateInfo {
                            usage: BufferUsage::VERTEX_BUFFER | BufferUsage::TRANSFER_DST,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                            ..Default::default()
                        },
                        mesh.vertices.len() as DeviceSize,
                    )
                })
                .transpose()?;

            let indices_dst = Buffer::new_slice::<u32>(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::INDEX_BUFFER | BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                mesh.indices_u32.len() as DeviceSize,
            )?;

            // Copy staging -> device-local.
            let mut cbb = AutoCommandBufferBuilder::primary(
                self.command_buffer_allocator.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )?;

            cbb.copy_buffer(CopyBufferInfo::buffers(vertices_src, vertices_dst.clone()))?;
            if let (Some(src), Some(dst)) = (skin_src, skin_dst.as_ref()) {
                cbb.copy_buffer(CopyBufferInfo::buffers(src, dst.clone()))?;
            }
            cbb.copy_buffer(CopyBufferInfo::buffers(indices_src, indices_dst.clone()))?;

            let cb = cbb.build()?;

            cb.execute(queue.clone())?
                .then_signal_fence_and_flush()?
                .wait(None)?;

            self.meshes.insert(
                handle,
                VulkanoGpuMesh {
                    vertices: vertices_dst,
                    skin_vertices: skin_dst,
                    indices: indices_dst,
                    index_count: mesh.index_count(),
                },
            );

            Ok(())
        }
    }
}

/// Vulkano-only renderer.
pub struct VulkanoRenderer {
    vulkano: Option<vulkano_backend::VulkanoState>,
    next_mesh_handle: u32,
    next_texture_handle: u32,
    did_enable_present_loop_log: bool,
    msaa_mode_override: Option<MsaaMode>,
}

impl VulkanoRenderer {
    pub fn new() -> Self {
        Self {
            vulkano: None,
            next_mesh_handle: 0,
            // Reserve handle 0 for the default white texture.
            next_texture_handle: 1,
            did_enable_present_loop_log: false,
            msaa_mode_override: None,
        }
    }

    pub fn msaa_mode_override(&self) -> Option<MsaaMode> {
        self.msaa_mode_override
    }

    pub fn set_msaa_mode(&mut self, mode: MsaaMode) -> Result<(), &'static str> {
        if self.vulkano.is_some() {
            return Err("cannot change MSAA mode after renderer initialization");
        }
        self.msaa_mode_override = Some(mode);
        Ok(())
    }

    pub fn init_for_window(
        &mut self,
        window: &Arc<Window>,
        xr_required: Option<(&[String], &[String])>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.vulkano.is_none() {
            let msaa_mode = self.msaa_mode_override.unwrap_or_default();
            self.vulkano = Some(vulkano_backend::VulkanoState::new(
                window.clone(),
                xr_required,
                msaa_mode,
            )?);
            println!("[VulkanoRenderer] Vulkano swapchain/render-pass initialized");
        }

        Ok(())
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        let _ = size;
        if let Some(vulkano) = self.vulkano.as_mut() {
            vulkano.window_resized = true;
        }
    }

    pub fn upload_mesh(
        &mut self,
        mesh: &CpuMesh,
    ) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        let Some(vulkano) = self.vulkano.as_mut() else {
            return Err("VulkanoRenderer not initialized (call init_for_window first)".into());
        };

        let handle = MeshHandle(self.next_mesh_handle);
        self.next_mesh_handle = self.next_mesh_handle.wrapping_add(1);

        vulkano.upload_mesh(handle, mesh)?;
        Ok(handle)
    }

    pub fn render_visual_world(
        &mut self,
        visual_world: &mut VisualWorld,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(vulkano) = self.vulkano.as_mut() else {
            return Err("VulkanoRenderer not initialized (call init_for_window first)".into());
        };

        if !self.did_enable_present_loop_log {
            self.did_enable_present_loop_log = true;
            println!("[VulkanoRenderer] Present loop enabled");
        }

        vulkano.render_visual_world(visual_world)
    }

    pub fn window_vk_format_raw(&self) -> Option<u32> {
        let vulkano = self.vulkano.as_ref()?;
        let format = vulkano.window_color_format();
        let vk: ash::vk::Format = format.into();
        Some(vk.as_raw() as u32)
    }

    pub fn render_xr_eye_offscreen(
        &mut self,
        visual_world: &mut VisualWorld,
        eye: usize,
        extent: [u32; 2],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(vulkano) = self.vulkano.as_mut() else {
            return Err("VulkanoRenderer not initialized (call init_for_window first)".into());
        };

        vulkano.render_xr_eye_offscreen(visual_world, eye, extent)
    }

    pub fn xr_offscreen_vk_image(&self, eye: usize) -> Option<ash::vk::Image> {
        self.vulkano.as_ref()?.xr_offscreen_vk_image(eye)
    }

    /// Returns raw Vulkan handles suitable for `openxr::Instance::create_session::<openxr::Vulkan>()`.
    ///
    /// Note: OpenXR expects these as opaque pointers; we cast from `ash` raw handles.
    pub fn xr_vulkan_graphics(&self) -> Option<crate::engine::graphics::XrVulkanGraphics> {
        use ash::vk::Handle as _;
        use std::ffi::c_void;
        use vulkano::VulkanObject;

        let vulkano = self.vulkano.as_ref()?;

        let device = vulkano.context.device().clone();
        let queue = vulkano.context.graphics_queue().clone();
        let instance = device.instance().clone();
        let physical_device = device.physical_device();

        let vk_instance = instance.handle().as_raw() as usize as *const c_void;
        let vk_physical_device = physical_device.handle().as_raw() as usize as *const c_void;
        let vk_device = device.handle().as_raw() as usize as *const c_void;
        let vk_queue = queue.handle().as_raw() as usize as *const c_void;

        Some(crate::engine::graphics::XrVulkanGraphics {
            vk_instance,
            vk_physical_device,
            vk_device,
            vk_queue,
            queue_family_index: queue.queue_family_index(),
            // Vulkano doesn't currently expose a stable “queue index within family” API here.
            // Using 0 is correct for the common single-queue case.
            queue_index: 0,
        })
    }
}

impl MeshUploader for VulkanoRenderer {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.upload_mesh(mesh)
    }
}

impl TextureUploader for VulkanoRenderer {
    fn upload_texture_rgba8(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        let Some(vulkano) = self.vulkano.as_mut() else {
            return Err("VulkanoRenderer not initialized (call init_for_window first)".into());
        };

        let handle = TextureHandle(self.next_texture_handle);
        self.next_texture_handle = self.next_texture_handle.wrapping_add(1);

        vulkano.upload_texture_rgba8(handle, rgba, width, height)?;
        Ok(handle)
    }

    fn upload_texture_bc7(
        &mut self,
        bc7_blocks: &[u8],
        width: u32,
        height: u32,
        srgb: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        let Some(vulkano) = self.vulkano.as_mut() else {
            return Err("VulkanoRenderer not initialized (call init_for_window first)".into());
        };

        let handle = TextureHandle(self.next_texture_handle);
        self.next_texture_handle = self.next_texture_handle.wrapping_add(1);

        vulkano.upload_texture_bc7(handle, bc7_blocks, width, height, srgb)?;
        Ok(handle)
    }
}
