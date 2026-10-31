//! Owned two-layer substitute for an OpenXR image, with readback for GPU checks.
use crate::graphics_device::Gpu;
use ash::{vk, vk::Handle};
use std::sync::Arc;

pub struct Target {
    gpu: Arc<Gpu>,
    pub image: vk::Image,
    pub extent: vk::Extent2D,
    memory: vk::DeviceMemory,
    buffer: vk::Buffer,
    buffer_memory: vk::DeviceMemory,
    pool: vk::CommandPool,
    command: vk::CommandBuffer,
}
impl Target {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        Self::with_extent(gpu, 96, 96)
    }
    pub fn with_extent(gpu: Arc<Gpu>, width: u32, height: u32) -> Self {
        unsafe {
            let d = &gpu.device;
            let props = gpu
                .instance
                .raw
                .get_physical_device_memory_properties(gpu.physical);
            let memory_type = |bits: u32, flags: vk::MemoryPropertyFlags| {
                (0..props.memory_type_count)
                    .find(|i| {
                        bits & (1 << i) != 0
                            && props.memory_types[*i as usize]
                                .property_flags
                                .contains(flags)
                    })
                    .unwrap()
            };
            let extent = vk::Extent2D { width, height };
            let image = d
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk::Format::R8G8B8A8_UNORM)
                        .extent(vk::Extent3D {
                            width,
                            height,
                            depth: 1,
                        })
                        .mip_levels(1)
                        .array_layers(2)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(
                            vk::ImageUsageFlags::COLOR_ATTACHMENT
                                | vk::ImageUsageFlags::TRANSFER_SRC,
                        ),
                    None,
                )
                .unwrap();
            let req = d.get_image_memory_requirements(image);
            let memory = d
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(req.size)
                        .memory_type_index(memory_type(
                            req.memory_type_bits,
                            vk::MemoryPropertyFlags::DEVICE_LOCAL,
                        )),
                    None,
                )
                .unwrap();
            d.bind_image_memory(image, memory, 0).unwrap();
            let buffer = d
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(u64::from(width) * u64::from(height) * 2 * 4)
                        .usage(vk::BufferUsageFlags::TRANSFER_DST),
                    None,
                )
                .unwrap();
            let req = d.get_buffer_memory_requirements(buffer);
            let buffer_memory = d
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(req.size)
                        .memory_type_index(memory_type(
                            req.memory_type_bits,
                            vk::MemoryPropertyFlags::HOST_VISIBLE
                                | vk::MemoryPropertyFlags::HOST_COHERENT,
                        )),
                    None,
                )
                .unwrap();
            d.bind_buffer_memory(buffer, buffer_memory, 0).unwrap();
            let pool = d
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(gpu.family)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .unwrap();
            let command = d
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .unwrap()[0];
            let target = Self {
                gpu,
                image,
                memory,
                buffer,
                buffer_memory,
                pool,
                command,
                extent,
            };
            target.begin();
            target.transition(
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::AccessFlags::empty(),
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            );
            target.submit();
            target
        }
    }
    pub fn handle(&self) -> u64 {
        self.image.as_raw()
    }
    unsafe fn begin(&self) {
        unsafe {
            self.gpu
                .device
                .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
                .unwrap();
            self.gpu
                .device
                .begin_command_buffer(self.command, &vk::CommandBufferBeginInfo::default())
                .unwrap();
        }
    }
    unsafe fn submit(&self) {
        unsafe {
            self.gpu.device.end_command_buffer(self.command).unwrap();
            self.gpu
                .device
                .queue_submit(
                    self.gpu.queue,
                    &[vk::SubmitInfo::default().command_buffers(&[self.command])],
                    vk::Fence::null(),
                )
                .unwrap();
            self.gpu.device.queue_wait_idle(self.gpu.queue).unwrap();
        }
    }
    unsafe fn transition(
        &self,
        old: vk::ImageLayout,
        new: vk::ImageLayout,
        src: vk::AccessFlags,
        dst: vk::AccessFlags,
    ) {
        unsafe {
            let barrier = vk::ImageMemoryBarrier::default()
                .image(self.image)
                .old_layout(old)
                .new_layout(new)
                .src_access_mask(src)
                .dst_access_mask(dst)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(2),
                );
            self.gpu.device.cmd_pipeline_barrier(
                self.command,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }
    }
    pub fn pixels(&self) -> Vec<u8> {
        unsafe {
            let d = &self.gpu.device;
            self.begin();
            self.transition(
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::AccessFlags::TRANSFER_READ,
            );
            d.cmd_copy_image_to_buffer(
                self.command,
                self.image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                self.buffer,
                &[vk::BufferImageCopy::default()
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(2),
                    )
                    .image_extent(vk::Extent3D {
                        width: self.extent.width,
                        height: self.extent.height,
                        depth: 1,
                    })],
            );
            let barrier = vk::BufferMemoryBarrier::default()
                .buffer(self.buffer)
                .size(vk::WHOLE_SIZE)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::HOST_READ)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED);
            d.cmd_pipeline_barrier(
                self.command,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::HOST,
                vk::DependencyFlags::empty(),
                &[],
                &[barrier],
                &[],
            );
            self.transition(
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::AccessFlags::TRANSFER_READ,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            );
            self.submit();
            let bytes = u64::from(self.extent.width) * u64::from(self.extent.height) * 2 * 4;
            let ptr = d
                .map_memory(self.buffer_memory, 0, bytes, vk::MemoryMapFlags::empty())
                .unwrap();
            let pixels = std::slice::from_raw_parts(ptr.cast::<u8>(), bytes as usize).to_vec();
            d.unmap_memory(self.buffer_memory);
            pixels
        }
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        unsafe {
            let _ = self.gpu.device.device_wait_idle();
            self.gpu.device.destroy_command_pool(self.pool, None);
            self.gpu.device.destroy_buffer(self.buffer, None);
            self.gpu.device.free_memory(self.buffer_memory, None);
            self.gpu.device.destroy_image(self.image, None);
            self.gpu.device.free_memory(self.memory, None);
        }
    }
}
