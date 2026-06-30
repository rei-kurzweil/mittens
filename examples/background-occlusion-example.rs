use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn rand01(seed: u32) -> f32 {
    (hash_u32(seed) as f32) / (u32::MAX as f32)
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Light purple-red background so the occlusion reads.
    let clear = universe
        .world
        .add_component(engine::ecs::component::BackgroundColorComponent::new());
    let clear_c = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.62, 0.38, 0.56, 1.0,
        ));
    let _ = universe.world.add_child(clear, clear_c);
    universe.add(clear);

    // A bit of ambient so the cluster volume reads.
    let ambient = universe
        .world
        .add_component(engine::ecs::component::AmbientLightComponent::rgb(
            0.20, 0.15, 0.3,
        ));
    universe.add(ambient);

    // --- Camera rig (WASD/QE) ---
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(2.0));
    let input_mode = universe.world.add_component(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    let rig_transform = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 6.0),
    );
    let _ = universe.attach(input, rig_transform);

    let camera3d = universe.world.add_component(
        engine::ecs::component::Camera3DComponent::new()
            .with_far(200.0)
            .with_fov(70.0),
    );
    let _ = universe.attach(rig_transform, camera3d);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);

    // Key directional light toward the cloud volume.
    //
    // Directional lights encode their direction in the node's world position.
    // The shader normalizes this vector.
    let sun = universe.world.add_component(
        engine::ecs::component::DirectionalLightComponent::new()
            .with_intensity(1.6)
            .with_color(1.0, 0.98, 0.92),
    );
    // Pointing from the clouds back toward the camera a bit (+Z), and slightly from above.
    let sun_dir = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.15, 0.65, 0.75),
    );
    let _ = universe.attach(sun_dir, sun);

    // A couple point lights to fill/shade the cloud volume.
    let light_a = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(80.0)
            .with_intensity(3.0)
            .with_color(0.9, 0.95, 1.0),
    );
    let light_a_tx = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(8.0, 8.0, 5.0),
    );
    let _ = universe.attach(light_a_tx, light_a);

    let light_b = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(80.0)
            .with_intensity(2.2)
            .with_color(1.0, 0.9, 0.85),
    );
    let light_b_tx = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(-10.0, 4.0, -6.0),
    );
    let _ = universe.attach(light_b_tx, light_b);

    universe.add(input);
    universe.add(sun_dir);
    universe.add(light_a_tx);
    universe.add(light_b_tx);

    let cube_mesh = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Cube);

    // --- Background occluded+lit world ---
    // Renderables under this node participate in a background stage that depth-writes for
    // self-occlusion + uses the normal lighting shader inputs.
    let bg_root = universe.world.add_component(
        engine::ecs::component::BackgroundComponent::new().with_occlusion_and_lighting(),
    );
    universe.add(bg_root);

    // Create overlapping cube clusters like cloud puffs.
    // Place them generally in front of the camera (negative Z).
    // Spawn two groups on opposite X sides.
    let cluster_count: u32 = 9;
    for (group_i, group_x) in [-16.0_f32, 16.0_f32].into_iter().enumerate() {
        let group_tx = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(group_x, 0.0, 0.0),
        );
        let _ = universe.attach(bg_root, group_tx);

        for cluster_i in 0..cluster_count {
            let seed = (0xC10u32 ^ (group_i as u32).wrapping_mul(0x9E37_79B9))
                ^ (cluster_i.wrapping_mul(7919));

            let cx = (rand01(seed ^ 0xA341_316C) - 0.5) * 18.0;
            let cy = (rand01(seed ^ 0xC801_3EA4) - 0.5) * 10.0 + 2.0;
            let cz = -18.0 - rand01(seed ^ 0xB529_7A4D) * 26.0;

            let center_tx = universe.world.add_component(
                engine::ecs::component::TransformComponent::new().with_position(cx, cy, cz),
            );
            let _ = universe.attach(group_tx, center_tx);

            let puffs = 18u32;
            for puff_i in 0..puffs {
                let puff_seed = seed ^ puff_i.wrapping_mul(1_103_515_245);

                // Slightly ellipsoidal distribution.
                let ox = (rand01(puff_seed ^ 0x68bc_21eb) - 0.5) * 7.0;
                let oy = (rand01(puff_seed ^ 0x02e5_be93) - 0.5) * 3.0;
                let oz = (rand01(puff_seed ^ 0xa1d3_4f2b) - 0.5) * 7.0;

                let base = 0.7 + rand01(puff_seed ^ 0x9e37_79b9) * 2.6;
                let sx = base * (0.7 + rand01(puff_seed ^ 0x243f_6a88) * 0.8);
                let sy = base * (0.6 + rand01(puff_seed ^ 0x85a3_08d3) * 0.9);
                let sz = base * (0.7 + rand01(puff_seed ^ 0x1319_8a2e) * 0.8);

                let tx = universe.world.add_component(
                    engine::ecs::component::TransformComponent::new()
                        .with_position(ox, oy, oz)
                        .with_scale(sx, sy, sz),
                );
                let renderable =
                    universe
                        .world
                        .add_component(engine::ecs::component::RenderableComponent::new(
                            engine::graphics::primitives::Renderable::new(
                                cube_mesh,
                                engine::graphics::primitives::MaterialHandle::TOON_MESH,
                            ),
                        ));

                // Slight blue-grey variation.
                let t = rand01(puff_seed ^ 0x7f4a_7c15);
                let r = 0.55 + 0.10 * t;
                let g = 0.58 + 0.10 * t;
                let b = 0.66 + 0.12 * t;
                let color = universe
                    .world
                    .add_component(engine::ecs::component::ColorComponent::rgba(r, g, b, 1.0));

                let _ = universe.attach(center_tx, tx);
                let _ = universe.attach(tx, renderable);
                let _ = universe.attach(renderable, color);
            }
        }
    }

    // Foreground reference cube (should never be occluded by background depth).
    let fg_tx = universe.world.add_component(
        engine::ecs::component::TransformComponent::new()
            .with_position(0.0, -0.5, -4.0)
            .with_scale(1.2, 0.8, 1.2),
    );
    let fg_renderable =
        universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::new(
                engine::graphics::primitives::Renderable::new(
                    cube_mesh,
                    engine::graphics::primitives::MaterialHandle::TOON_MESH,
                ),
            ));
    let fg_color = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.9, 0.2, 0.2, 1.0,
        ));

    universe.add(fg_tx);
    let _ = universe.attach(fg_tx, fg_renderable);
    let _ = universe.attach(fg_renderable, fg_color);

    // Foreground opaque floor: 16x16 larger white cubes, 10 units below the reference cube.
    let floor_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, -10.5, -10.0),
    );
    universe.add(floor_root);

    let spacing = 10_f32;
    let half = 5_f32;
    for x in 0..16u32 {
        for z in 0..64u32 {
            let px = (x as f32 - half) * spacing;
            let pz = (z as f32 * -1.0 + half) * spacing;
            let tx = universe.world.add_component(
                engine::ecs::component::TransformComponent::new()
                    .with_position(px, -5.0, pz)
                    .with_scale(5.0, 0.5, 5.0),
            );
            let renderable =
                universe
                    .world
                    .add_component(engine::ecs::component::RenderableComponent::new(
                        engine::graphics::primitives::Renderable::new(
                            cube_mesh,
                            engine::graphics::primitives::MaterialHandle::TOON_MESH,
                        ),
                    ));
            let color = universe
                .world
                .add_component(engine::ecs::component::ColorComponent::rgba(
                    1.0, 1.0, 1.0, 1.0,
                ));

            let _ = universe.attach(floor_root, tx);
            let _ = universe.attach(tx, renderable);
            let _ = universe.attach(renderable, color);
        }
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
