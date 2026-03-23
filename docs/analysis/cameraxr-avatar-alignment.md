# CameraXR + Avatar Alignment Audit

Date: 2026-03-23

This doc audits the full data path from physical HMD pose → ECS → rendered XR view and
avatar head bone, explains why CXR re-parenting to a bone is a no-op for rendering, and
explores the user hypothesis that "the head bone should just copy the InputXR pose directly".

---

## 1. How the rendered XR view is computed

`OpenXRSystem` computes the final per-eye view matrices in two steps:

### Step 1 — `xr_rig_origin_world`

```rust
fn xr_rig_origin_world(world: &World, visuals: &VisualWorld) -> [[f32; 4]; 4] {
    let camera_cid = visuals.active_xr_camera()
        .or_else(|| Self::first_enabled_camera_xr(world));
    if let Some(input_xr_cid) = Self::input_xr_ancestor(world, camera_cid) {
        if let Some(driven_transform) = Self::transform_child_of(world, input_xr_cid) {
            return Self::transform_parent_world(world, driven_transform);
        }
    }
    // fallback: TransformSystem::world_model of camera itself
}
```

Call chain for the avatar setup:
1. Find active CXR — after init this is the CXR that was re-parented under `J_Bip_C_Head`.
2. Walk up ancestors: `J_Bip_C_Head` → `splice_head` → various bones → `model_root` → pipeline output
   → `AVC` → **`driven_t`** → **`avatar_input_xr`** ← found.
3. Find TC child of `avatar_input_xr` → `driven_t` itself.
4. `transform_parent_world(world, driven_t)`:
   ```rust
   world.parent_of(driven_t)               // = avatar_input_xr (not a TC)
       .and_then(|p| TransformSystem::world_model(world, p))  // None: InputXR is not a TC
       .unwrap_or(identity)                 // → identity
   ```

**`rig_world = identity`** regardless of where CXR is parented in the tree, as long as the
InputXR ancestor (`avatar_input_xr`) has no `TransformComponent` above it.

### Step 2 — Eye view matrix assembly

```rust
let rig_world = xr_rig_origin_world(...); // identity
for v in &views {
    let world_from_eye = mul_mat4(&rig_world, &mat4_from_pose(v.pose));
    //                 = identity × physical_eye_pose
    //                 = physical_eye_pose
    let view = invert_affine_transform(&world_from_eye);
    let proj = proj_from_fov_rh_zo(v.fov, 0.1, 100.0);
}
```

The rendered XR view is **100% determined by the raw physical eye pose from the OpenXR
runtime**. No ECS transform contributes to it. The rendered view is always exactly where
the player's eyes physically are.

### Implication

Re-parenting `CameraXRComponent` to any ECS node — including a head bone — has **no effect**
on the rendered XR image. The bone's world transform is not consulted anywhere in the view
matrix path. The only thing `CXR`'s position in the tree affects is which `InputXR` ancestor
gets found in step 2, which determines `rig_world`. In the current avatar setup, that ancestor
is always `avatar_input_xr` at floor level, so `rig_world = identity` in all cases.

---

## 2. How InputXR sets driven_t

```rust
let world_from_head = mul_mat4(
    &transform_parent_world(world, tcid),   // identity (InputXR parent = ED or root, not a TC)
    &mat4_from_pose(head_pose),             // raw HMD stage pose
);
// convert to local TRS on driven_t, emit UpdateTransform
t.transform.translation = local_translation;
t.transform.rotation    = local_rotation;
```

Since InputXR also has no TC parent here, `transform_parent_world = identity`, so:

```
driven_t.world_pos = hmd_stage_pos
driven_t.world_rot = hmd_stage_rot
```

`driven_t` tracks the HMD pose exactly. This is the authoritative ground truth for the
avatar's head position and orientation in the world.

---

## 3. How AVC computes the head bone rotation

Every tick, `tick_one` does:

```rust
let driven_world_rot = mat_to_quat(driven_t.transform.matrix_world);
// For VR (-Z forward):
let head_world_rot = quat_mul(driven_world_rot, quat_rotation_y(PI));
let splice_local_rot = quat_mul(quat_inverse(neck_parent_world_rot), head_world_rot);
// emit UpdateTransform(splice_head, rotation = splice_local_rot)
```

### What the handedness correction is doing

OpenXR forward is **-Z** (the headset looks down -Z when neutral).
VRM/glTF forward is **+Z** (bones face +Z in rest pose).

So when the player looks straight ahead, `driven_world_rot` has -Z forward. The neck bone
needs +Z forward. Post-multiplying by `rot_y(π)` in the `driven_t` frame rotates the -Z
forward axis to +Z — bridging the OpenXR/VRM handedness gap.

The computation as a world-space rotation:

```
splice_head_world_rot = neck_parent_world_rot × splice_local_rot
                      = neck_parent_world_rot × inv(neck_parent_world_rot) × head_world_rot
                      = head_world_rot
                      = driven_world_rot × rot_y(π)
```

So the splice node's world rotation = HMD rotation composed with a 180° Y flip.

### Where the body pipeline interacts

`neck_parent_world_rot` comes from the bone above the splice node, which is somewhere in the
armature under `model_root`. `model_root` is under the body pipeline output, which applies
`YawFollow` — stripping pitch and roll from `driven_t`, passing through only yaw (with a
lag/threshold). So:

```
neck_parent_world_rot ≈ rot_y(body_yaw)   (yaw-only; pitch/roll stripped)
```

```
splice_local_rot = inv(rot_y(body_yaw)) × driven_world_rot × rot_y(π)
```

When head yaw ≈ body yaw (within threshold, no body rotation happening):

```
splice_local_rot ≈ rot_y(-body_yaw) × rot_y(head_yaw) × rot_x(head_pitch) × rot_y(π)
                 ≈ rot_y(head_yaw - body_yaw) × rot_x(head_pitch) × rot_y(π)
                 ≈ rot_x(head_pitch) × rot_y(π)   (when head_yaw ≈ body_yaw)
```

The local splice rotation encodes head pitch relative to the body, plus the 180° Y flip.
The bone then sees this as a rotation in its local rest frame.

---

## 4. Why "head doesn't follow pitch as far" might occur

Two distinct phenomena could cause this perception:

### 4a. The bone's local frame versus the splice rotation

The neck bone (`J_Bip_C_Neck`) is "displaced" under the splice node. Its local-space axes
depend on the rest pose in the glTF. If the rest pose has the neck bone tilted slightly or
oriented differently from the world axes, the splice rotation (computed in world space)
applied to the splice node might not map cleanly to "pitch the bone forward".

The 180° Y at the end of the splice rotation expression means the bone's +Z-forward frame
ends up rotated 180° in the splice node's local space — which is correct for looking straight
ahead, but the local rotation axes for pitch then go through that 180° flip, potentially
swapping or scaling apparent pitch range.

### 4b. The XR view vs the bone

The XR view is 100% physical (no ECS influence). The bone is a computed approximation. If
the bone computation has any scale error in pitch (e.g. the 180° Y flip composes with pitch
differently than expected), the avatar's visual head will not match the physical head
orientation — even though the player's actual eye view is always correct.

### 4c. What the user's hypothesis implies

> "the head bone should be set to also copy that InputXR pose directly"

The hypothesis is: instead of computing a world-space rotation and transforming it into
bone-local space through the armature hierarchy, the splice node should be driven with a
rotation derived directly from the InputXR stage pose, without the intermediate decomposition
through `neck_parent_world_rot`.

The minimal version of this: set `splice_head` world rotation = `driven_world_rot × rot_y(π)`
— which is already what the code does. The deeper variant: bypass the neck-parent frame
entirely and just directly set the bone's world rotation to match driven_t.

---

## 5. What CXR bone re-parenting is actually useful for

Even though re-parenting CXR to the head bone has no effect on XR view rendering, it may
still be meaningful for:

- **Desktop mirror rendering**: a `Camera3D` or `CameraXR`'s ECS world position is used to
  compute a frustum for desktop window rendering. If CXR is under the head bone, the desktop
  mirror view may track the avatar's visual head — though this only matters if a distinct
  desktop-side camera consults the CXR node's world matrix.
- **Editor / inspector display**: the camera gizmo shows where the CXR node is in world space.
  Parenting it to the head bone makes it visually correlated with the avatar's head.
- **Future semantics**: if `xr_rig_origin_world` were changed to read the camera's world
  transform directly (rather than the InputXR-ancestor approach), then parenting to the bone
  would matter for rendering. The current code does not do this.

---

## 6. Correct conceptual model

### Physical eye view (what the player actually sees)

```
physical_eye_world = hmd_stage_pose × left/right_eye_offset
view_matrix = inverse(physical_eye_world)
```

No ECS component affects this. This is the ground truth.

### Avatar head bone (visual representation)

```
splice_head_world_rot = driven_t_world_rot × rot_y(π)
splice_head_world_rot = hmd_stage_rot × rot_y(π)
```

The head bone tries to visually match the physical head orientation, accounting for the
OpenXR vs VRM +Z convention. This is an approximation applied to the ECS skeleton — it
should produce a visually plausible result but is not mechanically linked to the eye view.

### Where they should converge

For a physically correct avatar, the head bone's world rotation in the rest-facing VRM frame
should be identical to the HMD orientation. The current formula achieves this for yaw and
pitch when the body and neck-parent transforms are correctly factored out. Any divergence
in apparent pitch range is a bug in the coordinate transform chain.

### The direct-copy approach

The most correct approach for the splice node:

```
splice_head_world_rot = hmd_stage_rot × rot_y(π)
                      = driven_t_world_rot × rot_y(π)
```

This is already what `tick_one` computes. The question is whether `splice_local_rot` is then
correctly derived from this:

```
splice_local_rot = inv(neck_parent_world_rot) × splice_head_world_rot
```

This is correct **if** `neck_parent_world_rot` is exactly the world rotation of the node
above the splice node at tick time. If there's any lag, error, or the body pipeline hasn't
settled, `neck_parent_world_rot` will be stale and the local rotation will be wrong.

---

## 7. Possible directions

### A. Verify neck_parent_world_rot is current at tick time

The body pipeline runs in `TransformPipelineSystem` before `AvatarControlSystem` runs.
Confirm the tick order guarantees neck_parent's world matrix is fresh when `tick_one` reads it.

### B. Verify the 180° Y correction doesn't double-apply

If `model_root` also has a 180° Y rotation baked in (e.g. from `initial_body_yaw = π`),
the neck parent's world rotation already incorporates a 180° Y. In that case, composing
another `rot_y(π)` for the handedness correction might not behave as expected when pitch
is also involved (rotation composition is not commutative).

### C. Measure pitch transmission ratio

Empirically: nod the head to a known angle, read driven_t's pitch, read the head bone's
world pitch. They should be equal. If the transmitted pitch is consistently smaller, the
`rot_y(π)` composition is attenuating it for some orientations.

### D. Direct bone world-rotation assignment

Instead of computing via local frame, emit a `SetWorldRotation` intent (if one existed)
that bypasses the local decomposition. This would make the bone's world rotation exactly
equal to `driven_world_rot × rot_y(π)`, independent of neck_parent_world_rot.

---

## 8. Summary

| Question | Answer |
|---|---|
| Does CXR re-parent to bone affect XR view? | No. `rig_world = identity` regardless. |
| What controls XR view? | Raw OpenXR stage poses, composed with `rig_world`. |
| What does `rig_world` depend on? | `world_model` of InputXR's parent — identity for root-level InputXR. |
| What drives the head bone? | `tick_one`: `driven_world_rot × rot_y(π)`, converted to bone-local space. |
| Why might head pitch look wrong? | Possible: rot_y(π) composes unexpectedly with pitch for certain orientations, or neck_parent_world_rot is stale. |
| User's hypothesis correct? | Partially: the formula already attempts a direct copy. The gap may be in how neck_parent_world_rot factors in. |
| Should CXR be a direct AVC child? | For current semantics: it only affects InputXR discovery, not the rendered view. |

---

## 9. Is the local decomposition actually "ignoring" parent propagation?

The user's framing: "move the subtree from the head in world coordinates and have the head ignore
the matrix propagation from the body."

The short answer: **the local decomposition already does this correctly.**

```
splice_local_rot = inv(neck_parent_world_rot) × desired_head_world_rot
```

After `UpdateTransform(splice_head, splice_local_rot)` propagates:

```
splice_head world_rot = neck_parent_world_rot × splice_local_rot
                      = neck_parent_world_rot × inv(neck_parent_world_rot) × desired_head_world_rot
                      = desired_head_world_rot
```

The parent contribution is algebraically cancelled. As long as `neck_parent_world_rot` is
current at the time `tick_one` runs, the splice node ends up at exactly the desired world
rotation, regardless of what the body pipeline does to the parent chain.

So the question is not "can we ignore parent propagation" (we already do) — it is **"is
`neck_parent_world_rot` always current when we read it?"**

`TransformPipelineSystem` runs before `AvatarControlSystem` in the tick order. The body
pipeline therefore updates `model_root` and propagates down the skeleton before `tick_one`
reads `neck_parent_world_rot`. So by the time we read it, it should be the result of this
tick's body pipeline run — not last tick's. This means the cancellation should be exact,
with no frame-lag error.

If the pitch issue persists it is more likely in one of:
- Floating-point loss in the `mat_to_quat` of `neck_parent_world_rot` when the matrix has a
  significant rotation (accumulated small errors in a long bone chain).
- The VRM model's rest-pose bone axes not aligning with the assumption that "neck +Z = world
  +Z when model_root is identity". If `J_Bip_C_Neck` has an authored local offset rotation in
  the rest pose, `neck_parent_world_rot` does not equal `rot_y(body_yaw)` and the cancellation
  leaves a residue proportional to that offset.
- The `rot_y(π)` handedness correction conflicting with a 180° Y baked into `model_root` via
  `initial_body_yaw = π`. This is discussed in section 10 below.

---

## 10. The two 180° Y rotations problem

The current setup has two separate places that apply a 180° Y rotation:

1. **`initial_body_yaw = π`** — seeded into `QuatYawFollowComponent`, so `model_root` starts
   at (and tends toward) `rot_y(π)`. This makes the VRM model face the same direction as the
   player (OpenXR -Z forward → body rotated 180° → VRM +Z forward).

2. **`handedness_correction = rot_y(π)` in `tick_one`** — post-multiplied onto
   `driven_world_rot` before cancelling the neck parent, to bridge OpenXR -Z / VRM +Z.

These two corrections are conceptually doing the same job from different angles. Let's trace
through to make sure they don't double-apply.

At rest, looking straight ahead (body yaw settled at π, no pitch/roll):

```
driven_world_rot   = identity  (HMD at rest, looking down -Z in OpenXR)
neck_parent_world_rot ≈ rot_y(π) × (rest_pose_bone_rots)
```

If rest-pose bone rotations ≈ identity (spine/neck bones aligned with Y axis):

```
splice_local_rot = inv(rot_y(π)) × (identity × rot_y(π))
                = rot_y(-π) × rot_y(π)
                = identity                  ← neck at rest pose. ✓
```

Now pitch down by θ (OpenXR `rot_x(θ)`, positive = look down):

```
driven_world_rot = rot_x(θ)
head_world_rot   = rot_x(θ) × rot_y(π)

splice_local_rot = rot_y(-π) × rot_x(θ) × rot_y(π)
                = rot_x(-θ)             (by the conjugation identity for orthogonal matrices)
```

`rot_x(-θ)` in VRM space (+Z forward, +Y up, +X right) = pitch the nose down. **This is
correct.** The two 180° rotations do not double-apply; they cancel cleanly in the
conjugation.

However, this derivation assumes:
- `neck_parent_world_rot` is exactly `rot_y(π)` (body yaw = π exactly, no off-axis tilt).
- Rest-pose bone rotations above the splice node are identity (or cancel cleanly).

If either condition is violated, the pitch transmission is not exactly 1:1.

---

## 11. Head as ground truth — the inverted data flow

Current data flow (body drives skeleton, skeleton drives head bone):

```
InputXR → driven_t → body_pipeline → model_root → skeleton propagation
       \                                                           ↓
        \                                              neck_parent_world_rot
         \                                                         ↓
          ——————————— AVC tick_one: cancel parent, apply head rot ——→ splice_head world_rot
```

Desired conceptual data flow (head drives skeleton, body follows head):

```
InputXR → driven_t.world_pos/rot  (head ground truth)
              │
              ├──→ head bone world_rot = driven_world_rot × rot_y(π)  (direct)
              │
              └──→ body_pipeline input:
                     XZ translation → TranslationFollow → model_root XZ  (lagged)
                     yaw            → YawFollow         → model_root yaw (existing, lagged)
                     Y              → floor level        (from calibration)
```

The head bone result is the same either way — the local decomposition cancels the parent.
The difference is conceptual: treating the head as the origin of truth makes the body
calculation explicit and separable.

The real gain from this inversion is **TranslationFollow**: if the body's XZ position is
allowed to lag the head's XZ position, the body can "catch up" to where the player's head
goes, rather than always being directly under it.

---

## 12. TranslationFollow — body XZ lag

Currently `TransformMapTranslation {}` in the body pipeline passes translation through
unchanged — body XZ = HMD stage XZ exactly.

A `TranslationFollow` (or `Vec3Follow`) pipeline op would behave like `YawFollow` for
position: the output XZ position chases the input XZ position at some rate, snapping when
the distance exceeds a threshold.

```
// Hypothetical MMS:
TransformPipeline {
    TransformForkTRS {
        TransformMapTranslation {
            Vec3TranslationFollow {      // new op
                with_threshold_xz(0.10) // 10 cm dead-zone
                with_rate_xz(1.5)       // m/s catch-up speed
            }
        }
        TransformMapRotation {
            QuatYawFollow { threshold: FRAC_PI_4, rate: 3.0, initial_yaw: π }
        }
        TransformMergeTRS {}
    }
    TransformPipelineOutput { T { GLTF { ... } } }
}
```

With this in place:
- The player can lean forward or turn their upper body without the avatar's feet sliding.
- If the player walks (stage tracking), the avatar body follows with a slight lag, then snaps.
- Small sways / head tilts don't translate 1:1 to body position movement.

### Y axis

Y should not follow the same threshold logic — it should always match the calibrated height
(`model_root.y = -bone_local_y`). So `Vec3TranslationFollow` only applies to XZ; Y is either
passed through or explicitly set from calibration.

---

## 13. The IK problem and why it surfaces with TranslationFollow

If body XZ lags behind head XZ, there is now a gap between where the neck-top should be
and where the model_root's spine places it. Without IK:

```
body position: (0, 0, 0)     (lagging)
head position: (0.15, 1.65, 0)  (player leaned forward)
```

The neck bone's world position is determined by model_root + skeleton propagation. If
model_root is at (0, 0, 0) but the head bone's world rotation is computed from the HMD
pose at (0.15, 1.65, 0), the neck appears to teleport or stretch.

The translation of `splice_head` is currently always `[0, 0, 0]` local — it only rotates.
The neck's world position comes entirely from the skeleton rest-pose chain under model_root.

For small thresholds (a few cm) this is visually tolerable — the neck appears to compress
slightly. For larger offsets you'd need:

**Option A: simple neck-extension model** — when body lags head, scale the neck bone in Y
proportionally to cover the gap. Heuristic, fast, good enough for small offsets.

**Option B: 2-bone IK on spine** — solve IK from chest to head target position. This is the
physically correct approach but requires an IK solver for the spine chain.

**Option C: accept the limitation** — keep body following head exactly in XZ (no
TranslationFollow), only do yaw-follow. The "body slides under head" issue is then
minimal and the neck never needs to stretch.

For the current implementation, option C is what we have. Adding TranslationFollow meaningfully
requires either A or B to avoid neck artifacts.

---

## 14. The neck as derived from head, not head as derived from neck

The user's observation: "the neck should only move indirectly as a result of the head
rotating / translating."

In the current design, the head bone's world rotation is derived from InputXR, which IS the
ground truth. The neck bone (J_Bip_C_Neck) is displaced under splice_head, so it inherits
splice_head's world rotation. The neck effectively follows the head — not the other way.

What doesn't follow is **neck world position**: it is always at the rest-pose skeleton offset
under model_root. The head position itself (InputXR world XYZ) is what determines where the
player physically is; the skeleton just stretches to fill the space.

For this to work correctly and look natural:
1. `model_root` Y calibration ensures the head bone's rest-pose world Y = HMD world Y (done).
2. `model_root` XZ = HMD XZ (done currently, no lag).
3. Head bone world rot = HMD rot × handedness correction (done via splice_head).

The system is structurally correct. The gaps are:
- Possible pitch attenuation from rest-pose bone offsets (empirical, model-dependent).
- No TranslationFollow (body always under head with no lag — acceptable for now).
- No IK (neck doesn't bend when body lags — moot since there's no lag yet).

---

## 15. Recommended next investigative steps (no code changes)

1. **Measure actual pitch transmission**: add a debug print of `splice_head.matrix_world`
   pitch angle vs `driven_t.matrix_world` pitch angle on each tick. If they differ by more
   than float noise, the decomposition has an error worth diagnosing.

2. **Check rest-pose bone chain**: inspect `neck_parent_world_rot` at rest (HMD forward,
   body settled). If it is not `rot_y(π)` within tolerance, the bones above the splice have
   authored local rotations that affect the cancellation.

3. **Consider TranslationFollow as a future pipeline op**: design as `Vec3Follow` with
   separate XZ threshold/rate and Y pass-through. Only add once the rotation issues are
   resolved and only if avatar sway is noticeable.
