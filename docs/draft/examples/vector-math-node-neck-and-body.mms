// PROPOSED API -- syntax/specification example; not runnable yet.
// VMN (VectorMathNode) is the candidate replacement name for MG (MathGraph).
// One composite node, two output bundles, no nested composite nodes or state.
// Same preferred-neck math as vector-math-node-neck-rotation.mms, extended by
// the larger pose driver's stateless immediate-placement calculation.

VMN {
    // Calibrated head pivot (not raw HMD/eye), head orientation and body yaw.
    // All values are world-space; quaternion xyzw, +Y up, -Z forward.
    let head_position = ValueSource
        .input("head_world_position")
        .type("vec3")
    let head_orientation = ValueSource
        .input("head_world_orientation")
        .type("quaternion")
    let body_yaw = ValueSource
        .input("body_world_yaw")
        .type("quaternion")

    // Reusable preferred neck rotation; 0.5 share, +/-25 degrees in radians.
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

    // Synthetic rest geometry, in metres: identity rest rotations, unit scale.
    // Real AVC substitutes values from the retained calibrated humanoid path.
    let root_to_neck_rest = ValueSource.constant([0.02, 1.30, 0.03])
    let neck_to_head_rest = ValueSource.constant([0.0, 0.12, 0.0])
    let root_to_neck = Vec
        .rotate()
        .rotation(body_yaw)
        .vector(root_to_neck_rest)
    let arc = Vec
        .rotate()
        .rotation(orientation)
        .vector(neck_to_head_rest)
    let head_minus_root_to_neck = Vec
        .subtract()
        .left(head_position)
        .right(root_to_neck)
    let body_position = Vec
        .subtract()
        .left(head_minus_root_to_neck)
        .right(arc)

    // The SAME orientation node drives neck rotation and the arc calculation.
    // Vec3 needs an explicit translation/scale role; Quaternion means rotation.
    let neck_output = VMNOutput
        .name("neck")
        .rotation(orientation)
        .space("world")
    let body_output = VMNOutput
        .name("body")
        .translation(body_position)
        .space("world")

    // Two destinations: select each bundle explicitly. These identity T nodes
    // illustrate channel application, not a complete imported skeleton. Neck
    // pivot placement/rest offsets and the body's existing yaw basis belong to
    // rig integration; neither is secretly supplied by these one-channel exports.
    TOP.channels(neck_output) {
        T { name = "preferred_neck_rotation" }
    }
    TOP.channels(body_output) {
        T { name = "immediate_body_translation" }
    }
    // Head pose stays authoritative. No camera, tracking origin or capsule
    // write; bounded follow would add state/closure before these final outputs.
}
