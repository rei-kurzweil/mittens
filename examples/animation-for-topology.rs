use cat_engine::{engine, utils};

#[derive(Debug, Clone, Copy)]
struct GridLayout {
    origin: (f32, f32, f32),
    spacing: f32,
}

fn grid_anchor_local(layout: GridLayout, i: usize) -> (f32, f32, f32) {
    let gx = (i % 4) as f32;
    let gy = (i / 4) as f32;
    let x = (gx - 1.5) * layout.spacing;
    let y = (1.5 - gy) * layout.spacing;
    (layout.origin.0 + x, layout.origin.1 + y, layout.origin.2)
}

fn spawn_emissive_marker_cube(
    universe: &mut engine::Universe,
    parent: engine::ecs::ComponentId,
    local_pos: (f32, f32, f32),
    scale: f32,
    rgba: [f32; 4],
) {
    let tx = universe.world.register(
        engine::ecs::component::TransformComponent::new()
            .with_position(local_pos.0, local_pos.1, local_pos.2)
            .with_scale(scale, scale, scale),
    );
    let r = universe
        .world
        .register(engine::ecs::component::RenderableComponent::cube());
    let c = universe
        .world
        .register(engine::ecs::component::ColorComponent::rgba(
            rgba[0], rgba[1], rgba[2], rgba[3],
        ));
    let e = universe
        .world
        .register(engine::ecs::component::EmissiveComponent::on());

    let _ = universe.attach(parent, tx);
    let _ = universe.attach(tx, r);
    let _ = universe.attach(r, c);
    let _ = universe.attach(r, e);
}

/// Create a cube subtree ahead-of-time (not attached to the scene).
///
/// Returns the root `TransformComponent` id.
fn spawn_detached_cube_prefab(
    universe: &mut engine::Universe,
    scale: f32,
    rgba: [f32; 4],
) -> engine::ecs::ComponentId {
    let tx = universe.world.register(
        engine::ecs::component::TransformComponent::new().with_scale(scale, scale, scale),
    );
    let r = universe
        .world
        .register(engine::ecs::component::RenderableComponent::cube());
    let c = universe
        .world
        .register(engine::ecs::component::ColorComponent::rgba(
            rgba[0], rgba[1], rgba[2], rgba[3],
        ));
    let e = universe
        .world
        .register(engine::ecs::component::EmissiveComponent::on());

    let _ = universe.attach(tx, r);
    let _ = universe.attach(r, c);
    let _ = universe.attach(r, e);

    tx
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Minimal scene with a camera so the window opens.
    let clear = universe
        .world
        .register(engine::ecs::component::BackgroundColorComponent::rgba(
            0.06, 0.06, 0.07, 1.0,
        ));
    universe.add(clear);

    // Input-driven camera rig.
    let input = universe
        .world
        .register(engine::ecs::component::InputComponent::new().with_speed(2.0));
    let rig_transform = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 9.0));
    let input_mode = universe.world.register(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let camera3d = universe.world.register(
        engine::ecs::component::Camera3DComponent::new()
            .with_far(250.0)
            .with_fov(70.0),
    );
    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig_transform);
    let _ = universe.attach(rig_transform, camera3d);
    universe.add(input);

    // Light so we can see non-emissive materials.
    let light_tx = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(2.0, 3.5, 6.0));
    let light = universe.world.register(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(30.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.attach(light_tx, light);
    universe.add(light_tx);

    // ClockComponent drives the animation timeline in beats.
    let clock = universe
        .world
        .register(engine::ecs::component::ClockComponent::new().with_bpm(140.0));
    universe.add(clock);

    // Root for all visualization objects.
    let viz_root = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 0.0));
    universe.add(viz_root);

    let anchor_count = 16usize;
    let cube_pool_size = 8usize;

    // Three grids side-by-side.
    let layout_a = GridLayout {
        origin: (-6.0, 0.0, 0.0),
        spacing: 1.1,
    };
    let layout_b = GridLayout {
        origin: (0.0, 0.0, 0.0),
        spacing: 1.1,
    };
    let layout_c = GridLayout {
        origin: (6.0, 0.0, 0.0),
        spacing: 1.1,
    };

    // --- Grid A: detach + re-attach (reparent) ---
    let grid_a_root = universe
        .world
        .register(engine::ecs::component::TransformComponent::new());
    let _ = universe.attach(viz_root, grid_a_root);

    let mut anchors_a: Vec<engine::ecs::ComponentId> = Vec::with_capacity(anchor_count);
    for i in 0..anchor_count {
        let (x, y, z) = grid_anchor_local(layout_a, i);
        let anchor = universe
            .world
            .register(engine::ecs::component::TransformComponent::new().with_position(x, y, z));
        let _ = universe.attach(grid_a_root, anchor);
        anchors_a.push(anchor);
        spawn_emissive_marker_cube(
            &mut universe,
            anchor,
            (0.0, -0.35, 0.0),
            0.06,
            [0.18, 0.18, 0.22, 1.0],
        );
    }

    let mut cubes_a: Vec<engine::ecs::ComponentId> = Vec::with_capacity(cube_pool_size);
    for i in 0..cube_pool_size {
        let t = (i as f32) / ((cube_pool_size - 1) as f32).max(1.0);
        let rgba = [0.10, 0.40 + 0.50 * t, 0.90 - 0.70 * t, 1.0];
        cubes_a.push(spawn_detached_cube_prefab(&mut universe, 0.22, rgba));
    }

    let anim_a = universe
        .world
        .register(engine::ecs::component::AnimationComponent::new());
    for i in 0..anchor_count {
        let cube = cubes_a[i % cube_pool_size];
        let parent = anchors_a[i];

        // Explicit detach+attach to test topology changes via animations.

        // Put them on separate keyframes so ordering is time-deterministic.
        let beat_detach = i as f64;
        let beat_attach = i as f64 + 0.05;

        let kf_detach = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(beat_detach));
        let _ = universe.attach(anim_a, kf_detach);
        let detach_action = universe
            .world
            .register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::detach(vec![cube]),
            ));
        let _ = universe.attach(kf_detach, detach_action);

        let kf_attach = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(beat_attach));
        let _ = universe.attach(anim_a, kf_attach);
        let attach_action = universe
            .world
            .register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::attach(parent, cube),
            ));
        let _ = universe.attach(kf_attach, attach_action);
    }
    universe.add(anim_a);

    // --- Grid B: continuously spawn (attach_clone) + delete-behind (remove_child) ---
    //
    // Uses the new actions:
    // - `Action::attach_clone(parent, prefab_root)`
    // - `Action::remove_child(parent, index)`
    //
    // This avoids needing a pre-built pool of cube ComponentIds.
    let grid_b_root = universe
        .world
        .register(engine::ecs::component::TransformComponent::new());
    let _ = universe.attach(viz_root, grid_b_root);

    let mut anchors_b: Vec<engine::ecs::ComponentId> = Vec::with_capacity(anchor_count);
    for i in 0..anchor_count {
        let (x, y, z) = grid_anchor_local(layout_b, i);
        let anchor = universe
            .world
            .register(engine::ecs::component::TransformComponent::new().with_position(x, y, z));
        let _ = universe.attach(grid_b_root, anchor);
        anchors_b.push(anchor);

        // Marker is attached under the grid root (not under the anchor), so anchor child index 0
        // remains reserved for the dynamic cube subtree.
        spawn_emissive_marker_cube(
            &mut universe,
            grid_b_root,
            (x, y - 0.35, z),
            0.06,
            [0.22, 0.18, 0.18, 1.0],
        );
    }

    // Detached prefab that will be cloned on demand (reddish cube).
    let prefab_b = spawn_detached_cube_prefab(&mut universe, 0.20, [0.90, 0.25, 0.15, 1.0]);

    let steps_b = 32usize;
    let window = 8usize;

    let anim_b = universe
        .world
        .register(engine::ecs::component::AnimationComponent::new());

    // Phase 1: each beat, spawn a cube under an anchor; delete-behind by removing child(0).
    for i in 0..steps_b {
        let parent = anchors_b[i % anchor_count];

        let beat_attach = i as f64;
        let beat_remove = i as f64 + 0.05;

        let kf_attach = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(beat_attach));
        let _ = universe.attach(anim_b, kf_attach);

        let attach_action = universe
            .world
            .register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::attach_clone(parent, prefab_b),
            ));
        let _ = universe.attach(kf_attach, attach_action);

        if i >= window {
            let remove_parent = anchors_b[(i - window) % anchor_count];
            let kf_remove = universe
                .world
                .register(engine::ecs::component::KeyframeComponent::new(beat_remove));
            let _ = universe.attach(anim_b, kf_remove);

            let remove_action =
                universe
                    .world
                    .register(engine::ecs::component::ActionComponent::new(
                        engine::ecs::component::Action::remove_child(remove_parent, 0),
                    ));
            let _ = universe.attach(kf_remove, remove_action);
        }
    }

    // Phase 2: cleanup tail so the loop doesn't accumulate cubes.
    for j in 0..window {
        let remove_parent = anchors_b[(steps_b - window + j) % anchor_count];
        let beat_remove = steps_b as f64 + j as f64 + 0.05;

        let kf_remove = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(beat_remove));
        let _ = universe.attach(anim_b, kf_remove);

        let remove_action = universe
            .world
            .register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::remove_child(remove_parent, 0),
            ));
        let _ = universe.attach(kf_remove, remove_action);
    }

    universe.add(anim_b);

    // --- Grid C: move cubes via set_position (no topology changes) ---
    let grid_c_root = universe
        .world
        .register(engine::ecs::component::TransformComponent::new());
    let _ = universe.attach(viz_root, grid_c_root);

    let mut anchors_c: Vec<(f32, f32, f32)> = Vec::with_capacity(anchor_count);
    for i in 0..anchor_count {
        let (x, y, z) = grid_anchor_local(layout_c, i);
        anchors_c.push((x, y, z));

        let anchor = universe
            .world
            .register(engine::ecs::component::TransformComponent::new().with_position(x, y, z));
        let _ = universe.attach(grid_c_root, anchor);
        spawn_emissive_marker_cube(
            &mut universe,
            anchor,
            (0.0, -0.35, 0.0),
            0.06,
            [0.18, 0.22, 0.18, 1.0],
        );
    }

    let mut cubes_c: Vec<engine::ecs::ComponentId> = Vec::with_capacity(cube_pool_size);
    for i in 0..cube_pool_size {
        let t = (i as f32) / ((cube_pool_size - 1) as f32).max(1.0);
        let rgba = [0.25, 0.85 - 0.55 * t, 0.25 + 0.55 * t, 1.0];
        let cube_root = spawn_detached_cube_prefab(&mut universe, 0.22, rgba);
        let _ = universe.attach(grid_c_root, cube_root);
        cubes_c.push(cube_root);
    }

    let anim_c = universe
        .world
        .register(engine::ecs::component::AnimationComponent::new());
    for i in 0..anchor_count {
        let kf = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(i as f64));
        let _ = universe.attach(anim_c, kf);

        let cube = cubes_c[i % cube_pool_size];
        let (x, y, z) = anchors_c[i];
        let setpos_action = universe
            .world
            .register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::set_position(vec![cube], x, y, z),
            ));
        let _ = universe.attach(kf, setpos_action);
    }
    universe.add(anim_c);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
