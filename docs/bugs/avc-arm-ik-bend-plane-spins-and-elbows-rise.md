# AVC arm IK bend plane spins and elbows rise

## Status

Open investigation. An opt-in forbidden-angle prototype is implemented for the
inspection scene; it is not yet a confirmed fix for the elbow flip.

## Observed behavior

In the VR mirror, the bend-plane normal sometimes appears to rotate around body-local Z as a hand moves. At those times an elbow can bend the wrong way. A second symptom is that an elbow can rise to shoulder height while the hands are held against the chest; the desired pose keeps the elbows lower. These may have different causes.

The user checked forearm roll separately and found that it is not causing this bend-plane behavior. Hand movement relative to the solver's current arm pose is a more useful lead. The other IK debug vectors may also be wrong, so the normal alone should not be treated as proof of the cause.

## Repro and existing capture

- Scene: [examples/ik-rest-pose-vr.mms](../../examples/ik-rest-pose-vr.mms), launched with `cargo run --release --example ik-rest-pose-vr -- --model rei --rest-pose a`.
- Use the mirror and IK debug lines while moving a hand through the troublesome region. The magenta line is the bend-plane normal, cyan is the pole direction, green is the solved elbow direction, and white is the solved elbow point.
- Press right-controller B for the four prompted poses. The launcher writes JSONL captures under `data/ik-rest-pose/` (ignored by Git).
- Initial capture: `data/ik-rest-pose/rei-a-1790787315675.jsonl`, four poses, created before the scene's pole Y values changed from `-0.35` to `-1.5`. The capture is a set of still poses, not a trace of the reported spin.

| Captured pose | Left elbow below shoulder | Right elbow below shoulder | Hand target error | Observation |
| --- | ---: | ---: | --- | --- |
| Hands against chest | 0.5 cm | 4.5 cm | about 0.1 mm each | Both hands reach; elbows are too high. |
| Hands out front | elbow slightly above shoulder | elbow slightly above shoulder | 25–28 cm | Targets exceed the roughly 42.7 cm arm reach. |
| Hands at lap / hips | 21 cm | 21 cm | about 28 cm | Arms are nearly straight. |
| Hands at sides | 22 cm | 22 cm | about 32 cm | Arms are nearly straight. |

The chest sample shows a high-elbow problem even with a reachable target. In the other three samples the elbow bend is about 1.7–1.8 degrees because the targets are beyond reach. Those near-straight poses give little visible elbow displacement, and four still samples cannot establish when or why the normal spins.

The example now uses pole hints `[1, -1.5, 1]` and `[-1, -1.5, 1]` as a scene-level adjustment. Applying those hints to the *captured chest positions* predicts elbows roughly 14–16 cm below the shoulders. This is a geometric estimate; it has not yet been checked visually in XR and does not address the reported normal spin.

The example now shows two grabbable readouts in front of the avatar. Each reports both the raw bend normal's signed body-local azimuth around +Z and the last normal used by IK in degrees (`atan2(Y, X)`, with +X at 0°). A readout turns red when the last solve adjusted its raw normal, and returns to white otherwise. New B-button captures save both angles. The value wraps at ±180°; a numeric wrap is not itself a plane flip. The `FrameTick` readout precedes the IK pass, so it may trail the debug line during fast motion. The left arm excludes the interiors of -178° through -115° and -100° through -60° as a prototype; the right arm remains unrestricted.

## Solver path to inspect

In [src/engine/ecs/system/ik_system.rs](../../src/engine/ecs/system/ik_system.rs), `solve_two_bone` reads the current FK joint positions and target position each tick. For an AVC arm it rotates the authored pole from body-local space into world space, then builds:

```text
reach = target - upper_arm
normal = normalize(reach × pole_world)
elbow_direction = reach_direction * cos(upper_angle)
                + normalize(normal × reach_direction) * sin(upper_angle)
```

The cross product means the normal can rotate as the reach vector moves even when the pole is fixed in body space. It can also become ill-conditioned if reach and pole approach parallel; the solver then uses a fallback axis. The current capture's pole projections are substantial, so it does **not** demonstrate that singular case.

After solving the elbow, the solver aligns upper and lower bones to the new directions and then applies wrist-matching twist to the lower arm. Wrist twist is downstream of the bend-plane calculation and cannot directly set the debug normal. Given the same upper-arm root position, target position, body rotation, and pole, the normal calculation does not depend on the previous upper- or lower-arm rotation. The user's state-dependence hypothesis is still worth checking for the **visible elbow result** or for upstream motion of those inputs; it is not yet an explanation for the raw normal itself.

## Hypothesis and possible constraint

The current prototype lets AVC exclude one or more body-local Z azimuth intervals per arm. It searches for a nearby valid plane by rotating the normal around the shoulder-to-target reach. This keeps the normal perpendicular to reach. A previous plane biases which side of a forbidden interval is chosen while the raw normal remains inside it. It does not constrain the hand target, guarantee a feasible allowed angle for every reach, or prevent a jump when the raw normal exits on the far side. It is a candidate mitigation, not a confirmed root cause.

A plane normal must remain perpendicular to the shoulder-to-target reach. A naive component-wise clamp of the normal would violate that requirement. Investigate a preferred body-local elbow direction or an angular sector, project the preference onto the plane perpendicular to reach, then reconstruct a valid normal. Near a degenerate projection, use a continuous previous valid plane with a defined recovery rule. Keep left and right limits mirrored and verify that ordinary overhead and cross-body reaches still work.

## Next investigation

1. Record a per-frame trace while moving each hand slowly through a wrong-way bend and a high-elbow event. Include shoulder, upper arm, current and solved elbow, hand and target positions; model root rotation; pole, reach, pole projection length, raw normal, chosen normal, and elbow height relative to shoulder. Express the vectors in both world and body-local coordinates.
2. Plot or inspect the signed body-local normal angle around Z over time. Check whether a sudden jump, a continuous sweep, or a near-parallel reach/pole condition coincides with each visible failure. Compare the raw normal with the direction of the actual shoulder–elbow–hand plane.
3. Repeat with a stationary wrist orientation while moving the hand target, and with a stationary hand position while rotating the wrist. This tests the user's observation that forearm roll is separate.
4. Compare the same target path from different starting arm poses. If the result depends on history, inspect live FK reads, transform update order, and any discontinuity in the shortest-arc rotations before adding an angular limit.
5. Prototype a body-local allowed region only after the trace identifies an offending region. Evaluate both sides, reachable chest poses, near-full extension, cross-body movement, and body yaw changes. Record whether target error, elbow height, or normal continuity improves without new snapping.

## Success criteria

- Elbows stay on the intended side and below shoulder height in ordinary hands-to-chest poses.
- The bend plane changes continuously during hand motion; it does not flip or spin into a wrong-way elbow bend.
- Hand targets remain reachable when within the model's arm length, and body turns do not change the intended body-local elbow behavior.
- Any angular constraint preserves a plane perpendicular to reach and has a defined behavior near singular configurations.

## Related work

- [Body-local bend-plane limit](../task/avc-arm-ik-body-local-bend-plane-limits.md): opt-in prototype and remaining behavior to evaluate.
- [AVC arm IK control inventory and MMS semantics](../task/avc-arm-ik-control-inventory-and-mms-semantics.md): current ownership and possible manual-control surface.
- [Earlier right-arm inward bend](avc-right-arm-two-bone-ik-bends-inward.md): fixed a body-local pole-space issue; this investigation concerns behavior that remains with the current solver.
- [XR wrist kink and forearm roll](xr-hand-tracking-wrist-kink-and-jitter.md): a separate orientation/mesh issue.
