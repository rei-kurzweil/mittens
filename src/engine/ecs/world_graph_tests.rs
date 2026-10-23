#[cfg(test)]
mod tests {
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{
        ComponentRef, QueryRootMode, parse_scoped_query, resolve_component_ref,
    };

    #[test]
    fn shared_shading_edges_keep_identity_and_independent_revisions() {
        use crate::engine::ecs::component::{ShadingComponent, TransformComponent};
        let mut w = World::default();
        let a = w.add_component(TransformComponent::new());
        let b = w.add_component(TransformComponent::new());
        let c = w.add_component(TransformComponent::new());
        let source = w.add_component(ShadingComponent::new());
        w.add_child(a, source).unwrap();
        let original = w.attachment_between(a, source).unwrap();
        w.add_child(b, source).unwrap();
        w.add_child(c, source).unwrap();
        let b_edge = w.attachment_between(b, source).unwrap();
        let c_edge = w.attachment_between(c, source).unwrap();
        assert_eq!(w.parents_of(source).count(), 3);
        assert_eq!(
            w.parent_of(source),
            None,
            "ambiguous ancestry has no first-parent fallback"
        );
        assert_eq!(w.attachment_of(source), None);
        assert!(!w.world_roots().contains(&source));
        assert!(w.add_child(a, source).is_err());
        assert!(w.attachment_is_live(original));
        assert!(
            w.add_child(source, a).is_err(),
            "shared declarations must stay leaves"
        );
        assert!(w.set_parent(source, None).is_err());
        assert!(w.detach_child(a, source));
        assert!(!w.attachment_is_live(original));
        assert!(w.attachment_is_live(b_edge));
        assert!(
            w.attachment_is_live(c_edge),
            "promoting an inline edge preserves its revision"
        );
        assert!(w.detach_child(b, source));
        assert!(!w.attachment_is_live(b_edge));
        assert!(w.attachment_is_live(c_edge));
        assert_eq!(w.parent_of(source), Some(c));
        assert_eq!(w.parents_of(source).collect::<Vec<_>>(), vec![c]);
        assert_eq!(w.attachment_of(source), Some(c_edge));
        assert_eq!(w.attachment_between(b, source), None);
        w.add_child(b, source).unwrap();
        let b_edge = w.attachment_between(b, source).unwrap();
        assert_eq!(w.parent_of(source), None);
        w.add_child(a, source).unwrap();
        assert!(!w.attachment_is_live(original));
        assert!(!w.detach_child(a, b));
        w.detach_all_parents(source);
        assert_eq!(w.parents_of(source).count(), 0);
        assert!(w.children_of(a).is_empty());
        assert!(w.children_of(b).is_empty());
        assert!(w.children_of(c).is_empty());
        assert!(w.get_component_record(source).is_some());
        w.add_child(b, source).unwrap();
        assert!(!w.attachment_is_live(b_edge));
        assert_eq!(w.parent_of(source), Some(b));
    }

    #[test]
    fn deleting_a_parent_preserves_shared_shading_but_explicit_destruction_unlinks_all() {
        use crate::engine::ecs::component::{ShadingComponent, TransformComponent};
        let mut w = World::default();
        let a = w.add_component(TransformComponent::new());
        let b = w.add_component(TransformComponent::new());
        let source = w.add_component(ShadingComponent::new());
        w.add_child(a, source).unwrap();
        w.add_child(b, source).unwrap();
        let survivor = w.attachment_between(b, source).unwrap();
        w.remove_component_subtree(a).unwrap();
        assert!(w.get_component_record(source).is_some());
        assert!(w.attachment_is_live(survivor));
        let c = w.add_component(TransformComponent::new());
        w.add_child(c, source).unwrap();
        w.remove_component_subtree(source).unwrap();
        assert!(w.children_of(b).is_empty());
        assert!(w.children_of(c).is_empty());
        assert!(!w.attachment_is_live(survivor));
    }

    #[test]
    fn shared_shading_diamond_initializes_once_and_dies_with_its_last_owners() {
        use crate::engine::ecs::component::{ShadingComponent, TransformComponent};
        use crate::engine::ecs::{CommandQueue, IntentValue};
        let mut w = World::default();
        let root = w.add_component(TransformComponent::new());
        let a = w.add_component(TransformComponent::new());
        let b = w.add_component(TransformComponent::new());
        let source = w.add_component(ShadingComponent::new());
        w.add_child(root, a).unwrap();
        w.add_child(root, b).unwrap();
        w.add_child(a, source).unwrap();
        w.add_child(b, source).unwrap();
        let mut queue = CommandQueue::new();
        w.init_component_tree(root, &mut queue);
        let mut rx = crate::engine::ecs::RxWorld::default();
        queue.drain_into_rx(&mut rx);
        let count = rx.drain_ready_intents().iter().filter(|env| {
            matches!(env.intent.as_ref().map(|i| &i.value), Some(IntentValue::RegisterAnimeShading { component_id }) if *component_id == source)
        }).count();
        assert_eq!(count, 1);
        assert_eq!(w.find_all_components(root, "Shading"), vec![source]);
        w.remove_component_subtree(root).unwrap();
        assert_eq!(w.component_count(), 0);
    }

    #[test]
    fn shading_wrappers_cannot_be_shared_but_structural_reparenting_is_preserved() {
        use crate::engine::ecs::component::{ShadingComponent, TransformComponent};
        let mut w = World::default();
        let a = w.add_component(TransformComponent::new());
        let b = w.add_component(TransformComponent::new());
        let wrapper = w.add_component(ShadingComponent::new());
        let spatial = w.add_component(TransformComponent::new());
        w.add_child(wrapper, spatial).unwrap();
        w.add_child(a, wrapper).unwrap();
        assert!(w.add_child(b, wrapper).is_err());
        assert!(w.add_child(spatial, a).is_err());
        assert_eq!(w.parent_of(wrapper), Some(a));
        w.add_child(b, spatial).unwrap();
        assert_eq!(w.parent_of(spatial), Some(b));
        assert!(w.children_of(wrapper).is_empty());
    }

    #[test]
    fn selector_revision_tracks_metadata_and_topology_but_not_typed_pose_writes() {
        use crate::engine::ecs::component::TransformComponent;
        let mut world = World::default();
        let parent = world.add_component(TransformComponent::new());
        let child = world.add_component(TransformComponent::new());
        let mut revision = world.query_revision();
        world
            .get_component_by_id_as_mut::<TransformComponent>(child)
            .unwrap()
            .transform
            .matrix_world[3][1] = 2.0;
        assert_eq!(world.query_revision(), revision);
        world.get_component_record_mut(child).unwrap().name = "renamed".into();
        assert!(world.query_revision() > revision);
        revision = world.query_revision();
        world.add_child(parent, child).unwrap();
        assert!(world.query_revision() > revision);
        revision = world.query_revision();
        world.detach_from_parent(child);
        assert!(world.query_revision() > revision);
        revision = world.query_revision();
        world.remove_component_leaf(child).unwrap();
        assert!(world.query_revision() > revision);
    }

    #[test]
    fn attachment_references_distinguish_replacement_and_component_reuse() {
        use crate::engine::ecs::component::TransformComponent;
        let mut world = World::default();
        let parent = world.add_component(TransformComponent::new());
        let child = world.add_component(TransformComponent::new());
        world.add_child(parent, child).unwrap();
        let first = world.attachment_of(child).unwrap();
        assert_eq!(first.parent(), parent);
        assert_eq!(first.child(), child);
        assert!(world.attachment_is_live(first));
        assert!(world.add_child(child, parent).is_err());
        assert!(
            world.attachment_is_live(first),
            "failed mutation preserves edge"
        );

        world.detach_from_parent(child);
        assert!(!world.attachment_is_live(first));
        world.add_child(parent, child).unwrap();
        let replacement = world.attachment_of(child).unwrap();
        assert_ne!(first, replacement);
        assert!(!world.attachment_is_live(first));
        world.remove_component_leaf(child).unwrap();
        let reused = world.add_component(TransformComponent::new());
        world.add_child(parent, reused).unwrap();
        assert!(!world.attachment_is_live(replacement));
        let current = world.attachment_of(reused).unwrap();
        world.remove_component_subtree(parent).unwrap();
        assert!(!world.attachment_is_live(current));
    }

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

    #[test]
    fn find_component_matches_guid_via_mmq_and_css_forms() {
        let mut w = World::default();

        let root = w.add_component_boxed_named(
            "root",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let target = w.add_component(crate::engine::ecs::component::TransformComponent::new());
        w.add_child(root, target).unwrap();

        let guid = w.get_component_record(target).unwrap().guid;
        let guid_str = guid.to_string();

        let mmq_sel = format!("@uuid:{}", guid_str);
        assert_eq!(w.find_component(root, &mmq_sel), Some(target));

        let css_sel = format!("[guid={}]", guid_str);
        assert_eq!(w.find_component(root, &css_sel), Some(target));

        // Bogus guid string → no match, no panic.
        assert_eq!(w.find_component(root, "[guid=not-a-uuid]"), None);
    }

    #[test]
    fn parse_scoped_query_recognizes_relative_prefixes() {
        let scoped = parse_scoped_query("../../ #target");
        assert_eq!(
            scoped.root_mode,
            QueryRootMode::ParentScope { levels_up: 2 }
        );
        assert_eq!(scoped.selector, "#target");

        let scoped = parse_scoped_query("/ > #scene");
        assert_eq!(scoped.root_mode, QueryRootMode::WorldRoot);
        assert_eq!(scoped.selector, "> #scene");
    }

    #[test]
    fn resolve_component_ref_supports_parent_and_world_root_prefixes() {
        let mut w = World::default();
        let scene_root = w.add_component_boxed_named(
            "scene_root",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let owner_parent = w.add_component_boxed_named(
            "owner_parent",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let owner = w.add_component_boxed_named(
            "owner",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        let local_target = w.add_component_boxed_named(
            "local_target",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );

        w.add_child(scene_root, owner_parent).unwrap();
        w.add_child(owner_parent, owner).unwrap();
        w.add_child(owner_parent, local_target).unwrap();

        let parent_relative = resolve_component_ref(
            &w,
            &ComponentRef::Query("../#local_target".to_string()),
            Some(owner),
            QueryRootMode::WorldRoot,
        );
        assert_eq!(parent_relative, Some(local_target));

        let world_root = resolve_component_ref(
            &w,
            &ComponentRef::Query("/ > #scene_root".to_string()),
            Some(owner),
            QueryRootMode::SelfSubtree,
        );
        assert_eq!(world_root, Some(scene_root));
    }
}
