// PROPOSED API -- syntax/specification example; not runnable yet.
// VMN (VectorMathNode) is the candidate replacement name for MG (MathGraph).
// One composite node, one rotation output, exactly one direct child T.
// This extracts the larger pose driver's preferred_neck computation verbatim,
// flattening its scope. It does not implement bounded positional closure.

VMN {
    // Native live inputs supplied by AVC; these strings name input slots.
    // Quaternions are xyzw, calibrated to +Y up / -Z forward, identity rest.
    let head_orientation = ValueSource
        .input("head_world_orientation")
        .type("quaternion")

    let body_yaw = ValueSource
        .input("body_world_yaw")
        .type("quaternion")

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

    // Explicit export; Quaternion uniquely maps to rotation among TRS channels.
    // Keep .rotation while the inferred-export shorthand is still undecided.
    TransformChannelOutput
        .rotation(orientation)
        .space("world")

    // The only exported bundle is applied automatically before this T's local
    // TRS. Incoming translation/scale are preserved. AVC would insert this
    // identity rotation boundary at the calibrated neck pivot in the real rig.
    T { name = "preferred_neck_rotation" }
}
