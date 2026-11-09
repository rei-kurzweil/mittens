//! Compare sampled moon, analytic body, and analytic body plus glow.
//! Timings are GPU timestamp durations for BOTH eyes, including clears/resolve.
//! This isolates sky shading; it is not whole-scene or XR-compositor timing.
use super::*;
use vulkano::command_buffer::CopyImageToBufferInfo;

struct Variant {
    scene: Universe,
    renderer: VulkanoState,
    texture_sample_sites: usize,
}

impl Variant {
    fn moon_size(&mut self, scale: f32) {
        let sky =
            &self.scene.visuals.instances()[self.scene.visuals.background_order()[0] as usize];
        let material = self
            .scene
            .visuals
            .materials
            .instance(sky.material_instance.unwrap())
            .unwrap();
        let source = material.source;
        let definition = material.definition;
        let mut values = material.values.to_vec();
        let slot = self
            .scene
            .visuals
            .materials
            .definition(definition)
            .unwrap()
            .parameter_schema
            .field("moon_radius")
            .unwrap()
            .slot;
        values[slot] = 0.055 * scale;
        self.scene
            .visuals
            .materials
            .upsert_source(source, definition, values);
    }

    fn new(
        context: Arc<RendererContext>,
        shader: &str,
        texture: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let material =
            std::fs::read_to_string("assets/components/materials/kawaii-twilight-sky.mms")?
                .replace("assets/shaders/kawaii-twilight-sky.frag", shader);
        let source = format!(
            r#"{material}
            BG {{ T.scale(160,160,160) {{ R.sphere() {{
                kawaii_twilight_sky_material(false)
                Texture.with_uri("{texture}")
                TextureFiltering.linear()
            }} }} }}
            T {{ C3D {{ fov(70) far(100) }} }}
        "#
        );
        let mut scene = Universe::new(Default::default());
        scene.load_mms_source_at_path(&source, "examples/_twilight_profile.mms")?;
        for _ in 0..3 {
            scene.update(0., &Default::default());
        }
        let mut texture_sample_sites = 0;
        for (_, material) in scene.visuals.materials.instances() {
            let custom = scene
                .visuals
                .materials
                .definition(material.definition)
                .unwrap()
                .custom
                .as_ref()
                .unwrap();
            for _ in 0..500 {
                if custom.compiled.get().is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let compiled = custom
                .compiled
                .get()
                .ok_or("compiler timeout")?
                .as_ref()
                .map_err(Clone::clone)?;
            let module = rspirv::dr::load_words(&compiled.words)?;
            texture_sample_sites += module
                .functions
                .iter()
                .flat_map(|f| &f.blocks)
                .flat_map(|b| &b.instructions)
                .filter(|i| {
                    matches!(
                        i.class.opcode,
                        rspirv::spirv::Op::ImageSampleExplicitLod
                            | rspirv::spirv::Op::ImageSampleImplicitLod
                    )
                })
                .count();
        }
        let mut renderer = VulkanoState::new(
            context.clone(),
            RendererOutput::Xr(Format::B8G8R8A8_UNORM),
            MsaaMode::Msaa4x,
        )?;
        renderer.profile = Some(ProfileState::new(&context)?);
        if renderer.profile.as_ref().unwrap().pool.is_none() {
            return Err("GPU timestamps unavailable; refusing to substitute CPU timings".into());
        }
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut Uploader {
                renderer: &mut renderer,
                mesh: 1000,
                texture: 1000,
            },
            &mut scene.command_queue,
        );
        scene.visuals.prepare_draw_cache();
        assert_eq!(scene.visuals.background_order().len(), 1);
        assert!(!scene.visuals.effective_post_processing().is_active());
        Ok(Self {
            scene,
            renderer,
            texture_sample_sites,
        })
    }

    fn camera(
        &mut self,
        extent: [u32; 2],
        direction: [f32; 3],
        fov: f32,
        horizontal_fov: Option<f32>,
    ) {
        let position = [0., 60., 0.];
        let target = std::array::from_fn(|i| position[i] + direction[i]);
        let rotation = TransformComponent::look_at_world_rotation(position, target).unwrap();
        let camera = TransformComponent::new()
            .with_position(position[0], position[1], position[2])
            .with_rotation_quat(quat_mul(rotation, quat_rotation_y(std::f32::consts::PI)));
        let mut projection = self.scene.visuals.camera_proj_for(CameraTarget::Window);
        // Preserve the engine projection's Y sign; the renderer already handles
        // Vulkan viewport orientation. Negating it here flips the sky vertically.
        projection[1][1] = projection[1][1].signum() * (0.5 * fov.to_radians()).tan().recip();
        projection[0][0] = horizontal_fov.map_or_else(
            || projection[1][1].abs() * extent[1] as f32 / extent[0] as f32,
            |horizontal| (0.5 * horizontal.to_radians()).tan().recip(),
        );
        self.scene.visuals.set_xr_camera(
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
    }

    fn frame(&mut self, extent: [u32; 2], eyes: &[Arc<ImageView>]) -> Result<f64, Box<dyn Error>> {
        self.renderer.profile.as_mut().unwrap().reset_frame();
        for eye in 0..2 {
            self.renderer.submit_xr_eye_to_view(
                &mut self.scene.visuals,
                eye,
                extent,
                2,
                eyes[eye].clone(),
            )?;
        }
        self.renderer
            .profile
            .as_mut()
            .unwrap()
            .completion
            .take()
            .ok_or("missing GPU fence")?()?;
        if let Some(future) = self.renderer.submission_future.as_mut() {
            future.cleanup_finished();
        }
        Ok(self
            .renderer
            .profile
            .as_ref()
            .unwrap()
            .gpu_ms()?
            .ok_or("no timestamps")?[1])
    }

    fn capture(
        &mut self,
        context: &RendererContext,
        output: Arc<Image>,
        extent: [u32; 2],
        path: &str,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
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
            self.renderer.command_buffer_allocator.clone(),
            context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;
        commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
            output,
            readback.clone(),
        ))?;
        let done = self
            .renderer
            .submission_future
            .take()
            .unwrap()
            .then_execute(context.graphics_queue().clone(), commands.build()?)?
            .then_signal_fence_and_flush()?;
        done.wait(None)?;
        self.renderer.submission_future = Some(done.boxed());
        let pixels = readback.read()?;
        let rgba: Vec<_> = pixels[..size as usize]
            .chunks_exact(4)
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect();
        image::save_buffer(path, &rgba, extent[0], extent[1], image::ColorType::Rgba8)?;
        Ok(rgba)
    }
}

#[test]
#[ignore = "requires Vulkan; GPU timestamp A/B and captures for moon body variants"]
fn profile_twilight_moon_body() -> Result<(), Box<dyn Error>> {
    run_comparison(Comparison::Body)
}

#[test]
#[ignore = "requires Vulkan; compare baked and fully procedural moon/glow"]
fn profile_twilight_moon_glow() -> Result<(), Box<dyn Error>> {
    run_comparison(Comparison::Glow)
}

#[test]
#[ignore = "requires Vulkan; normal versus 5x moon at fixed 120H/110V test FOV"]
fn profile_twilight_moon_sizes() -> Result<(), Box<dyn Error>> {
    run_comparison(Comparison::Sizes)
}

#[test]
#[ignore = "requires Vulkan; cloud-only baseline versus procedural moon at fixed 120H/110V"]
fn profile_twilight_no_moon() -> Result<(), Box<dyn Error>> {
    run_comparison(Comparison::NoMoon)
}

#[derive(Clone, Copy)]
enum Comparison {
    Body,
    Glow,
    Sizes,
    NoMoon,
}

fn run_comparison(comparison: Comparison) -> Result<(), Box<dyn Error>> {
    let no_moon = matches!(comparison, Comparison::NoMoon);
    let compare_sizes = matches!(comparison, Comparison::Sizes);
    let wide_view = compare_sizes || no_moon;
    let include_glow = !matches!(comparison, Comparison::Body);
    let context = RendererContext::new(None, None, None)?;
    let output_path = std::env::var("TWILIGHT_PROFILE_OUTPUT").unwrap_or_else(|_| {
        if no_moon {
            "/tmp/twilight-no-moon-profile.json".into()
        } else if compare_sizes {
            "/tmp/twilight-moon-sizes-profile.json".into()
        } else if include_glow {
            "/tmp/twilight-moon-glow-profile.json".into()
        } else {
            "/tmp/twilight-moon-body-profile.json".into()
        }
    });
    let warmup: usize = std::env::var("TWILIGHT_PROFILE_WARMUP")
        .unwrap_or_else(|_| "24".into())
        .parse()?;
    let frames: usize = std::env::var("TWILIGHT_PROFILE_FRAMES")
        .unwrap_or_else(|_| "80".into())
        .parse()?;
    let repeats: usize = std::env::var("TWILIGHT_PROFILE_REPEATS")
        .unwrap_or_else(|_| if include_glow { "6".into() } else { "4".into() })
        .parse()?;
    // Symmetric vertical test FOV, not the headset's advertised binocular FOV
    // or a captured asymmetric OpenXR projection. Keep 70 for reproducibility.
    let regular_fov: f32 = std::env::var("TWILIGHT_PROFILE_FOV")
        .unwrap_or_else(|_| if wide_view { "110".into() } else { "70".into() })
        .parse()?;
    assert!((1.0..170.0).contains(&regular_fov));
    let horizontal_fov: Option<f32> = if wide_view {
        Some(
            std::env::var("TWILIGHT_PROFILE_HORIZONTAL_FOV")
                .unwrap_or_else(|_| "120".into())
                .parse()?,
        )
    } else {
        None
    };
    assert!(horizontal_fov.is_none_or(|fov| (1.0..170.0).contains(&fov)));
    assert!(frames >= 20 && repeats >= 2 && warmup >= 8);
    let original = std::fs::read_to_string("assets/shaders/kawaii-twilight-sky.frag")?;
    let labels = if no_moon {
        vec!["no-moon", "moon1x", "moon5x"]
    } else if include_glow {
        vec!["sampled", "procedural", "analytic-glow"]
    } else {
        vec!["sampled", "procedural"]
    };
    let mut variants = Vec::new();
    for (mode, label) in labels.iter().enumerate() {
        let path = format!("/tmp/twilight-{label}-profile.frag");
        let defines = format!(
            "#version 450\n#define MOON_BODY_PROCEDURAL {}\n#define MOON_GLOW_PROCEDURAL {}\n#define MOON_ENABLED {}",
            u8::from(no_moon || mode > 0),
            u8::from(no_moon || mode == 2),
            u8::from(!no_moon || mode > 0)
        );
        std::fs::write(&path, original.replacen("#version 450", &defines, 1))?;
        let variant = Variant::new(
            context.clone(),
            &path,
            if no_moon {
                "assets/textures/space/sky-cloud-mask.png"
            } else {
                "assets/textures/sky/kawaii-twilight-masks.png"
            },
        )?;
        assert_eq!(
            variant.texture_sample_sites,
            if no_moon || mode == 2 { 1 } else { 2 },
            "compiled shader must actually remove the moon lookup in analytic-glow mode"
        );
        variants.push(variant);
    }
    let mut results = Vec::new();
    let mut captures = Vec::new();
    for extent in [[960, 640], [1920, 1920]] {
        let output = Image::new(
            context.memory_allocator().clone(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::B8G8R8A8_UNORM,
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
        let cases = if no_moon {
            vec![("baseline", [-0.35, 0.30, -0.88], regular_fov, 1.0)]
        } else if compare_sizes {
            vec![
                ("moon1x", [-0.35, 0.30, -0.88], regular_fov, 1.0),
                ("moon5x", [-0.35, 0.30, -0.88], regular_fov, 5.0),
            ]
        } else {
            vec![
                ("moon", [-0.35, 0.30, -0.88], regular_fov, 1.0),
                ("away", [1., 0.3, 0.], regular_fov, 1.0),
                ("closeup", [-0.35, 0.30, -0.88], 12., 1.0),
            ]
        };
        for (view, direction, fov, moon_scale) in cases {
            for (mode, variant) in variants.iter_mut().enumerate() {
                variant.camera(extent, direction, fov, horizontal_fov);
                variant.moon_size(if no_moon && mode == 2 {
                    5.0
                } else {
                    moon_scale
                });
            }
            let mut samples = vec![Vec::new(); variants.len()];
            let mut blocks = Vec::new();
            // Balance order over all six permutations for three modes, or AB/BA
            // for two modes. Warm each block without shader/pipeline rebuilds.
            for round in 0..repeats {
                let order: Vec<usize> = if include_glow {
                    [
                        [0, 1, 2],
                        [1, 2, 0],
                        [2, 0, 1],
                        [2, 1, 0],
                        [1, 0, 2],
                        [0, 2, 1],
                    ][round % 6]
                        .to_vec()
                } else if round % 2 == 0 {
                    vec![0, 1]
                } else {
                    vec![1, 0]
                };
                for mode in order {
                    let variant = &mut variants[mode];
                    for _ in 0..warmup {
                        variant.frame(extent, &eyes)?;
                    }
                    let mut block = Vec::with_capacity(frames);
                    for _ in 0..frames {
                        block.push(variant.frame(extent, &eyes)?);
                    }
                    eprintln!(
                        "twilight {extent:?} {view} round={round} mode={mode}: {}",
                        summary(&block)
                    );
                    blocks.push(serde_json::json!({"round":round,"mode":mode,"summary":summary(&block),"gpu_stereo_ms":block}));
                    samples[mode].extend(block);
                }
            }
            let mut result = serde_json::json!({"extent_per_eye":extent,"view":view,"fov_degrees":fov,
                "vertical_fov_degrees":fov,"horizontal_fov_degrees":horizontal_fov.unwrap_or_else(||
                    (2.0*((extent[0] as f32/extent[1] as f32)*(fov.to_radians()/2.0).tan()).atan()).to_degrees()),
                "moon_angular_scale":moon_scale,"moon_angular_radius_radians":0.055*moon_scale,"blocks":blocks});
            if no_moon {
                for (mode, label) in labels.iter().enumerate() {
                    result[*label] = summary(&samples[mode]);
                }
            } else {
                result["sampled"] = summary(&samples[0]);
                result["procedural"] = summary(&samples[1]);
                if include_glow {
                    result["analytic_glow"] = summary(&samples[2]);
                }
            }
            results.push(result);
            if view != "away" {
                let mut images = Vec::new();
                for (mode, variant) in variants.iter_mut().enumerate() {
                    variant.frame(extent, &eyes)?;
                    let path = format!(
                        "/tmp/twilight-{}-fov{regular_fov}-{view}-{}x{}-{}.png",
                        if no_moon {
                            "baseline"
                        } else if compare_sizes {
                            "sizes"
                        } else if include_glow {
                            "glow"
                        } else {
                            "body"
                        },
                        extent[0],
                        extent[1],
                        labels[mode]
                    );
                    images.push(variant.capture(&context, output.clone(), extent, &path)?);
                }
                for mode in 1..images.len() {
                    let changed = images[0]
                        .chunks_exact(4)
                        .zip(images[mode].chunks_exact(4))
                        .filter(|(a, b)| a[..3] != b[..3])
                        .count();
                    let mae = images[0]
                        .iter()
                        .zip(&images[mode])
                        .map(|(a, b)| a.abs_diff(*b) as f64)
                        .sum::<f64>()
                        / images[0].len() as f64;
                    captures.push(serde_json::json!({"extent":extent,"view":view,"reference":labels[0],"compared_variant":labels[mode],"changed_pixels":changed,"rgba_mae":mae}));
                }
            }
        }
    }
    let report = serde_json::json!({"gpu":variants[0].renderer.gpu_device_name(),
        "api_version":format!("{:?}",context.device().physical_device().api_version()),
        "timestamp_period_ns":context.device().physical_device().properties().timestamp_period,
        "warmup_per_block":warmup,"frames_per_block":frames,"repeated_pairs":repeats,
        "msaa":4,"eyes":2,"scope":"GPU graphics timestamps, isolated sky including clear/resolve; no frame pacing, CPU load, bloom, or scene geometry",
        "texture":if no_moon {"original 1024x512 RGBA cloud mask with mips in all three variants; no moon samples"} else {"same 1024x512 RGBA UNORM with mips; sampled and body-only variants use a conditional moon lookup; analytic-glow variant samples only clouds"},
        "moon_modes":if no_moon {serde_json::json!([{"enabled":false,"scale":0},{"enabled":true,"scale":1},{"enabled":true,"scale":5}])} else {serde_json::Value::Null},
        "mode_labels":labels,
        "shader_texture_sample_sites":variants.iter().map(|v|v.texture_sample_sites).collect::<Vec<_>>(),
        "repeated_groups":repeats,
        "regular_fov_degrees":regular_fov,"regular_horizontal_fov_degrees":horizontal_fov,
        "fov_axis":"symmetric per eye; fov_degrees is vertical; horizontal is explicit in sizes test",
        "results":results,"captures":captures});
    std::fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
    eprintln!("wrote {output_path}");
    Ok(())
}
