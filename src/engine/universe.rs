use crate::engine::ecs::component::{
    Camera3DComponent, ColorComponent, InputComponent, PointLightComponent, RenderableComponent,
    TextureComponent, TransformComponent,
};
use crate::engine::graphics::BuiltinMeshType;
use crate::engine::graphics::primitives::MaterialHandle;
use crate::engine::user_input::InputState;
use crate::engine::{ecs, graphics};
use std::sync::Arc;
use winit::window::Window;

pub struct Universe {
    pub world: ecs::World,
    pub command_queue: ecs::CommandQueue,
    pub systems: ecs::SystemWorld,

    pub visuals: graphics::VisualWorld,
    pub render_assets: graphics::RenderAssets,

    repl: Option<crate::engine::repl::Repl>,
    repl_backend: Option<crate::engine::repl::ReplBackend>,

    renderer: graphics::VulkanoRenderer,
}

impl Universe {
    pub fn new(world: ecs::World) -> Self {
        Self {
            world,
            command_queue: ecs::CommandQueue::new(),
            systems: ecs::SystemWorld::new(),

            visuals: graphics::VisualWorld::new(),
            render_assets: graphics::RenderAssets::new(),
            renderer: graphics::VulkanoRenderer::new(),

            repl: None,
            repl_backend: None,
        }
    }

    pub fn enable_repl(&mut self) {
        if self.repl.is_none() {
            self.repl = Some(crate::engine::repl::Repl::new());
            self.repl_backend = Some(crate::engine::repl::ReplBackend::new());
            println!("[REPL] Ready. Commands: ls, cd <name>, cd .., cd /, pwd, help");
        }
    }

    fn sync_repl(&mut self) {
        let (Some(repl), Some(backend)) = (&self.repl, self.repl_backend.as_mut()) else {
            return;
        };
        backend.exec_all(&self.world, repl.try_recv_all());
    }

    /// Initialize the renderer for a window.
    /// This must be called before rendering.
    pub fn init_renderer_for_window(
        &mut self,
        window: &Arc<Window>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let size = window.inner_size();
        self.visuals
            .set_viewport([size.width as f32, size.height as f32]);

        self.renderer.init_for_window(window)
    }

    /// Resize the renderer when the window is resized.
    pub fn resize_renderer(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.visuals
            .set_viewport([size.width as f32, size.height as f32]);
        self.renderer.resize(size);
    }

    /// Build the demo scene with 7 shapes and a textured square.
    /// This can be called from main.rs after Universe creation.
    pub fn build_demo_scene_7_shapes(&mut self) {
        // Built-in CPU meshes are pre-registered; just fetch stable handles.
        let tri_mesh = self.render_assets.get_mesh(BuiltinMeshType::Triangle2D);
        let square_mesh = self.render_assets.get_mesh(BuiltinMeshType::Quad2D);
        let tetra_mesh = self.render_assets.get_mesh(BuiltinMeshType::Tetrahedron);

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
                crate::engine::graphics::primitives::Renderable::new(
                    mesh,
                    MaterialHandle::TOON_MESH,
                ),
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
                crate::engine::graphics::primitives::Renderable::new(
                    mesh,
                    MaterialHandle::TOON_MESH,
                ),
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
        let tri_input = self
            .world
            .add_component(InputComponent::new().with_speed(0.5));
        let input_mode = self
            .world
            .add_component(
                crate::engine::ecs::component::InputTransformModeComponent::forward_z()
                    .with_roll_axis_y(),
            );
        let _ = self.world.add_child(tri_input, input_mode);
        // Start pulled back so the demo meshes at z=0 are in view.
        // The camera will be attached directly under this transform, so there is no local
        // camera offset that would cause orbiting when yawing.
        let rig_transform = self
            .world
            .add_component(TransformComponent::new().with_position(0.0, 0.0, 2.5));
        let _ = self.world.add_child(tri_input, rig_transform);

        // Camera: attached directly to the rig transform.
        let camera3d = self.world.add_component(Camera3DComponent::new());
        let _ = self.world.add_child(rig_transform, camera3d);

        let tri_root_transform = self
            .world
            .add_component(TransformComponent::new().with_position(0.5, 0.50, 0.0));

        // Visual transform under the root; this is where we apply rotation/scale.
        // Rotating by PI should visually flip the triangle while leaving its input-driven
        // movement (on the root transform) unchanged.
        let tri_visual_transform = self.world.add_component(
            TransformComponent::new()
                .with_scale(0.30, 0.30, 1.0)
                .with_rotation_euler(0.0, 0.0, (2.0 * 3.14159 / 3.0) + 3.14159),
        );
        let tri_renderable = self.world.add_component(RenderableComponent::new(
            crate::engine::graphics::primitives::Renderable::new(
                tri_mesh,
                MaterialHandle::TOON_MESH,
            ),
        ));
        let tri_color = self
            .world
            .add_component(ColorComponent::rgba(0.2, 1.0, 0.2, 1.0));

        let _ = self.world.add_child(rig_transform, tri_root_transform);
        let _ = self
            .world
            .add_child(tri_root_transform, tri_visual_transform);
        let _ = self.world.add_child(tri_visual_transform, tri_renderable);
        let _ = self.world.add_child(tri_renderable, tri_color);
        //let _ = self.world.add_child(tri_root_transform, tri_light);

        let tri_light = self.world.add_component(
            PointLightComponent::new()
                .with_distance(10.0)
                .with_color(1.0, 1.0, 1.0),
        );

        let light_transform = self
            .world
            .add_component(
                TransformComponent::new()
                    .with_position(0.5, 0.50, 1.0)
                    .with_scale(0.1, 0.1, 0.1),
            );

        let _ = self.world.add_child(light_transform, tri_light);

        self.world
            .init_component_tree(tri_input, &mut self.command_queue);

        self.world
            .init_component_tree(light_transform, &mut self.command_queue);

        spawn(
            &mut self.world,
            &mut self.command_queue,
            square_mesh,
            -0.80,
            -0.30,
            0.25,
            0.0,
            [1.0, 0.2, 0.2, 1.0],
            false,
        );
        spawn(
            &mut self.world,
            &mut self.command_queue,
            square_mesh,
            -0.40,
            -0.30,
            0.25,
            0.0,
            [1.0, 0.6, 0.2, 1.0],
            false,
        );

        // 3D primitive: tetrahedron.
        // Rotated in X/Y so you can tell it's not a flat 2D mesh.
        spawn_3d(
            &mut self.world,
            &mut self.command_queue,
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
            &mut self.world,
            &mut self.command_queue,
            square_mesh,
            0.00,
            -0.30,
            0.25,
            0.0,
            [1.0, 1.0, 0.2, 1.0],
            false,
        );
        spawn(
            &mut self.world,
            &mut self.command_queue,
            square_mesh,
            0.40,
            -0.30,
            0.25,
            0.0,
            [0.2, 0.6, 1.0, 1.0],
            false,
        );
        spawn(
            &mut self.world,
            &mut self.command_queue,
            square_mesh,
            0.80,
            -0.30,
            0.25,
            0.0,
            [0.8, 0.2, 1.0, 1.0],
            false,
        );
        spawn(
            &mut self.world,
            &mut self.command_queue,
            tri_mesh,
            0.30,
            0.35,
            0.30,
            -3.14159,
            [1.0, 1.0, 1.0, 1.0],
            false,
        );

        // Textured square.
        let tex_transform = self.world.add_component(
            TransformComponent::new()
                .with_position(0.0, 0.10, 0.0)
                .with_scale(0.45, 0.45, 1.0),
        );
        let tex_renderable = self.world.add_component(RenderableComponent::new(
            crate::engine::graphics::primitives::Renderable::new(
                square_mesh,
                MaterialHandle::TOON_MESH,
            ),
        ));
        let tex_color = self
            .world
            .add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let tex = self
            .world
            .add_component(TextureComponent::from_dds("assets/textures/cat-face-amused.dds"));

        let _ = self.world.add_child(tex_transform, tex_renderable);
        let _ = self.world.add_child(tex_renderable, tex_color);
        let _ = self.world.add_child(tex_renderable, tex);
        self.world
            .init_component_tree(tex_transform, &mut self.command_queue);

        // NOTE: This demo spawns a Camera3D under the input-driven rig.
    }

    /// Game/update step
    pub fn update(&mut self, dt_sec: f32, input: &InputState) {
        self.sync_repl();

        // 1. Process input events (handled inside systems for now).
        // 2. Let systems call methods on components,
        //      for example, to update transforms or renderables, which
        //      will update VisualWorld can update draw_batches and give Renderer a snapshot
        self.systems.tick(
            &mut self.world,
            &mut self.visuals,
            input,
            &mut self.command_queue,
            dt_sec,
        );

        // Process commands after tick so any commands queued during tick are processed in the same frame
        self.systems
            .process_commands(&mut self.world, &mut self.visuals, &mut self.command_queue);
    }

    pub fn render(&mut self) {
        // Prepare render (mesh uploads) - cast renderer to trait
        self.systems.prepare_render(
            &mut self.world,
            &mut self.visuals,
            &mut self.render_assets,
            &mut self.renderer as &mut dyn graphics::RenderUploader,
        );

        // TODO: rebuild inspector around component graph instead of entities.

        self.renderer
            .render_visual_world(&mut self.visuals)
            .expect("render failed");
    }
}
