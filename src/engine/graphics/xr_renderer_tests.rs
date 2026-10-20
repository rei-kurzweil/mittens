use super::*;
use crate::engine::graphics::post_processing::{
    BloomConfig, PostProcessingConfig, PostProcessingRenderer,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyImageToBufferInfo, RenderingAttachmentInfo,
    RenderingAttachmentResolveInfo, RenderingInfo,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::{
    DeviceCreateInfo, DeviceExtensions, DeviceFeatures, QueueCreateInfo, QueueFlags,
};
use vulkano::format::{ClearValue, Format};
use vulkano::image::{Image, ImageType, ImageUsage, SampleCount};
use vulkano::instance::debug::{
    DebugUtilsMessageSeverity, DebugUtilsMessageType, DebugUtilsMessenger,
    DebugUtilsMessengerCallback, DebugUtilsMessengerCreateInfo,
};
use vulkano::instance::{Instance, InstanceCreateInfo, InstanceExtensions};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::render_pass::{AttachmentLoadOp, AttachmentStoreOp};
use vulkano::sync::{self, GpuFuture};
use vulkano::{Version, VulkanLibrary, VulkanObject};

/// Exercises real Vulkan images without requiring a headset or window. Run with:
/// cargo test --lib xr_renderer::tests -- --ignored --nocapture
#[test]
#[ignore = "requires a Vulkan device; enables Khronos validation when installed"]
fn borrowed_stereo_outputs_render_resolve_composite_and_reuse()
-> Result<(), Box<dyn std::error::Error>> {
    let library = VulkanLibrary::new()?;
    let validation_enabled = library
        .layer_properties()?
        .any(|layer| layer.name() == "VK_LAYER_KHRONOS_validation");
    eprintln!("Khronos validation layer enabled: {validation_enabled}");
    let instance = Instance::new(
        library,
        InstanceCreateInfo {
            max_api_version: Some(Version::V1_2),
            enabled_layers: if validation_enabled {
                vec!["VK_LAYER_KHRONOS_validation".into()]
            } else {
                vec![]
            },
            enabled_extensions: InstanceExtensions {
                ext_debug_utils: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    let validation_errors = Arc::new(AtomicUsize::new(0));
    let errors = validation_errors.clone();
    let _messenger = DebugUtilsMessenger::new(
        instance.clone(),
        DebugUtilsMessengerCreateInfo {
            message_severity: DebugUtilsMessageSeverity::ERROR,
            message_type: DebugUtilsMessageType::GENERAL | DebugUtilsMessageType::VALIDATION,
            ..DebugUtilsMessengerCreateInfo::user_callback(unsafe {
                DebugUtilsMessengerCallback::new(move |_, _, message| {
                    errors.fetch_add(1, Ordering::Relaxed);
                    eprintln!("Vulkan validation: {}", message.message);
                })
            })
        },
    )?;
    let (physical, queue_family_index) = instance
        .enumerate_physical_devices()?
        .filter(|p| {
            p.supported_extensions().khr_dynamic_rendering
                && p.supported_features().dynamic_rendering
        })
        .find_map(|p| {
            let index = p
                .queue_family_properties()
                .iter()
                .position(|q| q.queue_flags.intersects(QueueFlags::GRAPHICS))?;
            Some((p, index as u32))
        })
        .ok_or("no Vulkan graphics device with dynamic rendering")?;
    eprintln!(
        "XR output test device: {}",
        physical.properties().device_name
    );
    let (device, mut queues) = Device::new(
        physical,
        DeviceCreateInfo {
            enabled_extensions: DeviceExtensions {
                khr_dynamic_rendering: true,
                ..Default::default()
            },
            enabled_features: DeviceFeatures {
                dynamic_rendering: true,
                ..Default::default()
            },
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            ..Default::default()
        },
    )?;
    let queue = queues.next().unwrap();
    let allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
    let commands = Arc::new(StandardCommandBufferAllocator::new(
        device.clone(),
        Default::default(),
    ));
    let descriptors = Arc::new(StandardDescriptorSetAllocator::new(
        device.clone(),
        Default::default(),
    ));
    let mut post = PostProcessingRenderer::new(device.clone(), allocator.clone(), descriptors)?;
    let raw_entry = unsafe { ash::Entry::load()? };
    let raw_instance = unsafe { ash::Instance::load(raw_entry.static_fn(), instance.handle()) };
    let raw_device = unsafe { ash::Device::load(raw_instance.fp_v1_0(), device.handle()) };
    let fence = unsafe { raw_device.create_fence(&vk::FenceCreateInfo::default(), None)? };
    let extent = [8, 8];
    let pixel_count = (extent[0] * extent[1]) as usize;
    let create_info = ImageCreateInfo {
        image_type: ImageType::Dim2d,
        format: Format::R8G8B8A8_UNORM,
        extent: [extent[0], extent[1], 1],
        array_layers: 2,
        usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST,
        ..Default::default()
    };
    // Model three runtime-owned stereo images, each with one cached wrapper.
    let owners = (0..3)
        .map(|_| Image::new(allocator.clone(), create_info.clone(), Default::default()))
        .collect::<Result<Vec<_>, _>>()?;
    let views = owners
        .iter()
        .map(|image| unsafe {
            borrow_xr_image_views(device.clone(), image.handle(), create_info.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;

    for mode in 0..3 {
        for frame in 0..6 {
            let output = &views[frame % views.len()];
            assert_eq!(
                output[0].image().handle(),
                owners[frame % owners.len()].handle()
            );
            assert_eq!(output[0].subresource_range().array_layers, 0..1);
            assert_eq!(output[1].subresource_range().array_layers, 1..2);
            let colors = if frame % 2 == 0 {
                [[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]]
            } else {
                [[0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 0.0, 1.0]]
            };
            let mut future: Box<dyn GpuFuture> = sync::now(device.clone()).boxed();
            for eye in 0..2 {
                let mut cbb = AutoCommandBufferBuilder::primary(
                    commands.clone(),
                    queue_family_index,
                    CommandBufferUsage::OneTimeSubmit,
                )?;
                let source = if mode == 0 {
                    output[eye].clone()
                } else {
                    ImageView::new_default(Image::new(
                        allocator.clone(),
                        ImageCreateInfo {
                            samples: if mode == 1 {
                                SampleCount::Sample4
                            } else {
                                SampleCount::Sample1
                            },
                            array_layers: 1,
                            usage: if mode == 1 {
                                ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT
                            } else {
                                ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED
                            },
                            ..create_info.clone()
                        },
                        Default::default(),
                    )?)?
                };
                let mut attachment = RenderingAttachmentInfo {
                    load_op: AttachmentLoadOp::Clear,
                    store_op: AttachmentStoreOp::Store,
                    clear_value: Some(ClearValue::Float(colors[eye])),
                    ..RenderingAttachmentInfo::image_view(source.clone())
                };
                if mode == 1 {
                    attachment.store_op = AttachmentStoreOp::DontCare;
                    attachment.resolve_info = Some(RenderingAttachmentResolveInfo::image_view(
                        output[eye].clone(),
                    ));
                }
                cbb.begin_rendering(RenderingInfo {
                    render_area_extent: extent,
                    color_attachments: vec![Some(attachment)],
                    ..Default::default()
                })?;
                cbb.end_rendering()?;
                if mode == 2 {
                    // Exercise the actual final bloom-composite pipeline with a
                    // zero-intensity source, writing directly to the borrowed eye.
                    post.record_final_pass(
                        &mut cbb,
                        create_info.format,
                        output[eye].clone(),
                        extent,
                        source.clone(),
                        Some(source),
                        &PostProcessingConfig {
                            enabled: true,
                            bloom: Some(BloomConfig {
                                intensity: 0.0,
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )?;
                }
                future = future
                    .then_execute(queue.clone(), cbb.build()?)?
                    .then_signal_fence_and_flush()?
                    .boxed();
            }
            let readback = Buffer::new_slice::<u8>(
                allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                    ..Default::default()
                },
                (pixel_count * 4 * 2) as u64,
            )?;
            let mut cbb = AutoCommandBufferBuilder::primary(
                commands.clone(),
                queue_family_index,
                CommandBufferUsage::OneTimeSubmit,
            )?;
            cbb.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
                output[0].image().clone(),
                readback.clone(),
            ))?;
            future = future
                .then_execute(queue.clone(), cbb.build()?)?
                .then_signal_fence_and_flush()?
                .boxed();
            // SAFETY: the guard locks the same queue used above; this test owns
            // and resets the fence before each reuse on this live device.
            queue.with(|_| unsafe {
                submit_xr_completion_and_wait(&raw_device, queue.handle(), fence)
            })?;
            unsafe { future.signal_finished() };
            future.cleanup_finished();
            drop(future);
            let pixels = readback.read()?;
            for eye in 0..2 {
                let expected = colors[eye].map(|channel| (channel * 255.0) as u8);
                for pixel in
                    pixels[eye * pixel_count * 4..(eye + 1) * pixel_count * 4].chunks_exact(4)
                {
                    assert_eq!(pixel, expected, "mode={mode} frame={frame} eye={eye}");
                }
            }
        }
    }
    drop(post);
    drop(views);
    // Dropping the borrowed wrappers must not destroy the external images.
    for image in &owners {
        drop(ImageView::new_default(image.clone())?);
    }
    drop(owners);
    unsafe { raw_device.destroy_fence(fence, None) };
    assert_eq!(validation_errors.load(Ordering::Relaxed), 0);
    Ok(())
}
