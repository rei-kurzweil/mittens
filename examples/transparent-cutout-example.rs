use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, ColorComponent,
    InputComponent, InputTransformModeComponent, PointLightComponent, RenderableComponent,
    TextureComponent, TextureFilteringComponent, TransformComponent, TransparentCutoutComponent,
};

fn spawn_gold_cube(
    universe: &mut engine::Universe,
    parent: engine::ecs::ComponentId,
    position: (f32, f32, f32),
    scale: f32,
    color: (f32, f32, f32),
) {
    let t = universe.world.add_component(
        TransformComponent::new()
            .with_position(position.0, position.1, position.2)
            .with_scale(scale, scale, scale),
    );
    let r = universe.world.add_component(RenderableComponent::cube());
    let c = universe
        .world
        .add_component(ColorComponent::rgba(color.0, color.1, color.2, 1.0));

    let _ = universe.attach(parent, t);
    let _ = universe.attach(t, r);
    let _ = universe.attach(r, c);
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Orange/yellow-ish clear color so cutout edges read.
    let clear = universe
        .world
        .add_component(BackgroundColorComponent::new());
    let clear_c = universe
        .world
        .add_component(ColorComponent::rgba(0.98, 0.72, 0.22, 1.0));
    let _ = universe.world.add_child(clear, clear_c);
    universe.add(clear);

    // Warm-ish ambient so the gold cubes don’t go too dark.
    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.22, 0.16, 0.08));
    universe.add(ambient);

    // --- Camera rig (WASD/QE) ---
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(2.0));
    let input_mode = universe
        .world
        .add_component(InputTransformModeComponent::forward_z().with_roll_axis_y());
    let _ = universe.attach(input, input_mode);

    // Start a bit pulled back, looking toward the origin.
    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 5.0));
    let _ = universe.attach(input, rig_transform);

    let camera3d = universe
        .world
        .add_component(Camera3DComponent::new().with_far(200.0).with_fov(55.0));
    let _ = universe.attach(rig_transform, camera3d);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);
    universe.add(input);

    // Key light for toon shading.
    let light = universe.world.add_component(
        PointLightComponent::new()
            .with_distance(50.0)
            .with_intensity(2.2)
            .with_color(1.0, 0.98, 0.92),
    );
    let light_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(2.0, 3.0, 4.0));
    let _ = universe.attach(light_transform, light);
    universe.add(light_transform);

    // --- Transparent cutout: 100 cat faces in a 10x10 grid ---
    // Place them *behind* the starting camera position (camera starts at z=+5.0).
    // Not attached to the camera rig.
    let cat_grid_root = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 9.0));
    universe.add(cat_grid_root);

    let grid_w: i32 = 10;
    let grid_h: i32 = 10;
    let spacing: f32 = 0.7;
    let half_w = (grid_w as f32 - 1.0) * spacing * 0.5;
    let half_h = (grid_h as f32 - 1.0) * spacing * 0.5;

    for y in 0..grid_h {
        for x in 0..grid_w {
            // Topology: cat_grid_root { T_quad { R_quad { Texture + Filtering + Cutout + Color } } }
            let px = x as f32 * spacing - half_w;
            let py = y as f32 * spacing - half_h;

            let pz: f32 = (x as f32) % half_w;

            let quad_t = universe.world.add_component(
                TransformComponent::new()
                    .with_position(px, py, pz)
                    .with_scale(0.55, 0.55, 1.0),
            );
            let quad_r = universe.world.add_component(RenderableComponent::square());

            let quad_tex = universe.world.add_component(TextureComponent::with_uri(
                "assets/textures/cat-face-amused.dds",
            ));
            let quad_filtering = universe
                .world
                .add_component(TextureFilteringComponent::linear());
            let quad_cutout = universe
                .world
                .add_component(TransparentCutoutComponent::new());
            let quad_color = universe
                .world
                .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));

            let _ = universe.attach(cat_grid_root, quad_t);
            let _ = universe.attach(quad_t, quad_r);
            let _ = universe.attach(quad_r, quad_tex);
            let _ = universe.attach(quad_r, quad_filtering);
            let _ = universe.attach(quad_r, quad_cutout);
            let _ = universe.attach(quad_r, quad_color);
        }
    }

    // --- Gold/yellow cubes behind the quad ---
    // Parent them under a transform so it’s easy to tweak their depth.
    let cubes_root = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 4.6));

    // point light for cats
    let cat_light_tx = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 2.0, 7.0));

    let cat_light = universe.world.add_component(
        PointLightComponent::new()
            .with_distance(150.0)
            .with_intensity(1.5)
            .with_color(1.0, 0.98, 0.92),
    );
    let _ = universe.attach(cat_light_tx, cat_light);
    let _ = universe.attach(cubes_root, cat_light_tx);

    universe.add(cubes_root);

    let gold_a = (1.0, 0.86, 0.22);
    let gold_b = (1.0, 0.74, 0.10);
    let gold_c = (0.95, 0.92, 0.32);

    // A loose cluster that’s visible through the cutout (transparent) area.
    spawn_gold_cube(&mut universe, cubes_root, (-1.1, -0.3, -0.2), 0.45, gold_a);
    spawn_gold_cube(&mut universe, cubes_root, (1.0, -0.4, -0.4), 0.40, gold_b);
    spawn_gold_cube(&mut universe, cubes_root, (-0.2, 0.9, -0.6), 0.38, gold_c);
    spawn_gold_cube(&mut universe, cubes_root, (0.7, 0.6, -0.9), 0.32, gold_a);
    spawn_gold_cube(&mut universe, cubes_root, (-0.8, 0.4, -1.1), 0.36, gold_b);

    // Bigger orange/yellow cubes behind the cluster (to make the cutout depth obvious).
    let orange_gold_a = (1.0, 0.62, 0.10);
    let orange_gold_b = (1.0, 0.78, 0.18);
    spawn_gold_cube(
        &mut universe,
        cubes_root,
        (0.0, -0.1, -2.1),
        1.15,
        orange_gold_b,
    );
    spawn_gold_cube(
        &mut universe,
        cubes_root,
        (-1.8, 0.6, -2.6),
        0.95,
        orange_gold_a,
    );
    spawn_gold_cube(
        &mut universe,
        cubes_root,
        (1.9, 0.7, -2.9),
        1.05,
        orange_gold_b,
    );
    spawn_gold_cube(
        &mut universe,
        cubes_root,
        (0.9, 1.8, -3.2),
        0.90,
        orange_gold_a,
    );
    spawn_gold_cube(
        &mut universe,
        cubes_root,
        (-0.9, 1.7, -3.5),
        1.10,
        orange_gold_b,
    );

    // Process init-time registrations (loads textures, registers renderables, etc.).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
