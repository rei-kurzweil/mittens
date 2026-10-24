use super::*;
use crate::engine::graphics::{CameraTarget, CpuMesh, MeshHandle, MeshUploader, TextureUploader};
use vulkano::command_buffer::CopyImageToBufferInfo;

struct SceneUploader<'a> {
    renderer: &'a mut VulkanoState,
    next_mesh: &'a mut u32,
    next_texture: &'a mut u32,
}
impl MeshUploader for SceneUploader<'_> {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        let handle = MeshHandle(*self.next_mesh);
        *self.next_mesh += 1;
        self.renderer.upload_mesh(handle, mesh)?;
        Ok(handle)
    }
}
impl TextureUploader for SceneUploader<'_> {
    fn upload_texture_rgba8(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        let handle = TextureHandle(*self.next_texture);
        *self.next_texture += 1;
        self.renderer
            .upload_texture_rgba8(handle, rgba, width, height)?;
        Ok(handle)
    }
    fn upload_texture_bc7(
        &mut self,
        blocks: &[u8],
        width: u32,
        height: u32,
        srgb: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        let handle = TextureHandle(*self.next_texture);
        *self.next_texture += 1;
        self.renderer
            .upload_texture_bc7(handle, blocks, width, height, srgb)?;
        Ok(handle)
    }
}

#[test]
#[ignore = "requires Vulkan; renders the real MMS room, checks shared custom draws and animated stereo output"]
fn youtuber_room_renders_custom_wave_in_stereo() -> Result<(), Box<dyn std::error::Error>> {
    let context = RendererContext::new(None, None, None)?;
    let extent = [960, 640];
    let format = Format::B8G8R8A8_UNORM;
    let mut renderer = VulkanoState::new(
        context.clone(),
        RendererOutput::Xr(format),
        MsaaMode::Msaa4x,
    )?;
    let mut room = crate::engine::Universe::new(crate::engine::ecs::World::default());
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/youtuber-room.mms"
    ))
    .replace("\nXR.on()", "\nXR.off()");
    room.load_mms_source_at_path(&source, "examples/youtuber-room.mms")?;
    let mut next_mesh = 1000;
    let mut next_texture = 1000;
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
                    view_type: vulkano::image::view::ImageViewType::Dim2d,
                    subresource_range: ImageSubresourceRange {
                        array_layers: eye..eye + 1,
                        ..output.subresource_range()
                    },
                    ..ImageViewCreateInfo::from_image(&output)
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut frames = Vec::new();
    for frame in 0..3 {
        room.update(
            if frame == 1 { 6.0 } else { 0.0 },
            &crate::engine::user_input::InputState::default(),
        );
        room.systems.prepare_render(
            &mut room.world,
            &mut room.visuals,
            &mut room.render_assets,
            &mut SceneUploader {
                renderer: &mut renderer,
                next_mesh: &mut next_mesh,
                next_texture: &mut next_texture,
            },
            &mut room.command_queue,
        );
        let view = room.visuals.camera_view_for(CameraTarget::Window);
        let proj = room.visuals.camera_proj_for(CameraTarget::Window);
        room.visuals.set_camera_for_target(
            CameraTarget::Xr,
            (0..2)
                .map(|_| crate::engine::graphics::CameraData {
                    view,
                    proj,
                    transform: Default::default(),
                })
                .collect(),
        );
        let custom = room
            .visuals
            .materials
            .instances()
            .find(|(_, record)| !record.values.is_empty())
            .map(|(id, _)| id)
            .expect("room's shared custom instance");
        let definition = room.visuals.materials.instance(custom).unwrap().definition;
        if frame == 2 {
            let record = room.visuals.materials.instance(custom).unwrap();
            let source = record.source;
            let mut values = record.values.to_vec();
            values[4] = 0.0;
            room.visuals
                .materials
                .upsert_custom_source(source, definition, values);
        }
        if frame == 1 {
            assert!(
                room.visuals.materials.instance(custom).unwrap().values[0] > 1.0,
                "FrameTick must publish a new time value"
            );
        }
        let program = room
            .visuals
            .materials
            .definition(definition)
            .unwrap()
            .static_program;
        let shader = room
            .visuals
            .materials
            .definition(definition)
            .unwrap()
            .custom
            .as_ref()
            .unwrap()
            .clone();
        for _ in 0..200 {
            if shader.compiled.get().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        shader
            .compiled
            .get()
            .expect("compiler must finish")
            .as_ref()
            .map_err(Clone::clone)?;
        room.visuals.prepare_draw_cache();
        let count: usize = room
            .visuals
            .opaque_stream()
            .0
            .iter()
            .filter_map(|op| match op {
                crate::engine::graphics::visual_world::RenderOp::DrawBatch(batch)
                    if batch.material_instance == Some(custom) =>
                {
                    assert_eq!(
                        batch.material, program,
                        "custom instance must select its own program"
                    );
                    Some(batch.count)
                }
                _ => None,
            })
            .sum();
        assert_eq!(
            count, 4,
            "four combined LED meshes share a live custom material"
        );
        assert_eq!(
            room.visuals.emissive_draw_order().len(),
            4,
            "only LED meshes enter extraction; Anime backing stays out"
        );
        for &index in room.visuals.emissive_draw_order() {
            let visual = &room.visuals.instances()[index as usize];
            assert!(
                room.world
                    .get_component_by_id_as::<crate::engine::ecs::component::CombineMeshComponent>(
                        visual.component
                    )
                    .is_some()
            );
            assert!(room.world.children_of(visual.component).iter().all(|&child|
                room.world.get_component_by_id_as::<crate::engine::ecs::component::material::MaterialComponent>(child).is_some()),
                "source cube hierarchy must have been collapsed");
        }
        for eye in 0..2 {
            renderer.submit_xr_eye_to_view(&mut room.visuals, eye, extent, 2, eyes[eye].clone())?;
        }
        assert_eq!(renderer.custom_programs.len(), 1);
        assert!(
            renderer.custom_programs.values().all(Result::is_ok),
            "custom pipeline must be ready"
        );
        let size = (extent[0] * extent[1] * 4) as usize;
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
            (size * 2) as u64,
        )?;
        let make_readback = || {
            Buffer::new_slice::<u8>(
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
                (size * 2) as u64,
            )
        };
        let emission_readback = make_readback()?;
        let surface_readback = make_readback()?;
        let mut commands = AutoCommandBufferBuilder::primary(
            renderer.command_buffer_allocator.clone(),
            context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;
        commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
            output.clone(),
            readback.clone(),
        ))?;
        for eye in 0..2 {
            let targets = renderer
                .post_processing_renderer
                .xr_frame_targets(eye)
                .expect("Bloom targets");
            let range = (eye * size) as u64..((eye + 1) * size) as u64;
            commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
                targets
                    .bloom_source
                    .as_ref()
                    .expect("extraction target")
                    .image()
                    .clone(),
                emission_readback.clone().slice(range.clone()),
            ))?;
            commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
                targets.main_color.image().clone(),
                surface_readback.clone().slice(range),
            ))?;
        }
        let done = renderer
            .submission_future
            .take()
            .unwrap()
            .then_execute(context.graphics_queue().clone(), commands.build()?)?
            .then_signal_fence_and_flush()?;
        done.wait(None)?;
        renderer.submission_future = Some(done.boxed());
        let emission = emission_readback.read()?;
        let surface = surface_readback.read()?;
        assert_eq!(
            &emission[..size],
            &emission[size..],
            "extraction uses the same material revision in both eyes"
        );
        if frame == 2 {
            assert!(
                emission
                    .chunks_exact(4)
                    .all(|pixel| pixel[..3] == [0, 0, 0]),
                "zero authored intensity must produce no extracted light"
            );
        } else {
            let covered: Vec<_> = emission[..size]
                .chunks_exact(4)
                .zip(surface[..size].chunks_exact(4))
                .filter(|(emission, _)| emission[3] == 255)
                .collect();
            assert!(
                covered.len() > 100,
                "actual LED pixels must reach the extraction target"
            );
            assert!(
                covered.iter().all(|(emission, surface)| emission[..3]
                    .iter()
                    .zip(&surface[..3])
                    .all(|(a, b)| a.abs_diff(*b) <= 1)),
                "surface and extraction must evaluate the same brightness pattern without applying intensity twice"
            );
            assert!(
                covered
                    .iter()
                    .any(|(pixel, _)| pixel[..3].iter().copied().max().unwrap() > 180)
            );
            assert!(
                covered
                    .iter()
                    .any(|(pixel, _)| pixel[..3].iter().copied().max().unwrap() < 40)
            );
            // BGRA: t=0 is red; t=6 is orange (one eighth of the 48s hue cycle).
            assert!(
                covered.iter().all(|(pixel, _)| pixel[0] == 0
                    && (pixel[1] as f32 - pixel[2] as f32 * if frame == 0 { 0.0 } else { 0.75 })
                        .abs()
                        <= 2.0),
                "every LED must share one hue regardless of its wave brightness"
            );
        }
        let mut emission_rgba = emission[..size].to_vec();
        for pixel in emission_rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        image::save_buffer(
            format!("/tmp/youtuber-room-emission-{frame}.png"),
            &emission_rgba,
            extent[0],
            extent[1],
            image::ColorType::Rgba8,
        )?;
        let pixels = readback.read()?;
        assert_eq!(
            &pixels[..size],
            &pixels[size..],
            "matching cameras render matching eye images"
        );
        let mut rgba = pixels[..size].to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        image::save_buffer(
            format!("/tmp/youtuber-room-{frame}.png"),
            &rgba,
            extent[0],
            extent[1],
            image::ColorType::Rgba8,
        )?;
        frames.push(rgba);
    }
    let changed = frames[0]
        .chunks_exact(4)
        .zip(frames[1].chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 100,
        "the shared custom wave must change rendered pixels: {changed}"
    );
    assert!(
        renderer
            .material_instances
            .iter()
            .all(|(_, entry)| entry.custom_snapshots.len() <= MATERIAL_SNAPSHOT_LIMIT)
    );
    Ok(())
}

#[test]
#[ignore = "requires Vulkan; validates bounded custom-size buffers and retained revisions"]
fn custom_f32_snapshots_preserve_revisions_and_reuse_after_ten_thousand_updates()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::engine::graphics::{
        custom_material::{CustomMaterialDefinition, compile},
        materials::MaterialRegistry,
    };
    let context = RendererContext::new(None, None, None)?;
    let mut renderer = VulkanoState::new(
        context,
        RendererOutput::Xr(Format::B8G8R8A8_UNORM),
        MsaaMode::Off,
    )?;
    let mut world = crate::engine::ecs::World::default();
    let source = world
        .add_component(crate::engine::ecs::component::material::MaterialBaseComponent::default());
    let fields = [
        "time_seconds",
        "hue_speed",
        "wave_speed",
        "wave_count",
        "intensity",
    ]
    .map(str::to_owned)
    .to_vec();
    let shader = compile("assets/shaders/youtuber-led-wave.frag", &fields)?;
    let definition = Arc::new(CustomMaterialDefinition {
        base: crate::engine::graphics::materials::MaterialBaseKind::Surface,
        fields,
        fragment_uri: "assets/shaders/youtuber-led-wave.frag".into(),
        compiled: std::sync::OnceLock::from(Ok(shader)),
    });
    let mut visuals = VisualWorld::new();
    visuals.materials = MaterialRegistry::default();
    let definition = visuals
        .materials
        .register_custom_definition(source, definition);
    let material = visuals
        .materials
        .definition(definition)
        .unwrap()
        .static_program;
    let mut held = Vec::new();
    for revision in 0..8 {
        let id = visuals.materials.upsert_custom_source(
            source,
            definition,
            vec![revision as f32, 0.03, 0.5, 6.0, 1.0],
        );
        renderer.sync_material_instances(&visuals);
        held.push(renderer.custom_material_set(id, material)?.unwrap());
    }
    let id =
        visuals
            .materials
            .upsert_custom_source(source, definition, vec![8.0, 0.03, 0.5, 6.0, 1.0]);
    renderer.sync_material_instances(&visuals);
    assert!(renderer.custom_material_set(id, material).is_err());
    for (index, snapshot) in renderer.material_instances[id]
        .custom_snapshots
        .iter()
        .enumerate()
    {
        let bytes = snapshot.buffer.read()?;
        assert_eq!(
            f32::from_ne_bytes(bytes[0..4].try_into().unwrap()),
            index as f32
        );
    }
    held.clear();
    for edit in 0..10_000 {
        visuals.materials.upsert_custom_source(
            source,
            definition,
            vec![edit as f32, 0.03, 0.5, 6.0, 1.0],
        );
        renderer.sync_material_instances(&visuals);
        renderer.custom_material_set(id, material)?;
    }
    assert_eq!(renderer.material_upload_stats.allocations, 8);
    assert_eq!(renderer.material_instances[id].custom_snapshots.len(), 8);
    // A second authored schema has a different name/count and buffer size,
    // while retaining the same renderer-provided descriptor binding contract.
    let single = crate::engine::graphics::custom_material::compile_source(
        "#version 450\nlayout(location=0) out vec4 color; layout(set=1,binding=0) uniform Params {float gain;} p; void main(){color=vec4(p.gain);}",
        "one-f32",
        &["gain".into()],
    )?;
    let other = world
        .add_component(crate::engine::ecs::component::material::MaterialBaseComponent::default());
    let other_definition = visuals.materials.register_custom_definition(
        other,
        Arc::new(CustomMaterialDefinition {
            base: crate::engine::graphics::materials::MaterialBaseKind::Surface,
            fields: vec!["gain".into()],
            fragment_uri: "one-f32".into(),
            compiled: std::sync::OnceLock::from(Ok(single)),
        }),
    );
    let other_material = visuals
        .materials
        .definition(other_definition)
        .unwrap()
        .static_program;
    let other_id = visuals
        .materials
        .upsert_custom_source(other, other_definition, vec![0.75]);
    renderer.sync_material_instances(&visuals);
    renderer.custom_material_set(other_id, other_material)?;
    assert_eq!(renderer.custom_programs.len(), 2);
    assert_eq!(
        renderer.material_instances[other_id].custom_snapshots[0]
            .buffer
            .len(),
        16
    );
    assert_eq!(
        renderer.material_instances[id].custom_snapshots[0]
            .buffer
            .len(),
        32
    );
    assert_eq!(renderer.material_upload_stats.allocations, 9);
    Ok(())
}
