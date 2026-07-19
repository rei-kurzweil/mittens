# AvatarControl — transform timing analysis

## Observed symptoms

1. **Torso twist on idle**: while actively rotating, everything looks correct. When releasing
   q/e or stopping mouse-drag, the torso twists slightly, proportionate to how far the head
   yaw is offset from body yaw.

2. **Head/hair bone over-movement on pitch**: when pitching down to look at the ground,
   the editor bone markers for the head and hair appear to move further than the visual head
   mesh.

---

## Tick-order overview (desktop)

```
1193  InputSystem.process_input            → queues UpdateTransform(driven_t) if keys/drag held
1201  queue.flush                          → executes UpdateTransform(driven_t):
        transform_changed(driven_t)
          → body pipeline evaluates → YawFollow runs → model_root.matrix_world updated
          → GLTF armature propagated → neck_parent.matrix_world updated
1230  skinned_mesh.tick                    ← READS bone world matrices HERE
1261  queue.flush  (after OpenXR, gizmos)
1288  avatar_control.tick                  → reads neck_parent.matrix_world; emits UpdateTransform(head_mount)
1289  queue.flush                          → executes UpdateTransform(head_mount):
        transform_changed(head_mount)
          → J_Bip_C_Neck world matrix updated; skinned_mesh notified (next frame)
```

---

## What transforms are set on each tick type

### Active-rotation tick (q/e held or mouse dragging)

| Component | Who sets it | When |
|---|---|---|
| `driven_t` TRS | InputSystem | queue.flush at 1201 |
| `model_root.matrix_world` | body pipeline | queue.flush at 1201, BEFORE skinned_mesh.tick |
| `neck_parent.matrix_world` (& rest of armature) | body pipeline propagation | same flush |
| `head_mount` TRS | AvatarControlSystem.tick_one | queue.flush at 1289, AFTER skinned_mesh.tick |
| head/hair bone world matrices | transform_changed(head_mount) | same flush |

The SkinnedMesh at 1230 sees body transforms from the **current frame** (pipeline ran this frame)
but head_mount from the **previous frame** (ACS hasn't run yet). This is a 1-frame offset between
body and head.

### Idle tick (keys released, mouse not dragging)

| Component | Who sets it | When |
|---|---|---|
| `driven_t` TRS | **nobody** | InputSystem returns early at line 306 |
| `model_root.matrix_world` | **nobody** | body pipeline does not run (no driven_t UpdateTransform) |
| `neck_parent.matrix_world` | **nobody** | stays at value from last active frame |
| `head_mount` TRS | AvatarControlSystem.tick_one | queue.flush at 1289 (same value as previous frame, since driven_t didn't change) |

---

## Key difference from old code

### Old code — `AvatarControlSystem.tick_one` per tick, every tick

```
active tick:
  ACS emits UpdateTransform(model_root)   ← body rotation, local-space, anti-pitch compensated
  ACS emits UpdateTransform(head_mount)  ← head rotation
  → both flush at 1289, both are 1 frame behind skinned_mesh.tick at 1230

idle tick:
  ACS still emits UpdateTransform(model_root) — SAME VALUE (nothing changed)
  ACS still emits UpdateTransform(head_mount) — SAME VALUE
  → body_yaw follow logic ran; if body was mid-chase it continues advancing
```

The old code kept `body_yaw` advancing **every tick** via tick_one, regardless of whether
driven_t had changed. Even after the user stopped, if body_yaw was still outside the threshold
from head_yaw, it kept rotating toward head_yaw on every subsequent tick.

### New code — body driven by pipeline; head by tick_one per tick

```
active tick:
  body pipeline runs at flush 1201 → model_root.matrix_world = quat_rotation_y(body_yaw)
    ← 0 frames behind skinned_mesh.tick at 1230
  ACS emits UpdateTransform(head_mount) at flush 1289
    ← 1 frame behind skinned_mesh.tick

idle tick:
  body pipeline does NOT run (no driven_t UpdateTransform)
  body_yaw is FROZEN at last-active-frame value
  ACS emits UpdateTransform(head_mount) — same value (nothing changed)
```

The new code **freezes body_yaw immediately when input stops**.

---

## Hypotheses for the torso twist

### H1: Body_yaw frozen mid-chase (most likely)

If the user rotates quickly to a yaw that exceeds the threshold and then stops, the body
was lagging behind (body_yaw = head_yaw − threshold − lag). In old code, tick_one kept
advancing body_yaw on every idle tick until it reached `head_yaw − threshold`. In new code,
body_yaw is frozen immediately when input stops.

Result: on the first idle frame, the avatar body is frozen mid-chase, showing a yaw
separation proportional to how fast the user was rotating (more lag = bigger twist).
The twist doesn't animate away because the pipeline never re-runs.

The proportionality the user observes — twist ∝ (head_yaw − body_yaw) — is consistent
with this: a larger in-progress chase means a bigger frozen offset.

**Why it looks correct while rotating**: the 1-frame lag between body (0-frame behind)
and head (1-frame behind) is small and continuously updating, so small mismatches aren't
perceptible. When frozen, the misalignment becomes visually stable and obvious.

### H2: 1-frame inconsistency between body and head at stop boundary

On the last active frame (frame N): skinned_mesh sees body_yaw_N and head_mount_N-1.
On frame N+1 (first idle): skinned_mesh sees body_yaw_N (body pipeline didn't re-run) and
head_mount_N. Body and head are now from different "generations" — body from frame N's
pipeline, head from frame N's ACS run.

In old code both body and head were from frame N-1's ACS on frame N's skinned_mesh tick
(both consistently 1-frame behind). In new code they diverge at the stop boundary.

Whether this produces a visible difference depends on how far body_yaw_N − body_yaw_N-1
and head_mount_N − head_mount_N-1 differ.

### H3: Initial world matrices after Attach are wrong (secondary)

When `Attach(pipeline_output_id, model_root_id)` fires (frame T+1 after init), the
`UpdateTransformWorld` call walks UP the chain to find model_root's parent world. Since
`pipeline_output` and `AVC` are not TCs, it reaches `driven_t` and uses `driven_t.matrix_world`
directly — bypassing the body pipeline. This gives model_root an incorrect initial world matrix
(includes driven_t's pitch).

This is corrected on the next driven_t UpdateTransform (when the user first moves). But if
the user never moves, or if there's a frame where skinned_mesh reads the pre-correction state,
the bind-pose skin matrices could be off.

---

## Hypothesis for the head/hair bone over-movement

The bone markers are positioned at bone.matrix_world (world space position of the bone's
origin). The visual head mesh is deformed by skin matrices `bone.matrix_world * inverse_bind`.

When head_mount applies a pitch rotation R, J_Bip_C_Neck's world position shifts: the bone
origin moves forward/down because J_Bip_C_Neck's LOCAL translation (the ~15cm segment
from upper chest to neck) gets rotated by R. The bone marker follows this shifted origin.

The skin matrix deforms vertices around J_Bip_C_UpperChest's position (the pivot), so head
MESH vertices move less than the bone ORIGIN — the pivot is below the visible head. The bone
marker (at the neck origin) therefore appears further from its rest position than the visual
head center.

This is expected skinning behavior and not a code bug. The same offset would exist in the old code.

If the bone markers appear to move MORE than before the pipeline change, it could indicate
that head_mount is applying a larger rotation than it should. Possible causes:
- neck_parent.matrix_world already includes some contribution from the old tick_one body
  rotation (double-counting), inflating the inverse
- The handedness correction (π for VR) is being applied on desktop (forward_plus_z=true
  should suppress it; verify it's 0 not π)

---

## Recommended investigation

1. **Verify body_yaw freezing** (H1): add a debug print showing `yaw_follow_state` body_yaw
   value on each tick. Confirm it freezes on the first idle tick. In the old code, body_yaw
   continued to advance for a few ticks after input stopped.

2. **Fix body_yaw follow on idle ticks**: re-introduce body_yaw follow logic in
   `AvatarControlSystem.tick_one` that runs every tick. On each tick:
   - Read current body_yaw from some source (need an API or store it on AVC)
   - If delta > threshold, advance body_yaw and emit UpdateTransform(model_root) with
     `quat_rotation_y(body_yaw)` as WORLD rotation (need to account for model_root being
     under pipeline_output, not directly under driven_t)

3. **Check driven_t emission on idle ticks for VR**: OpenXRSystem updates driven_t every
   tick (line 1259). So for VR, the body pipeline always runs (no freeze issue). The torso
   twist may only manifest on desktop.

4. **Verify handedness correction on desktop**: `forward_plus_z` should be `true` for desktop,
   giving `handedness_correction = 0.0`. If any scene file has `forward_plus_z` wrong, the
   head rotation would be off by π.

5. **Head bone over-movement**: check if the pivot for head deformation is neck_parent vs.
   neck bone. If the bind matrices were computed after model_root was re-parented (or after
   head_mount was inserted), they may be misaligned from the authored rest pose.
