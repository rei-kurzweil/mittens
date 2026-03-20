#[cfg(test)]
mod tests {
    use crate::engine::ecs::World;

    #[test]
    fn add_child_sets_parent_and_child_list() {
        let mut w = World::default();

        let p = w.add_component(crate::engine::ecs::component::TransformComponent::new());
        let c = w.add_component(crate::engine::ecs::component::TransformComponent::new());

        w.add_child(p, c).unwrap();

        assert_eq!(w.parent_of(c), Some(p));
        assert!(w.children_of(p).contains(&c));
    }

    #[test]
    fn set_parent_none_detaches() {
        let mut w = World::default();

        let p = w.add_component(crate::engine::ecs::component::TransformComponent::new());
        let c = w.add_component(crate::engine::ecs::component::TransformComponent::new());

        w.add_child(p, c).unwrap();
        w.set_parent(c, None).unwrap();

        assert_eq!(w.parent_of(c), None);
        assert!(!w.children_of(p).contains(&c));
    }

    #[test]
    fn prevent_cycles() {
        let mut w = World::default();

        let a = w.add_component(crate::engine::ecs::component::TransformComponent::new());
        let b = w.add_component(crate::engine::ecs::component::TransformComponent::new());

        w.add_child(a, b).unwrap();

        // Can't make ancestor a child of its descendant.
        assert!(w.add_child(b, a).is_err());
    }

    #[test]
    fn remove_leaf_requires_no_children() {
        let mut w = World::default();

        let p = w.add_component(crate::engine::ecs::component::TransformComponent::new());
        let c = w.add_component(crate::engine::ecs::component::TransformComponent::new());

        w.add_child(p, c).unwrap();

        // Parent isn't a leaf.
        assert!(w.remove_component_leaf(p).is_err());

        // Child is a leaf.
        w.remove_component_leaf(c).unwrap();
        assert_eq!(w.parent_of(c), None);
        assert!(!w.children_of(p).contains(&c));
    }

    #[test]
    fn remove_subtree_deletes_descendants() {
        let mut w = World::default();

        let root = w.add_component(crate::engine::ecs::component::TransformComponent::new());
        let child = w.add_component(crate::engine::ecs::component::TransformComponent::new());
        let grandchild =
            w.add_component(crate::engine::ecs::component::RenderableComponent::cube());

        w.add_child(root, child).unwrap();
        w.add_child(child, grandchild).unwrap();

        w.remove_component_subtree(root).unwrap();

        assert!(w.get_component_record(root).is_none());
        assert!(w.get_component_record(child).is_none());
        assert!(w.get_component_record(grandchild).is_none());
    }

    #[test]
    fn find_component_matches_exact_name_selector() {
        let mut w = World::default();

        let root = w.add_component_boxed_named(
            "avatar_root",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let lower_arm = w.add_component_boxed_named(
            "J_Bip_L_Lower_Arm",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let hand = w.add_component_boxed_named(
            "J_Bip_L_Hand",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );

        w.add_child(root, lower_arm).unwrap();
        w.add_child(lower_arm, hand).unwrap();

        let found = w.find_component(root, "[name='J_Bip_L_Hand']");
        assert_eq!(found, Some(hand));
    }

    #[test]
    fn find_all_components_returns_all_name_matches_in_dfs_order() {
        let mut w = World::default();

        let root = w.add_component_boxed_named(
            "root",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let left = w.add_component_boxed_named(
            "match",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let middle = w.add_component_boxed_named(
            "middle",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let right = w.add_component_boxed_named(
            "match",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );

        w.add_child(root, left).unwrap();
        w.add_child(root, middle).unwrap();
        w.add_child(middle, right).unwrap();

        let found = w.find_all_components(root, "[name='match']");
        assert_eq!(found, vec![left, right]);
    }
}
