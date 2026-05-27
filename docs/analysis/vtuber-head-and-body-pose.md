# VTuber Head & Body Pose — Diagnostics

Historical note: topology references below to `TransformPipelineComponent` or `TransformPipelineOutput` describe the removed authored wrapper/output topology. The current authored form is `TransformForkTRS` as the pipeline root with direct downstream children.

## Topology overview (vtuber-desktop)

```
editor_root
  └── body_input  (InputComponent, fps_rotation, roll_axis_z, speed 1.5)
        └── body_driven_t  (TransformComponent — driven by InputSystem)
              └── av_pipeline  (TransformPipelineComponent — drops rotation)
                    └── av_output
                          └── avatar_body_yaw  (AvatarBodyYawComponent — reads body_driven_t yaw)
                                └── model_root
                                      └── GLTFComponent
                                            └── [armature]
                                                  J_Bip_C_Neck
                                                    └── head_input  (InputComponent, fps_rotation, roll_axis_y, speed 0)
                                                          ├── head_input_mode
                                                          └── driven_t  (TransformComponent)
                                                                └── pipeline  (SampleAncestor skip=1 → J_Bip_C_Neck pos)
                                                                      └── output
                                                                            └── J_Bip_C_Head  ← displaced here
```

## Topology overview (vr-input)

```
editor_root
  └── avatar_input_xr  (InputXRComponent — HMD body pose)
        └── avatar_driven_t  (TransformComponent — driven by OpenXRSystem)
              └── av_pipeline  (drops rotation)
                    └── av_output
                          └── avatar_body_yaw  (reads avatar_driven_t yaw)
                                └── model_root  (with_rotation_euler Y=π)
                                      └── GLTFComponent
                                            └── [armature]
                                                  J_Bip_C_Spine ... J_Bip_C_UpperChest
                                                    └── J_Bip_C_Neck's PARENT
                                                          └── splice_input_xr  (InputXRComponent — HMD head pose)
                                                                └── driven_t
                                                                      └── pipeline  (SampleAncestor skip=1 → neck parent pos)
                                                                            └── output
                                                                                  └── yaw_correction  (rotation_euler Y=π)
                                                                                        └── J_Bip_C_Neck  ← displaced here
                                                                                              └── J_Bip_C_Head
```

---

## Bug 1 — Splice point inconsistency

| Example | Splice inserted under | Bone displaced | Effect |
|---|---|---|---|
| vr-input | neck's parent | J_Bip_C_Neck | neck **and** head rotate as a unit from HMD |
| vtuber-desktop | J_Bip_C_Neck | J_Bip_C_Head only | only the head bone rotates; neck stays in armature |

This is an incidental divergence, not a deliberate design choice. Neither was derived
from the other. Which is correct depends on intent:

- **Neck displaced (vr-input style)**: more natural for HMD tracking; neck+head move
  together so the whole visible neck-to-head column follows the user's real head.
- **Head displaced only (vtuber-desktop style)**: finer isolation; the neck bone's
  skinned geometry stays under armature control. Correct if the neck bones are driven
  by a separate IK or blend system. Currently in vtuber-desktop there is no such
  system, so the neck just stays static while only the head rotates — looks detached.

The `SampleAncestor skip=1` comment in vtuber-desktop says it samples **neck** world
position. In vr-input the same skip=1 samples the neck's **parent** world position.
These are inconsistently labeled in the comments and should be verified.

---

## Bug 2 — Torso rotates when it shouldn't

### Cause (mouse drag)

`InputSystem` processes **all** `InputComponent` nodes each tick. Both `body_input` and
`head_input` receive the same right-click-drag event simultaneously.

- `head_input` (fps_rotation, roll_axis_y): drag adds to `yaw` on `driven_t` → head yaw
  changes. ✓
- `body_input` (fps_rotation, roll_axis_z): drag also adds to `yaw` on `body_driven_t`
  (fps_rotation accumulates yaw from mouse X drag regardless of roll_axis).

`AvatarBodyYawSystem` reads yaw from `body_driven_t.matrix_world`. When `body_input`
accumulates yaw from mouse drag, the system sees a changed body yaw and fires a
`UpdateTransform` on `model_root`. The torso follows.

This is the primary cause of torso rotation with mouse drag. Even though the user only
intends to rotate the head, both inputs are listening.

### Cause (Q/E)

`body_input` (roll_axis_z): Q/E → `roll += delta` around `fwd_world` (local forward of
`body_driven_t`). For an unrotated `body_driven_t`, `fwd_world ≈ -Z` (forward_z), so
Q/E produces a Z-bank (lean) on `body_driven_t`. The Z column of `body_driven_t` does
not change under pure Z-bank at rest, so `AvatarBodyYawSystem`'s yaw extraction should
be unaffected at zero prior rotation.

However, once the user has used mouse drag (yaw + pitch on `body_driven_t`), the
accumulated `fwd_world` rotates away from -Z. Subsequent Q/E roll is then applied
around a tilted axis, which **does** shift the Z column and confuses yaw extraction →
spurious model_root rotation.

---

## Bug 3 — Q/E produces wrong-axis head rotation

`head_input` (fps_rotation, roll_axis_y): Q/E → `yaw += qe_delta` → `driven_t` gets a
pure world-Y quaternion. This is geometrically correct: `compute_rotation_fps` builds
`q_yaw = quat_from_axis_angle([0,1,0], yaw)`. The head bone should yaw around world Y.

The visual artefact ("wrong axis") is likely a compound of:

1. **Torso also rotating** (Bug 2): the torso co-rotates, making the head rotation look
   wrong relative to the body even if the head bone itself is correct.
2. **Neck stays static** (Bug 1): because the splice only displaces the head, the neck
   skinned mesh doesn't follow. The result is a snapping or stretching at the
   neck-to-head junction that reads visually as incorrect axis rotation.
3. **Both inputs yawing simultaneously on Q/E** (possible): if `head_input` also
   receives Q/E and accumulates yaw, and the `AvatarBodyYawSystem` fires from
   `body_driven_t` yaw change at the same frame, the two rotations compound.

---

## Root causes summary

| # | Symptom | Root cause |
|---|---|---|
| 1 | Splice point differs between examples | Incidental divergence; not designed |
| 2 | Torso rotates on mouse drag | `body_input` and `head_input` share all input; `body_driven_t` accumulates mouse-drag yaw; `AvatarBodyYawSystem` reacts |
| 3 | Torso rotates on Q/E (after prior mouse drag) | Prior mouse-drag yaw on `body_driven_t` tilts `fwd_world`; Q/E roll then perturbs Z column → spurious yaw extraction |
| 4 | Head rotation looks wrong on Q/E | Torso co-rotating (bug 2/3) plus neck not following (bug 1) makes geometrically-correct head yaw look incorrect |

---

## Proposed fixes (not yet implemented)

### Fix A — Input isolation
Prevent `body_input` from responding to right-click drag and Q/E. One option: a flag on
`InputTransformModeComponent` that disables mouse-look so only keyboard translation is
active. Another: route head rotation through a mechanism not shared with `body_input`.

### Fix B — Decouple AvatarBodyYaw from body_driven_t rotation
`AvatarBodyYawSystem` on desktop should not watch `body_driven_t` rotation (which
conflates walk direction with head orientation). Options:
- Derive body yaw from walk velocity direction (translation delta), not transform rotation.
- Or: have the system read yaw from a dedicated "body facing" signal, not from the
  driven transform's matrix.

### Fix C — Standardize splice point to neck
Align vtuber-desktop splice with vr-input: displace J_Bip_C_Neck (not just the head).
Add a π Y `yaw_correction` node as in vr-input so the bone's natural facing direction
is preserved after displacement. This fixes the static-neck visual artefact.
