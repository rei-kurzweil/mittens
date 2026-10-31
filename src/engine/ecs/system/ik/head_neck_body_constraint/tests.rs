use super::*;
use crate::engine::ecs::CommandQueue;
use crate::engine::ecs::component::transform_operator::TransformQuatOp;
use crate::engine::ecs::component::{BoneRestPoseComponent, TransformOperatorComponent};
use crate::engine::ecs::system::SystemWorld;
use crate::engine::graphics::{RenderAssets, VisualWorld};

struct Fixture {
    world: World,
    systems: SystemWorld,
    visuals: VisualWorld,
    assets: RenderAssets,
    queue: CommandQueue,
    input: ComponentId,
    frame: ComponentId,
    driven: ComponentId,
    avatar: ComponentId,
    root: ComponentId,
    neck: ComponentId,
    head: ComponentId,
    mount: ComponentId,
}

fn close(a: [f32; 3], b: [f32; 3]) {
    assert!(vec3_len(vec3_sub(a, b)) < 0.00003, "{a:?} != {b:?}");
}

impl Fixture {
    fn new(desktop: bool) -> Self {
        let mut world = World::default();
        let frame = world.add_component(TransformComponent::new());
        let input = if desktop {
            world.add_component(InputComponent::new())
        } else {
            let mut xr = InputXRComponent::on();
            xr.pose_valid = true;
            world.add_component(xr)
        };
        let driven = world.add_component(TransformComponent::new().with_position(0.02, 1.42, 0.03));
        let mut avatar = AvatarControlComponent::new();
        avatar.collision_enabled = false;
        avatar.initial_body_yaw = 0.0;
        let avatar = world.add_component(avatar);
        let mut operator = TransformOperatorComponent::fork_trs();
        operator.push_rotation(TransformQuatOp::YawFollow {
            threshold: std::f32::consts::PI,
            rate: 0.0,
            initial_yaw: 0.0,
            forward_plus_z: true,
        });
        let operator = world.add_component(operator);
        let root = world.add_component(TransformComponent::new());
        let parent = world.add_component(TransformComponent::new().with_position(0.02, 1.30, 0.03));
        let neck = world.add_component(TransformComponent::new());
        let head = world.add_component(TransformComponent::new().with_position(0.0, 0.12, 0.0));
        for (p, c) in [
            (frame, input),
            (input, driven),
            (driven, avatar),
            (avatar, operator),
            (operator, root),
            (root, parent),
            (parent, neck),
            (neck, head),
        ] {
            world.add_child(p, c).unwrap();
        }
        let rest = world.add_component(BoneRestPoseComponent::new(
            [0.0, 0.12, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0; 3],
        ));
        world.add_child(head, rest).unwrap();
        let constraint = HeadNeckBodyConstraint::bind(
            &world,
            root,
            neck,
            head,
            driven,
            [0.0; 3],
            [0.0, 0.0, 0.0, 1.0],
            0,
        )
        .unwrap();
        let mount = world.add_component(TransformComponent::new());
        world.add_child(driven, mount).unwrap();
        world.set_parent(head, Some(mount)).unwrap();
        {
            let t = world
                .get_component_by_id_as_mut::<TransformComponent>(head)
                .unwrap();
            t.transform.translation = [0.0; 3];
            t.transform.recompute_model();
        }
        {
            let avc = world
                .get_component_by_id_as_mut::<AvatarControlComponent>(avatar)
                .unwrap();
            avc.head_mount = Some(mount);
            avc.displaced_head = Some(head);
            avc.model_root_id = Some(root);
            avc.body_operator_id = Some(operator);
            avc.neck_bone_id = Some(neck);
            avc.neck_rest_translation = Some([0.0; 3]);
            avc.head_neck_body_constraint = Some(constraint);
        }
        let mut systems = SystemWorld::default();
        systems.head_neck_body_constraint.register(avatar);
        Self {
            world,
            systems,
            visuals: VisualWorld::default(),
            assets: RenderAssets::new(),
            queue: CommandQueue::new(),
            input,
            frame,
            driven,
            avatar,
            root,
            neck,
            head,
            mount,
        }
    }

    fn pose(&mut self, position: [f32; 3], rotation: [f32; 4]) {
        let driven = self
            .world
            .get_component_by_id_as_mut::<TransformComponent>(self.driven)
            .unwrap();
        driven.transform.translation = position;
        driven.transform.rotation = rotation;
        driven.transform.recompute_model();
        self.systems
            .transform_changed(&mut self.world, &mut self.visuals, self.frame);
        self.step();
    }

    fn step(&mut self) {
        self.systems.head_neck_body_constraint.tick(
            &mut self.world,
            &mut self.queue,
            &self.systems.attachment,
        );
        self.queue.flush(
            &mut self.world,
            &mut self.systems,
            &mut self.visuals,
            &mut self.assets,
        );
    }

    fn position(&self, id: ComponentId) -> [f32; 3] {
        matrix(&self.world, id).unwrap()[3][..3].try_into().unwrap()
    }

    fn assert_closed(&self) {
        let rest = &self
            .world
            .get_component_by_id_as::<AvatarControlComponent>(self.avatar)
            .unwrap()
            .head_neck_body_constraint
            .as_ref()
            .unwrap()
            .neck_to_head;
        close(
            point(matrix(&self.world, self.neck).unwrap(), *rest),
            self.position(self.head),
        );
        assert_eq!(
            self.systems.head_neck_body_constraint.status(self.avatar),
            HeadNeckBodyConstraintStatus::Ready
        );
    }
}

#[test]
fn xr_pitch_closes_the_neck_and_moves_body_from_rest_without_accumulation() {
    let mut f = Fixture::new(false);
    let head = [0.02, 1.42, 0.03];
    f.pose(head, [0.0, 0.0, 0.0, 1.0]);
    close(f.position(f.root), [0.0; 3]);
    f.assert_closed();
    let requested = quat_from_axis_angle([1.0, 0.0, 0.0], 50_f32.to_radians());
    f.pose(head, requested);
    close(f.position(f.root), [0.0, 0.011243066, -0.05071419]);
    close(f.position(f.head), head);
    f.assert_closed();
    let body = f.position(f.root);
    for _ in 0..120 {
        f.pose(head, requested);
        f.assert_closed();
    }
    close(f.position(f.root), body);
    let translated = vec3_add(head, [0.4, -0.2, 0.3]);
    f.pose(translated, requested);
    close(f.position(f.root), vec3_add(body, [0.4, -0.2, 0.3]));
    f.assert_closed();
}

#[test]
fn desktop_look_derives_head_arc_without_translating_body_or_input() {
    let mut f = Fixture::new(true);
    let input_position = [0.02, 1.42, 0.03];
    f.pose(input_position, [0.0, 0.0, 0.0, 1.0]);
    let anchor = f.position(f.root);
    let neutral_head = f.position(f.head);
    for degrees in [-80_f32, -30.0, 0.0, 30.0, 50.0, 80.0, 0.0] {
        let q = quat_from_axis_angle([1.0, 0.0, 0.0], degrees.to_radians());
        f.pose(input_position, q);
        close(f.position(f.root), anchor);
        close(f.position(f.driven), input_position);
        f.assert_closed();
        if degrees == 50.0 {
            close(
                f.position(f.head),
                vec3_add(neutral_head, [0.0, -0.011243066, 0.05071419]),
            );
        }
    }
    let next = vec3_add(input_position, [0.3, 0.0, -0.2]);
    f.pose(next, quat_from_axis_angle([1.0, 0.0, 0.0], 0.7));
    close(f.position(f.root), vec3_add(anchor, [0.3, 0.0, -0.2]));
    f.assert_closed();
}

#[test]
fn rotation_scale_and_nonidentity_joint_rest_frames_keep_endpoint_connected() {
    let mut f = Fixture::new(false);
    {
        let frame = f
            .world
            .get_component_by_id_as_mut::<TransformComponent>(f.frame)
            .unwrap();
        frame.transform.rotation = crate::utils::math::quat_rotation_y(0.8);
        frame.transform.scale = [1.7; 3];
        frame.transform.recompute_model();
    }
    let neck_parent = f.world.parent_of(f.neck).unwrap();
    let rest_q = quat_from_axis_angle([0.0, 0.0, 1.0], 0.3);
    {
        let parent = f
            .world
            .get_component_by_id_as_mut::<TransformComponent>(neck_parent)
            .unwrap();
        parent.transform.rotation = crate::utils::math::quat_rotation_y(-0.4);
        parent.transform.recompute_model();
        let constraint = f
            .world
            .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
            .unwrap()
            .head_neck_body_constraint
            .as_mut()
            .unwrap();
        constraint.neck_rest.rotation_quat_xyzw = rest_q;
    }
    for degrees in [-80_f32, -30.0, 0.0, 45.0, 80.0, 90.0] {
        f.pose(
            [1.0, 1.42, -0.2],
            quat_from_axis_angle([1.0, 0.0, 0.0], degrees.to_radians()),
        );
        f.assert_closed();
    }
}

#[test]
fn invalid_tracking_does_not_solve_and_reacquisition_uses_current_pose() {
    let mut f = Fixture::new(false);
    f.pose(
        [0.02, 1.42, 0.03],
        quat_from_axis_angle([1.0, 0.0, 0.0], 0.8),
    );
    let before = f.position(f.root);
    f.world
        .get_component_by_id_as_mut::<InputXRComponent>(f.input)
        .unwrap()
        .pose_valid = false;
    f.step();
    assert_eq!(
        f.systems.head_neck_body_constraint.status(f.avatar),
        HeadNeckBodyConstraintStatus::Pending
    );
    close(f.position(f.root), before);
    f.world
        .get_component_by_id_as_mut::<InputXRComponent>(f.input)
        .unwrap()
        .pose_valid = true;
    f.pose(
        [1.0, 1.0, -0.3],
        quat_from_axis_angle([1.0, 0.0, 0.0], -0.8),
    );
    f.assert_closed();
}

#[test]
fn opt_out_restores_neck_and_desktop_mount_then_uses_legacy_fallback() {
    let mut f = Fixture::new(true);
    f.pose(
        [0.02, 1.42, 0.03],
        quat_from_axis_angle([1.0, 0.0, 0.0], 0.8),
    );
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .head_neck_body_enabled = false;
    f.step();
    assert_eq!(
        f.systems.head_neck_body_constraint.status(f.avatar),
        HeadNeckBodyConstraintStatus::Disabled
    );
    let neck = &f
        .world
        .get_component_by_id_as::<TransformComponent>(f.neck)
        .unwrap()
        .transform;
    assert_eq!(neck.rotation, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(
        f.world
            .get_component_by_id_as::<TransformComponent>(f.mount)
            .unwrap()
            .transform
            .translation,
        [0.0; 3]
    );
    f.world
        .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .head_neck_body_enabled = true;
    f.step();
    f.assert_closed();
}

#[test]
fn xr_roll_moves_body_sideways_but_yaw_alone_does_not_bend_the_neck() {
    let mut f = Fixture::new(false);
    let head = [0.02, 1.42, 0.03];
    for degrees in [-170_f32, -90.0, 0.0, 90.0, 170.0] {
        f.pose(
            head,
            crate::utils::math::quat_rotation_y(degrees.to_radians()),
        );
        close(f.position(f.root), [0.0; 3]);
        f.assert_closed();
    }
    for sign in [-1.0, 1.0] {
        let roll = quat_from_axis_angle([0.0, 0.0, 1.0], sign * 50_f32.to_radians());
        f.pose(head, roll);
        close(f.position(f.root), [sign * 0.05071419, 0.011243066, 0.0]);
        close(f.position(f.head), head);
        f.assert_closed();
        // The bend follows the head's heading without applying yaw to the neck.
        f.pose(
            head,
            quat_mul(
                crate::utils::math::quat_rotation_y(std::f32::consts::FRAC_PI_2),
                roll,
            ),
        );
        close(f.position(f.root), [0.0, 0.011243066, -sign * 0.05071419]);
        f.assert_closed();
    }
}

#[test]
fn combined_tilt_is_bounded_and_quaternion_sign_independent() {
    let mut f = Fixture::new(false);
    let head = [0.02, 1.42, 0.03];
    for pitch in [-90_f32, -35.0, 0.0, 50.0, 90.0] {
        for roll in [-80_f32, -40.0, 0.0, 40.0, 80.0] {
            let q = quat_mul(
                crate::utils::math::quat_rotation_y(0.7),
                quat_mul(
                    quat_from_axis_angle([1.0, 0.0, 0.0], pitch.to_radians()),
                    quat_from_axis_angle([0.0, 0.0, 1.0], roll.to_radians()),
                ),
            );
            f.pose(head, q);
            f.assert_closed();
            let p = f.position(f.root);
            f.pose(head, q.map(|v| -v));
            close(f.position(f.root), p);
            f.assert_closed();
        }
    }
    let mut desktop = Fixture::new(true);
    for axis in [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]] {
        desktop.pose(head, quat_from_axis_angle(axis, 0.8));
        close(desktop.position(desktop.root), [0.0; 3]);
        desktop.assert_closed();
    }
}

#[test]
fn neck_configuration_rejects_nonfinite_and_out_of_range_values() {
    for args in [
        (f32::NAN, 0.2, 0.3),
        (1.1, 0.2, 0.3),
        (0.5, -0.1, 0.3),
        (0.5, 0.2, 2.0),
    ] {
        assert!(
            AvatarControlComponent::new()
                .with_neck_pitch(args.0, args.1, args.2)
                .is_err()
        );
    }
    for (share, limit) in [
        (f32::NAN, 0.2),
        (1.1, 0.2),
        (0.5, -0.1),
        (0.5, f32::INFINITY),
    ] {
        assert!(
            AvatarControlComponent::new()
                .with_neck_roll(share, limit)
                .is_err()
        );
    }
}

#[test]
fn mms_can_select_legacy_follow_and_configure_native_pitch_limits() {
    use crate::engine::ecs::RxWorld;
    use crate::scripting::runner::RuntimeSpecSession;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let (_session, _intents) = RuntimeSpecSession::start(
        "AVC\n    .head_neck_body_enabled(false)\n    .neck_pitch(0.4, 0.2, 0.3)\n    .neck_roll(0.6, 0.35)",
        &mut world,
        &mut rx,
        None,
        &mut queue,
    )
    .unwrap();
    let avatar = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<AvatarControlComponent>(id))
        .unwrap();
    assert!(!avatar.head_neck_body_enabled);
    assert_eq!(avatar.neck_pitch_share, 0.4);
    assert_eq!(avatar.neck_extension_limit, 0.2);
    assert_eq!(avatar.neck_flexion_limit, 0.3);
    assert_eq!(avatar.neck_roll_share, 0.6);
    assert_eq!(avatar.neck_lateral_limit, 0.35);
}

#[test]
fn desktop_to_xr_rebind_does_not_reuse_the_generated_head_arc() {
    let mut f = Fixture::new(true);
    let rotation = quat_from_axis_angle([1.0, 0.0, 0.0], 0.8);
    f.pose([0.02, 1.42, 0.03], rotation);
    let mut xr = InputXRComponent::on();
    xr.pose_valid = true;
    let xr = f.world.add_component(xr);
    f.world.add_child(f.frame, xr).unwrap();
    f.world.set_parent(f.driven, Some(xr)).unwrap();
    let measured = [0.7, 1.6, -0.3];
    f.pose(measured, rotation);
    close(f.position(f.head), measured);
    f.assert_closed();
    assert_eq!(
        f.world
            .get_component_by_id_as::<TransformComponent>(f.mount)
            .unwrap()
            .transform
            .translation,
        [0.0; 3]
    );
}

#[test]
fn changed_rig_ancestry_and_nonuniform_scale_use_diagnosed_fallback() {
    for reparent in [false, true] {
        let mut f = Fixture::new(false);
        f.pose(
            [0.02, 1.42, 0.03],
            quat_from_axis_angle([1.0, 0.0, 0.0], 0.8),
        );
        if reparent {
            let parent = f.world.parent_of(f.neck).unwrap();
            let outside = f.world.add_component(TransformComponent::new());
            f.world.set_parent(parent, Some(outside)).unwrap();
        } else {
            let root = f
                .world
                .get_component_by_id_as_mut::<TransformComponent>(f.root)
                .unwrap();
            root.transform.scale = [1.0, 1.2, 1.0];
            root.transform.recompute_model();
            f.systems
                .transform_changed(&mut f.world, &mut f.visuals, f.frame);
        }
        f.step();
        assert!(matches!(
            f.systems.head_neck_body_constraint.status(f.avatar),
            HeadNeckBodyConstraintStatus::UnsupportedRig(_)
        ));
    }
}

#[test]
fn default_negative_z_source_and_pi_body_yaw_bend_in_the_correct_direction() {
    let mut f = Fixture::new(false);
    let body = f
        .world
        .get_component_by_id_as::<AvatarControlComponent>(f.avatar)
        .unwrap()
        .body_operator_id
        .unwrap();
    let mut operator = TransformOperatorComponent::fork_trs();
    operator.push_rotation(TransformQuatOp::YawFollow {
        threshold: std::f32::consts::PI,
        rate: 0.0,
        initial_yaw: std::f32::consts::PI,
        forward_plus_z: false,
    });
    *f.world
        .get_component_by_id_as_mut::<TransformOperatorComponent>(body)
        .unwrap() = operator;
    let mount = f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.mount)
        .unwrap();
    mount.transform.rotation = crate::utils::math::quat_rotation_y(std::f32::consts::PI);
    mount.transform.recompute_model();
    f.pose(
        [0.0, 1.42, 0.0],
        quat_from_axis_angle([-1.0, 0.0, 0.0], 50_f32.to_radians()),
    );
    f.assert_closed();
    let p = f.position(f.root);
    close(p, [0.02, 0.011243066, 0.03 + 0.05071419]);
}

#[test]
fn checked_in_bisket_and_mitten_rest_paths_bind_and_close() {
    for asset in ["assets/models/bisket.glb", "assets/models/mitten.glb"] {
        let document = gltf::Gltf::open(asset).unwrap();
        let mut f = Fixture::new(false);
        let mut nodes = Vec::new();
        let mut head = None;
        let mut neck = None;
        for node in document.nodes() {
            let (t, r, s) = node.transform().decomposed();
            let id = f.world.add_component(
                TransformComponent::new()
                    .with_position(t[0], t[1], t[2])
                    .with_rotation_quat(r)
                    .with_scale(s[0], s[1], s[2]),
            );
            let rest = f.world.add_component(BoneRestPoseComponent::new(t, r, s));
            f.world.add_child(id, rest).unwrap();
            f.world.add_child(f.root, id).unwrap();
            if node.name() == Some("J_Bip_C_Head") {
                head = Some(id);
            }
            if node.name() == Some("J_Bip_C_Neck") {
                neck = Some(id);
            }
            nodes.push(id);
        }
        for node in document.nodes() {
            for child in node.children() {
                f.world
                    .set_parent(nodes[child.index()], Some(nodes[node.index()]))
                    .unwrap();
            }
        }
        let head = head.expect("asset head fixture");
        let neck = neck.expect("asset neck fixture");
        let constraint = HeadNeckBodyConstraint::bind(
            &f.world,
            f.root,
            neck,
            head,
            f.driven,
            [0.0; 3],
            [0.0, 0.0, 0.0, 1.0],
            0,
        )
        .unwrap_or_else(|e| panic!("{asset}: {e}"));
        f.world.set_parent(head, Some(f.mount)).unwrap();
        let t = f
            .world
            .get_component_by_id_as_mut::<TransformComponent>(head)
            .unwrap();
        t.transform.translation = [0.0; 3];
        t.transform.recompute_model();
        f.head = head;
        f.neck = neck;
        let avatar = f
            .world
            .get_component_by_id_as_mut::<AvatarControlComponent>(f.avatar)
            .unwrap();
        avatar.displaced_head = Some(head);
        avatar.neck_bone_id = Some(neck);
        avatar.head_neck_body_constraint = Some(constraint);
        for degrees in [-60_f32, 0.0, 30.0, 60.0, 80.0] {
            f.pose(
                [0.3, 1.6, -0.2],
                quat_from_axis_angle([1.0, 0.0, 0.0], degrees.to_radians()),
            );
            f.assert_closed();
        }
    }
}

#[test]
fn avatar_stage_propagates_new_shoulders_before_arm_ik() {
    use crate::engine::ecs::component::{IKChainComponent, IKSolver};
    let mut f = Fixture::new(false);
    f.systems.avatar_control.register(f.avatar);
    let upper = f
        .world
        .add_component(TransformComponent::new().with_position(0.15, 1.1, 0.0));
    let lower = f
        .world
        .add_component(TransformComponent::new().with_position(0.3, 0.0, 0.0));
    let hand = f
        .world
        .add_component(TransformComponent::new().with_position(0.3, 0.0, 0.0));
    let target = f
        .world
        .add_component(TransformComponent::new().with_position(0.4, 1.25, -0.2));
    f.world.add_child(f.root, upper).unwrap();
    f.world.add_child(upper, lower).unwrap();
    f.world.add_child(lower, hand).unwrap();
    let chain = f.world.add_component(IKChainComponent::new(
        IKSolver::TwoBoneIK {
            root_joint_id: upper,
            mid_joint_id: lower,
            pole_direction: [0.0, -1.0, -1.0],
            copy_end_rotation: true,
        },
        target,
        hand,
    ));
    f.world.add_child(upper, chain).unwrap();
    f.systems.ik.register(chain);
    f.systems
        .transform_changed(&mut f.world, &mut f.visuals, target);
    let driven = f
        .world
        .get_component_by_id_as_mut::<TransformComponent>(f.driven)
        .unwrap();
    driven.transform.rotation = quat_from_axis_angle([1.0, 0.0, 0.0], 60_f32.to_radians());
    driven.transform.recompute_model();
    f.systems
        .transform_changed(&mut f.world, &mut f.visuals, f.frame);
    f.systems.tick(
        &mut f.world,
        &mut f.visuals,
        &mut f.assets,
        &Default::default(),
        &mut f.queue,
        1.0 / 60.0,
    );
    f.assert_closed();
    close(f.position(hand), f.position(target));
}
