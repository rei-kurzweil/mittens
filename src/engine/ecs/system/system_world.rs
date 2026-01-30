use super::World;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::system::CameraSystem;
use crate::engine::ecs::system::CollisionSystem;
use crate::engine::ecs::system::GLTFSystem;
use crate::engine::ecs::system::InputSystem;
use crate::engine::ecs::system::LightSystem;
use crate::engine::ecs::system::OpenXRSystem;
use crate::engine::ecs::system::RenderableSystem;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TextSystem;
use crate::engine::ecs::system::TextureSystem;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::graphics::{RenderAssets, RenderUploader, VisualWorld};
use crate::engine::user_input::InputState;

/// System world that holds and runs all registered systems.
#[derive(Debug, Default)]
pub struct SystemWorld {
    pub transform: TransformSystem,
    pub collision: CollisionSystem,
    pub renderable: RenderableSystem,

    pub gltf: GLTFSystem,

    pub openxr: OpenXRSystem,

    pub camera: CameraSystem,
    pub input: InputSystem,
    pub light: LightSystem,

    pub text: TextSystem,
    pub texture: TextureSystem,
}

impl SystemWorld {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a RenderableComponent instance with the RenderableSystem.
    pub fn register_renderable(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable
            .register_renderable(world, visuals, component);
    }

    /// Register a UVComponent and apply it to its ancestor RenderableComponent.
    pub fn register_uv(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable.register_uv(world, visuals, component);
    }

    /// Register a ColorComponent and apply it to its ancestor RenderableComponent.
    pub fn register_color(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable.register_color(world, visuals, component);
    }

    /// Register a BackgroundColorComponent and apply it to VisualWorld.
    pub fn register_background_color(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable
            .register_background_color(world, visuals, component);
    }

    /// Register an AmbientLightComponent and apply it to VisualWorld.
    pub fn register_ambient_light(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.light.register_ambient_light(world, visuals, component);
    }

    /// Register a TextureComponent and apply it to its ancestor RenderableComponent.
    pub fn register_texture(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.texture.register_texture(world, visuals, component);
    }

    /// Register a TextureFilteringComponent and apply it to its ancestor RenderableComponent.
    pub fn register_texture_filtering(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.texture
            .register_texture_filtering(world, visuals, component);
    }

    /// Register a TextComponent and expand it into per-glyph components.
    pub fn register_text(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
        queue: &mut crate::engine::ecs::CommandQueue,
    ) {
        let spawned = self.text.register_text(world, visuals, component);
        for g in spawned {
            world.init_component_tree(g.transform, queue);
        }
    }

    /// Register an EmissiveComponent and apply it to its ancestor RenderableComponent.
    pub fn register_emissive(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.renderable.register_emissive(world, visuals, component);
    }

    /// Register a CollisionComponent instance with the CollisionSystem.
    pub fn register_collision(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.collision.register_collision(world, visuals, component);
    }

    /// Remove a CollisionComponent instance from the CollisionSystem.
    pub fn remove_collision(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.collision.remove_collision(world, visuals, component);
    }

    /// Register an OpenXRComponent (initializes OpenXR runtime if enabled).
    pub fn register_openxr(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.openxr.register_openxr(world, visuals, component);
    }

    /// Register a PointLightComponent instance with the LightSystem.
    pub fn register_light(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.light.register_light(world, visuals, component);
    }

    /// Prepare render state before issuing a frame.
    ///
    /// This flushes any pending renderables by uploading meshes and inserting GPU-ready
    /// instances into `VisualWorld`.
    pub fn prepare_render(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut RenderAssets,
        uploader: &mut dyn RenderUploader,
    ) {
        // Ensure any imported assets are registered before renderables try to resolve meshes/textures.
        self.gltf
            .flush_imports(render_assets, &mut self.texture, uploader);

        self.renderable
            .flush_pending(world, visuals, render_assets, uploader);

        // Must run after renderables are flushed so instance handles exist.
        self.texture.flush_pending(world, visuals, uploader);
    }

    /// Called when a TransformComponent changes.
    pub fn transform_changed(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.transform.transform_changed(
            world,
            visuals,
            component,
            &mut self.camera,
            &mut self.light,
            &mut self.collision,
        );
    }

    /// Update a transform component's transform value and notify systems.
    pub fn update_transform(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
        transform: crate::engine::graphics::primitives::Transform,
    ) {
        // Update the transform in the component itself first
        if let Some(transform_comp) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(
                component,
            )
        {
            transform_comp.transform = transform;
        }
        self.transform_changed(world, visuals, component);
    }

    /// Remove/reset a transform component's transform value and notify systems.
    pub fn remove_transform(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        if let Some(transform_comp) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(
                component,
            )
        {
            transform_comp.transform = crate::engine::graphics::primitives::Transform::default();
        }
        self.transform_changed(world, visuals, component);
    }

    /// Register a camera component.
    pub fn register_camera(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let handle = self.camera.register_camera(world, visuals, component);
        // Store the handle in the component
        if let Some(camera_comp) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::Camera3DComponent>(
                component,
            )
        {
            camera_comp.handle = Some(handle);
        }
    }

    /// Register a Camera2D component.
    pub fn register_camera2d(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let handle = self.camera.register_camera2d(world, visuals, component);
        // Store the handle in the component
        if let Some(camera2d_comp) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::Camera2DComponent>(
                component,
            )
        {
            camera2d_comp.handle = Some(handle);
        }

        // Apply 2D camera view transform from the parent Transform immediately so the camera is correct
        // on the first frame after registration.
        if let Some(parent) = world.parent_of(component) {
            if world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
                .is_some()
            {
                self.camera
                    .update_camera_2d_from_parent_transform(&*world, visuals, component, parent);
            }
        }
    }

    /// Register an InputComponent.
    pub fn register_input(&mut self, component: ComponentId) {
        self.input.register_input(component);
    }

    /// Make a camera active by its component ID.
    pub fn make_active_camera(
        &mut self,
        _world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        // XR rig cameras.
        if let Some(camera_xr) = _world
            .get_component_by_id_as::<crate::engine::ecs::component::CameraXRComponent>(component)
        {
            match camera_xr.target {
                crate::engine::graphics::CameraTarget::Xr => {
                    self.camera.set_active_xr_camera(_world, visuals, component);
                }
                crate::engine::graphics::CameraTarget::Window => {
                    // If someone explicitly targets Window with a CameraXRComponent, ignore for now.
                    // (Window camera matrices are driven by Camera2D/Camera3D.)
                }
            }
            return;
        }

        // Try Camera3DComponent first
        if let Some(camera_comp) = _world
            .get_component_by_id_as::<crate::engine::ecs::component::Camera3DComponent>(component)
        {
            if let Some(handle) = camera_comp.handle {
                match camera_comp.target {
                    crate::engine::graphics::CameraTarget::Window => {
                        self.camera.set_active_window_camera(visuals, handle);
                    }
                    crate::engine::graphics::CameraTarget::Xr => {
                        // XR camera matrices are driven by OpenXR each frame.
                        // Keep this as a no-op for now.
                    }
                }
                return;
            }
        }
        // Try Camera2DComponent
        if let Some(camera2d_comp) = _world
            .get_component_by_id_as::<crate::engine::ecs::component::Camera2DComponent>(component)
        {
            if let Some(handle) = camera2d_comp.handle {
                match camera2d_comp.target {
                    crate::engine::graphics::CameraTarget::Window => {
                        self.camera.set_active_window_camera(visuals, handle);
                    }
                    crate::engine::graphics::CameraTarget::Xr => {
                        // XR camera matrices are driven by OpenXR each frame.
                        // Keep this as a no-op for now.
                    }
                }
            }
        }
    }

    // first, tick is called on all systems,
    // process_commands is called after, systems.tick(), to process the commands in the queue

    pub fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        input: &InputState,
        queue: &mut crate::engine::ecs::CommandQueue,
        dt_sec: f32,
    ) {
        // Process input first - it may queue commands
        self.input.process_input(world, input, queue, dt_sec);

        // Spawn any GLTF component trees. This may queue component registrations.
        self.gltf.tick_with_queue(world, queue, dt_sec);

        // Ensure transforms are propagated before any camera systems consume world matrices.
        self.transform.tick(world, visuals, input, dt_sec);

        // Collision runs before camera/OpenXR for now; it reads cached world transforms.
        self.collision.tick(world, visuals, input, dt_sec);

        // Update window camera + select active XR camera rig before OpenXR consumes it.
        self.camera.tick(world, visuals, input, dt_sec);
        // OpenXR consumes the latest rig transform + publishes per-eye cameras.
        self.openxr.tick(world, visuals, input, dt_sec);

        self.renderable.tick(world, visuals, input, dt_sec);

        self.text.tick(world, visuals, input, dt_sec);

        self.light.tick(world, visuals, input, dt_sec);
    }

    /// Process commands from the command queue.
    pub fn process_commands(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        commands: &mut crate::engine::ecs::CommandQueue,
    ) {
        commands.flush(world, self, visuals);
    }
}
