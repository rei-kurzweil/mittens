use crate::engine::ecs::component::{
    FitBoundsComponent, FitBoundsMode, TransformComponent, FIT_BOUNDS_CONTENT_NAME,
    FIT_BOUNDS_TRANSFORM_NAME,
};
use crate::engine::ecs::system::bounds_system::{
    BoundsSystem, RenderableBoundsMeasure, UniformFitTransform,
};
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::graphics::RenderAssets;

#[derive(Debug, Default)]
pub struct FitBoundsSystem;

impl FitBoundsSystem {
    pub fn tick(&mut self, world: &mut World, render_assets: &RenderAssets) {
        let fit_nodes: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<FitBoundsComponent>(id)
                    .is_some()
            })
            .collect();

        for fit_node in fit_nodes {
            let Some(fit) = world
                .get_component_by_id_as::<FitBoundsComponent>(fit_node)
                .copied()
            else {
                continue;
            };

            if fit.mode != FitBoundsMode::RenderableOnly {
                continue;
            }

            let Some(fit_transform_id) =
                find_named_direct_child(world, fit_node, FIT_BOUNDS_TRANSFORM_NAME)
            else {
                continue;
            };
            let Some(content_root_id) =
                find_named_direct_child(world, fit_transform_id, FIT_BOUNDS_CONTENT_NAME)
            else {
                continue;
            };

            let transform = match BoundsSystem::measure_renderable_subtree_bounds(
                world,
                render_assets,
                content_root_id,
            ) {
                RenderableBoundsMeasure::Measured(aabb) => {
                    BoundsSystem::fit_aabb_uniform(&aabb, fit.target_bounds)
                }
                RenderableBoundsMeasure::Unmeasurable => None,
            };

            apply_fit_transform(world, fit_transform_id, transform);
        }
    }
}

fn find_named_direct_child(world: &World, parent: ComponentId, name: &str) -> Option<ComponentId> {
    world.children_of(parent).iter().copied().find(|child| {
        world
            .get_component_record(*child)
            .map(|node| node.name == name)
            .unwrap_or(false)
    })
}

fn apply_fit_transform(
    world: &mut World,
    fit_transform_id: ComponentId,
    transform: Option<UniformFitTransform>,
) {
    let Some(fit_transform) =
        world.get_component_by_id_as_mut::<TransformComponent>(fit_transform_id)
    else {
        return;
    };

    let (translation, scale) = match transform {
        Some(transform) => (transform.translation, transform.scale),
        None => ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
    };

    if fit_transform.transform.translation == translation && fit_transform.transform.scale == scale
    {
        return;
    }

    fit_transform.transform.translation = translation;
    fit_transform.transform.scale = scale;
    fit_transform.transform.recompute_model();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{ColorComponent, RenderableComponent};

    #[test]
    fn renderable_only_fit_updates_internal_transform() {
        let mut world = World::default();
        let render_assets = RenderAssets::new();
        let mut fit_system = FitBoundsSystem;

        let fit_root = world.add_component(FitBoundsComponent {
            mode: FitBoundsMode::RenderableOnly,
            target_bounds: [-1.0, -1.0, -0.1, 1.0, 1.0, 0.1],
        });
        if let Some(node) = world.get_component_record_mut(fit_root) {
            node.name = "fit_root".to_string();
        }

        let fit_transform = world.add_component_boxed_named(
            FIT_BOUNDS_TRANSFORM_NAME,
            Box::new(TransformComponent::new()),
        );
        let content_root = world.add_component_boxed_named(
            FIT_BOUNDS_CONTENT_NAME,
            Box::new(TransformComponent::new()),
        );
        let icon_root = world.add_component(TransformComponent::new());
        let icon_shape = world.add_component(TransformComponent::new().with_scale(0.2, 1.0, 0.1));
        let icon_color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let icon_renderable = world.add_component(RenderableComponent::cube());

        world
            .add_child(fit_root, fit_transform)
            .expect("attach fit transform");
        world
            .add_child(fit_transform, content_root)
            .expect("attach content root");
        world
            .add_child(content_root, icon_root)
            .expect("attach icon root");
        world
            .add_child(icon_root, icon_shape)
            .expect("attach icon shape");
        world
            .add_child(icon_shape, icon_color)
            .expect("attach icon color");
        world
            .add_child(icon_color, icon_renderable)
            .expect("attach icon renderable");

        fit_system.tick(&mut world, &render_assets);

        let fit_transform = world
            .get_component_by_id_as::<TransformComponent>(fit_transform)
            .expect("fit transform");
        assert_eq!(fit_transform.transform.translation, [0.0, 0.0, 0.0]);
        assert!((fit_transform.transform.scale[0] - 2.0).abs() < 1e-4);
        assert!((fit_transform.transform.scale[1] - 2.0).abs() < 1e-4);
        assert!((fit_transform.transform.scale[2] - 2.0).abs() < 1e-4);
    }
}
