use crate::engine;
use crate::engine::ecs;
use crate::engine::ecs::component::{
    Camera3DComponent, ColorComponent, GLTFComponent, InputComponent,
    InputTransformModeComponent, PointLightComponent, RenderableComponent, TextureComponent,
    TransformComponent,
};
use crate::engine::graphics::BuiltinMeshType;
use crate::engine::graphics::primitives::MaterialHandle;

pub fn build_demo_scene_7_shapes(universe: &mut engine::Universe) {
    // Built-in CPU meshes are pre-registered; just fetch stable handles.
    let tri_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Triangle2D);
    let square_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Quad2D);
    let tetra_mesh = universe.render_assets.get_mesh(BuiltinMeshType::Tetrahedron);

    fn spawn(
        world: &mut ecs::World,
        queue: &mut ecs::CommandQueue,
        mesh: crate::engine::graphics::primitives::CpuMeshHandle,
        x: f32,
        y: f32,
        s: f32,
        r: f32,
        color: [f32; 4],
        input_driven: bool,
    ) -> ecs::ComponentId {
        let transform = world.add_component(
            TransformComponent::new()
                .with_position(x, y, 0.0)
                .with_scale(s, s, 1.0)
                .with_rotation_euler(0.0, 0.0, r),
        );
        let renderable = world.add_component(RenderableComponent::new(
            crate::engine::graphics::primitives::Renderable::new(mesh, MaterialHandle::TOON_MESH),
        ));
        let color_c = world.add_component(ColorComponent { rgba: color });

        // Topology: (optional Input) -> Transform -> Renderable
        let _ = world.add_child(transform, renderable);
        let _ = world.add_child(renderable, color_c);

        if input_driven {
            let input = world.add_component(InputComponent::new().with_speed(0.5));
            let _ = world.add_child(input, transform);
            world.init_component_tree(input, queue);
        } else {
            world.init_component_tree(transform, queue);
        }

        transform
    }

    fn spawn_3d(
        world: &mut ecs::World,
        queue: &mut ecs::CommandQueue,
        mesh: crate::engine::graphics::primitives::CpuMeshHandle,
        x: f32,
        y: f32,
        z: f32,
        s: f32,
        rx: f32,
        ry: f32,
        rz: f32,
        color: [f32; 4],
    ) -> ecs::ComponentId {
        let transform = world.add_component(
            TransformComponent::new()
                .with_position(x, y, z)
                .with_scale(s, s, s)
                .with_rotation_euler(rx, ry, rz),
        );
        let renderable = world.add_component(RenderableComponent::new(
            crate::engine::graphics::primitives::Renderable::new(mesh, MaterialHandle::TOON_MESH),
        ));
        let color_c = world.add_component(ColorComponent { rgba: color });

        let _ = world.add_child(transform, renderable);
        let _ = world.add_child(renderable, color_c);
        world.init_component_tree(transform, queue);

        transform
    }

    // Spawn shapes.
    // One triangle is input-driven (WASD/QE). Build a small "rig" so both the triangle
    // and the camera can be driven by the same InputComponent.

    // Topology: Input -> (InputTransformMode) -> RigTransform -> (CameraTransform -> Camera3D), (TriRootTransform -> ...)
    let tri_input = universe
        .world
        .add_component(InputComponent::new().with_speed(0.5));
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z().with_roll_axis_y(),
    );
    let _ = universe.world.add_child(tri_input, input_mode);

    // Start pulled back so the demo meshes at z=0 are in view.
    // The camera will be attached directly under this transform, so there is no local
    // camera offset that would cause orbiting when yawing.
    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 2.5));
    let _ = universe.world.add_child(tri_input, rig_transform);

    // Camera: attached directly to the rig transform.
    let camera3d = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.world.add_child(rig_transform, camera3d);

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
        crate::engine::graphics::primitives::Renderable::new(tri_mesh, MaterialHandle::TOON_MESH),
    ));
    let tri_color = universe
        .world
        .add_component(ColorComponent::rgba(0.2, 1.0, 0.2, 1.0));

    let _ = universe.world.add_child(rig_transform, tri_root_transform);
    let _ = universe
        .world
        .add_child(tri_root_transform, tri_visual_transform);
    let _ = universe.world.add_child(tri_visual_transform, tri_renderable);
    let _ = universe.world.add_child(tri_renderable, tri_color);

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

    let _ = universe.world.add_child(light_transform, tri_light);

    universe
        .world
        .init_component_tree(tri_input, &mut universe.command_queue);

    universe
        .world
        .init_component_tree(light_transform, &mut universe.command_queue);

    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        -0.80,
        -0.30,
        0.25,
        0.0,
        [1.0, 0.2, 0.2, 1.0],
        false,
    );
    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        -0.40,
        -0.30,
        0.25,
        0.0,
        [1.0, 0.6, 0.2, 1.0],
        false,
    );

    // 3D primitive: tetrahedron.
    spawn_3d(
        &mut universe.world,
        &mut universe.command_queue,
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
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        0.00,
        -0.30,
        0.25,
        0.0,
        [1.0, 1.0, 0.2, 1.0],
        false,
    );
    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        0.40,
        -0.30,
        0.25,
        0.0,
        [0.2, 0.6, 1.0, 1.0],
        false,
    );
    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        square_mesh,
        0.80,
        -0.30,
        0.25,
        0.0,
        [0.8, 0.2, 1.0, 1.0],
        false,
    );
    spawn(
        &mut universe.world,
        &mut universe.command_queue,
        tri_mesh,
        0.30,
        0.35,
        0.30,
        -3.14159,
        [1.0, 1.0, 1.0, 1.0],
        false,
    );

    // Textured square.
    let tex_transform = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.10, 0.0)
            .with_scale(0.45, 0.45, 1.0),
    );
    let tex_renderable = universe.world.add_component(RenderableComponent::new(
        crate::engine::graphics::primitives::Renderable::new(square_mesh, MaterialHandle::TOON_MESH),
    ));
    let tex_color = universe
        .world
        .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
    let tex = universe
        .world
        .add_component(TextureComponent::from_dds("assets/textures/cat-face-amused.dds"));

    let _ = universe.world.add_child(tex_transform, tex_renderable);
    let _ = universe.world.add_child(tex_renderable, tex_color);
    let _ = universe.world.add_child(tex_renderable, tex);
    universe
        .world
        .init_component_tree(tex_transform, &mut universe.command_queue);

    // glTF: color-cat
    // Attach GLTFComponent under a Transform so GLTFSystem can use it as an anchor.
    let cat_anchor = universe.world.add_component(
        TransformComponent::new()
            .with_position(0.0, -0.10, 0.0)
            .with_scale(0.50, 0.50, 0.50),
    );
    let cat_gltf = universe
        .world
        .add_component(GLTFComponent::new("assets/models/color-cat.2.glb"));
    let _ = universe.world.add_child(cat_anchor, cat_gltf);
    universe
        .world
        .init_component_tree(cat_anchor, &mut universe.command_queue);
}
