use super::*;
use crate::engine::ecs::component::TransformComponent;
use crate::engine::graphics::{CameraTarget, MeshUploader, TextureUploader};
use crate::utils::math::{mat4_inverse, quat_mul, quat_rotation_y};

struct Upload<'a> {
    renderer: &'a mut VulkanoState,
    mesh: u32,
    texture: u32,
}
impl MeshUploader for Upload<'_> {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.mesh += 1;
        let handle = MeshHandle(self.mesh);
        self.renderer.upload_mesh(handle, mesh)?;
        Ok(handle)
    }
}
impl TextureUploader for Upload<'_> {
    fn upload_texture_rgba8(
        &mut self,
        rgba: &[u8],
        w: u32,
        h: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.texture += 1;
        let handle = TextureHandle(self.texture);
        self.renderer.upload_texture_rgba8(handle, rgba, w, h)?;
        Ok(handle)
    }
    fn upload_texture_bc7(
        &mut self,
        blocks: &[u8],
        w: u32,
        h: u32,
        srgb: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.texture += 1;
        let handle = TextureHandle(self.texture);
        self.renderer
            .upload_texture_bc7(handle, blocks, w, h, srgb)?;
        Ok(handle)
    }
}

#[test]
#[ignore = "requires Vulkan; captures twilight sky in stereo with bloom off/on"]
fn twilight_sky_stereo_has_baked_glow_without_bloom_or_translation_parallax()
-> Result<(), Box<dyn std::error::Error>> {
    let context = RendererContext::new(None, None, None)?;
    let extent = [960, 640];
    let format = Format::B8G8R8A8_UNORM;
    let mut renderer = VulkanoState::new(
        context.clone(),
        RendererOutput::Xr(format),
        MsaaMode::Msaa4x,
    )?;
    let mut scene = crate::engine::Universe::new(Default::default());
    // Keep actual scene geometry/materials. The fixture supplies cameras and
    // omits live XR/audio/avatar drivers rather than requiring a headset.
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/paper-craft.mms"
    ));
    let a = original.find("// One tracked XR player;").unwrap();
    let b = original.find("// Weapon table").unwrap();
    let source = format!(
        "{}{}\nT {{ C3D {{ fov(70) far(100) }} }}",
        &original[..a],
        &original[b..]
    );
    scene.load_mms_source_at_path(&source, "examples/paper-craft.mms")?;
    for _ in 0..3 {
        scene.update(0., &Default::default());
    }
    for (_, material) in scene.visuals.materials.instances() {
        if let Some(custom) = scene
            .visuals
            .materials
            .definition(material.definition)
            .and_then(|d| d.custom.as_ref())
        {
            for _ in 0..500 {
                if custom.compiled.get().is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            custom
                .compiled
                .get()
                .ok_or("shader compilation timeout")?
                .as_ref()
                .map_err(Clone::clone)?;
        }
    }
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut Upload {
            renderer: &mut renderer,
            mesh: 1000,
            texture: 1000,
        },
        &mut scene.command_queue,
    );
    scene.visuals.prepare_draw_cache();
    assert_eq!(scene.visuals.background_order().len(), 1);
    assert!(
        scene
            .visuals
            .background_occluded_lit_emissive_order()
            .is_empty()
    );
    let sky = &scene.visuals.instances()[scene.visuals.background_order()[0] as usize];
    assert!(sky.texture.is_some(), "cloud mask must reach the sky draw");
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
    let projection = scene.visuals.camera_proj_for(CameraTarget::Window);
    let toggle = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("graphics_bloom_toggle"))
        .unwrap();
    let mut sky_reference: Option<Vec<u8>> = None;
    for (label, position, direction, bloom) in [
        ("moon-off", [0., 60., 0.], [-0.35, 0.30, -0.88], false),
        ("moon-on", [0., 60., 0.], [-0.35, 0.30, -0.88], true),
        (
            "moon-translated",
            [300., 260., 150.],
            [-0.35, 0.30, -0.88],
            false,
        ),
        ("roof", [-12.8, 22., 4.2], [-5.2, -1.5, -7.5], false),
        ("street", [0., 1.7, 30.], [-18., 10., -30.], false),
        ("occluded", [0., 1.7, 40.], [-0.35, 0.30, -0.88], false),
        ("stairs", [-20.5, 17.7, 1.2], [-2.2, -1., 3.8], false),
        ("zenith", [0., 60., 0.], [0.001, 1., -0.001], false),
        ("seam", [0., 60., 0.], [-1., 0.1, 0.001], false),
    ] {
        scene.systems.rx.push_event(
            toggle,
            crate::engine::ecs::EventSignal::ToggleChanged {
                toggle,
                value: bloom,
            },
        );
        scene.update(0., &Default::default());
        let target = std::array::from_fn(|i| position[i] + direction[i]);
        let rotation = TransformComponent::look_at_world_rotation(position, target).unwrap();
        let camera = TransformComponent::new()
            .with_position(position[0], position[1], position[2])
            .with_rotation_quat(quat_mul(rotation, quat_rotation_y(std::f32::consts::PI)));
        scene.visuals.set_camera_for_target(
            CameraTarget::Xr,
            (0..2)
                .map(|eye| {
                    let mut model = camera.transform.model;
                    for axis in 0..3 {
                        model[3][axis] += model[0][axis] * if eye == 0 { -0.032 } else { 0.032 };
                    }
                    CameraData {
                        view: mat4_inverse(model).unwrap(),
                        proj: projection,
                        transform: Default::default(),
                    }
                })
                .collect(),
        );
        for eye in 0..2 {
            renderer.submit_xr_eye_to_view(
                &mut scene.visuals,
                eye,
                extent,
                2,
                eyes[eye].clone(),
            )?;
        }
        let size = (extent[0] * extent[1] * 4) as u64;
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
            size * 2,
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
        let done = renderer
            .submission_future
            .take()
            .unwrap()
            .then_execute(context.graphics_queue().clone(), commands.build()?)?
            .then_signal_fence_and_flush()?;
        done.wait(None)?;
        renderer.submission_future = Some(done.boxed());
        let pixels = readback.read()?;
        for eye in 0..2 {
            let bytes = &pixels[eye * size as usize..(eye + 1) * size as usize];
            let rgba: Vec<_> = bytes
                .chunks_exact(4)
                .flat_map(|p| [p[2], p[1], p[0], p[3]])
                .collect();
            image::save_buffer(
                format!("/tmp/paper-craft-twilight-{label}-{eye}.png"),
                &rgba,
                extent[0],
                extent[1],
                image::ColorType::Rgba8,
            )?;
        }
        if label == "moon-off" {
            assert!(
                pixels[..size as usize]
                    .chunks_exact(4)
                    .filter(|p| p[0] > 245 && p[1] > 245 && p[2] > 245)
                    .count()
                    > 100,
                "crescent must be visible with bloom off"
            );
            sky_reference = Some(pixels[..size as usize].to_vec());
        } else if label.starts_with("moon-") {
            let difference: f64 = pixels[..size as usize]
                .iter()
                .zip(sky_reference.as_ref().unwrap())
                .map(|(a, b)| a.abs_diff(*b) as f64)
                .sum::<f64>()
                / size as f64;
            assert!(
                difference < 0.5,
                "bloom/translation changed the baked sky: {difference}"
            );
        } else if label == "seam" {
            // This view crosses atan's longitude wrap at its center. A broken
            // implicit texture derivative previously produced a 36-level stripe.
            let mut largest_jump = 0;
            for y in 20..extent[1] - 20 {
                for x in extent[0] / 2 - 3..extent[0] / 2 + 4 {
                    let index = ((y * extent[0] + x) * 4) as usize;
                    for channel in 0..3 {
                        largest_jump = largest_jump
                            .max(pixels[index + channel].abs_diff(pixels[index - 4 + channel]));
                    }
                }
            }
            assert!(largest_jump < 5, "visible longitude seam: {largest_jump}");
        }
    }
    Ok(())
}
