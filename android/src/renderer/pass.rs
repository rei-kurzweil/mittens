//! One opaque pass per eye. Only the runtime color image survives the pass.
use super::resources::Image;
use crate::{
    graphics_device::Gpu,
    xr_host::{ApiResult, Result},
};
use ash::{vk, vk::Handle};
use std::sync::Arc;

pub(super) struct EyePass {
    gpu: Arc<Gpu>,
    pub handle: vk::RenderPass,
    pub extent: vk::Extent2D,
    pub depth_format: vk::Format,
    pub samples: vk::SampleCountFlags,
    depth: Vec<Image>,
    color_msaa: Vec<Image>,
    views: Vec<[vk::ImageView; 2]>,
    framebuffers: Vec<[vk::Framebuffer; 2]>,
}
impl Drop for EyePass {
    fn drop(&mut self) {
        unsafe {
            for pair in &self.framebuffers {
                for &fb in pair {
                    self.gpu.device.destroy_framebuffer(fb, None);
                }
            }
            for pair in &self.views {
                for &view in pair {
                    self.gpu.device.destroy_image_view(view, None);
                }
            }
            self.gpu.device.destroy_render_pass(self.handle, None);
        }
    }
}
impl EyePass {
    pub fn new(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        images: &[u64],
        samples: vk::SampleCountFlags,
    ) -> Result<Self> {
        if images.is_empty() || extent.width == 0 || extent.height == 0 {
            return Err("empty eye target".into());
        }
        if !matches!(
            samples,
            vk::SampleCountFlags::TYPE_1 | vk::SampleCountFlags::TYPE_4
        ) {
            return Err("opaque eye pass supports 1x or 4x MSAA".into());
        }
        let multisampled = samples != vk::SampleCountFlags::TYPE_1;
        let supports = |format, usage| unsafe {
            gpu.instance
                .raw
                .get_physical_device_image_format_properties(
                    gpu.physical,
                    format,
                    vk::ImageType::TYPE_2D,
                    vk::ImageTiling::OPTIMAL,
                    usage,
                    vk::ImageCreateFlags::empty(),
                )
                .is_ok_and(|p| {
                    p.sample_counts.contains(samples)
                        && p.max_extent.width >= extent.width
                        && p.max_extent.height >= extent.height
                })
        };
        let color_usage =
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT;
        let depth_usage = vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
            | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT;
        if multisampled && !supports(format, color_usage) {
            return Err(format!(
                "requested {samples:?} transient color is unsupported for {format:?}"
            ));
        }
        let depth_format = [vk::Format::D16_UNORM, vk::Format::D32_SFLOAT]
            .into_iter()
            .find(|&format| supports(format, depth_usage))
            .ok_or("no depth format supports the requested sample count and transient usage")?;
        let mut pass = Self {
            gpu,
            handle: vk::RenderPass::null(),
            extent,
            depth_format,
            samples,
            depth: Vec::new(),
            color_msaa: Vec::new(),
            views: Vec::new(),
            framebuffers: Vec::new(),
        };
        unsafe {
            let device = &pass.gpu.device;
            let mut attachments = vec![
                vk::AttachmentDescription::default()
                    .format(format)
                    .samples(samples)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(if multisampled {
                        vk::AttachmentStoreOp::DONT_CARE
                    } else {
                        vk::AttachmentStoreOp::STORE
                    })
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(if multisampled {
                        vk::ImageLayout::UNDEFINED
                    } else {
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
                    })
                    .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
                vk::AttachmentDescription::default()
                    .format(depth_format)
                    .samples(samples)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
            ];
            if multisampled {
                attachments.push(
                    vk::AttachmentDescription::default()
                        .format(format)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .load_op(vk::AttachmentLoadOp::DONT_CARE)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                        .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
                );
            }
            let colors = [vk::AttachmentReference {
                attachment: 0,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            }];
            let depth = vk::AttachmentReference {
                attachment: 1,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            };
            let resolves = [vk::AttachmentReference {
                attachment: 2,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            }];
            let mut subpass = vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&colors)
                .depth_stencil_attachment(&depth);
            if multisampled {
                subpass = subpass.resolve_attachments(&resolves);
            }
            let subpasses = [subpass];
            // Synchronize the runtime's prior use and the previous frame's depth
            // writes. BY_REGION permits attachment work to stay tile-local.
            let dependencies = [vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::ALL_COMMANDS)
                .dst_stage_mask(
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                )
                .src_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
                .dst_access_mask(
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                        | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                )
                .dependency_flags(vk::DependencyFlags::BY_REGION)];
            pass.handle = device
                .create_render_pass(
                    &vk::RenderPassCreateInfo::default()
                        .attachments(&attachments)
                        .subpasses(&subpasses)
                        .dependencies(&dependencies),
                    None,
                )
                .api("vkCreateRenderPass(opaque eyes)")?;
            for _ in 0..2 {
                pass.depth.push(Image::new(
                    pass.gpu.clone(),
                    depth_format,
                    extent,
                    depth_usage,
                    vk::ImageAspectFlags::DEPTH,
                    samples,
                )?);
                if multisampled {
                    pass.color_msaa.push(Image::new(
                        pass.gpu.clone(),
                        format,
                        extent,
                        color_usage,
                        vk::ImageAspectFlags::COLOR,
                        samples,
                    )?);
                }
            }
            for &image in images {
                pass.views.push([vk::ImageView::null(); 2]);
                pass.framebuffers.push([vk::Framebuffer::null(); 2]);
                let index = pass.views.len() - 1;
                for eye in 0..2 {
                    pass.views[index][eye] = device
                        .create_image_view(
                            &vk::ImageViewCreateInfo::default()
                                .image(vk::Image::from_raw(image))
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
                        .api("vkCreateImageView(scene eye)")?;
                    let attachments = if multisampled {
                        vec![
                            pass.color_msaa[eye].view,
                            pass.depth[eye].view,
                            pass.views[index][eye],
                        ]
                    } else {
                        vec![pass.views[index][eye], pass.depth[eye].view]
                    };
                    pass.framebuffers[index][eye] = device
                        .create_framebuffer(
                            &vk::FramebufferCreateInfo::default()
                                .render_pass(pass.handle)
                                .attachments(&attachments)
                                .width(extent.width)
                                .height(extent.height)
                                .layers(1),
                            None,
                        )
                        .api("vkCreateFramebuffer(scene eye)")?;
                }
            }
        }
        Ok(pass)
    }
    pub fn lazy_depth(&self) -> bool {
        self.depth.iter().all(|image| image.lazy)
    }
    pub fn lazy_msaa_color(&self) -> bool {
        !self.color_msaa.is_empty() && self.color_msaa.iter().all(|image| image.lazy)
    }
    pub fn target_count(&self) -> usize {
        self.framebuffers.len()
    }
    pub unsafe fn begin(
        &self,
        command: vk::CommandBuffer,
        index: usize,
        eye: usize,
        color: [f32; 4],
    ) {
        let area = vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: self.extent,
        };
        let clears = [
            vk::ClearValue {
                color: vk::ClearColorValue { float32: color },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            },
        ];
        unsafe {
            self.gpu.device.cmd_begin_render_pass(
                command,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.handle)
                    .framebuffer(self.framebuffers[index][eye])
                    .render_area(area)
                    .clear_values(&clears),
                vk::SubpassContents::INLINE,
            );
            self.gpu.device.cmd_set_viewport(
                command,
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
            self.gpu.device.cmd_set_scissor(command, 0, &[area]);
        }
    }
}
