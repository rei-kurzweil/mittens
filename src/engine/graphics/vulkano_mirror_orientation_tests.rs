use super::*;
use crate::engine::graphics::{MeshUploader, TextureHandle, TextureUploader};

struct Upload<'a> {
    renderer: &'a mut VulkanoState,
    mesh: &'a mut u32,
    texture: &'a mut u32,
}
impl MeshUploader for Upload<'_> {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        *self.mesh += 1;
        let handle = MeshHandle(*self.mesh);
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
        *self.texture += 1;
        let handle = TextureHandle(*self.texture);
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
        *self.texture += 1;
        let handle = TextureHandle(*self.texture);
        self.renderer
            .upload_texture_bc7(handle, blocks, width, height, srgb)?;
        Ok(handle)
    }
}

#[test]
#[ignore = "requires Vulkan; checks mirror orientation in both eyes"]
fn planar_mirror_keeps_reflected_objects_upright() -> Result<(), Box<dyn std::error::Error>> {
    mirror_fixture(false, false)
}
#[test]
#[ignore = "requires Vulkan; verifies per-mirror pause preserves stereo images and other captures"]
fn individual_mirror_pause_retains_last_images() -> Result<(), Box<dyn std::error::Error>> {
    mirror_fixture(true, false)
}
#[test]
#[ignore = "requires Vulkan; checks cube mirror UVs with inherited depth-bucket material"]
fn cube_mirror_keeps_reflected_objects_upright() -> Result<(), Box<dyn std::error::Error>> {
    mirror_fixture(false, true)
}
fn mirror_fixture(freeze: bool, cube: bool) -> Result<(), Box<dyn std::error::Error>> {
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
    // Exercise an off-axis capture, not just a centered viewer.
    let mut view = view;
    view.pose.position.x = 0.35;
    view.pose.position.y = 0.12;
    let mut left = view;
    let mut right = view;
    left.pose.position.x -= 0.032;
    right.pose.position.x += 0.032;
    scene.systems.xr.set_external_frame(Some(
        crate::engine::ecs::system::external_xr_frame::ExternalXrFrame {
            views: Some([left, right]),
            ..Default::default()
        },
    ));
    // Objects are behind the viewer, so only their reflections can contribute
    // colored pixels. Leave the space behind the mirror open, as in the ship.
    let source = r#"
        RendererSettings {window_visible(false)}
        BGC.rgba(0,0,0,1)
        RenderGraph.off()
        let mirror=Mirror.quality(256) {name="pause_target"}
        T.position(0,1.6,0).scale(4,3,1) {R.plane() {mirror}}
        // A second mirror outside the main view must continue updating.
        T.position(5,1.6,0).scale(2,2,1) {R.plane() {Mirror.quality(64) {name="other_mirror"}}}
        let upper=T.position(-0.65,2.35,4).scale(0.8,0.6,0.15) {
            Unlit {}
            R.cube() {C.rgba(1,0,0,1)}
        }
        upper
        T.position(0.65,0.85,4).scale(0.8,0.6,0.15) {
            Unlit {}
            R.cube() {C.rgba(0,0,1,1)}
        }
        let controls=T {name="mirror_test_commands"}
        controls
        on(controls,"DataEvent",fn(event) {
            if event=="Freeze" {
                mirror.set_enabled(false)
                upper.update_transform([-0.65,3.1,4],[0,0,0],[0.8,0.6,0.15])
            }
            if event=="Resume" {mirror.set_enabled(true)}
        })
        T.position(0,1.6,3) {InputXR.on() {T {CXR {}}}}
    "#;
    let source = if cube {
        source.replace(
            "T.position(0,1.6,0).scale(4,3,1) {R.plane() {mirror}}",
            "T.position(0,1.6,0).scale(4,3,0.06) {Material(MaterialDefinition.anime().opaque_bucket(\"far\")) R.cube() {mirror}}",
        )
    } else {
        source.to_owned()
    };
    scene.load_mms_source_at_path(&source, "examples/_mirror_orientation_test.mms")?;
    let mut counts = [1000, 1000];
    draw_frames(&mut renderer, &mut scene, &eyes, extent, &mut counts, 8)?;
    let pixels_per_eye = (extent[0] * extent[1]) as usize;
    let bytes = read_capture(&mut renderer, &context, &output, extent)?;
    for eye in 0..2 {
        let image = &bytes[eye * pixels_per_eye * 4..(eye + 1) * pixels_per_eye * 4];
        let mut red = [0usize; 3];
        let mut blue = [0usize; 3];
        let mut red_bounds = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        let mut blue_bounds = red_bounds;
        for (index, pixel) in image.chunks_exact(4).enumerate() {
            let (target, bounds) = if pixel[2] > 160 && pixel[0] < 80 && pixel[1] < 80 {
                (&mut red, &mut red_bounds)
            } else if pixel[0] > 160 && pixel[2] < 80 && pixel[1] < 80 {
                (&mut blue, &mut blue_bounds)
            } else {
                continue;
            };
            let x = (index % extent[0] as usize) as f32 + 0.5;
            let y = (index / extent[0] as usize) as f32 + 0.5;
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x);
            bounds[3] = bounds[3].max(y);
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
        let mirror = scene
            .visuals
            .mirrors()
            .iter()
            .find(|mirror| {
                scene.world.component_label(mirror.mirror_component) == Some("pause_target")
            })
            .unwrap();
        let camera = &scene
            .visuals
            .visual_camera(crate::engine::graphics::CameraTarget::Xr)
            .unwrap()
            .eyes[eye];
        for (center, actual) in [
            ([-0.65, 2.35, 4.0], red_bounds),
            ([0.65, 0.85, 4.0], blue_bounds),
        ] {
            let expected = reflected_marker_bounds(camera, mirror, center, extent);
            for edge in 0..4 {
                assert!(
                    (actual[edge] - expected[edge]).abs() < 2.0,
                    "eye {eye}: reflected marker position/size must match planar geometry: {actual:?} vs {expected:?}"
                );
            }
        }
    }

    if freeze {
        use crate::engine::ecs::{EventSignal, SignalEmitter};
        use crate::engine::graphics::visual_world::MirrorViewerFamily;
        let named = |label| {
            scene
                .world
                .all_components()
                .find(|&id| scene.world.component_label(id) == Some(label))
                .unwrap()
        };
        let mirror = named("pause_target");
        let control = named("mirror_test_commands");
        let targets: Vec<_> = (0..2)
            .map(|eye| {
                let key = scene
                    .visuals
                    .mirror_texture_key_for_view(mirror, MirrorViewerFamily::Stereoscopic, eye)
                    .unwrap();
                let texture = scene.visuals.runtime_texture_handle(key).unwrap();
                (texture, renderer.textures[&texture].view.clone())
            })
            .collect();
        scene.command_queue.push_event(
            control,
            EventSignal::DataEvent {
                name: "Freeze".into(),
                payload: None,
            },
        );
        let before = renderer.perf_counters().mirror_captures;
        draw_frames(&mut renderer, &mut scene, &eyes, extent, &mut counts, 3)?;
        assert_eq!(
            renderer.perf_counters().mirror_captures - before,
            6,
            "only the second mirror updates, for both eyes"
        );
        let frozen = read_capture(&mut renderer, &context, &output, extent)?;
        assert_eq!(
            bytes, frozen,
            "a disabled mirror must keep displaying its last stereo images"
        );
        for (eye, (texture, image)) in targets.iter().enumerate() {
            let key = scene
                .visuals
                .mirror_texture_key_for_view(mirror, MirrorViewerFamily::Stereoscopic, eye)
                .unwrap();
            assert_eq!(scene.visuals.runtime_texture_handle(key), Some(*texture));
            assert!(std::sync::Arc::ptr_eq(
                image,
                &renderer.textures[texture].view
            ));
        }
        // Global settings still override updating; turning them back on must
        // respect this mirror's separate disabled state.
        scene
            .systems
            .graphics_settings
            .set_mirrors_enabled(false, &mut scene.command_queue);
        let before = renderer.perf_counters().mirror_captures;
        draw_frames(&mut renderer, &mut scene, &eyes, extent, &mut counts, 1)?;
        assert_eq!(renderer.perf_counters().mirror_captures, before);
        scene
            .systems
            .graphics_settings
            .set_mirrors_enabled(true, &mut scene.command_queue);
        draw_frames(&mut renderer, &mut scene, &eyes, extent, &mut counts, 1)?;
        assert_eq!(renderer.perf_counters().mirror_captures - before, 2);
        scene.command_queue.push_event(
            control,
            EventSignal::DataEvent {
                name: "Resume".into(),
                payload: None,
            },
        );
        let before = renderer.perf_counters().mirror_captures;
        draw_frames(&mut renderer, &mut scene, &eyes, extent, &mut counts, 3)?;
        assert_eq!(renderer.perf_counters().mirror_captures - before, 12);
        assert_ne!(
            read_capture(&mut renderer, &context, &output, extent)?,
            bytes,
            "resuming captures reveals the moved marker"
        );
    }
    Ok(())
}

fn draw_frames(
    renderer: &mut VulkanoState,
    scene: &mut crate::engine::Universe,
    eyes: &[std::sync::Arc<ImageView>],
    extent: [u32; 2],
    counts: &mut [u32; 2],
    frames: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let [mesh, texture] = counts;
    let mut upload = Upload {
        renderer,
        mesh,
        texture,
    };
    for _ in 0..frames {
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
    Ok(())
}

fn read_capture(
    renderer: &mut VulkanoState,
    context: &RendererContext,
    output: &std::sync::Arc<Image>,
    extent: [u32; 2],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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
        output.clone(),
        readback.clone(),
    ))?;
    sync::now(context.device().clone())
        .then_execute(context.graphics_queue().clone(), commands.build()?)?
        .then_signal_fence_and_flush()?
        .wait(None)?;
    let bytes = readback.read()?.to_vec();
    Ok(bytes)
}

// Compare the raster against real reflected geometry, so orientation fixes cannot
// hide a stretched, shifted or sheared capture behind the correct color ordering.
fn reflected_marker_bounds(
    camera: &crate::engine::graphics::CameraData,
    mirror: &crate::engine::graphics::visual_world::VisualMirror,
    center: [f32; 3],
    extent: [u32; 2],
) -> [f32; 4] {
    use crate::utils::math::{mat4_mul_vec4, vec3_reflect_point};
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for dx in [-0.4, 0.4] {
        for dy in [-0.3, 0.3] {
            for dz in [-0.075, 0.075] {
                let point = vec3_reflect_point(
                    [center[0] + dx, center[1] + dy, center[2] + dz],
                    mirror.plane_origin,
                    mirror.plane_normal,
                );
                let clip = mat4_mul_vec4(
                    camera.proj,
                    mat4_mul_vec4(camera.view, [point[0], point[1], point[2], 1.]),
                );
                let x = (clip[0] / clip[3] + 1.) * 0.5 * extent[0] as f32;
                let y = (1. - clip[1] / clip[3]) * 0.5 * extent[1] as f32;
                bounds[0] = bounds[0].min(x);
                bounds[1] = bounds[1].min(y);
                bounds[2] = bounds[2].max(x);
                bounds[3] = bounds[3].max(y);
            }
        }
    }
    bounds
}
