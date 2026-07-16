use mittens_engine::{engine, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn build_demo_scene_7_shapes(universe: &mut engine::Universe) {
    use engine::ecs::component::{
        Camera3DComponent, ColorComponent, EmissiveComponent, GLTFComponent, InputComponent,
        InputTransformModeComponent, PointLightComponent, RenderableComponent, TextureComponent,
        TransformComponent,
    };
    use engine::graphics::BuiltinMeshType;
    use engine::graphics::primitives::MaterialHandle;

    // Built-in CPU meshes are pre-registered; just fetch stable handles.
    let tri_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Triangle2D);
    let square_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Quad2D);
    let tetra_mesh = universe
        .render_assets
        .get_mesh(BuiltinMeshType::Tetrahedron);

    fn spawn(
        universe: &mut engine::Universe,
        mesh: engine::graphics::primitives::CpuMeshHandle,
        x: f32,
        y: f32,
        s: f32,
        r: f32,
        color: [f32; 4],
        input_driven: bool,
        emissive: bool,
    ) -> engine::ecs::ComponentId {
        let transform = universe.world.add_component(
            TransformComponent::new()
                .with_position(x, y, 0.0)
                .with_scale(s, s, 1.0)
                .with_rotation_euler(0.0, 0.0, r),
        );
        let renderable = universe.world.add_component(RenderableComponent::new(
            engine::graphics::primitives::Renderable::new(mesh, MaterialHandle::TOON_MESH),
        ));
        let color_c = universe.world.add_component(ColorComponent { rgba: color });

        if emissive {
            let emissive_c = universe.world.add_component(EmissiveComponent::on());
            let _ = universe.attach(renderable, emissive_c);
        }

        // Topology: (optional Input) -> Transform -> Renderable
        let _ = universe.attach(transform, renderable);
        let _ = universe.attach(renderable, color_c);

        if input_driven {
            let input = universe
                .world
                .add_component(InputComponent::new().with_speed(0.5));
            let _ = universe.attach(input, transform);
            universe.add(input);
        } else {
            universe.add(transform);
        }

        transform
    }

    fn spawn_3d(
        universe: &mut engine::Universe,
        mesh: engine::graphics::primitives::CpuMeshHandle,
        x: f32,
        y: f32,
        z: f32,
        s: f32,
        rx: f32,
        ry: f32,
        rz: f32,
        color: [f32; 4],
    ) -> engine::ecs::ComponentId {
        let transform = universe.world.add_component(
            TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(s, s, s)
                .with_rotation_euler(rx, ry, rz),
        );
        let renderable = universe.world.add_component(RenderableComponent::new(
            engine::graphics::primitives::Renderable::new(mesh, MaterialHandle::TOON_MESH),
        ));
        let color_c = universe.world.add_component(ColorComponent { rgba: color });

        let _ = universe.attach(transform, renderable);
        let _ = universe.attach(renderable, color_c);
        universe.add(transform);

        transform
    }

    // Spawn shapes.
    // One triangle is input-driven (WASD/QE). Build a small "rig" so both the triangle
    // and the camera can be driven by the same InputComponent.

    // Topology: Input -> (InputTransformMode) -> RigTransform -> (CameraTransform -> Camera3D), (TriRootTransform -> ...)
    let tri_input = universe
        .world
        .add_component(InputComponent::new().with_speed(0.5));
    let input_mode = universe
        .world
        .add_component(InputTransformModeComponent::forward_z().with_roll_axis_y());
    let _ = universe.attach(tri_input, input_mode);

    // Start pulled back so the demo meshes at z=0 are in view.
    // The camera will be attached directly under this transform, so there is no local
    // camera offset that would cause orbiting when yawing.
    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 2.5));
    let _ = universe.attach(tri_input, rig_transform);

    // Camera: attached directly to the rig transform.
    let camera3d = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(rig_transform, camera3d);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(universe, rig_transform);

    let tri_root_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.5, 0.50, 0.0));

    // Visual transform under the root; this is where we apply rotation/scale.
    let tri_visual_transform = universe.world.add_component(
        TransformComponent::new()
            .with_scale(0.30, 0.30, 1.0)
            .with_rotation_euler(0.0, 0.0, (2.0 * 3.14159 / 3.0) + 3.14159),
    );
    let tri_renderable = universe.world.add_component(RenderableComponent::new(
        engine::graphics::primitives::Renderable::new(tri_mesh, MaterialHandle::TOON_MESH),
    ));
    let tri_color = universe
        .world
        .add_component(ColorComponent::rgba(0.2, 1.0, 0.2, 1.0));

    let _ = universe.attach(rig_transform, tri_root_transform);
    let _ = universe.attach(tri_root_transform, tri_visual_transform);
    let _ = universe.attach(tri_visual_transform, tri_renderable);
    let _ = universe.attach(tri_renderable, tri_color);

    let tri_light = universe.world.add_component(
        PointLightComponent::new()
            .with_distance(10.0)
            .with_color(1.0, 1.0, 1.0),
    );

    let light_transform = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.5, 0.50, 1.0)
            .with_scale(0.1, 0.1, 0.1),
    );

    let _ = universe.attach(light_transform, tri_light);

    universe.add(tri_input);
    universe.add(light_transform);

    spawn(
        universe,
        square_mesh,
        -0.80,
        -0.30,
        0.25,
        0.0,
        [1.0, 0.2, 0.2, 1.0],
        false,
        true,
    );
    spawn(
        universe,
        square_mesh,
        -0.40,
        -0.30,
        0.25,
        0.0,
        [1.0, 0.6, 0.2, 1.0],
        false,
        true,
    );

    // 3D primitive: tetrahedron.
    spawn_3d(
        universe,
        tetra_mesh,
        0.55,
        -0.15,
        0.0,
        0.35,
        0.75,
        0.55,
        0.0,
        [0.2, 0.7, 1.0, 1.0],
    );
    spawn(
        universe,
        square_mesh,
        0.00,
        -0.30,
        0.25,
        0.0,
        [1.0, 1.0, 0.2, 1.0],
        false,
        true,
    );
    spawn(
        universe,
        square_mesh,
        0.40,
        -0.30,
        0.25,
        0.0,
        [0.2, 0.6, 1.0, 1.0],
        false,
        true,
    );
    spawn(
        universe,
        square_mesh,
        0.80,
        -0.30,
        0.25,
        0.0,
        [0.8, 0.2, 1.0, 1.0],
        false,
        true,
    );
    spawn(
        universe,
        tri_mesh,
        0.30,
        0.35,
        0.30,
        -3.14159,
        [1.0, 1.0, 1.0, 1.0],
        false,
        false,
    );

    // Textured square.
    let tex_transform = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.1, 0.0)
            .with_scale(0.45, 0.45, 1.0),
    );
    let tex_renderable = universe.world.add_component(RenderableComponent::new(
        engine::graphics::primitives::Renderable::new(square_mesh, MaterialHandle::TOON_MESH),
    ));
    let tex_color = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let tex = universe.world.add_component(TextureComponent::from_dds(
        "assets/textures/cat-face-amused.dds",
    ));

    let _ = universe.attach(tex_transform, tex_renderable);
    let _ = universe.attach(tex_renderable, tex_color);
    let _ = universe.attach(tex_renderable, tex);
    universe.add(tex_transform);

    // glTF: color-cat
    // Attach GLTFComponent under a Transform so GLTFSystem can use it as an anchor.
    let cat_anchor = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, -0.10, -4.0)
            .with_scale(0.50, 0.50, 0.50)
            .with_rotation_euler(0.0, 0.0, 0.0),
    );
    let cat_gltf = universe
        .world
        .add_component(GLTFComponent::new("assets/models/color-cat.2.glb"));
    let _ = universe.attach(cat_anchor, cat_gltf);
    universe.add(cat_anchor);
}

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    build_demo_scene_7_shapes(&mut universe);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );
    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
