//! Screenshot of the packaged headset MMS through the actual mobile renderer.
use crate::{
    graphics_device,
    renderer::{SceneRenderer, view::EyeCamera},
    target::Target,
};
use ash::vk;
use mittens_engine::engine::{Universe, ecs::World};
use std::{
    io::Write,
    time::{Duration, Instant},
};
pub fn render(output: &str) {
    let gpu = graphics_device::headless(false);
    let target = Target::with_extent(gpu.clone(), 1440, 960);
    let mut renderer = SceneRenderer::with_samples(
        gpu,
        vk::Format::R8G8B8A8_UNORM,
        target.extent,
        vec![target.handle()],
        vk::SampleCountFlags::TYPE_4,
    )
    .unwrap();
    let directory = std::env::temp_dir().join(format!("mitten-droid-shell-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source =
        crate::demo_assets::install(&directory, crate::demo_assets::DemoScene::Shell).unwrap();
    mittens_engine::engine::runtime_assets::set_root(&directory.join("mittens-demo")).unwrap();
    let mut universe = Universe::new(World::default());
    universe
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    universe
        .load_mms_source_at_path(
            &source.source.replace("0.0.0.0:7000", "127.0.0.1:0"),
            &source.path.to_string_lossy(),
        )
        .unwrap();
    let start = Instant::now();
    for frame in 0..100 {
        universe.update(1.0 / 90.0, &Default::default());
        universe.systems.prepare_render(
            &mut universe.world,
            &mut universe.visuals,
            &mut universe.render_assets,
            &mut renderer,
            &mut universe.command_queue,
        );
        universe.visuals.prepare_draw_cache();
        let pending = universe.visuals.materials.instances().any(|(_, record)| {
            universe
                .visuals
                .materials
                .definition(record.definition)
                .and_then(|definition| definition.custom.as_ref())
                .is_some_and(|custom| custom.compiled.get().is_none())
        });
        if frame >= 12 && !pending && !universe.visuals.instances().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let cameras = [-0.032, 0.032].map(|x| {
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
                angle_left: -0.7,
                angle_right: 0.7,
                angle_up: 0.5,
                angle_down: -0.5,
            },
        })
        .unwrap()
    });
    let stats = renderer.draw(0, &universe.visuals, &cameras).unwrap();
    let pixels = target.pixels();
    let left_eye = &pixels[..1440 * 960 * 4];
    let cream = left_eye
        .chunks_exact(4)
        .filter(|p| p[0] > 220 && p[1] > 200 && p[2] > 140)
        .count();
    let amber_text = left_eye
        .chunks_exact(4)
        .enumerate()
        .filter(|(index, p)| {
            let x = index % 1440;
            let y = index / 1440;
            x > 120 && x < 710 && y > 260 && y < 700 && p[0] > 180 && p[1] > 100 && p[2] < 100
        })
        .count();
    assert!(cream > 100_000, "file-browser content must be visible");
    assert!(amber_text > 500, "terminal content must be visible");
    let mut file = std::fs::File::create(output).unwrap();
    write!(file, "P6\n1440 960\n255\n").unwrap();
    for pixel in pixels[..1440 * 960 * 4].chunks_exact(4) {
        file.write_all(&pixel[..3]).unwrap();
    }
    println!(
        "Shell prepared in {:?}; meshes={} draws={} pipelines={} instances={} screenshot={output}",
        start.elapsed(),
        stats.meshes,
        stats.draws,
        stats.pipelines,
        stats.instances
    );
}
