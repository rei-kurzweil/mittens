use cat_engine::{engine, utils};

use std::path::{Path, PathBuf};

fn measure_text_bounds(text: &str, wrap_at: usize) -> (usize, usize) {
    const TAB_WIDTH: usize = 4;

    let mut col: usize = 0;
    let mut line_count: usize = 0;
    let mut row: usize = 0;
    let mut max_col: usize = 0;

    for ch in text.chars() {
        if ch == '\n' {
            max_col = max_col.max(col);
            row += 1;
            col = 0;
            line_count = 0;
            continue;
        }

        if wrap_at != 0 && line_count >= wrap_at {
            max_col = max_col.max(col);
            row += 1;
            col = 0;
            line_count = 0;
        }

        let adv = if ch == '\t' { TAB_WIDTH } else { 1 };
        col += adv;
        line_count += adv;
        max_col = max_col.max(col);
    }

    // Keep a minimum 1x1 so empty text still gets a panel.
    (max_col.max(1), (row + 1).max(1))
}

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
    const MAX_FILES: usize = 42;
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

    // Dark magenta background + matching ambient light.
    let bg_r = 0.10;
    let bg_g = 0.00;
    let bg_b = 0.15;
    let background =
        universe
            .world
            .add_component(engine::ecs::component::BackgroundColorComponent::rgba(
                bg_r / 2.0,
                bg_g / 2.0,
                bg_b / 2.0,
                1.00,
            ));
    universe
        .world
        .init_component_tree(background, &mut universe.command_queue);

    let ambient = universe
        .world
        .add_component(engine::ecs::component::AmbientLightComponent::rgb(
            bg_r * 2.0,
            bg_g * 2.0,
            bg_b * 2.0,
        ));
    universe
        .world
        .init_component_tree(ambient, &mut universe.command_queue);

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
    let input_mode = universe.world.add_component(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let _ = universe.world.add_child(input, input_mode);
    let _ = universe.world.add_child(input, rig_transform);
    let _ = universe.world.add_child(rig_transform, camera3d);

    // Attach a point light to the same transform the camera is nested under.
    let camera_light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(100.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.world.add_child(rig_transform, camera_light);
    universe
        .world
        .init_component_tree(input, &mut universe.command_queue);

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
            .add_component(engine::ecs::component::ColorComponent::rgba(
                1.0, 0.0, 0.0, 1.0,
            ));
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

        // Per-file group at world scale.
        // This owns the (tiny) text subtree and a world-scale ground plane.
        let file_group = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(x, 0.0, 0.0),
        );

        // Text subtree (tiny scale).
        let file_root = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(0.0, 1.0, 0.0)
                .with_scale(TEXT_SCALE, TEXT_SCALE, 1.0)
                .with_rotation_euler(0.0, std::f32::consts::PI / 6.0, 0.0),
        );
        let _ = universe.world.add_child(file_group, file_root);

        // Pink background panel behind the text.
        // Size is based on wrapped line count (matches TextSystem's strict wrapping behavior).
        let (max_cols, rows) = measure_text_bounds(&display_text, WRAP_AT);
        let pad_x = 4.0;
        let pad_y = 4.0;
        let w = (max_cols as f32) + pad_x;
        let h = (rows as f32) + pad_y;

        // Text glyph quads are centered at integer (col,row) positions and are 1x1, so the
        // text's AABB (in text-space) is roughly:
        //   x: [-0.5, max_cols-0.5]
        //   y: [-(rows-1)-0.5, 0.5]
        // Centering the background quad at those midpoints keeps it aligned as text grows.
        let bg_x = (max_cols as f32 - 1.0) * 0.5;
        let bg_y = -((rows as f32 - 1.0) * 0.5);

        let bg_transform = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(bg_x, bg_y, -0.05)
                .with_scale(w, h, 1.0),
        );
        let bg_renderable = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::square());
        let bg_color = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                0.2, 0.2, 0.2, 1.0,
            ));
        let _ = universe.world.add_child(file_root, bg_transform);
        let _ = universe.world.add_child(bg_transform, bg_renderable);
        let _ = universe.world.add_child(bg_renderable, bg_color);

        let text = universe
            .world
            .add_component(engine::ecs::component::TextComponent::with_wrap(
                display_text,
                WRAP_AT,
            ));
        let filtering = universe.world.add_component(
            engine::ecs::component::TextureFilteringComponent::nearest_magnification(),
        );
        // let color = universe
        //     .world
        //     .add_component(engine::ecs::component::ColorComponent::rgba(0.7, 0.7, 1.0, 1.0));
        let emissive = universe
            .world
            .add_component(engine::ecs::component::EmissiveComponent::on());
        let _ = universe.world.add_child(file_root, text);
        let _ = universe.world.add_child(text, filtering);
        //let _ = universe.world.add_child(text, color);
        let _ = universe.world.add_child(text, emissive);

        universe
            .world
            .init_component_tree(file_group, &mut universe.command_queue);
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
