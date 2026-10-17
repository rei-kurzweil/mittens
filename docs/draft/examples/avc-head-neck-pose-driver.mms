// PROPOSED MMS SPECIFICATION -- NOT A RUNNABLE ENGINE EXAMPLE YET.
// MathGraph/MG, Vec/Quat, typed ports and output adapters are not implemented.
// Read ../avc-head-neck-pose-driver-example.md for bindings and acceptance cases.
//
// This spells out both head/neck slices using a deliberately simple calibrated
// rig. It is not the full real-rig FK/limit projection or the separate crouch IK.
// No MMS callbacks, array arithmetic or scene queries run per pose update.

fn head_neck_pose_driver(source_kind, bounded_follow) {
    // These arguments select graph structure ONCE, during construction:
    // source_kind = "desktop" or "xr"; bounded_follow = true or false.
    // MG is the proposed shortform of MathGraph.
    return MG {
        // Named native input bindings, not string queries into the scene.
        // Head poses have already been converted from eye/HMD to head pivot.
        // body_yaw comes from the ONE existing TOP.yaw_follow owner.
        let head_orientation = ValueSource
            .input("head_world_orientation")
            .type("quaternion")
        let body_yaw = ValueSource
            .input("body_world_yaw")
            .type("quaternion")
        let locomotion_anchor = ValueSource
            .input("body_anchor_world")
            .type("vec3")

        // Synthetic fixture, not Bisket/Rei(mu) dimensions or shipping defaults.
        // Metres, radians, quaternion xyzw; +Y up, -Z forward, identity rest
        // rotations. Uniform scale has already been applied exactly once.
        let root_to_neck_rest = ValueSource.constant([0.02, 1.30, 0.03])
        let neck_to_head_rest = ValueSource.constant([0.0, 0.12, 0.0])
        let root_to_neck = Vec
            .rotate()
            .rotation(body_yaw)
            .vector(root_to_neck_rest)
        let neutral_arc = Vec
            .rotate()
            .rotation(body_yaw)
            .vector(neck_to_head_rest)
        let neutral_root_to_head = Vec
            .add()
            .left(root_to_neck)
            .right(neutral_arc)

        // NESTED GRAPH, ONE OUTPUT BUNDLE: a rotation channel.
        // All identifiers passed below refer to nodes/ports, not MMS math.
        let preferred_neck = MG {
            let relative_head = Quat
                .relative()
                .reference(body_yaw)
                .desired(head_orientation)
            let pitch = Quat
                .pitch()
                .rotation(relative_head)
                .frame("y_up_negative_z_forward")
            let shared_pitch = ScalarArithmetic
                .multiply()
                .left(pitch)
                .right(0.5)
            let bounded_pitch = ScalarClamp
                .value(shared_pitch)
                .minimum(-0.4363323129985824)
                .maximum(0.4363323129985824)

            let bend = Quat
                .axis_angle()
                .axis([-1.0, 0.0, 0.0])
                .radians(bounded_pitch)
            let orientation = Quat
                .compose()
                .left(body_yaw)
                .right(bend)

            TransformChannelOutput
                .rotation(orientation)
                .space("world")
        }

        // A nested graph's .output.rotation is a live Quaternion port.
        let preferred_arc = Vec
            .rotate()
            .rotation(preferred_neck.output.rotation)
            .vector(neck_to_head_rest)

        // NESTED GRAPH, ONE OUTPUT BUNDLE: translation AND rotation.
        // Its construction-time branch chooses source authority. Both modes
        // subsequently use exactly the same placement/follow graph.
        let desired_head = MG {
            let position = null
            if source_kind == "desktop" {
                // Intentional locomotion supplies the body anchor. Look makes
                // a connected head arc; it never rewrites that anchor.
                position = Vec
                    .add()
                    .left(locomotion_anchor)
                    .right(Vec
                        .add()
                        .left(root_to_neck)
                        .right(preferred_arc))
            } else {
                // XR: the measured head pivot stays authoritative.
                // Never add the synthetic desktop neck arc to XR tracking.
                position = ValueSource
                    .input("head_world_position")
                    .type("vec3")
            }

            TransformChannelOutput
                .translation(position)
                .rotation(head_orientation)
                .space("world")
        }

        // NESTED GRAPH, THREE NAMED OUTPUT BUNDLES.
        // The two branches are implementation slices, not two active writers.
        let placement = MG {
            let body_position = null
            let neck_orientation = null

            if bounded_follow {
                // SLICE 2: total neutral-relative X/Z range, shared by drivers.
                let neutral_target = Vec
                    .project()
                    .value(Vec
                        .subtract()
                        .left(desired_head.output.translation)
                        .right(neutral_root_to_head))
                    .axes("xz")

                // Native temporal owner. The binding named "locomotion"
                // transports retained state when simulated locomotion moves.
                // .candidate is a live Vec3, not an automatically committed b.
                let follow = PlanarFollow
                    .target(neutral_target)
                    .radius(0.06)
                    .frame("locomotion")
                    .initial_anchor(locomotion_anchor)

                // Geometric closure for ONE upright 0.12 m segment with a
                // symmetric 25-degree swing cone. This fixture permits all
                // residual head rotations; stricter anatomical limits are not
                // hidden inside Quat.from_directions.
                let head_from_candidate_neck = Vec
                    .subtract()
                    .left(desired_head.output.translation)
                    .right(Vec
                        .add()
                        .left(follow.candidate)
                        .right(root_to_neck))

                let requested_horizontal = Vec
                    .project()
                    .value(head_from_candidate_neck)
                    .axes("xz")
                let requested_distance = Vec
                    .length()
                    .value(requested_horizontal)

                // reach_xz = 0.12 * sin(25 degrees), calibrated at build time.
                // A comfort radius is not a guarantee of anatomical reach.
                let reach_xz = ValueSource.constant(0.05071419140888393)
                let distance_guard = ScalarArithmetic
                    .maximum()
                    .left(requested_distance)
                    .right(0.000001)
                let horizontal_gain = ScalarClamp
                    .value(ScalarArithmetic
                        .divide()
                        .left(reach_xz)
                        .right(distance_guard))
                    .minimum(0.0)
                    .maximum(1.0)

                let horizontal = Vec
                    .multiply()
                    .vector(requested_horizontal)
                    .scalar(horizontal_gain)

                // Preserve segment length; solve its Y, rather than freezing
                // the body Y or stretching a neck to meet the tracked head.
                let length_squared = ScalarArithmetic
                    .multiply()
                    .left(0.12)
                    .right(0.12)
                let horizontal_squared = Vec
                    .dot()
                    .left(horizontal)
                    .right(horizontal)
                let height_squared = ScalarClamp
                    .value(ScalarArithmetic
                        .subtract()
                        .left(length_squared)
                        .right(horizontal_squared))
                    .minimum(0.0)
                    .maximum(length_squared)

                let height = ScalarArithmetic
                    .sqrt()
                    .value(height_squared)
                let required_arc = Vec
                    .add()
                    .left(horizontal)
                    .right(Vec
                        .multiply()
                        .vector([0.0, 1.0, 0.0])
                        .scalar(height))

                // Meet the positional target while remaining close to the
                // preferred rotation. Preferred and required arc have equal
                // length and stay in the supported upper hemisphere.
                let swing_correction = Quat
                    .from_directions()
                    .from(preferred_arc)
                    .to(required_arc)
                neck_orientation = Quat
                    .compose()
                    .left(swing_correction)
                    .right(preferred_neck.output.rotation)

                body_position = Vec
                    .subtract()
                    .left(Vec
                        .subtract()
                        .left(desired_head.output.translation)
                        .right(root_to_neck))
                    .right(required_arc)

                // PROPOSED construction-time wiring method: register a delayed
                // acceptance edge, NOT an MMS callback or a state write now.
                // Commit final body X/Z (converted to the locomotion frame)
                // only after ALL related output bundles are accepted/applied.
                // Never commit the unvalidated follow.candidate instead.
                follow.accept_from(body_position)
            } else {
                // SLICE 1: rotation-derived immediate placement, no history.
                // P_body = P_head - c - rotate(Q_neck, rest_segment).
                // This follows positional input 1:1; it has no lean allowance.
                neck_orientation = preferred_neck.output.rotation
                body_position = Vec
                    .subtract()
                    .left(Vec
                        .subtract()
                        .left(desired_head.output.translation)
                        .right(root_to_neck))
                    .right(preferred_arc)
            }

            let neck_position = Vec
                .add()
                .left(body_position)
                .right(root_to_neck)

            TransformChannelOutput
                .name("body")
                .translation(body_position)
                .rotation(body_yaw)
                .space("world")

            TransformChannelOutput
                .name("neck")
                .translation(neck_position)
                .rotation(neck_orientation)
                .space("world")

            TransformChannelOutput
                .from(desired_head.output)
                .name("head")
        }

        // Forward bundles from a nested graph without applying a matrix or
        // copying sampled values. One bundle uses .output; named multiple
        // bundles use .outputs.<name>. These are retained graph port accesses.
        let body_pose = TransformChannelOutput
            .from(placement.outputs.body)
            .name("body")
        let neck_pose = TransformChannelOutput
            .from(placement.outputs.neck)
            .name("neck")
        let head_pose = TransformChannelOutput
            .from(placement.outputs.head)
            .name("head")

        // Three distinct destinations require selection. The body branch uses
        // another MG to forward exactly one bundle; its child T then receives
        // the automatic adapter. Neck/head use the explicit TOP adapter instead.
        // Both forms consume the SAME bundle interface. These empty
        // identity T nodes stand for effective pose boundaries, not setters
        // for existing bone locals. Real AVC binding must preserve the original
        // FK path/rest offsets and avoid applying model calibration twice.
        MG {
            TransformChannelOutput.from(body_pose)
            T { name = "spec_body_root" }
        }
        TOP.channels(neck_pose) {
            T { name = "spec_neck_base" }
        }
        TOP.channels(head_pose) {
            T { name = "spec_head_pivot" }
        }
        // Do not mount an XR camera or collision capsule under these outputs.
        // Propagate the accepted pose before arm IK, crouch IK and skinning.
    }
}

// Default reading: XR with bounded planar follow and explicit geometric closure.
// Alternative construction cases to compare (one at a time):
// head_neck_pose_driver("xr", false)
// head_neck_pose_driver("desktop", false)
// head_neck_pose_driver("desktop", true)
head_neck_pose_driver("xr", true)
