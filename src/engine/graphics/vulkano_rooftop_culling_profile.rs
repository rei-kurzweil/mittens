//! Fixed-camera A/B of the actual rooftop scene. Timings cover both eye passes.
use super::*;
use crate::engine::ecs::system::camera_system::Camera3D;
use vulkano::image::view::ImageViewType;

#[test]
#[ignore = "requires Vulkan; times the Twilight Rooftop scene at two stereo viewpoints"]
fn profile_rooftop_backface_culling() -> Result<(), Box<dyn Error>> {
    let extent = [1920, 1920];
    let warmup = 80;
    let frames = 160;
    let format = Format::B8G8R8A8_UNORM;
    let context = RendererContext::new(None, None, None)?;
    let mut renderer = VulkanoState::new(
        context.clone(),
        RendererOutput::Xr(format),
        MsaaMode::Msaa4x,
    )?;
    renderer.profile = Some(ProfileState::new(&context)?);

    let mut scene = Universe::new(Default::default());
    let path = "examples/twilight_rooftop.mms";
    let source = std::fs::read_to_string(path)?
        .replace("if use_xr { XR.on() } else { XR.off() }", "XR.off()")
        .replace(
            "let microphone = AudioInput {}",
            "let microphone = AudioOscillator.square() { enabled(false) amplitude(0.0) }",
        );
    scene.load_mms_source_at_path(&source, path)?;
    let mut uploader = Uploader {
        renderer: &mut renderer,
        mesh: 1000,
        texture: 1000,
    };
    for _ in 0..4 {
        scene.update(1.0 / 75.0, &Default::default());
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut uploader,
            &mut scene.command_queue,
        );
    }
    for (_, instance) in scene.visuals.materials.instances() {
        if let Some(custom) = scene
            .visuals
            .materials
            .definition(instance.definition)
            .and_then(|definition| definition.custom.as_ref())
        {
            let started = Instant::now();
            while custom.compiled.get().is_none() && started.elapsed() < Duration::from_secs(30) {
                std::thread::sleep(Duration::from_millis(5));
            }
            custom
                .compiled
                .get()
                .ok_or("shader compile timeout")?
                .as_ref()
                .map_err(Clone::clone)?;
        }
    }

    let output = Image::new(
        context.memory_allocator().clone(),
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format,
            extent: [extent[0], extent[1], 1],
            array_layers: 2,
            usage: ImageUsage::COLOR_ATTACHMENT,
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

    for (view, position, target) in [
        ("rooftop", [-15.5, 18.0, 4.0], [-17.0, 14.8, -14.0]),
        ("pool-floor", [-14.5, 8.1, 3.0], [-22.0, 7.4, -4.0]),
    ] {
        let rotation = TransformComponent::look_at_world_rotation(position, target).unwrap();
        let camera = TransformComponent::new()
            .with_position(position[0], position[1], position[2])
            .with_rotation_quat(quat_mul(rotation, quat_rotation_y(std::f32::consts::PI)));
        let projection = Camera3D::perspective_rh_zo(80_f32.to_radians(), 1.0, 0.05, 100.0);
        let cameras = (0..2)
            .map(|eye| {
                let mut model = camera.transform.model;
                for axis in 0..3 {
                    model[3][axis] += model[0][axis] * if eye == 0 { -0.032 } else { 0.032 };
                }
                CameraData {
                    view: mat4_inverse(model).unwrap(),
                    proj: projection,
                    transform: camera.transform,
                }
            })
            .collect();
        scene.visuals.set_xr_camera(cameras);

        // Freeze simulation once the scene is loaded so both runs draw the same
        // geometry, materials, and poses, with no network or headset pacing.
        let mut graphics = Vec::with_capacity(frames);
        for frame in 0..warmup + frames {
            renderer.profile.as_mut().unwrap().reset_frame();
            for (eye, image) in eyes.iter().enumerate() {
                renderer.submit_xr_eye_to_view(
                    &mut scene.visuals,
                    eye,
                    extent,
                    2,
                    image.clone(),
                )?;
            }
            renderer
                .profile
                .as_mut()
                .unwrap()
                .completion
                .take()
                .ok_or("missing GPU fence")?()?;
            if let Some(future) = renderer.submission_future.as_mut() {
                future.cleanup_finished();
            }
            if frame >= warmup {
                let gpu = renderer
                    .profile
                    .as_ref()
                    .unwrap()
                    .gpu_ms()?
                    .ok_or("no GPU timestamps")?;
                graphics.push(gpu[1]);
            }
        }
        println!(
            "ROOFTOP_CULLING_RESULT {}",
            serde_json::json!({
                "view": view,
                "culling": std::env::var_os("TWILIGHT_PROFILE_DISABLE_BACKFACE_CULLING").is_none(),
                "gpu": renderer.gpu_device_name(),
                "extent_per_eye": extent,
                "msaa": 4,
                "frames": frames,
                "warmup": warmup,
                "gpu_graphics_ms": summary(&graphics),
                "draws": renderer.profile.as_ref().unwrap().draws,
            })
        );
    }
    Ok(())
}
