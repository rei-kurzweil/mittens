
use super::*;

struct Fixture {
    world: World,
    system: SecondaryMotionSystem,
    root: ComponentId,
    chain: ComponentId,
    first_config: ComponentId,
    imported: Vec<ComponentId>,
}

fn fixture() -> Fixture {
    let mut world = World::default();
    let anchor = world.add_component(TransformComponent::new());
    let gltf = world.add_component(GLTFComponent::new("retained-test.glb"));
    let first =
        world.add_component_boxed_named("retained_first", Box::new(TransformComponent::new()));
    let second = world.add_component_boxed_named(
        "retained_second",
        Box::new(TransformComponent::new().with_position(0.0, 1.0, 0.0)),
    );
    let first_rest = world.add_component(BoneRestPoseComponent::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.0; 3],
    ));
    let second_rest = world.add_component(BoneRestPoseComponent::new(
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.0; 3],
    ));
    let root = world.add_component(SecondaryMotionComponent::new());
    let chain = world.add_component(SpringBoneComponent::new("retained"));
    let first_config = world.add_component(SpringJointComponent::query("#retained_first"));
    let second_config = world.add_component(SpringJointComponent::query("#retained_second"));

    world.add_child(anchor, gltf).unwrap();
    world.add_child(gltf, first).unwrap();
    world.add_child(first, first_rest).unwrap();
    world.add_child(first, second).unwrap();
    world.add_child(second, second_rest).unwrap();
    world.add_child(gltf, root).unwrap();
    world.add_child(root, chain).unwrap();
    world.add_child(chain, first_config).unwrap();
    world.add_child(chain, second_config).unwrap();
    let component = world
        .get_component_by_id_as_mut::<GLTFComponent>(gltf)
        .unwrap();
    component.spawned_node_transforms = vec![first, second];
    component.armature_joint_transforms = vec![first, second];

    Fixture {
        world,
        system: SecondaryMotionSystem::default(),
        root,
        chain,
        first_config,
        imported: vec![first, second],
    }
}

fn automatic_fixture() -> (World, ComponentId, ComponentId, ComponentId, ComponentId) {
    let mut fixture = fixture();
    let gltf = nearest_armature_owner(&fixture.world, fixture.root).unwrap();
    fixture
        .world
        .get_component_by_id_as_mut::<SpringBoneComponent>(fixture.chain)
        .unwrap()
        .root = Some(ComponentRef::Query("#retained_first".into()));
    let explicit_configs: Vec<_> = fixture
        .world
        .children_of(fixture.chain)
        .iter()
        .copied()
        .filter(|id| {
            fixture
                .world
                .get_component_by_id_as::<SpringJointComponent>(*id)
                .is_some()
        })
        .collect();
    for config in explicit_configs {
        fixture.world.detach_from_parent(config);
    }
    // Helpers below a joint must not participate in automatic skin topology.
    let collider = fixture.world.add_component_boxed_named(
        "retained_first_collider",
        Box::new(TransformComponent::new()),
    );
    fixture
        .world
        .add_child(fixture.imported[0], collider)
        .unwrap();
    (
        fixture.world,
        gltf,
        fixture.chain,
        fixture.imported[0],
        fixture.imported[1],
    )
}

#[test]
fn automatic_chain_follows_skin_descendants_and_ignores_helpers() {
    let (world, gltf, chain, first, second) = automatic_fixture();
    let (bound, _) = build_chain(&world, gltf, chain, &[]).unwrap();
    assert_eq!(
        bound
            .joints
            .iter()
            .map(|joint| joint.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    assert!(bound.colliders.is_empty());
}

#[test]
fn automatic_leaf_chain_uses_virtual_endpoint_and_chain_tuning() {
    let (mut world, gltf, chain, _first, second) = automatic_fixture();
    {
        let component = world
            .get_component_by_id_as_mut::<SpringBoneComponent>(chain)
            .unwrap();
        component.root = Some(ComponentRef::Query("#retained_second".into()));
        component.virtual_end_length_ratio = Some(1.0);
        component.stiffness = 2.0;
        component.drag_force = 0.35;
        component.gravity_power = 3.0;
    }
    let (bound, _) = build_chain(&world, gltf, chain, &[]).unwrap();
    assert_eq!(bound.joints.len(), 1);
    assert_eq!(bound.joints[0].id, second);
    assert_eq!(bound.joints[0].stiffness, 2.0);
    assert_eq!(bound.joints[0].drag, 0.35);
    assert_eq!(bound.joints[0].gravity, [0.0, -3.0, 0.0]);
}

#[test]
fn explicit_collider_queries_are_gltf_scoped_deduplicated_and_skip_bad_targets() {
    let (mut world, gltf, chain, first, _second) = automatic_fixture();
    let config = world.add_component_boxed_named(
        "explicit_group",
        Box::new(SpringColliderComponent::spheres(
            vec![
                ComponentRef::Query("#retained_first".into()),
                ComponentRef::Query("#retained_first".into()),
                ComponentRef::Query("#missing_target".into()),
            ],
            0.25,
        )),
    );
    world.add_child(gltf, config).unwrap();
    let config_guid = world.get_component_record(config).unwrap().guid;
    {
        let spring = world
            .get_component_by_id_as_mut::<SpringBoneComponent>(chain)
            .unwrap();
        spring.colliders = vec![
            ComponentRef::Query("#explicit_group".into()),
            ComponentRef::Guid(config_guid),
            ComponentRef::Query("#missing_group".into()),
        ];
        spring.hit_radius = 0.05;
    }
    let (bound, warnings) = build_chain(&world, gltf, chain, &[]).unwrap();
    assert_eq!(bound.colliders.len(), 1);
    assert_eq!(bound.colliders[0].target, first);
    assert_eq!(bound.hit_radius, 0.05);
    assert_eq!(warnings.len(), 2);

    let other_anchor = world.add_component(TransformComponent::new());
    let other_gltf = world.add_component(GLTFComponent::new("other.glb"));
    let outside = world.add_component_boxed_named(
        "outside_group",
        Box::new(SpringColliderComponent::sphere(
            ComponentRef::Query("#retained_first".into()),
            1.0,
        )),
    );
    world.add_child(other_anchor, other_gltf).unwrap();
    world.add_child(other_gltf, outside).unwrap();
    let outside_guid = world.get_component_record(outside).unwrap().guid;
    world
        .get_component_by_id_as_mut::<SpringBoneComponent>(chain)
        .unwrap()
        .colliders = vec![ComponentRef::Guid(outside_guid)];
    let (bound, warnings) = build_chain(&world, gltf, chain, &[]).unwrap();
    assert!(bound.colliders.is_empty());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn bound_snapshot_projects_only_resolved_solver_geometry() {
    let mut fixture = fixture();
    let gltf = nearest_armature_owner(&fixture.world, fixture.root).unwrap();
    let collider = fixture.world.add_component_boxed_named(
        "snapshot_collider",
        Box::new(SpringColliderComponent::sphere(
            ComponentRef::Query("#retained_first".into()),
            0.25,
        )),
    );
    fixture.world.add_child(gltf, collider).unwrap();
    let collider_guid = fixture.world.get_component_record(collider).unwrap().guid;
    {
        let chain = fixture
            .world
            .get_component_by_id_as_mut::<SpringBoneComponent>(fixture.chain)
            .unwrap();
        chain.colliders = vec![ComponentRef::Guid(collider_guid)];
        chain.hit_radius = 0.05;
    }
    fixture.system.register(&fixture.world, fixture.root);

    let snapshot = fixture.system.bound_snapshot(&fixture.world);
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].armature_owner, gltf);
    assert_eq!(snapshot[0].chain, fixture.chain);
    assert!(snapshot[0].enabled);
    assert_eq!(snapshot[0].hit_radius, 0.05);
    assert_eq!(snapshot[0].segments.len(), 2);
    assert_eq!(snapshot[0].colliders.len(), 1);
    assert_eq!(snapshot[0].colliders[0].config, collider);
    assert_eq!(snapshot[0].colliders[0].target, fixture.imported[0]);
    assert_eq!(snapshot[0].colliders[0].scaled_base_radius, 0.25);
}

#[test]
fn sphere_exclusion_adds_hit_radius_and_preserves_segment_length() {
    let head = [0.0, 0.0, 0.0];
    let center = [0.0, 0.75, 0.0];
    let endpoint = [0.0, 1.0, 0.0];
    let projected = exclude_sphere_at_length(head, endpoint, 1.0, center, 0.5 + 0.1);
    assert!((vec3_len(vec3_sub(projected, head)) - 1.0).abs() < 1e-5);
    assert!((vec3_len(vec3_sub(projected, center)) - 0.6).abs() < 1e-5);
}

#[test]
fn coincident_sphere_center_has_a_deterministic_finite_result() {
    let first = exclude_sphere_at_length([0.0; 3], [0.0; 3], 1.0, [0.0; 3], 0.5);
    let second = exclude_sphere_at_length([0.0; 3], [0.0; 3], 1.0, [0.0; 3], 0.5);
    assert_eq!(first, second);
    assert_eq!(first, [1.0, 0.0, 0.0]);
}

#[test]
fn collider_target_topology_and_configuration_changes_rebind_the_chain() {
    let mut fixture = fixture();
    let gltf = nearest_armature_owner(&fixture.world, fixture.root).unwrap();
    let config = fixture.world.add_component_boxed_named(
        "live_group",
        Box::new(SpringColliderComponent::sphere(
            ComponentRef::Query("#retained_first".into()),
            0.25,
        )),
    );
    fixture.world.add_child(gltf, config).unwrap();
    let guid = fixture.world.get_component_record(config).unwrap().guid;
    fixture
        .world
        .get_component_by_id_as_mut::<SpringBoneComponent>(fixture.chain)
        .unwrap()
        .colliders = vec![ComponentRef::Guid(guid)];
    fixture.system.register(&fixture.world, fixture.root);

    let before = fixture.system.discovery_counts().0;
    fixture
        .system
        .topology_changed(&fixture.world, fixture.imported[0]);
    assert_eq!(fixture.system.discovery_counts().0, before + 1);

    fixture
        .world
        .get_component_by_id_as_mut::<SpringColliderComponent>(config)
        .unwrap()
        .radius = 0.5;
    fixture.system.configuration_changed(&fixture.world, config);
    assert_eq!(fixture.system.discovery_counts().0, before + 2);
}

#[test]
fn automatic_chain_rejects_skin_branches_with_diagnostic() {
    let (mut world, gltf, chain, first, _second) = automatic_fixture();
    let branch =
        world.add_component_boxed_named("retained_branch", Box::new(TransformComponent::new()));
    let rest = world.add_component(BoneRestPoseComponent::new(
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.0; 3],
    ));
    world.add_child(first, branch).unwrap();
    world.add_child(branch, rest).unwrap();
    {
        let component = world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap();
        component.spawned_node_transforms.push(branch);
        component.armature_joint_transforms.push(branch);
    }
    let error = build_chain(&world, gltf, chain, &[]).unwrap_err();
    assert!(error.message.contains("branches"), "{}", error.message);
    assert!(
        error.message.contains("retained_branch"),
        "{}",
        error.message
    );
}

#[test]
fn automatic_chain_rebinds_after_skin_topology_changes() {
    let (mut world, gltf, chain, first, _second) = automatic_fixture();
    let root = world.parent_of(chain).unwrap();
    let mut system = SecondaryMotionSystem::default();
    system.register(&world, root);
    assert_eq!(system.runtime_counts(), (1, 1, 1, 0, 0));

    let branch =
        world.add_component_boxed_named("late_branch", Box::new(TransformComponent::new()));
    let rest = world.add_component(BoneRestPoseComponent::new(
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.0; 3],
    ));
    world.add_child(first, branch).unwrap();
    world.add_child(branch, rest).unwrap();
    {
        let component = world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap();
        component.spawned_node_transforms.push(branch);
        component.armature_joint_transforms.push(branch);
    }
    system.topology_changed(&world, first);
    assert_eq!(system.runtime_counts(), (1, 1, 0, 0, 1));

    world.detach_from_parent(branch);
    system.topology_changed(&world, first);
    assert_eq!(system.runtime_counts(), (1, 1, 1, 0, 0));
}

#[test]
fn registration_is_order_independent_and_idempotent() {
    let mut fixture = fixture();
    fixture
        .system
        .register(&fixture.world, fixture.first_config);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
    let before = fixture.system.discovery_counts();
    fixture.system.register(&fixture.world, fixture.root);
    fixture.system.register(&fixture.world, fixture.chain);
    fixture
        .system
        .register(&fixture.world, fixture.first_config);
    assert_eq!(fixture.system.discovery_counts(), before);
}

#[test]
fn steady_state_tick_does_not_retry_binding_or_discover_topology() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    let before = fixture.system.discovery_counts();
    for _ in 0..256 {
        fixture.world.add_component(TransformComponent::new());
    }
    for _ in 0..8 {
        fixture.system.tick(&mut fixture.world, STEP);
    }
    assert_eq!(fixture.system.discovery_counts(), before);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
}

#[test]
fn global_pause_restores_once_preserves_authored_flags_and_resumes_from_live_pose() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    let first = fixture.imported[0];
    fixture
        .world
        .get_component_by_id_as_mut::<TransformComponent>(first)
        .unwrap()
        .transform
        .rotation = [0.0, 0.0, 0.70710677, 0.70710677];
    let ChainStatus::Bound(state) = &mut fixture
        .system
        .chains
        .get_mut(&fixture.chain)
        .unwrap()
        .status
    else {
        panic!("bound chain");
    };
    state.current.fill([100.0, 200.0, 300.0]);
    state.previous.fill([-100.0, -200.0, -300.0]);
    state.accumulator = STEP * 0.5;
    let before = fixture.system.discovery_counts();
    let dirty = fixture
        .system
        .tick_with_settings(&mut fixture.world, STEP, &HashMap::new(), false);
    assert_eq!(dirty, vec![first]);
    assert_eq!(
        fixture
            .world
            .get_component_by_id_as::<TransformComponent>(first)
            .unwrap()
            .transform
            .rotation,
        [0.0, 0.0, 0.0, 1.0]
    );
    assert!(
        fixture
            .world
            .get_component_by_id_as::<SpringBoneComponent>(fixture.chain)
            .unwrap()
            .enabled
    );
    // Animation can write joints while off; the solver must not overwrite them.
    let animated = [0.0, 0.0, 0.70710677, 0.70710677];
    fixture
        .world
        .get_component_by_id_as_mut::<TransformComponent>(first)
        .unwrap()
        .transform
        .rotation = animated;
    assert!(
        fixture
            .system
            .tick_with_settings(&mut fixture.world, STEP, &HashMap::new(), false)
            .is_empty()
    );
    assert_eq!(
        fixture
            .world
            .get_component_by_id_as::<TransformComponent>(first)
            .unwrap()
            .transform
            .rotation,
        animated
    );
    for (index, joint) in fixture.imported.iter().enumerate() {
        let transform = &mut fixture
            .world
            .get_component_by_id_as_mut::<TransformComponent>(*joint)
            .unwrap()
            .transform;
        transform.matrix_world[3] = [10.0, 20.0 + index as f32, 30.0, 1.0];
    }
    assert!(
        fixture
            .system
            .tick_with_settings(&mut fixture.world, STEP, &HashMap::new(), true)
            .is_empty()
    );
    let ChainStatus::Bound(state) = &fixture.system.chains[&fixture.chain].status else {
        panic!("bound chain");
    };
    assert!(state.enabled);
    assert_eq!(state.accumulator, 0.0);
    assert_eq!(
        state.current, state.previous,
        "no paused velocity may survive resume"
    );
    assert_eq!(state.current[0], [10.0, 21.0, 30.0]);
    assert_eq!(
        fixture.system.discovery_counts(),
        before,
        "toggle must not rebind geometry"
    );
}

#[test]
fn global_pause_handles_new_chains_and_keeps_authored_disabled_chains_off() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    fixture
        .system
        .tick_with_settings(&mut fixture.world, STEP, &HashMap::new(), false);
    let ChainStatus::Bound(state) = &fixture.system.chains[&fixture.chain].status else {
        panic!("bound chain");
    };
    assert!(!state.enabled, "a newly bound chain follows global off");
    fixture
        .world
        .get_component_by_id_as_mut::<SpringBoneComponent>(fixture.chain)
        .unwrap()
        .enabled = false;
    fixture
        .system
        .configuration_changed(&fixture.world, fixture.chain);
    assert!(
        fixture
            .system
            .tick_with_settings(&mut fixture.world, STEP, &HashMap::new(), true)
            .is_empty()
    );
    let ChainStatus::Bound(state) = &fixture.system.chains[&fixture.chain].status else {
        panic!("bound chain");
    };
    assert!(
        !state.enabled,
        "global on cannot enable an authored disabled chain"
    );
}

#[test]
fn rest_constraint_parks_chain_and_restores_imported_rotations() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    let mut weights = HashMap::new();
    weights.insert(fixture.chain, 1.0);
    let dirty = fixture
        .system
        .tick_with_constraints(&mut fixture.world, STEP, &weights);
    assert_eq!(dirty, vec![fixture.imported[0]]);
    let ChainStatus::Bound(state) = &fixture.system.chains[&fixture.chain].status else {
        panic!("chain should be bound");
    };
    assert!(state.parked);
    for joint in &state.joints {
        let rotation = fixture
            .world
            .get_component_by_id_as::<TransformComponent>(joint.id)
            .unwrap()
            .transform
            .rotation;
        assert_eq!(rotation, joint.rest_rotation);
    }
    weights.insert(fixture.chain, 0.0);
    fixture
        .system
        .tick_with_constraints(&mut fixture.world, STEP, &weights);
    let ChainStatus::Bound(state) = &fixture.system.chains[&fixture.chain].status else {
        panic!("chain should remain bound");
    };
    assert!(!state.parked);
}

#[test]
fn configuration_change_rebinds_only_the_owning_chain() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    let before = fixture.system.discovery_counts().0;
    fixture
        .world
        .get_component_by_id_as_mut::<SpringJointComponent>(fixture.first_config)
        .unwrap()
        .stiffness = 4.0;
    fixture
        .system
        .configuration_changed(&fixture.world, fixture.first_config);
    assert_eq!(fixture.system.discovery_counts().0, before + 1);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
}

#[test]
fn reparenting_and_subtree_cleanup_update_retained_ownership() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    fixture.world.detach_from_parent(fixture.chain);
    fixture
        .system
        .topology_changed(&fixture.world, fixture.chain);
    assert_eq!(fixture.system.runtime_counts(), (1, 0, 0, 0, 1));
    fixture
        .world
        .add_child(fixture.root, fixture.chain)
        .unwrap();
    fixture
        .system
        .topology_changed(&fixture.world, fixture.chain);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));

    fixture
        .system
        .component_removed(&fixture.world, fixture.first_config);
    fixture
        .system
        .component_removed(&fixture.world, fixture.chain);
    fixture
        .system
        .component_removed(&fixture.world, fixture.root);
    for imported in fixture.imported {
        fixture.system.component_removed(&fixture.world, imported);
    }
    assert_eq!(fixture.system.runtime_counts(), (0, 0, 0, 0, 0));
}

#[test]
fn gltf_readiness_retries_a_waiting_chain_without_frame_polling() {
    let mut fixture = fixture();
    let gltf = nearest_armature_owner(&fixture.world, fixture.root).unwrap();
    fixture
        .world
        .get_component_by_id_as_mut::<GLTFComponent>(gltf)
        .unwrap()
        .spawned_node_transforms
        .clear();
    fixture.system.register(&fixture.world, fixture.root);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 0, 1, 0));
    let before = fixture.system.discovery_counts().0;
    fixture.system.tick(&mut fixture.world, STEP);
    assert_eq!(fixture.system.discovery_counts().0, before);

    fixture
        .world
        .get_component_by_id_as_mut::<GLTFComponent>(gltf)
        .unwrap()
        .spawned_node_transforms = fixture.imported.clone();
    fixture.system.gltf_initialized(&fixture.world, gltf);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
}

#[test]
fn overlapping_chain_retries_when_the_owner_is_removed() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    let second_chain = fixture
        .world
        .add_component(SpringBoneComponent::new("overlap"));
    let first = fixture
        .world
        .add_component(SpringJointComponent::query("#retained_first"));
    let second = fixture
        .world
        .add_component(SpringJointComponent::query("#retained_second"));
    fixture.world.add_child(fixture.root, second_chain).unwrap();
    fixture.world.add_child(second_chain, first).unwrap();
    fixture.world.add_child(second_chain, second).unwrap();
    fixture.system.register(&fixture.world, second_chain);
    assert_eq!(fixture.system.runtime_counts(), (1, 2, 1, 0, 1));

    fixture
        .system
        .component_removed(&fixture.world, fixture.chain);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
}

#[test]
fn imported_transform_removal_invalidates_only_its_bound_chain() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    fixture
        .system
        .component_removed(&fixture.world, fixture.imported[0]);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 0, 1, 0));
    let before = fixture.system.discovery_counts();
    fixture.system.tick(&mut fixture.world, STEP);
    assert_eq!(fixture.system.discovery_counts(), before);
}

#[test]
fn joint_removal_reordering_and_readdition_rebind_the_chain() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    let second_config = fixture.world.children_of(fixture.chain)[1];

    fixture.world.detach_from_parent(second_config);
    fixture
        .system
        .topology_changed(&fixture.world, second_config);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 0, 0, 1));
    fixture
        .world
        .get_component_by_id_as_mut::<SpringBoneComponent>(fixture.chain)
        .unwrap()
        .virtual_end_length_ratio = Some(1.0);
    fixture
        .system
        .configuration_changed(&fixture.world, fixture.chain);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));

    fixture
        .world
        .add_child(fixture.chain, second_config)
        .unwrap();
    fixture
        .system
        .topology_changed(&fixture.world, second_config);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));
    fixture.world.detach_from_parent(fixture.first_config);
    fixture
        .world
        .add_child(fixture.chain, fixture.first_config)
        .unwrap();
    fixture
        .system
        .topology_changed(&fixture.world, fixture.first_config);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 0, 0, 1));
}

#[test]
fn root_reparent_transfers_gltf_ownership_and_targets_respawn() {
    let mut fixture = fixture();
    fixture.system.register(&fixture.world, fixture.root);
    let old_gltf = nearest_armature_owner(&fixture.world, fixture.root).unwrap();
    let anchor = fixture.world.parent_of(old_gltf).unwrap();
    let new_gltf = fixture
        .world
        .add_component(GLTFComponent::new("retained-test-respawn.glb"));
    fixture.world.add_child(anchor, new_gltf).unwrap();
    fixture
        .world
        .get_component_by_id_as_mut::<GLTFComponent>(new_gltf)
        .unwrap()
        .spawned_node_transforms = fixture.imported.clone();
    fixture.world.detach_from_parent(fixture.root);
    fixture.world.add_child(new_gltf, fixture.root).unwrap();
    fixture
        .system
        .topology_changed(&fixture.world, fixture.root);
    assert_eq!(fixture.system.runtime_counts(), (1, 1, 1, 0, 0));

    let before = fixture.system.discovery_counts().0;
    fixture.system.gltf_initialized(&fixture.world, old_gltf);
    assert_eq!(fixture.system.discovery_counts().0, before);
    fixture.system.gltf_initialized(&fixture.world, new_gltf);
    assert_eq!(fixture.system.discovery_counts().0, before + 1);
}
