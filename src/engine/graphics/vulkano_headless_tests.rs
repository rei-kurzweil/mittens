use super::*;
use crate::engine::graphics::{CameraData, GpuRenderable, MaterialHandle, MeshFactory, Transform};
use vulkano::command_buffer::CopyImageToBufferInfo;
use vulkano::image::view::ImageViewType;

/// Exercise the full scene pipelines, per-eye slots, and MSAA resolve without
/// a display server, Winit, or OpenXR runtime. Acquisition/lifecycle remain the
/// responsibility of the real-runtime openxr-only example.
#[test]
#[ignore = "requires a Vulkan graphics/compute device with dynamic rendering"]
fn headless_scene_draws_stereo_without_surface_extensions() -> Result<(), Box<dyn std::error::Error>>
{
    let context = RendererContext::new(None, None, None)?;
    assert!(!context.device().instance().enabled_extensions().khr_surface);
    assert!(!context.device().enabled_extensions().khr_swapchain);
    let format = Format::B8G8R8A8_UNORM;
    let extent = [16, 16];

    for msaa in [MsaaMode::Off, MsaaMode::Msaa4x] {
        let mut renderer = VulkanoState::new(context.clone(), RendererOutput::Xr(format), msaa)?;
        #[cfg(feature = "desktop")]
        assert!(renderer.window_output.is_none());
        assert_eq!(renderer.window_frame_slots(), 0);
        assert_eq!(renderer.scene_color_format(), format);
        eprintln!(
            "Headless scene test: {} MSAA={}",
            renderer.gpu_device_name(),
            renderer.msaa_description()
        );
        let output = Image::new(
            context.memory_allocator().clone(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format,
                extent: [extent[0], extent[1], 1],
                array_layers: 2,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
                ..Default::default()
            },
            Default::default(),
        )?;
        let eye_views = (0..2)
            .map(|eye| {
                ImageView::new(
                    output.clone(),
                    ImageViewCreateInfo {
                        view_type: ImageViewType::Dim2d,
                        subresource_range: ImageSubresourceRange {
                            array_layers: eye..eye + 1,
                            ..output.subresource_range()
                        },
                        ..ImageViewCreateInfo::from_image(&output)
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut visuals = VisualWorld::new();
        let transform = Transform::default();
        visuals.set_xr_camera(vec![
            CameraData {
                view: transform.model,
                proj: transform.model,
                transform,
            };
            2
        ]);
        let mesh = MeshHandle(0);
        renderer.upload_mesh(mesh, &MeshFactory::quad_2d())?;
        visuals.register(
            crate::engine::ecs::ComponentId::default(),
            GpuRenderable::new(mesh, MaterialHandle::UNLIT_MESH),
            transform,
            [1.0, 0.0, 0.0, 1.0],
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            3.0,
        );
        for frame in 0..3 {
            for eye in 0..2 {
                // Different colors make stale layers and slot reuse observable.
                visuals.set_clear_color(if (frame + eye) % 2 == 0 {
                    [0.0, 0.0, 1.0, 1.0]
                } else {
                    [0.0, 1.0, 0.0, 1.0]
                });
                renderer.submit_xr_eye_to_view(
                    &mut visuals,
                    eye,
                    extent,
                    2,
                    eye_views[eye].clone(),
                )?;
            }
            let readback = Buffer::new_slice::<u8>(
                context.memory_allocator().clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                    ..Default::default()
                },
                (extent[0] * extent[1] * 4 * 2) as u64,
            )?;
            let mut commands = AutoCommandBufferBuilder::primary(
                renderer.command_buffer_allocator.clone(),
                context.graphics_queue().queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )?;
            commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
                output.clone(),
                readback.clone(),
            ))?;
            let completion = renderer
                .submission_future
                .take()
                .unwrap()
                .then_execute(context.graphics_queue().clone(), commands.build()?)?
                .then_signal_fence_and_flush()?;
            completion.wait(None)?;
            renderer.submission_future = Some(completion.boxed());
            let pixels = readback.read()?;
            for eye in 0..2 {
                let base = eye * (extent[0] * extent[1] * 4) as usize;
                // BGRA output: center is the red mesh, corner is eye clear.
                let center = base + ((extent[1] / 2 * extent[0] + extent[0] / 2) * 4) as usize;
                assert_eq!(&pixels[center..center + 4], &[0, 0, 255, 255]);
                let expected = if (frame + eye) % 2 == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 255, 0, 255]
                };
                assert_eq!(&pixels[base..base + 4], &expected);
            }
        }
        assert!(renderer.images_in_flight.is_empty());
        assert_eq!(
            renderer
                .xr_eye_attachments
                .as_ref()
                .unwrap()
                .depth_views
                .len(),
            2
        );
        assert_eq!(renderer.perf_counters().xr_eyes, 6);
    }
    Ok(())
}

#[test]
#[ignore = "requires Vulkan; renders independent mutable material instances into both eye layers"]
fn material_instances_render_independent_revisions_in_both_eyes() -> Result<(), Box<dyn std::error::Error>> {
    use crate::engine::ecs::{World, component::ShadingComponent};
    use crate::engine::graphics::visual_world::AnimeShadingParams;
    let context = RendererContext::new(None, None, None)?;
    let format = Format::B8G8R8A8_UNORM;
    let extent = [16, 16];
    let mut renderer = VulkanoState::new(context.clone(), RendererOutput::Xr(format), MsaaMode::Off)?;
    let output = Image::new(context.memory_allocator().clone(), ImageCreateInfo {
        image_type: ImageType::Dim2d, format, extent: [16, 16, 1], array_layers: 2,
        usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC, ..Default::default()
    }, Default::default())?;
    let eye_views = (0..2).map(|eye| ImageView::new(output.clone(), ImageViewCreateInfo {
        view_type: ImageViewType::Dim2d,
        subresource_range: ImageSubresourceRange { array_layers: eye..eye+1, ..output.subresource_range() },
        ..ImageViewCreateInfo::from_image(&output)
    })).collect::<Result<Vec<_>, _>>()?;
    let mut world = World::default();
    let a = world.add_component(ShadingComponent::new());
    let b = world.add_component(ShadingComponent::new());
    let mut visuals = VisualWorld::new();
    let identity = Transform::default();
    visuals.set_xr_camera(vec![CameraData { view: identity.model, proj: identity.model, transform: identity }; 2]);
    let mesh = MeshHandle(0);
    renderer.upload_mesh(mesh, &MeshFactory::quad_2d())?;
    let definition = visuals.materials.anime_definition;
    let mut params = AnimeShadingParams::default();
    params.shade_color_strength = [1.0, 0.0, 0.0, 1.0];
    params.controls[2] = 0.0;
    let id_a = visuals.materials.upsert_source(a, definition, params);
    let id_b = visuals.materials.upsert_source(b, definition, params);
    for (offset, id) in [(-0.5, id_a), (0.5, id_b)] {
        let mut transform = Transform::default();
        transform.model[3][0] = offset;
        transform.matrix_world = transform.model;
        let handle = visuals.register(a, GpuRenderable::new(mesh, MaterialHandle::ANIME_MESH), transform,
            [1.0; 4], 1.0, false, false, false, false, false, 0.0, None, 3.0);
        assert!(visuals.bind_material_instance(handle, Some(id)));
    }
    for frame in 0..12 {
        params.shade_color_strength = if frame % 2 == 0 { [0.0, 1.0, 0.0, 1.0] } else { [0.0, 0.0, 1.0, 1.0] };
        visuals.materials.upsert_source(a, definition, params);
        for eye in 0..2 { renderer.submit_xr_eye_to_view(&mut visuals, eye, extent, 2, eye_views[eye].clone())?; }
        let readback = Buffer::new_slice::<u8>(context.memory_allocator().clone(),
            BufferCreateInfo { usage: BufferUsage::TRANSFER_DST, ..Default::default() },
            AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_RANDOM_ACCESS, ..Default::default() },
            (16 * 16 * 4 * 2) as u64)?;
        let mut commands = AutoCommandBufferBuilder::primary(renderer.command_buffer_allocator.clone(),
            context.graphics_queue().queue_family_index(), CommandBufferUsage::OneTimeSubmit)?;
        commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(output.clone(), readback.clone()))?;
        let completion = renderer.submission_future.take().unwrap()
            .then_execute(context.graphics_queue().clone(), commands.build()?)?.then_signal_fence_and_flush()?;
        completion.wait(None)?;
        renderer.submission_future = Some(completion.boxed());
        let pixels = readback.read()?;
        for eye in 0..2 {
            let base = eye * 16 * 16 * 4;
            let left = base + (8 * 16 + 4) * 4;
            let right = base + (8 * 16 + 12) * 4;
            let expected = if frame % 2 == 0 { [0, 255, 0, 255] } else { [255, 0, 0, 255] };
            assert_eq!(&pixels[left..left+4], &expected, "eye {eye}, frame {frame}: shared material revision");
            assert_eq!(&pixels[right..right+4], &[0, 0, 255, 255], "independent material stays red");
        }
        // The explicit completion wait above proves the entire same-queue chain
        // is finished, just as the real XR host's completion fence does.
        unsafe { renderer.finish_xr_batch_after_external_wait(); }
    }
    assert_eq!(renderer.material_upload_stats.allocations, 2, "two stable instance bindings, reused across frames and eyes");
    assert_eq!(renderer.cached_material_sets.len(), 0);

    // Record two different revisions BEFORE submitting either command buffer.
    // GPU access locks alone cannot protect this case: the descriptor ownership
    // rule must preserve the first recording's data while building the second.
    let mut recorded = Vec::new();
    for eye in 0..2 {
        params.shade_color_strength = if eye == 0 { [0.0, 1.0, 0.0, 1.0] } else { [0.0, 0.0, 1.0, 1.0] };
        visuals.materials.upsert_source(a, definition, params);
        let depth = renderer.xr_eye_attachments.as_ref().unwrap().depth_views[eye].clone();
        recorded.push(renderer.build_draw_batches_command_buffer(&mut visuals, &RenderView {
            view: identity.model, proj: identity.model, viewport: [16.0, 16.0], kind: RenderViewKind::XrEye { eye },
        }, eye, 2, eye_views[eye].clone(), None, depth, extent, None, None, None, None)?);
    }
    let readback = Buffer::new_slice::<u8>(context.memory_allocator().clone(),
        BufferCreateInfo { usage: BufferUsage::TRANSFER_DST, ..Default::default() },
        AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_RANDOM_ACCESS, ..Default::default() },
        (16 * 16 * 4 * 2) as u64)?;
    let mut commands = AutoCommandBufferBuilder::primary(renderer.command_buffer_allocator.clone(),
        context.graphics_queue().queue_family_index(), CommandBufferUsage::OneTimeSubmit)?;
    commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(output, readback.clone()))?;
    let first = recorded.remove(0);
    let second = recorded.remove(0);
    let completion = renderer.submission_future.take().unwrap()
        .then_execute(context.graphics_queue().clone(), first)?
        .then_execute(context.graphics_queue().clone(), second)?
        .then_execute(context.graphics_queue().clone(), commands.build()?)?.then_signal_fence_and_flush()?;
    completion.wait(None)?;
    let pixels = readback.read()?;
    for eye in 0..2 {
        let offset = eye * 16 * 16 * 4 + (8 * 16 + 4) * 4;
        let expected = if eye == 0 { [0, 255, 0, 255] } else { [255, 0, 0, 255] };
        assert_eq!(&pixels[offset..offset+4], &expected, "recording {eye} retains its own material revision");
    }
    assert_eq!(renderer.material_upload_stats.allocations, 3, "one additional snapshot protects an unsubmitted older revision");
    Ok(())
}
