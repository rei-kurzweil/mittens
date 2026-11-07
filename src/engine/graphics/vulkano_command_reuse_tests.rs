use super::*;
use crate::engine::graphics::post_processing::{BloomConfig, PostProcessingConfig};
use crate::engine::graphics::{CameraData, GpuRenderable, MaterialHandle, MeshFactory, Transform};
use vulkano::command_buffer::CopyImageToBufferInfo;
use vulkano::image::view::ImageViewType;

// Compare the real cached extraction/filter path with fresh recording. Joint
// matrices, camera uniforms and lights change every frame, including cache hits.
#[test]
#[ignore = "requires Vulkan; verifies reusable primary pixels under animated inputs and invalidation"]
fn retained_primaries_match_fresh_commands_with_changing_camera_lights_and_joints()
-> Result<(), Box<dyn std::error::Error>> {
    let context = RendererContext::new(None, None, None)?;
    let extent = [32, 32];
    let mut results = Vec::new();
    for mode in 0..3 {
        let enabled = mode != 0;
        let mut renderer = VulkanoState::new(
            context.clone(),
            RendererOutput::Xr(Format::B8G8R8A8_UNORM),
            MsaaMode::Msaa4x,
        )?;
        renderer.command_reuse.enabled = enabled;
        renderer.command_reuse.scene_enabled = mode == 2;
        renderer.retained_uploads.borrow_mut().enabled = true;
        let output = Image::new(
            context.memory_allocator().clone(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::B8G8R8A8_UNORM,
                extent: [32, 32, 1],
                array_layers: 2,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
                ..Default::default()
            },
            Default::default(),
        )?;
        let eyes = (0..2)
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
        let mesh = MeshHandle(91);
        let skin = || {
            let quad = MeshFactory::quad_2d();
            let count = quad.vertices.len();
            quad.with_skinning(vec![[0; 4]; count], vec![[1., 0., 0., 0.]; count])
        };
        renderer.upload_mesh(mesh, &skin())?;
        let mut ids = slotmap::SlotMap::<ComponentId, ()>::with_key();
        let object = ids.insert(());
        let light = ids.insert(());
        let handle = visuals.register(
            object,
            GpuRenderable::new(mesh, MaterialHandle::SKINNED_EMISSIVE_TOON_MESH),
            Transform::default(),
            [1., 0.3, 0.1, 1.],
            1.,
            false,
            false,
            false,
            false,
            false,
            1.,
            None,
            3.,
        );
        let lit_mesh = MeshHandle(92);
        renderer.upload_mesh(lit_mesh, &MeshFactory::quad_2d())?;
        let mut lit_transform = Transform::default();
        lit_transform.model[0][0] = 0.35;
        lit_transform.model[1][1] = 0.35;
        lit_transform.model[3][0] = -0.6;
        visuals.set_ambient_light([0.05; 3]);
        visuals.register(
            ids.insert(()),
            GpuRenderable::new(lit_mesh, MaterialHandle::TOON_MESH),
            lit_transform,
            [1.; 4],
            1.,
            false,
            false,
            false,
            false,
            false,
            0.,
            None,
            3.,
        );
        let mut frames = Vec::new();
        for frame in 0..9 {
            let mut config = PostProcessingConfig {
                enabled: true,
                bloom: Some(BloomConfig::default()),
                ..Default::default()
            };
            config.bloom.as_mut().unwrap().intensity = if frame < 4 { 0.8 } else { 1.4 };
            visuals.set_post_processing(config);
            let motion_frame = frame.min(7);
            let mut joint = Transform::default().model;
            joint[3][0] = motion_frame as f32 * 0.045;
            assert!(visuals.set_skin_matrices(handle, &[joint]));
            visuals.upsert_light(
                light,
                crate::engine::graphics::visual_world::VisualLight {
                    light_type: 2,
                    // Directional lights store their toward-light vector in this field.
                    position_ws: [0., 0., 1.],
                    intensity: 2.0,
                    distance: 10.,
                    color: if frame % 2 == 0 {
                        [1., 0., 0.]
                    } else {
                        [0., 1., 0.]
                    },
                    direction_ws: [0., 0., -1.],
                    angle: 1.,
                    penumbra: 0.,
                },
            );
            let identity = Transform::default();
            visuals.set_xr_camera(
                (0..2)
                    .map(|eye| {
                        let mut view = identity.model;
                        view[3][1] = motion_frame as f32 * -0.02;
                        view[3][0] = eye as f32 * 0.03;
                        CameraData {
                            view,
                            proj: identity.model,
                            transform: identity,
                        }
                    })
                    .collect(),
            );
            // Same logical mesh handle, replaced GPU storage: old commands must retire.
            if frame == 6 {
                renderer.meshes.remove(&mesh);
                renderer.upload_mesh(mesh, &skin())?;
            }
            let before = renderer.command_reuse.stats;
            for eye in 0..2 {
                renderer.submit_xr_eye_to_view(&mut visuals, eye, extent, 2, eyes[eye].clone())?;
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
                32 * 32 * 4 * 2,
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
            unsafe {
                renderer.finish_xr_batch_after_external_wait();
            }
            frames.push(readback.read()?.to_vec());
            if enabled {
                let delta = renderer.command_reuse.stats.delta(before);
                if matches!(frame, 2 | 3 | 5 | 8) {
                    assert_eq!(
                        delta.hits, 2,
                        "both eye recordings must survive animated frame {frame}"
                    );
                    assert_eq!(delta.rebuilds, 0);
                }
                if frame == 4 {
                    assert_eq!(
                        delta.rebuilds, 2,
                        "changed recorded dependency at frame {frame}"
                    );
                }
                if frame == 6 {
                    assert_eq!(delta.hits, 0, "replaced mesh must retire old commands");
                }
            }
        }
        assert_ne!(
            frames[2], frames[3],
            "reused commands must render changed pose/camera contents"
        );
        assert!(
            frames[7] != frames[8],
            "light-only changes must reach retained commands in mode {mode}"
        );
        results.push(frames);
    }
    for cached in &results[1..] {
        assert_eq!(
            &results[0], cached,
            "cached and fresh recording must produce identical pixels for every eye/frame"
        );
    }
    Ok(())
}
