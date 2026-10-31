#[path = "openxr_tracked_frame.rs"]
mod tracked_frame;

use super::hand_tracking::{
    HandCoordinateSpace, HandDataSource, HandJoint, HandJointLocation, HandOrientationProvenance,
    HandSampleTime, HandTrackingFrame, HandTrackingProvider, TrackedHand,
};
use crate::engine::ecs::component::CameraXRComponent;
use crate::engine::ecs::component::{
    ControllerHand, ControllerPoseKind, ControllerPoseSource, InputXRComponent, XRHandComponent,
    XrComponent,
};
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::vr_types::{XrGamepadState, XrHandGamepadState, XrInputState};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::CameraData;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::VulkanoRenderer;
use crate::engine::graphics::XRSwapchain;
use crate::engine::graphics::XrVulkanGraphics;
use crate::engine::graphics::xr_renderer;
use crate::engine::user_input::InputState;
use crate::engine::vr_perf::{
    VrPerfCollector, VrPerfConfig, VrPerfFrameCpu, VrPerfMetadata, VrPerfPreXrCpu,
};
use crate::utils::math;

use ash::vk::Handle as _;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

fn openxr_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CAT_OPENXR_DEBUG")
            .ok()
            .map(|s| {
                let s = s.trim().to_ascii_lowercase();
                s == "1" || s == "true" || s == "on" || s == "yes"
            })
            .unwrap_or(false)
    })
}

#[cfg(test)]
#[path = "openxr_hand_alignment_tests.rs"]
mod hand_alignment_tests;

#[cfg(test)]
#[path = "openxr_avatar_free_pointer_tests.rs"]
mod avatar_free_pointer_tests;

#[cfg(test)]
mod grip_aim_tests {
    use super::*;

    fn pose(position: [f32; 3], rotation: [f32; 4], generation: u64) -> ControllerPoseSample {
        ControllerPoseSample {
            pose: openxr::Posef {
                orientation: openxr::Quaternionf {
                    x: rotation[0],
                    y: rotation[1],
                    z: rotation[2],
                    w: rotation[3],
                },
                position: openxr::Vector3f {
                    x: position[0],
                    y: position[1],
                    z: position[2],
                },
            },
            generation,
        }
    }

    #[test]
    fn grip_aim_uses_grip_translation_and_aim_rotation_from_one_frame() {
        let grip = pose([1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0], 7);
        let aim = pose([9.0, 8.0, 7.0], [0.1, 0.2, 0.3, 0.9], 7);
        let combined = grip_aim_pose(Some(grip), Some(aim)).unwrap();
        assert_eq!(
            [
                combined.position.x,
                combined.position.y,
                combined.position.z
            ],
            [1.0, 2.0, 3.0]
        );
        assert_eq!(
            [
                combined.orientation.x,
                combined.orientation.y,
                combined.orientation.z,
                combined.orientation.w,
            ],
            [0.1, 0.2, 0.3, 0.9]
        );
        assert!(grip_aim_pose(None, Some(aim)).is_none());
        assert!(grip_aim_pose(Some(grip), None).is_none());
        assert!(grip_aim_pose(Some(grip), Some(pose([0.0; 3], [0.0, 0.0, 0.0, 1.0], 8))).is_none());
    }

    #[test]
    fn controller_pose_wins_for_controller_derived_or_unknown_hand_joints() {
        let controller = pose([1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0], 1).pose;
        let hand_root = pose([4.0, 5.0, 6.0], [0.0, 0.0, 0.0, 1.0], 1).pose;

        for data_source in [None, Some(openxr::HandTrackingDataSourceEXT::CONTROLLER)] {
            let (selected, source) = select_hand_pose(
                Some(controller),
                ControllerPoseSource::ControllerGripAim,
                Some(hand_root),
                data_source,
            );
            assert_eq!(source, ControllerPoseSource::ControllerGripAim);
            assert_eq!(selected.unwrap().position.x, 1.0);
        }

        let (selected, source) = select_hand_pose(
            None,
            ControllerPoseSource::ControllerGripAim,
            Some(hand_root),
            Some(openxr::HandTrackingDataSourceEXT::CONTROLLER),
        );
        assert_eq!(source, ControllerPoseSource::WristPalm);
        assert_eq!(selected.unwrap().position.x, 4.0);
    }

    #[test]
    fn unobstructed_hand_pose_wins_over_emulated_controller_actions() {
        let controller = pose([1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0], 1).pose;
        let hand_root = pose([4.0, 5.0, 6.0], [0.0, 0.0, 1.0, 0.0], 1).pose;
        for action_source in [
            ControllerPoseSource::ControllerAim,
            ControllerPoseSource::ControllerGrip,
            ControllerPoseSource::ControllerGripAim,
        ] {
            // SteamVR can keep actions active under a controller profile while its
            // hand joints come from an unobstructed hand-tracking device.
            let (selected, source) = select_hand_pose(
                Some(controller),
                action_source,
                Some(hand_root),
                Some(openxr::HandTrackingDataSourceEXT::UNOBSTRUCTED),
            );
            assert_eq!(source, ControllerPoseSource::WristPalm);
            let selected = selected.unwrap();
            assert_eq!(selected.position.x, 4.0);
            assert_eq!(selected.orientation.z, 1.0);

            // A provider classification alone cannot select an invalid/missing root.
            let (selected, source) = select_hand_pose(
                Some(controller),
                action_source,
                None,
                Some(openxr::HandTrackingDataSourceEXT::UNOBSTRUCTED),
            );
            assert_eq!(source, action_source);
            assert_eq!(selected.unwrap().position.x, 1.0);
        }
    }

    #[test]
    fn mixed_hand_and_controller_sources_switch_independently() {
        let controller_left = pose([1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], 7);
        let controller_right = pose([2.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], 7);
        let controllers = ControllerPoseCache {
            left_aim: Some(controller_left),
            left_grip: Some(controller_left),
            right_aim: Some(controller_right),
            right_grip: Some(controller_right),
        };
        let mut hands = HandRootPoseCache {
            left_root: Some(pose([3.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], 7).pose),
            right_root: Some(pose([4.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], 7).pose),
            ..Default::default()
        };
        for (left_source, right_source, expected_positions) in [
            (
                openxr::HandTrackingDataSourceEXT::UNOBSTRUCTED,
                openxr::HandTrackingDataSourceEXT::CONTROLLER,
                [3.0, 2.0],
            ),
            (
                openxr::HandTrackingDataSourceEXT::CONTROLLER,
                openxr::HandTrackingDataSourceEXT::UNOBSTRUCTED,
                [1.0, 4.0],
            ),
        ] {
            hands.left_data_source = Some(left_source);
            hands.right_data_source = Some(right_source);
            for (hand, expected_x) in [
                (ControllerHand::Left, expected_positions[0]),
                (ControllerHand::Right, expected_positions[1]),
            ] {
                let (selected, _) = OpenXRSystem::preferred_pose(
                    &controllers,
                    &hands,
                    hand,
                    ControllerPoseKind::GripAim,
                );
                assert_eq!(selected.unwrap().position.x, expected_x);
            }
        }
        // The next sample loses the right hand. Its previous source cannot retain authority.
        hands.right_root = None;
        hands.right_data_source = None;
        let (selected, source) = OpenXRSystem::preferred_pose(
            &controllers,
            &hands,
            ControllerHand::Right,
            ControllerPoseKind::GripAim,
        );
        assert_eq!(selected.unwrap().position.x, 2.0);
        assert_eq!(source, ControllerPoseSource::ControllerGripAim);
    }

    #[test]
    fn full_hand_sample_preserves_all_joint_flags_raw_values_and_root_identity() {
        let mut joints = [openxr::HandJointLocationEXT::default(); openxr::HAND_JOINT_COUNT];
        for (index, joint) in joints.iter_mut().enumerate() {
            *joint = tracked_joint([index as f32, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0]);
            if index % 2 == 0 {
                joint.location_flags |= openxr::SpaceLocationFlags::POSITION_TRACKED;
            }
            if index % 3 == 0 {
                joint.location_flags |= openxr::SpaceLocationFlags::ORIENTATION_TRACKED;
            }
        }
        joints[openxr::HandJointEXT::WRIST].location_flags =
            openxr::SpaceLocationFlags::ORIENTATION_VALID;
        joints[openxr::HandJointEXT::THUMB_TIP].pose.position.x = f32::NAN;
        let sample = HandJointSample {
            joints,
            data_source: Some(openxr::HandTrackingDataSourceEXT::CONTROLLER),
        };
        let normalized = OpenXRSystem::normalize_hand_sample(&sample, ControllerHand::Left);
        assert_eq!(normalized.data_source, HandDataSource::Controller);
        assert_eq!(normalized.root_position.unwrap().0, HandJoint::Palm);
        assert!(normalized.canonical_orientation.is_some());
        assert!(!normalized.joints[HandJoint::Wrist as usize].position_valid);
        assert!(normalized.joints[HandJoint::ThumbTip as usize].position[0].is_nan());
        assert!(normalized.position(HandJoint::ThumbTip).is_none());
        for index in 2..26 {
            let joint = normalized.joints[index];
            assert_eq!(joint.position_tracked, index % 2 == 0);
            assert_eq!(joint.orientation_tracked, index % 3 == 0);
            assert!(joint.position_valid && joint.orientation_valid);
            assert_eq!(joint.radius_meters, Some(0.01));
            assert_eq!(
                joint.orientation_provenance,
                HandOrientationProvenance::NativeOpenXr
            );
        }
        // Articulation does not need authority over wrist translation.
        let mut sample = sample;
        sample.joints[openxr::HandJointEXT::PALM].location_flags =
            openxr::SpaceLocationFlags::ORIENTATION_VALID;
        let normalized = OpenXRSystem::normalize_hand_sample(&sample, ControllerHand::Left);
        assert!(normalized.root_position.is_none());
        assert!(normalized.has_articulation());
    }

    #[test]
    fn prepare_without_session_discards_previous_full_hand_frame() {
        let mut xr = OpenXRSystem::default();
        xr.hand_tracking_frame = Some(Arc::new(HandTrackingFrame {
            provider: HandTrackingProvider::OpenXr,
            epoch: 1,
            sequence: 1,
            sample_time: HandSampleTime::OpenXrPredictedNanoseconds(1),
            received_at: Instant::now(),
            space: HandCoordinateSpace::OpenXrStage { generation: 1 },
            hands: [None, None],
        }));
        xr.prepare_frame(&World::default(), &mut VisualWorld::default(), 0.0);
        assert!(xr.hand_tracking_frame().is_none());
    }

    fn tracked_joint(position: [f32; 3], orientation: [f32; 4]) -> openxr::HandJointLocationEXT {
        openxr::HandJointLocationEXT {
            location_flags: openxr::SpaceLocationFlags::POSITION_VALID
                | openxr::SpaceLocationFlags::ORIENTATION_VALID,
            pose: openxr::Posef {
                orientation: openxr::Quaternionf {
                    x: orientation[0],
                    y: orientation[1],
                    z: orientation[2],
                    w: orientation[3],
                },
                position: openxr::Vector3f {
                    x: position[0],
                    y: position[1],
                    z: position[2],
                },
            },
            radius: 0.01,
        }
    }

    #[test]
    fn hand_root_landmark_fallback_uses_the_shared_canonical_basis() {
        let mut joints = [openxr::HandJointLocationEXT::default(); openxr::HAND_JOINT_COUNT];
        let arbitrary_joint_orientation = [0.0, 0.70710677, 0.0, 0.70710677];
        joints[openxr::HandJointEXT::WRIST] =
            tracked_joint([3.0, 4.0, 5.0], arbitrary_joint_orientation);
        joints[openxr::HandJointEXT::WRIST].location_flags =
            openxr::SpaceLocationFlags::POSITION_VALID;
        joints[openxr::HandJointEXT::MIDDLE_METACARPAL] =
            tracked_joint([0.0, 0.0, 0.5], arbitrary_joint_orientation);
        joints[openxr::HandJointEXT::MIDDLE_PROXIMAL] =
            tracked_joint([0.0, 0.0, 0.0], arbitrary_joint_orientation);
        joints[openxr::HandJointEXT::MIDDLE_DISTAL] =
            tracked_joint([0.0, 0.0, -1.0], arbitrary_joint_orientation);
        joints[openxr::HandJointEXT::LITTLE_PROXIMAL] =
            tracked_joint([-1.0, 0.0, 0.0], arbitrary_joint_orientation);
        joints[openxr::HandJointEXT::INDEX_PROXIMAL] =
            tracked_joint([1.0, 0.0, 0.0], arbitrary_joint_orientation);

        let (pose, root) =
            OpenXRSystem::select_hand_root_pose(&joints, ControllerHand::Left).unwrap();
        assert_eq!(root, openxr::HandJointEXT::WRIST);
        assert_eq!(
            [pose.position.x, pose.position.y, pose.position.z],
            [3.0, 4.0, 5.0]
        );
        let q = [
            pose.orientation.x,
            pose.orientation.y,
            pose.orientation.z,
            pose.orientation.w,
        ];
        let forward = math::quat_rotate_vec3(q, [0.0, 0.0, -1.0]);
        let up = math::quat_rotate_vec3(q, [0.0, 1.0, 0.0]);
        assert!(math::vec3_dot(forward, [0.0, 0.0, -1.0]) > 0.9999);
        assert!(math::vec3_dot(up, [1.0, 0.0, 0.0]) > 0.9999);

        joints[openxr::HandJointEXT::WRIST] = openxr::HandJointLocationEXT::default();
        joints[openxr::HandJointEXT::PALM] =
            tracked_joint([6.0, 7.0, 8.0], arbitrary_joint_orientation);
        joints[openxr::HandJointEXT::PALM].location_flags =
            openxr::SpaceLocationFlags::POSITION_VALID;
        let (pose, root) =
            OpenXRSystem::select_hand_root_pose(&joints, ControllerHand::Left).unwrap();
        assert_eq!(root, openxr::HandJointEXT::PALM);
        assert_eq!(
            [pose.position.x, pose.position.y, pose.position.z],
            [6.0, 7.0, 8.0]
        );

        joints[openxr::HandJointEXT::PALM].location_flags |=
            openxr::SpaceLocationFlags::ORIENTATION_VALID;
        joints[openxr::HandJointEXT::PALM].pose.orientation.x = f32::NAN;
        assert!(OpenXRSystem::select_hand_root_pose(&joints, ControllerHand::Left).is_some());

        joints[openxr::HandJointEXT::MIDDLE_METACARPAL] =
            joints[openxr::HandJointEXT::MIDDLE_PROXIMAL];
        assert!(OpenXRSystem::select_hand_root_pose(&joints, ControllerHand::Left).is_none());
    }

    #[test]
    fn finger_curl_does_not_pitch_the_hand_root() {
        let identity = [0.0, 0.0, 0.0, 1.0];
        for rotation in [
            identity,
            math::quat_from_axis_angle([1.0, 0.0, 0.0], std::f32::consts::FRAC_PI_2),
            math::quat_from_axis_angle([0.0, 0.0, 1.0], std::f32::consts::PI),
            math::quat_from_axis_angle([1.0, 2.0, 3.0], 0.7),
        ] {
            for thumb_side in [-1.0, 1.0] {
                let joint = |position| {
                    let mut joint = tracked_joint(
                        math::vec3_add(
                            math::quat_rotate_vec3(rotation, position),
                            [0.3, 1.2, -0.4],
                        ),
                        identity,
                    );
                    joint.location_flags = openxr::SpaceLocationFlags::POSITION_VALID;
                    joint
                };
                let hand = if thumb_side > 0.0 {
                    ControllerHand::Left
                } else {
                    ControllerHand::Right
                };
                let mut joints =
                    [openxr::HandJointLocationEXT::default(); openxr::HAND_JOINT_COUNT];
                joints[openxr::HandJointEXT::WRIST] = joint([0.0, 0.0, 0.0]);
                joints[openxr::HandJointEXT::MIDDLE_METACARPAL] = joint([0.0, 0.0, -0.02]);
                joints[openxr::HandJointEXT::MIDDLE_PROXIMAL] = joint([0.0, 0.0, -0.08]);
                joints[openxr::HandJointEXT::LITTLE_PROXIMAL] =
                    joint([-0.03 * thumb_side, 0.0, -0.07]);
                joints[openxr::HandJointEXT::INDEX_PROXIMAL] =
                    joint([0.03 * thumb_side, 0.0, -0.07]);

                let expected_forward = math::quat_rotate_vec3(rotation, [0.0, 0.0, -1.0]);
                let expected_width = math::quat_rotate_vec3(rotation, [thumb_side, 0.0, 0.0]);
                for curl_degrees in [0.0_f32, 45.0, 90.0, 135.0] {
                    let curl = curl_degrees.to_radians();
                    joints[openxr::HandJointEXT::MIDDLE_DISTAL] =
                        joint([0.0, -0.04 * curl.sin(), -0.08 - 0.04 * curl.cos()]);
                    let (pose, _) = OpenXRSystem::select_hand_root_pose(&joints, hand).unwrap();
                    let q = OpenXRSystem::quat_from_posef(pose);
                    let forward = math::quat_rotate_vec3(q, [0.0, 0.0, -1.0]);
                    let width = math::quat_rotate_vec3(q, [0.0, 1.0, 0.0]);
                    assert!(
                        math::vec3_dot(forward, expected_forward) > 0.9999,
                        "fixed palm pitched with {curl_degrees}-degree finger curl"
                    );
                    assert!(math::vec3_dot(width, expected_width) > 0.9999);
                }
            }
        }
    }

    #[test]
    fn hand_root_does_not_require_distal_finger_tracking() {
        let identity = [0.0, 0.0, 0.0, 1.0];
        let mut joints = [openxr::HandJointLocationEXT::default(); openxr::HAND_JOINT_COUNT];
        for (joint, position) in [
            (openxr::HandJointEXT::PALM, [0.0, 0.0, -0.03]),
            (openxr::HandJointEXT::MIDDLE_METACARPAL, [0.0, 0.0, -0.02]),
            (openxr::HandJointEXT::MIDDLE_PROXIMAL, [0.0, 0.0, -0.08]),
            (openxr::HandJointEXT::LITTLE_PROXIMAL, [-0.03, 0.0, -0.07]),
            (openxr::HandJointEXT::INDEX_PROXIMAL, [0.03, 0.0, -0.07]),
        ] {
            joints[joint] = tracked_joint(position, identity);
            joints[joint].location_flags = openxr::SpaceLocationFlags::POSITION_VALID;
        }
        let (pose, root) =
            OpenXRSystem::select_hand_root_pose(&joints, ControllerHand::Left).unwrap();
        assert_eq!(root, openxr::HandJointEXT::PALM);
        assert_eq!(pose.position.z, -0.03);

        joints[openxr::HandJointEXT::MIDDLE_METACARPAL]
            .pose
            .position
            .x = f32::NAN;
        assert!(OpenXRSystem::select_hand_root_pose(&joints, ControllerHand::Left).is_none());
    }

    #[test]
    fn palm_orientation_preserves_yaw_and_roll_despite_skewed_landmarks() {
        for (hand, thumb_side) in [(ControllerHand::Left, 1.0), (ControllerHand::Right, -1.0)] {
            for yaw in [-45.0_f32, 0.0, 45.0] {
                for roll in [-90.0_f32, -45.0, 0.0, 45.0, 90.0, 180.0] {
                    let palm_q = math::quat_mul(
                        math::quat_from_axis_angle([0.0, 1.0, 0.0], yaw.to_radians()),
                        math::quat_from_axis_angle([0.0, 0.0, -1.0], roll.to_radians()),
                    );
                    let skew_q = math::quat_mul(
                        palm_q,
                        math::quat_from_axis_angle([0.0, 1.0, 0.0], 45.0_f32.to_radians()),
                    );
                    let mut joints =
                        [openxr::HandJointLocationEXT::default(); openxr::HAND_JOINT_COUNT];
                    joints[openxr::HandJointEXT::WRIST] = tracked_joint([0.3, 1.2, -0.4], skew_q);
                    joints[openxr::HandJointEXT::PALM] = tracked_joint([0.0; 3], palm_q);
                    // Orientation and position validity are independent: keep the wrist origin
                    // even when only the palm's orientation is usable.
                    joints[openxr::HandJointEXT::PALM].location_flags =
                        openxr::SpaceLocationFlags::ORIENTATION_VALID;
                    for (joint, position) in [
                        (openxr::HandJointEXT::MIDDLE_METACARPAL, [0.0, 0.0, -0.02]),
                        (openxr::HandJointEXT::MIDDLE_PROXIMAL, [0.0, 0.0, -0.08]),
                        (
                            openxr::HandJointEXT::LITTLE_PROXIMAL,
                            [-0.03 * thumb_side, 0.0, -0.07],
                        ),
                        (
                            openxr::HandJointEXT::INDEX_PROXIMAL,
                            [0.03 * thumb_side, 0.0, -0.07],
                        ),
                    ] {
                        joints[joint] =
                            tracked_joint(math::quat_rotate_vec3(skew_q, position), skew_q);
                    }
                    let (pose, origin) =
                        OpenXRSystem::select_hand_root_pose(&joints, hand).unwrap();
                    assert_eq!(origin, openxr::HandJointEXT::WRIST);
                    assert_eq!(pose.position.x, 0.3);
                    let canonical_q = OpenXRSystem::quat_from_posef(pose);
                    for (canonical_axis, palm_axis) in [
                        ([0.0, 0.0, -1.0], [0.0, 0.0, -1.0]),
                        ([0.0, 1.0, 0.0], [thumb_side, 0.0, 0.0]),
                        ([-thumb_side, 0.0, 0.0], [0.0, 1.0, 0.0]),
                    ] {
                        let actual = math::quat_rotate_vec3(canonical_q, canonical_axis);
                        let expected = math::quat_rotate_vec3(palm_q, palm_axis);
                        assert!(
                            math::vec3_dot(actual, expected) > 0.9999,
                            "{hand:?} yaw={yaw} roll={roll}: actual={actual:?}, expected={expected:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn wrist_orientation_fallback_needs_no_finger_landmarks() {
        let wrist_q = math::quat_from_axis_angle([1.0, 2.0, 3.0], 0.7);
        for (hand, thumb_side) in [(ControllerHand::Left, 1.0), (ControllerHand::Right, -1.0)] {
            let mut joints = [openxr::HandJointLocationEXT::default(); openxr::HAND_JOINT_COUNT];
            joints[openxr::HandJointEXT::WRIST] =
                tracked_joint([1.0, 2.0, 3.0], wrist_q.map(|value| value * 2.0));
            for invalid_palm_q in [
                [0.0; 4],
                [f32::NAN, 0.0, 0.0, 1.0],
                [0.0, f32::INFINITY, 0.0, 1.0],
            ] {
                joints[openxr::HandJointEXT::PALM] = tracked_joint([4.0; 3], invalid_palm_q);
                let (pose, origin) = OpenXRSystem::select_hand_root_pose(&joints, hand).unwrap();
                assert_eq!(origin, openxr::HandJointEXT::WRIST);
                let canonical_q = OpenXRSystem::quat_from_posef(pose);
                let expected_forward = math::quat_rotate_vec3(wrist_q, [0.0, 0.0, -1.0]);
                let expected_width = math::quat_rotate_vec3(wrist_q, [thumb_side, 0.0, 0.0]);
                assert!(
                    math::vec3_dot(
                        math::quat_rotate_vec3(canonical_q, [0.0, 0.0, -1.0]),
                        expected_forward,
                    ) > 0.9999
                );
                assert!(
                    math::vec3_dot(
                        math::quat_rotate_vec3(canonical_q, [0.0, 1.0, 0.0]),
                        expected_width,
                    ) > 0.9999
                );
            }
            joints[openxr::HandJointEXT::WRIST].location_flags =
                openxr::SpaceLocationFlags::POSITION_VALID;
            assert!(OpenXRSystem::select_hand_root_pose(&joints, hand).is_none());
        }
    }

    #[test]
    fn hand_debug_measures_motion_in_the_selected_canonical_frame() {
        for hand in [ControllerHand::Left, ControllerHand::Right] {
            let mut joints = [openxr::HandJointLocationEXT::default(); openxr::HAND_JOINT_COUNT];
            joints[openxr::HandJointEXT::WRIST] = tracked_joint([0.0; 3], [0.0, 0.0, 0.0, 1.0]);
            let root = OpenXRSystem::select_hand_root_pose(&joints, hand);
            let previous = Some(OpenXRSystem::quat_from_posef(root.unwrap().0));
            let debug = OpenXRSystem::hand_debug_state(Some(&joints), root, previous);
            assert_eq!(debug.wrist_step_deg, Some(0));
            assert!(!debug.wrist_step_spike);

            let roll = math::quat_from_axis_angle([0.0, 0.0, -1.0], 10.0_f32.to_radians());
            joints[openxr::HandJointEXT::WRIST] = tracked_joint([0.0; 3], roll);
            let root = OpenXRSystem::select_hand_root_pose(&joints, hand);
            let debug = OpenXRSystem::hand_debug_state(Some(&joints), root, previous);
            assert_eq!(debug.wrist_step_deg, Some(10));
        }
    }
}

fn log_xr_gamepad_changes(prev: XrGamepadState, next: XrGamepadState) {
    if !openxr_debug_enabled() {
        return;
    }
    if prev.active != next.active {
        eprintln!(
            "[OpenXR][gamepad] active -> {}",
            if next.active { "true" } else { "false" }
        );
    }
    for (index, hand_name) in ["left", "right"].into_iter().enumerate() {
        let before = prev.hands[index];
        let after = next.hands[index];
        if before.thumbstick != after.thumbstick {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} thumbstick -> {:?}",
                after.thumbstick
            );
        }
        if before.trigger_value != after.trigger_value {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} trigger_value -> {:?}",
                after.trigger_value
            );
        }
        if before.trigger_pressed != after.trigger_pressed {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} trigger_pressed -> {:?}",
                after.trigger_pressed
            );
        }
        if before.grip_value != after.grip_value {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} grip_value -> {:?}",
                after.grip_value
            );
        }
        if before.grip_pressed != after.grip_pressed {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} grip_pressed -> {:?}",
                after.grip_pressed
            );
        }
        if before.button_x != after.button_x {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} button_x -> {:?}",
                after.button_x
            );
        }
        if before.button_y != after.button_y {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} button_y -> {:?}",
                after.button_y
            );
        }
        if before.button_a != after.button_a {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} button_a -> {:?}",
                after.button_a
            );
        }
        if before.button_b != after.button_b {
            eprintln!(
                "[OpenXR][gamepad] {hand_name} button_b -> {:?}",
                after.button_b
            );
        }
    }
}

fn log_active_interaction_profile(
    instance: &openxr::Instance,
    session: &openxr::Session<openxr::Vulkan>,
    user_path: openxr::Path,
    user_path_str: &str,
    last_logged: &mut Option<openxr::Path>,
) {
    let Ok(profile) = session.current_interaction_profile(user_path) else {
        return;
    };
    if *last_logged == Some(profile) {
        return;
    }
    *last_logged = Some(profile);
    if profile == openxr::Path::NULL {
        eprintln!("[OpenXR][profile] {user_path_str}: no active interaction profile");
        return;
    }
    match instance.path_to_string(profile) {
        Ok(profile_str) => {
            eprintln!("[OpenXR][profile] {user_path_str}: active profile = {profile_str}");
        }
        Err(err) => {
            eprintln!(
                "[OpenXR][profile] {user_path_str}: active profile path {:?} (path_to_string failed: {:?})",
                profile, err
            );
        }
    }
}

fn profile_string_or_none(instance: &openxr::Instance, profile: Option<openxr::Path>) -> String {
    match profile {
        Some(profile) if profile == openxr::Path::NULL => "none".to_string(),
        Some(profile) => instance
            .path_to_string(profile)
            .unwrap_or_else(|_| format!("{profile:?}")),
        None => "unknown".to_string(),
    }
}

fn scalar_stick_value(
    x: Option<openxr::ActionState<f32>>,
    y: Option<openxr::ActionState<f32>>,
) -> Option<[f32; 2]> {
    match (x, y) {
        (Some(x), Some(y)) if x.is_active || y.is_active => {
            Some([x.current_state, y.current_state])
        }
        _ => None,
    }
}

fn scalar_stick_state(
    x: Option<openxr::ActionState<f32>>,
    y: Option<openxr::ActionState<f32>>,
) -> Option<(bool, [f32; 2])> {
    match (x, y) {
        (Some(x), Some(y)) => Some((
            x.is_active || y.is_active,
            [x.current_state, y.current_state],
        )),
        _ => None,
    }
}

const XR_TRIGGER_PRESS_THRESHOLD: f32 = 0.7;
const XR_GRIP_PRESS_THRESHOLD: f32 = 0.7;

fn action_state_bool(
    action: &openxr::Action<bool>,
    session: &openxr::Session<openxr::Vulkan>,
    hand: openxr::Path,
) -> Option<openxr::ActionState<bool>> {
    action.state(session, hand).ok().filter(|s| s.is_active)
}

fn action_state_f32(
    action: &openxr::Action<f32>,
    session: &openxr::Session<openxr::Vulkan>,
    hand: openxr::Path,
) -> Option<openxr::ActionState<f32>> {
    action.state(session, hand).ok().filter(|s| s.is_active)
}

fn derive_trigger_like_pressed(
    primary_click: Option<openxr::ActionState<bool>>,
    analog_value: Option<openxr::ActionState<f32>>,
    threshold: f32,
) -> Option<(bool, f32)> {
    primary_click
        .map(|state| {
            (
                state.current_state,
                if state.current_state { 1.0 } else { 0.0 },
            )
        })
        .or_else(|| {
            analog_value.map(|state| (state.current_state >= threshold, state.current_state))
        })
}

fn derive_select_down(
    select: Option<openxr::ActionState<bool>>,
    trigger_click: Option<openxr::ActionState<bool>>,
    trigger_value: Option<openxr::ActionState<f32>>,
) -> bool {
    select
        .map(|state| state.current_state)
        .or_else(|| trigger_click.map(|state| state.current_state))
        .or_else(|| trigger_value.map(|state| state.current_state >= XR_TRIGGER_PRESS_THRESHOLD))
        .unwrap_or(false)
}

fn log_action_bound_sources<T: openxr::ActionTy>(
    session: &openxr::Session<openxr::Vulkan>,
    label: &str,
    action: &openxr::Action<T>,
) {
    let flags = openxr::InputSourceLocalizedNameFlags::USER_PATH
        | openxr::InputSourceLocalizedNameFlags::INTERACTION_PROFILE
        | openxr::InputSourceLocalizedNameFlags::COMPONENT;
    match action.bound_sources(session) {
        Ok(sources) if sources.is_empty() => {
            eprintln!("[OpenXR][bound_sources] {label}: <unbound>");
        }
        Ok(sources) => {
            for source in sources {
                let path_str = action
                    .instance()
                    .path_to_string(source)
                    .unwrap_or_else(|_| format!("{source:?}"));
                let localized = session
                    .input_source_localized_name(source, flags)
                    .unwrap_or_else(|err| format!("<localized-name error: {err:?}>"));
                eprintln!(
                    "[OpenXR][bound_sources] {label}: path={} localized={}",
                    path_str, localized
                );
            }
        }
        Err(err) => {
            eprintln!("[OpenXR][bound_sources] {label}: error {err:?}");
        }
    }
}

fn log_controller_input_bound_sources(
    session: &openxr::Session<openxr::Vulkan>,
    ci: &ControllerInput,
) {
    log_action_bound_sources(session, "select", &ci.select);
    log_action_bound_sources(session, "left_stick_x", &ci.left_stick_x);
    log_action_bound_sources(session, "left_stick_y", &ci.left_stick_y);
    log_action_bound_sources(session, "right_stick_x", &ci.right_stick_x);
    log_action_bound_sources(session, "right_stick_y", &ci.right_stick_y);
    log_action_bound_sources(session, "trigger_value", &ci.trigger_value);
    log_action_bound_sources(session, "trigger_click", &ci.trigger_click);
    log_action_bound_sources(session, "grip_value", &ci.grip_value);
    log_action_bound_sources(session, "grip_click", &ci.grip_click);
    log_action_bound_sources(session, "button_x", &ci.button_x);
    log_action_bound_sources(session, "button_y", &ci.button_y);
    log_action_bound_sources(session, "button_a", &ci.button_a);
    log_action_bound_sources(session, "button_b", &ci.button_b);
}

fn append_action_bindings<'a>(
    instance: &openxr::Instance,
    suggested: &mut Vec<(&'static str, &'static str, openxr::Binding<'a>)>,
    spec: BindingSpec,
    left_stick_x: &'a openxr::Action<f32>,
    left_stick_y: &'a openxr::Action<f32>,
    right_stick_x: &'a openxr::Action<f32>,
    right_stick_y: &'a openxr::Action<f32>,
    trigger_value: &'a openxr::Action<f32>,
    trigger_click: &'a openxr::Action<bool>,
    grip_value: &'a openxr::Action<f32>,
    grip_click: &'a openxr::Action<bool>,
    button_a: &'a openxr::Action<bool>,
    button_b: &'a openxr::Action<bool>,
    button_x: &'a openxr::Action<bool>,
    button_y: &'a openxr::Action<bool>,
) {
    for &path_str in spec.paths {
        let Ok(path) = instance.string_to_path(path_str) else {
            continue;
        };
        match spec.label {
            "left_stick_x" => suggested.push((
                spec.label,
                path_str,
                openxr::Binding::new(left_stick_x, path),
            )),
            "left_stick_y" => suggested.push((
                spec.label,
                path_str,
                openxr::Binding::new(left_stick_y, path),
            )),
            "right_stick_x" => suggested.push((
                spec.label,
                path_str,
                openxr::Binding::new(right_stick_x, path),
            )),
            "right_stick_y" => suggested.push((
                spec.label,
                path_str,
                openxr::Binding::new(right_stick_y, path),
            )),
            "trigger_value" => suggested.push((
                spec.label,
                path_str,
                openxr::Binding::new(trigger_value, path),
            )),
            "trigger_click" => suggested.push((
                spec.label,
                path_str,
                openxr::Binding::new(trigger_click, path),
            )),
            "grip_value" => {
                suggested.push((spec.label, path_str, openxr::Binding::new(grip_value, path)))
            }
            "grip_click" => {
                suggested.push((spec.label, path_str, openxr::Binding::new(grip_click, path)))
            }
            "a" => suggested.push((spec.label, path_str, openxr::Binding::new(button_a, path))),
            "b" => suggested.push((spec.label, path_str, openxr::Binding::new(button_b, path))),
            "x" => suggested.push((spec.label, path_str, openxr::Binding::new(button_x, path))),
            "y" => suggested.push((spec.label, path_str, openxr::Binding::new(button_y, path))),
            _ => {}
        }
    }
}

fn suggest_binding_best_effort(
    instance: &openxr::Instance,
    profile: openxr::Path,
    profile_name: &str,
    binding_label: &str,
    binding_path: &str,
    binding: openxr::Binding<'_>,
) -> Result<BindingSuggestionOutcome, String> {
    match instance.suggest_interaction_profile_bindings(profile, &[binding]) {
        Ok(()) => Ok(BindingSuggestionOutcome::Accepted),
        Err(openxr::sys::Result::ERROR_PATH_UNSUPPORTED) => {
            eprintln!(
                "[OpenXR] ignoring unsupported binding profile={} label={} path={}",
                profile_name, binding_label, binding_path
            );
            Ok(BindingSuggestionOutcome::Unsupported)
        }
        Err(err) => Err(format!(
            "suggest_interaction_profile_bindings({}, {} -> {}): {err:?}",
            profile_name, binding_label, binding_path
        )),
    }
}

fn log_profile_binding_dump(spec: &ProfileBindingSpec) {
    if !openxr_debug_enabled() {
        return;
    }
    eprintln!("[OpenXR][binding_dump] profile {}", spec.profile);
    eprintln!(
        "[OpenXR][binding_dump]   select left={:?} right={:?}",
        spec.select_left, spec.select_right
    );
    for spec in spec.left_specs {
        eprintln!(
            "[OpenXR][binding_dump]   left {:<13} -> {}",
            spec.label,
            spec.paths.join(", ")
        );
    }
    for spec in spec.right_specs {
        eprintln!(
            "[OpenXR][binding_dump]   right {:<12} -> {}",
            spec.label,
            spec.paths.join(", ")
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingSuggestionOutcome {
    Accepted,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BindingSuggestionRecord {
    label: &'static str,
    path: String,
    outcome: BindingSuggestionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfileBindingReport {
    profile_path: String,
    suggestions: Vec<BindingSuggestionRecord>,
}

impl ProfileBindingReport {
    fn accepted_count(&self) -> usize {
        self.suggestions
            .iter()
            .filter(|record| record.outcome == BindingSuggestionOutcome::Accepted)
            .count()
    }

    fn unsupported_count(&self) -> usize {
        self.suggestions
            .iter()
            .filter(|record| record.outcome == BindingSuggestionOutcome::Unsupported)
            .count()
    }
}

fn log_runtime_profile_binding_report(
    active_profile_label: &str,
    report: &ProfileBindingReport,
    snapshot: &ControllerDebugSnapshot,
) {
    eprintln!(
        "[OpenXR][binding_report] hand={} profile={} accepted={} unsupported={}",
        active_profile_label,
        report.profile_path,
        report.accepted_count(),
        report.unsupported_count(),
    );
    for record in &report.suggestions {
        let outcome = match record.outcome {
            BindingSuggestionOutcome::Accepted => "accepted",
            BindingSuggestionOutcome::Unsupported => "unsupported",
        };
        eprintln!(
            "[OpenXR][binding_report]   {:<12} {:<11} {}",
            record.label, outcome, record.path
        );
    }
    eprintln!(
        "[OpenXR][binding_report]   runtime_state profileL={} profileR={} poseL(aim={},grip={}) poseR(aim={},grip={})",
        snapshot.left_profile,
        snapshot.right_profile,
        snapshot.left_aim_valid,
        snapshot.left_grip_valid,
        snapshot.right_aim_valid,
        snapshot.right_grip_valid,
    );
    eprintln!(
        "[OpenXR][binding_report]   buttons/select left(select={:?},x={:?},y={:?}) right(select={:?},a={:?},b={:?})",
        snapshot.left_select,
        snapshot.left_x,
        snapshot.left_y,
        snapshot.right_select,
        snapshot.right_a,
        snapshot.right_b,
    );
    eprintln!(
        "[OpenXR][binding_report]   analog left(stick={:?},trigger={:?}/{:?},grip={:?}/{:?}) right(stick={:?},trigger={:?}/{:?},grip={:?}/{:?})",
        snapshot.left_thumbstick,
        snapshot.left_trigger_value,
        snapshot.left_trigger_click,
        snapshot.left_grip_value,
        snapshot.left_grip_click,
        snapshot.right_thumbstick,
        snapshot.right_trigger_value,
        snapshot.right_trigger_click,
        snapshot.right_grip_value,
        snapshot.right_grip_click,
    );
}

pub struct OpenXRSystem {
    state: Option<OpenXRState>,
    last_init_error: Option<String>,
    vulkan_graphics: Option<XrVulkanGraphics>,
    preferred_swapchain_format: Option<u32>,
    runtime_exit_requested: bool,

    // Best-effort XR frame timing diagnostics.
    last_render_instant: Option<Instant>,
    last_render_dt_sec: Option<f32>,
    render_profile: XrRenderProfile,
    vr_perf: Option<VrPerfCollector>,
    vr_perf_exit_requested: bool,
    vr_perf_pre_xr: VrPerfPreXrCpu,
    next_xr_batch_generation: u64,
    floor_correction_meters: f32,

    input_xr_components: HashSet<ComponentId>,
    controller_components: HashSet<ComponentId>,
    controller_pose_source_last_logged: HashMap<ComponentId, ControllerPoseSource>,

    external_frame: Option<super::external_xr_frame::ExternalXrFrame>,
    xr_input_state: XrInputState,
    xr_gamepad_state: XrGamepadState,
    xr_gamepad_state_last_logged: XrGamepadState,
    hand_tracking_frame: Option<Arc<HandTrackingFrame>>,
    hand_tracking_epoch: u64,
    hand_tracking_sequence: u64,
}

#[derive(Debug, Default)]
struct XrRenderProfile {
    frames: u64,
    total: std::time::Duration,
    wait_frame: std::time::Duration,
    tracking_input: std::time::Duration,
    acquire_wait: std::time::Duration,
    eye_render: std::time::Duration,
    completion_wait: std::time::Duration,
    submit: std::time::Duration,
}

struct OpenXRState {
    #[allow(dead_code)]
    entry: openxr::Entry,
    #[allow(dead_code)]
    instance: openxr::Instance,
    #[allow(dead_code)]
    system: openxr::SystemId,
    events: openxr::EventDataBuffer,

    session: Option<OpenXRSessionState>,
    view_type: openxr::ViewConfigurationType,
    blend_mode: openxr::EnvironmentBlendMode,
}

struct OpenXRSessionState {
    session: openxr::Session<openxr::Vulkan>,
    frame_waiter: openxr::FrameWaiter,
    frame_stream: openxr::FrameStream<openxr::Vulkan>,
    reference_space: openxr::Space,
    running: bool,
    render_failed: bool,
    current_state: openxr::SessionState,

    xr_swapchain: XRSwapchain,

    swapchain_image_initialized: Vec<bool>,

    did_log_format_mismatch: bool,

    vk_device: ash::Device,
    vk_queue: ash::vk::Queue,

    #[allow(dead_code)]
    vk_command_pool: ash::vk::CommandPool,
    vk_command_buffer: ash::vk::CommandBuffer,
    vk_completion_fence: ash::vk::Fence,
    vk_completion_fence_usable: bool,

    hand_tracking: Option<HandTrackingState>,
    hand_root_pose_cache: HandRootPoseCache,
    hand_rotation_debug: HandRotationDebugState,
    last_hand_debug_snapshot: Option<HandDebugSnapshot>,
    head_pose_cache: Option<openxr::Posef>,
    controller_input: Option<ControllerInput>,
    controller_pose_cache: ControllerPoseCache,
    controller_pose_generation: u64,
    prepared_frame: Option<PreparedXrFrame>,
}

/// Located once before the engine pose phases, consumed once by rendering.
struct PreparedXrFrame {
    state: openxr::FrameState,
    views: Vec<openxr::View>,
    wait_time: Duration,
    started: Instant,
}

impl Drop for OpenXRSessionState {
    fn drop(&mut self) {
        unsafe {
            // Teardown is an exceptional synchronization boundary. Universe
            // drops the XR system before the renderer/device owner.
            let _ = self.vk_device.device_wait_idle();
            self.vk_device.destroy_fence(self.vk_completion_fence, None);
            self.vk_device
                .destroy_command_pool(self.vk_command_pool, None);
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct ControllerPoseCache {
    left_aim: Option<ControllerPoseSample>,
    right_aim: Option<ControllerPoseSample>,
    left_grip: Option<ControllerPoseSample>,
    right_grip: Option<ControllerPoseSample>,
}

#[derive(Debug, Clone, Copy)]
struct ControllerPoseSample {
    pose: openxr::Posef,
    generation: u64,
}

fn grip_aim_pose(
    grip: Option<ControllerPoseSample>,
    aim: Option<ControllerPoseSample>,
) -> Option<openxr::Posef> {
    let grip = grip?;
    let aim = aim?;
    if grip.generation != aim.generation {
        return None;
    }
    Some(openxr::Posef {
        orientation: aim.pose.orientation,
        position: grip.pose.position,
    })
}

fn select_hand_pose(
    controller_pose: Option<openxr::Posef>,
    controller_source: ControllerPoseSource,
    hand_root: Option<openxr::Posef>,
    hand_data_source: Option<openxr::HandTrackingDataSourceEXT>,
) -> (Option<openxr::Posef>, ControllerPoseSource) {
    // A free-hand device may also emulate active controller actions under a
    // controller profile. Its anatomical root must win over those action poses.
    if hand_data_source == Some(openxr::HandTrackingDataSourceEXT::UNOBSTRUCTED) {
        if let Some(pose) = hand_root {
            return (Some(pose), ControllerPoseSource::WristPalm);
        }
    }
    if let Some(pose) = controller_pose {
        return (Some(pose), controller_source);
    }
    if let Some(pose) = hand_root {
        return (Some(pose), ControllerPoseSource::WristPalm);
    }
    (None, ControllerPoseSource::None)
}

struct HandTrackingState {
    left: openxr::HandTracker,
    right: openxr::HandTracker,
}

struct HandJointSample {
    joints: openxr::HandJointLocations,
    data_source: Option<openxr::HandTrackingDataSourceEXT>,
}

#[derive(Debug, Default, Clone, Copy)]
struct HandRootPoseCache {
    left_root: Option<openxr::Posef>,
    right_root: Option<openxr::Posef>,
    left_root_joint: Option<openxr::HandJointEXT>,
    right_root_joint: Option<openxr::HandJointEXT>,
    left_data_source: Option<openxr::HandTrackingDataSourceEXT>,
    right_data_source: Option<openxr::HandTrackingDataSourceEXT>,
}

#[derive(Debug, Default)]
struct HandRotationDebugState {
    left: RollingAngleWindow,
    right: RollingAngleWindow,
}

#[derive(Debug, Default)]
struct RollingAngleWindow {
    previous_quat_xyzw: Option<[f32; 4]>,
    step_deg: VecDeque<f32>,
    sample_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HandDebugState {
    root_joint: Option<openxr::HandJointEXT>,
    wrist_palm_delta_deg: Option<u16>,
    wrist_step_deg: Option<u16>,
    wrist_step_spike: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HandDebugSnapshot {
    left: HandDebugState,
    right: HandDebugState,
}

#[derive(Debug, Clone, PartialEq)]
struct ControllerDebugSnapshot {
    left_profile: String,
    right_profile: String,
    left_aim_valid: bool,
    right_aim_valid: bool,
    left_grip_valid: bool,
    right_grip_valid: bool,
    left_select: Option<(bool, bool)>,
    right_select: Option<(bool, bool)>,
    left_thumbstick: Option<(bool, [f32; 2])>,
    right_thumbstick: Option<(bool, [f32; 2])>,
    left_trigger_value: Option<(bool, f32)>,
    right_trigger_value: Option<(bool, f32)>,
    left_trigger_click: Option<(bool, bool)>,
    right_trigger_click: Option<(bool, bool)>,
    left_grip_value: Option<(bool, f32)>,
    right_grip_value: Option<(bool, f32)>,
    left_grip_click: Option<(bool, bool)>,
    right_grip_click: Option<(bool, bool)>,
    left_x: Option<(bool, bool)>,
    left_y: Option<(bool, bool)>,
    right_a: Option<(bool, bool)>,
    right_b: Option<(bool, bool)>,
}

#[derive(Debug, Clone, Copy)]
struct BindingSpec {
    label: &'static str,
    paths: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
struct ProfileBindingSpec {
    profile: &'static str,
    select_left: Option<&'static str>,
    select_right: Option<&'static str>,
    left_specs: &'static [BindingSpec],
    right_specs: &'static [BindingSpec],
}

#[allow(dead_code)]
struct ControllerInput {
    action_set: openxr::ActionSet,
    aim_pose: openxr::Action<openxr::Posef>,
    grip_pose: openxr::Action<openxr::Posef>,
    select: openxr::Action<bool>,
    left_stick_x: openxr::Action<f32>,
    left_stick_y: openxr::Action<f32>,
    right_stick_x: openxr::Action<f32>,
    right_stick_y: openxr::Action<f32>,
    trigger_value: openxr::Action<f32>,
    trigger_click: openxr::Action<bool>,
    grip_value: openxr::Action<f32>,
    grip_click: openxr::Action<bool>,
    button_a: openxr::Action<bool>,
    button_b: openxr::Action<bool>,
    button_x: openxr::Action<bool>,
    button_y: openxr::Action<bool>,

    left: openxr::Path,
    right: openxr::Path,

    left_aim_space: openxr::Space,
    right_aim_space: openxr::Space,
    left_grip_space: openxr::Space,
    right_grip_space: openxr::Space,

    profile_poll_counter: u32,
    last_logged_left_profile: Option<openxr::Path>,
    last_logged_right_profile: Option<openxr::Path>,
    last_reported_left_profile: Option<openxr::Path>,
    last_reported_right_profile: Option<openxr::Path>,
    last_debug_snapshot: Option<ControllerDebugSnapshot>,
    did_log_bound_sources: bool,
    binding_reports: Vec<ProfileBindingReport>,
}

const PROFILE_SIMPLE_CONTROLLER: &str = "/interaction_profiles/khr/simple_controller";
const PROFILE_OCULUS_TOUCH: &str = "/interaction_profiles/oculus/touch_controller";
const PROFILE_HTC_VIVE: &str = "/interaction_profiles/htc/vive_controller";
const PROFILE_HTC_VIVE_FOCUS3: &str = "/interaction_profiles/htc/vive_focus3_controller";
const PROFILE_VALVE_INDEX: &str = "/interaction_profiles/valve/index_controller";
const PROFILE_MICROSOFT_MOTION: &str = "/interaction_profiles/microsoft/motion_controller";

const SIMPLE_CONTROLLER_LEFT_SPECS: &[BindingSpec] = &[];
const SIMPLE_CONTROLLER_RIGHT_SPECS: &[BindingSpec] = &[];

const OCULUS_TOUCH_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "left_stick_x",
        paths: &["/user/hand/left/input/thumbstick/x"],
    },
    BindingSpec {
        label: "left_stick_y",
        paths: &["/user/hand/left/input/thumbstick/y"],
    },
    BindingSpec {
        label: "trigger_value",
        paths: &["/user/hand/left/input/trigger/value"],
    },
    BindingSpec {
        label: "grip_value",
        paths: &["/user/hand/left/input/squeeze/value"],
    },
    BindingSpec {
        label: "x",
        paths: &["/user/hand/left/input/x/click"],
    },
    BindingSpec {
        label: "y",
        paths: &["/user/hand/left/input/y/click"],
    },
];
const OCULUS_TOUCH_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "right_stick_x",
        paths: &["/user/hand/right/input/thumbstick/x"],
    },
    BindingSpec {
        label: "right_stick_y",
        paths: &["/user/hand/right/input/thumbstick/y"],
    },
    BindingSpec {
        label: "trigger_value",
        paths: &["/user/hand/right/input/trigger/value"],
    },
    BindingSpec {
        label: "grip_value",
        paths: &["/user/hand/right/input/squeeze/value"],
    },
    BindingSpec {
        label: "a",
        paths: &["/user/hand/right/input/a/click"],
    },
    BindingSpec {
        label: "b",
        paths: &["/user/hand/right/input/b/click"],
    },
];

const VALVE_INDEX_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "left_stick_x",
        paths: &[
            "/user/hand/left/input/thumbstick/x",
            "/user/hand/left/input/joystick/x",
        ],
    },
    BindingSpec {
        label: "left_stick_y",
        paths: &[
            "/user/hand/left/input/thumbstick/y",
            "/user/hand/left/input/joystick/y",
        ],
    },
    BindingSpec {
        label: "trigger_value",
        paths: &["/user/hand/left/input/trigger/value"],
    },
    BindingSpec {
        label: "grip_value",
        paths: &[
            "/user/hand/left/input/squeeze/value",
            "/user/hand/left/input/grip/value",
        ],
    },
];
const VALVE_INDEX_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "right_stick_x",
        paths: &[
            "/user/hand/right/input/thumbstick/x",
            "/user/hand/right/input/joystick/x",
        ],
    },
    BindingSpec {
        label: "right_stick_y",
        paths: &[
            "/user/hand/right/input/thumbstick/y",
            "/user/hand/right/input/joystick/y",
        ],
    },
    BindingSpec {
        label: "trigger_value",
        paths: &["/user/hand/right/input/trigger/value"],
    },
    BindingSpec {
        label: "grip_value",
        paths: &[
            "/user/hand/right/input/squeeze/value",
            "/user/hand/right/input/grip/value",
        ],
    },
    BindingSpec {
        label: "a",
        paths: &["/user/hand/right/input/a/click"],
    },
    BindingSpec {
        label: "b",
        paths: &["/user/hand/right/input/b/click"],
    },
];

const MICROSOFT_MOTION_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "left_stick_x",
        paths: &[
            "/user/hand/left/input/thumbstick/x",
            "/user/hand/left/input/joystick/x",
        ],
    },
    BindingSpec {
        label: "left_stick_y",
        paths: &[
            "/user/hand/left/input/thumbstick/y",
            "/user/hand/left/input/joystick/y",
        ],
    },
    BindingSpec {
        label: "trigger_value",
        paths: &["/user/hand/left/input/trigger/value"],
    },
    BindingSpec {
        label: "grip_click",
        paths: &[
            "/user/hand/left/input/squeeze/click",
            "/user/hand/left/input/grip/click",
        ],
    },
];
const MICROSOFT_MOTION_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "right_stick_x",
        paths: &[
            "/user/hand/right/input/thumbstick/x",
            "/user/hand/right/input/joystick/x",
        ],
    },
    BindingSpec {
        label: "right_stick_y",
        paths: &[
            "/user/hand/right/input/thumbstick/y",
            "/user/hand/right/input/joystick/y",
        ],
    },
    BindingSpec {
        label: "trigger_value",
        paths: &["/user/hand/right/input/trigger/value"],
    },
    BindingSpec {
        label: "grip_click",
        paths: &[
            "/user/hand/right/input/squeeze/click",
            "/user/hand/right/input/grip/click",
        ],
    },
];

const HTC_VIVE_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "trigger_click",
        paths: &["/user/hand/left/input/trigger/click"],
    },
    BindingSpec {
        label: "grip_click",
        paths: &[
            "/user/hand/left/input/squeeze/click",
            "/user/hand/left/input/grip/click",
        ],
    },
];
const HTC_VIVE_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "trigger_click",
        paths: &["/user/hand/right/input/trigger/click"],
    },
    BindingSpec {
        label: "grip_click",
        paths: &[
            "/user/hand/right/input/squeeze/click",
            "/user/hand/right/input/grip/click",
        ],
    },
];

const HTC_VIVE_FOCUS3_LEFT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "left_stick_x",
        paths: &["/user/hand/left/input/thumbstick/x"],
    },
    BindingSpec {
        label: "left_stick_y",
        paths: &["/user/hand/left/input/thumbstick/y"],
    },
    BindingSpec {
        label: "trigger_value",
        paths: &["/user/hand/left/input/trigger/value"],
    },
    BindingSpec {
        label: "grip_value",
        paths: &["/user/hand/left/input/squeeze/value"],
    },
    BindingSpec {
        label: "x",
        paths: &["/user/hand/left/input/x/click"],
    },
    BindingSpec {
        label: "y",
        paths: &["/user/hand/left/input/y/click"],
    },
];
const HTC_VIVE_FOCUS3_RIGHT_SPECS: &[BindingSpec] = &[
    BindingSpec {
        label: "right_stick_x",
        paths: &["/user/hand/right/input/thumbstick/x"],
    },
    BindingSpec {
        label: "right_stick_y",
        paths: &["/user/hand/right/input/thumbstick/y"],
    },
    BindingSpec {
        label: "trigger_value",
        paths: &["/user/hand/right/input/trigger/value"],
    },
    BindingSpec {
        label: "grip_value",
        paths: &["/user/hand/right/input/squeeze/value"],
    },
    BindingSpec {
        label: "a",
        paths: &["/user/hand/right/input/a/click"],
    },
    BindingSpec {
        label: "b",
        paths: &["/user/hand/right/input/b/click"],
    },
];

#[path = "openxr_hand_bindings.rs"]
mod hand_bindings;
use hand_bindings::{
    EXT_HAND_INTERACTION_LEFT_SPECS, EXT_HAND_INTERACTION_RIGHT_SPECS, PROFILE_EXT_HAND_INTERACTION,
};

const PROFILE_BINDING_SPECS: &[ProfileBindingSpec] = &[
    ProfileBindingSpec {
        profile: PROFILE_SIMPLE_CONTROLLER,
        select_left: Some("/user/hand/left/input/select/click"),
        select_right: Some("/user/hand/right/input/select/click"),
        left_specs: SIMPLE_CONTROLLER_LEFT_SPECS,
        right_specs: SIMPLE_CONTROLLER_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_OCULUS_TOUCH,
        select_left: None,
        select_right: None,
        left_specs: OCULUS_TOUCH_LEFT_SPECS,
        right_specs: OCULUS_TOUCH_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_HTC_VIVE,
        select_left: Some("/user/hand/left/input/trigger/click"),
        select_right: Some("/user/hand/right/input/trigger/click"),
        left_specs: HTC_VIVE_LEFT_SPECS,
        right_specs: HTC_VIVE_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_HTC_VIVE_FOCUS3,
        select_left: None,
        select_right: None,
        left_specs: HTC_VIVE_FOCUS3_LEFT_SPECS,
        right_specs: HTC_VIVE_FOCUS3_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_VALVE_INDEX,
        select_left: Some("/user/hand/left/input/trigger/value"),
        select_right: Some("/user/hand/right/input/trigger/value"),
        left_specs: VALVE_INDEX_LEFT_SPECS,
        right_specs: VALVE_INDEX_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_MICROSOFT_MOTION,
        select_left: Some("/user/hand/left/input/trigger/value"),
        select_right: Some("/user/hand/right/input/trigger/value"),
        left_specs: MICROSOFT_MOTION_LEFT_SPECS,
        right_specs: MICROSOFT_MOTION_RIGHT_SPECS,
    },
    ProfileBindingSpec {
        profile: PROFILE_EXT_HAND_INTERACTION,
        select_left: Some("/user/hand/left/input/pinch_ext/value"),
        select_right: Some("/user/hand/right/input/pinch_ext/value"),
        left_specs: EXT_HAND_INTERACTION_LEFT_SPECS,
        right_specs: EXT_HAND_INTERACTION_RIGHT_SPECS,
    },
];

impl Default for OpenXRSystem {
    fn default() -> Self {
        Self {
            state: None,
            last_init_error: None,
            vulkan_graphics: None,
            preferred_swapchain_format: None,
            runtime_exit_requested: false,

            last_render_instant: None,
            last_render_dt_sec: None,
            render_profile: XrRenderProfile::default(),
            vr_perf: None,
            vr_perf_exit_requested: false,
            vr_perf_pre_xr: VrPerfPreXrCpu::default(),
            next_xr_batch_generation: 1,
            floor_correction_meters: 0.0,

            input_xr_components: HashSet::new(),
            controller_components: HashSet::new(),
            controller_pose_source_last_logged: HashMap::new(),

            external_frame: None,
            xr_input_state: XrInputState::default(),
            xr_gamepad_state: XrGamepadState::default(),
            xr_gamepad_state_last_logged: XrGamepadState::default(),
            hand_tracking_frame: None,
            hand_tracking_epoch: 1,
            hand_tracking_sequence: 0,
        }
    }
}

impl Drop for OpenXRSystem {
    fn drop(&mut self) {
        self.discard_prepared_frame();
    }
}

impl std::fmt::Debug for OpenXRSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenXRSystem")
            .field("initialized", &self.state.is_some())
            .field("last_init_error", &self.last_init_error)
            .field("last_render_dt_sec", &self.last_render_dt_sec)
            .finish()
    }
}

impl OpenXRSystem {
    pub fn configure_vr_perf(&mut self, config: VrPerfConfig) {
        self.vr_perf = Some(VrPerfCollector::new(config));
        self.vr_perf_exit_requested = false;
    }

    pub fn vr_perf_exit_requested(&self) -> bool {
        self.vr_perf_exit_requested
    }

    pub fn set_vr_perf_pre_xr(&mut self, timings: VrPerfPreXrCpu) {
        self.vr_perf_pre_xr = timings;
    }

    fn record_vr_perf_frame(
        &mut self,
        visuals: &VisualWorld,
        renderer: &VulkanoRenderer,
        runtime: String,
        extent: [u32; 2],
        display_interval: Duration,
        cpu: VrPerfFrameCpu,
    ) {
        let Some(collector) = self.vr_perf.as_mut() else {
            return;
        };
        let metadata = VrPerfMetadata {
            build_profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            gpu_device: renderer
                .gpu_device_name()
                .unwrap_or_else(|| "unavailable".to_string()),
            openxr_runtime: runtime,
            render_extent: extent,
            msaa: renderer
                .msaa_description()
                .unwrap_or("unavailable")
                .to_string(),
        };
        let resources_ready = visuals.deformation_cache_live_vertices() > 0;
        if let Some(result) = collector.presented_frame(
            Instant::now(),
            resources_ready,
            display_interval,
            cpu,
            &metadata,
        ) {
            match result {
                Ok(path) => println!("[vr-perf] report written: {}", path.display()),
                Err(error) => eprintln!("[vr-perf] {error}"),
            }
            self.vr_perf_exit_requested = true;
        }
    }

    fn profile_systems_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            matches!(
                std::env::var("CAT_PROFILE_SYSTEMS")
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase()
                    .as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
    }

    fn finish_xr_render_profile_frame(&mut self, frame_started: Instant) {
        self.render_profile.frames += 1;
        self.render_profile.total += frame_started.elapsed();
        if self.render_profile.frames < 120 {
            return;
        }
        let frames = self.render_profile.frames as f64;
        let avg_ms = |duration: std::time::Duration| duration.as_secs_f64() * 1000.0 / frames;
        let line = format!(
            "[xr-render-profile] frames={} avg_ms total={:.3} wait_frame={:.3} tracking_input={:.3} acquire_wait={:.3} eye_render={:.3} completion_wait={:.3} submit={:.3}",
            self.render_profile.frames,
            avg_ms(self.render_profile.total),
            avg_ms(self.render_profile.wait_frame),
            avg_ms(self.render_profile.tracking_input),
            avg_ms(self.render_profile.acquire_wait),
            avg_ms(self.render_profile.eye_render),
            avg_ms(self.render_profile.completion_wait),
            avg_ms(self.render_profile.submit),
        );
        eprintln!("{line}");
        crate::utils::profile_log::append("system-profile", &line);
        self.render_profile = XrRenderProfile::default();
    }

    pub fn xr_input_state(&self) -> &XrInputState {
        &self.xr_input_state
    }

    pub fn xr_gamepad_state(&self) -> &XrGamepadState {
        &self.xr_gamepad_state
    }

    pub fn last_init_error(&self) -> Option<&str> {
        self.last_init_error.as_deref()
    }

    pub fn prepare_for_renderer_init(&mut self, world: &World) {
        let should_enable = world.all_components().any(|cid| {
            world
                .get_component_by_id_as::<XrComponent>(cid)
                .map(|component| component.enabled)
                .unwrap_or(false)
        });

        if should_enable {
            let _ = self.initialize_runtime();
        }
    }

    fn debug_hand_rotation_enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var("CAT_DEBUG_XR_HAND_ROTATION")
                .ok()
                .map(|value| {
                    let value = value.trim().to_ascii_lowercase();
                    matches!(value.as_str(), "1" | "true" | "yes" | "on")
                })
                .unwrap_or(false)
        })
    }

    fn debug_hand_rotation_window_len() -> usize {
        static WINDOW_LEN: OnceLock<usize> = OnceLock::new();
        *WINDOW_LEN.get_or_init(|| {
            std::env::var("CAT_DEBUG_XR_HAND_ROTATION_WINDOW")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .map(|value| value.clamp(1, 600))
                .unwrap_or(60)
        })
    }

    fn quat_from_posef(pose: openxr::Posef) -> [f32; 4] {
        [
            pose.orientation.x,
            pose.orientation.y,
            pose.orientation.z,
            pose.orientation.w,
        ]
    }

    fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
        let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        if len > 0.0 {
            [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        }
    }

    fn quat_nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
        let mut end = b;
        let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
        if dot < 0.0 {
            end = [-b[0], -b[1], -b[2], -b[3]];
        }
        let one_minus_t = 1.0 - t;
        Self::quat_normalize([
            a[0] * one_minus_t + end[0] * t,
            a[1] * one_minus_t + end[1] * t,
            a[2] * one_minus_t + end[2] * t,
            a[3] * one_minus_t + end[3] * t,
        ])
    }

    fn pose_from_quat_translation(
        rotation_xyzw: [f32; 4],
        translation_xyz: [f32; 3],
    ) -> openxr::Posef {
        openxr::Posef {
            orientation: openxr::Quaternionf {
                x: rotation_xyzw[0],
                y: rotation_xyzw[1],
                z: rotation_xyzw[2],
                w: rotation_xyzw[3],
            },
            position: openxr::Vector3f {
                x: translation_xyz[0],
                y: translation_xyz[1],
                z: translation_xyz[2],
            },
        }
    }

    fn derive_head_pose(views: &[openxr::View]) -> Option<openxr::Posef> {
        match views {
            [] => None,
            [view] => Some(view.pose),
            [left, right, ..] => {
                let left_q = Self::quat_from_posef(left.pose);
                let right_q = Self::quat_from_posef(right.pose);
                let center_q = Self::quat_nlerp(left_q, right_q, 0.5);
                let center_t = [
                    0.5 * (left.pose.position.x + right.pose.position.x),
                    0.5 * (left.pose.position.y + right.pose.position.y),
                    0.5 * (left.pose.position.z + right.pose.position.z),
                ];
                Some(Self::pose_from_quat_translation(center_q, center_t))
            }
        }
    }

    fn quat_angle_degrees(a: [f32; 4], b: [f32; 4]) -> f32 {
        let a = Self::quat_normalize(a);
        let b = Self::quat_normalize(b);
        let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3])
            .abs()
            .clamp(0.0, 1.0);
        (2.0 * dot.acos()).to_degrees()
    }

    fn rolling_avg(window: &VecDeque<f32>) -> f32 {
        if window.is_empty() {
            0.0
        } else {
            window.iter().copied().sum::<f32>() / window.len() as f32
        }
    }

    fn rolling_max(window: &VecDeque<f32>) -> f32 {
        window.iter().copied().fold(0.0, f32::max)
    }

    fn update_hand_rotation_debug(
        debug_state: &mut HandRotationDebugState,
        hand: ControllerHand,
        _joint: Option<openxr::HandJointEXT>,
        pose: openxr::Posef,
    ) {
        if !Self::debug_hand_rotation_enabled() {
            return;
        }

        let quat = Self::quat_from_posef(pose);
        let window_len = Self::debug_hand_rotation_window_len();
        let debug = match hand {
            ControllerHand::Left => &mut debug_state.left,
            ControllerHand::Right => &mut debug_state.right,
        };

        let step_deg = debug
            .previous_quat_xyzw
            .map(|previous| Self::quat_angle_degrees(previous, quat))
            .unwrap_or(0.0);
        debug.previous_quat_xyzw = Some(quat);

        if debug.step_deg.len() >= window_len {
            let _ = debug.step_deg.pop_front();
        }
        debug.step_deg.push_back(step_deg);
        debug.sample_count += 1;

        let _ = window_len;
    }

    fn preferred_pose(
        controllers: &ControllerPoseCache,
        hands: &HandRootPoseCache,
        hand: ControllerHand,
        pose: ControllerPoseKind,
    ) -> (Option<openxr::Posef>, ControllerPoseSource) {
        let (hand_root, hand_data_source) = match hand {
            ControllerHand::Left => (hands.left_root, hands.left_data_source),
            ControllerHand::Right => (hands.right_root, hands.right_data_source),
        };
        let controller_pose = match (hand, pose) {
            (ControllerHand::Left, ControllerPoseKind::Aim) => {
                controllers.left_aim.map(|sample| sample.pose)
            }
            (ControllerHand::Right, ControllerPoseKind::Aim) => {
                controllers.right_aim.map(|sample| sample.pose)
            }
            (ControllerHand::Left, ControllerPoseKind::Grip) => {
                controllers.left_grip.map(|sample| sample.pose)
            }
            (ControllerHand::Right, ControllerPoseKind::Grip) => {
                controllers.right_grip.map(|sample| sample.pose)
            }
            (ControllerHand::Left, ControllerPoseKind::GripAim) => {
                grip_aim_pose(controllers.left_grip, controllers.left_aim)
            }
            (ControllerHand::Right, ControllerPoseKind::GripAim) => {
                grip_aim_pose(controllers.right_grip, controllers.right_aim)
            }
        };

        let source = match pose {
            ControllerPoseKind::Aim => ControllerPoseSource::ControllerAim,
            ControllerPoseKind::Grip => ControllerPoseSource::ControllerGrip,
            ControllerPoseKind::GripAim => ControllerPoseSource::ControllerGripAim,
        };
        select_hand_pose(controller_pose, source, hand_root, hand_data_source)
    }

    fn transform_child_of(world: &World, component: ComponentId) -> Option<ComponentId> {
        world.children_of(component).iter().copied().find(|&cid| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(cid)
                .is_some()
        })
    }

    fn transform_parent_world(world: &World, transform_cid: ComponentId) -> [[f32; 4]; 4] {
        world
            .parent_of(transform_cid)
            .and_then(|parent| TransformSystem::world_model(world, parent))
            .unwrap_or([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
    }

    fn input_xr_ancestor(world: &World, cid: ComponentId) -> Option<ComponentId> {
        let mut cur = cid;
        loop {
            if world
                .get_component_by_id_as::<InputXRComponent>(cur)
                .is_some()
            {
                return Some(cur);
            }
            let Some(parent) = world.parent_of(cur) else {
                return None;
            };
            cur = parent;
        }
    }

    fn xr_rig_origin_world(world: &World, visuals: &VisualWorld) -> [[f32; 4]; 4] {
        let Some(camera_cid) = visuals
            .active_xr_camera()
            .or_else(|| Self::first_enabled_camera_xr(world))
        else {
            return [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
        };

        if let Some(input_xr_cid) = Self::input_xr_ancestor(world, camera_cid) {
            if let Some(driven_transform) = Self::transform_child_of(world, input_xr_cid) {
                return Self::transform_parent_world(world, driven_transform);
            }
        }

        TransformSystem::world_model(world, camera_cid).unwrap_or([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    fn corrected_head_pose(pose: openxr::Posef, correction: f32) -> [[f32; 4]; 4] {
        let mut matrix = Self::mat4_from_pose(pose);
        matrix[3][1] -= correction;
        matrix
    }

    fn world_from_stage(head_world: [[f32; 4]; 4], raw_head: openxr::Posef) -> [[f32; 4]; 4] {
        Self::mul_mat4(
            &head_world,
            &Self::invert_affine_transform(&Self::mat4_from_pose(raw_head)),
        )
    }

    /// CXR selects its InputXR's driven head, never an avatar camera wrapper.
    fn tracking_world_frame(
        world: &World,
        visuals: &VisualWorld,
        raw_head: openxr::Posef,
    ) -> Option<[[f32; 4]; 4]> {
        let camera = visuals
            .active_xr_camera()
            .or_else(|| Self::first_enabled_camera_xr(world))?;
        if !world
            .get_component_by_id_as::<CameraXRComponent>(camera)?
            .enabled
        {
            return None;
        }
        let Some(input) = Self::input_xr_ancestor(world, camera) else {
            // Preserve the authored-origin behavior for legacy CXR without InputXR.
            return Some(Self::xr_rig_origin_world(world, visuals));
        };
        let config = world.get_component_by_id_as::<InputXRComponent>(input)?;
        if !config.enabled || !config.pose_valid {
            return None;
        }
        let head = Self::transform_child_of(world, input)?;
        let head_world = TransformSystem::world_model(world, head)?;
        Some(Self::world_from_stage(head_world, raw_head))
    }

    /// Refresh eyes after ancestor/contact changes using this frame's raw eye offsets.
    pub fn publish_eye_cameras(&self, world: &World, visuals: &mut VisualWorld) -> bool {
        if let Some(frame) = self.external_frame {
            let eyes = frame.views.as_ref().and_then(|views| {
                Self::eye_cameras(world, visuals, Self::derive_head_pose(views)?, views)
            });
            if let Some(eyes) = eyes {
                visuals.set_xr_camera(eyes);
                return true;
            }
            visuals.set_xr_camera(Vec::new());
            return false;
        }
        let Some(sess) = self.state.as_ref().and_then(|state| state.session.as_ref()) else {
            return false;
        };
        let eyes = sess.prepared_frame.as_ref().and_then(|frame| {
            Self::eye_cameras(world, visuals, sess.head_pose_cache?, &frame.views)
        });
        let Some(eyes) = eyes else {
            // Tracking loss or a disabled/removed driver must not publish stale eyes.
            visuals.set_xr_camera(Vec::new());
            return false;
        };
        visuals.set_xr_camera(eyes);
        true
    }

    fn eye_cameras(
        world: &World,
        visuals: &VisualWorld,
        raw_head: openxr::Posef,
        views: &[openxr::View],
    ) -> Option<Vec<CameraData>> {
        let world_from_stage = Self::tracking_world_frame(world, visuals, raw_head)?;
        Some(
            views
                .iter()
                .map(|v| {
                    let world_from_eye =
                        Self::mul_mat4(&world_from_stage, &Self::mat4_from_pose(v.pose));
                    CameraData {
                        view: Self::invert_affine_transform(&world_from_eye),
                        proj: Self::proj_from_fov_rh_zo(
                            v.fov,
                            CameraXRComponent::DEFAULT_Z_NEAR,
                            CameraXRComponent::DEFAULT_Z_FAR,
                        ),
                        transform: Self::transform_from_matrix_world(world_from_eye),
                    }
                })
                .collect(),
        )
    }

    fn drive_head_pose(
        world: &mut World,
        input_xr_cid: ComponentId,
        head_pose: openxr::Posef,
        correction: f32,
        emit: &mut dyn SignalEmitter,
    ) {
        let Some(tcid) = Self::transform_child_of(world, input_xr_cid) else {
            return;
        };

        let parent_world = Self::transform_parent_world(world, tcid);
        let world_from_head = Self::mul_mat4(
            &parent_world,
            &Self::corrected_head_pose(head_pose, correction),
        );
        let desired_world_pos = [
            world_from_head[3][0],
            world_from_head[3][1],
            world_from_head[3][2],
        ];
        let desired_world_rot = math::mat_to_quat(world_from_head);

        let local_translation = Self::world_to_local_translation(world, tcid, desired_world_pos);
        let parent_world_rot =
            Self::parent_world_rotation_quat(world, tcid).unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let local_rotation =
            math::quat_mul(math::quat_conjugate(parent_world_rot), desired_world_rot);

        let Some(t) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(tcid)
        else {
            return;
        };

        t.transform.translation = local_translation;
        t.transform.rotation = local_rotation;
        t.transform.recompute_model();
        // Hands consume this head frame before the queued propagation flush.
        t.transform.matrix_world = Self::mul_mat4(&parent_world, &t.transform.model);

        let transform = t.transform;
        emit.push_intent_now(
            tcid,
            IntentValue::UpdateTransform {
                component_id: tcid,
                translation: transform.translation,
                rotation_quat_xyzw: transform.rotation,
                scale: transform.scale,
            },
        );
        if let Some(input) = world.get_component_by_id_as_mut::<InputXRComponent>(input_xr_cid) {
            input.pose_valid = true;
        }
    }

    /// Hint: prefer this Vulkan `VkFormat` (as raw `u32`) when creating the OpenXR swapchain.
    ///
    /// Windowed startup uses this to match scene pipelines to the runtime's
    /// output. XR-only startup builds pipelines from the actual XR format.
    pub fn set_preferred_swapchain_format(&mut self, format: u32) {
        self.preferred_swapchain_format = Some(format);
    }

    /// Returns the Vulkan instance/device extensions required by the active OpenXR runtime.
    ///
    /// This uses the `XR_KHR_vulkan_enable` query APIs (space-delimited strings). Even when the
    /// runtime supports `XR_KHR_vulkan_enable2`, SteamVR commonly still reports the required
    /// extension lists via these functions.
    pub fn required_vulkan_extensions(&self) -> Option<(Vec<String>, Vec<String>)> {
        let state = self.state.as_ref()?;

        let instance_exts = state
            .instance
            .vulkan_legacy_instance_extensions(state.system)
            .ok()?;
        let device_exts = state
            .instance
            .vulkan_legacy_device_extensions(state.system)
            .ok()?;

        let split = |s: String| -> Vec<String> {
            s.split_whitespace()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect()
        };

        Some((split(instance_exts), split(device_exts)))
    }

    /// Clone the initialized runtime handles for Vulkan bootstrap. Query its
    /// required GPU before creating a device, not only when creating a session.
    pub(crate) fn vulkan_runtime(&self) -> Option<(openxr::Instance, openxr::SystemId)> {
        let state = self.state.as_ref()?;
        Some((state.instance.clone(), state.system))
    }

    pub(crate) fn swapchain_format(&self) -> Option<u32> {
        Some(self.state.as_ref()?.session.as_ref()?.xr_swapchain.format())
    }

    /// Whether the XR-only host should keep driving frames. STOPPING is not
    /// terminal: the runtime can subsequently send READY and restart a session.
    pub fn runtime_exit_requested(&self) -> bool {
        self.runtime_exit_requested
    }

    pub fn session_running(&self) -> bool {
        self.state
            .as_ref()
            .and_then(|state| state.session.as_ref())
            .is_some_and(|session| session.running)
    }

    pub fn render_failed(&self) -> bool {
        self.state
            .as_ref()
            .and_then(|state| state.session.as_ref())
            .is_some_and(|session| session.render_failed)
    }

    pub fn set_vulkan_graphics(&mut self, gfx: XrVulkanGraphics) {
        self.vulkan_graphics = Some(gfx);

        // If OpenXR is already initialized, opportunistically create the session now.
        let Some(state) = self.state.as_mut() else {
            return;
        };

        if state.session.is_none() {
            if let Err(err) = Self::try_init_session(state, gfx, self.preferred_swapchain_format) {
                eprintln!("[OpenXR] Session init failed: {err}");
                self.last_init_error = Some(err);
            }
        }
    }

    pub fn initialize_runtime(&mut self) -> Result<(), String> {
        if self.state.is_some() {
            return Ok(());
        }

        match Self::try_init_openxr() {
            Ok(state) => {
                println!("[OpenXR] Initialized.");
                self.state = Some(state);
                self.last_init_error = None;
                self.runtime_exit_requested = false;

                if let (Some(state), Some(gfx)) = (self.state.as_mut(), self.vulkan_graphics) {
                    if state.session.is_none() {
                        if let Err(err) =
                            Self::try_init_session(state, gfx, self.preferred_swapchain_format)
                        {
                            eprintln!("[OpenXR] Session init failed: {err}");
                            self.last_init_error = Some(err.clone());
                            return Err(err);
                        }
                    }
                }

                Ok(())
            }
            Err(err) => {
                eprintln!("[OpenXR] Init failed: {err}");
                self.last_init_error = Some(err.clone());
                Err(err)
            }
        }
    }

    pub fn retry_runtime(&mut self) -> Result<(), String> {
        if let Some(state) = self.state.as_mut() {
            if state.session.is_some() {
                return Ok(());
            }
            let gfx = self
                .vulkan_graphics
                .ok_or_else(|| "Vulkan graphics are not initialized yet".to_string())?;
            return Self::try_init_session(state, gfx, self.preferred_swapchain_format).map_err(
                |error| {
                    self.last_init_error = Some(error.clone());
                    error
                },
            );
        }
        self.initialize_runtime()
    }

    pub fn register_xr(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        let Some(cfg) = world.get_component_by_id_as::<XrComponent>(component) else {
            return;
        };

        if !cfg.enabled {
            return;
        }

        if self.state.is_some() || self.external_frame.is_some() {
            return;
        }

        let _ = self.initialize_runtime();
    }

    pub fn register_controller_xr(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.controller_components.insert(component);
        self.controller_pose_source_last_logged.remove(&component);
        if world
            .get_component_by_id_as::<XRHandComponent>(component)
            .is_some_and(|config| config.enabled)
            && Self::transform_child_of(world, component).is_none()
        {
            eprintln!(
                "[XRHand] {component:?} has no direct Transform child to receive tracking; use XRHand.new(...) {{ T {{ Pointer {{}} }} }}. No avatar is required."
            );
        }
    }

    pub fn register_input_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.input_xr_components.insert(component);
    }

    pub fn remove_controller_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.controller_components.remove(&component);
        self.controller_pose_source_last_logged.remove(&component);
    }

    pub fn remove_input_xr(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        component: ComponentId,
    ) {
        self.input_xr_components.remove(&component);
    }

    fn pump_events(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        // Drain events; for now we just print key session state transitions.
        loop {
            let evt = match state.instance.poll_event(&mut state.events) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[OpenXR] poll_event error: {e:?}");
                    return;
                }
            };

            let Some(evt) = evt else {
                break;
            };

            match evt {
                openxr::Event::InstanceLossPending(_) => {
                    self.runtime_exit_requested = true;
                    self.hand_tracking_frame = None;
                    self.hand_tracking_epoch = self.hand_tracking_epoch.wrapping_add(1);
                    eprintln!("[OpenXR] Event: InstanceLossPending");
                }
                openxr::Event::SessionStateChanged(e) => {
                    println!("[OpenXR] Event: SessionStateChanged -> {:?}", e.state());

                    if let Some(sess) = state.session.as_mut() {
                        sess.current_state = e.state();
                        match e.state() {
                            openxr::SessionState::READY => {
                                if !sess.running {
                                    if let Err(err) = sess.session.begin(state.view_type) {
                                        eprintln!("[OpenXR] session.begin failed: {err:?}");
                                    } else {
                                        sess.running = true;
                                        self.hand_tracking_epoch =
                                            self.hand_tracking_epoch.wrapping_add(1);
                                    }
                                }
                            }
                            openxr::SessionState::STOPPING => {
                                self.hand_tracking_frame = None;
                                self.hand_tracking_epoch = self.hand_tracking_epoch.wrapping_add(1);
                                if sess.running {
                                    if let Err(err) = sess.session.end() {
                                        eprintln!("[OpenXR] session.end failed: {err:?}");
                                    }
                                    sess.running = false;
                                }
                            }
                            openxr::SessionState::EXITING | openxr::SessionState::LOSS_PENDING => {
                                self.runtime_exit_requested = true;
                                sess.running = false;
                                self.hand_tracking_frame = None;
                                self.hand_tracking_epoch = self.hand_tracking_epoch.wrapping_add(1);
                            }
                            _ => {}
                        }
                    }
                }
                openxr::Event::EventsLost(e) => {
                    eprintln!("[OpenXR] Event: EventsLost ({})", e.lost_event_count());
                }
                openxr::Event::ReferenceSpaceChangePending(_) => {
                    self.hand_tracking_frame = None;
                    self.hand_tracking_epoch = self.hand_tracking_epoch.wrapping_add(1);
                }
                _ => {
                    // Too noisy to print everything by default.
                }
            }
        }
    }

    fn drive_controller_pose(
        world: &mut World,
        controller_cid: ComponentId,
        pose: openxr::Posef,
        rig_world: [[f32; 4]; 4],
        emit: &mut dyn SignalEmitter,
    ) -> bool {
        // Semantics: ControllerXRComponent drives a TransformComponent child, if present.
        let Some(tcid) = Self::transform_child_of(world, controller_cid) else {
            return false;
        };

        // Pose is in OpenXR reference space; convert to engine world by applying rig transform.
        let world_from_controller = Self::mul_mat4(&rig_world, &Self::mat4_from_pose(pose));
        let desired_world_pos = [
            world_from_controller[3][0],
            world_from_controller[3][1],
            world_from_controller[3][2],
        ];
        let desired_world_rot = math::mat_to_quat(world_from_controller);

        let local_translation = Self::world_to_local_translation(world, tcid, desired_world_pos);
        let parent_world_rot =
            Self::parent_world_rotation_quat(world, tcid).unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let local_rotation =
            math::quat_mul(math::quat_conjugate(parent_world_rot), desired_world_rot);

        let Some(t) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::TransformComponent>(tcid)
        else {
            return false;
        };

        // Convert world-space target into local-space values relative to the nearest parent
        // transform above `tcid` (if any), matching how transform chains are composed.
        t.transform.translation = local_translation;
        t.transform.rotation = local_rotation;
        t.transform.recompute_model();

        let transform = t.transform;
        emit.push_intent_now(
            tcid,
            IntentValue::UpdateTransform {
                component_id: tcid,
                translation: transform.translation,
                rotation_quat_xyzw: transform.rotation,
                scale: transform.scale,
            },
        );
        if let Some(controller) =
            world.get_component_by_id_as_mut::<XRHandComponent>(controller_cid)
        {
            controller.pose_valid = true;
        }
        true
    }

    fn world_to_local_translation(
        world: &World,
        transform_cid: ComponentId,
        desired_world: [f32; 3],
    ) -> [f32; 3] {
        let mut cur = transform_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
            {
                if let Some(inv) = math::mat4_inverse(t.transform.matrix_world) {
                    let p_local = math::mat4_mul_vec4(
                        inv,
                        [desired_world[0], desired_world[1], desired_world[2], 1.0],
                    );
                    return [p_local[0], p_local[1], p_local[2]];
                }
                break;
            }
            cur = parent;
        }

        desired_world
    }

    fn parent_world_rotation_quat(world: &World, transform_cid: ComponentId) -> Option<[f32; 4]> {
        let mut cur = transform_cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world
                .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(parent)
            {
                return Some(math::mat_to_quat(t.transform.matrix_world));
            }
            cur = parent;
        }
        None
    }

    fn try_init_hand_tracking(
        session: &openxr::Session<openxr::Vulkan>,
    ) -> Result<HandTrackingState, String> {
        let left = Self::create_hand_tracker(session, openxr::HandEXT::LEFT)
            .map_err(|e| format!("create_hand_tracker(left): {e:?}"))?;
        let right = Self::create_hand_tracker(session, openxr::HandEXT::RIGHT)
            .map_err(|e| format!("create_hand_tracker(right): {e:?}"))?;

        Ok(HandTrackingState { left, right })
    }

    fn create_hand_tracker(
        session: &openxr::Session<openxr::Vulkan>,
        hand: openxr::HandEXT,
    ) -> Result<openxr::HandTracker, openxr::sys::Result> {
        let extensions = session.instance().exts();
        if extensions.ext_hand_tracking_data_source.is_none() {
            return session.create_hand_tracker(hand);
        }
        let tracking = extensions
            .ext_hand_tracking
            .as_ref()
            .ok_or(openxr::sys::Result::ERROR_EXTENSION_NOT_PRESENT)?;
        let mut sources = [
            openxr::HandTrackingDataSourceEXT::UNOBSTRUCTED,
            openxr::HandTrackingDataSourceEXT::CONTROLLER,
        ];
        let source_info = openxr::sys::HandTrackingDataSourceInfoEXT {
            ty: openxr::sys::HandTrackingDataSourceInfoEXT::TYPE,
            next: std::ptr::null(),
            requested_data_source_count: sources.len() as u32,
            requested_data_sources: sources.as_mut_ptr(),
        };
        let info = openxr::sys::HandTrackerCreateInfoEXT {
            ty: openxr::sys::HandTrackerCreateInfoEXT::TYPE,
            next: (&source_info as *const openxr::sys::HandTrackingDataSourceInfoEXT).cast(),
            hand,
            hand_joint_set: openxr::sys::HandJointSetEXT::DEFAULT,
        };
        let mut handle = openxr::sys::HandTrackerEXT::NULL;
        // SAFETY: Both enabled extensions belong to this session's instance. The
        // complete input chain and its source array live through this synchronous call.
        let result =
            unsafe { (tracking.create_hand_tracker)(session.as_raw(), &info, &mut handle) };
        if result.into_raw() < 0 {
            return Err(result);
        }
        // SAFETY: The successful call created this owned handle for this session.
        Ok(unsafe { openxr::HandTracker::from_raw(session, handle) })
    }

    fn locate_hand_sample(
        reference_space: &openxr::Space,
        tracker: &openxr::HandTracker,
        time: openxr::Time,
    ) -> Result<Option<HandJointSample>, openxr::sys::Result> {
        let extensions = reference_space.instance().exts();
        if extensions.ext_hand_tracking_data_source.is_none() {
            return reference_space
                .locate_hand_joints(tracker, time)
                .map(|joints| {
                    joints.map(|joints| HandJointSample {
                        joints,
                        data_source: None,
                    })
                });
        }
        let tracking = extensions
            .ext_hand_tracking
            .as_ref()
            .ok_or(openxr::sys::Result::ERROR_EXTENSION_NOT_PRESENT)?;
        let info = openxr::sys::HandJointsLocateInfoEXT {
            ty: openxr::sys::HandJointsLocateInfoEXT::TYPE,
            next: std::ptr::null(),
            base_space: reference_space.as_raw(),
            time,
        };
        let mut source = openxr::sys::HandTrackingDataSourceStateEXT {
            ty: openxr::sys::HandTrackingDataSourceStateEXT::TYPE,
            next: std::ptr::null_mut(),
            is_active: false.into(),
            data_source: openxr::HandTrackingDataSourceEXT::CONTROLLER,
        };
        let mut joints = [openxr::HandJointLocationEXT::default(); openxr::HAND_JOINT_COUNT];
        let mut locations = openxr::sys::HandJointLocationsEXT {
            ty: openxr::sys::HandJointLocationsEXT::TYPE,
            next: (&mut source as *mut openxr::sys::HandTrackingDataSourceStateEXT).cast(),
            is_active: false.into(),
            joint_count: joints.len() as u32,
            joint_locations: joints.as_mut_ptr(),
        };
        // SAFETY: The private caller supplies this session's space and tracker,
        // created with the source-info chain above. Output storage lives through
        // the call; all 26 default-set joints and the enabled source chain are writable.
        let result =
            unsafe { (tracking.locate_hand_joints)(tracker.as_raw(), &info, &mut locations) };
        if result.into_raw() < 0 {
            return Err(result);
        }
        Ok(bool::from(locations.is_active).then_some(HandJointSample {
            joints,
            data_source: bool::from(source.is_active).then_some(source.data_source),
        }))
    }

    fn valid_pose_from_hand_joint(joint: &openxr::HandJointLocationEXT) -> Option<openxr::Posef> {
        let flags = joint.location_flags;
        if flags.contains(openxr::SpaceLocationFlags::POSITION_VALID)
            && flags.contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
        {
            Some(joint.pose)
        } else {
            None
        }
    }

    fn valid_position_from_hand_joint(joint: &openxr::HandJointLocationEXT) -> Option<[f32; 3]> {
        joint
            .location_flags
            .contains(openxr::SpaceLocationFlags::POSITION_VALID)
            .then_some([
                joint.pose.position.x,
                joint.pose.position.y,
                joint.pose.position.z,
            ])
            .filter(|position| position.iter().all(|value| value.is_finite()))
    }

    fn valid_orientation_from_hand_joint(joint: &openxr::HandJointLocationEXT) -> Option<[f32; 4]> {
        if !joint
            .location_flags
            .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
        {
            return None;
        }
        let q = Self::quat_from_posef(joint.pose);
        let length_squared = q.iter().map(|value| value * value).sum::<f32>();
        if !length_squared.is_finite() || length_squared <= 1e-12 {
            return None;
        }
        let inverse_length = length_squared.sqrt().recip();
        Some(q.map(|value| value * inverse_length))
    }

    fn canonical_hand_orientation(
        joints: &openxr::HandJointLocations,
        hand: ControllerHand,
    ) -> Option<openxr::Quaternionf> {
        for joint in [openxr::HandJointEXT::PALM, openxr::HandJointEXT::WRIST] {
            let Some(joint_q) = Self::valid_orientation_from_hand_joint(&joints[joint]) else {
                continue;
            };
            // OpenXR -Z points along the hand and +Y out of its back. Our avatar
            // basis instead uses +Y across the palm, from little to index finger:
            // native +X on the left hand and -X on the right. Convert in joint-local
            // space, preserving the runtime's forward and palm normal through roll.
            let half_sqrt = std::f32::consts::FRAC_1_SQRT_2;
            let canonical_to_joint = [
                0.0,
                0.0,
                match hand {
                    ControllerHand::Left => -half_sqrt,
                    ControllerHand::Right => half_sqrt,
                },
                half_sqrt,
            ];
            let q = math::quat_mul(joint_q, canonical_to_joint);
            return Some(openxr::Quaternionf {
                x: q[0],
                y: q[1],
                z: q[2],
                w: q[3],
            });
        }
        Self::landmark_hand_orientation(joints)
    }

    fn landmark_hand_orientation(
        joints: &openxr::HandJointLocations,
    ) -> Option<openxr::Quaternionf> {
        let position = |joint| Self::valid_position_from_hand_joint(&joints[joint]);
        // Follow the metacarpal within the palm. Proximal-to-distal follows finger
        // curl and can pitch the entire wrist by 90 degrees when making a fist.
        let forward_raw = math::vec3_sub(
            position(openxr::HandJointEXT::MIDDLE_PROXIMAL)?,
            position(openxr::HandJointEXT::MIDDLE_METACARPAL)?,
        );
        if math::vec3_len(forward_raw) <= 1e-6 {
            return None;
        }
        let forward = math::vec3_normalize(forward_raw);
        // Canonical +Y is palm width, matching the avatar's retained hand basis;
        // it is deliberately different from raw OpenXR joint +Y (back of hand).
        let up_raw = math::vec3_sub(
            position(openxr::HandJointEXT::INDEX_PROXIMAL)?,
            position(openxr::HandJointEXT::LITTLE_PROXIMAL)?,
        );
        if math::vec3_len(up_raw) <= 1e-6 {
            return None;
        }
        let projected_up = math::vec3_sub(
            up_raw,
            math::vec3_scale(forward, math::vec3_dot(up_raw, forward)),
        );
        if math::vec3_len(projected_up) <= 1e-6 {
            return None;
        }
        let up = math::vec3_normalize(projected_up);
        let z = math::vec3_scale(forward, -1.0);
        let right = math::vec3_normalize(math::vec3_cross(up, z));
        let up = math::vec3_normalize(math::vec3_cross(z, right));
        let q = math::mat_to_quat([
            [right[0], right[1], right[2], 0.0],
            [up[0], up[1], up[2], 0.0],
            [z[0], z[1], z[2], 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        Some(openxr::Quaternionf {
            x: q[0],
            y: q[1],
            z: q[2],
            w: q[3],
        })
    }

    fn select_hand_root_pose(
        joints: &openxr::HandJointLocations,
        hand: ControllerHand,
    ) -> Option<(openxr::Posef, openxr::HandJointEXT)> {
        let orientation = Self::canonical_hand_orientation(joints, hand)?;
        for joint in [openxr::HandJointEXT::WRIST, openxr::HandJointEXT::PALM] {
            if let Some(position) = Self::valid_position_from_hand_joint(&joints[joint]) {
                return Some((
                    openxr::Posef {
                        orientation,
                        position: openxr::Vector3f {
                            x: position[0],
                            y: position[1],
                            z: position[2],
                        },
                    },
                    joint,
                ));
            }
        }
        None
    }

    fn normalize_hand_sample(sample: &HandJointSample, hand: ControllerHand) -> TrackedHand {
        let joints = std::array::from_fn(|index| {
            let joint = &sample.joints[openxr::HandJointEXT::from_raw(index as i32)];
            let flags = joint.location_flags;
            let position_valid = flags.contains(openxr::SpaceLocationFlags::POSITION_VALID);
            HandJointLocation {
                position: [
                    joint.pose.position.x,
                    joint.pose.position.y,
                    joint.pose.position.z,
                ],
                orientation: [
                    joint.pose.orientation.x,
                    joint.pose.orientation.y,
                    joint.pose.orientation.z,
                    joint.pose.orientation.w,
                ],
                position_valid,
                orientation_valid: flags.contains(openxr::SpaceLocationFlags::ORIENTATION_VALID),
                position_tracked: flags.contains(openxr::SpaceLocationFlags::POSITION_TRACKED),
                orientation_tracked: flags
                    .contains(openxr::SpaceLocationFlags::ORIENTATION_TRACKED),
                radius_meters: (position_valid && joint.radius.is_finite() && joint.radius > 0.0)
                    .then_some(joint.radius),
                orientation_provenance: HandOrientationProvenance::NativeOpenXr,
            }
        });
        let root_position = [HandJoint::Wrist, HandJoint::Palm]
            .into_iter()
            .find_map(|joint| joints[joint as usize].valid_position().map(|p| (joint, p)));
        let canonical_orientation =
            Self::canonical_hand_orientation(&sample.joints, hand).map(|q| [q.x, q.y, q.z, q.w]);
        TrackedHand {
            active: true,
            data_source: match sample.data_source {
                Some(openxr::HandTrackingDataSourceEXT::UNOBSTRUCTED) => {
                    HandDataSource::Unobstructed
                }
                Some(openxr::HandTrackingDataSourceEXT::CONTROLLER) => HandDataSource::Controller,
                _ => HandDataSource::Unknown,
            },
            joints,
            root_position,
            canonical_orientation,
            canonical_orientation_provenance: if Self::valid_orientation_from_hand_joint(
                &sample.joints[openxr::HandJointEXT::PALM],
            )
            .is_some()
                || Self::valid_orientation_from_hand_joint(
                    &sample.joints[openxr::HandJointEXT::WRIST],
                )
                .is_some()
            {
                HandOrientationProvenance::NativeOpenXr
            } else if canonical_orientation.is_some() {
                HandOrientationProvenance::LandmarkDerived
            } else {
                HandOrientationProvenance::Absent
            },
        }
    }

    fn log_hand_frame(
        hand: ControllerHand,
        joints: &openxr::HandJointLocations,
        root: Option<(openxr::Posef, openxr::HandJointEXT)>,
        data_source: Option<openxr::HandTrackingDataSourceEXT>,
        generation: u64,
    ) {
        let palm_q = Self::valid_orientation_from_hand_joint(&joints[openxr::HandJointEXT::PALM]);
        let wrist_q = Self::valid_orientation_from_hand_joint(&joints[openxr::HandJointEXT::WRIST]);
        let orientation_joint = if palm_q.is_some() {
            Some(openxr::HandJointEXT::PALM)
        } else if wrist_q.is_some() {
            Some(openxr::HandJointEXT::WRIST)
        } else {
            None // Landmark fallback.
        };
        let selected_q = root.map(|(pose, _)| Self::quat_from_posef(pose));
        let landmark_delta_axis_angle_deg = selected_q.and_then(|selected| {
            let landmark = Self::landmark_hand_orientation(joints)?;
            let landmark_q = [landmark.x, landmark.y, landmark.z, landmark.w];
            let (axis, radians) = math::quat_to_axis_angle(math::quat_mul(
                math::quat_conjugate(selected),
                landmark_q,
            ));
            Some((axis, radians.to_degrees()))
        });
        eprintln!(
            "[OpenXR][hand-frame] generation={generation} hand={hand:?} data_source={data_source:?} position_joint={:?} orientation_joint={orientation_joint:?} palm_q={palm_q:?} wrist_q={wrist_q:?} canonical_q={selected_q:?} landmark_delta_axis_angle_deg={landmark_delta_axis_angle_deg:?}",
            root.map(|(_, joint)| joint),
        );
    }

    fn wrist_palm_delta_deg(joints: &openxr::HandJointLocations) -> Option<u16> {
        let wrist = Self::valid_pose_from_hand_joint(&joints[openxr::HandJointEXT::WRIST])?;
        let palm = Self::valid_pose_from_hand_joint(&joints[openxr::HandJointEXT::PALM])?;
        let deg =
            Self::quat_angle_degrees(Self::quat_from_posef(wrist), Self::quat_from_posef(palm));
        Some(deg.round().clamp(0.0, u16::MAX as f32) as u16)
    }

    fn quantize_angle_deg(value: u16, bucket_size: u16) -> u16 {
        if bucket_size <= 1 {
            value
        } else {
            (value / bucket_size) * bucket_size
        }
    }

    fn hand_debug_state(
        joints: Option<&openxr::HandJointLocations>,
        root: Option<(openxr::Posef, openxr::HandJointEXT)>,
        previous_root_quat_xyzw: Option<[f32; 4]>,
    ) -> HandDebugState {
        let wrist_palm_delta_deg = joints
            .and_then(Self::wrist_palm_delta_deg)
            .map(|deg| Self::quantize_angle_deg(deg, 5));
        let root_joint = root.map(|(_, joint)| joint);
        let root_pose = root.map(|(pose, _)| pose);
        let wrist_step_deg = match (previous_root_quat_xyzw, root_pose) {
            (Some(previous), Some(pose)) => Some(
                Self::quat_angle_degrees(previous, Self::quat_from_posef(pose))
                    .round()
                    .clamp(0.0, u16::MAX as f32) as u16,
            ),
            _ => None,
        }
        .map(|deg| Self::quantize_angle_deg(deg, 2));
        HandDebugState {
            root_joint,
            wrist_palm_delta_deg,
            wrist_step_deg,
            wrist_step_spike: wrist_step_deg.is_some_and(|deg| deg >= 8),
        }
    }

    fn log_hand_debug_snapshot(
        sess: &mut OpenXRSessionState,
        left_joints: Option<&openxr::HandJointLocations>,
        right_joints: Option<&openxr::HandJointLocations>,
        left_root: Option<(openxr::Posef, openxr::HandJointEXT)>,
        right_root: Option<(openxr::Posef, openxr::HandJointEXT)>,
    ) {
        let snapshot = HandDebugSnapshot {
            left: Self::hand_debug_state(
                left_joints,
                left_root,
                sess.hand_rotation_debug.left.previous_quat_xyzw,
            ),
            right: Self::hand_debug_state(
                right_joints,
                right_root,
                sess.hand_rotation_debug.right.previous_quat_xyzw,
            ),
        };
        if sess.last_hand_debug_snapshot == Some(snapshot) {
            return;
        }
        sess.last_hand_debug_snapshot = Some(snapshot);
        if openxr_debug_enabled() {
            eprintln!(
                "[OpenXR][hand] left root={:?} wrist_palm_delta_deg={:?} wrist_step_deg={:?} spike={} | right root={:?} wrist_palm_delta_deg={:?} wrist_step_deg={:?} spike={}",
                snapshot.left.root_joint,
                snapshot.left.wrist_palm_delta_deg,
                snapshot.left.wrist_step_deg,
                snapshot.left.wrist_step_spike,
                snapshot.right.root_joint,
                snapshot.right.wrist_palm_delta_deg,
                snapshot.right.wrist_step_deg,
                snapshot.right.wrist_step_spike,
            );
        }
    }

    /// Load OpenXR, initializing the Android loader before any enumeration.
    /// Android hosts must first initialize their activity/JVM context (for
    /// example through android-activity) and keep it alive while using OpenXR.
    pub fn load_entry() -> Result<openxr::Entry, String> {
        // Prefer dynamically loading the OpenXR loader. This keeps us from requiring
        // special linker setup and matches typical Linux setups.
        let entry = unsafe { openxr::Entry::load().map_err(|e| format!("Entry::load: {e:?}"))? };
        #[cfg(target_os = "android")]
        entry
            .initialize_android_loader()
            .map_err(|e| format!("xrInitializeLoaderKHR(Android): {e:?}"))?;
        Ok(entry)
    }

    fn try_init_openxr() -> Result<OpenXRState, String> {
        let entry = Self::load_entry()?;

        let available_extensions = entry
            .enumerate_extensions()
            .map_err(|e| format!("enumerate_extensions: {e:?}"))?;

        let app_info = openxr::ApplicationInfo {
            application_name: "Mittens",
            application_version: 1,
            engine_name: "Mittens",
            engine_version: 1,
            api_version: openxr::Version::new(1, 0, 0),
        };

        let mut extensions = openxr::ExtensionSet::default();
        #[cfg(target_os = "android")]
        {
            if !available_extensions.khr_android_create_instance {
                return Err("runtime lacks XR_KHR_android_create_instance".into());
            }
            extensions.khr_android_create_instance = true;
        }
        // Use the legacy Vulkan binding path for now (reuse an already-created VkInstance/VkDevice).
        // SteamVR can be stricter with XR_KHR_vulkan_enable2 unless you create Vulkan objects
        // via xrCreateVulkanInstanceKHR/xrCreateVulkanDeviceKHR.
        extensions.khr_vulkan_enable2 = false;
        // Needed for Vulkan session creation.
        extensions.khr_vulkan_enable = true;
        if available_extensions.ext_hand_tracking {
            extensions.ext_hand_tracking = true;
            extensions.ext_hand_tracking_data_source =
                available_extensions.ext_hand_tracking_data_source;
        }
        if available_extensions.ext_hand_interaction {
            extensions.ext_hand_interaction = true;
        }
        if available_extensions.htc_vive_focus3_controller_interaction {
            extensions.htc_vive_focus3_controller_interaction = true;
        }
        if available_extensions.htc_hand_interaction {
            extensions.htc_hand_interaction = true;
        }
        let layers: [&str; 0] = [];

        let instance = entry
            .create_instance(&app_info, &extensions, &layers)
            .map_err(|e| format!("create_instance: {e:?}"))?;

        println!(
            "[OpenXR] Enabled extensions: khr_vulkan_enable={}, ext_hand_tracking={}, ext_hand_tracking_data_source={}, ext_hand_interaction={}, htc_vive_focus3_controller_interaction={}, htc_hand_interaction={}",
            extensions.khr_vulkan_enable,
            extensions.ext_hand_tracking,
            extensions.ext_hand_tracking_data_source,
            extensions.ext_hand_interaction,
            extensions.htc_vive_focus3_controller_interaction,
            extensions.htc_hand_interaction,
        );

        // Best-effort runtime identification (helps debugging which OpenXR runtime is active).
        if let Ok(props) = instance.properties() {
            println!(
                "[OpenXR] Runtime: {} ({:?})",
                props.runtime_name, props.runtime_version
            );
        }

        let system = match instance.system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY) {
            Ok(system) => system,
            Err(openxr::sys::Result::ERROR_FORM_FACTOR_UNAVAILABLE) => {
                return Err(
                    "system(HMD): ERROR_FORM_FACTOR_UNAVAILABLE (no HMD detected / runtime not ready).\n\
                    Start an OpenXR runtime (e.g. SteamVR/Monado/ALVR) and ensure a headset is connected and the runtime is running before launching cat-engine.\n\
                    On Linux you can also check XR_RUNTIME_JSON points to an installed runtime manifest."
                        .to_string(),
                );
            }
            Err(e) => return Err(format!("system(HMD): {e:?}")),
        };

        // Pick a supported blend mode and stash common config.
        let view_type = openxr::ViewConfigurationType::PRIMARY_STEREO;
        let blend_mode = instance
            .enumerate_environment_blend_modes(system, view_type)
            .ok()
            .and_then(|m| m.first().copied())
            .unwrap_or(openxr::EnvironmentBlendMode::OPAQUE);

        Ok(OpenXRState {
            entry,
            instance,
            system,
            events: openxr::EventDataBuffer::new(),
            session: None,
            view_type,
            blend_mode,
        })
    }

    fn try_init_session(
        state: &mut OpenXRState,
        gfx: XrVulkanGraphics,
        preferred_swapchain_format: Option<u32>,
    ) -> Result<(), String> {
        // Log Vulkan version requirements (useful debugging).
        if let Ok(reqs) = state
            .instance
            .graphics_requirements::<openxr::Vulkan>(state.system)
        {
            println!(
                "[OpenXR] Vulkan requirements: min {:?}, max {:?}",
                reqs.min_api_version_supported, reqs.max_api_version_supported
            );
        }

        // SteamVR may validate that the VkPhysicalDevice matches the one it expects for the HMD.
        // If these differ (multi-GPU setups), create_session can fail.
        if let Ok(required_pd) = unsafe {
            state
                .instance
                .vulkan_graphics_device(state.system, gfx.vk_instance)
        } {
            if required_pd != gfx.vk_physical_device {
                return Err(format!(
                    "Vulkan physical device mismatch for OpenXR system.\n\
Engine/Vulkano physical device: {:?}\n\
OpenXR-required physical device: {:?}\n\
Fix: pick the OpenXR-required VkPhysicalDevice when creating the Vulkan device/queues.",
                    gfx.vk_physical_device, required_pd
                ));
            }
        }

        let info = openxr::vulkan::SessionCreateInfo {
            instance: gfx.vk_instance,
            physical_device: gfx.vk_physical_device,
            device: gfx.vk_device,
            queue_family_index: gfx.queue_family_index,
            queue_index: gfx.queue_index,
        };

        let (session, frame_waiter, frame_stream) = unsafe {
            state
                .instance
                .create_session::<openxr::Vulkan>(state.system, &info)
                .map_err(|e| {
                    let required_instance = state
                        .instance
                        .vulkan_legacy_instance_extensions(state.system)
                        .ok();
                    let required_device = state
                        .instance
                        .vulkan_legacy_device_extensions(state.system)
                        .ok();

                    let extra = match (required_instance, required_device) {
                        (Some(i), Some(d)) => format!(
                            "\n[OpenXR] Runtime-required Vulkan instance extensions: {i}\n[OpenXR] Runtime-required Vulkan device extensions: {d}"
                        ),
                        _ => String::new(),
                    };

                    format!(
                        "create_session(Vulkan): {e:?}.\n\
If this fails with Vulkan extension errors, the Vulkan instance/device created by Vulkano may be missing runtime-required extensions (from XR_KHR_vulkan_enable2)."
                    )
                    + extra.as_str()
                })?
        };

        let reference_space = session
            .create_reference_space(openxr::ReferenceSpaceType::STAGE, openxr::Posef::IDENTITY)
            .map_err(|e| format!("create_reference_space(STAGE): {e:?}"))?;

        let controller_input = match Self::try_init_controller_input(&state.instance, &session) {
            Ok(v) => {
                println!("[OpenXR] Controller input initialized.");
                Some(v)
            }
            Err(err) => {
                eprintln!("[OpenXR] Controller input init failed: {err}");
                None
            }
        };

        let hand_tracking = match state.instance.supports_hand_tracking(state.system) {
            Ok(v) => {
                println!("[OpenXR] Runtime supports hand tracking: {v}");
                if v {
                    match Self::try_init_hand_tracking(&session) {
                        Ok(trackers) => {
                            println!("[OpenXR] Hand tracking initialized.");
                            Some(trackers)
                        }
                        Err(err) => {
                            eprintln!("[OpenXR] Hand tracking init failed: {err}");
                            None
                        }
                    }
                } else {
                    None
                }
            }
            Err(e) => {
                eprintln!("[OpenXR] supports_hand_tracking query failed: {e:?}");
                None
            }
        };

        // SAFETY: the session was created from this exact graphics binding;
        // the engine retains its Vulkan device until after XR teardown.
        let xr_swapchain = unsafe {
            XRSwapchain::new(
                &state.instance,
                &session,
                state.system,
                state.view_type,
                preferred_swapchain_format,
                ash::vk::Device::from_raw(gfx.vk_device as usize as u64),
            )
        }?;

        let swapchain_image_initialized = vec![false; xr_swapchain.images().len()];

        // Minimal Vulkan command recording for XR bring-up: wrap the existing VkDevice via ash.
        let entry = unsafe { ash::Entry::load().map_err(|e| format!("ash::Entry::load: {e:?}"))? };

        let vk_instance = ash::vk::Instance::from_raw(gfx.vk_instance as usize as u64);
        let vk_device_handle = ash::vk::Device::from_raw(gfx.vk_device as usize as u64);

        let vk_instance = unsafe { ash::Instance::load(entry.static_fn(), vk_instance) };
        let vk_device = unsafe { ash::Device::load(vk_instance.fp_v1_0(), vk_device_handle) };
        // Use the exact queue exported by Vulkano. The completion fence must
        // follow every mirror/eye submission on this same queue.
        let vk_queue = ash::vk::Queue::from_raw(gfx.vk_queue as usize as u64);

        let vk_command_pool = unsafe {
            vk_device
                .create_command_pool(
                    &ash::vk::CommandPoolCreateInfo::default()
                        .queue_family_index(gfx.queue_family_index)
                        .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .map_err(|e| format!("create_command_pool: {e:?}"))?
        };

        let vk_command_buffer = unsafe {
            vk_device
                .allocate_command_buffers(
                    &ash::vk::CommandBufferAllocateInfo::default()
                        .command_pool(vk_command_pool)
                        .level(ash::vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .map_err(|e| format!("allocate_command_buffers: {e:?}"))?
                .into_iter()
                .next()
                .ok_or("allocate_command_buffers returned 0 command buffers")?
        };

        let vk_completion_fence = unsafe {
            vk_device
                .create_fence(&ash::vk::FenceCreateInfo::default(), None)
                .map_err(|e| format!("create_fence: {e:?}"))?
        };

        state.session = Some(OpenXRSessionState {
            session,
            frame_waiter,
            frame_stream,
            reference_space,
            running: false,
            render_failed: false,
            current_state: openxr::SessionState::IDLE,

            xr_swapchain,

            swapchain_image_initialized,

            did_log_format_mismatch: false,

            vk_device,
            vk_queue,
            vk_command_pool,
            vk_command_buffer,
            vk_completion_fence,
            vk_completion_fence_usable: true,

            hand_tracking,
            hand_root_pose_cache: HandRootPoseCache::default(),
            hand_rotation_debug: HandRotationDebugState::default(),
            last_hand_debug_snapshot: None,
            head_pose_cache: None,
            controller_input,
            controller_pose_cache: ControllerPoseCache::default(),
            controller_pose_generation: 0,
            prepared_frame: None,
        });

        println!("[OpenXR] Session created (Vulkan)");
        Ok(())
    }

    fn try_init_controller_input(
        instance: &openxr::Instance,
        session: &openxr::Session<openxr::Vulkan>,
    ) -> Result<ControllerInput, String> {
        let left = instance
            .string_to_path("/user/hand/left")
            .map_err(|e| format!("string_to_path(/user/hand/left): {e:?}"))?;
        let right = instance
            .string_to_path("/user/hand/right")
            .map_err(|e| format!("string_to_path(/user/hand/right): {e:?}"))?;

        let action_set = instance
            .create_action_set("mittens_engine", "Mittens", 0)
            .map_err(|e| format!("create_action_set: {e:?}"))?;
        let hand_subaction_paths = [left, right];

        let aim_pose = action_set
            .create_action::<openxr::Posef>("aim_pose", "Aim Pose", &hand_subaction_paths)
            .map_err(|e| format!("create_action(aim_pose): {e:?}"))?;
        let grip_pose = action_set
            .create_action::<openxr::Posef>("grip_pose", "Grip Pose", &hand_subaction_paths)
            .map_err(|e| format!("create_action(grip_pose): {e:?}"))?;
        let select = action_set
            .create_action::<bool>("select", "Select", &hand_subaction_paths)
            .map_err(|e| format!("create_action(select): {e:?}"))?;
        let left_stick_x = action_set
            .create_action::<f32>("left_stick_x", "Left Stick X", &hand_subaction_paths)
            .map_err(|e| format!("create_action(left_stick_x): {e:?}"))?;
        let left_stick_y = action_set
            .create_action::<f32>("left_stick_y", "Left Stick Y", &hand_subaction_paths)
            .map_err(|e| format!("create_action(left_stick_y): {e:?}"))?;
        let right_stick_x = action_set
            .create_action::<f32>("right_stick_x", "Right Stick X", &hand_subaction_paths)
            .map_err(|e| format!("create_action(right_stick_x): {e:?}"))?;
        let right_stick_y = action_set
            .create_action::<f32>("right_stick_y", "Right Stick Y", &hand_subaction_paths)
            .map_err(|e| format!("create_action(right_stick_y): {e:?}"))?;
        let trigger_value = action_set
            .create_action::<f32>("trigger_value", "Trigger Value", &hand_subaction_paths)
            .map_err(|e| format!("create_action(trigger_value): {e:?}"))?;
        let trigger_click = action_set
            .create_action::<bool>("trigger_click", "Trigger Click", &hand_subaction_paths)
            .map_err(|e| format!("create_action(trigger_click): {e:?}"))?;
        let grip_value = action_set
            .create_action::<f32>("grip_value", "Grip Value", &hand_subaction_paths)
            .map_err(|e| format!("create_action(grip_value): {e:?}"))?;
        let grip_click = action_set
            .create_action::<bool>("grip_click", "Grip Click", &hand_subaction_paths)
            .map_err(|e| format!("create_action(grip_click): {e:?}"))?;
        let button_a = action_set
            .create_action::<bool>("button_a", "Button A", &hand_subaction_paths)
            .map_err(|e| format!("create_action(button_a): {e:?}"))?;
        let button_b = action_set
            .create_action::<bool>("button_b", "Button B", &hand_subaction_paths)
            .map_err(|e| format!("create_action(button_b): {e:?}"))?;
        let button_x = action_set
            .create_action::<bool>("button_x", "Button X", &hand_subaction_paths)
            .map_err(|e| format!("create_action(button_x): {e:?}"))?;
        let button_y = action_set
            .create_action::<bool>("button_y", "Button Y", &hand_subaction_paths)
            .map_err(|e| format!("create_action(button_y): {e:?}"))?;

        let left_aim_space = aim_pose
            .create_space(session.clone(), left, openxr::Posef::IDENTITY)
            .map_err(|e| format!("aim_pose.create_space(left): {e:?}"))?;
        let right_aim_space = aim_pose
            .create_space(session.clone(), right, openxr::Posef::IDENTITY)
            .map_err(|e| format!("aim_pose.create_space(right): {e:?}"))?;
        let left_grip_space = grip_pose
            .create_space(session.clone(), left, openxr::Posef::IDENTITY)
            .map_err(|e| format!("grip_pose.create_space(left): {e:?}"))?;
        let right_grip_space = grip_pose
            .create_space(session.clone(), right, openxr::Posef::IDENTITY)
            .map_err(|e| format!("grip_pose.create_space(right): {e:?}"))?;

        // Suggest bindings before attaching the action set. Once attached, action sets
        // become immutable, and hiding suggestion failures makes profile bring-up
        // impossible to debug on SteamVR.
        let left_aim_path = instance
            .string_to_path("/user/hand/left/input/aim/pose")
            .map_err(|e| format!("string_to_path(left aim): {e:?}"))?;
        let right_aim_path = instance
            .string_to_path("/user/hand/right/input/aim/pose")
            .map_err(|e| format!("string_to_path(right aim): {e:?}"))?;
        let left_grip_path = instance
            .string_to_path("/user/hand/left/input/grip/pose")
            .map_err(|e| format!("string_to_path(left grip): {e:?}"))?;
        let right_grip_path = instance
            .string_to_path("/user/hand/right/input/grip/pose")
            .map_err(|e| format!("string_to_path(right grip): {e:?}"))?;

        // Pose bindings are common across all profiles.
        let pose_bindings = [
            openxr::Binding::new(&aim_pose, left_aim_path),
            openxr::Binding::new(&aim_pose, right_aim_path),
            openxr::Binding::new(&grip_pose, left_grip_path),
            openxr::Binding::new(&grip_pose, right_grip_path),
        ];

        let mut binding_reports = Vec::new();

        for spec in PROFILE_BINDING_SPECS {
            log_profile_binding_dump(spec);
            let Ok(profile) = instance.string_to_path(spec.profile) else {
                continue;
            };
            let mut report = ProfileBindingReport {
                profile_path: spec.profile.to_string(),
                suggestions: Vec::new(),
            };
            let mut accepted_bindings = Vec::new();
            for (binding_label, binding_path, binding) in [
                (
                    "aim_pose_left",
                    "/user/hand/left/input/aim/pose",
                    pose_bindings[0],
                ),
                (
                    "aim_pose_right",
                    "/user/hand/right/input/aim/pose",
                    pose_bindings[1],
                ),
                (
                    "grip_pose_left",
                    "/user/hand/left/input/grip/pose",
                    pose_bindings[2],
                ),
                (
                    "grip_pose_right",
                    "/user/hand/right/input/grip/pose",
                    pose_bindings[3],
                ),
            ] {
                let outcome = suggest_binding_best_effort(
                    instance,
                    profile,
                    spec.profile,
                    binding_label,
                    binding_path,
                    binding,
                )?;
                if outcome == BindingSuggestionOutcome::Accepted {
                    accepted_bindings.push(binding);
                }
                report.suggestions.push(BindingSuggestionRecord {
                    label: binding_label,
                    path: binding_path.to_string(),
                    outcome,
                });
            }
            if let (Some(l), Some(r)) = (spec.select_left, spec.select_right) {
                if let (Ok(lp), Ok(rp)) = (instance.string_to_path(l), instance.string_to_path(r)) {
                    let outcome = suggest_binding_best_effort(
                        instance,
                        profile,
                        spec.profile,
                        "select_left",
                        l,
                        openxr::Binding::new(&select, lp),
                    )?;
                    if outcome == BindingSuggestionOutcome::Accepted {
                        accepted_bindings.push(openxr::Binding::new(&select, lp));
                    }
                    report.suggestions.push(BindingSuggestionRecord {
                        label: "select_left",
                        path: l.to_string(),
                        outcome,
                    });
                    let outcome = suggest_binding_best_effort(
                        instance,
                        profile,
                        spec.profile,
                        "select_right",
                        r,
                        openxr::Binding::new(&select, rp),
                    )?;
                    if outcome == BindingSuggestionOutcome::Accepted {
                        accepted_bindings.push(openxr::Binding::new(&select, rp));
                    }
                    report.suggestions.push(BindingSuggestionRecord {
                        label: "select_right",
                        path: r.to_string(),
                        outcome,
                    });
                }
            }
            let mut suggested = Vec::new();
            for hand_spec in spec.left_specs {
                append_action_bindings(
                    instance,
                    &mut suggested,
                    *hand_spec,
                    &left_stick_x,
                    &left_stick_y,
                    &right_stick_x,
                    &right_stick_y,
                    &trigger_value,
                    &trigger_click,
                    &grip_value,
                    &grip_click,
                    &button_a,
                    &button_b,
                    &button_x,
                    &button_y,
                );
            }
            for hand_spec in spec.right_specs {
                append_action_bindings(
                    instance,
                    &mut suggested,
                    *hand_spec,
                    &left_stick_x,
                    &left_stick_y,
                    &right_stick_x,
                    &right_stick_y,
                    &trigger_value,
                    &trigger_click,
                    &grip_value,
                    &grip_click,
                    &button_a,
                    &button_b,
                    &button_x,
                    &button_y,
                );
            }
            if !suggested.is_empty() {
                for (binding_label, binding_path, binding) in suggested {
                    let outcome = suggest_binding_best_effort(
                        instance,
                        profile,
                        spec.profile,
                        binding_label,
                        binding_path,
                        binding,
                    )?;
                    if outcome == BindingSuggestionOutcome::Accepted {
                        accepted_bindings.push(binding);
                    }
                    report.suggestions.push(BindingSuggestionRecord {
                        label: binding_label,
                        path: binding_path.to_string(),
                        outcome,
                    });
                }
            }
            if !accepted_bindings.is_empty() {
                instance
                    .suggest_interaction_profile_bindings(profile, &accepted_bindings)
                    .map_err(|err| {
                        format!(
                            "suggest_interaction_profile_bindings(final batch {}, count={}): {err:?}",
                            spec.profile,
                            accepted_bindings.len()
                        )
                    })?;
            }
            binding_reports.push(report);
        }

        session
            .attach_action_sets(&[&action_set])
            .map_err(|e| format!("attach_action_sets: {e:?}"))?;

        Ok(ControllerInput {
            action_set,
            aim_pose,
            grip_pose,
            select,
            left_stick_x,
            left_stick_y,
            right_stick_x,
            right_stick_y,
            trigger_value,
            trigger_click,
            grip_value,
            grip_click,
            button_a,
            button_b,
            button_x,
            button_y,
            left,
            right,
            left_aim_space,
            right_aim_space,
            left_grip_space,
            right_grip_space,
            profile_poll_counter: 0,
            last_logged_left_profile: None,
            last_logged_right_profile: None,
            last_reported_left_profile: None,
            last_reported_right_profile: None,
            last_debug_snapshot: None,
            did_log_bound_sources: false,
            binding_reports,
        })
    }

    fn first_enabled_camera_xr(world: &World) -> Option<ComponentId> {
        world
            .all_components()
            .filter_map(|id| {
                world
                    .get_component_by_id_as::<CameraXRComponent>(id)
                    .map(|c| (id, c.enabled))
            })
            .find(|(_, enabled)| *enabled)
            .map(|(id, _)| id)
    }

    fn mul_mat4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
        let mut c = [[0.0f32; 4]; 4];
        for col in 0..4 {
            for row in 0..4 {
                c[col][row] = a[0][row] * b[col][0]
                    + a[1][row] * b[col][1]
                    + a[2][row] * b[col][2]
                    + a[3][row] * b[col][3];
            }
        }
        c
    }

    fn transform_from_matrix_world(
        m: [[f32; 4]; 4],
    ) -> crate::engine::graphics::primitives::Transform {
        let mut t = crate::engine::graphics::primitives::Transform::default();
        t.model = m;
        t.matrix_world = m;
        t
    }

    fn mat4_from_pose(pose: openxr::Posef) -> [[f32; 4]; 4] {
        // IMPORTANT: This must match the engine's quaternion convention.
        // `Transform::recompute_model` is the canonical implementation.
        let q = pose.orientation;
        let p = pose.position;

        let mut t = crate::engine::graphics::primitives::Transform::default();
        t.translation = [p.x, p.y, p.z];
        t.rotation = [q.x, q.y, q.z, q.w];
        t.scale = [1.0, 1.0, 1.0];
        t.recompute_model();

        // OpenXR view poses are right-handed with -Z forward, which matches the engine's
        // Camera3D convention (forward -Z) and our projection math.
        //
        // Do not apply additional basis flips here unless we have a proven convention mismatch,
        // since flipping X/Z will also flip head translation direction and can break stereo.
        t.model
    }

    fn invert_affine_transform(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
        // Upper-left 3x3 in column-major.
        let c0 = [m[0][0], m[0][1], m[0][2]];
        let c1 = [m[1][0], m[1][1], m[1][2]];
        let c2 = [m[2][0], m[2][1], m[2][2]];

        // Row-major elements for determinant/cofactors.
        let a00 = c0[0];
        let a10 = c0[1];
        let a20 = c0[2];
        let a01 = c1[0];
        let a11 = c1[1];
        let a21 = c1[2];
        let a02 = c2[0];
        let a12 = c2[1];
        let a22 = c2[2];

        let det = a00 * (a11 * a22 - a12 * a21) - a01 * (a10 * a22 - a12 * a20)
            + a02 * (a10 * a21 - a11 * a20);

        if det.abs() < 1e-8 {
            return [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
        }
        let inv_det = 1.0 / det;

        // Inverse in row-major.
        let inv00 = (a11 * a22 - a12 * a21) * inv_det;
        let inv01 = (a02 * a21 - a01 * a22) * inv_det;
        let inv02 = (a01 * a12 - a02 * a11) * inv_det;

        let inv10 = (a12 * a20 - a10 * a22) * inv_det;
        let inv11 = (a00 * a22 - a02 * a20) * inv_det;
        let inv12 = (a02 * a10 - a00 * a12) * inv_det;

        let inv20 = (a10 * a21 - a11 * a20) * inv_det;
        let inv21 = (a01 * a20 - a00 * a21) * inv_det;
        let inv22 = (a00 * a11 - a01 * a10) * inv_det;

        let tx = m[3][0];
        let ty = m[3][1];
        let tz = m[3][2];

        let itx = -(inv00 * tx + inv01 * ty + inv02 * tz);
        let ity = -(inv10 * tx + inv11 * ty + inv12 * tz);
        let itz = -(inv20 * tx + inv21 * ty + inv22 * tz);

        [
            [inv00, inv10, inv20, 0.0],
            [inv01, inv11, inv21, 0.0],
            [inv02, inv12, inv22, 0.0],
            [itx, ity, itz, 1.0],
        ]
    }

    fn proj_from_fov_rh_zo(fov: openxr::Fovf, z_near: f32, z_far: f32) -> [[f32; 4]; 4] {
        let l = (fov.angle_left).tan() * z_near;
        let r = (fov.angle_right).tan() * z_near;
        let d = (fov.angle_down).tan() * z_near;
        let u = (fov.angle_up).tan() * z_near;

        let w = r - l;
        let h = u - d;
        let nf = 1.0 / (z_near - z_far);

        // Match the engine's column-major RH ZO layout used in Camera3D::perspective_rh_zo.
        // Now: +Y is up in clip space, matching OpenXR and GLTF conventions.
        [
            [2.0 * z_near / w, 0.0, 0.0, 0.0],
            [0.0, 2.0 * z_near / h, 0.0, 0.0],
            [(r + l) / w, (u + d) / h, z_far * nf, -1.0],
            [0.0, 0.0, (z_near * z_far) * nf, 0.0],
        ]
    }
}

impl System for OpenXRSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        self.pump_events();
    }
}

impl OpenXRSystem {
    /// One coherent sample from prepare_frame; no OpenXR calls on the consumer side.
    pub fn hand_tracking_frame(&self) -> Option<Arc<HandTrackingFrame>> {
        self.hand_tracking_frame.clone()
    }

    pub fn last_render_dt_sec(&self) -> Option<f32> {
        self.last_render_dt_sec
    }

    /// Locate this presentation frame before InputXR, controllers, AVC, and IK run.
    pub fn prepare_frame(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        floor_correction_meters: f32,
    ) {
        if let Some(frame) = self.external_frame {
            self.floor_correction_meters = floor_correction_meters;
            self.xr_input_state = frame.input;
            self.xr_gamepad_state = frame.gamepad;
            self.hand_tracking_frame = None;
            return;
        }
        let profile = Self::profile_systems_enabled();
        let perf_enabled = self.vr_perf.is_some();
        let started = Instant::now();
        let mut perf_wait_frame = Duration::ZERO;
        // Every early return publishes loss rather than preserving last frame's joints.
        self.hand_tracking_frame = None;
        self.discard_prepared_frame();
        self.pump_events();
        self.floor_correction_meters = floor_correction_meters;
        let Some(state) = self.state.as_mut() else {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            visuals.set_xr_frame_dt_sec(None);
            return;
        };

        let Some(sess) = state.session.as_mut() else {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            visuals.set_xr_frame_dt_sec(None);
            return;
        };
        sess.head_pose_cache = None;
        sess.hand_root_pose_cache = HandRootPoseCache::default();
        sess.controller_pose_cache = ControllerPoseCache::default();
        if !sess.running || sess.render_failed {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            visuals.set_xr_frame_dt_sec(None);
            return;
        }

        let now = Instant::now();
        let dt_sec = self
            .last_render_instant
            .map(|prev| now.saturating_duration_since(prev).as_secs_f32());
        self.last_render_instant = Some(now);
        if let Some(dt_sec) = dt_sec {
            self.last_render_dt_sec = Some(dt_sec);
            visuals.set_xr_frame_dt_sec(Some(dt_sec));
        }

        // If no XR camera is enabled, do not enter the OpenXR frame pacing path at all.
        // `wait_frame()` can block to headset cadence, which would otherwise throttle the
        // desktop window even though XR scene rendering is effectively disabled.
        if visuals
            .active_xr_camera()
            .or_else(|| Self::first_enabled_camera_xr(world))
            .is_none()
        {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            sess.head_pose_cache = None;
            sess.hand_root_pose_cache = HandRootPoseCache::default();
            sess.controller_pose_cache = ControllerPoseCache::default();
            visuals.set_xr_frame_dt_sec(None);
            return;
        }

        let stage_started = (profile || perf_enabled).then(Instant::now);
        let frame_state = match sess.frame_waiter.wait() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[OpenXR] wait_frame failed: {e:?}");
                return;
            }
        };
        if let Some(started) = stage_started {
            let elapsed = started.elapsed();
            perf_wait_frame = elapsed;
            if profile {
                self.render_profile.wait_frame += elapsed;
            }
        }

        if let Err(e) = sess.frame_stream.begin() {
            eprintln!("[OpenXR] begin_frame failed: {e:?}");
            return;
        }

        if !frame_state.should_render {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            let _ =
                sess.frame_stream
                    .end(frame_state.predicted_display_time, state.blend_mode, &[]);
            return;
        }

        let tracking_started = profile.then(Instant::now);

        let views = match sess.session.locate_views(
            state.view_type,
            frame_state.predicted_display_time,
            &sess.reference_space,
        ) {
            Ok((flags, views))
                if flags.contains(
                    openxr::ViewStateFlags::POSITION_VALID
                        | openxr::ViewStateFlags::ORIENTATION_VALID,
                ) && !views.is_empty() =>
            {
                views
            }
            Ok(_) => {
                let _ = sess.frame_stream.end(
                    frame_state.predicted_display_time,
                    state.blend_mode,
                    &[],
                );
                self.xr_input_state = XrInputState::default();
                self.xr_gamepad_state = XrGamepadState::default();
                return;
            }
            Err(e) => {
                eprintln!("[OpenXR] locate_views failed: {e:?}");
                let _ = sess.frame_stream.end(
                    frame_state.predicted_display_time,
                    state.blend_mode,
                    &[],
                );
                return;
            }
        };

        sess.head_pose_cache = Self::derive_head_pose(&views);
        self.xr_gamepad_state.head_pose_rotation = sess.head_pose_cache.map(Self::quat_from_posef);

        let mut left_root_for_debug: Option<(openxr::Posef, openxr::HandJointEXT)> = None;
        let mut right_root_for_debug: Option<(openxr::Posef, openxr::HandJointEXT)> = None;
        let mut left_joints_for_debug: Option<openxr::HandJointLocations> = None;
        let mut right_joints_for_debug: Option<openxr::HandJointLocations> = None;
        let mut tracked_hands = [None, None];

        // Source and root belong to one sample. Clear both before locating so a
        // lost hand or failed query cannot leave stale source-selection authority.
        sess.hand_root_pose_cache = HandRootPoseCache::default();
        if let Some(hand_tracking) = sess.hand_tracking.as_ref() {
            let left_joints = Self::locate_hand_sample(
                &sess.reference_space,
                &hand_tracking.left,
                frame_state.predicted_display_time,
            );
            let right_joints = Self::locate_hand_sample(
                &sess.reference_space,
                &hand_tracking.right,
                frame_state.predicted_display_time,
            );

            match left_joints {
                Ok(Some(sample)) => {
                    tracked_hands[0] =
                        Some(Self::normalize_hand_sample(&sample, ControllerHand::Left));
                    let joints = sample.joints;
                    let root = Self::select_hand_root_pose(&joints, ControllerHand::Left);
                    sess.hand_root_pose_cache.left_root = root.map(|(pose, _)| pose);
                    sess.hand_root_pose_cache.left_root_joint = root.map(|(_, joint)| joint);
                    sess.hand_root_pose_cache.left_data_source = sample.data_source;
                    left_root_for_debug = root;
                    left_joints_for_debug = Some(joints);
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("[OpenXR] locate_hand_joints(left) failed: {e:?}");
                }
            }

            match right_joints {
                Ok(Some(sample)) => {
                    tracked_hands[1] =
                        Some(Self::normalize_hand_sample(&sample, ControllerHand::Right));
                    let joints = sample.joints;
                    let root = Self::select_hand_root_pose(&joints, ControllerHand::Right);
                    sess.hand_root_pose_cache.right_root = root.map(|(pose, _)| pose);
                    sess.hand_root_pose_cache.right_root_joint = root.map(|(_, joint)| joint);
                    sess.hand_root_pose_cache.right_data_source = sample.data_source;
                    right_root_for_debug = root;
                    right_joints_for_debug = Some(joints);
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("[OpenXR] locate_hand_joints(right) failed: {e:?}");
                }
            }
        }

        self.hand_tracking_sequence = self.hand_tracking_sequence.wrapping_add(1);
        self.hand_tracking_frame = Some(Arc::new(HandTrackingFrame {
            provider: HandTrackingProvider::OpenXr,
            epoch: self.hand_tracking_epoch,
            sequence: self.hand_tracking_sequence,
            sample_time: HandSampleTime::OpenXrPredictedNanoseconds(
                frame_state.predicted_display_time.as_nanos(),
            ),
            received_at: Instant::now(),
            space: HandCoordinateSpace::OpenXrStage {
                generation: self.hand_tracking_epoch,
            },
            hands: tracked_hands,
        }));

        Self::log_hand_debug_snapshot(
            sess,
            left_joints_for_debug.as_ref(),
            right_joints_for_debug.as_ref(),
            left_root_for_debug,
            right_root_for_debug,
        );

        if let Some((pose, joint)) = left_root_for_debug {
            Self::update_hand_rotation_debug(
                &mut sess.hand_rotation_debug,
                ControllerHand::Left,
                Some(joint),
                pose,
            );
        }
        if let Some((pose, joint)) = right_root_for_debug {
            Self::update_hand_rotation_debug(
                &mut sess.hand_rotation_debug,
                ControllerHand::Right,
                Some(joint),
                pose,
            );
        }

        // Update controller pose cache at the same predicted time as views.
        sess.controller_pose_generation = sess.controller_pose_generation.wrapping_add(1);
        let controller_pose_generation = sess.controller_pose_generation;
        if openxr_debug_enabled() && controller_pose_generation % 120 == 1 {
            for (hand, joints, root, data_source) in [
                (
                    ControllerHand::Left,
                    left_joints_for_debug.as_ref(),
                    left_root_for_debug,
                    sess.hand_root_pose_cache.left_data_source,
                ),
                (
                    ControllerHand::Right,
                    right_joints_for_debug.as_ref(),
                    right_root_for_debug,
                    sess.hand_root_pose_cache.right_data_source,
                ),
            ] {
                if let Some(joints) = joints {
                    Self::log_hand_frame(
                        hand,
                        joints,
                        root,
                        data_source,
                        controller_pose_generation,
                    );
                }
            }
        }
        if let Some(ci) = sess.controller_input.as_mut() {
            // Sync actions (best-effort).
            let active = openxr::ActiveActionSet::new(&ci.action_set);
            match sess.session.sync_actions(&[active]) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("[OpenXR] sync_actions failed: {e:?}");
                }
            }
            // A locatable action space is not sufficient evidence that its action is active.
            // Some runtimes continue to expose controller-derived hand joints and action-space
            // locations at the same time, so use action activity to decide whether the explicit
            // controller pose may take priority over the wrist/palm fallback.
            let left_aim_active = ci
                .aim_pose
                .is_active(&sess.session, ci.left)
                .unwrap_or(false);
            let right_aim_active = ci
                .aim_pose
                .is_active(&sess.session, ci.right)
                .unwrap_or(false);
            let left_grip_active = ci
                .grip_pose
                .is_active(&sess.session, ci.left)
                .unwrap_or(false);
            let right_grip_active = ci
                .grip_pose
                .is_active(&sess.session, ci.right)
                .unwrap_or(false);
            let should_poll_profiles =
                ci.profile_poll_counter == 0 || ci.profile_poll_counter >= 89;
            ci.profile_poll_counter = if should_poll_profiles {
                1
            } else {
                ci.profile_poll_counter.saturating_add(1)
            };
            if should_poll_profiles {
                log_active_interaction_profile(
                    &state.instance,
                    &sess.session,
                    ci.left,
                    "/user/hand/left",
                    &mut ci.last_logged_left_profile,
                );
                log_active_interaction_profile(
                    &state.instance,
                    &sess.session,
                    ci.right,
                    "/user/hand/right",
                    &mut ci.last_logged_right_profile,
                );
            }
            let update_pose = |space: &openxr::Space, base: &openxr::Space, t: openxr::Time| {
                space.locate(base, t).ok()
            };

            let left_profile = ci.last_logged_left_profile;
            let right_profile = ci.last_logged_right_profile;

            sess.controller_pose_cache.left_aim = update_pose(
                &ci.left_aim_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            )
            .filter(|_| left_aim_active)
            .filter(|loc| {
                loc.location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
            })
            .map(|loc| ControllerPoseSample {
                pose: loc.pose,
                generation: controller_pose_generation,
            });
            sess.controller_pose_cache.right_aim = update_pose(
                &ci.right_aim_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            )
            .filter(|_| right_aim_active)
            .filter(|loc| {
                loc.location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
            })
            .map(|loc| ControllerPoseSample {
                pose: loc.pose,
                generation: controller_pose_generation,
            });

            sess.controller_pose_cache.left_grip = update_pose(
                &ci.left_grip_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            )
            .filter(|_| left_grip_active)
            .filter(|loc| {
                loc.location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
            })
            .map(|loc| ControllerPoseSample {
                pose: loc.pose,
                generation: controller_pose_generation,
            });
            sess.controller_pose_cache.right_grip = update_pose(
                &ci.right_grip_space,
                &sess.reference_space,
                frame_state.predicted_display_time,
            )
            .filter(|_| right_grip_active)
            .filter(|loc| {
                loc.location_flags
                    .contains(openxr::SpaceLocationFlags::POSITION_VALID)
                    && loc
                        .location_flags
                        .contains(openxr::SpaceLocationFlags::ORIENTATION_VALID)
            })
            .map(|loc| ControllerPoseSample {
                pose: loc.pose,
                generation: controller_pose_generation,
            });

            let left_select = action_state_bool(&ci.select, &sess.session, ci.left);
            let right_select = action_state_bool(&ci.select, &sess.session, ci.right);
            let left_stick_x = ci.left_stick_x.state(&sess.session, ci.left).ok();
            let left_stick_y = ci.left_stick_y.state(&sess.session, ci.left).ok();
            let right_stick_x = ci.right_stick_x.state(&sess.session, ci.right).ok();
            let right_stick_y = ci.right_stick_y.state(&sess.session, ci.right).ok();
            let left_trigger_value = action_state_f32(&ci.trigger_value, &sess.session, ci.left);
            let right_trigger_value = action_state_f32(&ci.trigger_value, &sess.session, ci.right);
            let left_trigger_click = action_state_bool(&ci.trigger_click, &sess.session, ci.left);
            let right_trigger_click = action_state_bool(&ci.trigger_click, &sess.session, ci.right);
            let left_grip_value = action_state_f32(&ci.grip_value, &sess.session, ci.left);
            let right_grip_value = action_state_f32(&ci.grip_value, &sess.session, ci.right);
            let left_grip_click = action_state_bool(&ci.grip_click, &sess.session, ci.left);
            let right_grip_click = action_state_bool(&ci.grip_click, &sess.session, ci.right);
            let left_x = action_state_bool(&ci.button_x, &sess.session, ci.left);
            let left_y = action_state_bool(&ci.button_y, &sess.session, ci.left);
            let right_a = action_state_bool(&ci.button_a, &sess.session, ci.right);
            let right_b = action_state_bool(&ci.button_b, &sess.session, ci.right);
            let snapshot = ControllerDebugSnapshot {
                left_profile: profile_string_or_none(&state.instance, left_profile),
                right_profile: profile_string_or_none(&state.instance, right_profile),
                left_aim_valid: sess.controller_pose_cache.left_aim.is_some(),
                right_aim_valid: sess.controller_pose_cache.right_aim.is_some(),
                left_grip_valid: sess.controller_pose_cache.left_grip.is_some(),
                right_grip_valid: sess.controller_pose_cache.right_grip.is_some(),
                left_select: left_select.map(|s| (s.is_active, s.current_state)),
                right_select: right_select.map(|s| (s.is_active, s.current_state)),
                left_thumbstick: scalar_stick_state(left_stick_x, left_stick_y),
                right_thumbstick: scalar_stick_state(right_stick_x, right_stick_y),
                left_trigger_value: left_trigger_value.map(|s| (s.is_active, s.current_state)),
                right_trigger_value: right_trigger_value.map(|s| (s.is_active, s.current_state)),
                left_trigger_click: left_trigger_click.map(|s| (s.is_active, s.current_state)),
                right_trigger_click: right_trigger_click.map(|s| (s.is_active, s.current_state)),
                left_grip_value: left_grip_value.map(|s| (s.is_active, s.current_state)),
                right_grip_value: right_grip_value.map(|s| (s.is_active, s.current_state)),
                left_grip_click: left_grip_click.map(|s| (s.is_active, s.current_state)),
                right_grip_click: right_grip_click.map(|s| (s.is_active, s.current_state)),
                left_x: left_x.map(|s| (s.is_active, s.current_state)),
                left_y: left_y.map(|s| (s.is_active, s.current_state)),
                right_a: right_a.map(|s| (s.is_active, s.current_state)),
                right_b: right_b.map(|s| (s.is_active, s.current_state)),
            };
            if let Some(left_profile) = left_profile {
                if left_profile != openxr::Path::NULL
                    && ci.last_reported_left_profile != Some(left_profile)
                {
                    if let Some(report) = ci
                        .binding_reports
                        .iter()
                        .find(|report| report.profile_path == snapshot.left_profile)
                    {
                        log_runtime_profile_binding_report("left", report, &snapshot);
                    } else {
                        eprintln!(
                            "[OpenXR][binding_report] left active profile has no stored binding report: {}",
                            snapshot.left_profile
                        );
                    }
                    ci.last_reported_left_profile = Some(left_profile);
                }
            }
            if let Some(right_profile) = right_profile {
                if right_profile != openxr::Path::NULL
                    && ci.last_reported_right_profile != Some(right_profile)
                {
                    if let Some(report) = ci
                        .binding_reports
                        .iter()
                        .find(|report| report.profile_path == snapshot.right_profile)
                    {
                        log_runtime_profile_binding_report("right", report, &snapshot);
                    } else {
                        eprintln!(
                            "[OpenXR][binding_report] right active profile has no stored binding report: {}",
                            snapshot.right_profile
                        );
                    }
                    ci.last_reported_right_profile = Some(right_profile);
                }
            }
            if !ci.did_log_bound_sources
                && (matches!(left_profile, Some(profile) if profile != openxr::Path::NULL)
                    || matches!(right_profile, Some(profile) if profile != openxr::Path::NULL))
            {
                log_controller_input_bound_sources(&sess.session, ci);
                ci.did_log_bound_sources = true;
            }
            if openxr_debug_enabled() && ci.last_debug_snapshot.as_ref() != Some(&snapshot) {
                eprintln!(
                    "[OpenXR][debug] profiles left={} right={}",
                    snapshot.left_profile, snapshot.right_profile,
                );
                eprintln!(
                    "[OpenXR][debug] pose_valid left_aim={} right_aim={} left_grip={} right_grip={}",
                    snapshot.left_aim_valid,
                    snapshot.right_aim_valid,
                    snapshot.left_grip_valid,
                    snapshot.right_grip_valid,
                );
                eprintln!(
                    "[OpenXR][debug] select L={:?} R={:?} thumbstick L={:?} R={:?}",
                    snapshot.left_select,
                    snapshot.right_select,
                    snapshot.left_thumbstick,
                    snapshot.right_thumbstick,
                );
                eprintln!(
                    "[OpenXR][debug] trigger_value L={:?} R={:?} trigger_click L={:?} R={:?}",
                    snapshot.left_trigger_value,
                    snapshot.right_trigger_value,
                    snapshot.left_trigger_click,
                    snapshot.right_trigger_click,
                );
                eprintln!(
                    "[OpenXR][debug] grip_value L={:?} R={:?} grip_click L={:?} R={:?}",
                    snapshot.left_grip_value,
                    snapshot.right_grip_value,
                    snapshot.left_grip_click,
                    snapshot.right_grip_click,
                );
                eprintln!(
                    "[OpenXR][debug] face_buttons X={:?} Y={:?} A={:?} B={:?}",
                    snapshot.left_x, snapshot.left_y, snapshot.right_a, snapshot.right_b,
                );
            }
            ci.last_debug_snapshot = Some(snapshot);

            // Select is a semantic trigger action: prefer the dedicated select action when it is
            // live, otherwise fall back to trigger click or trigger analog threshold.
            let prev = self.xr_input_state.trigger_down;
            let prev_grip = self.xr_input_state.grip_down;
            self.xr_gamepad_state = XrGamepadState {
                active: true,
                hands: [XrHandGamepadState::default(), XrHandGamepadState::default()],
                head_pose_rotation: self.xr_gamepad_state.head_pose_rotation,
            };
            for i in 0..2 {
                let hand_path = if i == 0 { ci.left } else { ci.right };
                let select_state = action_state_bool(&ci.select, &sess.session, hand_path);
                let trigger_click_state =
                    action_state_bool(&ci.trigger_click, &sess.session, hand_path);
                let trigger_value_state =
                    action_state_f32(&ci.trigger_value, &sess.session, hand_path);
                let grip_click_state = action_state_bool(&ci.grip_click, &sess.session, hand_path);
                let grip_value_state = action_state_f32(&ci.grip_value, &sess.session, hand_path);

                let cur_down =
                    derive_select_down(select_state, trigger_click_state, trigger_value_state);
                self.xr_input_state.trigger_down[i] = cur_down;
                self.xr_input_state.trigger_pressed[i] = cur_down && !prev[i];
                self.xr_input_state.trigger_released[i] = !cur_down && prev[i];

                self.xr_gamepad_state.hands[i].thumbstick = match i {
                    0 => scalar_stick_value(
                        ci.left_stick_x.state(&sess.session, ci.left).ok(),
                        ci.left_stick_y.state(&sess.session, ci.left).ok(),
                    ),
                    1 => scalar_stick_value(
                        ci.right_stick_x.state(&sess.session, ci.right).ok(),
                        ci.right_stick_y.state(&sess.session, ci.right).ok(),
                    ),
                    _ => None,
                };

                let trigger_value = trigger_value_state.map(|s| s.current_state);
                self.xr_gamepad_state.hands[i].trigger_value = trigger_value;
                self.xr_gamepad_state.hands[i].trigger_pressed = derive_trigger_like_pressed(
                    trigger_click_state,
                    trigger_value_state,
                    XR_TRIGGER_PRESS_THRESHOLD,
                );

                let grip_value = grip_value_state.map(|s| s.current_state);
                self.xr_gamepad_state.hands[i].grip_value = grip_value;
                self.xr_gamepad_state.hands[i].grip_pressed = derive_trigger_like_pressed(
                    grip_click_state,
                    grip_value_state,
                    XR_GRIP_PRESS_THRESHOLD,
                );
                let grip_down = self.xr_gamepad_state.hands[i]
                    .grip_pressed
                    .is_some_and(|(pressed, _)| pressed);
                self.xr_input_state.grip_down[i] = grip_down;
                self.xr_input_state.grip_pressed[i] = grip_down && !prev_grip[i];
                self.xr_input_state.grip_released[i] = !grip_down && prev_grip[i];
            }
            self.xr_gamepad_state.hands[0].button_x = ci
                .button_x
                .state(&sess.session, ci.left)
                .ok()
                .filter(|s| s.is_active)
                .map(|s| (s.current_state, if s.current_state { 1.0 } else { 0.0 }));
            self.xr_gamepad_state.hands[0].button_y = ci
                .button_y
                .state(&sess.session, ci.left)
                .ok()
                .filter(|s| s.is_active)
                .map(|s| (s.current_state, if s.current_state { 1.0 } else { 0.0 }));
            self.xr_gamepad_state.hands[1].button_a = ci
                .button_a
                .state(&sess.session, ci.right)
                .ok()
                .filter(|s| s.is_active)
                .map(|s| (s.current_state, if s.current_state { 1.0 } else { 0.0 }));
            self.xr_gamepad_state.hands[1].button_b = ci
                .button_b
                .state(&sess.session, ci.right)
                .ok()
                .filter(|s| s.is_active)
                .map(|s| (s.current_state, if s.current_state { 1.0 } else { 0.0 }));
            log_xr_gamepad_changes(self.xr_gamepad_state_last_logged, self.xr_gamepad_state);
            self.xr_gamepad_state_last_logged = self.xr_gamepad_state;
        } else {
            self.xr_input_state = XrInputState::default();
            self.xr_gamepad_state = XrGamepadState::default();
            log_xr_gamepad_changes(self.xr_gamepad_state_last_logged, self.xr_gamepad_state);
            self.xr_gamepad_state_last_logged = self.xr_gamepad_state;
        }

        if let Some(started) = tracking_started {
            self.render_profile.tracking_input += started.elapsed();
        }
        sess.prepared_frame = Some(PreparedXrFrame {
            state: frame_state,
            views,
            wait_time: perf_wait_frame,
            started,
        });
    }

    /// A second update, scene change, or shutdown may abandon a prepared frame.
    fn discard_prepared_frame(&mut self) {
        if let Some(state) = self.state.as_mut() {
            if let Some(sess) = state.session.as_mut() {
                if let Some(frame) = sess.prepared_frame.take() {
                    let _ = sess.frame_stream.end(
                        frame.state.predicted_display_time,
                        state.blend_mode,
                        &[],
                    );
                }
            }
        }
    }

    pub fn render_xr(
        &mut self,
        world: &World,
        visuals: &mut VisualWorld,
        renderer: &mut VulkanoRenderer,
    ) {
        let profile = Self::profile_systems_enabled();
        let perf_enabled = self.vr_perf.is_some();
        if !self.publish_eye_cameras(world, visuals) {
            self.discard_prepared_frame();
            return;
        }
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let Some(sess) = state.session.as_mut() else {
            return;
        };
        let Some(frame) = sess.prepared_frame.take() else {
            return;
        };
        let frame_state = frame.state;
        let views = frame.views;
        let frame_profile_started = profile.then_some(frame.started);
        let perf_frame_started = frame.started;
        let perf_wait_frame = frame.wait_time;
        let perf_renderer_before = renderer.perf_counters();
        let perf_pre_xr = self.vr_perf_pre_xr;
        let mut perf_eye_render = Duration::ZERO;
        let mut perf_completion_wait = Duration::ZERO;
        let mut perf_raw_queue_submissions = 0;
        let mut perf_raw_fence_waits = 0;
        let mut image_ready_for_projection = false;
        let mut image_waited = false;
        let mut gpu_work_complete = true;

        // Acquire XR swapchain image.
        let acquire_started = profile.then(Instant::now);
        let image_index = {
            let swapchain = &mut sess.xr_swapchain;
            match swapchain.acquire_image() {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("[OpenXR] acquire_image failed: {e:?}");
                    let _ = sess.frame_stream.end(
                        frame_state.predicted_display_time,
                        state.blend_mode,
                        &[],
                    );
                    return;
                }
            }
        };

        if let Err(e) = sess.xr_swapchain.wait_image(openxr::Duration::INFINITE) {
            eprintln!("[OpenXR] wait_image failed: {e:?}");
        } else {
            image_waited = true;
            if let Some(started) = acquire_started {
                self.render_profile.acquire_wait += started.elapsed();
            }
            // Render/resolve each eye directly into the acquired OpenXR image layer.
            let extent = sess.xr_swapchain.extent();
            let extent_u = [extent.width as u32, extent.height as u32];

            let image_index_usize = image_index as usize;
            let batch_generation = self.next_xr_batch_generation;
            self.next_xr_batch_generation = self.next_xr_batch_generation.wrapping_add(1).max(1);
            let dst_was_initialized = sess
                .swapchain_image_initialized
                .get(image_index_usize)
                .copied()
                .unwrap_or(false);

            // The runtime owns the output; only transient MSAA/depth and effect
            // intermediates are engine-owned.
            let view_count = sess.xr_swapchain.view_count() as usize;

            let xr_format = sess.xr_swapchain.format();
            let scene_format = renderer.scene_vk_format_raw();
            let format_matches = scene_format.map(|f| f == xr_format).unwrap_or(true);

            // Windowed startup can still negotiate a different XR format.
            // XR-only startup builds scene pipelines from the XR format itself.
            // Keep the diagnostic clear for a window/XR mismatch.
            let dst_image = sess.xr_swapchain.images()[image_index_usize];
            if !format_matches {
                if !sess.did_log_format_mismatch {
                    sess.did_log_format_mismatch = true;
                    eprintln!(
                        "[OpenXR] XR swapchain format mismatch (xr={} scene={:?}); falling back to clear_color",
                        xr_format, scene_format
                    );
                }

                if !sess.vk_completion_fence_usable {
                    eprintln!("[OpenXR] clear skipped: XR completion fence is not reusable");
                } else if let Err(e) = renderer
                    .with_xr_graphics_queue_lock(|| {
                        gpu_work_complete = false;
                        xr_renderer::submit_clear_xr_swapchain_image_and_wait(
                            &sess.vk_device,
                            sess.vk_queue,
                            sess.vk_command_buffer,
                            sess.vk_completion_fence,
                            sess.xr_swapchain.view_count(),
                            dst_image,
                            visuals.clear_color(),
                            dst_was_initialized,
                        )
                    })
                    .unwrap_or(Err(ash::vk::Result::ERROR_DEVICE_LOST))
                {
                    sess.vk_completion_fence_usable = false;
                    eprintln!("[OpenXR] clear XR image failed: {e:?}");
                    gpu_work_complete = renderer.wait_for_xr_batch_on_error().is_ok();
                } else {
                    gpu_work_complete = true;
                    perf_raw_queue_submissions += 1;
                    perf_raw_fence_waits += 1;
                    image_ready_for_projection = true;
                    if openxr_debug_enabled() {
                        eprintln!(
                            "[OpenXR][batch] generation={batch_generation} image={image_index} \
                             mode=clear queue=0x{:x} raw_submissions=1 completion_waits=1",
                            sess.vk_queue.as_raw()
                        );
                    }
                    if let Some(slot) = sess.swapchain_image_initialized.get_mut(image_index_usize)
                    {
                        *slot = true;
                    }
                }
            } else {
                gpu_work_complete = false;
                let eye_render_started = (profile || perf_enabled).then(Instant::now);
                let mut batch_error = None;
                if !sess.vk_completion_fence_usable {
                    batch_error = Some("XR completion fence is not reusable".to_string());
                } else if views.len() < view_count {
                    batch_error = Some(format!(
                        "located {} views for {view_count} swapchain layers",
                        views.len()
                    ));
                } else if let Err(e) = renderer.begin_xr_frame(&mut sess.xr_swapchain, image_index)
                {
                    batch_error = Some(format!("XR image import failed: {e}"));
                } else if let Err(e) = renderer.submit_xr_mirror_captures(visuals) {
                    batch_error = Some(format!("mirror capture submission failed: {e}"));
                }
                if batch_error.is_none() {
                    for eye in 0..view_count {
                        if let Err(e) = renderer.submit_xr_eye(visuals, eye, extent_u) {
                            batch_error = Some(format!("eye {eye} submission failed: {e}"));
                            break;
                        }
                    }
                }
                if let Some(started) = eye_render_started {
                    let elapsed = started.elapsed();
                    perf_eye_render = elapsed;
                    if profile {
                        self.render_profile.eye_render += elapsed;
                    }
                }

                if let Some(error) = batch_error {
                    eprintln!("[OpenXR] XR batch {batch_generation} aborted: {error}");
                    match renderer.wait_for_xr_batch_on_error() {
                        Ok(()) => gpu_work_complete = true,
                        Err(wait_error) => eprintln!(
                            "[OpenXR] XR batch {batch_generation} recovery wait failed: {wait_error}"
                        ),
                    }
                } else {
                    let completion_started = (profile || perf_enabled).then(Instant::now);
                    if let Err(e) = renderer
                        .with_xr_graphics_queue_lock(|| {
                            // SAFETY: the queue guard provides synchronization;
                            // this session owns the live, reusable fence.
                            unsafe {
                                xr_renderer::submit_xr_completion_and_wait(
                                    &sess.vk_device,
                                    sess.vk_queue,
                                    sess.vk_completion_fence,
                                )
                            }
                        })
                        .unwrap_or(Err(ash::vk::Result::ERROR_DEVICE_LOST))
                    {
                        sess.vk_completion_fence_usable = false;
                        eprintln!(
                            "[OpenXR] XR batch {batch_generation} completion submission/wait failed: {e:?}"
                        );
                        match renderer.wait_for_xr_batch_on_error() {
                            Ok(()) => gpu_work_complete = true,
                            Err(wait_error) => eprintln!(
                                "[OpenXR] XR batch {batch_generation} recovery wait failed: {wait_error}"
                            ),
                        }
                    } else {
                        gpu_work_complete = true;
                        perf_raw_queue_submissions += 1;
                        // The completion fence proves completion of every preceding
                        // Vulkano submission on this queue. Only now may Vulkano release
                        // the retained resource-use chain.
                        unsafe {
                            if let Err(e) = renderer.finish_xr_batch_after_external_wait() {
                                eprintln!("[OpenXR] failed to finish XR render batch: {e}");
                            }
                        }
                        perf_raw_fence_waits += 1;
                        image_ready_for_projection = true;
                        if openxr_debug_enabled() {
                            let batch_counters = renderer
                                .perf_counters()
                                .saturating_delta(perf_renderer_before);
                            eprintln!(
                                "[OpenXR][batch] generation={batch_generation} image={image_index} \
                                 eyes={} mirrors={} queue=0x{:x} vulkano_submissions={} \
                                 raw_submissions=1 completion_waits=1",
                                batch_counters.xr_eyes,
                                batch_counters.mirror_captures,
                                sess.vk_queue.as_raw(),
                                batch_counters.queue_submissions,
                            );
                        }
                        if let Some(slot) =
                            sess.swapchain_image_initialized.get_mut(image_index_usize)
                        {
                            *slot = true;
                        }
                    }
                    if let Some(started) = completion_started {
                        let elapsed = started.elapsed();
                        perf_completion_wait = elapsed;
                        if profile {
                            self.render_profile.completion_wait += elapsed;
                        }
                    }
                }
            }
        }

        if image_waited && gpu_work_complete {
            if let Err(e) = sess.xr_swapchain.release_image() {
                image_ready_for_projection = false;
                eprintln!("[OpenXR] release_image failed: {e:?}");
                let _ = sess.session.request_exit();
                sess.render_failed = true;
            }
        } else {
            // Never release an unwaited image or one still referenced by GPU
            // work. Stop producing frames and let session teardown recover.
            image_ready_for_projection = false;
            let _ = sess.session.request_exit();
            sess.render_failed = true;
        }

        // Submit a projection layer.
        if image_ready_for_projection && views.len() >= 2 {
            let rect = openxr::Rect2Di {
                offset: openxr::Offset2Di { x: 0, y: 0 },
                extent: sess.xr_swapchain.extent(),
            };

            let projection_swapchain = sess.xr_swapchain.swapchain();
            let pv0 = openxr::CompositionLayerProjectionView::new()
                .pose(views[0].pose)
                .fov(views[0].fov)
                .sub_image(
                    openxr::SwapchainSubImage::new()
                        .swapchain(&projection_swapchain)
                        .image_array_index(0)
                        .image_rect(rect),
                );

            let pv1 = openxr::CompositionLayerProjectionView::new()
                .pose(views[1].pose)
                .fov(views[1].fov)
                .sub_image(
                    openxr::SwapchainSubImage::new()
                        .swapchain(&projection_swapchain)
                        .image_array_index(1)
                        .image_rect(rect),
                );

            let projection_views = [pv0, pv1];
            let layer = openxr::CompositionLayerProjection::new()
                .space(&sess.reference_space)
                .views(&projection_views);

            let submit_started = (profile || perf_enabled).then(Instant::now);
            if let Err(e) = sess.frame_stream.end(
                frame_state.predicted_display_time,
                state.blend_mode,
                &[&layer],
            ) {
                eprintln!("[OpenXR] end_frame failed: {e:?}");
            }
            drop(projection_swapchain);
            let submit_elapsed = submit_started.map_or(Duration::ZERO, |started| started.elapsed());
            if profile {
                self.render_profile.submit += submit_elapsed;
            }

            let extent = sess.xr_swapchain.extent();
            let runtime = state
                .instance
                .properties()
                .map(|properties| {
                    format!(
                        "{} ({:?})",
                        properties.runtime_name, properties.runtime_version
                    )
                })
                .unwrap_or_else(|_| "unavailable".to_string());
            if let Some(started) = frame_profile_started {
                self.finish_xr_render_profile_frame(started);
            }
            self.record_vr_perf_frame(
                visuals,
                renderer,
                runtime,
                [extent.width as u32, extent.height as u32],
                Duration::from_nanos(frame_state.predicted_display_period.as_nanos().max(0) as u64),
                VrPerfFrameCpu {
                    pre_xr: perf_pre_xr,
                    total: perf_frame_started.elapsed(),
                    wait_frame: perf_wait_frame,
                    eye_render: perf_eye_render,
                    completion_wait: perf_completion_wait,
                    submit: submit_elapsed,
                    renderer: {
                        let mut counters = renderer
                            .perf_counters()
                            .saturating_delta(perf_renderer_before);
                        counters.queue_submissions += perf_raw_queue_submissions;
                        counters.cpu_fence_waits += perf_raw_fence_waits;
                        counters
                    },
                },
            );
            return;
        }

        let submit_started = (profile || perf_enabled).then(Instant::now);
        if let Err(e) =
            sess.frame_stream
                .end(frame_state.predicted_display_time, state.blend_mode, &[])
        {
            eprintln!("[OpenXR] end_frame failed: {e:?}");
        }
        let submit_elapsed = submit_started.map_or(Duration::ZERO, |started| started.elapsed());
        if profile {
            self.render_profile.submit += submit_elapsed;
        }
        let extent = sess.xr_swapchain.extent();
        let runtime = state
            .instance
            .properties()
            .map(|properties| {
                format!(
                    "{} ({:?})",
                    properties.runtime_name, properties.runtime_version
                )
            })
            .unwrap_or_else(|_| "unavailable".to_string());
        if let Some(started) = frame_profile_started {
            self.finish_xr_render_profile_frame(started);
        }
        self.record_vr_perf_frame(
            visuals,
            renderer,
            runtime,
            [extent.width as u32, extent.height as u32],
            Duration::from_nanos(frame_state.predicted_display_period.as_nanos().max(0) as u64),
            VrPerfFrameCpu {
                pre_xr: perf_pre_xr,
                total: perf_frame_started.elapsed(),
                wait_frame: perf_wait_frame,
                eye_render: perf_eye_render,
                completion_wait: perf_completion_wait,
                submit: submit_elapsed,
                renderer: {
                    let mut counters = renderer
                        .perf_counters()
                        .saturating_delta(perf_renderer_before);
                    counters.queue_submissions += perf_raw_queue_submissions;
                    counters.cpu_fence_waits += perf_raw_fence_waits;
                    counters
                },
            },
        );
    }
}

#[cfg(test)]
mod floor_calibration_tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::TransformComponent;
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::graphics::RenderAssets;

    fn near(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1e-4, "{actual} != {expected}");
    }

    #[test]
    fn xr_settings_eyes_and_hands_follow_inputxr_world_transform_without_double_tracking() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let rig = world.add_component(
            TransformComponent::new()
                .with_position(2.0, 3.0, 4.0)
                .with_rotation_euler(0.0, 0.7, 0.0),
        );
        let input = world.add_component(InputXRComponent::on());
        let head = world.add_component(TransformComponent::new());
        // AVC's camera anchor is authoring data, not a second camera translation.
        let wrapper = world.add_component(TransformComponent::new().with_position(0.0, 17.0, 0.12));
        let camera = world.add_component(CameraXRComponent::on());
        for (parent, child) in [
            (rig, input),
            (input, head),
            (head, wrapper),
            (wrapper, camera),
        ] {
            world.add_child(parent, child).unwrap();
        }
        visuals.set_active_xr_camera(Some(camera));
        systems.transform_changed(&mut world, &mut visuals, rig);
        let fov = openxr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.7,
            angle_down: -0.7,
        };
        // Successive samples change position and orientation, exercising the head-relative
        // conversion with current data rather than a cached previous-frame head.
        for (height, angle) in [(1.6, 0.0), (0.7, 1.1), (1.8, -0.8)] {
            let raw_head = OpenXRSystem::pose_from_quat_translation(
                math::quat_rotation_y(angle),
                [0.2, height, -0.3],
            );
            let raw_head_matrix = OpenXRSystem::mat4_from_pose(raw_head);
            let views: Vec<_> = [-0.032, 0.032]
                .into_iter()
                .map(|x| {
                    let offset =
                        math::quat_rotate_vec3(math::quat_rotation_y(angle), [x, 0.0, 0.0]);
                    openxr::View {
                        pose: OpenXRSystem::pose_from_quat_translation(
                            math::quat_rotation_y(angle),
                            [0.2 + offset[0], height, -0.3 + offset[2]],
                        ),
                        fov,
                    }
                })
                .collect();
            let sampled_head = OpenXRSystem::derive_head_pose(&views).unwrap();
            for correction in [0.0, 0.15, -0.2, 0.0] {
                OpenXRSystem::drive_head_pose(
                    &mut world,
                    input,
                    sampled_head,
                    correction,
                    &mut queue,
                );
                systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
                let eyes =
                    OpenXRSystem::eye_cameras(&world, &visuals, sampled_head, &views).unwrap();
                let head_world = TransformSystem::world_model(&world, head).unwrap();
                near(head_world[3][1], 3.0 + height - correction);
                for eye in &eyes {
                    near(eye.transform.matrix_world[3][1], head_world[3][1]);
                    // Avatar surfaces 5 cm from the eyes must survive near clipping.
                    let clip = math::mat4_mul_vec4(eye.proj, [0.0, 0.0, -0.05, 1.0]);
                    assert!(clip[2] >= 0.0 && clip[2] <= clip[3]);
                }
                let eye_delta: [f32; 3] = std::array::from_fn(|i| {
                    eyes[1].transform.matrix_world[3][i] - eyes[0].transform.matrix_world[3][i]
                });
                near(math::vec3_len(eye_delta), 0.064);
                // The head center is exactly between the rendered eyes; wrapper Y=17 has no effect.
                for axis in 0..3 {
                    near(
                        (eyes[0].transform.matrix_world[3][axis]
                            + eyes[1].transform.matrix_world[3][axis])
                            * 0.5,
                        head_world[3][axis],
                    );
                }
                let mapping =
                    OpenXRSystem::tracking_world_frame(&world, &visuals, sampled_head).unwrap();
                let raw_hand = OpenXRSystem::pose_from_quat_translation(
                    [0.0, 0.0, 0.0, 1.0],
                    [-0.4, height - 0.4, -0.5],
                );
                let hand_world =
                    OpenXRSystem::mul_mat4(&mapping, &OpenXRSystem::mat4_from_pose(raw_hand));
                near(hand_world[3][1], head_world[3][1] - 0.4);
                near(
                    TransformSystem::world_position(&world, rig).unwrap()[1],
                    3.0,
                );
                assert_eq!(OpenXRSystem::mat4_from_pose(raw_head), raw_head_matrix);
                // Prove the renderer reads the driven matrix, not a second settings calculation.
                world
                    .get_component_by_id_as_mut::<TransformComponent>(head)
                    .unwrap()
                    .transform
                    .matrix_world[3][1] += 0.23;
                let moved =
                    OpenXRSystem::eye_cameras(&world, &visuals, sampled_head, &views).unwrap();
                near(
                    moved[0].transform.matrix_world[3][1] - eyes[0].transform.matrix_world[3][1],
                    0.23,
                );
            }
        }
        world
            .get_component_by_id_as_mut::<InputXRComponent>(input)
            .unwrap()
            .pose_valid = false;
        assert!(
            OpenXRSystem::eye_cameras(&world, &visuals, openxr::Posef::IDENTITY, &[]).is_none()
        );
    }

    #[test]
    fn xr_settings_corrected_mapping_preserves_full_eye_orientation_under_scaled_rig() {
        let raw_head =
            OpenXRSystem::pose_from_quat_translation(math::quat_rotation_y(0.9), [1.0, 1.7, -0.5]);
        let rig = TransformComponent::new()
            .with_position(2.0, 4.0, -3.0)
            .with_scale(2.0, 2.0, 2.0)
            .transform
            .model;
        let head_world =
            OpenXRSystem::mul_mat4(&rig, &OpenXRSystem::corrected_head_pose(raw_head, 0.15));
        let mapping = OpenXRSystem::world_from_stage(head_world, raw_head);
        let eye = OpenXRSystem::pose_from_quat_translation(
            math::quat_rotation_y(1.0),
            [1.02, 1.7, -0.49],
        );
        let world_eye = OpenXRSystem::mul_mat4(&mapping, &OpenXRSystem::mat4_from_pose(eye));
        near(world_eye[3][1], 4.0 + 2.0 * (1.7 - 0.15));
        // Eye orientation need not equal the derived head orientation.
        near(world_eye[0][0], 2.0 * 1.0f32.cos());
        near(world_eye[0][2], -2.0 * 1.0f32.sin());
    }
}
