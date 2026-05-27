# Editor gizmo coordinate spaces (Local vs World)

This doc defines a small but important editor feature: independently configurable **coordinate spaces** for translation and rotation gizmo operations.

The immediate motivation is a correctness/UX bug:

- Gizmo visuals are parented under the target transform and currently inherit translation + rotation (scale is filtered).
- After you rotate an object using the gizmo, subsequent rotations no longer correspond to *world* axes. It starts to feel like you’re rotating relative to the object’s new orientation (local / incremental drift).

We want to support both behaviors explicitly.

## Goals

- Add two independent editor settings:
  - `transform_gizmo_translation_space: Local | World`
  - `transform_gizmo_rotation_space: Local | World`
- Make gizmo **visual axes** and **drag application math** agree with the chosen mode.
- Allow mixed modes (e.g. translate in World, rotate in Local).

## Non-goals

- This doc does not cover constant screen-space *thickness* (see the separate screen-space lines doc).
- This doc does not define snapping, angle increments, or numeric input.

## Definitions

- **World space**: axes are fixed to global XYZ.
  - Translate along world X means position changes only along world X regardless of object rotation.
  - Rotate around world X means object’s orientation changes by a rotation around the global X axis.

- **Local space**: axes are fixed to the object’s local frame.
  - Translate along local X means move “forward” in the direction the object is facing.
  - Rotate around local X means change orientation around the object’s own X axis.

- **Target transform**: the `TransformComponent` that the gizmo edits (`TransformGizmoComponent.target_transform`).

- **Parent transform**: the nearest ancestor `TransformComponent` above the target in the hierarchy.

## Current behavior (why it feels wrong)

As of now, in [src/engine/ecs/system/gizmo_system.rs](src/engine/ecs/system/gizmo_system.rs):

- Gizmo visuals inherit translation and rotation from the target through an explicit transform pipeline, while dropping inherited scale for the visual subtree.
- Translation drag logic projects the world drag delta onto a hard-coded axis `axis.unit_vec3()`.
- Rotation drag logic also starts from a hard-coded world axis `axis.unit_vec3()`, but it then converts it to a “local” axis via `world_dir_to_target_local`.
- That conversion currently depends on `CAT_GIZMO_USE_PARENT_INVERSE` and is **off by default**.

Net effect: after the object rotates, the ring visuals rotate with it, and the math tends to behave like “rotate around whichever axis the object is now oriented to”, even if the user expects “rotate around world axes”.

## Proposed API / state

### Enums

Add a single enum (name bikeshed):

```rust
pub enum TransformGizmoCoordSpace {
    Local,
    World,
}
```

### Editor settings

These should be editor-level settings so one `EditorComponent` subtree can have its own gizmo behavior:

- `transform_gizmo_translation_space: TransformGizmoCoordSpace`
- `transform_gizmo_rotation_space: TransformGizmoCoordSpace`

Where this state lives:

- Preferred: `EditorComponent` (because it is already “per editor subtree” state)
- Alternative: store the resolved values on `TransformGizmoComponent` at spawn time (derived from the owning editor), if we want the gizmo to be self-contained.

Serialization note: per repo policy, when these fields are added, do not add legacy aliases or multi-shape decoding for older saved JSON.

## Visual behavior (what the gizmo looks like)

The key UX requirement is: **the displayed axis should match the axis you’re applying**.

That implies we should allow translation handles and rotation handles to be oriented differently.

### Required ability

- Translation handles can be world-aligned even when rotation handles are local-aligned.

### How to express this with current ECS primitives

Because gizmo visuals are parented under the target transform, they naturally inherit the target’s world matrix.

We can model selective translation/rotation/scale inheritance with explicit transform-pipeline primitives. That lets us create two visual “spaces” under the gizmo:

- **World-aligned visual group**: inherit translation only
  - `inherit_translation=true`
  - `inherit_rotation=false`
  - `inherit_scale=false`

- **Local-aligned visual group**: inherit translation + rotation
  - `inherit_translation=true`
  - `inherit_rotation=true`
  - drop inherited scale in the top-level gizmo pipeline

Then:

- translation arrows live under whichever group `transform_gizmo_translation_space` selects
- rotation rings live under whichever group `transform_gizmo_rotation_space` selects

This directly solves the “rings no longer correspond to world axes” visual mismatch.

## Operation behavior (what gets applied to the target)

There are two layers:

1) pick an **axis direction** to interpret the drag
2) convert the resulting world delta into the target’s **local** parameters (`translation`, `rotation`), because the component stores locals

### Translation

Let:

- `axis_local = axis.unit_vec3()` in the gizmo’s axis enum
- `R_world` = target’s world rotation (or equivalent basis)

Define `axis_world` by mode:

- `World`: `axis_world = axis_local` (i.e. `[1,0,0]`, `[0,1,0]`, `[0,0,1]`)
- `Local`: `axis_world = normalize(R_world * axis_local)`

Then, from the drag event:

- project the world delta onto that axis: `d = dot(delta_world, axis_world)`
- `delta_world_axis = axis_world * d`

Finally, convert to a local translation delta:

- Let `M_parent` be the parent’s world 4x4 matrix (or identity if no parent)
- `delta_local = inverse(M_parent) * vec4(delta_world_axis, 0)` and take xyz

Then apply:

- `t_local_next = t_local_cur + delta_local`

Notes:

- Using the full inverse of the parent matrix makes world translation behave sensibly under rotated and scaled parents.
- The current `world_delta_to_target_local` already does essentially this when `CAT_GIZMO_USE_PARENT_INVERSE` is enabled. For editor coord spaces, this conversion should be part of the defined behavior, not an env-var opt-in.

### Rotation

We want two explicit semantics:

- **Local rotation**: apply delta around the target’s local axis.
- **World rotation**: apply delta around a fixed world axis regardless of the target’s current orientation.

Let:

- `q_local` be the target’s stored local rotation
- `q_parent_world` be the parent’s world rotation (identity if no parent)

Compute an axis by mode:

- `Local`: axis in target-local space is `axis_local = axis.unit_vec3()`
- `World`: axis in world space is `axis_world = axis.unit_vec3()`

Now define the delta quaternion:

- `Local` mode:
  - `q_delta_local = quat(axis_local, angle)`
  - `q_local_next = q_delta_local * q_local`

- `World` mode:
  - We want `q_world_next = q_delta_world * q_world`
  - With hierarchy: `q_world = q_parent_world * q_local`
  - So:

    `q_local_next = (q_parent_world^-1 * q_delta_world * q_parent_world) * q_local`

  - where `q_delta_world = quat(axis_world, angle)`

This “conjugate by parent world rotation” step is what keeps world axes stable.

Implementation note: the engine currently doesn’t expose a `world_rotation()` helper; we’d extract/compute `q_parent_world` from the parent `matrix_world` basis (normalize out scale) or extend `TransformSystem` to expose it.

### Mixed modes

Because translation and rotation are independent, the above logic must be chosen per operation:

- translate uses `transform_gizmo_translation_space`
- rotate uses `transform_gizmo_rotation_space`

## Interaction with existing gesture types

Rotation currently supports a screen-space 1D slider mode (integrating `dx + dy` into an angle). This doc does not change how the *angle* is produced; it only changes how that angle is interpreted (world vs local axis).

## Suggested implementation plan (phased)

1) Add the enum and editor fields
- wire them into editor state (wherever we store editor settings today)

2) Update gizmo visuals
- split gizmo visuals into two filtered groups (translation group, rotation group)
- parent handles under the group selected by the corresponding coord space

3) Update gizmo drag math
- translation: choose `axis_world` by mode, then convert `delta_world_axis` through parent inverse
- rotation: implement the parent-conjugation path for world rotation

4) Add a small example / debug affordance
- e.g. a rotating object under a rotated parent, demonstrating that `World` rotation stays world-aligned

## Acceptance criteria

- With `rotation_space = World`, rotating around X twice (with an intermediate object orientation change) still behaves like rotations around the same world X axis.
- With `rotation_space = Local`, rotating around X twice behaves like rotations around the object’s own X axis.
- With `translation_space = World`, dragging the X arrow moves the object along world X even if the object is rotated.
- With `translation_space = Local`, dragging the X arrow moves the object along its forward/right axis depending on its current rotation.
- Visual handles reflect the chosen spaces (world-aligned handles do not “spin with the object”).
