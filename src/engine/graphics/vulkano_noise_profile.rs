//! Compare a baked noise texture against procedural Perlin noise in a custom
//! material. Timings are GPU timestamp durations of the graphics pass for BOTH
//! eyes, including clear and 4x MSAA resolve. A background sphere covers every
//! pixel exactly once, so the deltas versus `flat` isolate fragment cost.
//!
//! cargo test --release --lib noise_profile -- --ignored --nocapture
//!
//! Env: NOISE_PROFILE_OUTPUT (default /tmp/noise-profile.json),
//! NOISE_PROFILE_WARMUP (24), NOISE_PROFILE_FRAMES (80), NOISE_PROFILE_REPEATS (6),
//! NOISE_PROFILE_OCTAVES (6).
use super::*;

const SHADER: &str = "assets/shaders/perlin-noise-benchmark.frag";
const PNG: &str = "assets/images/noise/perlin-1024.png";
const DDS: &str = "assets/textures/noise/perlin-1024.dds";

struct Case {
    label: &'static str,
    mode: u8,
    texture: &'static str,
    texture_sample_sites: usize,
}

const CASES: [Case; 5] = [
    Case { label: "flat", mode: 0, texture: PNG, texture_sample_sites: 0 },
    Case { label: "texture-rgba8", mode: 1, texture: PNG, texture_sample_sites: 1 },
    Case { label: "texture-bc7", mode: 1, texture: DDS, texture_sample_sites: 1 },
    Case { label: "perlin-1-octave", mode: 2, texture: PNG, texture_sample_sites: 0 },
    Case { label: "perlin-fbm", mode: 3, texture: PNG, texture_sample_sites: 0 },
];

struct Variant {
    scene: Universe,
    renderer: VulkanoState,
}

impl Variant {
    fn new(
        context: Arc<RendererContext>,
        case: &Case,
        octaves: u32,
    ) -> Result<Self, Box<dyn Error>> {
        let shader = format!("/tmp/noise-profile-{}.frag", case.label);
        let defines = format!(
            "#version 450\n#define NOISE_MODE {}\n#define NOISE_OCTAVES {octaves}",
            case.mode
        );
        std::fs::write(
            &shader,
            std::fs::read_to_string(SHADER)?.replacen("#version 450", &defines, 1),
        )?;
        // Every variant binds a texture so descriptor setup is identical; only
        // texture modes sample it.
        let source = format!(
            r#"import {{ f32, vec3 }} from "types"
            let definition = MaterialDefinition(MaterialBase())
                .fragment("{shader}")
                .inputs({{ tint: vec3, base_cell_px: f32, texture_px: f32 }})
            BG {{ T.scale(160,160,160) {{ R.sphere() {{
                Material(definition).inputs({{ tint = [1.0, 1.0, 1.0], base_cell_px = 128.0, texture_px = 1024.0 }})
                Texture.with_uri("{texture}")
                TextureFiltering.linear()
            }} }} }}
            T {{ C3D {{ fov(90) far(400) }} }}
        "#,
            texture = case.texture
        );
        let mut scene = Universe::new(Default::default());
        scene.load_mms_source_at_path(&source, "examples/_noise_profile.mms")?;
        for _ in 0..3 {
            scene.update(0., &Default::default());
        }
        let mut sample_sites = 0;
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
            sample_sites += module
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
        assert_eq!(
            sample_sites, case.texture_sample_sites,
            "{}: compiled shader has the wrong number of texture samples",
            case.label
        );
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
        let mut variant = Self { scene, renderer };
        variant.camera();
        Ok(variant)
    }

    fn camera(&mut self) {
        let mut projection = self.scene.visuals.camera_proj_for(CameraTarget::Window);
        let focal = (0.5 * 90f32.to_radians()).tan().recip();
        projection[1][1] = projection[1][1].signum() * focal;
        projection[0][0] = focal;
        let view = mat4_inverse(TransformComponent::new().transform.model).unwrap();
        self.scene.visuals.set_xr_camera(
            (0..2)
                .map(|_| CameraData {
                    view,
                    proj: projection,
                    transform: Default::default(),
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
}

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> Result<T, Box<dyn Error>>
where
    T::Err: Error + 'static,
{
    Ok(match std::env::var(name) {
        Ok(value) => value.parse()?,
        Err(_) => default,
    })
}

#[test]
#[ignore = "requires Vulkan; GPU timestamp A/B of baked noise texture versus procedural Perlin"]
fn noise_profile_texture_vs_procedural() -> Result<(), Box<dyn Error>> {
    let output_path = std::env::var("NOISE_PROFILE_OUTPUT")
        .unwrap_or_else(|_| "/tmp/noise-profile.json".into());
    let warmup: usize = env_or("NOISE_PROFILE_WARMUP", 24)?;
    let frames: usize = env_or("NOISE_PROFILE_FRAMES", 80)?;
    let repeats: usize = env_or("NOISE_PROFILE_REPEATS", 6)?;
    let octaves: u32 = env_or("NOISE_PROFILE_OCTAVES", 6)?;
    assert!(frames >= 20 && repeats >= 2 && warmup >= 8 && (1..=10).contains(&octaves));
    let context = RendererContext::new(None, None, None)?;
    let mut variants = CASES
        .iter()
        .map(|case| Variant::new(context.clone(), case, octaves))
        .collect::<Result<Vec<_>, _>>()?;
    let mut results = Vec::new();
    // Desktop reference, usual XR render extent, and Focus 3 panel extent.
    for extent in [[1920, 1080], [1920, 1920], [2448, 2448]] {
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
        let mut samples = vec![Vec::new(); variants.len()];
        let mut blocks = Vec::new();
        for round in 0..repeats {
            // Rotate, and reverse on odd rounds, so no case always follows
            // another (thermal/clock drift is spread across cases).
            let n = variants.len();
            let mut order: Vec<usize> = (0..n).map(|i| (i + round) % n).collect();
            if round % 2 == 1 {
                order.reverse();
            }
            for index in order {
                let variant = &mut variants[index];
                for _ in 0..warmup {
                    variant.frame(extent, &eyes)?;
                }
                let mut block = Vec::with_capacity(frames);
                for _ in 0..frames {
                    block.push(variant.frame(extent, &eyes)?);
                }
                eprintln!(
                    "noise {extent:?} round={round} {}: {}",
                    CASES[index].label,
                    summary(&block)
                );
                blocks.push(serde_json::json!({"round":round,"case":CASES[index].label,"summary":summary(&block)}));
                samples[index].extend(block);
            }
        }
        let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
        let flat = mean(&samples[0]);
        let pixels = 2.0 * extent[0] as f64 * extent[1] as f64;
        let mut result = serde_json::json!({"extent_per_eye":extent,"pixels_both_eyes":pixels,"blocks":blocks});
        for (index, case) in CASES.iter().enumerate() {
            let delta = mean(&samples[index]) - flat;
            result[case.label] = serde_json::json!({
                "gpu_stereo_ms": summary(&samples[index]),
                "delta_vs_flat_ms": delta,
                "delta_ns_per_pixel": delta * 1e6 / pixels,
            });
        }
        results.push(result);
    }
    let report = serde_json::json!({"gpu":variants[0].renderer.gpu_device_name(),
        "api_version":format!("{:?}",context.device().physical_device().api_version()),
        "timestamp_period_ns":context.device().physical_device().properties().timestamp_period,
        "warmup_per_block":warmup,"frames_per_block":frames,"repeats":repeats,
        "msaa":4,"eyes":2,"fbm_octaves":octaves,
        "scope":"GPU graphics-pass timestamps for a full-coverage background sphere, both eyes, including clear and MSAA resolve; no frame pacing, CPU load, post-processing, or scene geometry",
        "texture":"1024x1024 perlin fBm sampled 1:1 (texel per pixel) with repeat addressing; RGBA8 from PNG versus BC7 from DDS",
        "cases":CASES.iter().map(|c| c.label).collect::<Vec<_>>(),
        "results":results});
    std::fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
    eprintln!("wrote {output_path}");
    Ok(())
}
