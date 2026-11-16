use super::*;

#[test]
fn native_head_neck_body_aligns_mapped_eyes_with_xr_view_on_checked_in_rigs() {
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::system::{HeadNeckBodyConstraintSystem, SystemWorld};
    use crate::engine::graphics::VisualWorld;
    use crate::utils::math::{quat_from_axis_angle, vec3_len, vec3_sub};

    // Check actual skinned body surfaces, not only joint endpoints. A ray
    // exiting a back face first is the first-person "inside torso" failure.
    use super::super::first_person_test_support::body_hit;

    for asset in ["assets/models/bisket.glb", "assets/models/mitten.glb"] {
        let document = gltf::Gltf::open(asset).unwrap();
        let mut world = World::default();
        let mut xr = InputXRComponent::on();
        xr.pose_valid = true;
        let input = world.add_component(xr);
        let driven =
            world.add_component(TransformComponent::new().with_position(0.0, 1.6, 0.0));
        let mut config = AvatarControlComponent::new();
        config.collision_enabled = false;
        let avatar = world.add_component(config);
        let root = world.add_component(TransformComponent::new());
        for (p, c) in [(input, driven), (driven, avatar), (avatar, root)] {
            world.add_child(p, c).unwrap();
        }
        let mut nodes = Vec::new();
        for node in document.nodes() {
            let (t, r, s) = node.transform().decomposed();
            let id = world.add_component(
                TransformComponent::new()
                    .with_position(t[0], t[1], t[2])
                    .with_rotation_quat(r)
                    .with_scale(s[0], s[1], s[2]),
            );
            world.add_child(root, id).unwrap();
            let rest = world.add_component(BoneRestPoseComponent::new(t, r, s));
            world.add_child(id, rest).unwrap();
            nodes.push(id);
        }
        for node in document.nodes() {
            for child in node.children() {
                world
                    .set_parent(nodes[child.index()], Some(nodes[node.index()]))
                    .unwrap();
            }
        }
        let find = |name| {
            nodes[document
                .nodes()
                .find(|n| n.name() == Some(name))
                .unwrap()
                .index()]
        };
        let head = find("J_Bip_C_Head");
        let neck = find("J_Bip_C_Neck");
        let left = rest_model_relative_to(&world, head, find("J_Adj_L_FaceEye")).unwrap();
        let right = rest_model_relative_to(&world, head, find("J_Adj_R_FaceEye")).unwrap();
        let eye = std::array::from_fn::<_, 3, _>(|i| (left[3][i] + right[3][i]) * 0.5);
        let anchor = world
            .add_component(TransformComponent::new().with_position(eye[0], eye[1], eye[2]));
        world.add_child(head, anchor).unwrap();
        // Same authored wrapper as mitten.mms / mittens-corp.mms.
        let wrapper =
            world.add_component(TransformComponent::new().with_position(0.0, 0.08, 0.12));
        let camera = world.add_component(CameraXRComponent::on());
        world.add_child(avatar, wrapper).unwrap();
        world.add_child(wrapper, camera).unwrap();
        let mut report = left_arm_report(head, head, head);
        report.slots.clear();
        for (slot, component) in [
            (HumanoidSlot::Head, head),
            (HumanoidSlot::Neck, neck),
            (HumanoidSlot::CameraAnchor, anchor),
        ] {
            report.slots.insert(
                slot,
                HumanoidSlotReport {
                    slot,
                    status: HumanoidSlotStatus::Resolved(ResolvedHumanoidTarget {
                        component,
                        kind: ResolvedHumanoidTargetKind::Transform,
                    }),
                    provenance: HumanoidSlotProvenance::ConventionName,
                    diagnostic: None,
                },
            );
        }
        let mut queue = CommandQueue::new();
        try_init_pose_drivers(
            avatar,
            &mut world,
            &mut Default::default(),
            &report,
            &mut queue,
        );
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        queue.flush(&mut world, &mut systems, &mut visuals, &mut assets);
        let mut constraint = HeadNeckBodyConstraintSystem::default();
        constraint.register(avatar);
        let mut visible_body_hits = 0;
        for (pitch, roll) in [
            (0_f32, 0_f32),
            (50.0, 0.0),
            (90.0, 0.0),
            (0.0, 45.0),
            (50.0, -35.0),
        ] {
            let t = world
                .get_component_by_id_as_mut::<TransformComponent>(driven)
                .unwrap();
            t.transform.rotation = quat_mul(
                quat_rotation_y(0.3),
                quat_mul(
                    quat_from_axis_angle([-1.0, 0.0, 0.0], pitch.to_radians()),
                    quat_from_axis_angle([0.0, 0.0, -1.0], roll.to_radians()),
                ),
            );
            t.transform.recompute_model();
            systems.transform_changed(&mut world, &mut visuals, driven);
            constraint.tick(&mut world, &mut queue, &systems.attachment);
            queue.flush(&mut world, &mut systems, &mut visuals, &mut assets);
            let view = world
                .get_component_by_id_as::<TransformComponent>(driven)
                .unwrap()
                .transform
                .matrix_world;
            let eye_world = world
                .get_component_by_id_as::<TransformComponent>(wrapper)
                .unwrap()
                .transform
                .matrix_world;
            let delta = vec3_sub(
                eye_world[3][..3].try_into().unwrap(),
                view[3][..3].try_into().unwrap(),
            );
            assert!(
                vec3_len(delta) < 0.00003,
                "{asset}, pitch {pitch}, roll {roll}: eye/view mismatch {delta:?}"
            );
            // The rendered head/view source must not be shifted by this solve.
            assert_eq!(view[3], [0.0, 1.6, 0.0, 1.0]);
            for y in [0.0, -0.2, -0.4, -0.6] {
                for x in [-0.1, 0.0, 0.1] {
                    if let Some((distance, facing)) =
                        body_hit(&document, &world, &nodes, view, [x, y, -1.0])
                    {
                        visible_body_hits += 1;
                        assert!(
                            facing < 0.0,
                            "{asset}, pitch {pitch}, roll {roll}: view enters a body back face"
                        );
                        assert!(
                            distance >= CameraXRComponent::DEFAULT_Z_NEAR,
                            "{asset}, pitch {pitch}, roll {roll}: nearest body surface at {distance} is clipped"
                        );
                    }
                }
            }
            assert_eq!(constraint.status(avatar), super::super::super::ik::head_neck_body_constraint::HeadNeckBodyConstraintStatus::Ready);
        }
        assert!(
            visible_body_hits > 0,
            "fixture rays must exercise visible body surfaces"
        );
    }
}

#[test]
fn head_binding_splices_both_desktop_and_xr_before_shared_body_solving() {
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::system::{HeadNeckBodyConstraintSystem, SystemWorld};
    use crate::engine::graphics::VisualWorld;
    for xr in [false, true] {
        let mut world = World::default();
        let driven =
            world.add_component(TransformComponent::new().with_position(0.0, 1.42, 0.0));
        let input = if xr {
            let mut source = InputXRComponent::on();
            source.pose_valid = true;
            world.add_component(source)
        } else {
            world.add_component(InputComponent::new())
        };
        world.add_child(input, driven).unwrap();
        let mut config = AvatarControlComponent::new();
        config.collision_enabled = false;
        let avatar = world.add_component(config);
        let root = world.add_component(TransformComponent::new());
        let neck = world.add_component(TransformComponent::new().with_position(0.0, 1.3, 0.0));
        let head = world.add_component(TransformComponent::new().with_position(0.0, 0.12, 0.0));
        for (p, c) in [(driven, avatar), (avatar, root), (root, neck), (neck, head)] {
            world.add_child(p, c).unwrap();
        }
        let mut report = left_arm_report(head, head, head);
        report.slots.clear();
        for (slot, component) in [(HumanoidSlot::Head, head), (HumanoidSlot::Neck, neck)] {
            report.slots.insert(
                slot,
                HumanoidSlotReport {
                    slot,
                    status: HumanoidSlotStatus::Resolved(ResolvedHumanoidTarget {
                        component,
                        kind: ResolvedHumanoidTargetKind::SkinJoint,
                    }),
                    provenance: HumanoidSlotProvenance::ConventionName,
                    diagnostic: None,
                },
            );
        }
        let mut queue = CommandQueue::new();
        try_init_pose_drivers(
            avatar,
            &mut world,
            &mut Default::default(),
            &report,
            &mut queue,
        );
        assert!(
            world
                .get_component_by_id_as::<AvatarControlComponent>(avatar)
                .unwrap()
                .head_neck_body_constraint
                .is_some()
        );
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        queue.flush(&mut world, &mut systems, &mut visuals, &mut assets);
        assert_ne!(world.parent_of(head), Some(neck));
        assert_eq!(
            world.parent_of(head),
            world
                .get_component_by_id_as::<AvatarControlComponent>(avatar)
                .unwrap()
                .head_mount
        );
        let driven_t = world
            .get_component_by_id_as_mut::<TransformComponent>(driven)
            .unwrap();
        driven_t.transform.rotation =
            crate::utils::math::quat_from_axis_angle([-1.0, 0.0, 0.0], 50_f32.to_radians());
        driven_t.transform.recompute_model();
        systems.transform_changed(&mut world, &mut visuals, driven);
        let mut constraint = HeadNeckBodyConstraintSystem::default();
        constraint.register(avatar);
        constraint.tick(&mut world, &mut queue, &systems.attachment);
        queue.flush(&mut world, &mut systems, &mut visuals, &mut assets);
        let neck_world = world
            .get_component_by_id_as::<TransformComponent>(neck)
            .unwrap()
            .transform
            .matrix_world;
        let endpoint = crate::utils::math::mat4_mul_vec4(neck_world, [0.0, 0.12, 0.0, 1.0]);
        let head_world = world
            .get_component_by_id_as::<TransformComponent>(head)
            .unwrap()
            .transform
            .matrix_world;
        for axis in 0..3 {
            assert!((endpoint[axis] - head_world[3][axis]).abs() < 0.00003);
        }
        assert_eq!(
            constraint.status(avatar),
            super::super::super::ik::head_neck_body_constraint::HeadNeckBodyConstraintStatus::Ready
        );
    }
}
