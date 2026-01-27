use std::sync::Arc;

use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::{
    allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
    CopyBufferToImageInfo,
};
use vulkano::command_buffer::PrimaryCommandBufferAbstract;
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::sync::GpuFuture;
use vulkano_util::context::VulkanoContext;

pub(crate) fn upload_texture_rgba8(
    context: &VulkanoContext,
    command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<Arc<ImageView>, Box<dyn std::error::Error>> {
    if width == 0 || height == 0 {
        return Err("texture has zero size".into());
    }

    let expected_len = width as usize * height as usize * 4;
    if rgba.len() != expected_len {
        return Err(format!(
            "texture rgba length mismatch: got={}, expected={}",
            rgba.len(),
            expected_len
        )
        .into());
    }

    let memory_allocator = context.memory_allocator().clone();
    let queue = context.graphics_queue().clone();

    let staging = Buffer::from_iter(
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
        rgba.iter().copied(),
    )?;

    let image = Image::new(
        memory_allocator,
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format: Format::R8G8B8A8_UNORM,
            extent: [width, height, 1],
            usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
            ..Default::default()
        },
    )?;

    let mut cbb = AutoCommandBufferBuilder::primary(
        command_buffer_allocator.clone(),
        queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;

    cbb.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(staging, image.clone()))?;

    let cb = cbb.build()?;

    cb.execute(queue.clone())?
        .then_signal_fence_and_flush()?
        .wait(None)?;

    let view = ImageView::new_default(image)
        .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;

    Ok(view)
}

pub(crate) fn upload_texture_bc7(
    context: &VulkanoContext,
    command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
    bc7_blocks: &[u8],
    width: u32,
    height: u32,
    srgb: bool,
) -> Result<Arc<ImageView>, Box<dyn std::error::Error>> {
    if width == 0 || height == 0 {
        return Err("texture has zero size".into());
    }

    let blocks_w = (width + 3) / 4;
    let blocks_h = (height + 3) / 4;
    let expected_len = blocks_w as usize * blocks_h as usize * 16;
    if bc7_blocks.len() != expected_len {
        return Err(format!(
            "texture bc7 length mismatch: got={}, expected={}",
            bc7_blocks.len(),
            expected_len
        )
        .into());
    }

    let memory_allocator = context.memory_allocator().clone();
    let queue = context.graphics_queue().clone();

    let staging = Buffer::from_iter(
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
        bc7_blocks.iter().copied(),
    )?;

    let format = if srgb {
        Format::BC7_SRGB_BLOCK
    } else {
        Format::BC7_UNORM_BLOCK
    };

    let image = Image::new(
        memory_allocator,
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format,
            extent: [width, height, 1],
            usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
            ..Default::default()
        },
    )?;

    let mut cbb = AutoCommandBufferBuilder::primary(
        command_buffer_allocator.clone(),
        queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;

    cbb.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(staging, image.clone()))?;

    let cb = cbb.build()?;

    cb.execute(queue.clone())?
        .then_signal_fence_and_flush()?
        .wait(None)?;

    let view = ImageView::new_default(image)
        .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;

    Ok(view)
}
