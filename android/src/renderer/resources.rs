//! Owned Vulkan allocations. No engine, material policy, or frame scheduling.
use crate::{
    graphics_device::Gpu,
    xr_host::{ApiResult, Result},
};
use ash::vk;
use std::{ptr::NonNull, sync::Arc};

fn memory_type(
    gpu: &Gpu,
    bits: u32,
    required: vk::MemoryPropertyFlags,
    preferred: vk::MemoryPropertyFlags,
) -> Result<(u32, vk::MemoryPropertyFlags)> {
    let memory = unsafe {
        gpu.instance
            .raw
            .get_physical_device_memory_properties(gpu.physical)
    };
    (0..memory.memory_type_count)
        .filter_map(|index| {
            let flags = memory.memory_types[index as usize].property_flags;
            (bits & (1 << index) != 0 && flags.contains(required)).then_some((index, flags))
        })
        .max_by_key(|(_, flags)| (flags.as_raw() & preferred.as_raw()).count_ones())
        .ok_or_else(|| format!("no compatible memory type: required={required:?} bits={bits:#x}"))
}

pub(super) struct Buffer {
    gpu: Arc<Gpu>,
    pub handle: vk::Buffer,
    memory: vk::DeviceMemory,
    mapped: Option<NonNull<u8>>,
    pub capacity: usize,
    coherent: bool,
}
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            if self.mapped.is_some() {
                self.gpu.device.unmap_memory(self.memory);
            }
            self.gpu.device.destroy_buffer(self.handle, None);
            self.gpu.device.free_memory(self.memory, None);
        }
    }
}
impl Buffer {
    pub fn new(gpu: Arc<Gpu>, capacity: usize, usage: vk::BufferUsageFlags) -> Result<Self> {
        let mut buffer = Self {
            gpu,
            handle: vk::Buffer::null(),
            memory: vk::DeviceMemory::null(),
            mapped: None,
            capacity,
            coherent: false,
        };
        unsafe {
            buffer.handle = buffer
                .gpu
                .device
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(capacity.max(1) as u64)
                        .usage(usage),
                    None,
                )
                .api("vkCreateBuffer(scene)")?;
            let requirements = buffer
                .gpu
                .device
                .get_buffer_memory_requirements(buffer.handle);
            let (index, flags) = memory_type(
                &buffer.gpu,
                requirements.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE,
                vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;
            buffer.coherent = flags.contains(vk::MemoryPropertyFlags::HOST_COHERENT);
            buffer.memory = buffer
                .gpu
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(requirements.size)
                        .memory_type_index(index),
                    None,
                )
                .api("vkAllocateMemory(scene buffer)")?;
            buffer
                .gpu
                .device
                .bind_buffer_memory(buffer.handle, buffer.memory, 0)
                .api("vkBindBufferMemory(scene)")?;
            buffer.mapped = NonNull::new(
                buffer
                    .gpu
                    .device
                    .map_memory(
                        buffer.memory,
                        0,
                        vk::WHOLE_SIZE,
                        vk::MemoryMapFlags::empty(),
                    )
                    .api("vkMapMemory(scene)")?
                    .cast(),
            );
            if buffer.mapped.is_none() {
                return Err("vkMapMemory returned null".into());
            }
        }
        Ok(buffer)
    }
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > self.capacity {
            return Err("mapped scene buffer capacity exceeded".into());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.mapped.unwrap().as_ptr(),
                bytes.len(),
            );
            if !self.coherent {
                self.gpu
                    .device
                    .flush_mapped_memory_ranges(&[vk::MappedMemoryRange::default()
                        .memory(self.memory)
                        .size(vk::WHOLE_SIZE)])
                    .api("vkFlushMappedMemoryRanges(scene)")?;
            }
        }
        Ok(())
    }
}

/// Only use with fully initialized, padding-free C-layout numeric data.
pub(super) unsafe fn bytes<T>(values: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), std::mem::size_of_val(values)) }
}

pub(super) struct Image {
    gpu: Arc<Gpu>,
    pub handle: vk::Image,
    pub view: vk::ImageView,
    memory: vk::DeviceMemory,
    pub lazy: bool,
}
impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            self.gpu.device.destroy_image_view(self.view, None);
            self.gpu.device.destroy_image(self.handle, None);
            self.gpu.device.free_memory(self.memory, None);
        }
    }
}
impl Image {
    pub fn new(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        usage: vk::ImageUsageFlags,
        aspect: vk::ImageAspectFlags,
        samples: vk::SampleCountFlags,
    ) -> Result<Self> {
        let mut image = Self {
            gpu,
            handle: vk::Image::null(),
            view: vk::ImageView::null(),
            memory: vk::DeviceMemory::null(),
            lazy: false,
        };
        unsafe {
            image.handle = image
                .gpu
                .device
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(format)
                        .extent(vk::Extent3D {
                            width: extent.width,
                            height: extent.height,
                            depth: 1,
                        })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(samples)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(usage),
                    None,
                )
                .api("vkCreateImage(scene)")?;
            let requirements = image.gpu.device.get_image_memory_requirements(image.handle);
            let preferred = if usage.contains(vk::ImageUsageFlags::TRANSIENT_ATTACHMENT) {
                vk::MemoryPropertyFlags::LAZILY_ALLOCATED
            } else {
                vk::MemoryPropertyFlags::empty()
            };
            let (index, flags) = memory_type(
                &image.gpu,
                requirements.memory_type_bits,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                preferred,
            )?;
            image.lazy = flags.contains(vk::MemoryPropertyFlags::LAZILY_ALLOCATED);
            image.memory = image
                .gpu
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(requirements.size)
                        .memory_type_index(index),
                    None,
                )
                .api("vkAllocateMemory(scene image)")?;
            image
                .gpu
                .device
                .bind_image_memory(image.handle, image.memory, 0)
                .api("vkBindImageMemory(scene)")?;
            image.view = image
                .gpu
                .device
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image.handle)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(aspect)
                                .level_count(1)
                                .layer_count(1),
                        ),
                    None,
                )
                .api("vkCreateImageView(scene)")?;
        }
        Ok(image)
    }
}

/// A one-time transfer used only when creating textures, never during drawing.
pub(super) fn upload_rgba(
    gpu: Arc<Gpu>,
    rgba: &[u8],
    width: u32,
    height: u32,
    srgb: bool,
) -> Result<Image> {
    if width == 0
        || height == 0
        || width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(4))
            .map(|n| n as usize)
            != Some(rgba.len())
    {
        return Err("invalid RGBA8 texture dimensions/data".into());
    }
    let mut staging = Buffer::new(gpu.clone(), rgba.len(), vk::BufferUsageFlags::TRANSFER_SRC)?;
    staging.write(rgba)?;
    let image = Image::new(
        gpu.clone(),
        if srgb {
            vk::Format::R8G8B8A8_SRGB
        } else {
            vk::Format::R8G8B8A8_UNORM
        },
        vk::Extent2D { width, height },
        vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
        vk::ImageAspectFlags::COLOR,
        vk::SampleCountFlags::TYPE_1,
    )?;
    let transfer = Transfer::new(gpu.clone())?;
    unsafe {
        let device = &gpu.device;
        device
            .begin_command_buffer(
                transfer.command,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .api("vkBeginCommandBuffer(texture)")?;
        let barrier = vk::ImageMemoryBarrier::default()
            .image(image.handle)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1),
            );
        device.cmd_pipeline_barrier(
            transfer.command,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
        device.cmd_copy_buffer_to_image(
            transfer.command,
            staging.handle,
            image.handle,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[vk::BufferImageCopy::default()
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1),
                )
                .image_extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })],
        );
        device.cmd_pipeline_barrier(
            transfer.command,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)],
        );
        device
            .end_command_buffer(transfer.command)
            .api("vkEndCommandBuffer(texture)")?;
        device
            .queue_submit(
                gpu.queue,
                &[vk::SubmitInfo::default().command_buffers(&[transfer.command])],
                transfer.fence,
            )
            .api("vkQueueSubmit(texture)")?;
        device
            .wait_for_fences(&[transfer.fence], true, u64::MAX)
            .api("vkWaitForFences(texture)")?;
    }
    Ok(image)
}

struct Transfer {
    gpu: Arc<Gpu>,
    pool: vk::CommandPool,
    command: vk::CommandBuffer,
    fence: vk::Fence,
}
impl Transfer {
    fn new(gpu: Arc<Gpu>) -> Result<Self> {
        let mut transfer = Self {
            gpu,
            pool: vk::CommandPool::null(),
            command: vk::CommandBuffer::null(),
            fence: vk::Fence::null(),
        };
        unsafe {
            transfer.pool = transfer
                .gpu
                .device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default().queue_family_index(transfer.gpu.family),
                    None,
                )
                .api("vkCreateCommandPool(texture)")?;
            transfer.command = transfer
                .gpu
                .device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(transfer.pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .api("vkAllocateCommandBuffers(texture)")?[0];
            transfer.fence = transfer
                .gpu
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .api("vkCreateFence(texture)")?;
        }
        Ok(transfer)
    }
}
impl Drop for Transfer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.gpu.device.device_wait_idle();
            self.gpu.device.destroy_fence(self.fence, None);
            self.gpu.device.destroy_command_pool(self.pool, None);
        }
    }
}
