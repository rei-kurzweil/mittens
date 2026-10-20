use ash::vk;
use std::sync::{Arc, Mutex};
use vulkano::device::Device;
use vulkano::image::sys::RawImage;
use vulkano::image::view::{ImageView, ImageViewCreateInfo, ImageViewType};
use vulkano::image::{ImageCreateInfo, ImageSubresourceRange};

/// Cached views into one runtime-owned array image. Views are dropped before
/// the shared swapchain owner, including when a failed batch retains a clone.
pub(crate) struct XrSwapchainImage {
    eye_views: Vec<Arc<ImageView>>,
    _swapchain: Arc<Mutex<openxr::Swapchain<openxr::Vulkan>>>,
    // The runtime may use VkDevice while destroying its swapchain, so retain
    // the device after both the views and the swapchain owner in drop order.
    _device: Arc<Device>,
}

impl XrSwapchainImage {
    /// # Safety
    /// The image must belong to `device` and `swapchain`, have memory bound by
    /// the runtime, and match `create_info`. All accesses must follow OpenXR's
    /// acquire/wait/release protocol and the renderer's completion tracking.
    pub(crate) unsafe fn new(
        device: Arc<Device>,
        handle: vk::Image,
        create_info: ImageCreateInfo,
        swapchain: Arc<Mutex<openxr::Swapchain<openxr::Vulkan>>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // SAFETY: the runtime owns the image and its bound allocation. Borrowed
        // wrappers never destroy either; the retained owner outlives the views.
        let eye_views = unsafe { borrow_xr_image_views(device.clone(), handle, create_info) }?;
        Ok(Self {
            eye_views,
            _swapchain: swapchain,
            _device: device,
        })
    }

    pub(crate) fn eye_view(&self, eye: usize) -> Option<Arc<ImageView>> {
        self.eye_views.get(eye).cloned()
    }

    pub(crate) fn view_count(&self) -> usize {
        self.eye_views.len()
    }
}

/// # Safety
/// `handle` must be an already-bound image from `device` matching `create_info`.
/// Its external owner must outlive every returned view and all submitted uses.
unsafe fn borrow_xr_image_views(
    device: Arc<Device>,
    handle: vk::Image,
    create_info: ImageCreateInfo,
) -> Result<Vec<Arc<ImageView>>, Box<dyn std::error::Error>> {
    let view_count = create_info.array_layers;
    let raw = unsafe { RawImage::from_handle_borrowed(device, handle, create_info) }?;
    let image = Arc::new(unsafe { raw.assume_bound() });
    (0..view_count)
        .map(|eye| {
            ImageView::new(
                image.clone(),
                ImageViewCreateInfo {
                    view_type: ImageViewType::Dim2d,
                    subresource_range: ImageSubresourceRange {
                        array_layers: eye..eye + 1,
                        ..image.subresource_range()
                    },
                    ..ImageViewCreateInfo::from_image(&image)
                },
            )
            .map_err(Into::into)
        })
        .collect()
}

pub fn submit_clear_xr_swapchain_image_and_wait(
    vk_device: &ash::Device,
    vk_queue: vk::Queue,
    vk_command_buffer: vk::CommandBuffer,
    completion_fence: vk::Fence,
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
        vk_device.queue_submit(vk_queue, &[submit_info], completion_fence)?;
        vk_device.wait_for_fences(&[completion_fence], true, u64::MAX)?;
        vk_device.reset_fences(&[completion_fence])?;
    }

    Ok(())
}

/// Establishes completion of the preceding same-queue eye/mirror submissions.
/// No image copy or extra output image is needed. The wait must complete before
/// releasing the acquired image to OpenXR or retiring Vulkano's resource chain.
///
/// # Safety
/// Queue and fence must belong to the live device, the fence must be unsignaled
/// and reusable, and the caller must externally synchronize queue access.
pub unsafe fn submit_xr_completion_and_wait(
    vk_device: &ash::Device,
    vk_queue: vk::Queue,
    completion_fence: vk::Fence,
) -> Result<(), vk::Result> {
    unsafe {
        vk_device.queue_submit(vk_queue, &[vk::SubmitInfo::default()], completion_fence)?;
        vk_device.wait_for_fences(&[completion_fence], true, u64::MAX)?;
        vk_device.reset_fences(&[completion_fence])?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "xr_renderer_tests.rs"]
mod tests;
