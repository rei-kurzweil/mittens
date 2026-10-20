use super::*;
use crate::engine::ecs::component::{ComponentRef, HumanoidBoneMapComponent};
use crate::engine::ecs::{CommandQueue, SystemWorld};
use crate::engine::graphics::{RenderAssets, VisualWorld};
use std::time::Duration;

struct Fixture {
    world: World,
    systems: SystemWorld,
    visuals: VisualWorld,
    assets: RenderAssets,
    queue: CommandQueue,
    tracking: HandTrackingSystem,
    config: ComponentId,
    gltf: ComponentId,
    root: ComponentId,
    sequence: u64,
}

fn append_reimu(
    world: &mut World,
    rebase_axes: bool,
) -> (ComponentId, ComponentId, ComponentId, ComponentId) {
    let document = gltf::Gltf::open("assets/models/rei(mu).glb").unwrap();
    let root = world.add_component(
        TransformComponent::new()
            .with_position(2.0, 0.3, -1.0)
            .with_rotation_quat(quat_rotation_y(0.7)),
    );
    let gltf = world.add_component(GLTFComponent::new("assets/models/rei(mu).glb"));
    world.add_child(root, gltf).unwrap();
    let mut parents = vec![None; document.nodes().len()];
    for node in document.nodes() {
        for child in node.children() {
            parents[child.index()] = Some(node.index());
        }
    }
    let gauges: Vec<_> = document
        .nodes()
        .map(|node| {
            if rebase_axes {
                quat_from_axis_angle(vec3_normalize([0.3, 0.5, 0.8]), 0.17 * node.index() as f32)
            } else {
                [0.0, 0.0, 0.0, 1.0]
            }
        })
        .collect();
    let mut nodes = Vec::new();
    for node in document.nodes() {
        let (t, r, s) = node.transform().decomposed();
        let parent_gauge = parents[node.index()]
            .map(|p| gauges[p])
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let t = quat_rotate_vec3(quat_conjugate(parent_gauge), t);
        let r = quat_mul(
            quat_conjugate(parent_gauge),
            quat_mul(r, gauges[node.index()]),
        );
        let joint = world.add_component_boxed_named(
            node.name().unwrap_or("node"),
            Box::new(
                TransformComponent::new()
                    .with_position(t[0], t[1], t[2])
                    .with_rotation_quat(r)
                    .with_scale(s[0], s[1], s[2]),
            ),
        );
        let rest = world.add_component(BoneRestPoseComponent::new(t, r, s));
        world.add_child(root, joint).unwrap();
        world.add_child(joint, rest).unwrap();
        nodes.push(joint);
    }
    for node in document.nodes() {
        for child in node.children() {
            world
                .set_parent(nodes[child.index()], Some(nodes[node.index()]))
                .unwrap();
        }
    }
    let gltf_component = world
        .get_component_by_id_as_mut::<GLTFComponent>(gltf)
        .unwrap();
    gltf_component.armature_joint_transforms = document
        .skins()
        .next()
        .unwrap()
        .joints()
        .map(|node| nodes[node.index()])
        .collect();
    gltf_component.spawned_node_transforms = nodes.clone();
    gltf_component.spawned = true;
    let mut map = HumanoidBoneMapComponent::new().with_automap_disabled();
    for (side, label) in [(0, "L"), (1, "R")] {
        map = map.with_slot(
            if side == 0 {
                HumanoidSlot::LeftHand
            } else {
                HumanoidSlot::RightHand
            },
            ComponentRef::Query(format!("[name='J_Bip_{label}_Hand']")),
        );
        for (finger, slots) in finger_slots(side).into_iter().enumerate() {
            let name = ["Thumb", "Index", "Middle", "Ring", "Little"][finger];
            for (segment, slot) in slots.into_iter().enumerate() {
                map = map.with_slot(
                    slot,
                    ComponentRef::Query(format!("[name='J_Bip_{label}_{name}{}']", segment + 1)),
                );
            }
        }
    }
    let map = world.add_component(map);
    world.add_child(gltf, map).unwrap();
    let config = world.add_component(HandTrackingComponent::new());
    world.add_child(gltf, config).unwrap();
    (root, gltf, config, map)
}

impl Fixture {
    fn reimu(rebase_axes: bool) -> Self {
        let mut world = World::default();
        let (root, gltf, config, map) = append_reimu(&mut world, rebase_axes);
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        systems.transform_changed(&mut world, &mut visuals, root);
        systems
            .humanoid_bone_map
            .register_component(&mut world, map);
        let mut tracking = HandTrackingSystem::default();
        tracking.register(config);
        let mut fixture = Self {
            world,
            systems,
            visuals,
            assets: RenderAssets::new(),
            queue: CommandQueue::new(),
            tracking,
            config,
            gltf,
            root,
            sequence: 0,
        };
        fixture.step(1.0 / 90.0);
        assert!(
            fixture.tracking.diagnostics(config).is_empty(),
            "{:?}",
            fixture.tracking.diagnostics(config)
        );
        assert!(
            fixture.tracking.rigs[&config].hands.iter().all(|hand| hand
                .as_ref()
                .unwrap()
                .fingers
                .len()
                == 5)
        );
        fixture
    }

    fn step(&mut self, dt: f32) {
        self.tracking.tick(
            &mut self.world,
            &mut self.systems.humanoid_bone_map,
            &self.systems.joint_basis_retargeting,
            &mut self.queue,
            dt,
        );
        self.queue.flush(
            &mut self.world,
            &mut self.systems,
            &mut self.visuals,
            &mut self.assets,
        );
    }

    /// Build measured positions from the imported anatomical geometry, then change
    /// each segment independently. World position, rotation and hand size differ
    /// from the avatar, so direct world copying or translation retargeting fails.
    fn frame(&mut self, curl: [[f32; 5]; 2], splay: f32) -> Arc<HandTrackingFrame> {
        self.sequence += 1;
        let source_hand = quat_mul(
            quat_rotation_y(-1.1),
            quat_from_axis_angle([1.0, 0.0, 0.0], 0.8),
        );
        let root = imported_rest_space_root(
            &self.world,
            self.gltf,
            self.tracking.rigs[&self.config].hands[0]
                .as_ref()
                .unwrap()
                .hand,
        );
        let hands = std::array::from_fn(|side| {
            let binding = self.tracking.rigs[&self.config].hands[side]
                .as_ref()
                .unwrap();
            let hand_model = rest_model_from_root(&self.world, root, binding.hand).unwrap();
            let hand_frame = quat_mul(mat_to_quat(hand_model), binding.canonical_to_hand);
            let mut sample = TrackedHand {
                active: true,
                data_source: HandDataSource::Unobstructed,
                joints: [HandJointLocation::default(); 26],
                root_position: Some((HandJoint::Wrist, [8.0, 9.0, 10.0])),
                canonical_orientation: Some(source_hand),
                canonical_orientation_provenance: HandOrientationProvenance::LandmarkDerived,
            };
            for finger in &binding.fingers {
                let rest_positions: Vec<_> = finger
                    .bones
                    .iter()
                    .map(|bone| {
                        let m = rest_model_from_root(&self.world, root, bone.target).unwrap();
                        [m[3][0], m[3][1], m[3][2]]
                    })
                    .collect();
                let mut point = [8.0 + finger.finger as f32 * 0.03, 9.0, 10.0];
                for segment in 0..=3 {
                    let joint = HandJoint::FINGERS[finger.finger][segment];
                    sample.joints[joint as usize] = HandJointLocation {
                        position: point,
                        position_valid: true,
                        position_tracked: true,
                        ..Default::default()
                    };
                    if segment == 3 {
                        break;
                    }
                    let from = rest_positions[segment.min(1)];
                    let to = rest_positions[(segment + 1).min(2)];
                    let direction =
                        quat_rotate_vec3(quat_conjugate(hand_frame), vec3_sub(to, from));
                    let angle = curl[side][finger.finger] * (segment as f32 + 1.0);
                    let bend = quat_mul(
                        quat_from_axis_angle([0.0, 1.0, 0.0], angle),
                        quat_from_axis_angle([1.0, 0.0, 0.0], splay),
                    );
                    let direction =
                        quat_rotate_vec3(source_hand, quat_rotate_vec3(bend, direction));
                    point = vec3_add(point, vec3_scale(direction, 1.6));
                }
            }
            Some(sample)
        });
        Arc::new(HandTrackingFrame {
            provider: HandTrackingProvider::OpenXr,
            epoch: 1,
            sequence: self.sequence,
            sample_time: HandSampleTime::OpenXrPredictedNanoseconds(
                self.sequence as i64 * 11_111_111,
            ),
            received_at: Instant::now(),
            space: HandCoordinateSpace::OpenXrStage { generation: 1 },
            hands,
        })
    }

    fn assert_matches(&self, frame: &HandTrackingFrame) {
        let rig = &self.tracking.rigs[&self.config];
        for (side, binding) in rig.hands.iter().enumerate() {
            let binding = binding.as_ref().unwrap();
            let sample = frame.hands[side].as_ref().unwrap();
            let hand_world = world_rotation(&self.world, binding.hand).unwrap();
            let source_to_avatar = quat_mul(
                quat_mul(hand_world, binding.canonical_to_hand),
                quat_conjugate(sample.canonical_orientation.unwrap()),
            );
            for finger in &binding.fingers {
                for segment in 0..3 {
                    let bone = &finger.bones[segment];
                    let transform = self
                        .world
                        .get_component_by_id_as::<TransformComponent>(bone.target)
                        .unwrap();
                    let rest = self
                        .world
                        .children_of(bone.target)
                        .iter()
                        .find_map(|id| {
                            self.world
                                .get_component_by_id_as::<BoneRestPoseComponent>(*id)
                        })
                        .unwrap();
                    assert_eq!(transform.transform.translation, rest.translation);
                    assert_eq!(transform.transform.scale, rest.scale);
                    let actual = if segment < 2 {
                        vec3_sub(
                            TransformSystem::world_position(
                                &self.world,
                                finger.bones[segment + 1].target,
                            )
                            .unwrap(),
                            TransformSystem::world_position(&self.world, bone.target).unwrap(),
                        )
                    } else {
                        quat_rotate_vec3(
                            world_rotation(&self.world, bone.target).unwrap(),
                            quat_rotate_vec3(bone.canonical_to_rest, [0.0, 0.0, -1.0]),
                        )
                    };
                    let joints = HandJoint::FINGERS[finger.finger];
                    let expected = quat_rotate_vec3(
                        source_to_avatar,
                        vec3_sub(
                            sample.position(joints[segment + 1]).unwrap(),
                            sample.position(joints[segment]).unwrap(),
                        ),
                    );
                    assert!(
                        vec3_dot(
                            unit_direction(actual).unwrap(),
                            unit_direction(expected).unwrap()
                        ) > 0.9999,
                        "side={side} finger={} segment={segment} actual={actual:?} expected={expected:?}",
                        finger.finger
                    );
                }
            }
        }
    }

    fn assert_rest(&self) {
        for hand in self.tracking.rigs[&self.config].hands.iter().flatten() {
            for finger in &hand.fingers {
                for bone in &finger.bones {
                    let current = self
                        .world
                        .get_component_by_id_as::<TransformComponent>(bone.target)
                        .unwrap();
                    let rest = self
                        .world
                        .children_of(bone.target)
                        .iter()
                        .find_map(|id| {
                            self.world
                                .get_component_by_id_as::<BoneRestPoseComponent>(*id)
                        })
                        .unwrap();
                    assert!(
                        same_rotation(current.transform.rotation, rest.rotation),
                        "{:?}",
                        bone.target
                    );
                    assert_eq!(current.transform.translation, rest.translation);
                    assert_eq!(current.transform.scale, rest.scale);
                }
            }
        }
    }
}

#[test]
fn reimu_five_fingers_rest_curl_splay_opposition_and_controller_wrist() {
    for rebased in [false, true] {
        let mut fixture = Fixture::reimu(rebased);
        let rest = fixture.frame([[0.0; 5]; 2], 0.0);
        fixture.tracking.publish(rest.clone());
        fixture.step(1.0 / 90.0);
        fixture.assert_rest();
        fixture.assert_matches(&rest);
        for active in 0..5 {
            let mut curls = [[0.0; 5]; 2];
            curls[0][active] = 0.75;
            curls[1][active] = -0.55;
            let frame = fixture.frame(curls, 0.15);
            let hands: Vec<_> = fixture.tracking.rigs[&fixture.config]
                .hands
                .iter()
                .flatten()
                .map(|h| (h.hand, world_rotation(&fixture.world, h.hand).unwrap()))
                .collect();
            fixture.tracking.publish(frame.clone());
            fixture.step(1.0 / 90.0);
            fixture.assert_matches(&frame);
            for (hand, before) in hands {
                assert!(same_rotation(
                    world_rotation(&fixture.world, hand).unwrap(),
                    before
                ));
            }
        }
        let fist = fixture.frame([[1.0; 5]; 2], 0.0);
        fixture.tracking.publish(fist.clone());
        fixture.step(0.01);
        fixture.assert_matches(&fist);
        fixture.tracking.set_openxr_frame(None);
        for _ in 0..20 {
            fixture.step(1.0 / 90.0);
        }
        fixture.assert_rest();
    }
}

#[test]
fn partial_nan_degenerate_and_stale_hands_release_without_freezing() {
    let mut fixture = Fixture::reimu(false);
    let mut frame = fixture.frame([[0.8; 5]; 2], 0.0);
    fixture.tracking.publish(frame.clone());
    fixture.step(0.01);
    let sample = Arc::make_mut(&mut frame);
    sample.sequence += 1;
    sample.hands[0].as_mut().unwrap().joints[HandJoint::ThumbTip as usize].position[0] = f32::NAN;
    sample.hands[0].as_mut().unwrap().joints[HandJoint::IndexTip as usize].position_valid = false;
    let middle_distal = sample.hands[0]
        .as_ref()
        .unwrap()
        .position(HandJoint::MiddleDistal)
        .unwrap();
    sample.hands[0].as_mut().unwrap().joints[HandJoint::MiddleTip as usize].position =
        middle_distal;
    sample.hands[1] = None;
    fixture.tracking.publish(frame.clone());
    for _ in 0..20 {
        fixture.step(0.01);
    }
    let rig = &fixture.tracking.rigs[&fixture.config];
    for finger in &rig.hands[0].as_ref().unwrap().fingers {
        for bone in &finger.bones {
            assert_eq!(bone.pose.tracked, finger.finger >= 3);
        }
    }
    let sample = Arc::make_mut(&mut frame);
    sample.sequence += 1;
    sample.received_at = Instant::now() - Duration::from_secs(1);
    fixture.tracking.publish(frame);
    for _ in 0..20 {
        fixture.step(0.01);
    }
    fixture.assert_rest();
}

#[test]
fn release_tracks_new_authored_animation_and_stops_writing() {
    let rest = [0.0, 0.0, 0.0, 1.0];
    let tracked = quat_rotation_y(1.2);
    let authored = quat_rotation_y(-0.3);
    let mut owner = RotationOwner::new(rest);
    assert_eq!(owner.update(rest, Some(tracked), 0.01, 0.15), Some(tracked));
    let halfway = owner.update(authored, None, 0.075, 0.15).unwrap();
    assert!(same_rotation(halfway, quat_nlerp(tracked, authored, 0.5)));
    assert!(same_rotation(
        owner.update(halfway, None, 0.075, 0.15).unwrap(),
        authored
    ));
    assert!(owner.update(authored, None, 0.01, 0.15).is_none());
}

#[test]
fn per_hand_selection_rejects_reordered_future_mirrored_or_wrong_provider_samples() {
    let mut fixture = Fixture::reimu(false);
    let mut openxr = fixture.frame([[0.0; 5]; 2], 0.0);
    Arc::make_mut(&mut openxr).hands[1] = None;
    let mut mediapipe = (*fixture.frame([[0.0; 5]; 2], 0.0)).clone();
    mediapipe.provider = HandTrackingProvider::MediaPipe;
    mediapipe.space = HandCoordinateSpace::Calibrated {
        generation: 7,
        mirrored: false,
    };
    fixture.tracking.publish(openxr.clone());
    fixture.tracking.publish(Arc::new(mediapipe.clone()));
    let config = HandTrackingComponent::new();
    let now = Instant::now();
    assert_eq!(
        fixture
            .tracking
            .selected_frame(&config, 0, now)
            .unwrap()
            .provider,
        HandTrackingProvider::OpenXr
    );
    assert_eq!(
        fixture
            .tracking
            .selected_frame(&config, 1, now)
            .unwrap()
            .provider,
        HandTrackingProvider::MediaPipe
    );
    assert!(
        fixture
            .tracking
            .selected_frame(&config.clone().source("openxr").unwrap(), 1, now)
            .is_none()
    );
    let mut reordered = (*openxr).clone();
    reordered.sequence = 0;
    reordered.hands[1] = mediapipe.hands[1].clone();
    fixture.tracking.publish(Arc::new(reordered));
    assert!(fixture.tracking.sources[&HandTrackingProvider::OpenXr].hands[1].is_none());
    let mut degenerate = (*openxr).clone();
    degenerate.sequence += 1;
    for joint in &mut degenerate.hands[0].as_mut().unwrap().joints {
        joint.position = [0.0; 3];
    }
    fixture.tracking.publish(Arc::new(degenerate));
    assert_eq!(
        fixture
            .tracking
            .selected_frame(&config, 0, Instant::now())
            .unwrap()
            .provider,
        HandTrackingProvider::MediaPipe,
        "an unusable first provider must not suppress a capable fallback"
    );
    mediapipe.sequence += 1;
    mediapipe.space = HandCoordinateSpace::Calibrated {
        generation: 8,
        mirrored: true,
    };
    fixture.tracking.publish(Arc::new(mediapipe.clone()));
    assert!(
        fixture
            .tracking
            .selected_frame(&config, 1, Instant::now())
            .is_none()
    );
    mediapipe.sequence += 1;
    mediapipe.space = HandCoordinateSpace::Calibrated {
        generation: 8,
        mirrored: false,
    };
    mediapipe.received_at = Instant::now() + Duration::from_secs(5);
    fixture.tracking.publish(Arc::new(mediapipe));
    assert!(
        fixture
            .tracking
            .selected_frame(&config, 1, Instant::now())
            .is_none()
    );
}

#[test]
fn map_generation_rebind_and_component_removal_restore_surviving_bones() {
    let mut fixture = Fixture::reimu(false);
    let frame = fixture.frame([[0.7; 5]; 2], 0.0);
    fixture.tracking.publish(frame);
    fixture.step(0.01);
    fixture
        .systems
        .humanoid_bone_map
        .resolve_gltf(&mut fixture.world, fixture.gltf, false);
    fixture.step(0.01); // retires old pose before rebinding
    fixture.step(0.01);
    fixture.tracking.component_removed(fixture.config);
    fixture.step(0.01);
    assert!(!fixture.tracking.rigs.contains_key(&fixture.config));
    for id in &fixture
        .world
        .get_component_by_id_as::<GLTFComponent>(fixture.gltf)
        .unwrap()
        .armature_joint_transforms
    {
        let transform = fixture
            .world
            .get_component_by_id_as::<TransformComponent>(*id)
            .unwrap();
        let rest = fixture
            .world
            .children_of(*id)
            .iter()
            .find_map(|id| {
                fixture
                    .world
                    .get_component_by_id_as::<BoneRestPoseComponent>(*id)
            })
            .unwrap();
        assert!(same_rotation(transform.transform.rotation, rest.rotation));
    }
}

#[test]
fn helper_parent_uses_current_evaluated_rotation_and_new_driven_parent() {
    let mut fixture = Fixture::reimu(true);
    let binding = fixture.tracking.rigs[&fixture.config].hands[0]
        .as_ref()
        .unwrap();
    let proximal = binding.fingers[1].bones[0].target;
    let intermediate = binding.fingers[1].bones[1].target;
    let old = fixture
        .world
        .get_component_by_id_as::<TransformComponent>(intermediate)
        .unwrap()
        .transform;
    let helper_q = quat_from_axis_angle(vec3_normalize([0.7, 0.1, 0.2]), 1.1);
    let helper = fixture
        .world
        .add_component(TransformComponent::new().with_rotation_quat(helper_q));
    let rest = fixture
        .world
        .add_component(BoneRestPoseComponent::new([0.0; 3], helper_q, [1.0; 3]));
    fixture.world.add_child(helper, rest).unwrap();
    fixture.world.add_child(proximal, helper).unwrap();
    fixture
        .world
        .set_parent(intermediate, Some(helper))
        .unwrap();
    let new_t = quat_rotate_vec3(quat_conjugate(helper_q), old.translation);
    let new_q = quat_mul(quat_conjugate(helper_q), old.rotation);
    let transform = fixture
        .world
        .get_component_by_id_as_mut::<TransformComponent>(intermediate)
        .unwrap();
    transform.transform.translation = new_t;
    transform.transform.rotation = new_q;
    transform.transform.recompute_model();
    let rest_id = *fixture
        .world
        .children_of(intermediate)
        .iter()
        .find(|id| {
            fixture
                .world
                .get_component_by_id_as::<BoneRestPoseComponent>(**id)
                .is_some()
        })
        .unwrap();
    let rest = fixture
        .world
        .get_component_by_id_as_mut::<BoneRestPoseComponent>(rest_id)
        .unwrap();
    rest.translation = new_t;
    rest.rotation = new_q;
    fixture
        .systems
        .transform_changed(&mut fixture.world, &mut fixture.visuals, fixture.root);
    fixture
        .systems
        .humanoid_bone_map
        .resolve_gltf(&mut fixture.world, fixture.gltf, false);
    fixture.step(0.01);
    fixture.step(0.01);
    // An animated helper twist keeps the segment endpoint in place while changing
    // the child's actual parent orientation. Rest-parent inversion would fail.
    let extra_twist = quat_from_axis_angle(unit_direction(old.translation).unwrap(), 0.6);
    let helper_transform = fixture
        .world
        .get_component_by_id_as_mut::<TransformComponent>(helper)
        .unwrap();
    helper_transform.transform.rotation = quat_mul(extra_twist, helper_q);
    helper_transform.transform.recompute_model();
    fixture
        .systems
        .transform_changed(&mut fixture.world, &mut fixture.visuals, helper);
    let frame = fixture.frame([[0.65; 5]; 2], 0.1);
    fixture.tracking.publish(frame.clone());
    fixture.step(0.01);
    fixture.assert_matches(&frame);
}

#[test]
fn identical_names_in_two_avatars_bind_locally_and_retire_independently() {
    let mut fixture = Fixture::reimu(false);
    let (root, second_gltf, second_config, map) = append_reimu(&mut fixture.world, true);
    fixture
        .world
        .get_component_by_id_as_mut::<TransformComponent>(root)
        .unwrap()
        .transform
        .translation[0] += 2.0;
    fixture
        .systems
        .transform_changed(&mut fixture.world, &mut fixture.visuals, root);
    fixture
        .systems
        .humanoid_bone_map
        .register_component(&mut fixture.world, map);
    fixture.tracking.register(second_config);
    fixture.step(0.01);
    let second_ids: HashSet<_> = fixture
        .world
        .get_component_by_id_as::<GLTFComponent>(second_gltf)
        .unwrap()
        .armature_joint_transforms
        .iter()
        .copied()
        .collect();
    let first_ids: HashSet<_> = fixture
        .world
        .get_component_by_id_as::<GLTFComponent>(fixture.gltf)
        .unwrap()
        .armature_joint_transforms
        .iter()
        .copied()
        .collect();
    assert!(first_ids.is_disjoint(&second_ids));
    for hand in fixture.tracking.rigs[&second_config].hands.iter().flatten() {
        assert!(second_ids.contains(&hand.hand));
        for finger in &hand.fingers {
            for bone in &finger.bones {
                assert!(second_ids.contains(&bone.target));
            }
        }
    }
    let frame = fixture.frame([[0.7; 5]; 2], 0.0);
    fixture.tracking.publish(frame);
    fixture.step(0.01);
    fixture.tracking.component_removed(fixture.config);
    fixture.step(0.01);
    assert!(!fixture.tracking.rigs.contains_key(&fixture.config));
    assert!(
        fixture.tracking.rigs[&second_config]
            .hands
            .iter()
            .flatten()
            .flat_map(|hand| &hand.fingers)
            .flat_map(|finger| &finger.bones)
            .all(|bone| bone.pose.tracked)
    );
}

#[test]
fn epoch_space_and_data_source_changes_use_only_the_new_coherent_hand() {
    let mut fixture = Fixture::reimu(false);
    let first = fixture.frame([[0.9; 5]; 2], 0.0);
    fixture.tracking.publish(first.clone());
    fixture.step(0.01);
    let mut restarted = (*fixture.frame([[0.1; 5]; 2], -0.1)).clone();
    restarted.epoch = 2;
    restarted.sequence = 0;
    restarted.space = HandCoordinateSpace::OpenXrStage { generation: 2 };
    for sample in restarted.hands.iter_mut().flatten() {
        sample.data_source = HandDataSource::Controller;
        for joint in &mut sample.joints {
            joint.position = vec3_add(joint.position, [100.0, -50.0, 10.0]);
        }
    }
    fixture.tracking.publish(Arc::new(restarted.clone()));
    fixture.step(0.01);
    fixture.assert_matches(&restarted);
    let status = fixture.tracking.status(fixture.config, 0).unwrap();
    assert_eq!(status.epoch, Some(2));
    assert_eq!(status.data_source, Some(HandDataSource::Controller));
    assert_eq!(status.tracked_fingers, 5);
    fixture.tracking.publish(first); // delayed delivery from the previous session
    assert_eq!(
        fixture.tracking.sources[&HandTrackingProvider::OpenXr].epoch,
        2
    );
    fixture.tracking.set_openxr_frame(None);
    fixture.tracking.publish(Arc::new(restarted.clone())); // clearing is not a replay watermark reset
    assert!(
        !fixture
            .tracking
            .sources
            .contains_key(&HandTrackingProvider::OpenXr)
    );
    restarted.sequence += 1;
    restarted.space = HandCoordinateSpace::OpenXrStage { generation: 3 };
    fixture.tracking.publish(Arc::new(restarted.clone()));
    fixture.step(0.01);
    fixture.assert_matches(&restarted);
}

#[test]
fn empty_armature_releases_owned_rotations_and_rebinds_after_respawn() {
    let mut fixture = Fixture::reimu(false);
    let frame = fixture.frame([[0.8; 5]; 2], 0.0);
    fixture.tracking.publish(frame);
    fixture.step(0.01);
    let old_ids = std::mem::take(
        &mut fixture
            .world
            .get_component_by_id_as_mut::<GLTFComponent>(fixture.gltf)
            .unwrap()
            .armature_joint_transforms,
    );
    fixture.step(0.01);
    assert!(!fixture.tracking.rigs.contains_key(&fixture.config));
    fixture
        .world
        .get_component_by_id_as_mut::<GLTFComponent>(fixture.gltf)
        .unwrap()
        .armature_joint_transforms = old_ids;
    fixture.tracking.set_openxr_frame(None);
    fixture.step(0.01);
    fixture.assert_rest();
}

#[test]
fn component_lifecycle_intents_register_and_unregister_the_hand_system() {
    let mut fixture = Fixture::reimu(false);
    fixture
        .world
        .init_component_tree(fixture.config, &mut fixture.queue);
    fixture.queue.flush(
        &mut fixture.world,
        &mut fixture.systems,
        &mut fixture.visuals,
        &mut fixture.assets,
    );
    assert!(
        fixture
            .systems
            .hand_tracking
            .registrations
            .contains(&fixture.config)
    );
    use crate::engine::ecs::component::Component;
    let component = fixture
        .world
        .get_component_by_id_as_mut::<HandTrackingComponent>(fixture.config)
        .unwrap();
    component.cleanup(&mut fixture.queue, fixture.config);
    fixture.queue.flush(
        &mut fixture.world,
        &mut fixture.systems,
        &mut fixture.visuals,
        &mut fixture.assets,
    );
    assert!(
        !fixture
            .systems
            .hand_tracking
            .registrations
            .contains(&fixture.config)
    );
}
