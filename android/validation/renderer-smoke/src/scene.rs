use crate::{
    graphics_device,
    renderer::{SceneRenderer, view::EyeCamera},
    target::Target,
};
use ash::vk;
use mittens_engine::engine::{
    Universe,
    ecs::World,
    graphics::{TextureUploader, visual_world::VisualWorld},
    user_input::InputState,
};

fn prepare(universe: &mut Universe, renderer: &mut SceneRenderer) {
    universe.update(1.0 / 90.0, &InputState::default());
    universe.systems.prepare_render(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        renderer,
        &mut universe.command_queue,
    );
    universe.visuals.prepare_draw_cache();
}
fn cameras() -> [EyeCamera; 2] {
    [-0.032, 0.032].map(|x| {
        EyeCamera::from_xr(&openxr::View {
            pose: openxr::Posef {
                position: openxr::Vector3f { x, y: 0.0, z: 0.0 },
                orientation: openxr::Quaternionf {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
            },
            fov: openxr::Fovf {
                angle_left: -0.6,
                angle_right: 0.6,
                angle_down: -0.6,
                angle_up: 0.6,
            },
        })
        .unwrap()
    })
}
fn pixel(pixels: &[u8], eye: usize, x: usize, y: usize) -> &[u8] {
    &pixels[(eye * 96 * 96 + y * 96 + x) * 4..][..4]
}
pub fn run(samples: vk::SampleCountFlags) {
    let gpu = graphics_device::headless(false);
    let target = Target::new(gpu.clone());
    let mut renderer = SceneRenderer::with_samples(
        gpu.clone(),
        vk::Format::R8G8B8A8_UNORM,
        target.extent,
        vec![target.handle()],
        samples,
    )
    .unwrap();
    println!("{}", renderer.attachment_policy());
    let directory =
        std::env::temp_dir().join(format!("mittens-mobile-scene-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let fragment = directory.join("albedo.frag");
    std::fs::write(&fragment, include_str!("../../../assets/albedo.frag")).unwrap();
    let mut universe = Universe::new(World::default());
    // Fully overlapping near/far cubes make depth test behavior observable,
    // regardless of the engine's material ordering. Duplicate the near cube
    // off-center with one shared retained material to exercise instancing.
    let source = format!(
        r#"
import {{ f32, vec3 }} from "types"
let definition = MaterialDefinition(MaterialBase()).fragment("{}").inputs({{ tint: vec3, gain: f32 }})
T.position(0.0, 0.0, -2.0).scale(0.65, 0.65, 0.65) {{
    Material(definition).inputs({{ tint = [0.1, 0.8, 0.2], gain = 1.0 }}) {{
        R.cube()
        T.position(1.8, 0.0, 0.0) {{ R.cube() }}
    }}
}}
T.position(0.0, 0.0, -3.0).scale(0.65, 0.65, 0.65) {{ R.cube() {{ Material(definition).inputs({{ tint = [0.9, 0.1, 0.1], gain = 1.0 }}) }} }}
"#,
        fragment.display()
    );
    universe
        .load_mms_source_at_path(&source, &directory.join("scene.mms").to_string_lossy())
        .unwrap();
    for _ in 0..100 {
        prepare(&mut universe, &mut renderer);
        let pending = universe.visuals.materials.instances().any(|(_, record)| {
            universe
                .visuals
                .materials
                .definition(record.definition)
                .unwrap()
                .custom
                .as_ref()
                .unwrap()
                .compiled
                .get()
                .is_none()
        });
        if !pending && universe.visuals.instances().len() == 3 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(universe.visuals.instances().len(), 3);
    let cameras = cameras();
    let stats = renderer.draw(0, &universe.visuals, &cameras).unwrap();
    assert_eq!(stats.pipelines, 1);
    assert_eq!(stats.bindings, 2);
    assert_eq!(stats.meshes, 1);
    assert_eq!(
        stats.draws, 4,
        "two material batches per eye, not one draw per instance"
    );
    assert_eq!(stats.instances, 6);
    let pixels = target.pixels();
    let solid = [
        pixel(&pixels, 0, 0, 0),
        pixel(&pixels, 0, 48, 48),
        &[229, 25, 25, 255],
    ];
    let partial = pixels
        .chunks_exact(4)
        .filter(|pixel| {
            !solid.iter().any(|color| {
                pixel
                    .iter()
                    .zip(color.iter())
                    .all(|(&a, &b)| a.abs_diff(b) <= 2)
            })
        })
        .count();
    if samples == vk::SampleCountFlags::TYPE_4 {
        assert!(
            partial > 0,
            "4x resolve should produce partially covered edge pixels"
        );
    } else {
        assert_eq!(
            partial, 0,
            "1x flat colors should have hard geometric edges"
        );
    }
    println!(
        "PASS sample_count={} partial_coverage_pixels={partial}",
        samples.as_raw()
    );
    for eye in 0..2 {
        let center = pixel(&pixels, eye, 48, 48);
        assert!(
            center[1] > 180 && center[0] < 40,
            "near green surface should occlude far red: eye={eye} {center:?}"
        );
    }
    assert_ne!(
        &pixels[..96 * 96 * 4],
        &pixels[96 * 96 * 4..],
        "per-eye views must produce stereo parallax"
    );
    // Change retained numeric values without rebuilding the definition/pipeline.
    let (id, record) = universe
        .visuals
        .materials
        .instances()
        .find(|(_, record)| {
            universe
                .visuals
                .materials
                .definition(record.definition)
                .unwrap()
                .parameter_schema
                .read(&record.values, "tint")
                .unwrap()[1]
                > 0.5
        })
        .unwrap();
    let record = record.clone();
    let schema = &universe
        .visuals
        .materials
        .definition(record.definition)
        .unwrap()
        .parameter_schema;
    let mut values = record.values.to_vec();
    let tint = schema.field("tint").unwrap().slot;
    values[tint..tint + 3].copy_from_slice(&[0.1, 0.1, 0.9]);
    universe
        .visuals
        .materials
        .upsert_source(record.source, record.definition, values);
    assert_eq!(
        universe.visuals.materials.source_instance(record.source),
        Some(id)
    );
    for _ in 0..8 {
        let next = renderer.draw(0, &universe.visuals, &cameras).unwrap();
        assert_eq!(
            (
                next.pipelines,
                next.bindings,
                next.meshes,
                next.textures,
                next.instance_capacity
            ),
            (
                stats.pipelines,
                stats.bindings,
                stats.meshes,
                stats.textures,
                stats.instance_capacity
            )
        );
    }
    let pixels = target.pixels();
    for eye in 0..2 {
        let center = pixel(&pixels, eye, 48, 48);
        assert!(
            center[2] > 210 && center[1] < 40,
            "retained uniform revision not visible: {center:?}"
        );
    }
    let component = universe
        .visuals
        .instances()
        .iter()
        .find(|instance| {
            instance.transform.matrix_world[3][2] > -2.5
                && instance.transform.matrix_world[3][0].abs() < 0.1
        })
        .unwrap()
        .component;
    let handle = universe
        .world
        .get_component_by_id_as::<mittens_engine::engine::ecs::component::RenderableComponent>(
            component,
        )
        .unwrap()
        .get_handle()
        .unwrap();
    let texture = renderer
        .upload_texture_rgba8(&[255, 0, 255, 255], 1, 1)
        .unwrap();
    assert!(universe.visuals.update_texture(handle, Some(texture)));
    universe.visuals.prepare_draw_cache();
    renderer.draw(0, &universe.visuals, &cameras).unwrap();
    let pixels = target.pixels();
    for eye in 0..2 {
        let center = pixel(&pixels, eye, 48, 48);
        assert!(
            center[2] > 210 && center[1] == 0,
            "RGBA texture binding was not sampled: {center:?}"
        );
    }
    assert!(universe.visuals.update_opacity(handle, 0.5));
    universe.visuals.prepare_draw_cache();
    assert!(
        renderer
            .draw(0, &universe.visuals, &cameras)
            .unwrap_err()
            .contains("opacity=0.5")
    );
    assert!(universe.visuals.update_opacity(handle, 1.0));
    universe.visuals.prepare_draw_cache();
    // Legacy geometry cannot silently select a fallback custom pipeline.
    let mut legacy = Universe::new(World::default());
    legacy
        .load_mms_source_at_path(
            "T.position(0.0, 0.0, -2.0) { R.cube() }",
            &directory.join("legacy.mms").to_string_lossy(),
        )
        .unwrap();
    prepare(&mut legacy, &mut renderer);
    let error = renderer.draw(0, &legacy.visuals, &cameras).unwrap_err();
    assert!(error.contains("custom"), "{error}");
    renderer.draw(0, &VisualWorld::new(), &cameras).unwrap();
    println!(
        "PASS custom scene: stereo pixels, depth occlusion, retained uniforms, RGBA texture, batched instancing, stable caches, transparency/legacy rejection; {stats:?}"
    );
    // Also load the actual embedded app scene through the production wrapper.
    let mut app = crate::scene_runtime::SceneRuntime::new(
        gpu,
        vk::Format::R8G8B8A8_UNORM,
        target.extent,
        vec![target.handle()],
        &directory,
        &mut crate::xr_host::Report,
    )
    .unwrap();
    let views = [openxr::View {
        pose: openxr::Posef::IDENTITY,
        fov: openxr::Fovf {
            angle_left: -0.6,
            angle_right: 0.6,
            angle_up: 0.6,
            angle_down: -0.6,
        },
    }; 2];
    let mut ready = false;
    for frame in 0..100 {
        if app
            .draw(
                0,
                &views,
                openxr::Time::from_nanos(frame * 11_111_111),
                &mut crate::xr_host::Report,
            )
            .unwrap()
            == "custom_opaque_scene"
        {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(ready, "embedded app scene did not render");
    println!("PASS embedded app MMS scene rendered by production SceneRuntime");
}
