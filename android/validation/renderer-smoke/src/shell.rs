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
    let mode = std::env::args().nth(4).unwrap_or_default();
    let title_preview = mode == "title-bar";
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
    if title_preview {
        universe
            .load_mms_source_at_path(
                include_str!("../../../../assets/components/ui/title_bar.preview.mms"),
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../assets/components/ui/title_bar.preview.mms"
                ),
            )
            .unwrap();
    } else {
        universe
            .load_mms_source_at_path(
                &source.source.replace("0.0.0.0:7000", "127.0.0.1:0"),
                &source.path.to_string_lossy(),
            )
            .unwrap();
    }
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
    let minimized = mode == "minimized";
    if mode == "grabbed" {
        use mittens_engine::engine::ecs::EventSignal;
        let named = |name: &str| {
            universe
                .world
                .all_components()
                .find(|&id| universe.world.component_label(id) == Some(name))
                .unwrap()
        };
        let grabs = [
            (
                named("shell_TERMINAL_title"),
                named("shell_TERMINAL_window"),
                named("shell_left_pointer"),
            ),
            (
                named("shell_FILES_title"),
                named("shell_FILES_window"),
                named("shell_right_pointer"),
            ),
        ];
        for (title, target, pointer) in grabs {
            universe.systems.rx.push_event(
                title,
                EventSignal::GrabStart {
                    pointer,
                    raycaster: pointer,
                    renderable: title,
                    target,
                    ray_origin_world: [0.0; 3],
                    ray_dir_world: [0.0, 0.0, -1.0],
                },
            );
        }
        // Reparent without advancing the easing animation; both held windows
        // remain visible, and must still satisfy the mobile material contract.
        universe.update(0.0, &Default::default());
        for (_, target, pointer) in grabs {
            assert_eq!(
                universe.world.parent_of(target),
                universe.world.parent_of(pointer)
            );
        }
        universe.systems.prepare_render(
            &mut universe.world,
            &mut universe.visuals,
            &mut universe.render_assets,
            &mut renderer,
            &mut universe.command_queue,
        );
        universe.visuals.prepare_draw_cache();
    }
    if minimized {
        let toggles = ["shell_TERMINAL_window", "shell_FILES_window"].map(|name| {
            let root = universe
                .world
                .all_components()
                .find(|&id| universe.world.component_label(id) == Some(name))
                .unwrap();
            let mut nodes = vec![root];
            while let Some(id) = nodes.pop() {
                if universe.world.component_label(id) == Some("accordion_toggle") {
                    return id;
                }
                nodes.extend(universe.world.children_of(id));
            }
            panic!("missing toggle for {name}");
        });
        for toggle in toggles {
            universe.systems.rx.push_event(
                toggle,
                mittens_engine::engine::ecs::EventSignal::Click {
                    raycaster: toggle,
                    renderable: toggle,
                    hit_point: [0.0; 3],
                    screen_pos_px: None,
                },
            );
        }
        for _ in 0..40 {
            universe.update(1.0 / 90.0, &Default::default());
            universe.systems.prepare_render(
                &mut universe.world,
                &mut universe.visuals,
                &mut universe.render_assets,
                &mut renderer,
                &mut universe.command_queue,
            );
        }
        universe.visuals.prepare_draw_cache();
    }
    let yaw = std::env::args()
        .nth(3)
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.0)
        .to_radians();
    let cameras = [-0.032, 0.032].map(|x| {
        EyeCamera::from_xr(&openxr::View {
            pose: openxr::Posef {
                position: openxr::Vector3f { x, y: 0.0, z: 0.0 },
                orientation: openxr::Quaternionf {
                    x: 0.0,
                    y: (yaw * 0.5).sin(),
                    z: 0.0,
                    w: (yaw * 0.5).cos(),
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
    if yaw == 0.0 && !minimized && !title_preview {
        assert!(cream > 100_000, "file-browser content must be visible");
        assert!(amber_text > 500, "terminal content must be visible");
    }
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
