use super::gpu::Gpu;
use crate::xr_probe::{ApiResult, Result};
use ash::{vk, vk::Handle};
use std::{io::Cursor, sync::Arc};

pub(super) struct Renderer {
    gpu: Arc<Gpu>,
    extent: vk::Extent2D,
    images: Vec<vk::Image>,
    views: Vec<[vk::ImageView; 2]>,
    framebuffers: Vec<[vk::Framebuffer; 2]>,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    shaders: Vec<vk::ShaderModule>,
    pool: vk::CommandPool,
    command: vk::CommandBuffer,
    fence: vk::Fence,
}
impl Drop for Renderer {
    fn drop(&mut self) {
        // Covers failure paths too: never destroy resources used by a pending
        // submission, including when a fence wait returned an error.
        unsafe {
            if let Err(error) = self.gpu.device.device_wait_idle() {
                log::error!("probe renderer teardown: {error:?}");
            }
            let device = &self.gpu.device;
            device.destroy_fence(self.fence, None);
            device.destroy_command_pool(self.pool, None);
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
            for shader in self.shaders.drain(..) {
                device.destroy_shader_module(shader, None);
            }
            for eyes in &self.framebuffers {
                for framebuffer in eyes {
                    device.destroy_framebuffer(*framebuffer, None);
                }
            }
            for eyes in &self.views {
                for view in eyes {
                    device.destroy_image_view(*view, None);
                }
            }
            device.destroy_render_pass(self.pass, None);
            // Images/memory belong to OpenXR. We never destroy or free them.
        }
    }
}

impl Renderer {
    pub(super) fn new(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        images: Vec<u64>,
    ) -> Result<Self> {
        let mut renderer = Self {
            gpu,
            extent,
            images: images.into_iter().map(vk::Image::from_raw).collect(),
            views: Vec::new(),
            framebuffers: Vec::new(),
            pass: vk::RenderPass::null(),
            layout: vk::PipelineLayout::null(),
            pipeline: vk::Pipeline::null(),
            shaders: Vec::new(),
            pool: vk::CommandPool::null(),
            command: vk::CommandBuffer::null(),
            fence: vk::Fence::null(),
        };
        unsafe {
            let device = &renderer.gpu.device;
            if !renderer.gpu.dynamic {
                let attachments = [vk::AttachmentDescription::default()
                    .format(format)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
                let color = [vk::AttachmentReference {
                    attachment: 0,
                    layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                }];
                let subpasses = [vk::SubpassDescription::default()
                    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                    .color_attachments(&color)];
                renderer.pass = device
                    .create_render_pass(
                        &vk::RenderPassCreateInfo::default()
                            .attachments(&attachments)
                            .subpasses(&subpasses),
                        None,
                    )
                    .api("vkCreateRenderPass")?;
            }
            for image in &renderer.images {
                // Store handles immediately so partial construction also cleans up.
                renderer.views.push([vk::ImageView::null(); 2]);
                renderer.framebuffers.push([vk::Framebuffer::null(); 2]);
                let index = renderer.views.len() - 1;
                for eye in 0..2 {
                    let view = device
                        .create_image_view(
                            &vk::ImageViewCreateInfo::default()
                                .image(*image)
                                .view_type(vk::ImageViewType::TYPE_2D)
                                .format(format)
                                .subresource_range(
                                    vk::ImageSubresourceRange::default()
                                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                                        .level_count(1)
                                        .base_array_layer(eye as u32)
                                        .layer_count(1),
                                ),
                            None,
                        )
                        .api("vkCreateImageView(XR eye)")?;
                    renderer.views[index][eye] = view;
                    if !renderer.gpu.dynamic {
                        renderer.framebuffers[index][eye] = device
                            .create_framebuffer(
                                &vk::FramebufferCreateInfo::default()
                                    .render_pass(renderer.pass)
                                    .attachments(&[view])
                                    .width(extent.width)
                                    .height(extent.height)
                                    .layers(1),
                                None,
                            )
                            .api("vkCreateFramebuffer(XR eye)")?;
                    }
                }
            }
            renderer.pool = device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                        .queue_family_index(renderer.gpu.family),
                    None,
                )
                .api("vkCreateCommandPool")?;
            renderer.command = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(renderer.pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .api("vkAllocateCommandBuffers")?[0];
            renderer.fence = device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .api("vkCreateFence")?;
            renderer.layout = device
                .create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default(), None)
                .api("vkCreatePipelineLayout")?;
            for code in [
                include_bytes!(concat!(env!("OUT_DIR"), "/triangle.vert.spv")).as_slice(),
                include_bytes!(concat!(env!("OUT_DIR"), "/triangle.frag.spv")).as_slice(),
            ] {
                let words =
                    ash::util::read_spv(&mut Cursor::new(code)).api("read triangle SPIR-V")?;
                renderer.shaders.push(
                    device
                        .create_shader_module(
                            &vk::ShaderModuleCreateInfo::default().code(&words),
                            None,
                        )
                        .api("vkCreateShaderModule")?,
                );
            }
            let stages = [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(renderer.shaders[0])
                    .name(c"main"),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(renderer.shaders[1])
                    .name(c"main"),
            ];
            let vertex = vk::PipelineVertexInputStateCreateInfo::default();
            let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let viewport = vk::PipelineViewportStateCreateInfo::default()
                .viewport_count(1)
                .scissor_count(1);
            let raster = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL)
                .cull_mode(vk::CullModeFlags::NONE)
                .line_width(1.0);
            let samples = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let blend_attachments = [vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA)];
            let blend =
                vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachments);
            let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic =
                vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
            let formats = [format];
            let mut rendering =
                vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&formats);
            let mut pipeline = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&vertex)
                .input_assembly_state(&assembly)
                .viewport_state(&viewport)
                .rasterization_state(&raster)
                .multisample_state(&samples)
                .color_blend_state(&blend)
                .dynamic_state(&dynamic)
                .layout(renderer.layout)
                .render_pass(renderer.pass);
            if renderer.gpu.dynamic {
                pipeline = pipeline.push_next(&mut rendering);
            }
            renderer.pipeline = match device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[pipeline],
                None,
            ) {
                Ok(pipelines) => pipelines[0],
                Err((partial, error)) => {
                    for handle in partial {
                        device.destroy_pipeline(handle, None);
                    }
                    return Err(format!("vkCreateGraphicsPipelines: {error:?}"));
                }
            };
            for shader in renderer.shaders.drain(..) {
                device.destroy_shader_module(shader, None);
            }
        }
        Ok(renderer)
    }

    pub(super) fn draw(&mut self, index: usize, triangle: bool) -> Result<()> {
        unsafe {
            let device = &self.gpu.device;
            device
                .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
                .api("vkResetCommandPool")?;
            device
                .begin_command_buffer(
                    self.command,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .api("vkBeginCommandBuffer")?;
            let barrier = vk::ImageMemoryBarrier::default()
                .image(self.images[index])
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .src_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(2),
                );
            device.cmd_pipeline_barrier(
                self.command,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
            let area = vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: self.extent,
            };
            for eye in 0..2 {
                // Distinct, dim clears make eye order visible without a bright flash.
                let color = if eye == 0 {
                    [0.07, 0.015, 0.025, 1.0]
                } else {
                    [0.015, 0.03, 0.09, 1.0]
                };
                let clear = vk::ClearValue {
                    color: vk::ClearColorValue { float32: color },
                };
                if let Some(dynamic) = &self.gpu.dynamic_api {
                    let attachment = [vk::RenderingAttachmentInfo::default()
                        .image_view(self.views[index][eye])
                        .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(clear)];
                    dynamic.cmd_begin_rendering(
                        self.command,
                        &vk::RenderingInfo::default()
                            .render_area(area)
                            .layer_count(1)
                            .color_attachments(&attachment),
                    );
                } else {
                    device.cmd_begin_render_pass(
                        self.command,
                        &vk::RenderPassBeginInfo::default()
                            .render_pass(self.pass)
                            .framebuffer(self.framebuffers[index][eye])
                            .render_area(area)
                            .clear_values(&[clear]),
                        vk::SubpassContents::INLINE,
                    );
                }
                if triangle {
                    device.cmd_set_viewport(
                        self.command,
                        0,
                        &[vk::Viewport {
                            x: 0.0,
                            y: 0.0,
                            width: self.extent.width as f32,
                            height: self.extent.height as f32,
                            min_depth: 0.0,
                            max_depth: 1.0,
                        }],
                    );
                    device.cmd_set_scissor(self.command, 0, &[area]);
                    device.cmd_bind_pipeline(
                        self.command,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipeline,
                    );
                    device.cmd_draw(self.command, 3, 1, 0, 0);
                }
                if let Some(dynamic) = &self.gpu.dynamic_api {
                    dynamic.cmd_end_rendering(self.command);
                } else {
                    device.cmd_end_render_pass(self.command);
                }
            }
            device
                .end_command_buffer(self.command)
                .api("vkEndCommandBuffer")?;
            device.reset_fences(&[self.fence]).api("vkResetFences")?;
            device
                .queue_submit(
                    self.gpu.queue,
                    &[vk::SubmitInfo::default().command_buffers(&[self.command])],
                    self.fence,
                )
                .api("vkQueueSubmit")?;
            // The prototype deliberately completes GPU work before releasing the
            // runtime-owned image; it is a correctness probe, not a paced engine.
            device
                .wait_for_fences(&[self.fence], true, u64::MAX)
                .api("vkWaitForFences(before xrReleaseSwapchainImage)")?;
        }
        Ok(())
    }
}
