use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Light pink background.
    let clear = universe
        .world
        .register(engine::ecs::component::BackgroundColorComponent::rgba(
            1.0, 0.86, 0.92, 1.0,
        ));
    universe.add(clear);

    // Camera rig:
    // I { InputTransformMode(with_roll_axis_y)  T { C3D } }
    let input = universe
        .world
        .register(engine::ecs::component::InputComponent::new().with_speed(2.0));
    let input_mode = universe.world.register(
        engine::ecs::component::InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    let rig_transform = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, 10.0));
    let _ = universe.attach(input, rig_transform);

    let camera3d = universe
        .world
        .register(engine::ecs::component::Camera3DComponent::new().with_far(250.0).with_fov(70.0));
    let _ = universe.attach(rig_transform, camera3d);
    universe.add(input);

    // Directional light facing slightly down and forward (+Z).
    let sun = universe.world.register(
        engine::ecs::component::DirectionalLightComponent::new()
            .with_intensity(1.4)
            .with_color(1.0, 0.98, 0.94),
    );
    let sun_dir = universe
        .world
        .register(engine::ecs::component::TransformComponent::new().with_position(0.0, -0.35, 1.0));
    let _ = universe.attach(sun_dir, sun);
    universe.add(sun_dir);

    // ambient pink light
    let ambient = universe
        .world
        .register(engine::ecs::component::AmbientLightComponent::rgb(0.12, 0.08, 0.10));
    universe.add(ambient);

    // Background stage: occlusion + lighting, with 5 cloud clusters around the perimeter.
    let bg_root = universe.world.register(
        engine::ecs::component::BackgroundComponent::new().with_occlusion_and_lighting(),
    );
    universe.add(bg_root);

    example_util::spawn_cloud_ring(
        &mut universe,
        bg_root,
        example_util::CloudRingParams::default(),
    );

    // ClockComponent sets global tempo.
    let clock = universe
        .world
        .register(engine::ecs::component::ClockComponent::new().with_bpm(160.0));
    universe.add(clock);

    // Beat-timed animation that prints when keyframes fire.
    let anim = universe
        .world
        .register(engine::ecs::component::AnimationComponent::new());

    // Second animation: 4-on-the-floor pattern (beats 0..3) for another set of cubes.
    // let anim_floor = universe
    //     .world
    //     .register(engine::ecs::component::AnimationComponent::new());

    // 5 cubes, one per beat.
    let mut beat_cubes: Vec<engine::ecs::ComponentId> = Vec::new();
    for i in 0..5 {
        let x = (i as f32) * 1.5 - 3.0;
        let root = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, 0.25, 0.0)
                .with_scale(0.9, 0.9, 0.9),
        );
        let renderable = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let color = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(0.0, 0.0, 0.0, 1.0));

        let _ = universe.attach(root, renderable);
        let _ = universe.attach(renderable, color);
        universe.add(root);

        beat_cubes.push(root);
    }

    // // 4 cubes on the floor, one per beat (0..3).
    // let mut floor_cubes: Vec<engine::ecs::ComponentId> = Vec::new();
    // for i in 0..4 {
    //     let x = (i as f32) * 1.5 - 2.25;
    //     let root = universe.world.register(
    //         engine::ecs::component::TransformComponent::new()
    //             .with_position(x, -0.75, -2.5)
    //             .with_scale(0.9, 0.25, 0.9),
    //     );
    //     let renderable = universe
    //         .world
    //         .register(engine::ecs::component::RenderableComponent::cube());
    //     let color = universe
    //         .world
    //         .register(engine::ecs::component::ColorComponent::rgba(0.0, 0.0, 0.0, 1.0));

    //     let _ = universe.attach(root, renderable);
    //     let _ = universe.attach(renderable, color);
    //     universe.add(root);

    //     floor_cubes.push(root);
    // }

    // 4 more cubes, controlled by the first animation (beats 0,1,2 + a special beat 3.0).
    let mut anim2_cubes: Vec<engine::ecs::ComponentId> = Vec::new();
    for i in 0..4 {
        let x = (i as f32) * 1.5 - 2.25;
        let root = universe.world.register(
            engine::ecs::component::TransformComponent::new()
                .with_position(x, 0.25, -1.75)
                .with_scale(0.75, 0.75, 0.75),
        );
        let renderable = universe
            .world
            .register(engine::ecs::component::RenderableComponent::cube());
        let color = universe
            .world
            .register(engine::ecs::component::ColorComponent::rgba(0.0, 0.0, 0.0, 1.0));

        let _ = universe.attach(root, renderable);
        let _ = universe.attach(renderable, color);
        universe.add(root);

        anim2_cubes.push(root);
    }

    let beats = [0.0_f64, 1.0, 2.0, 2.75, 3.5];
    for (i, b) in beats.iter().copied().enumerate() {
        let kf = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(b));

        // Highlight current cube.
        // For beats 0,1,2 also light the corresponding cube from the new 4-cube set.
        let mut hi_targets = vec![beat_cubes[i]];
        if i < 3 {
            hi_targets.push(anim2_cubes[i]);
        }
        let action_hi = universe.world.register(engine::ecs::component::ActionComponent::new(
            engine::ecs::component::Action::set_color(hi_targets, [1.0, 1.0, 1.0, 1.0]),
        ));

        // Darken the other cubes.
        // For beats 0,1,2 also darken the other cubes in the new 4-cube set.
        let mut others: Vec<engine::ecs::ComponentId> = Vec::new();
        for (j, &cid) in beat_cubes.iter().enumerate() {
            if j != i {
                others.push(cid);
            }
        }
        if i < 3 {
            for (j, &cid) in anim2_cubes.iter().enumerate() {
                if j != i {
                    others.push(cid);
                }
            }
        }
        let action_dim = universe.world.register(engine::ecs::component::ActionComponent::new(
            engine::ecs::component::Action::set_color(others, [0.0, 0.0, 0.0, 1.0]),
        ));

        // Also print for debugging.
        let action_print = universe
            .world
            .register(engine::ecs::component::ActionComponent::print(format!(
                "keyframe fired (beat={})",
                b
            )));

        let _ = universe.attach(anim, kf);
        let _ = universe.attach(kf, action_hi);
        let _ = universe.attach(kf, action_dim);
        let _ = universe.attach(kf, action_print);
    }

    // Special keyframe at beat 3.0: light the 4th cube of the new set.
    // Keep this isolated to the new set (don't affect the original 5 cubes).
    {
        let b = 3.0_f64;
        let kf = universe
            .world
            .register(engine::ecs::component::KeyframeComponent::new(b));

        let action_hi = universe.world.register(engine::ecs::component::ActionComponent::new(
            engine::ecs::component::Action::set_color(vec![anim2_cubes[3]], [1.0, 1.0, 1.0, 1.0]),
        ));

        let action_dim = universe.world.register(engine::ecs::component::ActionComponent::new(
            engine::ecs::component::Action::set_color(
                vec![anim2_cubes[0], anim2_cubes[1], anim2_cubes[2]],
                [0.0, 0.0, 0.0, 1.0],
            ),
        ));


        let _ = universe.attach(anim, kf);
        let _ = universe.attach(kf, action_hi);
        let _ = universe.attach(kf, action_dim);
    }

    universe.add(anim);

    // 4-on-the-floor: beats 0,1,2,3 repeating.
    // let floor_beats = [0.0_f64, 1.0, 2.0, 3.0];
    // for (i, b) in floor_beats.iter().copied().enumerate() {
    //     let kf = universe
    //         .world
    //         .register(engine::ecs::component::KeyframeComponent::new(b));

    //     let action_hi = universe.world.register(engine::ecs::component::ActionComponent::new(
    //         engine::ecs::component::Action::set_color(vec![floor_cubes[i]], [1.0, 1.0, 1.0, 1.0]),
    //     ));

    //     let mut others = Vec::new();
    //     for (j, &cid) in floor_cubes.iter().enumerate() {
    //         if j != i {
    //             others.push(cid);
    //         }
    //     }
    //     let action_dim = universe.world.register(engine::ecs::component::ActionComponent::new(
    //         engine::ecs::component::Action::set_color(others, [0.0, 0.0, 0.0, 1.0]),
    //     ));

    //     let action_print = universe
    //         .world
    //         .register(engine::ecs::component::ActionComponent::print(format!(
    //             "floor keyframe fired (beat={})",
    //             b
    //         )));

    //     let _ = universe.attach(anim_floor, kf);
    //     let _ = universe.attach(kf, action_hi);
    //     let _ = universe.attach(kf, action_dim);
    //     let _ = universe.attach(kf, action_print);
    // }

    //universe.add(anim_floor);

    universe.enable_repl();

    let xr_root = universe
        .world
        .register(engine::ecs::component::OpenXRComponent::on());
    universe.add(xr_root);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}
