use crate::engine::graphics::MeshUploader;
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

    use crate::engine::graphics::mesh::{CpuMesh, CpuVertex};
    use crate::engine::graphics::pipeline_descriptor_set_layouts::PipelineDescriptorSetLayouts;
    use crate::engine::graphics::primitives::MeshHandle;
    use crate::engine::graphics::primitives::TextureHandle;
    use crate::engine::graphics::vulkano_texture_upload;
    use crate::engine::graphics::visual_world::{TextureFiltering, VisualWorld};
    use crate::engine::graphics::vulkano_swapchain::VulkanoSwapchainState;
    use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
    use vulkano::command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo, PrimaryCommandBufferAbstract,
        allocator::StandardCommandBufferAllocator,
    };
    use vulkano::command_buffer::{RenderingAttachmentInfo, RenderingInfo};
    use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
    use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
    use vulkano::format::ClearValue;
    use vulkano::image::view::ImageView;
    use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
    use vulkano::pipeline::graphics::color_blend::{
        AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState,
        ColorComponents,
    };
    use vulkano::pipeline::graphics::depth_stencil::{DepthState, DepthStencilState};
    use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
    use vulkano::pipeline::graphics::multisample::MultisampleState;
    use vulkano::pipeline::graphics::rasterization::RasterizationState;
    use vulkano::pipeline::graphics::subpass::PipelineSubpassType;
    use vulkano::pipeline::graphics::subpass::PipelineRenderingCreateInfo;
    use vulkano::pipeline::graphics::vertex_input::{
        VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
        VertexInputState,
    };
    use vulkano::pipeline::graphics::viewport::{Scissor, Viewport, ViewportState};
    use vulkano::pipeline::layout::{PipelineLayout, PipelineLayoutCreateInfo};

    use vulkano::DeviceSize;
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
    use vulkano::Version;
    use vulkano::VulkanObject;
    use vulkano_util::context::{VulkanoConfig, VulkanoContext};
    use winit::window::Window;

    use vulkano::device::DeviceExtensions;

    mod toon_mesh_vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            path: "assets/shaders/toon-mesh.vert",
        }
    }

    mod toon_mesh_fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            path: "assets/shaders/toon-mesh.frag",
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
    }

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    struct MaterialUBO {
        base_color: [f32; 4],
        quant_steps: f32,
        emissive: u32,
        _pad0: [u32; 2],
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
        #[format(R32_UINT)]
        pub i_emissive: u32,
    }

    pub struct VulkanoGpuMesh {
        #[allow(dead_code)]
        pub vertices: Subbuffer<[CpuVertex]>,
        #[allow(dead_code)]
        pub indices: Subbuffer<[u32]>,
        #[allow(dead_code)]
        pub index_count: u32,
    }

    pub struct VulkanoGpuTexture {
        pub view: Arc<ImageView>,
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

        // --- Per-frame CPU work reduction ---
        cached_instance_buffer: Option<Subbuffer<[InstanceData]>>,
        cached_instance_count: usize,
        cached_material_sets: HashMap<
            (
                crate::engine::graphics::MaterialHandle,
                TextureHandle,
                TextureFiltering,
            ),
            Arc<DescriptorSet>,
        >,

        xr_offscreen: Option<XrOffscreenTargets>,

        pub window_resized: bool,
        pub recreate_swapchain: bool,
        pub images_in_flight: Vec<Option<Box<dyn GpuFuture>>>,
    }

    struct XrOffscreenTargets {
        extent: [u32; 2],
        color_format: Format,
        color_images: Vec<Arc<vulkano::image::Image>>,
        color_views: Vec<Arc<ImageView>>,
        depth_views: Vec<Arc<ImageView>>,
    }

    const MAX_POINT_LIGHTS: usize = 64;

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    struct GpuPointLight {
        // xyz position (world), w intensity
        pos_intensity: [f32; 4],
        // rgb color, w distance
        color_distance: [f32; 4],
    }

    #[derive(BufferContents, Clone, Copy, Debug)]
    #[repr(C, align(16))]
    struct LightsSSBO {
        count: u32,
        _pad0: [u32; 3],
        lights: [GpuPointLight; MAX_POINT_LIGHTS],
    }

    impl Default for LightsSSBO {
        fn default() -> Self {
            Self {
                count: 0,
                _pad0: [0, 0, 0],
                lights: [GpuPointLight::default(); MAX_POINT_LIGHTS],
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

        fn create_material_ubo(material: crate::engine::graphics::MaterialHandle) -> MaterialUBO {
            match material {
                crate::engine::graphics::MaterialHandle::TOON_MESH => MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps: 3.0,
                    emissive: 0,
                    _pad0: [0, 0],
                },
                // While migrating, treat UNLIT as a simple toon material too.
                crate::engine::graphics::MaterialHandle::UNLIT_MESH => MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps: 1.0,
                    emissive: 1,
                    _pad0: [0, 0],
                },
                _ => MaterialUBO::default(),
            }
        }

        pub fn new(
            window: Arc<Window>,
            xr_required: Option<(&[String], &[String])>,
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
                    config.device_filter_fn = Arc::new(move |p| {
                        p.supported_extensions().contains(&required_dev_exts)
                    });

                    if !unknown_instance_exts.is_empty() || !unknown_device_exts.is_empty() {
                        // These might still be satisfied by Vulkan API version or be irrelevant to Vulkano;
                        // we log them so we can extend the mapping as needed.
                        eprintln!(
                            "[VulkanoRenderer] Note: some OpenXR-required Vulkan extensions were not mapped: instance={:?} device={:?}",
                            unknown_instance_exts,
                            unknown_device_exts
                        );
                    }
                }

                VulkanoContext::new(config)
            };
            let device = context.device().clone();

            let swapchain_state = VulkanoSwapchainState::new(&context, window.clone())?;
            let framebuffer_count = swapchain_state.swapchain_views.len();

            let set_layouts = PipelineDescriptorSetLayouts::new(device.clone())?;

            let vs = toon_mesh_vs::load(device.clone())?;
            let fs = toon_mesh_fs::load(device.clone())?;

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

            let layout = PipelineLayout::new(
                device.clone(),
                PipelineLayoutCreateInfo {
                    set_layouts: vec![set_layouts.global.clone(), set_layouts.material.clone()],
                    ..Default::default()
                },
            )?;

            // Important: `CpuVertex` contains more than just position (e.g. UV).
            // We explicitly declare which attributes are consumed by the shader.
            // Instance data occupies locations 1-4 (+ per-instance color/emissive).
            let vertex_input_state = VertexInputState::new()
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
                        format: Format::R32_UINT,
                        offset: 80,
                        ..Default::default()
                    },
                );

            let color_format = swapchain_state.swapchain.image_format();
            let mut pipeline_ci =
                vulkano::pipeline::graphics::GraphicsPipelineCreateInfo::layout(layout);
            pipeline_ci.stages = stages.into();
            pipeline_ci.vertex_input_state = Some(vertex_input_state);
            pipeline_ci.input_assembly_state = Some(InputAssemblyState::default());
            pipeline_ci.viewport_state = Some(ViewportState::default());
            pipeline_ci.rasterization_state = Some(RasterizationState::default());
            pipeline_ci.multisample_state = Some(MultisampleState::default());
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

            pipeline_ci.subpass = Some(PipelineSubpassType::BeginRendering(pipeline_rendering));

            let pipeline_toon_mesh = GraphicsPipeline::new(device.clone(), None, pipeline_ci)?;

            let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
                device.clone(),
                Default::default(),
            ));

            let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
                device.clone(),
                Default::default(),
            ));

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
                meshes: HashMap::new(),

                textures: HashMap::new(),
                sampler_linear,
                sampler_nearest,
                sampler_nearest_mag,
                default_white_texture: TextureHandle(0),

                set_layouts,

                pipeline_toon_mesh,

                cached_instance_buffer: None,
                cached_instance_count: 0,
                cached_material_sets: HashMap::new(),

                xr_offscreen: None,

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

            let needs_recreate = self
                .xr_offscreen
                .as_ref()
                .is_none_or(|t| {
                    t.extent != extent
                        || t.color_format != color_format
                        || t.color_views.len() != view_count
                });

            if !needs_recreate {
                return Ok(());
            }

            let memory_allocator = self.context.memory_allocator().clone();

            let mut color_images = Vec::with_capacity(view_count);
            let mut color_views = Vec::with_capacity(view_count);
            let mut depth_views = Vec::with_capacity(view_count);

            for _ in 0..view_count {
                let color_image = vulkano::image::Image::new(
                    memory_allocator.clone(),
                    vulkano::image::ImageCreateInfo {
                        image_type: vulkano::image::ImageType::Dim2d,
                        format: color_format,
                        extent: [extent[0], extent[1], 1],
                        usage: vulkano::image::ImageUsage::COLOR_ATTACHMENT
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

                let depth_image = vulkano::image::Image::new(
                    memory_allocator.clone(),
                    vulkano::image::ImageCreateInfo {
                        image_type: vulkano::image::ImageType::Dim2d,
                        format: VulkanoSwapchainState::DEPTH_FORMAT,
                        extent: [extent[0], extent[1], 1],
                        usage: vulkano::image::ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                        ..Default::default()
                    },
                )?;

                let depth_view = ImageView::new_default(depth_image)
                    .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;

                color_images.push(color_image);
                color_views.push(color_view);
                depth_views.push(depth_view);
            }

            self.xr_offscreen = Some(XrOffscreenTargets {
                extent,
                color_format,
                color_images,
                color_views,
                depth_views,
            });

            Ok(())
        }

        pub fn render_xr_eye_offscreen(
            &mut self,
            visual_world: &mut VisualWorld,
            eye: usize,
            extent: [u32; 2],
        ) -> Result<(), Box<dyn std::error::Error>> {
            // MVP: assume PRIMARY_STEREO (2 eyes).
            let view_count = 2;
            self.ensure_xr_offscreen_targets(view_count, extent)?;

            let Some(targets) = self.xr_offscreen.as_ref() else {
                return Err("XR offscreen targets missing".into());
            };

            let color_view = targets
                .color_views
                .get(eye)
                .ok_or("XR offscreen eye out of range")?
                .clone();
            let depth_view = targets
                .depth_views
                .get(eye)
                .ok_or("XR depth eye out of range")?
                .clone();

            let cb = self.build_draw_batches_command_buffer(
                visual_world,
                crate::engine::graphics::CameraTarget::Xr,
                eye,
                color_view,
                depth_view,
                extent,
            )?;

            let device = self.context.device().clone();
            let queue = self.context.graphics_queue().clone();

            sync::now(device)
                .then_execute(queue, cb)?
                .then_signal_fence_and_flush()?
                .wait(None)?;

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

            self.textures.insert(handle, VulkanoGpuTexture { view });
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

            self.textures.insert(handle, VulkanoGpuTexture { view });
            Ok(())
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
                    .map_err(|e| -> Box<dyn std::error::Error> { format!("wait_idle failed: {e}").into() })?;

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
                println!(
                    "[VulkanoRenderer] failed to recreate swapchain: {}",
                    e
                );
                return Ok(());
            }

            // After swapchain recreation, all old swapchain images/depth attachments are gone.
            // Reset per-image in-flight tracking.
            self.images_in_flight = (0..self.swapchain_state.swapchain_views.len())
                .map(|_| None)
                .collect();

            self.window_resized = false;
            Ok(())
        }

        fn build_draw_batches_command_buffer(
            &mut self,
            visual_world: &mut VisualWorld,
            camera_target: crate::engine::graphics::CameraTarget,
            eye: usize,
            color_view: Arc<ImageView>,
            depth_view: Arc<ImageView>,
            extent: [u32; 2],
        ) -> Result<Arc<vulkano::command_buffer::PrimaryAutoCommandBuffer>, Box<dyn std::error::Error>> {
            let queue = self.context.graphics_queue().clone();

            // Always rebuild draw cache cheaply.
            visual_world.prepare_draw_cache();

            // Consume dirty flags so they reflect "changed since last render".
            // For multi-eye (XR) rendering, only consume on the first eye.
            let instance_data_dirty = if eye == 0 {
                visual_world.take_instance_data_dirty()
            } else {
                visual_world.instance_data_dirty()
            };

            // Build instance buffer in draw order so each DrawBatch maps to a contiguous range.
            let instance_count = visual_world.draw_order().len();
            let instances_ref = visual_world.instances();

            let need_instance_buffer = instance_data_dirty
                || self.cached_instance_buffer.is_none()
                || self.cached_instance_count != instance_count;

            // `Buffer::from_iter` with an empty iterator can panic inside Vulkano.
            let instance_buffer: Subbuffer<[InstanceData]> = if !need_instance_buffer {
                self.cached_instance_buffer
                    .as_ref()
                    .expect("cached_instance_buffer")
                    .clone()
            } else {
                let instance_data_iter = visual_world.draw_order().iter().map(|&idx| {
                    let inst = instances_ref[idx as usize];
                    let m = inst.transform.model;
                    InstanceData {
                        i_model_c0: m[0],
                        i_model_c1: m[1],
                        i_model_c2: m[2],
                        i_model_c3: m[3],
                        i_color: inst.color,
                        i_emissive: inst.emissive,
                    }
                });

                let buf: Subbuffer<[InstanceData]> = if instance_count == 0 {
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

                self.cached_instance_count = instance_count;
                self.cached_instance_buffer = Some(buf.clone());
                buf
            };

            let clear_color = match camera_target {
                crate::engine::graphics::CameraTarget::Xr => [1.0f32, 0.713_725_5, 0.756_862_76, 1.0],
                crate::engine::graphics::CameraTarget::Window => [0.0f32, 0.0, 0.0, 1.0],
            };

            let color_attachment = RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::Store,
                clear_value: Some(ClearValue::from(clear_color)),
                ..RenderingAttachmentInfo::image_view(color_view)
            };

            let depth_attachment = RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::DontCare,
                clear_value: Some(ClearValue::Depth(1.0)),
                ..RenderingAttachmentInfo::image_view(depth_view)
            };

            let rendering_info = RenderingInfo {
                render_area_offset: [0, 0],
                render_area_extent: [extent[0], extent[1]],
                layer_count: 1,
                color_attachments: vec![Some(color_attachment)],
                depth_attachment: Some(depth_attachment),
                stencil_attachment: None,
                ..Default::default()
            };

            let viewport = Viewport {
                offset: [0.0, 0.0],
                extent: [extent[0] as f32, extent[1] as f32],
                depth_range: 0.0..=1.0,
                ..Default::default()
            };

            // Camera uniform buffer (set=0, binding=0).
            let camera_ubo = CameraUBO {
                view: visual_world.camera_view_for_eye(camera_target, eye),
                proj: visual_world.camera_proj_for_eye(camera_target, eye),
                camera2d: visual_world.camera_2d(),
                viewport: [extent[0] as f32, extent[1] as f32],
                _pad0: [0.0, 0.0],
            };

            let camera_buffer: Subbuffer<CameraUBO> = Buffer::from_data(
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
                camera_ubo,
            )?;

            // Lights storage buffer (set=0, binding=1).
            let mut lights_ssbo = LightsSSBO::default();
            let lights = visual_world.point_lights();
            let count = (lights.len()).min(MAX_POINT_LIGHTS);
            lights_ssbo.count = count as u32;
            for (i, l) in lights.iter().take(count).enumerate() {
                lights_ssbo.lights[i] = GpuPointLight {
                    pos_intensity: [
                        l.position_ws[0],
                        l.position_ws[1],
                        l.position_ws[2],
                        l.intensity,
                    ],
                    color_distance: [l.color[0], l.color[1], l.color[2], l.distance],
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

            let global_set = DescriptorSet::new(
                self.descriptor_set_allocator.clone(),
                self.set_layouts.global.clone(),
                [
                    WriteDescriptorSet::buffer(0, camera_buffer),
                    WriteDescriptorSet::buffer(1, lights_buffer),
                ],
                [],
            )?;

            let mut cbb = AutoCommandBufferBuilder::primary(
                self.command_buffer_allocator.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )?;

            cbb.begin_rendering(rendering_info)?;

            cbb.set_viewport(0, vec![viewport].into())?;
            cbb.set_scissor(
                0,
                vec![Scissor {
                    offset: [0, 0],
                    extent: [extent[0], extent[1]],
                    ..Default::default()
                }]
                .into(),
            )?;

            // Bind pipeline/descriptor sets per (material, texture).
            let mut bound_material: Option<crate::engine::graphics::MaterialHandle> = None;
            let mut bound_texture: Option<TextureHandle> = None;
            let mut bound_filtering: Option<TextureFiltering> = None;

            for batch in visual_world.draw_batches() {
                let texture_handle = batch.texture.unwrap_or(self.default_white_texture);

                let filtering = batch.texture_filtering;

                if bound_material != Some(batch.material)
                    || bound_texture != Some(texture_handle)
                    || bound_filtering != Some(filtering)
                {
                    match batch.material {
                        crate::engine::graphics::MaterialHandle::TOON_MESH
                        | crate::engine::graphics::MaterialHandle::UNLIT_MESH => {
                            let Some(tex) = self.textures.get(&texture_handle) else {
                                continue;
                            };

                            let material_key = (batch.material, texture_handle, filtering);
                            let material_set = if let Some(set) =
                                self.cached_material_sets.get(&material_key)
                            {
                                set.clone()
                            } else {
                                let material_ubo = Self::create_material_ubo(batch.material);
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
                                        WriteDescriptorSet::image_view_sampler(
                                            1,
                                            tex.view.clone(),
                                            sampler,
                                        ),
                                    ],
                                    [],
                                )?;

                                self.cached_material_sets.insert(material_key, set.clone());
                                set
                            };

                            cbb.bind_pipeline_graphics(self.pipeline_toon_mesh.clone())?;
                            cbb.bind_descriptor_sets(
                                PipelineBindPoint::Graphics,
                                self.pipeline_toon_mesh.layout().clone(),
                                0,
                                (global_set.clone(), material_set),
                            )?;
                        }
                        _ => {
                            continue;
                        }
                    }

                    bound_material = Some(batch.material);
                    bound_texture = Some(texture_handle);
                    bound_filtering = Some(filtering);
                }

                let Some(mesh) = self.meshes.get(&batch.mesh) else {
                    continue;
                };
                cbb.bind_vertex_buffers(0, (mesh.vertices.clone(), instance_buffer.clone()))?;
                cbb.bind_index_buffer(mesh.indices.clone())?;

                if instance_count > 0 && batch.count > 0 {
                    unsafe {
                        cbb.draw_indexed(
                            mesh.index_count,
                            batch.count as u32,
                            0,
                            0,
                            batch.start as u32,
                        )?;
                    }
                }
            }

            cbb.end_rendering()?;

            let cb = cbb.build()?;

            Ok(cb)
        }

        pub fn render_visual_world(
            &mut self,
            visual_world: &mut VisualWorld,
        ) -> Result<(), Box<dyn std::error::Error>> {
            self.recreate_swapchain_if_needed()?;

            let device = self.context.device().clone();
            let queue = self.context.graphics_queue().clone();

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

            let color_view = self.swapchain_state.swapchain_views[image_i as usize].clone();
            let depth_view = self.swapchain_state.depth_views[image_i as usize].clone();
            let cb = self.build_draw_batches_command_buffer(
                visual_world,
                crate::engine::graphics::CameraTarget::Window,
                0,
                color_view,
                depth_view,
                extent,
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
            cbb.copy_buffer(CopyBufferInfo::buffers(indices_src, indices_dst.clone()))?;

            let cb = cbb.build()?;

            cb.execute(queue.clone())?
                .then_signal_fence_and_flush()?
                .wait(None)?;

            self.meshes.insert(
                handle,
                VulkanoGpuMesh {
                    vertices: vertices_dst,
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
}

impl VulkanoRenderer {
    pub fn new() -> Self {
        Self {
            vulkano: None,
            next_mesh_handle: 0,
            // Reserve handle 0 for the default white texture.
            next_texture_handle: 1,
            did_enable_present_loop_log: false,
        }
    }

    pub fn init_for_window(
        &mut self,
        window: &Arc<Window>,
        xr_required: Option<(&[String], &[String])>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.vulkano.is_none() {
            self.vulkano = Some(vulkano_backend::VulkanoState::new(window.clone(), xr_required)?);
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
        use std::ffi::c_void;
        use vulkano::VulkanObject;
        use ash::vk::Handle as _;

        let vulkano = self.vulkano.as_ref()?;

        let device = vulkano.context.device().clone();
        let queue = vulkano.context.graphics_queue().clone();
        let instance = device.instance().clone();
        let physical_device = device.physical_device();

        let vk_instance = instance.handle().as_raw() as usize as *const c_void;
        let vk_physical_device = physical_device.handle().as_raw() as usize as *const c_void;
        let vk_device = device.handle().as_raw() as usize as *const c_void;

        Some(crate::engine::graphics::XrVulkanGraphics {
            vk_instance,
            vk_physical_device,
            vk_device,
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
