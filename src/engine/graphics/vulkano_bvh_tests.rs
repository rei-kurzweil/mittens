use super::*;
use crate::engine::graphics::{
    CameraData, CpuMesh, MeshHandle, MeshUploader, TextureUploader, Transform,
};
use crate::engine::{
    Universe,
    ecs::{World, component::*},
};
use vulkano::command_buffer::CopyImageToBufferInfo;
struct Uploader<'a> {
    renderer: &'a mut VulkanoState,
    next: u32,
}
impl MeshUploader for Uploader<'_> {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.next += 1;
        let handle = MeshHandle(self.next);
        self.renderer.upload_mesh(handle, mesh)?;
        Ok(handle)
    }
}
impl TextureUploader for Uploader<'_> {
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        unreachable!()
    }
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        unreachable!()
    }
}
#[test]
#[ignore = "requires Vulkan; verifies the desktop BVH shader and changing highlighted levels"]
fn desktop_bvh_animation_uses_custom_shader_and_dims_previous_levels()
-> Result<(), Box<dyn std::error::Error>> {
    let context = RendererContext::new(None, None, None)?;
    let extent = [256, 256];
    let format = Format::B8G8R8A8_UNORM;
    let mut renderer =
        VulkanoState::new(context.clone(), RendererOutput::Xr(format), MsaaMode::Off)?;
    let mut world = World::default();
    let editor = world.add_component(EditorComponent::new());
    for (x, height) in [(-0.3, 0.3), (0.3, 0.2)] {
        let mut pose = TransformComponent::new().with_position(x, 0., 0.5);
        pose.transform.matrix_world = pose.transform.model;
        let frame = world.add_component(pose);
        world.add_child(editor, frame)?;
        let zone = world.add_component(ZoneComponent::cube([0.2, height, 0.05]));
        world.add_child(frame, zone)?;
        let collider = world.add_component(CollidableComponent::static_());
        world.add_child(zone, collider)?;
    }
    let mut ui = EditorUIComponent::new().with_panels([]);
    ui.set_bvh_snapshot_visible(true, vec![editor]);
    let owner = world.add_component(ui);
    let mut scene = Universe::new(world);
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        &mut Uploader {
            renderer: &mut renderer,
            next: 1000,
        },
        &mut scene.command_queue,
    );
    let definition = scene.visuals.materials.source_definition(owner).unwrap();
    let custom = scene
        .visuals
        .materials
        .definition(definition)
        .unwrap()
        .custom
        .as_ref()
        .unwrap()
        .clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while custom.compiled.get().is_none() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    custom
        .compiled
        .get()
        .unwrap()
        .as_ref()
        .map_err(Clone::clone)?;
    let transform = Transform::default();
    scene.visuals.set_xr_camera(vec![
        CameraData {
            view: transform.model,
            proj: transform.model,
            transform
        };
        2
    ]);
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
    for phase in 0..4 {
        if phase == 1 {
            scene
                .world
                .get_component_by_id_as_mut::<EditorUIComponent>(owner)
                .unwrap()
                .set_bvh_animation_enabled(true);
        }
        if phase == 2 {
            std::thread::sleep(std::time::Duration::from_millis(260));
        }
        if phase == 3 {
            scene
                .world
                .get_component_by_id_as_mut::<EditorUIComponent>(owner)
                .unwrap()
                .set_bvh_animation_enabled(false);
        }
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut Uploader {
                renderer: &mut renderer,
                next: 2000,
            },
            &mut scene.command_queue,
        );
        scene.visuals.prepare_draw_cache();
        for eye in 0..2 {
            renderer.submit_xr_eye_to_view(
                &mut scene.visuals,
                eye,
                extent,
                2,
                eyes[eye].clone(),
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
        let complete = renderer
            .submission_future
            .take()
            .unwrap()
            .then_execute(context.graphics_queue().clone(), commands.build()?)?
            .then_signal_fence_and_flush()?;
        complete.wait(None)?;
        renderer.submission_future = Some(complete.boxed());
        frames.push(readback.read()?.to_vec());
    }
    assert!(
        frames[0]
            .chunks_exact(4)
            .any(|p| p[2] > 140 && p[1] > 40 && p[1] < 150 && p[0] < 70),
        "static bounds must use the orange shader rather than grey fallback"
    );
    assert_ne!(frames[0], frames[1], "the roots must visibly brighten");
    assert_ne!(
        frames[1], frames[2],
        "children must brighten after their parents dim"
    );
    assert_eq!(
        frames[0], frames[3],
        "disabling animation preserves all static boxes"
    );
    eprintln!(
        "PASS desktop BVH animation: static orange, roots/children highlight, off restores static snapshot"
    );
    Ok(())
}
