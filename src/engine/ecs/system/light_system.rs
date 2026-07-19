use crate::engine::ecs::component::DirectionalLightComponent;
use crate::engine::ecs::component::PointLightComponent;
use crate::engine::ecs::component::SpotLightComponent;
use crate::engine::ecs::component::{AmbientLightComponent, ColorComponent};
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

    fn spot_direction(world: &World, component: ComponentId) -> [f32; 3] {
        let Some(model) = TransformSystem::world_model(world, component) else {
            return [0.0, 0.0, 1.0];
        };
        let z = [model[2][0], model[2][1], model[2][2]];
        let len = (z[0] * z[0] + z[1] * z[1] + z[2] * z[2]).sqrt();
        if len <= 1e-5 {
            [0.0, 0.0, 1.0]
        } else {
            [z[0] / len, z[1] / len, z[2] / len]
        }
    }

    fn visual_spot_light(
        world: &World,
        component: ComponentId,
        position_ws: [f32; 3],
        light: &SpotLightComponent,
    ) -> crate::engine::graphics::visual_world::VisualLight {
        crate::engine::graphics::visual_world::VisualLight {
            light_type: 3,
            position_ws,
            intensity: light.intensity,
            distance: light.distance,
            color: light.color,
            direction_ws: Self::spot_direction(world, component),
            angle: light.angle,
            penumbra: light.penumbra,
        }
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
            visuals.upsert_light(
                component,
                crate::engine::graphics::visual_world::VisualLight {
                    light_type: 1,
                    position_ws,
                    intensity: light.intensity,
                    distance: light.distance,
                    color: light.color,
                    ..Default::default()
                },
            );
            return;
        }

        if let Some(light) = world.get_component_by_id_as::<DirectionalLightComponent>(component) {
            let color = world
                .children_of(component)
                .iter()
                .find_map(|&ch| {
                    world
                        .get_component_by_id_as::<ColorComponent>(ch)
                        .map(|c| [c.rgba[0], c.rgba[1], c.rgba[2]])
                })
                .unwrap_or(light.color);
            // Direction is encoded in the node's world position.
            visuals.upsert_light(
                component,
                crate::engine::graphics::visual_world::VisualLight {
                    light_type: 2,
                    position_ws,
                    intensity: light.intensity,
                    distance: 0.0,
                    color,
                    ..Default::default()
                },
            );
            return;
        }

        if let Some(light) = world.get_component_by_id_as::<SpotLightComponent>(component) {
            visuals.upsert_light(
                component,
                Self::visual_spot_light(world, component, position_ws, light),
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

        // Color driven by an immediate child ColorComponent; falls back to ambient.rgb.
        let rgb = world
            .children_of(component)
            .iter()
            .find_map(|&ch| {
                world
                    .get_component_by_id_as::<ColorComponent>(ch)
                    .map(|c| [c.rgba[0], c.rgba[1], c.rgba[2]])
            })
            .unwrap_or(ambient.rgb);

        // Global state: last registered wins.
        visuals.set_ambient_light(rgb);
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
        let mut _visited_nodes = 0usize;
        let mut _updated_lights = 0usize;

        let mut stack = vec![component];
        while let Some(node) = stack.pop() {
            _visited_nodes += 1;
            for &child in world.children_of(node) {
                stack.push(child);

                let position_ws =
                    TransformSystem::world_position(world, child).unwrap_or([0.0, 0.0, 0.0]);

                if let Some(light) = world.get_component_by_id_as::<PointLightComponent>(child) {
                    _updated_lights += 1;
                    visuals.upsert_light(
                        child,
                        crate::engine::graphics::visual_world::VisualLight {
                            light_type: 1,
                            position_ws,
                            intensity: light.intensity,
                            distance: light.distance,
                            color: light.color,
                            ..Default::default()
                        },
                    );
                    continue;
                }

                if let Some(light) =
                    world.get_component_by_id_as::<DirectionalLightComponent>(child)
                {
                    _updated_lights += 1;
                    let color = world
                        .children_of(child)
                        .iter()
                        .find_map(|&ch| {
                            world
                                .get_component_by_id_as::<ColorComponent>(ch)
                                .map(|c| [c.rgba[0], c.rgba[1], c.rgba[2]])
                        })
                        .unwrap_or(light.color);
                    visuals.upsert_light(
                        child,
                        crate::engine::graphics::visual_world::VisualLight {
                            light_type: 2,
                            position_ws,
                            intensity: light.intensity,
                            distance: 0.0,
                            color,
                            ..Default::default()
                        },
                    );
                    continue;
                }

                if let Some(light) = world.get_component_by_id_as::<SpotLightComponent>(child) {
                    _updated_lights += 1;
                    visuals.upsert_light(
                        child,
                        Self::visual_spot_light(world, child, position_ws, light),
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
