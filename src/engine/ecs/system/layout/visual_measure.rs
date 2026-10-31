//! Intrinsic visual measurement at nested layout ownership boundaries.
use super::measure::is_layout_item;
use crate::engine::ecs::component::{LayoutComponent, TransformComponent};
use crate::engine::ecs::system::bounds_system::BoundsSystem;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::bounds::Aabb;
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VisualContentMeasurement {
    pub bounds_parent_local: Aabb,
    pub content_root: ComponentId,
}

/// Measure eligible renderables in `root` local space. Plain transforms are
/// presentational and contribute their transformed cached bounds; nested styled
/// transforms and nested layout roots own independent layout boxes and are
/// therefore excluded.
pub(crate) fn find_visual_intrinsic_bounds(world: &World, root: ComponentId) -> Option<Aabb> {
    BoundsSystem::measure_cached_renderable_subtree_bounds(world, root, |node| {
        world
            .component_label(node)
            .is_some_and(|label| label.starts_with("__"))
            || world
                .get_component_by_id_as::<LayoutComponent>(node)
                .is_some()
            || (world
                .get_component_by_id_as::<TransformComponent>(node)
                .is_some()
                && is_layout_item(world, node))
    })
}

/// Return the complete pre-placement visual bounds and their one unambiguous
/// direct authored transform root. V1 deliberately declines multiple direct
/// visual branches; a future version can place a measured union of roots.
pub(crate) fn find_visual_content_measurement(
    world: &World,
    root: ComponentId,
) -> Option<VisualContentMeasurement> {
    let candidates: Vec<ComponentId> = world
        .children_of(root)
        .iter()
        .copied()
        .filter(|&child| {
            world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
                && !is_layout_item(world, child)
                // A panel/widget with a private LayoutRoot owns its placement.
                // Do not align its entire movable frame to an incidental
                // icon/decoration that happens to be measurable outside that root.
                && !world.children_of(child).iter().any(|&node|world.get_component_by_id_as::<LayoutComponent>(node).is_some())
                && !world
                    .component_label(child)
                    .is_some_and(|label| label.starts_with("__"))
                && find_visual_intrinsic_bounds(world, child).is_some()
        })
        .collect();
    let [content_root] = candidates.as_slice() else {
        return None;
    };
    let authored_local = world
        .get_component_by_id_as::<TransformComponent>(*content_root)?
        .transform
        .model;
    let bounds_parent_local =
        find_visual_intrinsic_bounds(world, *content_root)?.transformed(authored_local);
    Some(VisualContentMeasurement {
        bounds_parent_local,
        content_root: *content_root,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{BoundsComponent, RenderableComponent};
    #[test]
    fn private_layout_frame_is_not_placed_using_an_incidental_icon() {
        let mut world = World::default();
        let slot = world.add_component(TransformComponent::new());
        let panel = world.add_component(TransformComponent::new());
        world.add_child(slot, panel).unwrap();
        let layout = world.add_component(LayoutComponent::new(16.));
        world.add_child(panel, layout).unwrap();
        let icon = world.add_component(RenderableComponent::cube());
        world.add_child(panel, icon).unwrap();
        let bounds = world.add_component(BoundsComponent::new(Aabb {
            min: [0.1, -0.2, 0.],
            max: [0.2, -0.1, 0.01],
        }));
        world.add_child(icon, bounds).unwrap();
        assert!(find_visual_intrinsic_bounds(&world, panel).is_some());
        assert_eq!(find_visual_content_measurement(&world, slot), None);
    }
}
