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
    crate::demo_assets::install(&directory, crate::demo_assets::DemoScene::SakuraStage).unwrap();
    mittens_engine::engine::runtime_assets::set_root(&directory.join("mittens-demo")).unwrap();
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
    let back_cameras = [-0.032, 0.032].map(|x| {
        EyeCamera::from_xr(&openxr::View {
            pose: openxr::Posef {
                position: openxr::Vector3f { x, y: 0.0, z: -4.0 },
                orientation: openxr::Quaternionf {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                    w: 0.0,
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
    });
    renderer.draw(0, &universe.visuals, &back_cameras).unwrap();
    let back_pixels = target.pixels();
    for eye in 0..2 {
        let center = pixel(&back_pixels, eye, 48, 48);
        assert!(
            center[0] > 200 && center[1] < 40,
            "cube back face missing or depth wrong: {center:?}"
        );
    }
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
        .upload_texture_rgba8(&[128, 0, 255, 255], 1, 1)
        .unwrap();
    assert!(universe.visuals.update_texture(handle, Some(texture)));
    universe.visuals.prepare_draw_cache();
    renderer.draw(0, &universe.visuals, &cameras).unwrap();
    let pixels = target.pixels();
    for eye in 0..2 {
        let center = pixel(&pixels, eye, 48, 48);
        assert!(
            center[2] > 210 && center[1] == 0 && (11..=15).contains(&center[0]),
            "RGBA texture binding was not sampled: {center:?}"
        );
    }
    let checker: Vec<u8> = (0..64)
        .flat_map(|y| {
            (0..64).flat_map(move |x| {
                let v = if (x + y) % 2 == 0 { 0 } else { 255 };
                [v, v, v, 255]
            })
        })
        .collect();
    let checker_texture = renderer.upload_texture_rgba8(&checker, 64, 64).unwrap();
    assert!(
        universe
            .visuals
            .update_texture(handle, Some(checker_texture))
    );
    universe.visuals.prepare_draw_cache();
    renderer.draw(0, &universe.visuals, &cameras).unwrap();
    let pixels = target.pixels();
    for eye in 0..2 {
        let center = pixel(&pixels, eye, 48, 48);
        assert!(
            (105..=125).contains(&center[2]),
            "minified checkerboard should sample averaged mip levels: {center:?}"
        );
    }
    println!("PASS GPU minification samples averaged mip levels");
    assert!(universe.visuals.update_opacity(handle, 0.5));
    universe.visuals.prepare_draw_cache();
    renderer.draw(0, &universe.visuals, &cameras).unwrap();
    let blended = target.pixels();
    for eye in 0..2 {
        let center = pixel(&blended, eye, 48, 48);
        assert!(
            center[0] > 20 && center[2] > 20 && center[2] < 110,
            "expected blended near and far surfaces: {center:?}"
        );
    }
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
        "PASS custom scene: stereo pixels, depth occlusion, retained uniforms, RGBA texture, batched instancing, blending, stable caches, legacy rejection; {stats:?}"
    );
    // Also load the actual embedded app scene through the production wrapper.
    let mut app = crate::scene_runtime::SceneRuntime::new(
        gpu.clone(),
        vk::Format::R8G8B8A8_UNORM,
        target.extent,
        vec![target.handle()],
        crate::scene_runtime::PreparedScene::load(&directory, crate::demo_assets::DemoScene::Cubes)
            .unwrap(),
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
    drop(app);
    let configured_stage = crate::scene_runtime::PreparedScene::load(
        &directory,
        crate::demo_assets::DemoScene::SakuraStage,
    )
    .unwrap();
    assert_eq!(configured_stage.desired_xr_resolution(), None);
    let lean =
        crate::demo_assets::install(&directory, crate::demo_assets::DemoScene::SakuraLean).unwrap();
    assert!(lean.source.contains("hand(\"left\")"));
    assert!(
        !lean.source.contains("EditorUI")
            && !lean.source.contains("Zone.capsule_y")
            && !lean.source.contains("Gravity {")
    );
    let prepared_lean = crate::scene_runtime::PreparedScene::load(
        &directory,
        crate::demo_assets::DemoScene::SakuraLean,
    )
    .unwrap();
    assert_eq!(prepared_lean.desired_xr_resolution(), None);
    let mut lean_app = crate::scene_runtime::SceneRuntime::new(
        gpu.clone(),
        vk::Format::R8G8B8A8_UNORM,
        target.extent,
        vec![target.handle()],
        prepared_lean,
        &mut crate::xr_host::Report,
    )
    .unwrap();
    let mut lean_ready = false;
    for frame in 0..120 {
        if lean_app
            .draw(
                0,
                &views,
                openxr::Time::from_nanos(frame * 11_111_111),
                &mut crate::xr_host::Report,
            )
            .unwrap()
            == "custom_opaque_scene"
        {
            lean_ready = true;
        }
    }
    assert!(lean_ready, "lean scene did not render");
    drop(lean_app);
    let source =
        crate::demo_assets::install(&directory, crate::demo_assets::DemoScene::SakuraStage)
            .unwrap();
    let mut stage = Universe::new(World::default());
    stage
        .load_mms_source_at_path(&source.source, &source.path.to_string_lossy())
        .unwrap();
    let mut stage_renderer = SceneRenderer::with_samples(
        gpu,
        vk::Format::R8G8B8A8_UNORM,
        target.extent,
        vec![target.handle()],
        samples,
    )
    .unwrap();
    let mut complete = false;
    for _ in 0..300 {
        prepare(&mut stage, &mut stage_renderer);
        let compiling = stage.visuals.materials.instances().any(|(_, record)| {
            let definition = stage
                .visuals
                .materials
                .definition(record.definition)
                .unwrap();
            let custom = definition
                .custom
                .as_ref()
                .expect("stage must only use custom materials");
            match custom.compiled.get() {
                None => true,
                Some(Ok(_)) => false,
                Some(Err(e)) => panic!("stage shader: {e}"),
            }
        });
        let pistols: Vec<_> = stage
            .world
            .all_components()
            .filter_map(|id| {
                stage.world
            .get_component_by_id_as::<mittens_engine::engine::ecs::component::GLTFComponent>(id)
            })
            .collect();
        if !compiling
            && !stage.visuals.background_batches().is_empty()
            && stage.visuals.lights().len() == 3
            && pistols.len() == 3
            && pistols.iter().all(|pistol| pistol.spawned)
        {
            complete = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        complete,
        "stage did not prepare its sky, materials and lights"
    );
    let stats = stage_renderer.draw(0, &stage.visuals, &cameras).unwrap_or_else(|e| {
        for instance in stage.visuals.instances().iter().filter(|v| v.emissive != 0.0 || v.material_instance.is_none() || v.stencil_ref != 0 || v.is_stencil_clip || v.transparent_cutout) {
            let mut current = Some(instance.component);
            let mut ancestry = Vec::new();
            while let Some(id) = current { ancestry.push(format!("{:?}:{:?}", id, stage.world.component_label(id))); current = stage.world.parent_of(id); }
            eprintln!("unsupported candidate emissive={} custom={} clip={} cutout={} ancestry={ancestry:?}", instance.emissive, instance.material_instance.is_some(), instance.stencil_ref, instance.transparent_cutout);
        }
        panic!("stage draw: {e}");
    });
    assert!(
        stats.pipelines >= 4 && stats.textures >= 3 && stats.lights == 3,
        "stage resources missing: {stats:?}"
    );
    let pixels = target.pixels();
    let distinct: std::collections::HashSet<_> =
        pixels.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
    let sky = pixel(&pixels, 0, 0, 0);
    assert!(
        sky[2] > sky[0] && sky[2] > 50,
        "expected blue upper sky, got {sky:?}"
    );
    assert!(
        sky[0].abs_diff(59) > 5 || sky[1].abs_diff(82) > 5 || sky[2].abs_diff(158) > 5,
        "sky should show its gradient rather than the flat background clear: {sky:?}"
    );
    let sun = pixel(&pixels, 0, 20, 24);
    assert!(
        sun[0] > 220 && sun[1] > 200 && sun[2] > 190,
        "expected white sun disc, got {sun:?}"
    );
    assert!(distinct.len() > 10, "stage render is unexpectedly uniform");
    // Update retained lights directly after engine preparation. Both ambient
    // and direct-light data must visibly affect the actual custom pipelines.
    stage.visuals.set_ambient_light([0.8, 0.0, 0.0]);
    stage_renderer.draw(0, &stage.visuals, &cameras).unwrap();
    let ambient_pixels = target.pixels();
    assert_ne!(
        pixels, ambient_pixels,
        "ambient color did not affect the scene"
    );
    let ids: Vec<_> = stage.world.all_components().filter(|&id| {
        stage.world.get_component_by_id_as::<mittens_engine::engine::ecs::component::DirectionalLightComponent>(id).is_some()
            || stage.world.get_component_by_id_as::<mittens_engine::engine::ecs::component::PointLightComponent>(id).is_some()
    }).collect();
    assert_eq!(ids.len(), 3);
    for id in ids {
        stage.visuals.upsert_light(
            id,
            mittens_engine::engine::graphics::visual_world::VisualLight::default(),
        );
    }
    stage_renderer.draw(0, &stage.visuals, &cameras).unwrap();
    assert_ne!(
        ambient_pixels,
        target.pixels(),
        "direct lights did not affect the scene"
    );
    println!(
        "PASS Sakura stage materials, sky, lights and textures, {stats:?}, colors={}",
        distinct.len()
    );
    crate::editor_click::click_cursor_mode(&mut stage);
    stage.systems.prepare_render(
        &mut stage.world,
        &mut stage.visuals,
        &mut stage.render_assets,
        &mut stage_renderer,
        &mut stage.command_queue,
    );
    stage.visuals.prepare_draw_cache();
    stage_renderer
        .draw(0, &stage.visuals, &cameras)
        .expect("draw after clicking editor settings");
}
