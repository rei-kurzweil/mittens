use std::sync::Arc;

use super::renderer_context::RendererContext;
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::PrimaryCommandBufferAbstract;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, BufferImageCopy, CommandBufferUsage, CopyBufferToImageInfo,
    allocator::StandardCommandBufferAllocator,
};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::sync::GpuFuture;

pub(crate) fn upload_texture_rgba8(
    context: &RendererContext,
    command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<Arc<ImageView>, Box<dyn std::error::Error>> {
    let (mip_data, levels) = super::mipmaps::generate(rgba, width, height, false)?;
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
        mip_data.into_iter(),
    )?;

    let image = Image::new(
        memory_allocator,
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format: Format::R8G8B8A8_UNORM,
            extent: [width, height, 1],
            mip_levels: levels.len() as u32,
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

    let mut copy = CopyBufferToImageInfo::buffer_image(staging, image.clone());
    copy.regions = levels
        .iter()
        .enumerate()
        .map(|(index, level)| BufferImageCopy {
            buffer_offset: level.offset as u64,
            image_subresource: vulkano::image::ImageSubresourceLayers {
                aspects: vulkano::image::ImageAspects::COLOR,
                mip_level: index as u32,
                array_layers: 0..1,
            },
            image_extent: [level.width, level.height, 1],
            ..Default::default()
        })
        .collect();
    cbb.copy_buffer_to_image(copy)?;

    let cb = cbb.build()?;

    cb.execute(queue.clone())?
        .then_signal_fence_and_flush()?
        .wait(None)?;

    let view = ImageView::new_default(image)
        .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;

    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a Vulkan GPU"]
    fn rgba_upload_creates_a_view_over_the_complete_mip_chain()
    -> Result<(), Box<dyn std::error::Error>> {
        let context = RendererContext::new(None, None, None)?;
        let allocator = Arc::new(StandardCommandBufferAllocator::new(
            context.device().clone(),
            Default::default(),
        ));
        let view = upload_texture_rgba8(&context, &allocator, &[255; 4 * 8 * 4], 4, 8)?;
        assert_eq!(view.image().mip_levels(), 4);
        assert_eq!(view.subresource_range().mip_levels, 0..4);
        Ok(())
    }
}

pub(crate) fn upload_texture_bc7(
    context: &RendererContext,
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
