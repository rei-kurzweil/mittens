#![allow(dead_code)]
#[path = "../../../src/renderer/mod.rs"]
mod renderer;
mod scene;
#[path = "../../../src/scene_runtime.rs"]
mod scene_runtime;
mod target;
mod xr_host {
    pub type Result<T> = std::result::Result<T, String>;
    pub trait ApiResult<T> {
        fn api(self, op: &str) -> Result<T>;
    }
    impl<T, E: std::fmt::Debug> ApiResult<T> for std::result::Result<T, E> {
        fn api(self, op: &str) -> Result<T> {
            self.map_err(|e| format!("{op}: {e:?}"))
        }
    }
    pub struct Report;
    impl Report {
        pub fn line(&mut self, _: impl std::fmt::Display) {}
    }
}
use vulkan_probe::gpu as graphics_device;
mod vulkan_probe {
    pub mod gpu {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src/graphics_device.rs"
        ));
        pub(crate) fn headless(dynamic: bool) -> Arc<Gpu> {
            unsafe {
                let entry = ash::Entry::load().unwrap();
                let app = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_2);
                let raw = entry
                    .create_instance(
                        &vk::InstanceCreateInfo::default().application_info(&app),
                        None,
                    )
                    .unwrap();
                let instance = Arc::new(InstanceOwner { raw, _entry: entry });
                let physical = instance.raw.enumerate_physical_devices().unwrap()[0];
                let properties = instance.raw.get_physical_device_properties(physical);
                println!(
                    "GPU={} dynamic={dynamic}",
                    CStr::from_ptr(properties.device_name.as_ptr()).to_string_lossy()
                );
                let family = instance
                    .raw
                    .get_physical_device_queue_family_properties(physical)
                    .iter()
                    .position(|q| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                    .unwrap() as u32;
                let queues = [vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family)
                    .queue_priorities(&[1.0])];
                let mut feature =
                    vk::PhysicalDeviceDynamicRenderingFeatures::default().dynamic_rendering(true);
                let extensions = [ash::khr::dynamic_rendering::NAME.as_ptr()];
                let mut info = vk::DeviceCreateInfo::default().queue_create_infos(&queues);
                if dynamic {
                    info = info
                        .enabled_extension_names(&extensions)
                        .push_next(&mut feature);
                }
                let device = instance.raw.create_device(physical, &info, None).unwrap();
                let queue = device.get_device_queue(family, 0);
                let dynamic_api = dynamic
                    .then(|| ash::khr::dynamic_rendering::Device::new(&instance.raw, &device));
                Arc::new(Gpu {
                    device,
                    instance,
                    physical,
                    queue,
                    family,
                    dynamic,
                    dynamic_api,
                })
            }
        }
    }
    pub mod renderer {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../src/triangle_probe.rs"
        ));
    }
    pub fn run(dynamic: bool) {
        unsafe {
            use ash::{vk, vk::Handle};
            let gpu = gpu::headless(dynamic);
            let d = &gpu.device;
            let mem = gpu
                .instance
                .raw
                .get_physical_device_memory_properties(gpu.physical);
            let memory_type = |bits: u32, flags: vk::MemoryPropertyFlags| -> u32 {
                (0..mem.memory_type_count)
                    .find(|i| {
                        bits & (1 << i) != 0
                            && mem.memory_types[*i as usize].property_flags.contains(flags)
                    })
                    .unwrap()
            };
            let extent = vk::Extent2D {
                width: 96,
                height: 96,
            };
            let image = d
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk::Format::R8G8B8A8_UNORM)
                        .extent(vk::Extent3D {
                            width: 96,
                            height: 96,
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
            let image_memory = d
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
            d.bind_image_memory(image, image_memory, 0).unwrap();
            let bytes = 96 * 96 * 2 * 4;
            let buffer = d
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(bytes)
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
            let cmd = d
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .unwrap()[0];
            let range = vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .level_count(1)
                .layer_count(2);
            let transition = |old, new, src, dst| {
                vk::ImageMemoryBarrier::default()
                    .image(image)
                    .old_layout(old)
                    .new_layout(new)
                    .src_access_mask(src)
                    .dst_access_mask(dst)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .subresource_range(range)
            };
            let submit = || {
                d.end_command_buffer(cmd).unwrap();
                d.queue_submit(
                    gpu.queue,
                    &[vk::SubmitInfo::default().command_buffers(&[cmd])],
                    vk::Fence::null(),
                )
                .unwrap();
                d.queue_wait_idle(gpu.queue).unwrap();
            };
            d.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
                .unwrap();
            d.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[transition(
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    vk::AccessFlags::empty(),
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                )],
            );
            submit();
            let mut renderer = renderer::Renderer::new(
                gpu.clone(),
                vk::Format::R8G8B8A8_UNORM,
                extent,
                vec![image.as_raw()],
            )
            .unwrap();
            for triangle in [false, true, true, false] {
                renderer.draw(0, triangle).unwrap();
                d.reset_command_pool(pool, vk::CommandPoolResetFlags::empty())
                    .unwrap();
                d.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
                    .unwrap();
                d.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[transition(
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                        vk::AccessFlags::TRANSFER_READ,
                    )],
                );
                d.cmd_copy_image_to_buffer(
                    cmd,
                    image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    buffer,
                    &[vk::BufferImageCopy::default()
                        .image_subresource(
                            vk::ImageSubresourceLayers::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .layer_count(2),
                        )
                        .image_extent(vk::Extent3D {
                            width: 96,
                            height: 96,
                            depth: 1,
                        })],
                );
                let barrier = vk::BufferMemoryBarrier::default()
                    .buffer(buffer)
                    .size(vk::WHOLE_SIZE)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::HOST_READ);
                d.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::HOST,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[barrier],
                    &[],
                );
                d.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[transition(
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        vk::AccessFlags::TRANSFER_READ,
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                    )],
                );
                submit();
                let ptr = d
                    .map_memory(buffer_memory, 0, bytes, vk::MemoryMapFlags::empty())
                    .unwrap();
                let pixels = std::slice::from_raw_parts(ptr as *const u8, bytes as usize);
                let pixel = |eye: usize, x: usize, y: usize| {
                    pixels[(eye * 96 * 96 + y * 96 + x) * 4..][..4].to_vec()
                };
                let left = pixel(0, 0, 0);
                let right = pixel(1, 0, 0);
                assert!(
                    left[0] > left[2] && right[2] > right[0],
                    "eye clears {left:?} {right:?}"
                );
                for eye in 0..2 {
                    let corner = pixel(eye, 0, 0);
                    let center = pixel(eye, 48, 48);
                    if triangle {
                        assert!(
                            center[1] > corner[1] + 30,
                            "missing triangle eye={eye} center={center:?}"
                        );
                    } else {
                        assert_eq!(corner, center);
                    }
                }
                d.unmap_memory(buffer_memory);
                println!(
                    "PASS dynamic={dynamic} triangle={triangle} left={left:?} right={right:?}"
                );
            }
            drop(renderer);
            d.destroy_command_pool(pool, None);
            d.destroy_buffer(buffer, None);
            d.free_memory(buffer_memory, None);
            d.destroy_image(image, None);
            d.free_memory(image_memory, None);
        }
    }
}
fn main() {
    vulkan_probe::run(true);
    vulkan_probe::run(false);
    scene::run();
}
