use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

use std::collections::BTreeMap;
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

    #[derive(Debug, Clone)]
    struct FileEntry {
        path: PathBuf,
        display_text: String,
        max_cols: usize,
        rows: usize,
    }

    // Group files by folder.
    // Layout:
    // - each folder becomes a column along +X
    // - within a folder, files stack along +Y (8 high)
    // - after 8, additional files start a new stack further along +Z
    let mut files_by_folder: BTreeMap<PathBuf, Vec<FileEntry>> = BTreeMap::new();
    for path in files {
        let Ok(content) = std::fs::read_to_string(&path) else {
            eprintln!("[folder-text] Failed to read {:?}", path);
            continue;
        };

        let mut display_text = format!(
            "// {}\n\n{}",
            path.strip_prefix(&manifest_dir).unwrap_or(&path).display(),
            content
        );

        if display_text.chars().count() > MAX_CHARS_PER_FILE {
            display_text = display_text
                .chars()
                .take(MAX_CHARS_PER_FILE)
                .collect::<String>();
            display_text.push_str("\n\n// ... truncated ...\n");
        }

        let (max_cols, rows) = measure_text_bounds(&display_text, WRAP_AT);

        let folder_key = path.parent().unwrap_or(&scan_root).to_path_buf();
        files_by_folder
            .entry(folder_key)
            .or_default()
            .push(FileEntry {
                path,
                display_text,
                max_cols,
                rows,
            });
    }

    let column_count = files_by_folder.len().max(1);

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let bg_r = 0.25;
    let bg_g = 0.25;
    let bg_b = 0.25;
    let background = universe.world.add_component(engine::ecs::component::BackgroundColorComponent::new());
    let background_c = universe.world.add_component(engine::ecs::component::ColorComponent::rgba(bg_r, bg_g, bg_b, 1.00));
    let _ = universe.world.add_child(background, background_c);
    universe.add(background);

    let ambient = universe
        .world
        .add_component(engine::ecs::component::AmbientLightComponent::rgb(
            bg_r, bg_g, bg_b,
        ));
    universe.add(ambient);

    // Input-driven camera rig.
    // Topology: I { T { C3D } }
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(1.5));

    // Center the camera horizontally over the spawned columns.
    let center_x = if column_count <= 1 {
        0.0
    } else {
        ((column_count - 1) as f32) * spacing * 0.5
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
    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig_transform);
    let _ = universe.attach(rig_transform, camera3d);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);
    universe.add(input);

    // Big floor plane under everything.
    // RenderableComponent::square() is an XY quad facing +Z; rotate it to XZ facing +Y.
    let max_stacks = files_by_folder
        .values()
        .map(|v| (v.len() + 7) / 8)
        .max()
        .unwrap_or(1)
        .max(1);
    let total_width = (column_count as f32) * spacing;
    let total_depth = (max_stacks as f32) * spacing;
    let floor_w = total_width.max(10.0);
    let floor_h = total_depth.max(10.0);
    let floor_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new()
            .with_position(0.0, -2.0, 0.0)
            .with_rotation_euler(-std::f32::consts::FRAC_PI_2, 0.0, 0.0)
            .with_scale(floor_w, floor_h, 1.0),
    );
    let floor_renderable = universe
        .world
        .add_component(engine::ecs::component::RenderableComponent::square());
    let floor_color = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.88, 0.88, 0.88, 1.0,
        ));
    let _ = universe.attach(floor_transform, floor_renderable);
    let _ = universe.attach(floor_renderable, floor_color);
    universe.add(floor_transform);

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

        let light_transform = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(0.0, s * 0.75, 0.0),
        );
        let light = universe.world.add_component(
            engine::ecs::component::PointLightComponent::new()
                .with_intensity(1.5)
                .with_distance(20.0)
                .with_color(1.0, 0.0, 0.0),
        );

        let _ = universe.attach(t, r);
        let _ = universe.attach(r, c);
        let _ = universe.attach(t, light_transform);
        let _ = universe.attach(light_transform, light);

        universe.add(t);
    }

    let p = 1.5;
    let s = 0.25;
    spawn_red_cube(&mut universe, -p, -p, 0.0, s);
    spawn_red_cube(&mut universe, p, -p, 0.0, s);
    spawn_red_cube(&mut universe, -p, p, 0.0, s);
    spawn_red_cube(&mut universe, p, p, 0.0, s);

    let start_x = -center_x;
    let stack_depth = spacing;
    let row_gap_world = 0.35;

    // One global point light at the top of the overall text "tower".
    let pad_y = 4.0;
    let global_max_panel_h_world = files_by_folder
        .values()
        .flat_map(|v| v.iter())
        .map(|e| ((e.rows as f32) + pad_y) * TEXT_SCALE)
        .fold(0.0_f32, f32::max)
        .max(0.6);
    let global_slot_h_world = global_max_panel_h_world + row_gap_world;
    let tower_top_y = (7.0 * global_slot_h_world) + 4.0;
    let tower_center_z = total_depth * 0.5;

    let tower_light_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(
            0.0,
            tower_top_y,
            tower_center_z,
        ),
    );
    let tower_light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_intensity(2.0)
            .with_distance(120.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.attach(tower_light_transform, tower_light);
    universe.add(tower_light_transform);

    for (col_idx, (_folder, mut entries)) in files_by_folder.into_iter().enumerate() {
        entries.sort_by(|a, b| a.path.cmp(&b.path));

        // Slot height (world units) based on the tallest panel in this folder.
        let pad_y = 4.0;
        let max_panel_h_world = entries
            .iter()
            .map(|e| ((e.rows as f32) + pad_y) * TEXT_SCALE)
            .fold(0.0_f32, f32::max)
            .max(0.6);
        let slot_h_world = max_panel_h_world + row_gap_world;

        let col_x = start_x + (col_idx as f32) * spacing;

        for (i, entry) in entries.into_iter().enumerate() {
            let row = (i % 8) as f32;
            let stack = (i / 8) as f32;
            let file_y = row * slot_h_world;
            let file_z = stack * stack_depth;

            let file_group = universe.world.add_component(
                engine::ecs::component::TransformComponent::new()
                    .with_position(col_x, file_y, file_z),
            );

            // Text subtree (tiny scale).
            let file_root = universe.world.add_component(
                engine::ecs::component::TransformComponent::new()
                    .with_position(0.0, 1.0, 0.0)
                    .with_scale(TEXT_SCALE, TEXT_SCALE, 1.0)
                    .with_rotation_euler(0.0, std::f32::consts::PI / 6.0, 0.0),
            );
            let _ = universe.attach(file_group, file_root);

            // Background panel behind the text.
            // Size is based on wrapped line count (matches TextSystem's strict wrapping behavior).
            let (max_cols, rows) = (entry.max_cols, entry.rows);
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
            let bg_quant = universe.world.add_component(
                engine::ecs::component::LightQuantizationComponent::steps(5.0),
            );
            let bg_color =
                universe
                    .world
                    .add_component(engine::ecs::component::ColorComponent::rgba(
                        0.2, 0.2, 0.2, 1.0,
                    ));
            let _ = universe.attach(file_root, bg_transform);
            let _ = universe.attach(bg_transform, bg_renderable);
            let _ = universe.attach(bg_renderable, bg_quant);
            let _ = universe.attach(bg_renderable, bg_color);

            let text =
                universe
                    .world
                    .add_component(engine::ecs::component::TextComponent::with_wrap(
                        entry.display_text,
                        WRAP_AT,
                    ));
            let cutout = universe
                .world
                .add_component(engine::ecs::component::TransparentCutoutComponent::new());
            let filtering = universe.world.add_component(
                engine::ecs::component::TextureFilteringComponent::nearest_magnification(),
            );
            // let color = universe
            //     .world
            //     .add_component(engine::ecs::component::ColorComponent::rgba(0.7, 0.7, 1.0, 1.0));
            let emissive = universe
                .world
                .add_component(engine::ecs::component::EmissiveComponent::on());
            let _ = universe.attach(file_root, text);
            let _ = universe.attach(text, cutout);
            let _ = universe.attach(text, filtering);
            //let _ = universe.world.add_child(text, color);
            let _ = universe.attach(text, emissive);

            universe.add(file_group);
        }
    }

    // Process init-time registrations (Text expands into glyph subtrees here).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
