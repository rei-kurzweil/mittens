use super::*;
use crate::engine::ecs::component::{CollidableComponent, ZoneComponent};

fn posed(world: &mut World, position: [f32; 3]) -> ComponentId {
    let mut transform =
        TransformComponent::new().with_position(position[0], position[1], position[2]);
    transform.transform.matrix_world = transform.transform.model;
    world.add_component(transform)
}

fn actor(world: &mut World, position: [f32; 3]) -> (ComponentId, ComponentId) {
    let anchor = posed(world, [0.0; 3]);
    let gltf = world.add_component(GLTFComponent::new("collision-test.glb"));
    let joint = posed(world, position);
    world.add_child(anchor, gltf).unwrap();
    world.add_child(gltf, joint).unwrap();
    let asset = world
        .get_component_by_id_as_mut::<GLTFComponent>(gltf)
        .unwrap();
    asset.spawned_node_transforms = vec![joint];
    asset.armature_joint_transforms = vec![joint];
    (gltf, joint)
}

fn tail(
    world: &mut World,
    system: &mut SecondaryMotionSystem,
) -> (ComponentId, ComponentId, ComponentId) {
    let (gltf, joint) = actor(world, [0.0, 0.4, 0.0]);
    let rest = world.add_component(BoneRestPoseComponent::new(
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
        [1.0; 3],
    ));
    world.add_child(joint, rest).unwrap();
    let root = world.add_component(SecondaryMotionComponent::new());
    let reference = ComponentRef::Guid(world.get_component_record(joint).unwrap().guid);
    let chain = world.add_component(
        SpringBoneComponent::from_root(reference)
            .virtual_end_length_ratio(1.0)
            .gravity(20.0, [0.0, -1.0, 0.0])
            .hit_radius(0.05),
    );
    world.add_child(gltf, root).unwrap();
    world.add_child(root, chain).unwrap();
    system.register(world, root);
    let ChainStatus::Bound(state) = &mut system.chains.get_mut(&chain).unwrap().status else {
        panic!("unbound")
    };
    state.current[0] = [1.0, 0.4, 0.0];
    state.previous[0] = state.current[0];
    (gltf, chain, joint)
}

fn state(system: &SecondaryMotionSystem, chain: ComponentId) -> &BoundChain {
    let ChainStatus::Bound(state) = &system.chains[&chain].status else {
        panic!("unbound")
    };
    state
}

fn hand(world: &mut World, position: [f32; 3]) -> (ComponentId, ComponentId, ComponentId) {
    let (gltf, joint) = actor(world, position);
    let reference = ComponentRef::Guid(world.get_component_record(joint).unwrap().guid);
    let config = world.add_component(SpringColliderComponent::sphere(reference, 0.2));
    world.add_child(gltf, config).unwrap();
    (gltf, joint, config)
}

#[test]
fn tail_rests_on_regular_floor_preserving_length_and_releases_after_disable() {
    let mut world = World::default();
    let mut system = SecondaryMotionSystem::default();
    let (_, chain, _) = tail(&mut world, &mut system);
    let floor = posed(&mut world, [0.0, -0.01, 0.0]);
    let zone = world.add_component(ZoneComponent::cube([5.0, 0.01, 5.0]));
    let collider = world.add_component(CollidableComponent::static_());
    world.add_child(floor, zone).unwrap();
    world.add_child(zone, collider).unwrap();
    let mut surfaces = CollidableBvh::default();
    surfaces.sync(&world);
    for _ in 0..240 {
        system.tick_with_collisions(
            &mut world,
            STEP,
            &HashMap::new(),
            true,
            false,
            Some(&surfaces),
        );
        let endpoint = state(&system, chain).current[0];
        assert!(endpoint[1] >= 0.0499, "tail penetrated floor: {endpoint:?}");
        assert!((vec3_len(vec3_sub(endpoint, [0.0, 0.4, 0.0])) - 1.0).abs() < 1e-5);
    }
    assert!(state(&system, chain).current[0][1] < 0.051);
    world
        .get_component_by_id_as_mut::<CollidableComponent>(collider)
        .unwrap()
        .enabled = false;
    surfaces.sync(&world);
    for _ in 0..120 {
        system.tick_with_collisions(
            &mut world,
            STEP,
            &HashMap::new(),
            true,
            false,
            Some(&surfaces),
        );
    }
    assert!(state(&system, chain).current[0][1] < -0.1);
}

#[test]
fn thin_floor_sweep_and_world_collision_opt_out() {
    let mut world = World::default();
    let mut system = SecondaryMotionSystem::default();
    let (_, chain, _) = tail(&mut world, &mut system);
    let floor = posed(&mut world, [0.0, -0.001, 0.0]);
    let zone = world.add_component(ZoneComponent::cube([5.0, 0.001, 5.0]));
    let collider = world.add_component(CollidableComponent::static_());
    world.add_child(floor, zone).unwrap();
    world.add_child(zone, collider).unwrap();
    let mut surfaces = CollidableBvh::default();
    surfaces.sync(&world);
    let mut bound = state(&system, chain).clone();
    bound.current[0] = [0.98_f32.sqrt(), 0.4 - 0.02_f32.sqrt(), 0.0];
    bound.previous[0] = [0.7, 1.0, 0.0]; // Cross the entire floor in one step.
    let frame = system.collision_frame(&world);
    let settings = world
        .get_component_by_id_as::<SpringBoneComponent>(chain)
        .unwrap()
        .clone();
    let contacts = chain_contacts(&world, &bound, &settings, &frame, Some(&surfaces));
    simulate_step(&world, &mut bound, &contacts);
    assert!(
        bound.current[0][1] >= 0.0499,
        "sweep missed: {:?}",
        bound.current
    );
    world
        .get_component_by_id_as_mut::<SpringBoneComponent>(chain)
        .unwrap()
        .world_collision = false;
    for _ in 0..240 {
        system.tick_with_collisions(
            &mut world,
            STEP,
            &HashMap::new(),
            true,
            false,
            Some(&surfaces),
        );
    }
    assert!(state(&system, chain).current[0][1] < -0.1);
}

#[test]
fn foreign_hand_contacts_are_live_scoped_scaled_and_removable() {
    let mut world = World::default();
    let mut system = SecondaryMotionSystem::default();
    let (gltf, chain, _) = tail(&mut world, &mut system);
    let (_, foreign_joint, config) = hand(&mut world, [1.0, 0.4, 0.0]);
    // An unselected collider on this avatar must not suddenly become a self collider.
    let own = world.add_component(SpringColliderComponent::sphere(
        ComponentRef::Guid(
            world
                .get_component_record(state(&system, chain).joints[0].id)
                .unwrap()
                .guid,
        ),
        10.0,
    ));
    world.add_child(gltf, own).unwrap();
    let frame = system.collision_frame(&world);
    let settings = world
        .get_component_by_id_as::<SpringBoneComponent>(chain)
        .unwrap()
        .clone();
    let contacts = chain_contacts(&world, state(&system, chain), &settings, &frame, None);
    assert_eq!(contacts.spheres.len(), 1);
    system.tick(&mut world, STEP);
    let endpoint = state(&system, chain).current[0];
    assert!(vec3_len(vec3_sub(endpoint, [1.0, 0.4, 0.0])) >= 0.2499);
    assert!((vec3_len(vec3_sub(endpoint, [0.0, 0.4, 0.0])) - 1.0).abs() < 1e-5);
    // Ordinary pose writes resample positions/radii without resolving selectors again.
    let revision = system.collider_pool_revision;
    world
        .get_component_by_id_as_mut::<TransformComponent>(foreign_joint)
        .unwrap()
        .transform
        .matrix_world[3][0] = 50.0;
    let frame = system.collision_frame(&world);
    assert_eq!(system.collider_pool_revision, revision);
    let contacts = chain_contacts(&world, state(&system, chain), &settings, &frame, None);
    assert!(contacts.spheres.is_empty());
    let transform = world
        .get_component_by_id_as_mut::<TransformComponent>(foreign_joint)
        .unwrap();
    transform.transform.matrix_world[3][0] = 1.0;
    transform.transform.matrix_world[0][0] = 2.0;
    let frame = system.collision_frame(&world);
    assert_eq!(frame.by_source[&(config, foreign_joint)].radius, 0.4);
    world
        .get_component_by_id_as_mut::<SpringBoneComponent>(chain)
        .unwrap()
        .avatar_collision = false;
    let settings = world
        .get_component_by_id_as::<SpringBoneComponent>(chain)
        .unwrap()
        .clone();
    assert!(
        chain_contacts(&world, state(&system, chain), &settings, &frame, None)
            .spheres
            .is_empty()
    );
    world
        .get_component_by_id_as_mut::<SpringColliderComponent>(config)
        .unwrap()
        .radius = 0.3;
    let frame = system.collision_frame(&world);
    assert_eq!(frame.by_source[&(config, foreign_joint)].radius, 0.6);
    world.remove_component_leaf(config).unwrap();
    assert!(
        !system
            .collision_frame(&world)
            .by_source
            .contains_key(&(config, foreign_joint))
    );
}

#[test]
fn bisket_reimu_and_lumae_bind_both_enlarged_hand_colliders() {
    use crate::engine::Universe;
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(r#"
        import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
        import { lumae_colliders } from "../assets/components/characters/lumae/colliders.mms"
        import { bisket_secondary_motion } from "../assets/components/secondary_motion/bisket.mms"
        import { lumae_secondary_motion } from "../assets/components/characters/lumae/secondary_motion.mms"
        T.position(-2, 2, 0) { GLTF("assets/models/bisket.glb") { bisket_colliders() bisket_secondary_motion(false) } }
        T.position(0, 2, 0) { GLTF("assets/models/rei(mu).glb") { bisket_colliders() bisket_secondary_motion(false) } }
        T.position(2, 2, 0) { GLTF("assets/components/characters/lumae/lumae.glb") { lumae_colliders() lumae_secondary_motion() } }
    "#, "examples/_spring-collision-test.mms").unwrap();
    for _ in 0..4 {
        scene.update(STEP, &Default::default());
    }
    let snapshots = scene.systems.secondary_motion.bound_snapshot(&scene.world);
    let gltfs: Vec<_> = scene
        .world
        .all_components()
        .filter(|&id| {
            scene
                .world
                .get_component_by_id_as::<GLTFComponent>(id)
                .is_some()
        })
        .collect();
    assert_eq!(gltfs.len(), 3);
    for gltf in gltfs {
        let hands: HashSet<_> = snapshots
            .iter()
            .filter(|chain| chain.gltf == gltf)
            .flat_map(|chain| &chain.colliders)
            .filter(|collider| {
                matches!(
                    scene.world.component_label(collider.target),
                    Some("J_Bip_L_Hand" | "J_Bip_R_Hand")
                )
            })
            .map(|collider| {
                assert!(
                    (collider.scaled_base_radius - 0.11).abs() < 1e-4,
                    "hand radius: {collider:?}"
                );
                collider.target
            })
            .collect();
        assert_eq!(
            hands.len(),
            2,
            "missing hand colliders for {:?}",
            scene
                .world
                .get_component_by_id_as::<GLTFComponent>(gltf)
                .unwrap()
                .uri
        );
    }
}

#[test]
fn floor_contact_reanchors_downstream_segments_without_stretching() {
    let mut world = World::default();
    let mut system = SecondaryMotionSystem::default();
    let (_, chain, _) = tail(&mut world, &mut system);
    let second = posed(&mut world, [0.5, 0.4, 0.0]);
    let mut bound = state(&system, chain).clone();
    let mut next = bound.joints[0].clone();
    next.id = second;
    next.parent_id = bound.joints[0].id;
    bound.joints.push(next);
    bound.lengths = vec![0.5, 0.5];
    bound.current = vec![[0.5, 0.4, 0.0], [1.0, 0.4, 0.0]];
    bound.previous = bound.current.clone();
    let floor = posed(&mut world, [0.0, -0.01, 0.0]);
    let zone = world.add_component(ZoneComponent::cube([5.0, 0.01, 5.0]));
    let collider = world.add_component(CollidableComponent::static_());
    world.add_child(floor, zone).unwrap();
    world.add_child(zone, collider).unwrap();
    let mut surfaces = CollidableBvh::default();
    surfaces.sync(&world);
    let frame = system.collision_frame(&world);
    let settings = world
        .get_component_by_id_as::<SpringBoneComponent>(chain)
        .unwrap();
    let contacts = chain_contacts(&world, &bound, settings, &frame, Some(&surfaces));
    for _ in 0..240 {
        simulate_step(&world, &mut bound, &contacts);
        let mut head = [0.0, 0.4, 0.0];
        for endpoint in &bound.current {
            assert!(
                (vec3_len(vec3_sub(*endpoint, head)) - 0.5).abs() < 1e-5,
                "stretched: {:?}",
                bound.current
            );
            assert!(endpoint[1] >= 0.0499, "penetrated: {:?}", bound.current);
            head = *endpoint;
        }
    }
}
