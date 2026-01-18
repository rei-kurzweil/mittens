use little_cat::{engine, utils};

use std::path::{Path, PathBuf};

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn main() {
    utils::logger::init();

    // Usage:
    //   cargo run --example folder-text -- [folder] [spacing]
    // Defaults:
    //   folder=src  spacing=0.3
    let mut args = std::env::args().skip(1);
    let folder = args.next().unwrap_or_else(|| "src".to_string());
    let spacing: f32 = args
        .next()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0);

    // Safety caps: this demo can explode the ECS if we load huge projects.
    const MAX_FILES: usize = 30;
    const MAX_CHARS_PER_FILE: usize = 10_000;
    const WRAP_AT: usize = 90;
    const TEXT_SCALE: f32 = 0.01;

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scan_root = {
        let p = PathBuf::from(&folder);
        if p.is_absolute() {
            p
        } else {
            // Resolve relative paths from the crate root, not the process CWD.
            manifest_dir.join(p)
        }
    };
    eprintln!("[folder-text] cwd={:?}", cwd);
    eprintln!("[folder-text] manifest_dir={:?}", manifest_dir);
    eprintln!("[folder-text] scan_root={:?}", scan_root);

    let mut files = Vec::new();
    collect_rs_files(&scan_root, &mut files);
    files.sort();

    if files.is_empty() {
        eprintln!(
            "[folder-text] No .rs files found under folder={:?} (resolved={:?})",
            folder, scan_root
        );
    }

    let files: Vec<PathBuf> = files.into_iter().take(MAX_FILES).collect();
    eprintln!(
        "[folder-text] Loaded {} file(s) from {:?} (spacing={})",
        files.len(),
        folder,
        spacing
    );

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Input-driven camera rig.
    // Topology: I { T { C3D } }
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(1.5));

    // Center the camera horizontally over the spawned blocks.
    let center_x = if files.len() <= 1 {
        0.0
    } else {
        ((files.len() - 1) as f32) * spacing * 0.5
    };

    // Bring camera closer: these text blocks are tiny.
    let rig_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(center_x, 0.0, 1.2),
    );
    let camera3d = universe
        .world
        .add_component(engine::ecs::component::Camera3DComponent::new());
    let input_mode = universe
        .world
        .add_component(engine::ecs::component::InputTransformModeComponent::forward_z()
            .with_roll_axis_y()
        );
    let _ = universe.world.add_child(input, input_mode);
    let _ = universe.world.add_child(input, rig_transform);
    let _ = universe.world.add_child(rig_transform, camera3d);
    universe
        .world
        .init_component_tree(input, &mut universe.command_queue);

    // Light so we can see meshes / UI consistently.
    let light_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(center_x, 0.0, 2.0),
    );
    let light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(25.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.world.add_child(light_transform, light);
    universe
        .world
        .init_component_tree(light_transform, &mut universe.command_queue);

    // 4 red cubes around the perimeter of the world (easy visual anchors).
    fn spawn_red_cube(universe: &mut engine::Universe, x: f32, y: f32, z: f32, s: f32) {
        let t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(s, s, s),
        );
        let r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(1.0, 0.0, 0.0, 1.0));
        let e = universe
            .world
            .add_component(engine::ecs::component::EmissiveComponent::on());

        let _ = universe.world.add_child(t, r);
        let _ = universe.world.add_child(r, c);
        let _ = universe.world.add_child(r, e);

        universe
            .world
            .init_component_tree(t, &mut universe.command_queue);
    }

    let p = 1.5;
    let s = 0.25;
    spawn_red_cube(&mut universe, -p, -p, 0.0, s);
    spawn_red_cube(&mut universe, p, -p, 0.0, s);
    spawn_red_cube(&mut universe, -p, p, 0.0, s);
    spawn_red_cube(&mut universe, p, p, 0.0, s);

    // Place each file beside the previous along X.
    let start_x = -center_x;

    for (i, path) in files.iter().enumerate() {
        let Ok(content) = std::fs::read_to_string(path) else {
            eprintln!("[folder-text] Failed to read {:?}", path);
            continue;
        };

        let mut display_text = format!(
            "// {}\n\n{}",
            path.strip_prefix(&manifest_dir).unwrap_or(path).display(),
            content
        );

        if display_text.chars().count() > MAX_CHARS_PER_FILE {
            display_text = display_text
                .chars()
                .take(MAX_CHARS_PER_FILE)
                .collect::<String>();
            display_text.push_str("\n\n// ... truncated ...\n");
        }

        let x = start_x + (i as f32) * spacing;
        let file_root = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, 1.0, 0.0)
                .with_scale(TEXT_SCALE, TEXT_SCALE, 1.0),
        );

        let text = universe.world.add_component(
            engine::ecs::component::TextComponent::with_wrap(display_text, WRAP_AT),
        );
        let _ = universe.world.add_child(file_root, text);

        universe
            .world
            .init_component_tree(file_root, &mut universe.command_queue);
    }

    // Process init-time registrations (Text expands into glyph subtrees here).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
