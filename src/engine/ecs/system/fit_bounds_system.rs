use crate::engine::ecs::component::{
    FitBoundsComponent, FitBoundsMode, FitBoundsTarget, LayoutBoundsComponent, SerializeComponent,
    TransformComponent,
};
use crate::engine::ecs::system::bounds_system::{BoundsSystem, UniformFitTransform};
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::engine::graphics::RenderAssets;
use crate::engine::graphics::bounds::Aabb;

const OWNED_FIT_CONTENT_LABEL: &str = "__fit_bounds_content";

#[derive(Debug, Default)]
pub struct FitBoundsSystem;

impl FitBoundsSystem {
    pub fn tick(
        &mut self,
        world: &mut World,
        render_assets: &RenderAssets,
        emit: &mut dyn SignalEmitter,
    ) {
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

            let Some(host_transform_id) = fit_owner_transform(world, fit_node) else {
                continue;
            };

            let fit_transform_id =
                ensure_fit_content_transform(world, emit, fit_node, host_transform_id);
            let Some(target_bounds) = resolve_target_bounds(world, fit, host_transform_id) else {
                apply_fit_transform(world, fit_transform_id, None);
                continue;
            };

            let transform = BoundsSystem::calculate_subtree_local_bounds(
                world,
                render_assets,
                fit_transform_id,
            )
            .and_then(|aabb| {
                BoundsSystem::fit_aabb_uniform(&aabb, aabb_to_bounds_array(target_bounds))
            });

            apply_fit_transform(world, fit_transform_id, transform);
        }
    }
}

fn fit_owner_transform(world: &World, fit_node: ComponentId) -> Option<ComponentId> {
    let owner = world.parent_of(fit_node)?;
    world
        .get_component_by_id_as::<TransformComponent>(owner)
        .map(|_| owner)
}

fn resolve_target_bounds(
    world: &World,
    fit: FitBoundsComponent,
    host_transform_id: ComponentId,
) -> Option<Aabb> {
    match fit.target {
        FitBoundsTarget::ExplicitBounds => Some(Aabb {
            min: [
                fit.target_bounds[0],
                fit.target_bounds[1],
                fit.target_bounds[2],
            ],
            max: [
                fit.target_bounds[3],
                fit.target_bounds[4],
                fit.target_bounds[5],
            ],
        }),
        FitBoundsTarget::ParentPaddingBox => parent_padding_box(world, host_transform_id),
    }
}

fn parent_padding_box(world: &World, host_transform_id: ComponentId) -> Option<Aabb> {
    world
        .children_of(host_transform_id)
        .iter()
        .find_map(|&child| {
            world
                .get_component_by_id_as::<LayoutBoundsComponent>(child)
                .map(|bounds| bounds.padding_local)
        })
}

fn ensure_fit_content_transform(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    fit_node: ComponentId,
    host_transform_id: ComponentId,
) -> ComponentId {
    let fit_transform_id = world
        .children_of(fit_node)
        .iter()
        .copied()
        .find(|&child| {
            world.component_label(child) == Some(OWNED_FIT_CONTENT_LABEL)
                && world
                    .get_component_by_id_as::<TransformComponent>(child)
                    .is_some()
        })
        .unwrap_or_else(|| {
            let fit_transform_id = world.add_component_boxed_named(
                OWNED_FIT_CONTENT_LABEL,
                Box::new(TransformComponent::new()),
            );
            let serialize_id = world.add_component(SerializeComponent::off());
            let _ = world.add_child(fit_transform_id, serialize_id);
            let _ = world.add_child(fit_node, fit_transform_id);
            world.init_component_tree(fit_transform_id, emit);
            fit_transform_id
        });

    reparent_fit_body_children(world, fit_node, fit_transform_id);

    if fit_body_is_empty(world, fit_node, fit_transform_id) {
        reparent_legacy_host_children(world, host_transform_id, fit_node, fit_transform_id);
    }

    fit_transform_id
}

fn reparent_fit_body_children(
    world: &mut World,
    fit_node: ComponentId,
    fit_transform_id: ComponentId,
) {
    let children: Vec<ComponentId> = world.children_of(fit_node).to_vec();
    for child in children {
        if child == fit_transform_id {
            continue;
        }
        let _ = world.add_child(fit_transform_id, child);
    }
}

fn fit_body_is_empty(world: &World, fit_node: ComponentId, fit_transform_id: ComponentId) -> bool {
    world
        .children_of(fit_node)
        .iter()
        .all(|&child| child == fit_transform_id)
        && world.children_of(fit_transform_id).iter().all(|&child| {
            world
                .get_component_by_id_as::<SerializeComponent>(child)
                .is_some()
        })
}

fn reparent_legacy_host_children(
    world: &mut World,
    host_transform_id: ComponentId,
    fit_node: ComponentId,
    fit_transform_id: ComponentId,
) {
    let children: Vec<ComponentId> = world.children_of(host_transform_id).to_vec();
    for child in children {
        if child == fit_node {
            continue;
        }
        if world
            .component_label(child)
            .is_some_and(|label| label.starts_with("__"))
        {
            continue;
        }
        if world
            .get_component_by_id_as::<TransformComponent>(child)
            .is_none()
        {
            continue;
        }
        if crate::engine::ecs::system::layout::measure::is_layout_item(world, child) {
            continue;
        }
        let _ = world.add_child(fit_transform_id, child);
    }
}

fn aabb_to_bounds_array(aabb: Aabb) -> [f32; 6] {
    [
        aabb.min[0],
        aabb.min[1],
        aabb.min[2],
        aabb.max[0],
        aabb.max[1],
        aabb.max[2],
    ]
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
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::{
        ColorComponent, Display, EdgeInsets, LayoutComponent, RenderableComponent, SizeDimension,
        StyleComponent,
    };
    use crate::engine::ecs::system::layout::LayoutSystem;

    #[test]
    fn renderable_only_fit_updates_owned_content_transform_not_parent() {
        let mut world = World::default();
        let mut render_assets = RenderAssets::new();
        let mut fit_system = FitBoundsSystem;
        let mut queue = CommandQueue::new();

        let fit_root = world.add_component(TransformComponent::new());
        let fit = world.add_component(FitBoundsComponent {
            mode: FitBoundsMode::RenderableOnly,
            target: FitBoundsTarget::ExplicitBounds,
            target_bounds: [-1.0, -1.0, -0.1, 1.0, 1.0, 0.1],
        });
        let icon_root = world.add_component(TransformComponent::new());
        let icon_shape = world.add_component(TransformComponent::new().with_scale(0.2, 1.0, 0.1));
        let icon_color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let icon_renderable = world.add_component(RenderableComponent::cube());

        world
            .add_child(fit_root, fit)
            .expect("attach fit component");
        world.add_child(fit, icon_root).expect("attach icon root");
        world
            .add_child(icon_root, icon_shape)
            .expect("attach icon shape");
        world
            .add_child(icon_shape, icon_color)
            .expect("attach icon color");
        world
            .add_child(icon_color, icon_renderable)
            .expect("attach icon renderable");

        world.init_component_tree(fit_root, &mut queue);
        fit_system.tick(&mut world, &mut render_assets, &mut queue);

        let fit_root = world
            .get_component_by_id_as::<TransformComponent>(fit_root)
            .expect("fit root transform");
        assert_eq!(fit_root.transform.translation, [0.0, 0.0, 0.0]);
        assert_eq!(fit_root.transform.scale, [1.0, 1.0, 1.0]);

        let owned_fit = world
            .children_of(fit)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some(OWNED_FIT_CONTENT_LABEL))
            .expect("owned fit transform");
        let owned_fit = world
            .get_component_by_id_as::<TransformComponent>(owned_fit)
            .expect("owned fit transform component");
        assert!((owned_fit.transform.scale[0] - 2.0).abs() < 1e-4);
        assert!((owned_fit.transform.scale[1] - 2.0).abs() < 1e-4);
        assert!((owned_fit.transform.scale[2] - 2.0).abs() < 1e-4);
    }

    #[test]
    fn to_container_reads_parent_padding_box() {
        let mut world = World::default();
        let mut render_assets = RenderAssets::new();
        let mut fit_system = FitBoundsSystem;
        let mut layout_system = LayoutSystem;
        let mut queue = CommandQueue::new();

        let root = world.add_component(LayoutComponent::new(10.0));
        let item = world.add_component(TransformComponent::new());
        let style = world.add_component({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::GlyphUnits(4.0);
            style.height = SizeDimension::GlyphUnits(6.0);
            style.padding = EdgeInsets::axes(0.5, 1.0);
            style
        });
        let fit = world.add_component(FitBoundsComponent {
            mode: FitBoundsMode::RenderableOnly,
            target: FitBoundsTarget::ParentPaddingBox,
            target_bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
        });
        let icon_root = world.add_component(TransformComponent::new());
        let icon_shape = world.add_component(TransformComponent::new().with_scale(0.5, 1.0, 0.1));
        let icon_color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let icon_renderable = world.add_component(RenderableComponent::cube());

        let _ = world.add_child(root, item);
        let _ = world.add_child(item, style);
        let _ = world.add_child(item, fit);
        let _ = world.add_child(fit, icon_root);
        let _ = world.add_child(icon_root, icon_shape);
        let _ = world.add_child(icon_shape, icon_color);
        let _ = world.add_child(icon_color, icon_renderable);

        world.init_component_tree(root, &mut queue);
        layout_system.tick(&mut world, &mut queue);
        fit_system.tick(&mut world, &mut render_assets, &mut queue);

        let owned_fit = world
            .children_of(fit)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some(OWNED_FIT_CONTENT_LABEL))
            .expect("owned fit transform");
        let owned_fit = world
            .get_component_by_id_as::<TransformComponent>(owned_fit)
            .expect("owned fit transform component");
        assert!((owned_fit.transform.scale[0] - 6.0).abs() < 1e-4);
        assert!((owned_fit.transform.scale[1] - 6.0).abs() < 1e-4);
    }
}
