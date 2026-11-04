use super::*;
use crate::engine::graphics::{MeshUploader, TextureHandle, TextureUploader};

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
        pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.texture += 1;
        let handle = TextureHandle(self.texture);
        self.renderer
            .upload_texture_rgba8(handle, pixels, width, height)?;
        Ok(handle)
    }
    fn upload_texture_bc7(
        &mut self,
        blocks: &[u8],
        width: u32,
        height: u32,
        srgb: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.texture += 1;
        let handle = TextureHandle(self.texture);
        self.renderer
            .upload_texture_bc7(handle, blocks, width, height, srgb)?;
        Ok(handle)
    }
}

#[test]
#[ignore = "requires Vulkan; checks actual planar mirror image orientation in both eyes"]
fn planar_mirror_keeps_reflected_objects_upright() -> Result<(), Box<dyn std::error::Error>> {
    let context = RendererContext::new(None, None, None)?;
    let extent = [256, 192];
    let format = Format::B8G8R8A8_UNORM;
    let mut renderer =
        VulkanoState::new(context.clone(), RendererOutput::Xr(format), MsaaMode::Off)?;
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
    let mut scene = crate::engine::Universe::new(Default::default());
    let view = openxr::View {
        pose: openxr::Posef {
            orientation: openxr::Quaternionf {
                x: 0.,
                y: 0.,
                z: 0.,
                w: 1.,
            },
            position: openxr::Vector3f {
                x: 0.,
                y: 0.,
                z: 0.,
            },
        },
        fov: openxr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.6,
            angle_down: -0.6,
        },
    };
    let mut left = view;
    let mut right = view;
    left.pose.position.x = -0.032;
    right.pose.position.x = 0.032;
    scene.systems.xr.set_external_frame(Some(
        crate::engine::ecs::system::external_xr_frame::ExternalXrFrame {
            views: Some([left, right]),
            ..Default::default()
        },
    ));
    // Objects are behind the viewer, so only their reflections can contribute
    // colored pixels. Leave the space behind the mirror open, as in the ship.
    scene.load_mms_source_at_path(
        r#"
        RendererSettings {window_visible(false)}
        BGC.rgba(0,0,0,1)
        RenderGraph.off()
        T.position(0,1.6,0).scale(4,3,1) {R.plane() {Mirror.quality(256) {}}}
        T.position(-0.65,2.35,4).scale(0.8,0.6,0.15) {
            Unlit {}
            R.cube() {C.rgba(1,0,0,1)}
        }
        T.position(0.65,0.85,4).scale(0.8,0.6,0.15) {
            Unlit {}
            R.cube() {C.rgba(0,0,1,1)}
        }
        T.position(0,1.6,3) {InputXR.on() {T {CXR {}}}}
    "#,
        "examples/_mirror_orientation_test.mms",
    )?;
    let mut upload = Upload {
        renderer: &mut renderer,
        mesh: 1000,
        texture: 1000,
    };
    for _ in 0..8 {
        scene.update(1. / 90., &Default::default());
        // Allow any explicit Unlit custom definitions to finish before drawing.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        for material in scene
            .visuals
            .materials
            .instances()
            .filter_map(|(_, record)| {
                scene
                    .visuals
                    .materials
                    .definition(record.definition)
                    .and_then(|d| d.custom.as_ref())
            })
        {
            while material.compiled.get().is_none() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut upload,
            &mut scene.command_queue,
        );
        upload
            .renderer
            .submit_xr_mirror_captures(&mut scene.visuals)?;
        for eye in 0..2 {
            upload.renderer.submit_xr_eye_to_view(
                &mut scene.visuals,
                eye,
                extent,
                2,
                eyes[eye].clone(),
            )?;
        }
    }
    let pixels_per_eye = (extent[0] * extent[1]) as usize;
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
        (pixels_per_eye * 8) as u64,
    )?;
    let mut commands = AutoCommandBufferBuilder::primary(
        renderer.command_buffer_allocator.clone(),
        context.graphics_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;
    commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
        output,
        readback.clone(),
    ))?;
    sync::now(context.device().clone())
        .then_execute(context.graphics_queue().clone(), commands.build()?)?
        .then_signal_fence_and_flush()?
        .wait(None)?;
    let bytes = readback.read()?;
    for eye in 0..2 {
        let image = &bytes[eye * pixels_per_eye * 4..(eye + 1) * pixels_per_eye * 4];
        let mut red = [0usize; 3];
        let mut blue = [0usize; 3];
        for (index, pixel) in image.chunks_exact(4).enumerate() {
            let target = if pixel[2] > 160 && pixel[0] < 80 && pixel[1] < 80 {
                &mut red
            } else if pixel[0] > 160 && pixel[2] < 80 && pixel[1] < 80 {
                &mut blue
            } else {
                continue;
            };
            target[0] += 1;
            target[1] += index % extent[0] as usize;
            target[2] += index / extent[0] as usize;
        }
        assert!(
            red[0] > 10 && blue[0] > 10,
            "eye {eye}: mirror must show both markers: {red:?} {blue:?}"
        );
        let red_y = red[2] / red[0];
        let blue_y = blue[2] / blue[0];
        assert!(
            red_y < extent[1] as usize / 2 && blue_y > extent[1] as usize / 2,
            "eye {eye}: upper red marker must remain above lower blue marker, got y={red_y}/{blue_y}"
        );
        assert!(
            red[1] / red[0] < blue[1] / blue[0],
            "preserve the mirror's horizontal orientation"
        );
    }
    Ok(())
}
