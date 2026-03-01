use cat_engine::{
    engine::{self, ecs::component::AmbientLightComponent},
    utils,
};

#[path = "example_util/mod.rs"]
mod example_util;

struct Scene {
    bg_root: engine::ecs::ComponentId,
}

fn build_gestures_and_gizmos_scene(universe: &mut engine::Universe) -> Scene {
    use engine::ecs::component::{
        BackgroundColorComponent, BackgroundComponent, Camera3DComponent, ColorComponent,
        DirectionalLightComponent, GizmoComponent, InputComponent, InputTransformModeComponent,
        RayCastComponent, RaycastableComponent, RenderableComponent, TransformComponent,
    };
    use engine::graphics::BuiltinMeshType;
    use engine::graphics::primitives::{MaterialHandle, Renderable};

    let tri_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Triangle2D);
    let cube_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Cube);
    let tetra_mesh = universe
        .render_assets
        .get_mesh(BuiltinMeshType::Tetrahedron);

    // BackgroundColor { dark grey }
    let bg_color = universe
        .world
        .add_component(BackgroundColorComponent::rgba(0.90, 0.90, 0.90, 1.0));
    universe.add(bg_color);

    // ambient light
    let ambient = universe
        .world
        .register(AmbientLightComponent::rgb(0.25, 0.25, 0.25));
    universe.add(ambient);

    // ground plane
    let ground_tx = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, -2.5, 0.0)
            .with_scale(20.0, 1.0, 20.0),
    );
    let ground_r = universe
        .world
        .add_component(RenderableComponent::new(Renderable::new(
            universe.render_assets.get_mesh(BuiltinMeshType::Cube),
            MaterialHandle::TOON_MESH,
        )));
    let ground_c = universe
        .world
        .add_component(ColorComponent::rgba(0.75, 0.75, 0.75, 1.0));
    let _ = universe.attach(ground_tx, ground_r);
    let _ = universe.attach(ground_r, ground_c);
    let _ = universe.add(ground_tx);

    // Background {
    //     with_occlusion_and_lighting()
    //     // using the example utils to add clouds to the background
    // }
    let bg_root = universe
        .world
        .add_component(BackgroundComponent::new().with_occlusion_and_lighting());
    universe.add(bg_root);

    // DirectionalLight {
    //     T { translate [1, 1, 1] }
    // }
    // Directional lights encode their direction in the node's world position.
    let sun_t = universe
        .world
        .add_component(TransformComponent::new().with_position(1.0, 1.0, 1.0));
    let sun = universe
        .world
        .add_component(DirectionalLightComponent::new());
    let _ = universe.attach(sun_t, sun);
    universe.add(sun_t);

    // i = input
    // t = transform
    // c3d = camera3d
    //
    // I {
    //     T {
    //         C3D { with_fps_rotation().with_roll_axis_y() }
    //     }
    // }
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(2.5));
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );

    let _ = universe.attach(input, input_mode);

    // Forward is -Z, so put the camera at +Z looking toward the origin.
    let rig_t = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 3.5));
    let _ = universe.attach(input, rig_t);

    let cam = universe
        .world
        .add_component(Camera3DComponent::new().with_far(600.0).with_fov(70.0));
    let _ = universe.attach(rig_t, cam);

    let raycaster = universe
        .world
        .add_component(RayCastComponent::event_driven().with_max_distance(100.0));
    let _ = universe.attach(rig_t, raycaster);

    fn spawn_shape_with_gizmo(
        universe: &mut engine::Universe,
        mesh: engine::graphics::primitives::CpuMeshHandle,
        pos: [f32; 3],
        scale: [f32; 3],
        rot_euler: [f32; 3],
        color: [f32; 4],
    ) {
        let t = universe.world.add_component(
            TransformComponent::new()
                .with_position(pos[0], pos[1], pos[2])
                .with_scale(scale[0], scale[1], scale[2])
                .with_rotation_euler(rot_euler[0], rot_euler[1], rot_euler[2]),
        );
        let r = universe
            .world
            .add_component(RenderableComponent::new(Renderable::new(
                mesh,
                MaterialHandle::TOON_MESH,
            )));
        let c = universe
            .world
            .add_component(ColorComponent::rgba(color[0], color[1], color[2], color[3]));
        let rc = universe
            .world
            .add_component(RaycastableComponent::enabled());
        let g = universe.world.add_component(GizmoComponent::new());

        let _ = universe.attach(t, r);
        let _ = universe.attach(r, c);
        let _ = universe.attach(r, rc);
        let _ = universe.attach(t, g);

        universe.add(t);
    }

    fn spawn_shape_raycastable_no_gizmo(
        universe: &mut engine::Universe,
        mesh: engine::graphics::primitives::CpuMeshHandle,
        pos: [f32; 3],
        scale: [f32; 3],
        rot_euler: [f32; 3],
        color: [f32; 4],
    ) {
        let t = universe.world.add_component(
            TransformComponent::new()
                .with_position(pos[0], pos[1], pos[2])
                .with_scale(scale[0], scale[1], scale[2])
                .with_rotation_euler(rot_euler[0], rot_euler[1], rot_euler[2]),
        );
        let r = universe
            .world
            .add_component(RenderableComponent::new(Renderable::new(
                mesh,
                MaterialHandle::TOON_MESH,
            )));
        let c = universe
            .world
            .add_component(ColorComponent::rgba(color[0], color[1], color[2], color[3]));
        let rc = universe
            .world
            .add_component(RaycastableComponent::enabled());

        let _ = universe.attach(t, r);
        let _ = universe.attach(r, c);
        let _ = universe.attach(r, rc);

        universe.add(t);
    }

    spawn_shape_with_gizmo(
        universe,
        tri_mesh,
        [-1.2, 0.0, 0.0],
        [0.65, 0.65, 0.65],
        [0.0, 0.0, 0.0],
        [0.2, 0.9, 0.25, 1.0],
    );
    spawn_shape_with_gizmo(
        universe,
        cube_mesh,
        [0.0, 0.0, 0.0],
        [0.55, 0.55, 0.55],
        [0.0, 0.0, 0.0],
        [0.95, 0.25, 0.2, 1.0],
    );
    spawn_shape_with_gizmo(
        universe,
        tetra_mesh,
        [1.2, 0.0, 0.0],
        [0.7, 0.7, 0.7],
        [0.0, 0.0, 0.0],
        [0.2, 0.55, 1.0, 1.0],
    );

    // Standalone tetrahedron: no gizmo, but explicitly raycastable.
    // This helps isolate tetra picking vs gizmo-handle interception.
    spawn_shape_raycastable_no_gizmo(
        universe,
        tetra_mesh,
        [2.6, -0.6, 0.0],
        [0.95, 0.95, 0.95],
        [0.0, 0.0, 0.0],
        [0.85, 0.85, 1.0, 1.0],
    );

    universe.add(input);

    Scene { bg_root }
}

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scene = build_gestures_and_gizmos_scene(&mut universe);

    // Background (occluded + lit) cloud dressing.
    // Using the example utils to add clouds to the background.
    let bg_cloud_root = universe.world.add_component(
        engine::ecs::component::TransformComponent::new().with_position(0.0, 0.0, -0.0),
    );
    let _ = universe.attach(scene.bg_root, bg_cloud_root);

    let mut cloud_params = example_util::CloudRingParams::default();
    cloud_params.cloud_count = 11;
    cloud_params.radius = 32.0;
    cloud_params.center_y = 4.0;
    cloud_params.puffs_per_cloud = 28;
    cloud_params.angle_jitter = 0.55;
    cloud_params.high_y_probability = 0.35;
    cloud_params.high_y_multiplier = 1.84;
    cloud_params.seed = 0xC10_6D27u32;
    example_util::spawn_cloud_ring(&mut universe, bg_cloud_root, cloud_params);

    // Flush registrations (spawns gizmo visuals, uploads meshes, builds BVH eligibility, etc.).
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
