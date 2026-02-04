use crate::engine::ecs::component::AmbientLightComponent;
use crate::engine::ecs::component::DirectionalLightComponent;
use crate::engine::ecs::component::PointLightComponent;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::user_input::InputState;

/// ECS lighting system.
///
/// Keeps `VisualWorld`'s point-light list in sync with ECS.
#[derive(Debug, Default)]
pub struct LightSystem;

impl LightSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn register_light(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let position_ws =
            TransformSystem::world_position(world, component).unwrap_or([0.0, 0.0, 0.0]);

        if let Some(light) = world.get_component_by_id_as::<PointLightComponent>(component) {
            visuals.upsert_point_light(
                component,
                crate::engine::graphics::visual_world::VisualPointLight {
                    light_type: 1,
                    position_ws,
                    intensity: light.intensity,
                    distance: light.distance,
                    color: light.color,
                },
            );
            return;
        }

        if let Some(light) = world.get_component_by_id_as::<DirectionalLightComponent>(component) {
            // Direction is encoded in the node's world position.
            visuals.upsert_point_light(
                component,
                crate::engine::graphics::visual_world::VisualPointLight {
                    light_type: 2,
                    position_ws,
                    intensity: light.intensity,
                    distance: 0.0,
                    color: light.color,
                },
            );
        }
    }

    pub fn register_ambient_light(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(ambient) = world.get_component_by_id_as::<AmbientLightComponent>(component) else {
            return;
        };

        // Global state: last registered wins.
        visuals.set_ambient_light(ambient.rgb);
    }

    /// Called when a TransformComponent changes.
    ///
    /// Updates all descendant point lights' positions in `VisualWorld`.
    pub fn transform_changed(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let mut visited_nodes = 0usize;
        let mut updated_lights = 0usize;

        let mut stack = vec![component];
        while let Some(node) = stack.pop() {
            visited_nodes += 1;
            for &child in world.children_of(node) {
                stack.push(child);

                let position_ws =
                    TransformSystem::world_position(world, child).unwrap_or([0.0, 0.0, 0.0]);

                if let Some(light) = world.get_component_by_id_as::<PointLightComponent>(child) {
                    updated_lights += 1;
                    visuals.upsert_point_light(
                        child,
                        crate::engine::graphics::visual_world::VisualPointLight {
                            light_type: 1,
                            position_ws,
                            intensity: light.intensity,
                            distance: light.distance,
                            color: light.color,
                        },
                    );
                    continue;
                }

                if let Some(light) =
                    world.get_component_by_id_as::<DirectionalLightComponent>(child)
                {
                    updated_lights += 1;
                    visuals.upsert_point_light(
                        child,
                        crate::engine::graphics::visual_world::VisualPointLight {
                            light_type: 2,
                            position_ws,
                            intensity: light.intensity,
                            distance: 0.0,
                            color: light.color,
                        },
                    );
                }
            }
        }
    }
}

impl System for LightSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // No-op for now.
    }
}
