use std::collections::{BTreeMap, HashMap};
use std::mem::size_of;
use std::sync::Arc;

use vulkano::buffer::BufferContents;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderingAttachmentInfo, RenderingInfo,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::layout::{
    DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
    DescriptorType,
};
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::device::Device;
use vulkano::format::{ClearValue, Format};
use vulkano::image::sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo};
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage, SampleCount};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::pipeline::graphics::color_blend::{ColorBlendAttachmentState, ColorBlendState};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::multisample::MultisampleState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::subpass::{
    PipelineRenderingCreateInfo, PipelineSubpassType,
};
use vulkano::pipeline::graphics::vertex_input::VertexInputState;
use vulkano::pipeline::graphics::viewport::{Scissor, Viewport, ViewportState};
use vulkano::pipeline::layout::{PipelineLayout, PipelineLayoutCreateInfo, PushConstantRange};
use vulkano::pipeline::{
    DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineShaderStageCreateInfo,
};
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::shader::ShaderStages;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BloomSource {
    #[default]
    Emissive,
}

#[derive(Debug, Clone)]
pub struct EmissivePassConfig {
    pub output_texture: Option<String>,
}

impl Default for EmissivePassConfig {
    fn default() -> Self {
        Self {
            output_texture: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlurPassConfig {
    pub enabled: bool,
    pub radius_ndc: f32,
    pub half_res: bool,
}

impl Default for BlurPassConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            radius_ndc: 0.05,
            half_res: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BloomConfig {
    pub intensity: f32,
    pub radius_ndc: f32,
    pub emissive_scale: f32,
    pub half_res: bool,
    pub source: BloomSource,
    pub debug_emissive_texture: Option<String>,
    pub debug_bloom_texture: Option<String>,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            radius_ndc: 0.05,
            emissive_scale: 1.0,
            half_res: true,
            source: BloomSource::Emissive,
            debug_emissive_texture: None,
            debug_bloom_texture: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BokehConfig {
    pub focus_distance: f32,
    pub aperture: f32,
    pub max_blur_radius: f32,
}

impl Default for BokehConfig {
    fn default() -> Self {
        Self {
            focus_distance: 2.0,
            aperture: 0.0,
            max_blur_radius: 8.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PostProcessingConfig {
    pub enabled: bool,
    pub emissive_pass: Option<EmissivePassConfig>,
    pub blur_pass: Option<BlurPassConfig>,
    pub bloom: Option<BloomConfig>,
    pub bokeh: Option<BokehConfig>,
    pub debug_show_emissive: bool,
    pub debug_show_bloom: bool,
}

impl PostProcessingConfig {
    pub fn is_active(&self) -> bool {
        self.enabled && (self.bloom.is_some() || self.bokeh.is_some())
    }

    pub fn needs_depth(&self) -> bool {
        self.enabled && (self.bloom.is_some() || self.bokeh.is_some())
    }

    pub fn bloom_radius_pixels(&self, viewport_width: u32) -> Option<u32> {
        let bloom = self.bloom.as_ref()?;
        Some(Self::ndc_radius_to_pixels(bloom.radius_ndc, viewport_width))
    }

    /// Effective blur radius: uses `BlurPassConfig` when present, falls back to bloom.
    pub fn effective_blur_radius_pixels(&self, viewport_width: u32) -> Option<u32> {
        if let Some(blur) = &self.blur_pass {
            if blur.enabled {
                return Some(Self::ndc_radius_to_pixels(blur.radius_ndc, viewport_width));
            }
        }
        self.bloom_radius_pixels(viewport_width)
    }

    /// Effective half-res flag: uses `BlurPassConfig` when present, falls back to bloom.
    pub fn effective_blur_half_res(&self) -> bool {
        if let Some(blur) = &self.blur_pass {
            if blur.enabled {
                return blur.half_res;
            }
        }
        self.bloom.as_ref().map(|b| b.half_res).unwrap_or(false)
    }

    pub fn ndc_radius_to_pixels(radius_ndc: f32, viewport_width: u32) -> u32 {
        ((radius_ndc.max(0.0) * viewport_width as f32) / 2.0)
            .round()
            .max(1.0) as u32
    }
}

#[derive(Debug, Clone)]
pub struct PostProcessFrameTargets {
    pub main_color: Arc<ImageView>,
    pub main_msaa_color: Option<Arc<ImageView>>,
    pub depth: Arc<ImageView>,
    pub bloom_source_msaa: Option<Arc<ImageView>>,
    pub bloom_source: Option<Arc<ImageView>>,
    pub bloom_a: Option<Arc<ImageView>>,
    pub bloom_b: Option<Arc<ImageView>>,
    pub bloom_extent: [u32; 2],
}

#[derive(Debug, Clone)]
struct PostProcessTargetSet {
    extent: [u32; 2],
    color_format: Format,
    msaa_samples: SampleCount,
    frames: Vec<PostProcessFrameTargets>,
}

#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct PostProcessPushConstants {
    direction: [f32; 2],
    bloom_intensity: f32,
    radius_pixels: f32,
}

mod fullscreen_vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "assets/shaders/post-process-fullscreen.vert",
    }
}

mod blit_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "assets/shaders/post-process-copy.frag",
    }
}

mod blur_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "assets/shaders/post-process-bloom-blur.frag",
    }
}

mod composite_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "assets/shaders/post-process-bloom-composite.frag",
    }
}

pub struct PostProcessingRenderer {
    device: Arc<Device>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    sampler_linear: Arc<Sampler>,
    sampled_layout: Arc<DescriptorSetLayout>,
    blit_pipelines: HashMap<Format, Arc<GraphicsPipeline>>,
    blur_pipelines: HashMap<Format, Arc<GraphicsPipeline>>,
    composite_pipelines: HashMap<Format, Arc<GraphicsPipeline>>,
    window_targets: Option<PostProcessTargetSet>,
    xr_targets: Option<PostProcessTargetSet>,
}

impl PostProcessingRenderer {
    pub fn new(
        device: Arc<Device>,
        memory_allocator: Arc<StandardMemoryAllocator>,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut bindings = BTreeMap::new();

        let mut src0 = DescriptorSetLayoutBinding::descriptor_type(DescriptorType::CombinedImageSampler);
        src0.descriptor_count = 1;
        src0.stages = ShaderStages::FRAGMENT;
        bindings.insert(0, src0);

        let mut src1 = DescriptorSetLayoutBinding::descriptor_type(DescriptorType::CombinedImageSampler);
        src1.descriptor_count = 1;
        src1.stages = ShaderStages::FRAGMENT;
        bindings.insert(1, src1);

        let sampled_layout = DescriptorSetLayout::new(
            device.clone(),
            DescriptorSetLayoutCreateInfo {
                bindings,
                ..Default::default()
            },
        )?;

        let sampler_linear = Sampler::new(
            device.clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [SamplerAddressMode::ClampToEdge; 3],
                ..Default::default()
            },
        )?;

        Ok(Self {
            device,
            memory_allocator,
            descriptor_set_allocator,
            sampler_linear,
            sampled_layout,
            blit_pipelines: HashMap::new(),
            blur_pipelines: HashMap::new(),
            composite_pipelines: HashMap::new(),
            window_targets: None,
            xr_targets: None,
        })
    }

    pub fn ensure_window_targets(
        &mut self,
        frame_count: usize,
        extent: [u32; 2],
        color_format: Format,
        msaa_samples: SampleCount,
        config: &PostProcessingConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !config.is_active() {
            self.window_targets = None;
            return Ok(());
        }

        let needs_recreate = self.window_targets.as_ref().is_none_or(|targets| {
            targets.extent != extent
                || targets.color_format != color_format
                || targets.msaa_samples != msaa_samples
                || targets.frames.len() != frame_count
        });

        if needs_recreate {
            self.window_targets = Some(self.build_target_set(
                frame_count,
                extent,
                color_format,
                msaa_samples,
                config,
            )?);
        }

        Ok(())
    }

    pub fn ensure_xr_targets(
        &mut self,
        view_count: usize,
        extent: [u32; 2],
        color_format: Format,
        msaa_samples: SampleCount,
        config: &PostProcessingConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !config.is_active() {
            self.xr_targets = None;
            return Ok(());
        }

        let needs_recreate = self.xr_targets.as_ref().is_none_or(|targets| {
            targets.extent != extent
                || targets.color_format != color_format
                || targets.msaa_samples != msaa_samples
                || targets.frames.len() != view_count
        });

        if needs_recreate {
            self.xr_targets = Some(self.build_target_set(
                view_count,
                extent,
                color_format,
                msaa_samples,
                config,
            )?);
        }

        Ok(())
    }

    pub fn window_frame_targets(&self, frame: usize) -> Option<&PostProcessFrameTargets> {
        self.window_targets.as_ref()?.frames.get(frame)
    }

    pub fn xr_frame_targets(&self, eye: usize) -> Option<&PostProcessFrameTargets> {
        self.xr_targets.as_ref()?.frames.get(eye)
    }

    pub fn record_blur_pass(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        color_format: Format,
        source: Arc<ImageView>,
        destination: Arc<ImageView>,
        extent: [u32; 2],
        direction: [f32; 2],
        radius_pixels: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pipeline = self.blur_pipeline(color_format)?;
        let set = self.sampled_set(source.clone(), source)?;
        self.record_fullscreen_pass(
            cbb,
            destination,
            extent,
            pipeline,
            set,
            PostProcessPushConstants {
                direction,
                bloom_intensity: 0.0,
                radius_pixels: radius_pixels as f32,
            },
        )
    }

    pub fn record_final_pass(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        color_format: Format,
        output: Arc<ImageView>,
        extent: [u32; 2],
        main_color: Arc<ImageView>,
        bloom_color: Option<Arc<ImageView>>,
        config: &PostProcessingConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (pipeline, set) = if let Some(bloom_view) = bloom_color {
            (
                self.composite_pipeline(color_format)?,
                self.sampled_set(main_color, bloom_view)?,
            )
        } else {
            (
                self.blit_pipeline(color_format)?,
                self.sampled_set(main_color.clone(), main_color)?,
            )
        };

        self.record_fullscreen_pass(
            cbb,
            output,
            extent,
            pipeline,
            set,
            PostProcessPushConstants {
                direction: [0.0, 0.0],
                bloom_intensity: config
                    .bloom
                    .as_ref()
                    .map(|b| b.intensity.max(0.0))
                    .unwrap_or(0.0),
                radius_pixels: 0.0,
            },
        )
    }

    pub fn record_debug_overlay_pass(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        color_format: Format,
        output: Arc<ImageView>,
        _output_extent: [u32; 2],
        panel_offset: [u32; 2],
        panel_extent: [u32; 2],
        source: Arc<ImageView>,
        flip_y: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pipeline = self.blit_pipeline(color_format)?;
        let set = self.sampled_set(source.clone(), source)?;

        let border = 6u32.min(panel_extent[0] / 4).min(panel_extent[1] / 4);
        let inner_offset = [
            panel_offset[0].saturating_add(border),
            panel_offset[1].saturating_add(border),
        ];
        let inner_extent = [
            panel_extent[0].saturating_sub(border.saturating_mul(2)).max(1),
            panel_extent[1].saturating_sub(border.saturating_mul(2)).max(1),
        ];

        let viewport = if flip_y {
            Viewport {
                offset: [
                    inner_offset[0] as f32,
                    (inner_offset[1] + inner_extent[1]) as f32,
                ],
                extent: [inner_extent[0] as f32, -(inner_extent[1] as f32)],
                depth_range: 0.0..=1.0,
                ..Default::default()
            }
        } else {
            Viewport {
                offset: [inner_offset[0] as f32, inner_offset[1] as f32],
                extent: [inner_extent[0] as f32, inner_extent[1] as f32],
                depth_range: 0.0..=1.0,
                ..Default::default()
            }
        };

        cbb.begin_rendering(RenderingInfo {
            render_area_offset: [panel_offset[0], panel_offset[1]],
            render_area_extent: panel_extent,
            layer_count: 1,
            color_attachments: vec![Some(RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::Store,
                clear_value: Some(ClearValue::from([1.0, 1.0, 1.0, 1.0])),
                ..RenderingAttachmentInfo::image_view(output)
            })],
            ..Default::default()
        })?;

        cbb.set_viewport(0, vec![viewport].into())?;
        cbb.set_scissor(
            0,
            vec![Scissor {
                offset: [inner_offset[0], inner_offset[1]],
                extent: inner_extent,
                ..Default::default()
            }]
            .into(),
        )?;
        cbb.bind_pipeline_graphics(pipeline.clone())?;
        cbb.bind_descriptor_sets(
            PipelineBindPoint::Graphics,
            pipeline.layout().clone(),
            0,
            set,
        )?;
        cbb.push_constants(
            pipeline.layout().clone(),
            0,
            PostProcessPushConstants {
                direction: [0.0, 0.0],
                bloom_intensity: 0.0,
                radius_pixels: 0.0,
            },
        )?;
        unsafe {
            cbb.draw(3, 1, 0, 0)?;
        }
        cbb.end_rendering()?;
        Ok(())
    }

    fn record_fullscreen_pass(
        &self,
        cbb: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        output: Arc<ImageView>,
        extent: [u32; 2],
        pipeline: Arc<GraphicsPipeline>,
        set: Arc<DescriptorSet>,
        push_constants: PostProcessPushConstants,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: [extent[0] as f32, extent[1] as f32],
            depth_range: 0.0..=1.0,
            ..Default::default()
        };

        cbb.begin_rendering(RenderingInfo {
            render_area_offset: [0, 0],
            render_area_extent: extent,
            layer_count: 1,
            color_attachments: vec![Some(RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::Store,
                clear_value: Some(ClearValue::from([0.0, 0.0, 0.0, 1.0])),
                ..RenderingAttachmentInfo::image_view(output)
            })],
            ..Default::default()
        })?;

        cbb.set_viewport(0, vec![viewport].into())?;
        cbb.set_scissor(
            0,
            vec![Scissor {
                offset: [0, 0],
                extent,
                ..Default::default()
            }]
            .into(),
        )?;
        cbb.bind_pipeline_graphics(pipeline.clone())?;
        cbb.bind_descriptor_sets(
            PipelineBindPoint::Graphics,
            pipeline.layout().clone(),
            0,
            set,
        )?;
        cbb.push_constants(pipeline.layout().clone(), 0, push_constants)?;
        unsafe {
            cbb.draw(3, 1, 0, 0)?;
        }
        cbb.end_rendering()?;
        Ok(())
    }

    fn build_target_set(
        &self,
        count: usize,
        extent: [u32; 2],
        color_format: Format,
        msaa_samples: SampleCount,
        config: &PostProcessingConfig,
    ) -> Result<PostProcessTargetSet, Box<dyn std::error::Error>> {
        let bloom_extent = if config.effective_blur_half_res() {
            [extent[0].max(2) / 2, extent[1].max(2) / 2]
        } else {
            extent
        };

        let mut frames = Vec::with_capacity(count);
        for _ in 0..count {
            let main_color = Self::create_color_view(
                self.memory_allocator.clone(),
                color_format,
                extent,
                SampleCount::Sample1,
                ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC,
            )?;
            let main_msaa_color = if msaa_samples != SampleCount::Sample1 {
                Some(Self::create_color_view(
                    self.memory_allocator.clone(),
                    color_format,
                    extent,
                    msaa_samples,
                    ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
                )?)
            } else {
                None
            };
            let depth = Self::create_depth_view(self.memory_allocator.clone(), extent, msaa_samples)?;

            let (bloom_source_msaa, bloom_source, bloom_a, bloom_b) = if config.bloom.is_some() {
                (
                    if msaa_samples != SampleCount::Sample1 {
                        Some(Self::create_color_view(
                            self.memory_allocator.clone(),
                            color_format,
                            extent,
                            msaa_samples,
                            ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
                        )?)
                    } else {
                        None
                    },
                    Some(Self::create_color_view(
                        self.memory_allocator.clone(),
                        color_format,
                        extent,
                        SampleCount::Sample1,
                        ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC,
                    )?),
                    Some(Self::create_color_view(
                        self.memory_allocator.clone(),
                        color_format,
                        bloom_extent,
                        SampleCount::Sample1,
                        ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC,
                    )?),
                    Some(Self::create_color_view(
                        self.memory_allocator.clone(),
                        color_format,
                        bloom_extent,
                        SampleCount::Sample1,
                        ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC,
                    )?),
                )
            } else {
                (None, None, None, None)
            };

            frames.push(PostProcessFrameTargets {
                main_color,
                main_msaa_color,
                depth,
                bloom_source_msaa,
                bloom_source,
                bloom_a,
                bloom_b,
                bloom_extent,
            });
        }

        Ok(PostProcessTargetSet {
            extent,
            color_format,
            msaa_samples,
            frames,
        })
    }

    fn create_color_view(
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

    fn create_depth_view(
        memory_allocator: Arc<StandardMemoryAllocator>,
        extent: [u32; 2],
        samples: SampleCount,
    ) -> Result<Arc<ImageView>, Box<dyn std::error::Error>> {
        let image = Image::new(
            memory_allocator,
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::D32_SFLOAT,
                extent: [extent[0], extent[1], 1],
                samples,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        )?;
        Ok(ImageView::new_default(image)?)
    }

    fn sampled_set(
        &self,
        source0: Arc<ImageView>,
        source1: Arc<ImageView>,
    ) -> Result<Arc<DescriptorSet>, Box<dyn std::error::Error>> {
        Ok(DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.sampled_layout.clone(),
            [
                WriteDescriptorSet::image_view_sampler(0, source0, self.sampler_linear.clone()),
                WriteDescriptorSet::image_view_sampler(1, source1, self.sampler_linear.clone()),
            ],
            [],
        )?)
    }

    fn blit_pipeline(
        &mut self,
        color_format: Format,
    ) -> Result<Arc<GraphicsPipeline>, Box<dyn std::error::Error>> {
        if let Some(existing) = self.blit_pipelines.get(&color_format) {
            return Ok(existing.clone());
        }

        let pipeline = self.build_pipeline(
            color_format,
            blit_fs::load(self.device.clone())?
                .entry_point("main")
                .ok_or("missing post-process-copy.frag entry point")?,
        )?;
        self.blit_pipelines.insert(color_format, pipeline.clone());
        Ok(pipeline)
    }

    fn blur_pipeline(
        &mut self,
        color_format: Format,
    ) -> Result<Arc<GraphicsPipeline>, Box<dyn std::error::Error>> {
        if let Some(existing) = self.blur_pipelines.get(&color_format) {
            return Ok(existing.clone());
        }

        let pipeline = self.build_pipeline(
            color_format,
            blur_fs::load(self.device.clone())?
                .entry_point("main")
                .ok_or("missing post-process-bloom-blur.frag entry point")?,
        )?;
        self.blur_pipelines.insert(color_format, pipeline.clone());
        Ok(pipeline)
    }

    fn composite_pipeline(
        &mut self,
        color_format: Format,
    ) -> Result<Arc<GraphicsPipeline>, Box<dyn std::error::Error>> {
        if let Some(existing) = self.composite_pipelines.get(&color_format) {
            return Ok(existing.clone());
        }

        let pipeline = self.build_pipeline(
            color_format,
            composite_fs::load(self.device.clone())?
                .entry_point("main")
                .ok_or("missing post-process-bloom-composite.frag entry point")?,
        )?;
        self.composite_pipelines
            .insert(color_format, pipeline.clone());
        Ok(pipeline)
    }

    fn build_pipeline(
        &self,
        color_format: Format,
        fragment_entry: vulkano::shader::EntryPoint,
    ) -> Result<Arc<GraphicsPipeline>, Box<dyn std::error::Error>> {
        let vs = fullscreen_vs::load(self.device.clone())?;
        let layout = PipelineLayout::new(
            self.device.clone(),
            PipelineLayoutCreateInfo {
                set_layouts: vec![self.sampled_layout.clone()],
                push_constant_ranges: vec![PushConstantRange {
                    stages: ShaderStages::FRAGMENT,
                    offset: 0,
                    size: size_of::<PostProcessPushConstants>() as u32,
                }],
                ..Default::default()
            },
        )?;

        let mut ci = vulkano::pipeline::graphics::GraphicsPipelineCreateInfo::layout(layout);
        ci.stages = vec![
            PipelineShaderStageCreateInfo::new(
                vs.entry_point("main")
                    .ok_or("missing post-process-fullscreen.vert entry point")?,
            ),
            PipelineShaderStageCreateInfo::new(fragment_entry),
        ]
        .into();
        ci.vertex_input_state = Some(VertexInputState::default());
        ci.input_assembly_state = Some(InputAssemblyState::default());
        ci.viewport_state = Some(ViewportState::default());
        ci.rasterization_state = Some(RasterizationState::default());
        ci.multisample_state = Some(MultisampleState::default());
        ci.color_blend_state = Some(ColorBlendState::with_attachment_states(
            1,
            ColorBlendAttachmentState::default(),
        ));
        ci.dynamic_state = [DynamicState::Viewport, DynamicState::Scissor]
            .into_iter()
            .collect();

        let mut rendering = PipelineRenderingCreateInfo::default();
        rendering.color_attachment_formats = vec![Some(color_format)];
        ci.subpass = Some(PipelineSubpassType::BeginRendering(rendering));

        Ok(GraphicsPipeline::new(self.device.clone(), None, ci)?)
    }
}
