//! Real mobile-renderer check: a cut must expose the rear object, preserve its
//! surrounding custom material and release mesh resources without validation errors.
use crate::{
    graphics_device,
    renderer::{SceneRenderer, view::EyeCamera},
    target::Target,
};
use ash::vk;
use mittens_engine::engine::{
    Universe,
    ecs::{World, component::DamageComponent},
    user_input::InputState,
};
fn prepare(scene: &mut Universe, renderer: &mut SceneRenderer) {
    scene.update(1. / 90., &InputState::default());
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        renderer,
        &mut scene.command_queue,
    );
    scene.visuals.prepare_draw_cache();
}
pub fn run() {
    let gpu = graphics_device::headless(false);
    let target = Target::new(gpu.clone());
    let mut renderer = SceneRenderer::with_samples(
        gpu,
        vk::Format::R8G8B8A8_UNORM,
        target.extent,
        vec![target.handle()],
        vk::SampleCountFlags::TYPE_1,
    )
    .unwrap();
    let directory = std::env::temp_dir().join("mittens-destruction-mobile");
    std::fs::create_dir_all(&directory).unwrap();
    crate::demo_assets::install(&directory, crate::demo_assets::DemoScene::SakuraStage).unwrap();
    mittens_engine::engine::runtime_assets::set_root(&directory.join("mittens-demo")).unwrap();
    let fragment = directory.join("albedo.frag");
    std::fs::write(&fragment, include_str!("../../../assets/albedo.frag")).unwrap();
    let grouped = std::env::args().nth(1).as_deref() == Some("--destruction-groups");
    let front = if grouped {
        r#"
T {
    name="test_floor"
    Destroyable.threshold(3).chunk_size(1)
    CombineMesh.keep_transforms() {
        name="test_floor_white"
        T.position(0,0,-2).scale(2,2,0.12) {
            R.cube() { Raycastable.enabled() Material(definition).inputs({tint=[0.1,0.8,0.2],gain=1}) }
            Zone.cube([0.5,0.5,0.5]) { Collidable.static() }
        }
    }
    CombineMesh.keep_transforms() {
        name="test_floor_trim"
        T.position(0.65,0.65,-1.85).scale(0.3,0.3,0.12) {
            R.cube() { Raycastable.enabled() Material(definition).inputs({tint=[0.8,0.7,0.1],gain=1}) }
            Zone.cube([0.5,0.5,0.5]) { Collidable.static() }
        }
    }
}
T.position(0,4,0) {
    Destroyable.threshold(3)
    CombineMesh.keep_transforms() {
        name="test_other_floor"
        T.position(0,0,-2).scale(2,2,0.12) {
            R.cube() { Raycastable.enabled() Material(definition).inputs({tint=[0.1,0.8,0.2],gain=1}) }
            Zone.cube([0.5,0.5,0.5]) { Collidable.static() }
        }
    }
}
"#
    } else {
        r#"
T.position(0,0,-2).scale(2,2,0.12) {
    Destroyable.threshold(3).chunk_size(1)
    R.cube() { Raycastable.enabled() Material(definition).inputs({tint=[0.1,0.8,0.2],gain=1}) }
    Zone.cube([0.5,0.5,0.5]) { Collidable.static() }
}
"#
    };
    let source = format!(
        r#"
import {{ f32,vec3 }} from "types"
let definition=MaterialDefinition(MaterialBase()).fragment("{}").inputs({{tint:vec3,gain:f32}})
{front}
T.position(0,0,-3).scale(2,2,0.1) {{R.cube() {{Material(definition).inputs({{tint=[0.9,0.1,0.1],gain=1}})}}}}
"#,
        fragment.display()
    );
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(&source, &directory.join("test.mms").to_string_lossy())
        .unwrap();
    let cameras = [-0.032, 0.032].map(|x| {
        EyeCamera::from_xr(&openxr::View {
            pose: openxr::Posef {
                position: openxr::Vector3f { x, y: 0., z: 0. },
                orientation: openxr::Quaternionf {
                    x: 0.,
                    y: 0.,
                    z: 0.,
                    w: 1.,
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
    for _ in 0..100 {
        prepare(&mut scene, &mut renderer);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    renderer.draw(0, &scene.visuals, &cameras).unwrap();
    let before = target.pixels();
    assert!(
        before[(48 * 96 + 48) * 4 + 1] > 180,
        "front wall must be green"
    );
    for _ in 0..3 {
        scene.world.add_component(DamageComponent {
            position: [0., 0., -1.94],
            radius: 0.28,
            amount: 1.,
        });
        prepare(&mut scene, &mut renderer);
    }
    for _ in 0..120 {
        prepare(&mut scene, &mut renderer);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(scene.systems.destroyable.cuts, 1);
    renderer.draw(0, &scene.visuals, &cameras).unwrap();
    let after = target.pixels();
    for eye in 0..2 {
        let center = (eye * 96 * 96 + 48 * 96 + 48) * 4;
        let rim = (eye * 96 * 96 + 48 * 96 + 65) * 4;
        assert!(
            after[center] > 200 && after[center + 1] < 40,
            "hole must expose red back wall: {:?}",
            &after[center..center + 4]
        );
        assert!(
            after[rim + 1] > 180 && after[rim] < 40,
            "surviving rim must retain green custom material"
        );
    }
    if grouped {
        for eye in 0..2 {
            let trim = (eye * 96 * 96 + 23 * 96 + 73) * 4;
            assert!(
                before[trim] > 180 && before[trim + 1] > 150 && before[trim + 2] < 60,
                "intact trim batch must render yellow: {:?}",
                &before[trim..trim + 4]
            );
            assert_eq!(
                &before[trim..trim + 4],
                &after[trim..trim + 4],
                "unpacking preserves trim material"
            );
        }
        for (name, combined) in [
            ("test_floor_white", false),
            ("test_floor_trim", false),
            ("test_other_floor", true),
        ] {
            let id = scene
                .world
                .all_components()
                .find(|&id| {
                    scene
                        .world
                        .get_component_record(id)
                        .is_some_and(|r| r.name == name)
                })
                .unwrap();
            assert_eq!(
                scene.visuals.instances().iter().any(|v| v.component == id),
                combined,
                "{name}"
            );
        }
        println!("PASS floor activation: both material groups unpack; other floor stays combined");
    }
    for (name, pixels) in [("before", before), ("after", after.clone())] {
        let mut ppm = b"P6\n96 96\n255\n".to_vec();
        for p in pixels[..96 * 96 * 4].chunks_exact(4) {
            ppm.extend_from_slice(&p[..3]);
        }
        std::fs::write(format!("/tmp/mittens-destruction-{name}.ppm"), ppm).unwrap();
    }
    if std::env::args().nth(1).as_deref() == Some("--bvh-snapshot") {
        use mittens_engine::engine::ecs::component::EditorUIComponent;
        let scopes = scene
            .world
            .all_components()
            .filter(|&id| scene.world.parent_of(id).is_none())
            .collect();
        let mut ui = EditorUIComponent::new().with_panels([]);
        ui.set_bvh_snapshot_visible(true, scopes);
        let owner = scene.world.add_component(ui);
        for _ in 0..100 {
            prepare(&mut scene, &mut renderer);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        renderer.draw(0, &scene.visuals, &cameras).unwrap();
        let snapshot = target.pixels();
        let orange = snapshot
            .chunks_exact(4)
            .filter(|p| p[0] > 140 && p[1] > 40 && p[1] < 140 && p[2] < 50)
            .count();
        assert!(orange > 0, "snapshot should show orange wire bounds");
        let mut ppm = b"P6\n96 96\n255\n".to_vec();
        for p in snapshot[..96 * 96 * 4].chunks_exact(4) {
            ppm.extend_from_slice(&p[..3]);
        }
        std::fs::write("/tmp/mittens-bvh-snapshot.ppm", ppm).unwrap();
        // Exercise actual rendered animation over a complete traversal. The
        // diagnostic keeps its mesh and only changes material parameters.
        let wire = scene
            .visuals
            .instances()
            .iter()
            .find(|v| v.component == owner)
            .unwrap()
            .renderable
            .mesh;
        scene
            .world
            .get_component_by_id_as_mut::<EditorUIComponent>(owner)
            .unwrap()
            .set_bvh_animation_enabled(true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut levels = Vec::new();
        let mut bright_pixels = 0;
        loop {
            assert!(
                std::time::Instant::now() < deadline,
                "BVH traversal failed to loop"
            );
            prepare(&mut scene, &mut renderer);
            let material = scene.visuals.materials.source_instance(owner).unwrap();
            let depth = scene.visuals.materials.instance(material).unwrap().values[1] as u32;
            if levels.last().copied() != Some(depth) {
                if !levels.is_empty() && depth == 0 {
                    break;
                }
                assert_eq!(
                    depth as usize,
                    levels.len(),
                    "all levels must be visited in order"
                );
                levels.push(depth);
                renderer.draw(0, &scene.visuals, &cameras).unwrap();
                let pixels = target.pixels();
                bright_pixels += pixels
                    .chunks_exact(4)
                    .filter(|p| p[0] > 200 && p[1] > 150 && p[2] < 110)
                    .count();
                assert_eq!(
                    scene
                        .visuals
                        .instances()
                        .iter()
                        .find(|v| v.component == owner)
                        .unwrap()
                        .renderable
                        .mesh,
                    wire
                );
                if depth == 1 {
                    let mut ppm = b"P6\n96 96\n255\n".to_vec();
                    for p in pixels[..96 * 96 * 4].chunks_exact(4) {
                        ppm.extend_from_slice(&p[..3]);
                    }
                    std::fs::write("/tmp/mittens-bvh-animation.ppm", ppm).unwrap();
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            levels.len() > 1 && bright_pixels > 0,
            "animation must brighten visible wire boxes"
        );
        scene
            .world
            .get_component_by_id_as_mut::<EditorUIComponent>(owner)
            .unwrap()
            .set_bvh_animation_enabled(false);
        prepare(&mut scene, &mut renderer);
        renderer.draw(0, &scene.visuals, &cameras).unwrap();
        assert_eq!(
            target.pixels(),
            snapshot,
            "stopping animation restores every original wire box"
        );
        println!(
            "PASS BVH animation: levels={}, bright_pixels={bright_pixels}, mesh reused",
            levels.len()
        );
        scene
            .world
            .get_component_by_id_as_mut::<EditorUIComponent>(owner)
            .unwrap()
            .set_bvh_snapshot_visible(false, vec![]);
        prepare(&mut scene, &mut renderer);
        renderer.draw(0, &scene.visuals, &cameras).unwrap();
        assert_eq!(
            target.pixels(),
            after,
            "disabling snapshot must restore the previous render"
        );
        println!("PASS BVH snapshot: orange_pixels={orange}, off restores scene");
    }
    println!(
        "PASS destruction: physical mesh cut exposes rear surface, rim material retained; worker_ms={:.3}",
        scene.systems.destroyable.last_cut_time.as_secs_f64() * 1000.
    );
}
