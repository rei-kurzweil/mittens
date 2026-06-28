# Task: AvatarControl desktop head-facing and pitch-inversion follow-up

Date: 2026-06-27

Status: active narrowed regression follow-up.

This task narrows the current desktop AVC regression after the recent
desktop/VR facing unification attempt.

It is intentionally narrower than:

- [docs/task/avatar-control-desktop-vr-facing-and-eye-offset-unification.md](./avatar-control-desktop-vr-facing-and-eye-offset-unification.md)

because the current problem is no longer "desktop and VR are both generally
incoherent". VR is currently behaving normally. The remaining regression is
specifically in the desktop head/camera path.

Related files:

- [examples/vtuber-mirror-example.mms](../../examples/vtuber-mirror-example.mms)
- [src/engine/ecs/system/avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)
- [src/engine/ecs/system/input_system.rs](../../src/engine/ecs/system/input_system.rs)
- [src/engine/ecs/system/ik_system.rs](../../src/engine/ecs/system/ik_system.rs)

---

## 1. Current observed state

Primary proof scene:

- [examples/vtuber-mirror-example.mms](../../examples/vtuber-mirror-example.mms)

Current authored state in that scene:

- desktop camera wrapper uses `T.position(0.0, 0.08, -0.12)`
- XR camera wrapper uses `T.position(0.0, 0.08, 0.12)`
- there are no explicit `forward_plus_z()` / `initial_yaw(...)` overrides in
  the desktop AVC block there right now

Observed runtime behavior:

- VR path behaves normally
- desktop path still requires the camera-wrapper Z offset to be inverted
  relative to XR in order not to look at the back of the head
- desktop movement direction now matches the visible body/camera path again
- desktop mouse yaw/pitch now feel correct on screen
- desktop body-facing is now mostly correct relative to movement/camera
- desktop visible head mesh is still backward relative to the camera and body
- the remaining backward-facing symptom is specifically localized to the
  desktop visible head path, not the whole desktop rig

That means recent changes successfully narrowed the bug from "desktop rig is
globally incoherent" to "desktop visible head path is still 180° off".

---

## 2. Why this matters

For a normal humanoid first-person setup, desktop and XR should agree on the
meaning of:

- which side of the head pivot is "forward" for eye offset
- which way the visible head/model faces relative to the camera
- which sign of vertical mouse drag produces look-up vs look-down

Today, they do not.

The desktop proof scene still leaks internal convention mismatches into
authored content:

- camera wrapper sign experiments are still required
- the mirror still reveals a backward-facing desktop head path
- desktop body/camera can be made coherent while the visible head remains wrong
- that means the remaining bug is deeper than raw input sign or body yaw-follow

---

## 3. Current code-path split to inspect

The current implementation appears to mix at least three distinct conventions:

### A. Desktop input movement / yaw basis

`InputSystem` desktop FPS motion uses `InputTransformMode.forward_z()` and, for
`ForwardAxis::Z`, maps:

- `W` to local `-Z`
- yaw-only movement basis from desktop drag rotation

Relevant code:

- [src/engine/ecs/system/input_system.rs](../../src/engine/ecs/system/input_system.rs)

This path now appears correct enough to treat as non-primary for this task.

### B. AVC body yaw-follow basis

`AvatarControlSystem` currently infers:

- `resolved_forward_plus_z` from driver kind
  - desktop => `true`
  - XR => `false`

and feeds that into the body yaw-follow pipeline.

Relevant code:

- [src/engine/ecs/system/avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)

This appears to have improved desktop body-facing substantially.

### C. AVC head-target / visible-head basis

`AvatarControlSystem` also computes:

- `head_target_offset`
- `head_target_id` rotation
- `head_ik_offset_yaw`

and reparents the visible head path under `driven_t -> head_target`.

That means the head/camera path has its own convention layer, separate from
desktop movement and separate from body yaw-follow.

Relevant code:

- [src/engine/ecs/system/avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)
- [src/engine/ecs/system/ik_system.rs](../../src/engine/ecs/system/ik_system.rs)

This is now the strongest suspect for the remaining desktop regression.

### D. Desktop visible-head local rest rotation

Even after separating desktop body-facing from desktop head-target/camera-facing,
the desktop visible head mesh remains backward while:

- body-facing is correct
- camera-facing is correct
- desktop movement basis is correct

That strongly suggests a final desktop-only mismatch in one of:

- `head_rest_rot`
- the `head_bone_id` local rotation restore after reparenting
- the relationship between `head_target_id` world rotation and the preserved
  visible head local rest rotation

Relevant code:

- [src/engine/ecs/system/avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)

---

## 4. Working hypothesis

The current bug now strongly suggests that desktop still has a split between:

1. body-facing convention
2. visible head/model-facing convention
3. camera eye-offset convention
4. preserved head rest rotation convention

More concretely:

- the desktop body path is now close to the desired convention
- the desktop camera path is now close to the desired convention
- but the desktop visible head path is still effectively 180° off relative to
  both the camera and the body
- because of that, the authored desktop eye offset still has to use the
  opposite Z sign from XR
- attempts to fix the remaining issue by changing `initial_yaw(...)` did not
  affect the symptom
- attempts to fix the remaining issue by changing shared body/head forward
  defaults over-corrected either the camera or the body, but still did not
  resolve the visible head reliably
- that strongly suggests the last mismatch is not in the raw input path and not
  in the body yaw-follow path, but in the visible head transform restore/mount

In other words:

- desktop yaw, pitch, movement, and body-facing are no longer enough to prove
  the full desktop first-person stack is coherent
- the remaining bug lives specifically in the visible head path, not just the
  general head/camera-facing path

---

## 5. What this task should measure explicitly

In `vtuber-mirror-example`, compare desktop vs XR for:

- `driven_t` world forward
- desktop `Camera3D` world forward
- XR `CameraXR` effective forward
- desktop body pipeline output world forward
- `head_target` world forward
- visible head bone world forward
- `model_root` world forward
- `head_bone.local.rotation`
- `head_rest_rot`
- sign relationship between authored camera-wrapper local Z and resulting eye
  placement

The key question is not just "which value is wrong?" but:

- which transform stage first diverges from the expected first-person
  convention on desktop while XR remains correct
- whether the divergence appears before or after the visible head local rest
  rotation is restored under `head_target`

---

## 6. Concrete acceptance criteria

This follow-up is complete when the following are true in
`vtuber-mirror-example`:

1. desktop and XR can both use `T.position(0.0, 0.08, 0.12)` for the camera
   wrapper in the common humanoid case
2. desktop no longer looks at the back of the head with that shared offset
3. desktop mirror output no longer reveals the avatar/head facing backward
4. desktop pitch drag direction feels normal up/down without breaking yaw
5. VR behavior remains unchanged

Interim current state toward those criteria:

- criterion 4 is now effectively satisfied
- criterion 5 remains satisfied
- criterion 3 is not yet satisfied because the desktop head mesh remains
  backward
- criteria 1 and 2 remain unsatisfied because desktop still depends on the
  opposite camera-wrapper Z sign from XR

---

## 7. Recommended next investigation order

1. Treat `vtuber-mirror-example` as the only proof scene until the desktop
   head/camera mismatch is explained.
2. Instrument the desktop path first at the `driven_t -> body pipeline ->
   head_target -> head_bone -> camera wrapper` chain.
3. Compare that chain directly against the XR path in the same scene.
4. Specifically log the world forward of:
   `model_root`, `head_target`, `head_bone`, and desktop `Camera3D`.
5. Specifically log `head_rest_rot` and the final `head_bone.local.rotation`
   after AVC init on desktop vs XR.
6. Do not assume the remaining visible-head fix is the same fix as either the
   body-facing fix or the camera-facing fix.
7. Only after the transform-stage divergence is identified should the engine
   contract or defaults be changed again.

---

## 8. Progress log from this follow-up

Observed successful narrowing:

- VR behavior remained correct throughout recent investigation passes.
- Desktop pitch now feels correct.
- Desktop movement and body-facing can now be made coherent relative to the
  camera.
- The remaining desktop regression is now isolated to the visible head mesh.

Observed failed fix directions:

- changing desktop `initial_yaw(...)` alone did not fix the visible head
- forcing desktop to share XR defaults for both body and head fixed one layer
  but flipped the desktop camera backward
- separating desktop body-facing defaults from desktop head-target/camera
  defaults improved body + camera alignment but still left the visible head
  backward
- adding a desktop-only visible-head local `π` correction did not change the
  observed symptom enough to resolve the bug

Current interpretation:

- the remaining mismatch is likely in the interaction between:
  `head_target_id`, `head_bone_id`, and the restored `head_rest_rot`
- that is a narrower and more testable problem than the original
  "desktop rig vs XR rig" framing
