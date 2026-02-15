use cat_engine::engine::ecs::component::{
    Action, ActionComponent, AmbientLightComponent, AnimationComponent, AnimationState,
    BackgroundColorComponent, Camera3DComponent, ColorComponent, DirectionalLightComponent,
    EmissiveComponent, GLTFComponent, InputComponent, InputTransformModeComponent, JointComponent,
    KeyframeComponent, RenderableComponent, TransformComponent,
};
use cat_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Light pink background.
    let background = universe
        .world
        .register(BackgroundColorComponent::rgba(1.0, 0.82, 0.90, 1.0));
    universe.add(background);

    // Small ambient so shadowed areas aren't pure black.
    let ambient = universe
        .world
        .register(AmbientLightComponent::rgb(0.10, 0.10, 0.12));
    universe.add(ambient);

    // --- Camera rig (WASD + mouse) ---
    let input = universe
        .world
        .register(InputComponent::new().with_speed(1.5));
    let input_mode = universe.world.register(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    // Start slightly pulled back looking towards the origin.
    let rig_transform = universe
        .world
        .register(TransformComponent::new().with_position(0.0, 0.0, 6.0));
    let _ = universe.attach(input, rig_transform);

    let camera3d = universe.world.register(Camera3DComponent::new());
    let _ = universe.attach(rig_transform, camera3d);

    universe.add(input);

    // --- lighting ---
    let sun = universe.world.register(
        DirectionalLightComponent::new()
            .with_intensity(1.2)
            .with_color(1.0, 0.98, 0.94),
    );
    let sun_dir = universe
        .world
        .register(TransformComponent::new().with_position(0.0, -0.35, 1.0));
    let _ = universe.attach(sun_dir, sun);
    universe.add(sun_dir);

    let light_transform = universe.world.register(
        TransformComponent::new()
            .with_position(1.0, 6.0, 3.0)
            .with_scale(0.1, 0.1, 0.1),
    );
    let light = universe.world.register(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(120.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.attach(light_transform, light);
    universe.add(light_transform);

    // --- VTuber model ---
    let model_uri = "assets/models/pc-rei.hoodie.glb";

    let model_root = universe.world.register(TransformComponent::new());
    let model = universe.world.register(GLTFComponent::new(model_uri));

    // emissive for pc-rei
    let emissive = universe.world.register(EmissiveComponent { enabled: true });
    let _ = universe.attach(model, emissive);

    let _ = universe.attach(model_root, model);

    // Initialize the model root so GLTFComponent gets registered.
    universe.add(model_root);

    // --- Background clouds (occluded + lit) ---
    let bg_root = universe
        .world
        .register(engine::ecs::component::BackgroundComponent::new().with_occlusion_and_lighting());
    universe.add(bg_root);
    let mut cloud_params = example_util::CloudRingParams::default();
    cloud_params.cloud_count = 8; // +3 clusters
    cloud_params.angle_jitter = 0.35;
    cloud_params.high_y_probability = 0.5;
    cloud_params.high_y_multiplier = 1.5;
    cloud_params.seed = 0x57_55_B0_01u32;
    example_util::spawn_cloud_ring(&mut universe, bg_root, cloud_params);

    // --- Simple environment ---
    let spawn_cube = |universe: &mut engine::Universe,
                      position: (f32, f32, f32),
                      scale: (f32, f32, f32),
                      color: (f32, f32, f32, f32)| {
        let transform = universe.world.register(
            TransformComponent::new()
                .with_position(position.0, position.1, position.2)
                .with_scale(scale.0, scale.1, scale.2),
        );
        let renderable = universe.world.register(RenderableComponent::cube());
        let color = universe
            .world
            .register(ColorComponent::rgba(color.0, color.1, color.2, color.3));

        let _ = universe.attach(transform, renderable);
        let _ = universe.attach(renderable, color);

        universe.add(transform);
    };

    // floor
    spawn_cube(
        &mut universe,
        (0.0, -0.05, 0.0),
        (10.0, 0.1, 10.0),
        (0.92, 0.92, 0.92, 1.0),
    );

    // back wall
    spawn_cube(
        &mut universe,
        (-3.0, 1.5, -5.0),
        (3.0, 3.0, 1.0),
        (0.95, 0.94, 0.96, 1.0),
    );

    // desk
    spawn_cube(
        &mut universe,
        (0.0, 0.35, 1.0),
        (1.0, 0.75, 0.5),
        (0.75, 0.70, 0.65, 1.0),
    );

    let xr_root = universe
        .world
        .register(engine::ecs::component::OpenXRComponent::on());
    universe.add(xr_root);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Spawn the glTF subtree once up-front so joint ComponentIds exist.
    {
        let systems = &mut universe.systems;
        systems.gltf.tick_with_queue(
            &mut universe.world,
            &mut universe.visuals,
            &mut systems.skinned_mesh,
            &mut universe.command_queue,
            0.0,
        );
    }
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // --- Joint printout + animation binding (in the example) ---
    let all_joints = collect_joint_transforms(&universe.world, model_root);
    println!("[vtuber-joints-example] joints found: {}", all_joints.len());
    for (i, (node_index, joint_tx)) in all_joints.iter().enumerate() {
        println!("  joint[{i:03}] node_index={node_index} transform={joint_tx:?}");
    }

    let joint_offset: usize = std::env::var("VTUBER_JOINT_OFFSET")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let wiggle_count: usize = 16;

    // Disabled by default to avoid log spam. Set to 1/true/on to enable.
    let print_transform_updates: bool = std::env::var("VTUBER_PRINT_TRANSFORMS")
        .ok()
        .map(|s| {
            let s = s.trim().to_ascii_lowercase();
            s == "1" || s == "true" || s == "on" || s == "yes"
        })
        .unwrap_or(false);

    let selected_joint_transforms: Vec<engine::ecs::ComponentId> =
        select_joint_range(&all_joints, joint_offset, wiggle_count);
    println!(
        "[vtuber-joints-example] wiggle range: offset={} count={} selected={}",
        joint_offset,
        wiggle_count,
        selected_joint_transforms.len()
    );

    // Create a looping animation with many keyframes, and attach actions to the selected joints.
    let anim = universe
        .world
        .register(AnimationComponent::new().with_state(AnimationState::Looping));
    let _ = universe.attach(model_root, anim);

    // Fill [0, 2) beats densely so it looks smooth at bpm=120 (2 beats = 1 second).
    // Important: keep max beat < 2.0 so AnimationSystem uses loop_len=2.
    let steps: usize = 32;
    let amplitude_rad: f32 = 0.20;
    for i in 0..steps {
        let beat = (i as f64) * (2.0 / (steps as f64));
        let kf = universe.world.register(KeyframeComponent::new(beat));
        let _ = universe.attach(anim, kf);

        let angle = ((std::f64::consts::PI * beat).sin() as f32) * amplitude_rad;
        let delta = quat_from_axis_angle([0.0, 1.0, 0.0], angle);

        for &joint_tx in selected_joint_transforms.iter() {
            let Some((translation, base_rotation, scale)) = universe
                .world
                .get_component_by_id_as::<TransformComponent>(joint_tx)
                .map(|t| {
                    (
                        t.transform.translation,
                        t.transform.rotation,
                        t.transform.scale,
                    )
                })
            else {
                continue;
            };

            let rotation = quat_mul(base_rotation, delta);

            if print_transform_updates {
                // Print at runtime (when the keyframe fires) which transforms are updated and to what.
                let msg = format!(
                    "[vtuber-joints-example] beat={beat:.3} set_transform target={joint_tx:?} rotation_quat_xyzw={:?}",
                    rotation
                );
                let print_action = Action::print(msg);
                let print_action_cid = universe.world.register(ActionComponent::new(print_action));
                let _ = universe.attach(kf, print_action_cid);
            }

            let action = Action::set_transform(vec![joint_tx], translation, rotation, scale);
            let action_cid = universe.world.register(ActionComponent::new(action));
            let _ = universe.attach(kf, action_cid);
        }
    }

    // Ensure Animation/Keyframes are registered.
    universe
        .world
        .init_component_tree(anim, &mut universe.command_queue);
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    let user_input = engine::user_input::UserInput::new();
    universe.enable_repl();
    engine::Windowing::run_app(universe, user_input).expect("Windowing failed");
}

fn collect_joint_transforms(
    world: &engine::ecs::World,
    root: engine::ecs::ComponentId,
) -> Vec<(usize, engine::ecs::ComponentId)> {
    let mut joints: Vec<(usize, engine::ecs::ComponentId)> = Vec::new();

    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(j) = world.get_component_by_id_as::<JointComponent>(node) {
            if let Some(parent_tx) = world.parent_of(node) {
                if world
                    .get_component_by_id_as::<TransformComponent>(parent_tx)
                    .is_some()
                {
                    joints.push((j.node_index, parent_tx));
                }
            }
        }
        for &ch in world.children_of(node) {
            stack.push(ch);
        }
    }

    joints.sort_by_key(|(idx, _)| *idx);
    joints
}

fn select_joint_range(
    joints: &[(usize, engine::ecs::ComponentId)],
    offset: usize,
    count: usize,
) -> Vec<engine::ecs::ComponentId> {
    if joints.is_empty() || count == 0 {
        return Vec::new();
    }
    let len = joints.len();
    let start = offset % len;
    let n = count.min(len);

    (0..n).map(|i| joints[(start + i) % len].1).collect()
}

fn quat_from_axis_angle(axis: [f32; 3], angle_rad: f32) -> [f32; 4] {
    let [ax, ay, az] = axis;
    let len2 = ax * ax + ay * ay + az * az;
    if len2 <= 0.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv_len = len2.sqrt().recip();
    let (ax, ay, az) = (ax * inv_len, ay * inv_len, az * inv_len);

    let half = 0.5 * angle_rad;
    let (s, c) = half.sin_cos();
    [ax * s, ay * s, az * s, c]
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}
