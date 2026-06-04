use ash::vk;

use crate::engine::graphics::VulkanoRenderer;
use crate::engine::graphics::XRSwapchain;

pub fn clear_xr_swapchain_image(
    vk_device: &ash::Device,
    vk_queue: vk::Queue,
    vk_command_buffer: vk::CommandBuffer,
    view_count: u32,
    image: vk::Image,
    rgba: [f32; 4],
    was_initialized: bool,
) -> Result<(), vk::Result> {
    let clear = vk::ClearColorValue { float32: rgba };

    let old_layout = if was_initialized {
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
    } else {
        vk::ImageLayout::UNDEFINED
    };

    let src_stage = if was_initialized {
        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
    } else {
        vk::PipelineStageFlags::TOP_OF_PIPE
    };

    let src_access = if was_initialized {
        vk::AccessFlags::COLOR_ATTACHMENT_WRITE
    } else {
        vk::AccessFlags::empty()
    };

    unsafe {
        vk_device.reset_command_buffer(vk_command_buffer, vk::CommandBufferResetFlags::empty())?;

        vk_device.begin_command_buffer(
            vk_command_buffer,
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )?;

        let range = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(view_count);

        // Transition UNDEFINED/COLOR_ATTACHMENT_OPTIMAL -> TRANSFER_DST_OPTIMAL.
        let barrier_to_transfer = vk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_access_mask(src_access)
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .image(image)
            .subresource_range(range);

        vk_device.cmd_pipeline_barrier(
            vk_command_buffer,
            src_stage,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier_to_transfer],
        );

        vk_device.cmd_clear_color_image(
            vk_command_buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &clear,
            &[range],
        );

        // Transition TRANSFER_DST_OPTIMAL -> COLOR_ATTACHMENT_OPTIMAL.
        let barrier_to_color = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .image(image)
            .subresource_range(range);

        vk_device.cmd_pipeline_barrier(
            vk_command_buffer,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier_to_color],
        );

        vk_device.end_command_buffer(vk_command_buffer)?;

        let command_buffers = [vk_command_buffer];
        let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffers);
        vk_device.queue_submit(vk_queue, &[submit_info], vk::Fence::null())?;
        vk_device.queue_wait_idle(vk_queue)?;
    }

    Ok(())
}

pub fn copy_offscreen_to_xr_layers(
    vk_device: &ash::Device,
    vk_queue: vk::Queue,
    vk_command_buffer: vk::CommandBuffer,
    xr_swapchain: &XRSwapchain,
    renderer: &VulkanoRenderer,
    dst_image: vk::Image,
    dst_was_initialized: bool,
    view_count: usize,
) -> Result<(), vk::Result> {
    let dst_old_layout = if dst_was_initialized {
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
    } else {
        vk::ImageLayout::UNDEFINED
    };

    let dst_src_access = if dst_was_initialized {
        vk::AccessFlags::COLOR_ATTACHMENT_WRITE
    } else {
        vk::AccessFlags::empty()
    };

    unsafe {
        vk_device.reset_command_buffer(vk_command_buffer, vk::CommandBufferResetFlags::empty())?;

        vk_device.begin_command_buffer(
            vk_command_buffer,
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )?;

        for eye in 0..view_count {
            let Some(src_image) = renderer.xr_offscreen_vk_image(eye) else {
                continue;
            };

            let src_range = vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1);

            let dst_range = vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(eye as u32)
                .layer_count(1);

            // src: COLOR_ATTACHMENT_OPTIMAL -> TRANSFER_SRC_OPTIMAL
            let barrier_src_to_copy = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .image(src_image)
                .subresource_range(src_range);

            // dst: UNDEFINED/COLOR_ATTACHMENT_OPTIMAL -> TRANSFER_DST_OPTIMAL
            let barrier_dst_to_copy = vk::ImageMemoryBarrier::default()
                .old_layout(dst_old_layout)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_access_mask(dst_src_access)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .image(dst_image)
                .subresource_range(dst_range);

            vk_device.cmd_pipeline_barrier(
                vk_command_buffer,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_src_to_copy, barrier_dst_to_copy],
            );

            let extent = xr_swapchain.extent();
            let region = vk::ImageCopy::default()
                .src_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .mip_level(0)
                        .base_array_layer(0)
                        .layer_count(1),
                )
                .dst_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .mip_level(0)
                        .base_array_layer(eye as u32)
                        .layer_count(1),
                )
                .extent(vk::Extent3D {
                    width: extent.width as u32,
                    height: extent.height as u32,
                    depth: 1,
                });

            vk_device.cmd_copy_image(
                vk_command_buffer,
                src_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                dst_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );

            // src: TRANSFER_SRC_OPTIMAL -> COLOR_ATTACHMENT_OPTIMAL
            let barrier_src_back = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .image(src_image)
                .subresource_range(src_range);

            // dst: TRANSFER_DST_OPTIMAL -> COLOR_ATTACHMENT_OPTIMAL
            let barrier_dst_back = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .image(dst_image)
                .subresource_range(dst_range);

            vk_device.cmd_pipeline_barrier(
                vk_command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_src_back, barrier_dst_back],
            );
        }

        vk_device.end_command_buffer(vk_command_buffer)?;

        let command_buffers = [vk_command_buffer];
        let submit_info = vk::SubmitInfo::default().command_buffers(&command_buffers);
        vk_device.queue_submit(vk_queue, &[submit_info], vk::Fence::null())?;
        vk_device.queue_wait_idle(vk_queue)?;
    }

    Ok(())
}
