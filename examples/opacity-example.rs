use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, ColorComponent,
    InputComponent, InputTransformModeComponent, OpacityComponent, PointLightComponent,
    RenderableComponent, TextComponent, TextureFilteringComponent, TransformComponent,
};

fn spawn_cube(
    universe: &mut engine::Universe,
    parent: engine::ecs::ComponentId,
    position: (f32, f32, f32),
    scale: (f32, f32, f32),
    color: Option<(f32, f32, f32, f32)>,
    opacity: Option<OpacityComponent>,
) {
    let t = universe.world.add_component(
        TransformComponent::new()
            .with_position(position.0, position.1, position.2)
            .with_scale(scale.0, scale.1, scale.2),
    );
    let r = universe.world.add_component(RenderableComponent::cube());

    let _ = universe.attach(parent, t);
    let _ = universe.attach(t, r);

    if let Some((cr, cg, cb, ca)) = color {
        let c = universe
            .world
            .add_component(ColorComponent::rgba(cr, cg, cb, ca));
        let _ = universe.attach(r, c);
    }

    if let Some(o) = opacity {
        let o = universe.world.add_component(o);
        let _ = universe.attach(r, o);
    }
}

fn spawn_text_label(universe: &mut engine::Universe, position: (f32, f32, f32), text: &str) {
    // T_root { T_scale { TXT { filtering } } }
    let text_root = universe
        .world
        .add_component(TransformComponent::new().with_position(position.0, position.1, position.2));

    let text_scale = universe
        .world
        .add_component(TransformComponent::new().with_scale(0.18, 0.18, 1.0));
    let _ = universe.attach(text_root, text_scale);

    let text_c = universe.world.add_component(TextComponent::new(text));
    let _ = universe.attach(text_scale, text_c);

    // Keep it crisp.
    let filtering = universe
        .world
        .add_component(TextureFilteringComponent::nearest());
    let _ = universe.attach(text_c, filtering);

    // White label.
    let color = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let _ = universe.attach(text_c, color);

    universe.add(text_root);
}

fn text_block_dimensions(text: &str) -> (usize, usize) {
    let mut max_cols = 0usize;
    let mut rows = 1usize;
    let mut cur = 0usize;

    for ch in text.chars() {
        if ch == '\n' {
            max_cols = max_cols.max(cur);
            cur = 0;
            rows += 1;
        } else {
            cur += 1;
        }
    }
    max_cols = max_cols.max(cur);

    (max_cols.max(1), rows.max(1))
}

fn spawn_text_label_with_bg(
    universe: &mut engine::Universe,
    position: (f32, f32, f32),
    text: &str,
    bg_opacity: f32,
) {
    // T_root {
    //   T_bg { R_bg { Color black, Opacity } }
    //   T_scale { TXT { filtering } }
    // }

    let text_root = universe
        .world
        .add_component(TransformComponent::new().with_position(position.0, position.1, position.2));

    // Background quad (slightly behind the glyph quads).
    let (cols, rows) = text_block_dimensions(text);
    let text_scale = 0.18_f32;
    let pad_x = 0.55_f32;
    let pad_y = 0.45_f32;
    let bg_w = cols as f32 * text_scale + pad_x;
    let bg_h = rows as f32 * text_scale + pad_y;

    let bg_t = universe.world.add_component(
        TransformComponent::new()
            .with_position(1.5, 0.0, -0.02)
            .with_scale(bg_w, bg_h, 1.0),
    );
    let bg_r = universe.world.add_component(RenderableComponent::square());
    let _ = universe.attach(text_root, bg_t);
    let _ = universe.attach(bg_t, bg_r);

    let bg_c = universe
        .world
        .add_component(ColorComponent::rgba(0.0, 0.0, 0.0, 1.0));
    let _ = universe.attach(bg_r, bg_c);

    let bg_o = universe
        .world
        .add_component(OpacityComponent::new().with_opacity(bg_opacity));
    let _ = universe.attach(bg_r, bg_o);

    let text_scale_t = universe
        .world
        .add_component(TransformComponent::new().with_scale(text_scale, text_scale, 1.0));
    let _ = universe.attach(text_root, text_scale_t);

    let text_c = universe.world.add_component(TextComponent::new(text));
    let _ = universe.attach(text_scale_t, text_c);

    // Keep it crisp.
    let filtering = universe
        .world
        .add_component(TextureFilteringComponent::nearest());
    let _ = universe.attach(text_c, filtering);

    // Force the label into the transparent pass so it layers correctly with the background.
    // (Pass selection currently does not consider texture alpha.)
    let color = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 0.998));
    let _ = universe.attach(text_c, color);

    universe.add(text_root);
}

fn spawn_demo_spot(
    universe: &mut engine::Universe,
    spot_pos: (f32, f32, f32),
    opacity: f32,
    multiple_layers: bool,
    grid_n: i32,
) {
    let spot = universe
        .world
        .add_component(TransformComponent::new().with_position(spot_pos.0, spot_pos.1, spot_pos.2));

    // All cubes inherit this color.
    let white = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let _ = universe.attach(spot, white);

    // All cubes inherit this opacity (no per-cube OpacityComponent needed).
    let mut oc = OpacityComponent::new().with_opacity(opacity);
    if multiple_layers {
        oc = oc.with_multiple_layers();
    }
    let oc = universe.world.add_component(oc);
    let _ = universe.attach(spot, oc);

    universe.add(spot);

    // Cube grid.
    let n = grid_n.max(1);
    let spacing = if n <= 2 { 1.0 } else { 0.6 };
    let cube_scale = if n <= 2 { 0.8 } else { 0.45 };
    let half = (n as f32 - 1.0) * spacing * 0.5;

    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let px = x as f32 * spacing - half;
                let py = y as f32 * spacing;
                let pz = z as f32 * spacing - half;

                spawn_cube(
                    universe,
                    spot,
                    (px, py, pz),
                    (cube_scale, cube_scale, cube_scale),
                    None,
                    None,
                );
            }
        }
    }

    // Label above.
    let label = format!(
        "opacity: {:.2}\nmulti-layer: {}",
        opacity,
        if multiple_layers { "true" } else { "false" }
    );
    // Keep labels away from the cube volume so they don't intersect.
    let label_y = spot_pos.1 + (n as f32 * spacing) + 1.8;
    let label_x = spot_pos.0 - (half + 0.6);
    let label_z = spot_pos.2 - (half + 1.0);
    spawn_text_label(universe, (label_x, label_y, label_z), &label);
}

fn spawn_demo_strip_pair(
    universe: &mut engine::Universe,
    spot_pos: (f32, f32, f32),
    transparent_multiple_layers: bool,
) {
    let n_z: i32 = 4;
    let spacing = 1.0;
    let cube_scale = 0.8;
    let half_z = (n_z as f32 - 1.0) * spacing * 0.5;

    // Transparent strip (1x4 along Z).
    let transparent_root = universe
        .world
        .add_component(TransformComponent::new().with_position(spot_pos.0, spot_pos.1, spot_pos.2));

    let white = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let _ = universe.attach(transparent_root, white);

    let mut oc = OpacityComponent::new().with_opacity(0.50);
    if transparent_multiple_layers {
        oc = oc.with_multiple_layers();
    }
    let oc = universe.world.add_component(oc);
    let _ = universe.attach(transparent_root, oc);

    universe.add(transparent_root);

    for z in 0..n_z {
        let pz = z as f32 * spacing - half_z;
        spawn_cube(
            universe,
            transparent_root,
            (0.0, 0.0, pz),
            (cube_scale, cube_scale, cube_scale),
            None,
            None,
        );
    }

    // Opaque strip to the left, same spacing/scale.
    let opaque_root = universe
        .world
        .add_component(TransformComponent::new().with_position(
            spot_pos.0 - spacing,
            spot_pos.1,
            spot_pos.2,
        ));
    let white = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let _ = universe.attach(opaque_root, white);
    universe.add(opaque_root);

    for z in 0..n_z {
        let pz = z as f32 * spacing - half_z;
        spawn_cube(
            universe,
            opaque_root,
            (0.0, 0.0, pz),
            (cube_scale, cube_scale, cube_scale),
            None,
            None,
        );
    }

    let label = format!(
        "mini: opaque + transparent strip\ntransparent opacity: 0.50\nmulti-layer: {}",
        if transparent_multiple_layers {
            "true"
        } else {
            "false"
        }
    );
    let label_y = spot_pos.1 + spacing + 1.8;
    let label_x = spot_pos.0 - (spacing * 2.6);
    let label_z = spot_pos.2 - (half_z + 1.0);
    spawn_text_label(universe, (label_x, label_y, label_z), &label);
}

fn spawn_demo_xy_plane(
    universe: &mut engine::Universe,
    spot_pos: (f32, f32, f32),
    opacity: f32,
    n_x: i32,
    n_y: i32,
    z: f32,
) {
    let spot = universe
        .world
        .add_component(TransformComponent::new().with_position(spot_pos.0, spot_pos.1, spot_pos.2));

    let white = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let _ = universe.attach(spot, white);

    let oc = universe
        .world
        .add_component(OpacityComponent::new().with_opacity(opacity));
    let _ = universe.attach(spot, oc);

    universe.add(spot);

    let nx = n_x.max(1);
    let ny = n_y.max(1);
    let spacing = 0.45;
    let cube_scale = 0.35;
    let half_x = (nx as f32 - 1.0) * spacing * 0.5;

    for y in 0..ny {
        for x in 0..nx {
            let px = x as f32 * spacing - half_x;
            let py = y as f32 * spacing;
            spawn_cube(
                universe,
                spot,
                (px, py, z),
                (cube_scale, cube_scale, cube_scale),
                None,
                None,
            );
        }
    }

    // Label in front of the plane.
    let label = format!(
        "XY plane: {}x{}\nopacity: {:.2}\nmulti-layer: false",
        nx, ny, opacity
    );
    let label_x = spot_pos.0 - (half_x + 0.6);
    let label_y = spot_pos.1 + (ny as f32 * spacing) + 1.2;
    let label_z = spot_pos.2 - 2.5;
    spawn_text_label_with_bg(universe, (label_x, label_y, label_z), &label, 0.50);
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Dark brown / pink background.
    let bg = universe.world.add_component(BackgroundColorComponent::new());
    let bg_c = universe.world.add_component(ColorComponent::rgba(0.22, 0.08, 0.10, 1.0));
    let _ = universe.world.add_child(bg, bg_c);
    universe.add(bg);

    // Ambient so unlit areas aren't black.
    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.35, 0.35, 0.40));
    universe.add(ambient);

    // A bright overhead light.
    let light_t = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, 18.0, 10.0)
            .with_scale(0.2, 0.2, 0.2),
    );
    let light = universe.world.add_component(
        PointLightComponent::new()
            .with_color(1.0, 1.0, 1.0)
            .with_intensity(1.6)
            .with_distance(80.0),
    );
    let _ = universe.attach(light_t, light);
    universe.add(light_t);

    // --- Camera rig ---
    // I { T { C3D { with_fps_rotation with_roll_axis_y } } }
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(3.0));
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_roll_axis_y()
            .with_fps_rotation(),
    );
    let _ = universe.attach(input, input_mode);

    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 6.0, 20.0));
    let _ = universe.attach(input, rig_transform);

    let camera = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(rig_transform, camera);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);

    universe.add(input);

    // --- Ground ---
    let ground = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, -0.6, 0.0)
            .with_scale(60.0, 1.0, 60.0),
    );
    let ground_r = universe.world.add_component(RenderableComponent::cube());
    let ground_c = universe
        .world
        .add_component(ColorComponent::rgba(0.95, 0.95, 0.95, 1.0));
    let _ = universe.attach(ground, ground_r);
    let _ = universe.attach(ground_r, ground_c);
    universe.add(ground);

    // --- Opaque yellow cubes behind (contrast reference) ---
    for i in 0..6 {
        let x = -10.0 + i as f32 * 4.0;
        spawn_cube(
            &mut universe,
            ground,
            (x, 1.2, -18.0),
            (2.0, 2.0, 2.0),
            Some((1.0, 0.92, 0.15, 1.0)),
            None,
        );
    }

    // --- Demo spots ---
    // Left of the left big spot: a single-layer transparent XY plane (16x16).
    spawn_demo_xy_plane(&mut universe, (-14.0, 0.0, 0.0), 0.50, 16, 16, 0.0);

    // Big spots: 8x8x8
    // Left: single-layer transparent (instanced)
    spawn_demo_spot(&mut universe, (-4.0, 0.0, 0.0), 0.50, false, 8);

    // Right: multi-layer transparent (sorted)
    spawn_demo_spot(&mut universe, (4.0, 0.0, 0.0), 0.50, true, 8);

    // Small spots: opaque 1x4 strip + transparent 1x4 strip (along Z).
    spawn_demo_strip_pair(&mut universe, (-4.0, 0.0, 10.0), false);
    spawn_demo_strip_pair(&mut universe, (4.0, 0.0, 10.0), true);

    // Process init-time registrations.
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
