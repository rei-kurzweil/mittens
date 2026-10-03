# Task proposal: body-local bend-plane limits for AVC arm IK

Status: opt-in forbidden-angle prototype implemented; live XR behavior remains to be evaluated.

Related investigation: [AVC arm IK bend plane spins and elbows rise](../bugs/avc-arm-ik-bend-plane-spins-and-elbows-rise.md).

## Problem and measurement

The two-bone solver forms a world-space bend-plane normal from `cross(target - upper_arm, pole_world)`. The VR inspection scene now displays its signed azimuth about body-local +Z for each arm. The readout uses `atan2(normal_local.y, normal_local.x)` in degrees: body-local +X is 0°, positive angles turn toward +Y, and the displayed range wraps at ±180°. Crossing that display boundary alone is not a physical flip.

The readout marks the angle undefined when the cross product or its body-local XY projection is too small. Its `FrameTick` sample precedes the IK pass in the frame, so the number may trail the drawn debug normal during quick motion; use a recorded time series for exact event ordering.

The reported failures are (1) an elbow bending the wrong way as the normal moves around body-local Z and (2) elbows rising too high for a reachable hands-to-chest pose. A lower pole Y value in the inspection scene addresses the second symptom in a geometric estimate; it does not establish a fix for the first. A moving trace, rather than the four captured still poses, is needed to identify an offending angular region.

## Proposed behavior

Allow an AVC arm to declare an angular region for its bend plane in **body-local space**, with independent lower and upper bounds for left and right arms. The region constrains which side of the shoulder-to-target reach the elbow may occupy. It should rotate with the model root and should not depend on world yaw.

The limits are optional. Leaving them unset preserves the previous solver. The current MMS methods take degrees to match the inspection readout:

```mms
AVC {
    // Repeat either method to exclude more than one interval. Degrees.
    left_arm_forbidden_bend_normal_z_degrees(-178.0, -115.0)
    left_arm_forbidden_bend_normal_z_degrees(-100.0, -60.0)
    right_arm_forbidden_bend_normal_z_degrees(115.0, 178.0)
}
```

The two left intervals are the current inspection-scene prototype; the right interval is illustrative and is not enabled there. Each call accepts two endpoints within [-180°, 180°], sorts them, and excludes the interior. To cover the ±180° wrap, use two intervals. No calls preserve the current solver behavior.

## Geometry contract

The solved normal must remain perpendicular to the current shoulder-to-target reach. Clamping the X/Y values or the azimuth of a normalized 3D normal directly can violate this. A candidate algorithm is:

1. Rotate the raw normal into body-local space and measure its azimuth around +Z.
2. If the azimuth lies outside the allowed interval, select the closest allowed boundary with a continuity rule that avoids jumping between boundaries.
3. Construct a preferred elbow-plane direction at that boundary; project it onto the plane perpendicular to the **current** reach, then normalize it and reconstruct a perpendicular normal.
4. If projection length is too small, retain a previous valid plane for that arm or use a specified mirrored anatomical fallback. Never normalize a near-zero vector.
5. Apply the resulting normal before computing the elbow position and upper/lower bone rotations. Keep wrist matching downstream.

The prototype searches the reach-perpendicular circle at 0.5° increments for the first permitted normal in either direction, preferring the prior solved side while the raw angle remains forbidden. If no permitted direction is found it keeps the raw plane. A body-local azimuth sector may be infeasible for some reach directions, and rapid target motion can still snap on leaving an interval. These cases require live evaluation before treating this as an anatomical safety limit.

## Evidence gate before implementation

- Trace each frame's target and root positions, body rotation, pole, pole projection length, raw normal, actual elbow plane, displayed azimuth, and elbow height through both failures.
- Check whether the apparent spin is a continuous sweep, a sudden normal reversal, an `atan2` wrap, or a reach/pole singularity.
- Replay the same target path from different initial arm poses to test the suspected history dependence. The raw normal formula itself has no previous-arm-rotation input when root, target, body rotation, and pole are held fixed.
- Choose left and right candidate regions from observed good and bad poses, including hands at chest, out front, at sides, above the head, and across the body.

## Implementation boundaries and acceptance

The first implementation should be opt-in per arm on AVC, with a solver-level representation that can later serve a manually authored `IKChain`. It should not silently alter other `TwoBoneIK` chains or current AVC defaults. Record raw and constrained normals in debug output so the limit remains inspectable.

Accept the change only if wrong-way bends and high elbows are reduced in a live XR replay; reachable hand error stays within current tolerance; the normal remains perpendicular to reach; left/right behavior mirrors correctly; and body yaw, ±π display wrap, and near-straight reach do not cause new snaps.

Related API sketch: [Manual control of AVC arm IK and MMS semantics](avc-arm-ik-control-inventory-and-mms-semantics.md).
