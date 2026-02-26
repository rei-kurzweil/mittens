use cat_engine::{engine, utils};

fn quat_from_yaw(yaw_y: f32) -> [f32; 4] {
    // Y-axis rotation.
    let (s, c) = (0.5 * yaw_y).sin_cos();
    [0.0, s, 0.0, c]
}

fn quat_from_pitch(pitch_x: f32) -> [f32; 4] {
    // X-axis rotation.
    let (s, c) = (0.5 * pitch_x).sin_cos();
    [s, 0.0, 0.0, c]
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    // Hamilton product, xyzw.
    let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

fn ensure_emissive_on(
    world: &mut engine::ecs::World,
    queue: &mut engine::ecs::CommandQueue,
    renderable_cid: engine::ecs::ComponentId,
) {
    let existing = world
        .children_of(renderable_cid)
        .iter()
        .copied()
        .find(|&ch| {
            world
                .get_component_by_id_as::<engine::ecs::component::EmissiveComponent>(ch)
                .is_some()
        });

    if existing.is_some() {
        return;
    }

    let emissive_cid =
        world.register(engine::ecs::component::EmissiveComponent::on());
    let _ = world.add_child(renderable_cid, emissive_cid);
    queue.queue_register_emissive(emissive_cid);
}

fn ring_a_handler(
    world: &mut engine::ecs::World,
    queue: &mut engine::ecs::CommandQueue,
    env: &engine::ecs::Signal,
) {
    match &env.value {
        engine::ecs::SignalValue::Event(engine::ecs::EventSignal::RayIntersected {
            renderable,
            ..
        }) => {
            println!(
                "[ring_a_handler] fired scope={:?} renderable={:?}",
                env.scope, renderable
            );

            ensure_emissive_on(world, queue, *renderable);

            let action = engine::ecs::component::Action::set_color(
                vec![*renderable],
                [1.0, 1.0, 0.0, 1.0],
            );
            let mut action_system = engine::ecs::system::ActionSystem::new();
            let mut dummy_rx = engine::ecs::RxWorld::default();
            action_system.execute(world, queue, &mut dummy_rx, 0.0, &action);
        }
        _ => {}
    }
}

fn ring_b_handler(
    world: &mut engine::ecs::World,
    queue: &mut engine::ecs::CommandQueue,
    env: &engine::ecs::Signal,
) {
    match &env.value {
        engine::ecs::SignalValue::Event(engine::ecs::EventSignal::RayIntersected {
            renderable,
            ..
        }) => {
            println!(
                "[ring_b_handler] fired scope={:?} renderable={:?}",
                env.scope, renderable
            );

            ensure_emissive_on(world, queue, *renderable);

            let action = engine::ecs::component::Action::set_color(
                vec![*renderable],
                [0.0, 1.0, 1.0, 1.0],
            );
            let mut action_system = engine::ecs::system::ActionSystem::new();
            let mut dummy_rx = engine::ecs::RxWorld::default();
            action_system.execute(world, queue, &mut dummy_rx, 0.0, &action);
        }
        _ => {}
    }
}


fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Background.
    let bg_color = universe
        .world
        .register(engine::ecs::component::BackgroundColorComponent::rgba(
            0.1, 0.1, 0.1, 1.0,
        ));
    universe.add(bg_color);

    // Camera rig so we can see the scene.
    let input = universe
        .world
        .register(engine::ecs::component::InputComponent::new().with_speed(2.0));

    let rig_transform = universe.world.register(
        engine::ecs::component::TransformComponent::new()
            .with_position(0.0, 2.0, 8.0)
            .with_rotation_euler(-0.25, 0.0, 0.0),
    );

    let camera3d = universe
        .world
        .register(engine::ecs::component::Camera3DComponent::new());

    let input_mode = universe.world.register(
        engine::ecs::component::InputTransformModeComponent::forward_z()
            .with_roll_axis_y()
            .with_fps_rotation(),
    );

    let _ = universe.attach(input, input_mode);
    let _ = universe.attach(input, rig_transform);
    let _ = universe.attach(rig_transform, camera3d);
    universe.add(input);

    // Two rotating anchor transforms that the raycaster will be reparented under.
    // Each ring has its own anchor at a different height.
    let anchor_a = universe.world.register(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 1.0, 0.0),
    );
    let anchor_b = universe.world.register(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 2.2, 0.0),
    );

    // Visual markers for A/B.
    fn marker(universe: &mut engine::Universe, parent: engine::ecs::ComponentId, rgba: [f32; 4]) {
        let r = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let rc = universe
            .world
            .register(engine::ecs::component::RaycastableComponent::disabled());
        let c = universe.world.register(engine::ecs::component::ColorComponent::rgba(
            rgba[0], rgba[1], rgba[2], rgba[3],
        ));
        let e = universe
            .world
            .register(engine::ecs::component::EmissiveComponent::on());
        let t = universe.world.register(
            engine::ecs::component::TransformComponent::new().with_scale(0.15, 0.15, 0.15),
        );
        let _ = universe.attach(parent, t);
        let _ = universe.attach(t, r);
        let _ = universe.attach(r, rc);
        let _ = universe.attach(r, c);
        let _ = universe.attach(r, e);
    }

    marker(&mut universe, anchor_a, [0.9, 0.2, 0.2, 1.0]);
    marker(&mut universe, anchor_b, [0.2, 0.2, 0.9, 1.0]);

    universe.add(anchor_a);
    universe.add(anchor_b);

    // A ring of cubes around the origin to see which one gets hit.
    fn ring_cube(
        universe: &mut engine::Universe,
        ring_root: engine::ecs::ComponentId,
        x: f32,
        y: f32,
        z: f32,
        rgba: [f32; 4],
    ) {
        let t = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(0.35, 0.35, 0.35),
        );
        let r = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let rc = universe
            .world
            .register(engine::ecs::component::RaycastableComponent::enabled());
        let c = universe.world.register(engine::ecs::component::ColorComponent::rgba(
            rgba[0], rgba[1], rgba[2], rgba[3],
        ));
        let _ = universe.attach(ring_root, t);
        let _ = universe.attach(t, r);
        let _ = universe.attach(r, rc);
        let _ = universe.attach(r, c);
    }

    // Two rings: one per anchor height.
    let n = 28;
    let (radius_a, y_a) = (4.0, 1.0);
    let (radius_b, y_b) = (2.6, 2.2);

    let ring_a_root = universe
        .world
        .register(engine::ecs::component::TransformComponent::new());
    let ring_b_root = universe
        .world
        .register(engine::ecs::component::TransformComponent::new());

    for i in 0..n {
        let a = (i as f32) * (std::f32::consts::TAU / (n as f32));
        let (x, z) = (radius_a * a.cos(), radius_a * a.sin());
        let color = if i % 2 == 0 {
            [0.55, 0.20, 0.20, 1.0]
        } else {
            [0.20, 0.55, 0.20, 1.0]
        };
        ring_cube(&mut universe, ring_a_root, x, y_a, z, color);
    }

    for i in 0..n {
        let a = (i as f32) * (std::f32::consts::TAU / (n as f32));
        let (x, z) = (radius_b * a.cos(), radius_b * a.sin());
        let color = if i % 2 == 0 {
            [0.20, 0.25, 0.60, 1.0]
        } else {
            [0.20, 0.55, 0.55, 1.0]
        };
        ring_cube(&mut universe, ring_b_root, x, y_b, z, color);
    }

    // Init rings and attach scoped interaction handlers.
    universe.add(ring_a_root);
    universe.add(ring_b_root);
    universe.add_signal_handler(
        engine::ecs::SignalKind::RayIntersected,
        ring_a_root,
        ring_a_handler,
    );
    universe.add_signal_handler(
        engine::ecs::SignalKind::RayIntersected,
        ring_b_root,
        ring_b_handler,
    );

    // The raycaster component we will move between parents.
    // Source is inferred from topology:
    // - Under transforms A/B (no camera child) => parent-forward (-Z)
    // - Under camera rig transform (has camera child) => cursor-through-camera
    let raycaster = universe.world.register(
        engine::ecs::component::RayCastComponent::event_driven().with_max_distance(25.0),
    );

    // Global animation: move the raycaster between anchors.
    // Loop length is 8 beats (we include a noop keyframe at beat 7.0 to force loop_len=8).
    let anim_global = universe
        .world
        .register(engine::ecs::component::AnimationComponent::new());
    {
        // beat 0: attach to A.
        let kf0 = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(0.0));
        let act0 = universe.world.register(engine::ecs::component::ActionComponent::new(
            engine::ecs::component::Action::attach(anchor_a, raycaster),
        ));
        let _ = universe.attach(kf0, act0);
        let _ = universe.attach(anim_global, kf0);

        // beat 4: attach to B.
        let kf4 = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(4.0));
        let act4 = universe.world.register(engine::ecs::component::ActionComponent::new(
            engine::ecs::component::Action::attach(anchor_b, raycaster),
        ));
        let _ = universe.attach(kf4, act4);
        let _ = universe.attach(anim_global, kf4);

        // beat 7: noop to make loop_len=8.
        let kf7 = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(7.0));
        let noop = universe.world.register(engine::ecs::component::ActionComponent::new(
            engine::ecs::component::Action::default(),
        ));
        let _ = universe.attach(kf7, noop);
        let _ = universe.attach(anim_global, kf7);
    }
    universe.add(anim_global);

    // Ring A animation: rotate anchor A around Y (1 rev / 8 beats).
    // Also triggers Action::raycast(raycaster) on downbeats in the first half: beats 0,1,2,3.
    let anim_a = universe
        .world
        .register(engine::ecs::component::AnimationComponent::new());

    // Ring B animation: rotate anchor B differently (yaw + pitch).
    // Also triggers Action::raycast(raycaster) on offbeats in the second half: beats 4.5,5.5,6.5,7.5.
    let anim_b = universe
        .world
        .register(engine::ecs::component::AnimationComponent::new());

    let loop_beats = 8.0;
    let steps = 64;

    for step in 0..=steps {
        let t = (step as f32) / (steps as f32);
        let beat = (t as f64) * (loop_beats as f64);

        // Keyframe beats are per-animation local beats.
        let kf_a = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(beat));
        let kf_b = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(beat));

        // Anchor A rotation: smooth yaw.
        let yaw_a = t * std::f32::consts::TAU;
        let a_set = engine::ecs::component::ActionComponent::new(
            engine::ecs::component::Action::set_transform(
                vec![anchor_a],
                [0.0, 1.0, 0.0],
                quat_from_yaw(yaw_a),
                [1.0, 1.0, 1.0],
            ),
        );
        let a_set_id = universe.world.register(a_set);
        let _ = universe.attach(kf_a, a_set_id);

        // Anchor B rotation: yaw + pitch (different pattern).
        let yaw_b = -t * std::f32::consts::TAU * 1.5;
        let pitch_b = (t * std::f32::consts::TAU).sin() * 0.35;
        let rot_b = quat_mul(quat_from_yaw(yaw_b), quat_from_pitch(pitch_b));
        let b_set = engine::ecs::component::ActionComponent::new(
            engine::ecs::component::Action::set_transform(
                vec![anchor_b],
                [0.0, 2.2, 0.0],
                rot_b,
                [1.0, 1.0, 1.0],
            ),
        );
        let b_set_id = universe.world.register(b_set);
        let _ = universe.attach(kf_b, b_set_id);

        // Per-ring raycast patterns.
        // A: downbeats in first half (0..4): step % 8 == 0 -> beats 0,1,2,3,4.
        if beat < 4.0 && step % 8 == 0 {
            let act = universe.world.register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::raycast(raycaster),
            ));
            let _ = universe.attach(kf_a, act);
        }

        // B: offbeats in second half (4..8): step % 8 == 4 -> beats 0.5,1.5,...,7.5.
        if beat >= 4.0 && step % 8 == 4 {
            let act = universe.world.register(engine::ecs::component::ActionComponent::new(
                engine::ecs::component::Action::raycast(raycaster),
            ));
            let _ = universe.attach(kf_b, act);
        }

        let _ = universe.attach(anim_a, kf_a);
        let _ = universe.attach(anim_b, kf_b);
    }

    universe.add(anim_a);
    universe.add(anim_b);

    // Process init-time registrations.
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
