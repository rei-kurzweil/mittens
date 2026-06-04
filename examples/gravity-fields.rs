use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn cube_renderable_sibling_of_collider(
    world: &engine::ecs::World,
    collider_cid: engine::ecs::ComponentId,
) -> Option<engine::ecs::ComponentId> {
    let t = world.parent_of(collider_cid)?;
    for &sib in world.children_of(t).iter() {
        if sib == collider_cid {
            continue;
        }
        let Some(r) =
            world.get_component_by_id_as::<engine::ecs::component::RenderableComponent>(sib)
        else {
            continue;
        };
        if r.renderable.base_mesh == engine::graphics::primitives::CpuMeshHandle::CUBE {
            return Some(sib);
        }
    }
    None
}

fn set_renderable_color_rgba(
    world: &mut engine::ecs::World,
    emit: &mut dyn engine::ecs::SignalEmitter,
    renderable_cid: engine::ecs::ComponentId,
    rgba: [f32; 4],
) {
    // Prefer existing ColorComponent.
    let existing = world
        .children_of(renderable_cid)
        .iter()
        .copied()
        .find(|&ch| {
            world
                .get_component_by_id_as::<engine::ecs::component::ColorComponent>(ch)
                .is_some()
        });

    if let Some(color_cid) = existing {
        if let Some(c) =
            world.get_component_by_id_as_mut::<engine::ecs::component::ColorComponent>(color_cid)
        {
            c.rgba = rgba;
            emit.push_intent_now(
                color_cid,
                engine::ecs::IntentValue::RegisterColor {
                    component_ids: vec![color_cid],
                },
            );
        }
        return;
    }

    // Fallback: add a ColorComponent child if missing.
    let color_cid = world.register(engine::ecs::component::ColorComponent::rgba(
        rgba[0], rgba[1], rgba[2], rgba[3],
    ));
    let _ = world.add_child(renderable_cid, color_cid);
    emit.push_intent_now(
        color_cid,
        engine::ecs::IntentValue::RegisterColor {
            component_ids: vec![color_cid],
        },
    );
}

fn kinetic_response_child_of_collider(
    world: &engine::ecs::World,
    collider_cid: engine::ecs::ComponentId,
) -> Option<engine::ecs::ComponentId> {
    for &ch in world.children_of(collider_cid).iter() {
        if world
            .get_component_by_id_as::<engine::ecs::component::KineticResponseComponent>(ch)
            .is_some()
        {
            return Some(ch);
        }
    }
    None
}

fn on_collision_turn_white(
    world: &mut engine::ecs::World,
    emit: &mut dyn engine::ecs::SignalEmitter,
    signal: &engine::ecs::Signal,
) {
    let Some(engine::ecs::EventSignal::CollisionStarted { a, b, .. }) = signal.event.as_ref()
    else {
        return;
    };

    let self_collider = signal.scope;
    let other = if self_collider == *a {
        *b
    } else if self_collider == *b {
        *a
    } else {
        return;
    };

    // Only react to cube-vs-cube touches (ignore static walls/floor and non-renderable colliders like camera).
    let Some(other_cn) =
        world.get_component_by_id_as::<engine::ecs::component::CollisionComponent>(other)
    else {
        return;
    };
    if other_cn.mode == engine::ecs::component::CollisionMode::Static {
        return;
    }
    if cube_renderable_sibling_of_collider(world, other).is_none() {
        return;
    }

    let Some(self_renderable) = cube_renderable_sibling_of_collider(world, self_collider) else {
        return;
    };
    set_renderable_color_rgba(world, emit, self_renderable, [1.0, 1.0, 1.0, 1.0]);
}

fn on_collision_freeze_gravity(
    world: &mut engine::ecs::World,
    _emit: &mut dyn engine::ecs::SignalEmitter,
    signal: &engine::ecs::Signal,
) {
    let Some(engine::ecs::EventSignal::CollisionStarted { a, b, .. }) = signal.event.as_ref()
    else {
        return;
    };

    let self_collider = signal.scope;
    let other = if self_collider == *a {
        *b
    } else if self_collider == *b {
        *a
    } else {
        return;
    };

    // Only react to cube-vs-cube touches.
    let Some(other_cn) =
        world.get_component_by_id_as::<engine::ecs::component::CollisionComponent>(other)
    else {
        return;
    };
    if other_cn.mode == engine::ecs::component::CollisionMode::Static {
        return;
    }
    if cube_renderable_sibling_of_collider(world, other).is_none() {
        return;
    }

    let Some(response_cid) = kinetic_response_child_of_collider(world, self_collider) else {
        return;
    };
    if let Some(r) = world
        .get_component_by_id_as_mut::<engine::ecs::component::KineticResponseComponent>(
            response_cid,
        )
    {
        // Gravity is a cached per-responder coefficient; set it to 0 once touched.
        r.gravity_coefficient = 0.0;
    }
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let bg_color = universe
        .world
        .add_component(engine::ecs::component::BackgroundColorComponent::new());
    let bg_color_c = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.06, 0.04, 0.07, 1.0,
        ));
    let _ = universe.world.add_child(bg_color, bg_color_c);
    universe.add(bg_color);

    // overhead directional light
    let directional_light = universe.world.add_component(
        engine::ecs::component::DirectionalLightComponent::new()
            .with_color(0.16, 0.14, 0.12)
            .with_intensity(0.7),
    );
    let directional_light_t = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 1.0, 0.0),
    );
    let _ = universe.attach(directional_light_t, directional_light);
    universe.add(directional_light_t);

    // Simple input-driven camera.
    let input = universe
        .world
        .add_component(engine::ecs::component::InputComponent::new().with_speed(2.0));

    let cam_t = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 4.0, 10.0),
    );
    let cam = universe.world.add_component(
        engine::ecs::component::Camera3DComponent::new()
            // The default (150) clips background dressing (city + clouds).
            .with_far(600.0)
            .with_fov(70.0),
    );

    // Make the camera affect the collision system (same pattern as collision-perimeter).
    let cam_collision = universe
        .world
        .add_component(engine::ecs::component::CollisionComponent::RIGGED());
    let cam_response = universe
        .world
        .add_component(engine::ecs::component::KineticResponseComponent::slide());
    let cam_shape =
        universe
            .world
            .add_component(engine::ecs::component::CollisionShapeComponent::new(
                engine::ecs::component::CollisionShape::sphere_radius(0.25),
            ));

    let input_mode = universe.world.add_component(
        engine::ecs::component::InputTransformModeComponent::forward_z()
            .with_roll_axis_y()
            .with_fps_rotation(),
    );

    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, cam_t);
    let _ = universe.attach(cam_t, cam);
    let _ = universe.attach(cam_t, cam_collision);
    let _ = universe.attach(cam_collision, cam_response);
    let _ = universe.attach(cam_collision, cam_shape);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, cam_t);
    universe.add(input);

    let arena_half = 30.0;

    // --- Background world (occluded + lit) ---
    // Buildings are scene dressing and should not occlude foreground cubes.
    let bg_root = universe.world.add_component(
        engine::ecs::component::BackgroundComponent::new().with_occlusion_and_lighting(),
    );
    universe.add(bg_root);

    // Background ground plane (rendered in background pass) with top surface at y=0.
    // This should not occlude foreground due to depth clear between passes.
    let bg_ground_thickness = 0.20;
    let bg_ground_root_t = universe
        .world
        .add_component(engine::ecs::component::TransformComponent::new());
    let bg_ground_geom_t = universe.world.add_component(
        engine::ecs::component::TransformComponent::new()
            .with_position(0.0, -bg_ground_thickness, 0.0)
            .with_scale(
                arena_half * 10.0,
                bg_ground_thickness * 2.0,
                arena_half * 10.0,
            ),
    );
    let bg_ground_r = universe
        .world
        .add_component(engine::ecs::component::RenderableComponent::cube());
    let bg_ground_c = universe
        .world
        .add_component(engine::ecs::component::ColorComponent::rgba(
            0.03, 0.03, 0.035, 1.0,
        ));
    let _ = universe.attach(bg_root, bg_ground_root_t);
    let _ = universe.attach(bg_ground_root_t, bg_ground_geom_t);
    let _ = universe.attach(bg_ground_geom_t, bg_ground_r);
    let _ = universe.attach(bg_ground_r, bg_ground_c);

    // Background clouds (occluded + lit) using the shared example helper.
    // NOTE: `spawn_cloud_ring` places a ring centered at the parent transform's origin.
    // If we attach directly to `bg_root`, the ring can end up mostly out of view.
    // So we offset a cloud root forward along -Z and keep the radius within the camera far clip.
    let bg_cloud_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(
            0.0,
            0.0,
            -arena_half * 2.8,
        ),
    );
    let _ = universe.attach(bg_root, bg_cloud_root);

    let mut cloud_params = example_util::CloudRingParams::default();
    cloud_params.cloud_count = 9;
    cloud_params.radius = arena_half * 1.9;
    cloud_params.center_y = 18.0;
    cloud_params.puffs_per_cloud = 26;
    cloud_params.angle_jitter = 0.35;
    cloud_params.high_y_probability = 0.35;
    cloud_params.high_y_multiplier = 1.35;
    cloud_params.seed = 0xC10_71A5u32;
    example_util::spawn_cloud_ring(&mut universe, bg_cloud_root, cloud_params);

    // Foreground city: NOT background-layer.
    let fg_city_root = universe
        .world
        .add_component(engine::ecs::component::TransformComponent::new());
    universe.add(fg_city_root);

    // Background city: nested under the BackgroundComponent.
    let bg_city_root = universe
        .world
        .add_component(engine::ecs::component::TransformComponent::new());
    let _ = universe.attach(bg_root, bg_city_root);

    fn hash_u32(mut x: u32) -> u32 {
        // Small, deterministic integer hash (no external RNG dependency).
        x ^= x >> 16;
        x = x.wrapping_mul(0x7FEB_352D);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846C_A68B);
        x ^= x >> 16;
        x
    }

    fn rand01(seed: u32) -> f32 {
        let x = hash_u32(seed);
        (x as f32) / (u32::MAX as f32)
    }

    fn spawn_floor(
        universe: &mut engine::Universe,
        building_root: engine::ecs::ComponentId,
        floor_center_y: f32,
        footprint_x: f32,
        footprint_z: f32,
        floor_h: f32,
        floor_rgb: [f32; 4],
        building_scale: f32,
        window_mask: &[bool],
        window_height_coeff: f32,
        seed: u32,
    ) {
        // Floor geometry.
        // IMPORTANT: don't put non-uniform scale on the node that windows attach to,
        // otherwise child window meshes inherit the scale and look stretched.
        let floor_root_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(
                0.0,
                floor_center_y,
                0.0,
            ),
        );
        let floor_geom_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_scale(
                footprint_x,
                floor_h,
                footprint_z,
            ),
        );
        let floor_r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let floor_c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                floor_rgb[0],
                floor_rgb[1],
                floor_rgb[2],
                floor_rgb[3],
            ));

        let _ = universe.attach(building_root, floor_root_t);
        let _ = universe.attach(floor_root_t, floor_geom_t);
        let _ = universe.attach(floor_geom_t, floor_r);
        let _ = universe.attach(floor_r, floor_c);

        // Windows: a single clean row per floor.
        // Each floor receives a Vec<bool> (window_mask) that indicates which window slots are present.
        let slots = window_mask.len().max(1);
        let building_scale = building_scale.max(0.01);
        let window_height_coeff = window_height_coeff.clamp(0.05, 2.0);

        // Keep window size independent from floor height; floor height is already controlled by height_scale.
        let nominal_window = 0.32 * building_scale;
        let window_d = 0.06 * building_scale;

        // Compute per-side spacing and clamp window size to fit.
        let margin_x = (0.65 * nominal_window).min(0.18 * footprint_x);
        let margin_z = (0.65 * nominal_window).min(0.18 * footprint_z);
        let available_x = (footprint_x - 2.0 * margin_x).max(nominal_window);
        let available_z = (footprint_z - 2.0 * margin_z).max(nominal_window);
        let step_x = available_x / (slots as f32);
        let step_z = available_z / (slots as f32);

        let window_wx = nominal_window.min(step_x * 0.85);
        let window_wz = nominal_window.min(step_z * 0.85);
        let window_h = (nominal_window * window_height_coeff).min(floor_h * 0.80);

        let warm = 0.70 + 0.25 * rand01(seed ^ 0x1F12_6A77);
        let window_rgb = [1.0, 0.78 * warm, 0.48 * warm, 1.0];

        let mut spawn_window = |local_pos: (f32, f32, f32), local_scale: (f32, f32, f32)| {
            let window_t = universe.world.add_component(
                engine::ecs::component::TransformComponent::new()
                    .with_position(local_pos.0, local_pos.1, local_pos.2)
                    .with_scale(local_scale.0, local_scale.1, local_scale.2),
            );
            let window_r = universe
                .world
                .add_component(engine::ecs::component::RenderableComponent::cube());
            let window_c =
                universe
                    .world
                    .add_component(engine::ecs::component::ColorComponent::rgba(
                        window_rgb[0],
                        window_rgb[1],
                        window_rgb[2],
                        window_rgb[3],
                    ));
            let window_e = universe
                .world
                .add_component(engine::ecs::component::EmissiveComponent::on());

            let _ = universe.attach(floor_root_t, window_t);
            let _ = universe.attach(window_t, window_r);
            let _ = universe.attach(window_r, window_c);
            let _ = universe.attach(window_r, window_e);
        };

        // Place the row at the center of the floor segment.
        let wy = 0.0;

        for i in 0..slots {
            if !window_mask[i] {
                continue;
            }
            let tx = -0.5 * footprint_x + margin_x + (i as f32 + 0.5) * step_x;
            let tz = -0.5 * footprint_z + margin_z + (i as f32 + 0.5) * step_z;

            // +Z/-Z faces: windows laid out along X.
            for z_sign in [1.0_f32, -1.0_f32] {
                spawn_window(
                    (tx, wy, z_sign * (footprint_z * 0.5 + window_d * 0.6)),
                    (window_wx, window_h, window_d),
                );
            }

            // +X/-X faces: windows laid out along Z.
            for x_sign in [1.0_f32, -1.0_f32] {
                spawn_window(
                    (x_sign * (footprint_x * 0.5 + window_d * 0.6), wy, tz),
                    (window_d, window_h, window_wz),
                );
            }
        }
    }

    fn spawn_building(
        universe: &mut engine::Universe,
        parent: engine::ecs::ComponentId,
        x: f32,
        y_offset: f32,
        z: f32,
        floors: u32,
        seed: u32,
        building_scale: f32,
        height_scale: f32,
        total_windows_per_floor_per_building_side: u32,
        window_height_coeff: f32,
    ) {
        let building_scale = building_scale.max(0.01);
        let height_scale = height_scale.max(0.01);

        let floors = floors.max(1);
        let windows_per_side = total_windows_per_floor_per_building_side.max(1) as usize;

        let footprint_x = 3.2 * building_scale;
        let footprint_z = 3.2 * building_scale;
        let floor_h = 1.15 * building_scale * height_scale;

        let base_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(x, y_offset, z),
        );
        let _ = universe.attach(parent, base_t);

        // Slight per-building tint.
        let tint = 0.03 * (rand01(seed ^ 0xA511_E9B3) - 0.5);
        let base_rgb = [0.08 + tint, 0.085 + tint, 0.095 + tint, 1.0];

        // Probability that a given window slot exists.
        // (False means no window cube at that slot.)
        let window_fill_probability = 0.80_f32;

        for floor_i in 0..floors {
            let floor_seed = seed ^ floor_i.wrapping_mul(0x9E37_79B9) ^ 0xB17D_1E55;

            let mut window_mask = Vec::with_capacity(windows_per_side);
            for i in 0..windows_per_side {
                let s = floor_seed ^ (i as u32).wrapping_mul(0x85EB_CA6B) ^ 0x243F_6A88;
                window_mask.push(rand01(s) < window_fill_probability);
            }

            let floor_center_y = floor_h * (floor_i as f32) + floor_h * 0.5;
            spawn_floor(
                universe,
                base_t,
                floor_center_y,
                footprint_x,
                footprint_z,
                floor_h,
                base_rgb,
                building_scale,
                &window_mask,
                window_height_coeff,
                floor_seed,
            );
        }
    }

    fn spawn_city(
        universe: &mut engine::Universe,
        parent: engine::ecs::ComponentId,
        columns: u32,
        spacing: f32,
        position_scale: f32,
        building_scale: f32,
        height_scale: f32,
        seed_base: u32,
        y_offset: f32,
        total_windows_per_floor_per_building_side: u32,
        window_height_coeff: f32,
    ) {
        let columns = columns.max(1);
        let half = (columns as i32) / 2;

        for gz in -half..=half {
            for gx in -half..=half {
                // Hole in the middle.
                if gx == 0 && gz == 0 {
                    continue;
                }

                let seed = seed_base
                    ^ ((gx + half) as u32).wrapping_mul(0x9E37_79B9)
                    ^ ((gz + half) as u32).wrapping_mul(0x85EB_CA6B)
                    ^ 0xB17D_1E55;

                let floors = 3 + (hash_u32(seed) % 10); // 3..=12 floors

                // Centered around the origin.
                // - position_scale controls how far out the city sits (positions only)
                // - building_scale controls building footprint/window sizes
                let x = (gx as f32) * spacing * position_scale;
                let z = (gz as f32) * spacing * position_scale;

                spawn_building(
                    universe,
                    parent,
                    x,
                    y_offset,
                    z,
                    floors,
                    seed,
                    building_scale,
                    height_scale,
                    total_windows_per_floor_per_building_side,
                    window_height_coeff,
                );
            }
        }
    }

    // // Foreground city: 5x5 (hole in center), scaled out to sit around the floor.
    // spawn_city(
    //     &mut universe,
    //     fg_city_root,
    //     5,
    //     7.0,
    //     2.0,
    //     1.0,
    //     1.0,
    //     0xC170_0001u32,
    //     3,
    //     2,
    //     -0.0
    // );

    // Background city: larger grid, farther out, taller buildings.
    spawn_city(
        &mut universe,
        bg_city_root,
        11,
        3.0,
        3.0,
        0.45,
        1.0,
        0xC170_0002u32,
        -2.0,
        6,
        0.75,
    );

    fn spawn_street_light(universe: &mut engine::Universe) -> engine::ecs::ComponentId {
        // Dimensions (world units).
        let shaft_height = 6.0;
        let shaft_thickness = 0.22;
        let arm_length = shaft_height / 3.0;
        let arm_thickness = 0.18;

        // Colors.
        let pole_color = [0.35, 0.35, 0.38, 1.0];
        let housing_color = [0.18, 0.18, 0.20, 1.0];
        let diffuser_color = [1.0, 1.0, 1.0, 1.0];

        // Model root at origin.
        // The caller is expected to position + rotate this model using an external placement transform.
        // Street light is authored facing +X (arm extends toward +X).
        let root_t = universe
            .world
            .add_component(engine::ecs::component::TransformComponent::new());

        // Vertical shaft (split root/geom so scaling doesn't affect the arm).
        let shaft_root_t = universe
            .world
            .add_component(engine::ecs::component::TransformComponent::new());
        let shaft_geom_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(0.0, shaft_height * 0.5, 0.0)
                .with_scale(shaft_thickness, shaft_height, shaft_thickness),
        );
        let shaft_r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let shaft_c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                pole_color[0],
                pole_color[1],
                pole_color[2],
                pole_color[3],
            ));

        let _ = universe.attach(root_t, shaft_root_t);
        let _ = universe.attach(shaft_root_t, shaft_geom_t);
        let _ = universe.attach(shaft_geom_t, shaft_r);
        let _ = universe.attach(shaft_r, shaft_c);

        // Horizontal arm at the top; starts at the pole and extends toward +X.
        let arm_root_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(0.0, shaft_height, 0.0),
        );
        let arm_geom_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(arm_length * 0.5, -arm_thickness * 0.5, 0.0)
                .with_scale(arm_length, arm_thickness, arm_thickness),
        );
        let arm_r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let arm_c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                pole_color[0],
                pole_color[1],
                pole_color[2],
                pole_color[3],
            ));

        let _ = universe.attach(shaft_root_t, arm_root_t);
        let _ = universe.attach(arm_root_t, arm_geom_t);
        let _ = universe.attach(arm_geom_t, arm_r);
        let _ = universe.attach(arm_r, arm_c);

        // Light housing at the end of the arm (+X end).
        let housing_root_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(arm_length, 0.0, 0.0),
        );
        let housing_geom_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(0.5, 0.0, 0.0)
                // 2x1x1 box in (z, x, y) -> (x, y, z) = (1, 1, 2)
                .with_scale(2.0, 1.0, 1.0),
        );
        let housing_r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let housing_c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                housing_color[0],
                housing_color[1],
                housing_color[2],
                housing_color[3],
            ));

        let _ = universe.attach(arm_root_t, housing_root_t);
        let _ = universe.attach(housing_root_t, housing_geom_t);
        let _ = universe.attach(housing_geom_t, housing_r);
        let _ = universe.attach(housing_r, housing_c);

        // White diffuser cube (slightly smaller, slightly lower).
        let diffuser_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(0.5, -0.35, 0.0)
                .with_scale(1.85, 0.55, 0.70),
        );
        let diffuser_r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let diffuser_c =
            universe
                .world
                .add_component(engine::ecs::component::ColorComponent::rgba(
                    diffuser_color[0],
                    diffuser_color[1],
                    diffuser_color[2],
                    diffuser_color[3],
                ));

        let diffuser_emissive = universe
            .world
            .add_component(engine::ecs::component::EmissiveComponent::on());
        let _ = universe.attach(diffuser_r, diffuser_emissive);

        let _ = universe.attach(housing_root_t, diffuser_t);
        let _ = universe.attach(diffuser_t, diffuser_r);
        let _ = universe.attach(diffuser_r, diffuser_c);

        // Yellow (slightly red) point light.
        let light = universe.world.add_component(
            engine::ecs::component::PointLightComponent::new()
                .with_color(1.0, 0.82, 0.55)
                .with_intensity(5.0)
                .with_distance(40.0),
        );
        let _ = universe.attach(diffuser_t, light);

        root_t
    }

    // Spawn 6 street lights around the arena.
    // Arms point inward from each side.
    let light_x = arena_half - 2.0;
    for z in [-10.0, 0.0, 10.0] {
        // Left side: street light model faces +X by default, so it points inward.
        let left_place = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(-light_x, 0.0, z),
        );
        let left_model = spawn_street_light(&mut universe);
        let _ = universe.attach(left_place, left_model);
        universe.add(left_place);

        // Right side: rotate 180° around Y so the arm points toward -X (inward).
        let right_place = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(light_x, 0.0, z)
                .with_rotation_euler(0.0, std::f32::consts::PI, 0.0),
        );
        let right_model = spawn_street_light(&mut universe);
        let _ = universe.attach(right_place, right_model);
        universe.add(right_place);
    }

    // Big floor.
    {
        let floor_half = arena_half;
        let thickness = 0.20;

        // Split transforms so scaling the floor does not scale any potential children.
        // Top surface at y=0.
        let floor_root_t = universe
            .world
            .add_component(engine::ecs::component::TransformComponent::new());
        let floor_geom_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(0.0, -thickness, 0.0)
                .with_scale(floor_half * 2.0, thickness * 2.0, floor_half * 2.0),
        );
        let floor_r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let floor_color =
            universe
                .world
                .add_component(engine::ecs::component::ColorComponent::rgba(
                    0.08, 0.08, 0.09, 1.0,
                ));

        let floor_cn = universe
            .world
            .add_component(engine::ecs::component::CollisionComponent::STATIC());
        let floor_shape =
            universe
                .world
                .add_component(engine::ecs::component::CollisionShapeComponent::new(
                    engine::ecs::component::CollisionShape::cube_half_extents([
                        floor_half, thickness, floor_half,
                    ]),
                ));

        let _ = universe.attach(floor_root_t, floor_geom_t);
        let _ = universe.attach(floor_geom_t, floor_r);
        let _ = universe.attach(floor_r, floor_color);
        let _ = universe.attach(floor_geom_t, floor_cn);
        let _ = universe.attach(floor_cn, floor_shape);
        universe.add(floor_root_t);
    }

    // Square boundary walls around the floor edges.
    // Note: STATIC colliders don't need KineticResponse; they still collide with kinematic/rigged.
    fn spawn_boundary_wall(
        universe: &mut engine::Universe,
        x: f32,
        y: f32,
        z: f32,
        half_extents: [f32; 3],
        color: [f32; 4],
    ) {
        let t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(
                    half_extents[0] * 2.0,
                    half_extents[1] * 2.0,
                    half_extents[2] * 2.0,
                ),
        );
        let r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                color[0], color[1], color[2], color[3],
            ));

        let cn = universe
            .world
            .add_component(engine::ecs::component::CollisionComponent::STATIC());
        let shape =
            universe
                .world
                .add_component(engine::ecs::component::CollisionShapeComponent::new(
                    engine::ecs::component::CollisionShape::cube_half_extents(half_extents),
                ));

        let _ = universe.attach(t, r);
        let _ = universe.attach(r, c);
        let _ = universe.attach(t, cn);
        let _ = universe.attach(cn, shape);
        universe.add(t);
    }

    {
        let wall_half_height = 2.5;
        let wall_half_thickness = 0.30;
        let wall_center_y = wall_half_height; // bottom at y=0
        let wall_half_len = arena_half;

        // Subtle visible walls.
        let wall_color = [0.10, 0.10, 0.12, 0.30];

        // +Z / -Z
        spawn_boundary_wall(
            &mut universe,
            0.0,
            wall_center_y,
            arena_half,
            [wall_half_len, wall_half_height, wall_half_thickness],
            wall_color,
        );
        spawn_boundary_wall(
            &mut universe,
            0.0,
            wall_center_y,
            -arena_half,
            [wall_half_len, wall_half_height, wall_half_thickness],
            wall_color,
        );

        // +X / -X
        spawn_boundary_wall(
            &mut universe,
            arena_half,
            wall_center_y,
            0.0,
            [wall_half_thickness, wall_half_height, wall_half_len],
            wall_color,
        );
        spawn_boundary_wall(
            &mut universe,
            -arena_half,
            wall_center_y,
            0.0,
            [wall_half_thickness, wall_half_height, wall_half_len],
            wall_color,
        );
    }

    fn spawn_falling_cube(
        universe: &mut engine::Universe,
        parent: engine::ecs::ComponentId,
        x: f32,
        y: f32,
        z: f32,
        s: f32,
        r: f32,
        g: f32,
        b: f32,
    ) {
        let t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(s, s, s),
        );

        let renderable = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let color = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(r, g, b, 1.0));

        let cn = universe
            .world
            .add_component(engine::ecs::component::CollisionComponent::KINEMATIC());

        let response = universe.world.add_component({
            let mut r = engine::ecs::component::KineticResponseComponent::push()
                .with_push_strength(3.0)
                .with_friction_y(18.0);
            // Allow higher fall speeds so gravity coefficients are visible.
            r.max_speed = 80.0;
            r
        });

        let shape =
            universe
                .world
                .add_component(engine::ecs::component::CollisionShapeComponent::new(
                    engine::ecs::component::CollisionShape::cube_half_extents([
                        0.5 * s,
                        0.5 * s,
                        0.5 * s,
                    ]),
                ));

        let _ = universe.attach(t, renderable);
        let _ = universe.attach(renderable, color);

        let _ = universe.attach(t, cn);
        let _ = universe.attach(cn, response);
        let _ = universe.attach(cn, shape);

        let _ = universe.attach(parent, t);
    }

    // Three gravity fields, each with 20 cubes.
    let field_low = universe
        .world
        .add_component(engine::ecs::component::GravityComponent::new().with_coefficient(0.125));
    let field_mid = universe
        .world
        .add_component(engine::ecs::component::GravityComponent::new().with_coefficient(0.5));
    let field_high = universe
        .world
        .add_component(engine::ecs::component::GravityComponent::new().with_coefficient(1.0));

    universe.add(field_low);
    universe.add(field_mid);
    universe.add(field_high);

    fn spawn_field(
        universe: &mut engine::Universe,
        field: engine::ecs::ComponentId,
        base_x: f32,
        base_z: f32,
        cube_color: [f32; 3],
    ) {
        let spacing = 0.7;
        let start_x = base_x - 2.0 * spacing;
        let start_z = base_z - 1.5 * spacing;

        for i in 0..20 {
            let ix = (i % 5) as f32;
            let iz = (i / 5) as f32;

            let x = start_x + ix * spacing;
            let z = start_z + iz * spacing;
            let y = 2.0 + iz * 0.6 + ix * 0.1;

            spawn_falling_cube(
                universe,
                field,
                x,
                y,
                z,
                0.5,
                cube_color[0],
                cube_color[1],
                cube_color[2],
            );
        }

        // Visual marker for the field center.
        let marker_t = universe.world.add_component(
            engine::ecs::component::TransformComponent::new()
                .with_position(base_x, 0.1, base_z)
                .with_scale(0.3, 0.02, 0.3),
        );
        let marker_r = universe
            .world
            .add_component(engine::ecs::component::RenderableComponent::cube());
        let marker_c = universe
            .world
            .add_component(engine::ecs::component::ColorComponent::rgba(
                cube_color[0],
                cube_color[1],
                cube_color[2],
                1.0,
            ));
        let _ = universe.attach(marker_t, marker_r);
        let _ = universe.attach(marker_r, marker_c);
        universe.add(marker_t);
    }

    spawn_field(&mut universe, field_low, -8.0, 0.0, [0.3, 0.6, 1.0]);
    spawn_field(&mut universe, field_mid, 0.0, 0.0, [0.4, 1.0, 0.4]);
    spawn_field(&mut universe, field_high, 8.0, 0.0, [1.0, 0.5, 0.2]);

    // Group-scoped collision behaviors.
    // - Low + High fields: cubes turn white after colliding with another cube.
    // - Mid field: cubes lose gravity after colliding with another cube.
    universe.add_signal_handler(
        engine::ecs::SignalKind::CollisionStarted,
        field_low,
        on_collision_turn_white,
    );
    universe.add_signal_handler(
        engine::ecs::SignalKind::CollisionStarted,
        field_high,
        on_collision_turn_white,
    );
    universe.add_signal_handler(
        engine::ecs::SignalKind::CollisionStarted,
        field_mid,
        on_collision_freeze_gravity,
    );

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
