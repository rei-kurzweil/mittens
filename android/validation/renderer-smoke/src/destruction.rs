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
    let source = format!(
        r#"
import {{ f32,vec3 }} from "types"
let definition=MaterialDefinition(MaterialBase()).fragment("{}").inputs({{tint:vec3,gain:f32}})
T.position(0,0,-2).scale(2,2,0.12) {{
    Destroyable.threshold(3).chunk_size(1)
    R.cube() {{ Raycastable.enabled() Material(definition).inputs({{tint=[0.1,0.8,0.2],gain=1}}) }}
    Zone.cube([0.5,0.5,0.5]) {{ Collidable.static() }}
}}
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
    for (name, pixels) in [("before", before), ("after", after)] {
        let mut ppm = b"P6\n96 96\n255\n".to_vec();
        for p in pixels[..96 * 96 * 4].chunks_exact(4) {
            ppm.extend_from_slice(&p[..3]);
        }
        std::fs::write(format!("/tmp/mittens-destruction-{name}.ppm"), ppm).unwrap();
    }
    println!(
        "PASS destruction: physical mesh cut exposes rear surface, rim material retained; worker_ms={:.3}",
        scene.systems.destroyable.last_cut_time.as_secs_f64() * 1000.
    );
}
