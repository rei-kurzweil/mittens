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
    .replace("let desktop_view_enabled = false", "let desktop_view_enabled = true")
    .replace("let xr_view_enabled = true", "let xr_view_enabled = false")
    .replace("let pool_area = indoor_pool([0.0, 0.0, pool_room_origin_z], pool_room_width, pool_room_length, pool_width, pool_length, pool_water_enabled)", "let pool_area = T {}")
    // Keep the emission proof active while the user-facing scene is temporarily
    // configured for isolated lighting diagnostics.
    .replace("RenderGraph.off()", "RenderGraph { EmissivePass { BlurPass { radius_ndc(0.012) half_res(true) } } Bloom { intensity(0.55) radius_ndc(0.012) emissive_scale(1.0) half_res(true) } }\n")
    + "\nceiling_leds.set_input(\"intensity\", 1.0)\nmonitor_screen_content.remove_subtree()\npool_area.remove_subtree()\npool_hallway.remove_subtree()\n";
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
            .find(|(_, record)| {
                room.visuals
                    .materials
                    .definition(record.definition)
                    .is_some_and(|definition| definition.custom.is_some())
            })
            .map(|(id, _)| id)
            .expect("room's shared custom instance");
        let definition = room.visuals.materials.instance(custom).unwrap().definition;
        let fields = &room
            .visuals
            .materials
            .definition(definition)
            .unwrap()
            .custom
            .as_ref()
            .unwrap()
            .fields;
        let time_index = fields
            .iter()
            .position(|field| field.name == "time_seconds")
            .unwrap();
        let intensity_index = fields
            .iter()
            .position(|field| field.name == "intensity")
            .unwrap();
        if frame == 2 {
            let record = room.visuals.materials.instance(custom).unwrap();
            let source = record.source;
            let mut values = record.values.to_vec();
            values[intensity_index] = 0.0;
            room.visuals
                .materials
                .upsert_source(source, definition, values);
        }
        if frame == 1 {
            assert!(
                room.visuals.materials.instance(custom).unwrap().values[time_index] > 1.0,
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
    .map(crate::engine::graphics::materials::MaterialInputField::from)
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
        let id = visuals.materials.upsert_source(
            source,
            definition,
            vec![revision as f32, 0.03, 0.5, 6.0, 1.0],
        );
        renderer.sync_material_instances(&visuals);
        held.push(
            renderer
                .custom_material_set(id, material, TextureHandle(0), TextureFiltering::Linear)?
                .unwrap(),
        );
    }
    let id = visuals
        .materials
        .upsert_source(source, definition, vec![8.0, 0.03, 0.5, 6.0, 1.0]);
    renderer.sync_material_instances(&visuals);
    assert!(
        renderer
            .custom_material_set(id, material, TextureHandle(0), TextureFiltering::Linear)
            .is_err()
    );
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
        visuals
            .materials
            .upsert_source(source, definition, vec![edit as f32, 0.03, 0.5, 6.0, 1.0]);
        renderer.sync_material_instances(&visuals);
        renderer.custom_material_set(id, material, TextureHandle(0), TextureFiltering::Linear)?;
    }
    assert_eq!(renderer.material_upload_stats.allocations, 8);
    assert_eq!(renderer.material_instances[id].custom_snapshots.len(), 8);
    // A second typed schema exercises all vector widths and std140 padding
    // through actual GPU buffers/descriptors, alongside scalar snapshots.
    use crate::engine::graphics::materials::{MaterialFieldType as T, MaterialInputField};
    let mixed_fields = [
        ("gain", T::F32),
        ("tint", T::Vec3),
        ("uv", T::Vec2),
        ("rgba", T::Vec4),
    ]
    .map(|(name, ty)| MaterialInputField {
        name: name.into(),
        ty,
    })
    .to_vec();
    let single = crate::engine::graphics::custom_material::compile_source(
        "#version 450\nlayout(location=0) out vec4 color; layout(set=1,binding=0,std140) uniform Params {float gain;vec3 tint;vec2 uv;vec4 rgba;} p; void main(){color=p.rgba*vec4(p.tint*p.gain,1)+vec4(p.uv,0,0);}",
        "mixed-numeric",
        &mixed_fields,
    )?;
    let other = world
        .add_component(crate::engine::ecs::component::material::MaterialBaseComponent::default());
    let other_definition = visuals.materials.register_custom_definition(
        other,
        Arc::new(CustomMaterialDefinition {
            base: crate::engine::graphics::materials::MaterialBaseKind::Surface,
            fields: mixed_fields,
            fragment_uri: "mixed-numeric".into(),
            compiled: std::sync::OnceLock::from(Ok(single)),
        }),
    );
    let other_material = visuals
        .materials
        .definition(other_definition)
        .unwrap()
        .static_program;
    let other_id = visuals.materials.upsert_source(
        other,
        other_definition,
        vec![0.75, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9],
    );
    renderer.sync_material_instances(&visuals);
    renderer.custom_material_set(
        other_id,
        other_material,
        TextureHandle(0),
        TextureFiltering::Linear,
    )?;
    assert_eq!(renderer.custom_programs.len(), 2);
    assert_eq!(
        renderer.material_instances[other_id].custom_snapshots[0]
            .buffer
            .len(),
        64
    );
    assert_eq!(
        renderer.material_instances[id].custom_snapshots[0]
            .buffer
            .len(),
        32
    );
    assert_eq!(renderer.material_upload_stats.allocations, 9);
    // A shared material's numeric revision does not identify its per-draw
    // texture or sampler. Inspect the actual descriptor resources as well.
    let original = renderer
        .custom_material_set(
            other_id,
            other_material,
            TextureHandle(0),
            TextureFiltering::Linear,
        )?
        .unwrap();
    let image_handle = TextureHandle(90_001);
    renderer.upload_texture_rgba8(image_handle, &[210, 80, 30, 255], 1, 1)?;
    let textured = renderer
        .custom_material_set(
            other_id,
            other_material,
            image_handle,
            TextureFiltering::Nearest,
        )?
        .unwrap();
    assert!(!Arc::ptr_eq(&original, &textured));
    assert_eq!(
        renderer.material_instances[other_id].custom_snapshots.len(),
        2
    );
    let resources = textured.resources();
    let Some(vulkano::descriptor_set::DescriptorBindingResources::ImageViewSampler(images)) =
        resources.binding(1)
    else {
        panic!("material texture descriptor")
    };
    let (image, sampler) = images[0].as_ref().unwrap();
    assert!(Arc::ptr_eq(
        &image.image_view,
        &renderer.textures[&image_handle].view
    ));
    assert!(Arc::ptr_eq(sampler, &renderer.sampler_nearest));
    // Runtime captures publish a new image view under the same texture handle.
    // An unchanged numeric revision must still bind the replacement view.
    let replacement = TextureHandle(90_002);
    renderer.upload_texture_rgba8(replacement, &[30, 180, 90, 255], 1, 1)?;
    let replacement_view = renderer.textures[&replacement].view.clone();
    renderer.textures.get_mut(&image_handle).unwrap().view = replacement_view.clone();
    let replaced = renderer
        .custom_material_set(
            other_id,
            other_material,
            image_handle,
            TextureFiltering::Nearest,
        )?
        .unwrap();
    assert!(!Arc::ptr_eq(&textured, &replaced));
    let replacement_resources = replaced.resources();
    let Some(vulkano::descriptor_set::DescriptorBindingResources::ImageViewSampler(images)) =
        replacement_resources.binding(1)
    else {
        panic!("replacement image descriptor");
    };
    assert!(Arc::ptr_eq(
        &images[0].as_ref().unwrap().0.image_view,
        &replacement_view
    ));
    Ok(())
}

#[test]
#[ignore = "requires Vulkan; renders shared static/skinned Anime shading and the seated NPC in stereo"]
fn youtuber_room_shared_anime_shading_and_seated_reimu() -> Result<(), Box<dyn std::error::Error>> {
    use crate::engine::ecs::component::material::MaterialComponent;
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
    .replace(
        "let desktop_view_enabled = false",
        "let desktop_view_enabled = true",
    )
    .replace("let xr_view_enabled = true", "let xr_view_enabled = false");
    room.load_mms_source_at_path(&source, "examples/youtuber-room.mms")?;
    for _ in 0..4 {
        room.update(0.0, &crate::engine::user_input::InputState::default());
    }
    for (_, instance) in room.visuals.materials.instances() {
        if let Some(custom) = room
            .visuals
            .materials
            .definition(instance.definition)
            .and_then(|definition| definition.custom.as_ref())
        {
            for _ in 0..200 {
                if custom.compiled.get().is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            custom
                .compiled
                .get()
                .ok_or("custom compiler timed out")?
                .as_ref()
                .map_err(Clone::clone)?;
        }
    }
    let reimu = room
        .world
        .all_components()
        .find(|&id| room.world.component_label(id) == Some("room_reimu"))
        .unwrap();
    let gltf = room
        .world
        .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(reimu)
        .unwrap();
    assert!(gltf.spawned);
    let joints = gltf.armature_joint_transforms.clone();
    for (name, height) in [
        ("J_Bip_C_Hips", 0.18),
        ("J_Bip_L_Foot", 0.1035),
        ("J_Bip_R_Foot", 0.1035),
        ("J_Bip_L_Hand", 0.1991),
        ("J_Bip_R_Hand", 0.1991),
    ] {
        let joint = *joints
            .iter()
            .find(|&&id| room.world.component_label(id) == Some(name))
            .unwrap();
        let position =
            crate::engine::ecs::system::TransformSystem::world_position(&room.world, joint)
                .unwrap();
        assert!(
            (position[1] - height).abs() < 0.003,
            "{name} must retain the floor-seated pose: {position:?}"
        );
    }
    assert!(
        room.systems
            .secondary_motion
            .bound_snapshot(&room.world)
            .iter()
            .filter(|chain| chain.gltf == reimu)
            .count()
            >= 10,
        "hair, clothing, tail and bow chains must bind on the NPC"
    );
    let joint_id = |name: &str| {
        *joints
            .iter()
            .find(|&&id| room.world.component_label(id) == Some(name))
            .unwrap()
    };
    let left_foot = crate::engine::ecs::system::TransformSystem::world_position(
        &room.world,
        joint_id("J_Bip_L_Foot"),
    )
    .unwrap();
    let right_foot = crate::engine::ecs::system::TransformSystem::world_position(
        &room.world,
        joint_id("J_Bip_R_Foot"),
    )
    .unwrap();
    let spacing = left_foot
        .iter()
        .zip(right_foot)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f32>()
        .sqrt();
    assert!(
        (spacing - 0.52).abs() < 0.003,
        "the feet must retain the narrower seated stance"
    );
    for name in ["J_Bip_L_LowerArm", "J_Bip_R_LowerArm"] {
        let id = joint_id(name);
        let local = room
            .world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(id)
            .unwrap()
            .transform
            .rotation;
        let rest = room
            .world
            .children_of(id)
            .iter()
            .find_map(|&child| {
                room.world
                    .get_component_by_id_as::<crate::engine::ecs::component::BoneRestPoseComponent>(
                        child,
                    )
            })
            .unwrap();
        let flex =
            crate::utils::math::quat_mul(local, crate::utils::math::quat_conjugate(rest.rotation));
        assert!(
            flex[0] > 0.1 && flex[1].abs() < 0.001 && flex[2].abs() < 0.001,
            "{name} must flex around the bind-local positive-X hinge, not a body-space axis: {flex:?}"
        );
    }
    assert!(
        room.world.children_of(reimu).iter().any(|&id| room
            .world
            .get_component_by_id_as::<crate::engine::ecs::component::AnimationComponent>(id)
            .is_some()),
        "the GLTFInitialized callback must attach ambient eye movement"
    );
    assert!(
        !room.world.all_components().any(|id| room
            .world
            .get_component_by_id_as::<crate::engine::ecs::component::AvatarControlComponent>(id)
            .is_some()),
        "Reimu must not own the player or XR tracking"
    );
    let probe = room
        .world
        .all_components()
        .find(|&id| room.world.component_label(id) == Some("room_shading"))
        .expect("shared room shading");
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
    let mut next_mesh = 1000;
    let mut next_texture = 1000;
    let mut retained = None;
    for preview in [
        "room",
        "reimu",
        "monitor",
        "pool",
        "ladder",
        "slide",
        "slide_inside",
    ] {
        let closeup = preview != "room";
        room.update(0.0, &crate::engine::user_input::InputState::default());
        room.world
            .get_component_by_id_as_mut::<MaterialComponent>(probe)
            .unwrap()
            .values
            .insert(
                "shade_strength".into(),
                vec![if closeup { 0.8 } else { 0.6 }],
            );
        crate::engine::ecs::system::material_system::MaterialSystem::publish_source(
            &mut room.world,
            &mut room.visuals,
            probe,
        )?;
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
        assert_eq!(
            room.visuals
                .lights()
                .iter()
                .filter(|light| light.light_type == 3)
                .count(),
            6
        );
        if preview == "pool" {
            room.visuals.prepare_draw_cache();
            let water = room
                .visuals
                .instances()
                .iter()
                .find(|instance| {
                    room.world.component_label(instance.component) == Some("pool_water_surface")
                })
                .unwrap();
            let definition = room
                .visuals
                .materials
                .instance(water.material_instance.unwrap())
                .unwrap()
                .definition;
            let program = room
                .visuals
                .materials
                .definition(definition)
                .unwrap()
                .static_program;
            assert!(
                !renderer.custom_programs[&program]
                    .as_ref()
                    .unwrap()
                    .transparent_pipeline
                    .depth_stencil_state()
                    .unwrap()
                    .depth
                    .as_ref()
                    .unwrap()
                    .write_enable
            );
            assert!(water.color[3] < 1.0);
            let mut textured_materials = std::collections::HashSet::new();
            for instance in room.visuals.instances() {
                if let Some(material) = instance.material_instance {
                    if room
                        .visuals
                        .materials
                        .instance(material)
                        .and_then(|record| room.visuals.materials.definition(record.definition))
                        .and_then(|definition| definition.custom.as_ref())
                        .is_some_and(|custom| {
                            custom.fragment_uri.contains("pool-tiles")
                                || custom.fragment_uri.contains("matcap")
                        })
                    {
                        assert!(
                            instance
                                .texture
                                .is_some_and(|texture| texture != TextureHandle(0))
                        );
                        textured_materials.insert(material);
                    }
                }
            }
            assert_eq!(textured_materials.len(), 2);
        }
        let binding = room.visuals.materials.source_instance(probe).unwrap();
        assert!(
            room.visuals
                .instances()
                .iter()
                .filter(|instance| instance.material_instance == Some(binding)
                    && instance.renderable.material
                        == crate::engine::graphics::MaterialHandle::SKINNED_ANIME_MESH)
                .count()
                >= 5,
            "one retained material must shade both room and Reimu's skinned meshes"
        );
        assert_eq!(
            *retained.get_or_insert(binding),
            binding,
            "scalar updates must retain material identity"
        );
        let view = if closeup {
            use crate::engine::ecs::component::TransformComponent;
            use crate::utils::math::{mat4_inverse, quat_mul, quat_rotation_y};
            let (position, target) = if preview == "pool" {
                ([0.0, 2.1, 4.75], [0.0, 2.6, 11.8])
            } else if preview == "ladder" {
                ([0.85, 1.05, 4.95], [0.0, 0.15, 6.05])
            } else if preview == "slide" {
                ([2.4, 4.7, 10.5], [0.0, 3.6, 13.82])
            } else if preview == "slide_inside" {
                // Follow the centreline just inside the lower mouth.
                let point = |t: f32| {
                    let angle = std::f32::consts::TAU * 2.0 * t;
                    [
                        -0.70 + 1.62 * angle.cos(),
                        0.62 + 4.5696 * t,
                        4.57 + 9.25 + 1.62 * angle.sin(),
                    ]
                };
                (point(0.05), point(0.08))
            } else if preview == "monitor" {
                ([-0.30, 1.38, -0.80], [-0.30, 1.34, -1.94])
            } else {
                ([-0.75, 0.85, 1.25], [-2.08, 0.45, 0.2])
            };
            let rotation = TransformComponent::look_at_world_rotation(position, target).unwrap();
            let camera = TransformComponent::new()
                .with_position(position[0], position[1], position[2])
                .with_rotation_quat(quat_mul(rotation, quat_rotation_y(std::f32::consts::PI)));
            mat4_inverse(camera.transform.model).unwrap()
        } else {
            room.visuals.camera_view_for(CameraTarget::Window)
        };
        let proj = room.visuals.camera_proj_for(CameraTarget::Window);
        let matrix = crate::utils::math::mat4_inverse(view).unwrap();
        room.visuals.set_camera_for_target(
            CameraTarget::Xr,
            (0..2)
                .map(|_| crate::engine::graphics::CameraData {
                    view,
                    proj,
                    transform: crate::engine::graphics::primitives::Transform {
                        model: matrix,
                        matrix_world: matrix,
                        translation: [matrix[3][0], matrix[3][1], matrix[3][2]],
                        ..Default::default()
                    },
                })
                .collect(),
        );
        // Captures must follow the preview's actual eye cameras, just as
        // MirrorSystem runs after production XR camera publication.
        room.systems
            .mirror
            .refresh_captures(&mut room.world, &mut room.visuals);
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
        let captures_before = renderer.perf_counters().mirror_captures;
        renderer.submit_xr_mirror_captures(&mut room.visuals)?;
        assert_eq!(
            renderer.perf_counters().mirror_captures - captures_before,
            2,
            "one planar pool needs two eye captures, not a cubemap"
        );
        let water = room
            .visuals
            .instances()
            .iter()
            .find(|instance| {
                room.world.component_label(instance.component) == Some("pool_water_surface")
            })
            .unwrap();
        let definition = room
            .visuals
            .materials
            .definition(
                room.visuals
                    .materials
                    .instance(water.material_instance.unwrap())
                    .unwrap()
                    .definition,
            )
            .unwrap();
        assert!(
            room.visuals
                .materials
                .base(definition.base)
                .unwrap()
                .planar_reflection
        );
        assert_eq!(
            water.renderable.material, definition.static_program,
            "reflection capture must preserve the custom water shader"
        );
        for eye in 0..2 {
            renderer.submit_xr_eye_to_view(&mut room.visuals, eye, extent, 2, eyes[eye].clone())?;
            if preview == "pool" {
                let mirror = &room.visuals.mirrors()[0];
                assert!((mirror.plane_origin[1] + 0.12).abs() < 0.0001);
                assert!((mirror.plane_normal[1] - 1.0).abs() < 0.0001);
                assert!((mirror.aspect_ratio - 3.5 / 9.0).abs() < 0.0001);
                let key = room
                    .visuals
                    .mirror_texture_key_for_view(
                        mirror.mirror_component,
                        crate::engine::graphics::visual_world::MirrorViewerFamily::Stereoscopic,
                        eye,
                    )
                    .unwrap();
                let texture = room.visuals.runtime_texture_handle(key).unwrap();
                let water = room.visuals.instance(mirror.source_instance).unwrap();
                assert_eq!(water.texture, Some(texture));
                assert!(
                    renderer.material_instances[water.material_instance.unwrap()]
                        .custom_snapshots
                        .iter()
                        .any(|snapshot| snapshot.texture == texture
                            && Arc::ptr_eq(&snapshot.view, &renderer.textures[&texture].view)),
                    "each eye must sample its current reflection image view"
                );
            }
        }
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
        assert_eq!(
            &pixels[..size],
            &pixels[size..],
            "matching eye cameras must use the same material revision"
        );
        let mut rgba = pixels[..size].to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        image::save_buffer(
            match preview {
                "pool" => "/tmp/youtuber-pool-hall.png",
                "slide" => "/tmp/youtuber-plastic-slide.png",
                "slide_inside" => "/tmp/youtuber-plastic-slide-inside.png",
                "ladder" => "/tmp/youtuber-pool-ladder.png",
                "monitor" => "/tmp/youtuber-monitor-terminal.png",
                "reimu" => "/tmp/youtuber-reimu-seated.png",
                _ => "/tmp/youtuber-room-anime.png",
            },
            &rgba,
            extent[0],
            extent[1],
            image::ColorType::Rgba8,
        )?;
    }
    // A red board is toward the pool ladder; a blue board is behind the
    // viewer at the slide end. Swapping texture V reflects the wrong board.
    let marker_mesh = MeshHandle(19_000);
    renderer.upload_mesh(
        marker_mesh,
        room.render_assets
            .cpu_mesh(crate::engine::graphics::primitives::CpuMeshHandle::CUBE)
            .unwrap(),
    )?;
    for (z, color) in [(6.5, [1.0, 0.0, 0.0, 1.0]), (14.5, [0.0, 0.0, 1.0, 1.0])] {
        let component = room
            .world
            .add_component(crate::engine::ecs::component::TransformComponent::new());
        let mut model = crate::utils::math::mat4_identity();
        model[0][0] = 2.0;
        model[1][1] = 1.2;
        model[2][2] = 0.05;
        model[3] = [0.0, 2.0, z, 1.0];
        room.visuals.register(
            component,
            crate::engine::graphics::primitives::GpuRenderable::new(
                marker_mesh,
                crate::engine::graphics::MaterialHandle::UNLIT_MESH,
            ),
            crate::engine::graphics::primitives::Transform {
                model,
                matrix_world: model,
                ..Default::default()
            },
            color,
            1.0,
            false,
            false,
            false,
            false,
            false,
            0.0,
            None,
            1.0,
        );
    }
    let water_id = room
        .visuals
        .instances()
        .iter()
        .find(|instance| {
            room.world.component_label(instance.component) == Some("pool_water_surface")
        })
        .unwrap()
        .material_instance
        .unwrap();
    let record = room.visuals.materials.instance(water_id).unwrap().clone();
    let schema = room
        .visuals
        .materials
        .definition(record.definition)
        .unwrap()
        .parameter_schema
        .clone();
    let mut values = record.values.to_vec();
    // Isolate orientation from Fresnel/transmission and animated distortion.
    values[schema.field("reflection_strength").unwrap().slot] = 100.0;
    values[schema.field("wave_strength").unwrap().slot] = 0.0;
    values[schema.field("specular_strength").unwrap().slot] = 0.0;
    room.visuals
        .materials
        .upsert_source(record.source, record.definition, values);
    for (label, x, target_z, expected_red) in
        [("ladder", 0.0, 7.8, true), ("slide", 0.25, 13.3, false)]
    {
        use crate::engine::ecs::component::TransformComponent;
        use crate::utils::math::{mat4_inverse, quat_mul, quat_rotation_y};
        let position = [x, 1.3, 10.57];
        let target = [x, -0.12, target_z];
        let rotation = TransformComponent::look_at_world_rotation(position, target).unwrap();
        let camera = TransformComponent::new()
            .with_position(position[0], position[1], position[2])
            .with_rotation_quat(quat_mul(rotation, quat_rotation_y(std::f32::consts::PI)));
        let matrix = camera.transform.model;
        let view = mat4_inverse(matrix).unwrap();
        let proj = room.visuals.camera_proj_for(CameraTarget::Window);
        room.visuals.set_xr_camera(
            (0..2)
                .map(|_| crate::engine::graphics::CameraData {
                    view,
                    proj,
                    transform: crate::engine::graphics::primitives::Transform {
                        model: matrix,
                        matrix_world: matrix,
                        translation: position,
                        ..Default::default()
                    },
                })
                .collect(),
        );
        room.systems
            .mirror
            .refresh_captures(&mut room.world, &mut room.visuals);
        renderer.submit_xr_mirror_captures(&mut room.visuals)?;
        for eye in 0..2 {
            renderer.submit_xr_eye_to_view(&mut room.visuals, eye, extent, 2, eyes[eye].clone())?;
        }
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
        assert_eq!(
            &pixels[..size],
            &pixels[size..],
            "matching eye cameras must use the same material revision"
        );
        let mut rgba = pixels[..size].to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        let mut red = 0usize;
        let mut blue = 0usize;
        for pixel in rgba[(extent[0] * extent[1] / 2 * 4) as usize..].chunks_exact(4) {
            red += usize::from(pixel[0] > 160 && pixel[1] < 60 && pixel[2] < 60);
            blue += usize::from(pixel[2] > 160 && pixel[1] < 60 && pixel[0] < 60);
        }
        image::save_buffer(
            format!("/tmp/youtuber-water-reflection-{label}.png"),
            &rgba,
            extent[0],
            extent[1],
            image::ColorType::Rgba8,
        )?;
        let (front, behind) = if expected_red {
            (red, blue)
        } else {
            (blue, red)
        };
        assert!(
            front > 100 && behind < front / 8,
            "water must reflect the landmark ahead, not the one behind: view={label} red={red} blue={blue}"
        );
    }

    // At a fixed distance, a front-facing receiver must get the same Anime
    // contribution at head-on and grazing angles, while back-facing gets none.
    // Use the same real material/light-buffer path, with a receiver far outside
    // the room so its geometry and lights cannot obscure this measurement.
    let mesh = MeshHandle(20_000);
    renderer.upload_mesh(
        mesh,
        room.render_assets
            .cpu_mesh(crate::engine::graphics::primitives::CpuMeshHandle::QUAD_2D)
            .unwrap(),
    )?;
    let receiver = room
        .world
        .add_component(crate::engine::ecs::component::TransformComponent::new());
    let mut model = crate::utils::math::mat4_identity();
    model[3] = [100.0, 0.0, 0.0, 1.0];
    let handle = room.visuals.register(
        receiver,
        crate::engine::graphics::primitives::GpuRenderable::new(
            mesh,
            crate::engine::graphics::MaterialHandle::ANIME_MESH,
        ),
        crate::engine::graphics::primitives::Transform {
            model,
            matrix_world: model,
            ..Default::default()
        },
        [1.0; 4],
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
    room.visuals.bind_material_instance(handle, retained);
    let mut view = crate::utils::math::mat4_identity();
    view[3] = [-100.0, 0.0, -3.0, 1.0];
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
    let test_light = room
        .world
        .add_component(crate::engine::ecs::component::PointLightComponent::new());
    // Exercise the actual custom tile program as well as built-in Anime.
    // Disable normal/specular/rim detail to isolate the distance/cone ramp.
    let tile_definition = room
        .visuals
        .materials
        .instances()
        .find_map(|(_, record)| {
            let definition = room.visuals.materials.definition(record.definition)?;
            definition
                .custom
                .as_ref()?
                .fragment_uri
                .contains("pool-tiles")
                .then_some(record.definition)
        })
        .expect("pool tile definition");
    let schema = &room
        .visuals
        .materials
        .definition(tile_definition)
        .unwrap()
        .parameter_schema;
    let mut tile_values = vec![0.0; schema.scalar_count];
    tile_values[schema.field("tile_size").unwrap().slot] = 0.25;
    tile_values[schema.field("shininess").unwrap().slot] = 80.0;
    // Match the built-in receiver's ramp for the controlled comparison; the
    // authored pool uses its own wider transition.
    tile_values[schema.field("shade_threshold").unwrap().slot] = 0.35;
    tile_values[schema.field("lit_threshold").unwrap().slot] = 0.55;
    let tile_receiver =
        room.visuals
            .materials
            .upsert_source(receiver, tile_definition, tile_values);
    for material in [retained, Some(tile_receiver)] {
        let record = room.visuals.materials.instance(material.unwrap()).unwrap();
        let program = room
            .visuals
            .materials
            .definition(record.definition)
            .unwrap()
            .static_program;
        room.visuals.update_material(handle, program);
        room.visuals.bind_material_instance(handle, material);
        let mut samples = Vec::new();
        for (kind, offset, direction) in [
            (1, [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]),
            (1, [0.9797959, 0.0, 0.2], [0.0, 0.0, -1.0]),
            (1, [0.9797959, 0.0, -0.2], [0.0, 0.0, -1.0]),
            (3, [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]),
            (3, [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]),
            (3, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]),
            (3, [0.0, 0.0, -1.0], [0.0, 0.0, 1.0]),
            (3, [0.9797959, 0.0, 0.2], [-0.9797959, 0.0, -0.2]),
        ] {
            room.visuals.upsert_light(
                test_light,
                crate::engine::graphics::visual_world::VisualLight {
                    light_type: kind,
                    direction_ws: direction,
                    angle: 0.7,
                    penumbra: 0.2,
                    position_ws: [100.0 + offset[0], offset[1], offset[2]],
                    intensity: 0.7,
                    distance: 5.0,
                    color: [1.0; 3],
                    ..Default::default()
                },
            );
            renderer.submit_xr_eye_to_view(&mut room.visuals, 0, extent, 2, eyes[0].clone())?;
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
            let center = ((extent[1] / 2 * extent[0] + extent[0] / 2) * 4) as usize;
            samples.push(pixels[center]);
        }
        assert!(
            samples[0] > 160,
            "head-on receiver must be lit: {samples:?}"
        );
        assert!(
            samples[0].abs_diff(samples[1]) <= 3,
            "facing angle must not scale the distance-fed ramp: {samples:?}"
        );
        assert!(
            u16::from(samples[2]) + 40 < u16::from(samples[0]),
            "a back-facing receiver must stay in its shade color: {samples:?}"
        );

        assert!(
            samples[0].abs_diff(samples[3]) <= 3,
            "an aimed spotlight must feed the same distance ramp as a point light: {samples:?}"
        );
        assert!(
            u16::from(samples[4]) + 40 < u16::from(samples[3]),
            "an away-facing cone must not light the receiver: {samples:?}"
        );
        assert!(
            u16::from(samples[5]) + 40 < u16::from(samples[3]),
            "spotlight range attenuation must reduce ramp input: {samples:?}"
        );
        assert!(
            u16::from(samples[6]) + 40 < u16::from(samples[3]),
            "back-facing receivers must reject an aimed spotlight: {samples:?}"
        );
        assert!(
            samples[7].abs_diff(samples[3]) <= 3,
            "grazing but facing spotlight receivers must keep distance-only ramp strength: {samples:?}"
        );
    }

    // Verify both authored scales and their independent pixel-level effects.
    let plastic_materials: Vec<_> = room
        .visuals
        .materials
        .instances()
        .filter_map(|(_, record)| {
            let definition = room.visuals.materials.definition(record.definition)?;
            definition
                .custom
                .as_ref()?
                .fragment_uri
                .ends_with("/plastic.frag")
                .then_some(record.clone())
        })
        .collect();
    assert_eq!(
        plastic_materials.len(),
        2,
        "outer and inner share one plastic definition"
    );
    let plastic_definition = plastic_materials[0].definition;
    let definition = room
        .visuals
        .materials
        .definition(plastic_definition)
        .unwrap();
    let schema = definition.parameter_schema.clone();
    let program = definition.static_program;
    let field = |name: &str| schema.field(name).unwrap().slot;
    let outer = plastic_materials
        .iter()
        .find(|record| record.values[field("normal_sign")] > 0.0)
        .unwrap();
    let inner = plastic_materials
        .iter()
        .find(|record| record.values[field("normal_sign")] < 0.0)
        .unwrap();
    assert_eq!(inner.definition, outer.definition);
    for name in ["diffuse_scale", "specular_scale"] {
        assert_eq!(outer.values[field(name)], 1.0);
        assert!((inner.values[field(name)] - 1.0 / 3.0).abs() < 0.00001);
    }
    room.visuals.upsert_light(
        test_light,
        crate::engine::graphics::visual_world::VisualLight {
            light_type: 1,
            position_ws: [100.0, 0.0, 1.0],
            intensity: 0.7,
            distance: 5.0,
            color: [1.0; 3],
            ..Default::default()
        },
    );
    let mut plastic_samples = Vec::new();
    for (diffuse, specular) in [
        (1.0, 0.0),
        (1.0 / 3.0, 0.0),
        (0.0, 1.0),
        (0.0, 1.0 / 3.0),
        (1.0, 1.0),
        (1.0 / 3.0, 1.0 / 3.0),
    ] {
        let mut values = outer.values.to_vec();
        values[field("diffuse_scale")] = diffuse;
        values[field("specular_scale")] = specular;
        values[field("rim_strength")] = 0.0;
        let material = room
            .visuals
            .materials
            .upsert_source(receiver, plastic_definition, values);
        room.visuals.update_material(handle, program);
        room.visuals.bind_material_instance(handle, Some(material));
        renderer.submit_xr_eye_to_view(&mut room.visuals, 0, extent, 2, eyes[0].clone())?;
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
        let center = ((extent[1] / 2 * extent[0] + extent[0] / 2) * 4) as usize;
        plastic_samples.push(pixels[center]);
    }
    assert!(
        plastic_samples[0] > 30 && plastic_samples[2] > 5,
        "both plastic lobes must be visible: {plastic_samples:?}"
    );
    for index in [0, 2, 4] {
        assert!(
            (i16::from(plastic_samples[index + 1]) * 3 - i16::from(plastic_samples[index])).abs()
                <= 4,
            "diffuse, specular and combined output must independently scale by one third: {plastic_samples:?}"
        );
    }
    assert!(
        plastic_samples[4] > plastic_samples[0],
        "plastic highlights must add above diffuse: {plastic_samples:?}"
    );

    let eye = *joints
        .iter()
        .find(|&&id| room.world.component_label(id) == Some("J_Adj_L_FaceEye"))
        .unwrap();
    let before = room
        .world
        .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(eye)
        .unwrap()
        .transform
        .rotation;
    // Animation follows ClockDriver rather than update's dt. Advance a manual
    // driver through a complete idle hold instead of depending on render speed.
    struct GazeClock(std::sync::atomic::AtomicU64);
    impl crate::engine::ecs::system::ClockDriver for GazeClock {
        fn name(&self) -> &'static str {
            "gaze-test"
        }
        fn time_now_sec(&self) -> f64 {
            self.0.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0
        }
    }
    let gaze_clock = Arc::new(GazeClock(std::sync::atomic::AtomicU64::new(0)));
    room.systems.clock.set_driver(gaze_clock.clone());
    for _ in 0..12 {
        gaze_clock
            .0
            .fetch_add(250, std::sync::atomic::Ordering::Relaxed);
        room.update(0.25, &crate::engine::user_input::InputState::default());
        if room
            .world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(eye)
            .unwrap()
            .transform
            .rotation
            != before
        {
            break;
        }
    }
    assert_ne!(
        before,
        room.world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(eye)
            .unwrap()
            .transform
            .rotation,
        "idle gaze must animate the real eye joint without player tracking"
    );
    Ok(())
}

#[test]
#[ignore = "requires Vulkan; checks runtime capture storage reuse, resizing and retirement"]
fn runtime_capture_storage_reuses_and_retires_without_dropping_retained_views()
-> Result<(), Box<dyn std::error::Error>> {
    let context = RendererContext::new(None, None, None)?;
    let mut renderer = VulkanoState::new(
        context,
        RendererOutput::Xr(Format::B8G8R8A8_UNORM),
        MsaaMode::Msaa4x,
    )?;
    let mut visuals = VisualWorld::default();
    let source = TextureHandle(91_000);
    let target = TextureHandle(91_001);
    renderer.upload_texture_rgba8(source, &vec![180; 32 * 16 * 4], 32, 16)?;
    renderer.upload_texture_rgba8(target, &[0, 0, 0, 255], 1, 1)?;
    visuals.set_runtime_texture_handle("capture.mirror.test.stereo.0.color", target);
    renderer.retire_runtime_resources(&mut visuals);
    let source_view = renderer.textures[&source].view.clone();
    let retained = renderer.ensure_runtime_texture_target(target, &source_view)?;
    for _ in 0..1_000 {
        let view = renderer.ensure_runtime_texture_target(target, &source_view)?;
        assert!(
            Arc::ptr_eq(&retained, &view),
            "matching runtime publications must not allocate another image"
        );
        renderer.apply_pending_runtime_texture_updates();
    }
    let resized_source = TextureHandle(91_002);
    renderer.upload_texture_rgba8(resized_source, &vec![90; 64 * 16 * 4], 64, 16)?;
    let resized_view = renderer.textures[&resized_source].view.clone();
    let resized = renderer.ensure_runtime_texture_target(target, &resized_view)?;
    assert!(!Arc::ptr_eq(&retained, &resized));
    renderer.apply_pending_runtime_texture_updates();
    assert!(Arc::ptr_eq(
        &resized,
        &renderer.ensure_runtime_texture_target(target, &resized_view)?
    ));
    renderer.ensure_mirror_offscreen_targets(
        "capture.mirror.test.stereo.0.color",
        2,
        [32, 16],
        Format::B8G8R8A8_UNORM,
    )?;
    assert_eq!(renderer.mirror_offscreen.len(), 1);
    visuals.retain_runtime_textures(|_| false);
    renderer.retire_runtime_resources(&mut visuals);
    assert!(!renderer.textures.contains_key(&target));
    assert!(
        !renderer
            .pending_runtime_texture_updates
            .contains_key(&target)
    );
    assert!(renderer.mirror_offscreen.is_empty());
    assert!(renderer.managed_runtime_textures.is_empty());
    assert_eq!(
        retained.image().extent(),
        [32, 16, 1],
        "in-flight/external owners keep the old view alive"
    );
    Ok(())
}

#[test]
#[ignore = "requires Vulkan; renders the Sakura rooftop, city, bench and gradient/sun in stereo"]
fn sakura_rooftop_scene_renders_typed_sky_and_background_sun()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::engine::ecs::component::TransformComponent;
    use crate::utils::math::{mat4_inverse, quat_mul, quat_rotation_y};
    let context = RendererContext::new(None, None, None)?;
    let extent = [960, 640];
    let format = Format::B8G8R8A8_UNORM;
    let mut renderer = VulkanoState::new(
        context.clone(),
        RendererOutput::Xr(format),
        MsaaMode::Msaa4x,
    )?;
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sakura-drone-conversion.mms"
    ))
    .replace(
        "let desktop_view_enabled = false",
        "let desktop_view_enabled = true",
    )
    .replace("let xr_view_enabled = true", "let xr_view_enabled = false");
    let source = source + r#"
        on_global("DataEvent", fn(event) {
            if event == "PreviewCanHit" {
                let target = query("[name='sakura_range_can_2_body']")
                target.on_hit({
                    renderable = query("[name='sakura_range_can_2_body']")
                    hit_point = [-14.2, 26.56, -4.155]
                    origin = [-14.2, 26.9, -3.1]
                })
            }
        })
    "#;
    // Capturing several GPU views can outlast a transient effect. Freeze the
    // animation clock until the hit preview explicitly advances it.
    struct TargetClock(std::sync::atomic::AtomicU64);
    impl crate::engine::ecs::system::ClockDriver for TargetClock {
        fn name(&self) -> &'static str { "target-preview" }
        fn time_now_sec(&self) -> f64 {
            self.0.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0
        }
    }
    let target_clock = Arc::new(TargetClock(std::sync::atomic::AtomicU64::new(0)));
    let mut scene = crate::engine::Universe::new(crate::engine::ecs::World::default());
    scene.systems.clock.set_driver(target_clock.clone());
    scene.load_mms_source_at_path(&source, "examples/sakura-drone-conversion.mms")?;
    for _ in 0..3 {
        scene.update(0., &crate::engine::user_input::InputState::default());
    }
    for (_, instance) in scene.visuals.materials.instances() {
        if let Some(custom) = scene
            .visuals
            .materials
            .definition(instance.definition)
            .and_then(|definition| definition.custom.as_ref())
        {
            for _ in 0..200 {
                if custom.compiled.get().is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            custom
                .compiled
                .get()
                .ok_or("custom compiler timed out")?
                .as_ref()
                .map_err(Clone::clone)?;
        }
    }
    let mut next_mesh = 1000;
    let mut next_texture = 1000;
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut SceneUploader {
            renderer: &mut renderer,
            next_mesh: &mut next_mesh,
            next_texture: &mut next_texture,
        },
        &mut scene.command_queue,
    );
    scene.visuals.prepare_draw_cache();
    let pistol_grip = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("sakura_machine_pistol_placement"))
        .unwrap();
    assert_eq!(
        scene
            .world
            .all_components()
            .filter(|&id| scene
                .world
                .get_component_by_id_as::<crate::engine::ecs::component::RenderableComponent>(id)
                .is_some_and(|renderable| renderable.material_scope.is_some())
                && crate::engine::ecs::system::grabbable_system::grabbable_owner_for_hit(
                    &scene.world,
                    id
                ) == Some(pistol_grip))
            .count(),
        2,
        "both imported pistol primitives must resolve their grabbable grip root"
    );

    assert_eq!(scene.visuals.background_order().len(), 1);
    assert_eq!(
        scene.visuals.background_occluded_lit_emissive_order().len(),
        1,
        "sun participates in background extraction independently of the gradient"
    );
    let sky = &scene.visuals.instances()[scene.visuals.background_order()[0] as usize];
    let sky_definition = scene
        .visuals
        .materials
        .definition(
            scene
                .visuals
                .materials
                .instance(sky.material_instance.unwrap())
                .unwrap()
                .definition,
        )
        .unwrap();
    assert_eq!(
        sky_definition.custom.as_ref().unwrap().fragment_uri,
        "assets/shaders/sky-gradient.frag"
    );
    let tile_instances: Vec<_> = scene
        .visuals
        .instances()
        .iter()
        .filter(|instance| {
            instance
                .material_instance
                .and_then(|id| scene.visuals.materials.instance(id))
                .and_then(|record| scene.visuals.materials.definition(record.definition))
                .and_then(|definition| definition.custom.as_ref())
                .is_some_and(|custom| custom.fragment_uri == "assets/shaders/pool-tiles.frag")
        })
        .collect();
    assert!(tile_instances.len() >= 18);
    assert!(
        tile_instances
            .iter()
            .all(|instance| instance.texture.is_some()),
        "ceramics retain their packed tile image"
    );
    let metal_instances: Vec<_> = scene.visuals.instances().iter().filter(|instance| {
        instance.material_instance.and_then(|id|scene.visuals.materials.instance(id))
            .and_then(|instance|scene.visuals.materials.definition(instance.definition))
            .and_then(|definition|definition.custom.as_ref())
            .is_some_and(|shader|shader.fragment_uri=="assets/shaders/pool-metal-matcap.frag")
    }).collect();
    assert_eq!(metal_instances.len(),18,"six cans each have a colored body and two metal lids");
    assert!(metal_instances.iter().all(|instance|instance.texture.is_some()));
    let can = scene.world.all_components().find(|&id|scene.world.component_label(id)==Some("sakura_bench_can_1_body")).unwrap();
    assert!(scene.systems.bvh.raycast_renderables_candidates([-19.2,26.713,-3.5],[0.,0.,-1.],1.,0).iter().any(|(id,_)|*id==can),"can body must be a real ray target");
    let blossom_count = scene
        .visuals
        .cutout_order()
        .iter()
        .filter(|&&index| {
            let instance = &scene.visuals.instances()[index as usize];
            scene
                .world
                .children_of(instance.component)
                .iter()
                .any(|&child| {
                    scene
                        .world
                        .get_component_by_id_as::<crate::engine::ecs::component::TextureComponent>(
                            child,
                        )
                        .is_some_and(|texture| {
                            texture.uri()
                                == Some("assets/textures/cherry-blossoms/sakura-blossom-256.png")
                        })
                })
        })
        .count();
    assert_eq!(blossom_count, 3 * 27 * 3);
    let combined = |scene: &crate::engine::Universe| {
        scene
            .visuals
            .instances()
            .iter()
            .filter(|instance| {
                scene
                    .world
                    .get_component_by_id_as::<crate::engine::ecs::component::CombineMeshComponent>(
                        instance.component,
                    )
                    .is_some()
            })
            .count()
    };
    assert_eq!(
        combined(&scene),
        18,
        "two independently colored ceramic groups per building"
    );
    let check_building_rays = |scene: &crate::engine::Universe| {
        assert!(scene.systems.bvh.has_index());
        assert!(scene.systems.bvh.raycast_renderables_candidates(
            [-10.5,11.4,0.],[-1.,0.,0.],1.,0).is_empty(),
            "window openings must remain clear after visual combination");
        let hits=scene.systems.bvh.raycast_renderables_candidates(
            [-10.5,10.05,0.],[-1.,0.,0.],1.,0);
        assert!(!hits.is_empty(),"the solid ceramic wall beneath the window must be a BVH ray target");
        for (id, _) in hits {
            let primitive=scene.world.get_component_by_id_as::<crate::engine::ecs::component::RenderableComponent>(id).unwrap();
            assert!(primitive.get_handle().is_none(),"ray targets must use retained source boxes while only combined meshes draw");
        }
        for id in scene.world.all_components() {
            if let Some(combine)=scene.world.get_component_by_id_as::<crate::engine::ecs::component::CombineMeshComponent>(id) {
                assert!(combine.keep_transforms);
                let sources=scene.world.find_all_components(id,"Renderable");
                assert!(!sources.is_empty());
                for source in sources {
                    assert!(scene.systems.bvh.debug_renderable_bounds(source).is_some(),"retained source {source:?} in {:?} must enter BVH (eligible={})", scene.world.component_label(id), crate::engine::ecs::system::bvh_system::BvhSystem::renderable_is_raycastable(&scene.world,source));
                }
            }
        }
    };
    check_building_rays(&scene);
    let player = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("sakura_rooftop_building"))
        .unwrap();
    let player_meshes: Vec<_> = scene
        .world
        .all_components()
        .filter(|&id| {
            scene.world.component_label(id).is_some_and(|label| {
                label == "sakura_rooftop_building_ceramic_white"
                    || label == "sakura_rooftop_building_ceramic_trim"
            })
        })
        .map(|id| (id, scene.visuals.instance_handle_for_component(id).unwrap()))
        .collect();
    let slider = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("sakura_building_detail_slider"))
        .unwrap();
    let mut warm_meshes = None;
    for (iteration, count) in [0, 3, 8, 0, 8, 0, 8].into_iter().enumerate() {
        scene.systems.rx.push_event(
            slider,
            crate::engine::ecs::EventSignal::SliderChanged {
                slider,
                value: count as f32,
            },
        );
        scene.update(0., &crate::engine::user_input::InputState::default());
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut SceneUploader {
                renderer: &mut renderer,
                next_mesh: &mut next_mesh,
                next_texture: &mut next_texture,
            },
            &mut scene.command_queue,
        );
        assert!(scene.world.get_component_record(player).is_some());
        for &(id, handle) in &player_meshes {
            assert_eq!(
                scene.visuals.instance_handle_for_component(id),
                Some(handle),
                "player building must never regenerate"
            );
        }
        assert_eq!(
            combined(&scene),
            18,
            "old combined outputs must retire on regeneration"
        );
        // Slider callbacks publish after simulation; the following simulation
        // frame commits their queued BVH registrations before casting rays.
        scene.update(0., &crate::engine::user_input::InputState::default());
        check_building_rays(&scene);
        assert_eq!(
            scene
                .world
                .all_components()
                .filter(|&id| scene
                    .world
                    .component_label(id)
                    .is_some_and(|label| label.ends_with("_full_detail")))
                .count(),
            count + 1
        );
        assert_eq!(
            scene
                .world
                .all_components()
                .filter(|&id| scene
                    .world
                    .component_label(id)
                    .is_some_and(|label| label.ends_with("_top_two_only")))
                .count(),
            8 - count
        );
        if iteration == 2 {
            warm_meshes = Some(scene.render_assets.combined_mesh_count());
        }
        if iteration > 2 {
            assert_eq!(
                scene.render_assets.combined_mesh_count(),
                warm_meshes.unwrap(),
                "identical regenerations must reuse their immutable mesh assets"
            );
        }
        for instance in scene.visuals.instances().iter().filter(|instance| {
            scene
                .world
                .get_component_by_id_as::<crate::engine::ecs::component::CombineMeshComponent>(
                    instance.component,
                )
                .is_some()
        }) {
            assert!(
                instance.texture.is_some(),
                "regenerated outputs must retain ceramic textures"
            );
        }
    }
    let toggle = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("sakura_scene_bloom_toggle"))
        .unwrap();
    for enabled in [false, true] {
        scene.systems.rx.push_event(
            toggle,
            crate::engine::ecs::EventSignal::ToggleChanged {
                toggle,
                value: enabled,
            },
        );
        scene.update(0., &crate::engine::user_input::InputState::default());
        assert_eq!(
            scene.visuals.post_processing().enabled,
            enabled,
            "scene bloom control must update the actual renderer graph"
        );
    }
    scene.visuals.prepare_draw_cache();
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
    let projection = scene.visuals.camera_proj_for(CameraTarget::Window);
    for (label, position, target) in [
        ("rooftop", [-12.8, 28.4, 4.2], [-18., 26.5, -3.3]),
        ("city", [0., 35., 52.0], [0., 13., 0.]),
        ("bench", [-18., 27.0, -2.45], [-18., 26.65, -4.1]),
        ("targets", [-14.2, 26.9, -3.1], [-14.2, 26.56, -4.2]),
        ("tree", [-13.0, 27.8, 5.3], [-14.1, 27.7, 2.4]),
        ("sky", [0., 60., 0.], [-7., 66., -17.6]),
    ] {
        if label == "targets" {
            scene.systems.rx.push_event(
                crate::engine::ecs::ComponentId::default(),
                crate::engine::ecs::EventSignal::DataEvent {
                    name: "PreviewCanHit".into(), payload: None,
                },
            );
            for millis in [0, 0, 150] {
                target_clock.0.store(millis, std::sync::atomic::Ordering::Relaxed);
                scene.update(0., &crate::engine::user_input::InputState::default());
            }
            assert!(scene.world.all_components().any(|id| scene.world.component_label(id) == Some("hit_number_text")));
            scene.systems.prepare_render(
                &mut scene.world, &mut scene.visuals, &mut scene.render_assets,
                &mut SceneUploader {
                    renderer: &mut renderer, next_mesh: &mut next_mesh, next_texture: &mut next_texture,
                },
                &mut scene.command_queue,
            );
            for instance in scene.visuals.instances() {
                let mut node = Some(instance.component);
                while let Some(id) = node {
                    if matches!(scene.world.component_label(id), Some("hit_number_text" | "sakura_can_hit_readout")) {
                        let column = instance.transform.matrix_world[0];
                        let width = (column[0]*column[0]+column[1]*column[1]+column[2]*column[2]).sqrt();
                        assert!(width < 0.1, "hit feedback glyphs must remain small in world space: {width}");
                        break;
                    }
                    node = scene.world.parent_of(id);
                }
            }
        }
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
                    crate::engine::graphics::CameraData {
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
        let emission = Buffer::new_slice::<u8>(
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
            size,
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
        if label == "sky" {
            let source = renderer
                .post_processing_renderer
                .xr_frame_targets(0)
                .unwrap()
                .bloom_source
                .as_ref()
                .unwrap()
                .image()
                .clone();
            commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
                source,
                emission.clone(),
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
        let pixels = readback.read()?;
        let rgb: Vec<u8> = pixels[..size as usize]
            .chunks_exact(4)
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect();
        image::save_buffer(
            format!("/tmp/sakura-{label}.png"),
            &rgb,
            extent[0],
            extent[1],
            image::ColorType::Rgba8,
        )?;
        if label == "sky" {
            assert!(
                emission
                    .read()?
                    .chunks_exact(4)
                    .filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0)
                    .count()
                    > 100,
                "sun must reach bloom extraction"
            );
            let top = &rgb[40 * extent[0] as usize * 4..41 * extent[0] as usize * 4];
            let bottom = &rgb[600 * extent[0] as usize * 4..601 * extent[0] as usize * 4];
            assert_ne!(top, bottom, "two-stop sky must vary with viewing direction");
        }
    }
    assert!(renderer.custom_programs.values().all(Result::is_ok));
    Ok(())
}
