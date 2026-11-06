use super::*;
use crate::engine::graphics::{
    CameraData, GpuRenderable, MeshFactory, Transform,
    custom_material::CustomMaterialDefinition,
    materials::{MaterialBaseKind, MaterialFieldType, MaterialInputField},
};

#[test]
#[ignore = "requires Vulkan; verifies the Surface grid and named spacing updates in both eyes"]
fn surface_grid_renders_and_updates_spacing_without_legacy_grid_pipeline()
-> Result<(), Box<dyn std::error::Error>> {
    let context = RendererContext::new(None, None, None)?;
    let format = Format::B8G8R8A8_UNORM;
    let extent = [128, 128];
    let mut renderer = VulkanoState::new(
        context.clone(),
        RendererOutput::Xr(format),
        MsaaMode::Msaa4x,
    )?;
    let output = Image::new(
        context.memory_allocator().clone(),
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format,
            extent: [128, 128, 1],
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
    let fields: Vec<MaterialInputField> = [
        "spacing",
        "world_space",
        "local_extent",
        "minor_width",
        "major_width",
        "axis_width",
        "fade_start",
        "fade_end",
    ]
    .into_iter()
    .map(|name| MaterialInputField {
        name: name.into(),
        ty: if name == "local_extent" {
            MaterialFieldType::Vec2
        } else {
            MaterialFieldType::F32
        },
    })
    .collect();
    let custom = CustomMaterialDefinition::prepare(
        "assets/shaders/grid-surface.frag".into(),
        fields,
        MaterialBaseKind::Surface,
    )?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
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
    let mut visuals = VisualWorld::new();
    visuals.set_clear_color([0., 0., 0., 1.]);
    let transform = Transform::default();
    visuals.set_xr_camera(vec![
        CameraData {
            view: transform.model,
            proj: transform.model,
            transform
        };
        2
    ]);
    let source = slotmap::KeyData::from_ffi(1).into();
    let definition = visuals.materials.register_custom_definition(source, custom);
    let program = visuals
        .materials
        .definition(definition)
        .unwrap()
        .static_program;
    assert_ne!(program, crate::engine::graphics::MaterialHandle::GRID_MESH);
    let mesh = MeshHandle(1000);
    renderer.upload_mesh(mesh, &MeshFactory::quad_2d())?;
    let mut previous = None;
    for (spacing, world_space) in [(0.25, 0.), (0.5, 0.), (0.5, 1.)] {
        let id = visuals.materials.upsert_source(
            source,
            definition,
            vec![spacing, world_space, 2., 2., 1., 1.8, 2., 32., 96.],
        );
        if previous.is_none() {
            let handle = visuals.register(
                source,
                GpuRenderable::new(mesh, program),
                transform,
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
            visuals.bind_material_instance(handle, Some(id));
        }
        for eye in 0..2 {
            renderer.submit_xr_eye_to_view(&mut visuals, eye, extent, 2, eyes[eye].clone())?;
        }
        assert!(renderer.custom_programs[&program].is_ok());
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
            128 * 128 * 4 * 2,
        )?;
        let mut commands = AutoCommandBufferBuilder::primary(
            renderer.command_buffer_allocator.clone(),
            context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;
        commands.copy_image_to_buffer(
            vulkano::command_buffer::CopyImageToBufferInfo::image_buffer(
                output.clone(),
                readback.clone(),
            ),
        )?;
        let completion = renderer
            .submission_future
            .take()
            .unwrap()
            .then_execute(context.graphics_queue().clone(), commands.build()?)?
            .then_signal_fence_and_flush()?;
        completion.wait(None)?;
        renderer.submission_future = Some(completion.boxed());
        let pixels = readback.read()?.to_vec();
        for eye in 0..2 {
            let image = &pixels[eye * 128 * 128 * 4..(eye + 1) * 128 * 128 * 4];
            assert!(
                image.chunks_exact(4).filter(|pixel| pixel[0] > 20).count() > 100,
                "grid must render in eye {eye}"
            );
            assert!(
                image.chunks_exact(4).filter(|pixel| pixel[0] == 0).count() > 100,
                "grid must retain spaces between lines"
            );
        }
        if let Some(old) = previous.take() {
            assert_ne!(pixels, old, "named spacing update must change grid pixels");
        }
        previous = Some(pixels);
    }
    Ok(())
}
