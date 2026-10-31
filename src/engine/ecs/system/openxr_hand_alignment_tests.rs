use super::*;
use crate::engine::ecs::CommandQueue;
use crate::engine::ecs::component::{
    AvatarControlComponent, BoneRestPoseComponent, GLTFComponent, IKChainComponent, IKSolver,
    TransformComponent,
};
use crate::engine::ecs::system::{
    JointBasisRetargetingSystem, LandmarkDirection, RetargetBasisDefinition, SystemWorld,
};
use crate::engine::graphics::RenderAssets;

#[test]
fn mitten_palm_down_and_roll_survive_retargeting_and_arm_ik() {
    let document = gltf::Gltf::open("assets/models/mitten.glb").unwrap();
    let mut world = World::default();
    let tracking_rotation = math::quat_from_axis_angle([0.0, 1.0, 0.0], 0.7);
    let scene = world.add_component(
        TransformComponent::new()
            .with_position(2.0, 0.1, 3.0)
            .with_rotation_quat(tracking_rotation),
    );
    let avatar = world.add_component(AvatarControlComponent::new());
    let model = world.add_component(
        TransformComponent::new().with_rotation_quat(math::quat_rotation_y(std::f32::consts::PI)),
    );
    let gltf = world.add_component(GLTFComponent::new("assets/models/mitten.glb"));
    world.add_child(scene, avatar).unwrap();
    world.add_child(avatar, model).unwrap();
    world.add_child(model, gltf).unwrap();
    world
        .get_component_by_id_as_mut::<AvatarControlComponent>(avatar)
        .unwrap()
        .model_root_id = Some(model);

    // Import the actual node transforms and immutable rest sidecars without GPU assets.
    let mut nodes = Vec::new();
    for node in document.nodes() {
        let (t, r, s) = node.transform().decomposed();
        let joint = world.add_component(
            TransformComponent::new()
                .with_position(t[0], t[1], t[2])
                .with_rotation_quat(r)
                .with_scale(s[0], s[1], s[2]),
        );
        let rest = world.add_component(BoneRestPoseComponent::new(t, r, s));
        world.add_child(model, joint).unwrap();
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
    let find = |name: &str| {
        nodes[document
            .nodes()
            .find(|node| node.name() == Some(name))
            .unwrap()
            .index()]
    };
    world
        .get_component_by_id_as_mut::<GLTFComponent>(gltf)
        .unwrap()
        .armature_joint_transforms = document
        .skins()
        .next()
        .unwrap()
        .joints()
        .map(|joint| nodes[joint.index()])
        .collect();

    let mut systems = SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let mut retargeting = JointBasisRetargetingSystem::default();
    let mut hands = Vec::new();
    for (side, label, thumb_side) in [
        (ControllerHand::Left, "L", 1.0),
        (ControllerHand::Right, "R", -1.0),
    ] {
        let hand = find(&format!("J_Bip_{label}_Hand"));
        let middle_start = find(&format!("J_Bip_{label}_Middle1"));
        let middle_end = find(&format!("J_Bip_{label}_Middle3"));
        let little = find(&format!("J_Bip_{label}_Little1"));
        let index = find(&format!("J_Bip_{label}_Index1"));
        // Same rest landmarks used by mittens-corp's VRoid humanoid map and AVC.
        retargeting.replace_definition(
            &world,
            hand,
            RetargetBasisDefinition {
                target: hand,
                forward: LandmarkDirection {
                    start: middle_start,
                    end: middle_end,
                },
                up: LandmarkDirection {
                    start: little,
                    end: index,
                },
            },
        );
        let correction = math::mat_to_quat(
            retargeting
                .basis_for(hand)
                .unwrap()
                .target_rest_to_canonical,
        );
        let raw = world.add_component(TransformComponent::new().with_position(
            -0.22 * thumb_side,
            1.3,
            -0.4,
        ));
        let visual = world.add_component(TransformComponent::new().with_rotation_quat(correction));
        world.add_child(scene, raw).unwrap();
        world.add_child(raw, visual).unwrap();
        let mut chain = IKChainComponent::new(
            IKSolver::TwoBoneIK {
                root_joint_id: find(&format!("J_Bip_{label}_UpperArm")),
                mid_joint_id: find(&format!("J_Bip_{label}_LowerArm")),
                pole_direction: [thumb_side, -1.5, 1.0],
                copy_end_rotation: true,
            },
            visual,
            hand,
        );
        if side == ControllerHand::Left {
            chain.forbidden_bend_normal_z_degrees = vec![[-178.0, -115.0], [-100.0, -60.0]];
        }
        let chain = world.add_component(chain);
        world.add_child(avatar, chain).unwrap();
        systems.ik.register(chain);
        hands.push((
            side,
            thumb_side,
            raw,
            middle_start,
            middle_end,
            little,
            index,
        ));
    }

    for roll_degrees in [0.0_f32, 45.0, -45.0, 90.0, -90.0, 0.0] {
        for &(side, thumb_side, raw, ..) in &hands {
            let native = math::quat_from_axis_angle(
                [0.0, 0.0, -1.0],
                (roll_degrees * thumb_side).to_radians(),
            );
            let mut joints = [openxr::HandJointLocationEXT::default(); openxr::HAND_JOINT_COUNT];
            joints[openxr::HandJointEXT::PALM].location_flags =
                openxr::SpaceLocationFlags::ORIENTATION_VALID;
            joints[openxr::HandJointEXT::PALM].pose.orientation = openxr::Quaternionf {
                x: native[0],
                y: native[1],
                z: native[2],
                w: native[3],
            };
            let orientation = OpenXRSystem::canonical_hand_orientation(&joints, side).unwrap();
            let transform = world
                .get_component_by_id_as_mut::<TransformComponent>(raw)
                .unwrap();
            transform.transform.rotation =
                [orientation.x, orientation.y, orientation.z, orientation.w];
            transform.transform.recompute_model();
        }
        systems.transform_changed(&mut world, &mut visuals, scene);
        systems.ik.tick(&mut world, &mut queue, 1.0 / 90.0);
        queue.flush(&mut world, &mut systems, &mut visuals, &mut assets);

        for &(side, thumb_side, _, middle_start, middle_end, little, index) in &hands {
            let position = |id| {
                let model = world
                    .get_component_by_id_as::<TransformComponent>(id)
                    .unwrap()
                    .transform
                    .matrix_world;
                [model[3][0], model[3][1], model[3][2]]
            };
            // Measure the evaluated finger geometry rather than reusing the correction matrix.
            let forward =
                math::vec3_normalize(math::vec3_sub(position(middle_end), position(middle_start)));
            let width = math::vec3_sub(position(index), position(little));
            let normal = math::vec3_scale(
                math::vec3_normalize(math::vec3_cross(width, forward)),
                thumb_side,
            );
            let native = math::quat_from_axis_angle(
                [0.0, 0.0, -1.0],
                (roll_degrees * thumb_side).to_radians(),
            );
            let expected = math::quat_mul(tracking_rotation, native);
            let expected_forward = math::quat_rotate_vec3(expected, [0.0, 0.0, -1.0]);
            let expected_normal = math::quat_rotate_vec3(expected, [0.0, 1.0, 0.0]);
            assert!(
                math::vec3_dot(forward, expected_forward) > 0.9999,
                "{side:?} roll={roll_degrees}: forward={forward:?}"
            );
            assert!(
                math::vec3_dot(normal, expected_normal) > 0.9999,
                "{side:?} roll={roll_degrees}: normal={normal:?}, expected={expected_normal:?}"
            );
        }
    }
}
