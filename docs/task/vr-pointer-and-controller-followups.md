# Task: VR pointer and controller follow-ups

Date: 2026-06-16

Four follow-up issues blocking correct VR interaction, listed in dependency order.
Items 2 and 3 must be resolved before item 4 is useful to test.

---

## 1. Gizmo / selection drag-on-same-click regression

### Symptom

When clicking to select an object (e.g. the bisket avatar in `bisket-vr-demo`) the selection fires
and a transform gizmo appears, but the selected object is also immediately translated by 1–2 units
in what appears to be a direction related to the previously selected object or pointer position.

This happens in desktop select + cursor mode. The translation appears to come from a `DragMove`
or gizmo drag responding to a press that should have only triggered selection, not a gizmo move.

### Root cause hypothesis

The `GestureSystem` refactor moved from a single `GestureState` to a per-pointer
`HashMap<ComponentId, GestureState>`. The most likely cause is that a `DragStart` emitted on
the frame that caused a selection change is being consumed by the freshly installed gizmo handlers
on the same frame.

Previously, `DragStart` was dispatched in the same signal-processing pass as the selection and
gizmo registration. If `TransformGizmoSystem` installs its scoped handler at init time (which it
does via `RegisterTransformGizmo`), the handler may be active before the drag ends — causing the
gizmo to interpret the current drag as an intentional gizmo move.

### Fix direction

The gizmo should not respond to a drag that started before the gizmo was attached to the scene.
Options:

- Gate gizmo drag response: `TransformGizmoSystem::on_drag_start` should ignore any drag that
  began before the gizmo's own `RegisterTransformGizmo` intent was processed. A "born-at" beat or
  component-id guard is sufficient.
- Alternatively, `DragStart` should not be re-dispatched to newly installed handlers within the
  same signal-drain pass in which they were registered. This is a broader signal lifecycle change.

The first option (gizmo-local guard) is narrower and safer.

### Files to look at

- `src/engine/ecs/system/gizmo_system.rs` — `on_drag_start`, `register_transform_gizmo`
- `src/engine/ecs/system/editor_system.rs` — where selection triggers gizmo attachment
- `src/engine/ecs/rx/signal.rs` — drain-point order for events vs intents

---

## 2. XR camera / controller forward direction is flipped (Z axis)

### Symptom

In `bisket-vr-demo`, the XR camera and controller forward direction is inverted on Z compared to
the model's facing direction. The head and locomotion calculations (yaw tracking, body follow)
appear to work, but the world is seen from behind — i.e. turning your physical head left makes the
in-engine head turn right, or the scene appears mirrored along Z.

The hand poses appear on the correct sides left/right but are physically behind the avatar.

### Background

`AvatarControlSystem` has divergent code paths for:

- `InputXRComponent` + `CameraXRComponent` (VR)
- `InputComponent` + `Camera3DComponent` (desktop)

There is existing special-casing in AVC for determining forward direction and how yaw is applied
differently between the two paths. A π (180°) Y-axis correction is baked in somewhere under the
assumption that OpenXR and the VRoid armature face opposite directions.

The hypothesis is that this correction was added at a time when something else in the pipeline was
also flipped, and the two negatives cancelled. Now one side has been fixed and the π flip is
wrong.

### Fix direction

1. Read `AvatarControlSystem`'s head yaw derivation for both paths side by side.
   Specifically find where forward direction or yaw is computed differently for XR vs desktop.
2. Verify in `bisket-vr-demo` whether looking forward in the physical world produces forward in
   the engine scene for desktop mode.
3. Check whether the OpenXR reference space `LOCAL` origin matches the engine's +Z forward
   convention or requires a basis correction.
4. Remove or negate the π Y correction and re-test both `vtuber-desktop` and `bisket-vr-demo`
   to confirm neither regresses.

Do not special-case individual hardware. The fix should be correct for the OpenXR reference space
convention and the engine's coordinate system, and both examples must pass.

### Files to look at

- `src/engine/ecs/system/avatar_control_system.rs` — yaw derivation, π correction, per-path
  forward calculation
- `src/engine/ecs/system/openxr_system.rs` — `mat4_from_pose`, reference space type, how
  controller poses are converted to engine transforms

---

## 3. Wrist / controller pose rotation offset (blocked by item 2)

### Symptom

With a VRM/VRoid armature, the wrist bones appear rotated incorrectly when driven by controller
poses. The controllers appear to need an inward ~90° rotation along local Y on both hands.

### Reframe

VRoid rigs drive correctly from standard OpenXR grip poses in VRChat without any armature-specific
correction. This means the offset is not a VRoid quirk — it is an error in the engine's conversion
of the OpenXR grip pose into a transform, or in how that transform is applied to the hand bone.

The correct mental model is:

- OpenXR defines a standard grip pose orientation per interaction profile
- a correct engine implementation should apply that pose to the hand bone and get a natural wrist
  angle without any per-armature fudge factor
- if the angle is wrong, either the pose conversion or the bone application is wrong

### Blocked by

Item 2 must be fixed first. Until controller forward direction is correct and left/right are
confirmed on the right sides, any rotation offset applied now may compound rather than fix the
issue.

### Fix direction (after item 2 is resolved)

1. Compare the engine's grip-pose-to-transform path against OpenXR spec for the grip pose
   orientation. The grip pose +Z should point from the wrist toward the fingers along the palm,
   and +Y should point from the palm toward the back of the hand. Verify the engine honors this.
2. Check how `AvatarControlSystem` applies the controller transform to the hand bone. Is it
   composing the rotation correctly relative to the bone's bind pose?
3. If a residual offset remains after the math is correct, add an optional
   `pose_rotation_offset: Option<[f32; 4]>` field to `ControllerXRComponent` (quaternion, applied
   after the OpenXR pose before writing the transform) as a hardware-level correction escape hatch
   for runtimes or controllers that deviate from the spec — not for armature compatibility.

### Files to look at

- `src/engine/ecs/system/openxr_system.rs` — grip pose to transform conversion
- `src/engine/ecs/system/avatar_control_system.rs` — how controller transform drives hand bone
- `src/engine/ecs/component/controller_xr.rs` — potential `pose_rotation_offset` field

---

## 4. Controller laser pointer visual (`CTLXR.with_laser()`)

### Symptom / motivation

There is currently no visual feedback for what the controller is pointing at. This makes it
impossible to aim the pointer in VR without guesswork.

### Design

Add `with_laser() {}` as an opt-in child block on `CTLXR`:

```text
CTLXR.new(true, Left, Grip) {
    T {
        Pointer {}
    }
    with_laser() {}
}
```

When `with_laser()` is authored, `ControllerXRComponent` spawns a thin elongated mesh as a
runtime child of the controller's `TransformComponent`. Suggested geometry:

- a quad or cone mesh scaled to approximately `(0.03, 0.03, 10.0)` world units
- offset along -Z (engine forward) so its origin sits at the controller tip
- semi-transparent cyan material (`rgba(0.0, 1.0, 1.0, 0.35)`)
- unlit / emissive so it is visible in dark scenes
- `Transparent` render phase

The laser is purely a runtime visual — not serialized, not part of the authored subtree.
It is created by `ControllerXRSystem` or inside `ControllerXRComponent::init` analogously to
how `PointerSystem` spawns a child `RayCastComponent`.

### Blocked by

Item 3 (wrist rotation offset) is not a hard blocker for the laser visual, but the laser will
point in the wrong direction until item 2 is resolved. The laser can be implemented and tested
on `vtuber-desktop` desktop pointer for visual parity, and enabled on controllers once item 2 is
done.

### Files to look at

- `src/engine/ecs/component/controller_xr.rs` — add `laser: bool` field, `with_laser()` builder
- `src/engine/ecs/system/openxr_system.rs` or a new `controller_xr_system.rs` — spawn laser mesh
- `examples/bisket-vr-demo.mms` — add `with_laser() {}` to both CTLXR entries to test
