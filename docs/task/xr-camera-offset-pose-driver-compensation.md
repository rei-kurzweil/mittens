# Task: XR camera offset as a referenced inverse-anchor transform stream

Date: 2026-09-08

Status: phases 1 and 2 implemented and validated in XR; AVC humanoid integration deferred.

This task records the transform relationship discovered while using a
non-humanoid car model under `InputXR` in
[`examples/mittens-corp.mms`](../../examples/mittens-corp.mms).

The XR camera is the motivating case, but the proposed engine primitive is a
generic referenced transform-stream operator rather than an
`AvatarControl`-specific camera policy.

Related context:

- [`docs/analysis/cameraxr-avatar-alignment.md`](../analysis/cameraxr-avatar-alignment.md)
- [`docs/spec/transform-operators.md`](../spec/transform-operators.md)
- [`docs/spec/component-id-references.md`](../spec/component-id-references.md)
- [`docs/task/avatar-control-head-driven-redesign.md`](./avatar-control-head-driven-redesign.md)
- [`docs/task/avatar-control-simple-humanoid-body-follow.md`](./avatar-control-simple-humanoid-body-follow.md)
- [`src/engine/ecs/system/openxr_system.rs`](../../src/engine/ecs/system/openxr_system.rs)
- [`src/engine/ecs/system/transform_stream_system.rs`](../../src/engine/ecs/system/transform_stream_system.rs)
- [`src/engine/ecs/system/avatar_control_system.rs`](../../src/engine/ecs/system/avatar_control_system.rs)

---

## 1. Observed issue

The `mittens-corp` XR rig places a car and a wrapped `CameraXR` beneath the
same pose-driven subtree:

```text
InputXR
  -> tracked transform
       -> AvatarControl
            -> car model
            -> T.position(camera_offset)
                 -> CameraXR
```

Changing the camera wrapper to an obviously large offset, for example
`T.position(0, 17, 0) { CXR {} }`, does not change the rendered XR camera's
position relative to the car. Moving the wrapped camera outside
`AvatarControl`, while leaving it beneath the same `InputXR`, also does not
change the result.

The car's measured render bounds correctly contain its visible geometry. The
failure is not caused by bad bounds or an incorrect model origin.

---

## 2. Why the camera transform does not move the XR view

`OpenXRSystem::xr_rig_origin_world` finds the active camera's `InputXR`
ancestor and uses the world transform above the tracked transform as the XR
rig origin. It deliberately does not use the wrapped `CameraXR` node's world
transform as the physical eye pose.

The final rendered eye transform is:

```text
rendered_eye_world = xr_rig_origin_world * raw_openxr_eye_pose
```

This avoids applying the tracked HMD pose twice and keeps the physical headset
authoritative. Therefore, an authored transform around `CameraXR` must not
directly move the user's rendered eyes.

The transform is still meaningful authoring data. It says where the physical
camera is intended to sit relative to some other content transform. What is
missing is an explicit consumer of that relationship.

The existing partial consumer is `AvatarControlSystem`. It discovers a wrapped
camera offset during humanoid initialization, after resolving a skinned GLTF
and mapped head. A rigid car has no humanoid head, so initialization exits and
nothing consumes the authored offset. That coupling exposed the missing
generic transform operation.

---

## 3. Core relationship

This is not a rule to modify "everything else" beneath a pose driver. It is an
explicit relationship between:

- an incoming desired pose,
- one referenced local anchor transform, and
- one downstream content subtree.

Let:

- `P` be the incoming tracked or otherwise driven pose in world space,
- `C` be the selected anchor's authored local transform, and
- `A` be the downstream content's existing authored local transform.

The relationship is:

```text
operator output = P * inverse(C)
content world = P * inverse(C) * A
```

For a translation-only anchor, the inverse reduces to the required negative
translation. Rotation must eventually use a complete transform inverse rather
than independently negating Euler angles.

For XR:

```text
rendered XR camera = P
camera anchor = C
model content = P * inverse(C) * A
```

The physical camera stays at `P`. Only content explicitly connected to the
operator output receives the inverse anchor.

---

## 4. Proposed primitive

Add a transform-stream component, provisionally named
`TransformApplyInverseLocal`.

It has two inputs:

1. the ordinary inherited transform stream, and
2. a `ComponentRef` selecting a `TransformComponent` whose authored local
   matrix is the anchor offset.

Its output is:

```text
output_world = inherited_world * inverse(source.local_matrix)
```

The remote reference selects only the source to read. The destination is
structural: non-operator children of `TransformApplyInverseLocal` receive the
output stream.

Do not give the operator a second selector naming a transform to mutate. A
remote write would hide ownership, complicate ordering, allow multiple writers,
and make the affected subtree invisible in the authored topology.

The intended split is:

```text
source:      selected through ComponentRef
destination: expressed through ordinary child topology
```

The source should initially be an explicit `TransformComponent`, not a
`CameraXR` that the engine silently resolves to a transform ancestor.

The operator belongs to the existing transform-stream architecture and should
be evaluated by `TransformStreamSystem`. This is a real second input/reference,
but it does not require a general arbitrary node/edge graph runtime. The
existing live-handle, GUID, and query forms of `ComponentRef` are sufficient.

The first version should read `source.local_matrix`, not `source.world_matrix`.
Inverting the world matrix would also invert the tracked parent pose. A future
relative-world operator would need a separately defined reference basis and is
outside this task.

---

## 5. Draft MMS syntax

`TransformApplyInverseLocal` does not exist yet. These examples define the
proposed authored API using current MMS component-constructor and
`ComponentRef` conventions.

### 5.1 XR cockpit camera and rigid car

Passing the component object returned by `T.position(...)` follows the existing
live-handle reference convention. It can be retained and serialized as a
durable GUID reference.

```meow_meow
InputXR.on() {
    T {
        name = "car_xr_driver"

        let cockpit_camera_offset = T.position(0.0, 3.0, -1.55) {
            name = "cockpit_camera_offset"
            CXR { Pointer {} }
        }
        cockpit_camera_offset

        TransformApplyInverseLocal.source(cockpit_camera_offset) {
            T.position(0.0, 0.10, 0.0) {
                name = "car_vehicle"
                T.rotation(0.0, 0.0, 0.0) {
                    GLTF.new("assets/models/car.glb") {
                        bisket_anime_shading()
                    }
                }
            }
        }

        XRHand.new(true, "Left", "GripAim").laser() {
            T { Pointer {} }
        }
        XRHand.new(true, "Right", "GripAim").laser() {
            T { Pointer {} }
        }
    }
}
```

Only `car_vehicle` is beneath the inverse operator. The camera and tracked
controllers are siblings and do not receive its output.

The equivalent selector-authored source is:

```meow_meow
TransformApplyInverseLocal.source("#cockpit_camera_offset") {
    T {
        name = "car_vehicle"
        GLTF.new("assets/models/car.glb") {}
    }
}
```

### 5.2 Controller drives a tool by its grip anchor

This uses the same operation without a camera. The controller pose is the
incoming stream. `tool_grip_offset` describes the model-local grip point that
should land at the controller pose.

```meow_meow
let tool_grip_offset = T.position(0.0, -0.04, 0.12) {
    name = "tool_grip_offset"
}

XRHand.new(true, "Right", "Grip") {
    T {
        name = "right_grip_pose"

        TransformApplyInverseLocal.source(tool_grip_offset) {
            T.rotation(0.0, 1.5707963, 0.0) {
                name = "tool_model"
                GLTF.new("assets/models/tool.glb") {}
            }
        }
    }
}
```

The anchor is separate authored reference data. It must not live beneath the
operator output, which would make the source depend on its own result.

### 5.3 Place an object by an authored socket

```meow_meow
let lamp_socket_offset = T.position(0.0, 0.42, 0.0) {
    name = "lamp_socket_offset"
}

T.position(4.0, 2.0, -1.0) {
    name = "desired_socket_pose"

    TransformApplyInverseLocal.source("#lamp_socket_offset") {
        T {
            name = "lamp_model"
            GLTF.new("assets/models/lamp.glb") {}
        }
    }
}
```

The lamp is positioned so its authored socket, rather than its model origin,
lands at `desired_socket_pose`.

### 5.4 Humanoid camera anchor

The generic operator expresses the camera-offset relationship.
`AvatarControl` remains responsible for humanoid-only head-anchor calibration,
body follow, and IK.

```meow_meow
InputXR.on() {
    T {
        name = "avatar_xr_driver"

        let eye_offset = T.position(0.0, 0.08, 0.12) {
            name = "avatar_eye_offset"
            CXR { Pointer {} }
        }
        eye_offset

        AVC {
            TransformApplyInverseLocal.source(eye_offset) {
                T {
                    name = "avatar_model_root"
                    GLTF.new("assets/models/avatar.vrm") {}
                }
            }

            XRHand.new(true, "Left", "Grip") { T {} }
            XRHand.new(true, "Right", "Grip") { T {} }
        }
    }
}
```

This is draft topology, not yet a decision that AVC can discover its model root
through the operator exactly as written. The important division is that the
inverse-local operator owns `inverse(C)`, while AVC adds only humanoid
specialization.

---

## 6. Runtime topology and ownership

The XR car resolves to this conceptual topology:

```text
tracked pose P
  |-- camera wrapper C
  |     `-- CameraXR
  |-- TransformApplyInverseLocal(source=C)
  |     `-- authored model root A
  |-- independently tracked left controller
  `-- independently tracked right controller
```

This preserves the model root's authored ground/model-origin adjustment. It
also gives the inverse offset one explicit owner.

The following do not receive compensation unless explicitly placed beneath the
operator:

- OpenXR eye poses,
- XR hand/controller target transforms,
- eye-tracking samples,
- locomotion roots, and
- unrelated sibling models.

Collision visuals or shapes that must follow the compensated model can be
placed beneath the same operator. The locomotion movement target should remain
outside it.

---

## 7. Humanoid specialization

A mapped humanoid head adds calibration information; it does not enable the
base inverse-anchor meaning.

For a rigid or non-humanoid model:

```text
model_world = P * inverse(C) * A
```

For a humanoid, let `H` represent the mapped head/camera anchor in model space.
The body additionally needs the inverse head-anchor calibration:

```text
model_world = P * inverse(C) * inverse(H) * A
head_target_world = P * inverse(C)
```

Existing head displacement, body yaw follow, neck policy, and arm IK may then
operate from those calibrated targets. The generic operator owns the `C`
inverse. AVC must not also apply its old eye-offset adjustment to the same
branch.

Missing head mapping may skip humanoid-specific work without affecting an
explicitly authored generic inverse-local operator.

---

## 8. Open design decisions

1. Whether the public name is `TransformApplyInverseLocal`,
   `TransformInverseAnchor`, or another name that makes the selected source and
   local-space behavior clear.
2. Whether the first implementation supports translation only or complete
   inverse TRS. Translation is required. Rotation needs a mathematically
   complete inverse. Non-unit scale needs an explicit support or rejection
   policy.
3. Whether version one permits any referenced transform or restricts the
   source to the same authored assembly/subtree.
4. Whether query selectors support an explicit `.root(...)` scope like
   `TransformParent`, in addition to `.source(...)`.
5. Whether live edits to the referenced local transform update the stream
   immediately. This is preferred, but requires dependency invalidation.
6. How unresolved sources behave. Retaining the last valid result is preferable
   to silently substituting identity.
7. How source/output dependency cycles are detected and diagnosed.
8. How this operator composes with `TransformForkTRS`, especially body-yaw
   shaping, and which ordering expresses the desired space for the anchor.
9. How existing AVC camera discovery, reparenting, and eye-offset compensation
   migrate without applying the offset twice.
10. Whether AVC should create this operator as convenience/runtime topology or
    require it to be explicit in authored MMS. The underlying semantics remain
    outside AVC either way.

---

## 9. Proposed implementation phases

Implementation note (2026-09-08): the first implementation uses the public
name `TransformApplyInverseLocal`, applies a complete invertible local matrix,
retains the last valid inverse while a source is unresolved or singular, and
rejects a structurally downstream source. `mittens-corp.mms` now uses the
operator for its rigid car without `AvatarControl`. The deliberately simple
camera-height case was validated in XR: the inverse places the rigid model at
the intended offset while headset rotation and gamepad locomotion remain
coherent.

### Phase 1: Add the generic inverse-local operator

- Add `TransformApplyInverseLocalComponent` with a `ComponentRef` source and
  MMS round-trip support.
- Evaluate it as a transform-stream boundary in `TransformStreamSystem`.
- Pass its result only to structural downstream children.
- Define unresolved-reference and dependency-cycle behavior.
- Add source-change invalidation.

### Phase 2: Author the rigid XR car explicitly

- Update `mittens-corp.mms` to place the car beneath the inverse-local operator.
- Preserve the car root's authored local transform.
- Keep XR controller transforms outside the compensated branch.
- Verify with a deliberately large camera Y offset.

### Phase 3: Refactor humanoid integration

Priority note (2026-09-08): defer this phase. The generic operator solves the
rigid/non-humanoid case without AVC changes, and consolidating AVC's partial
camera-offset handling does not currently unlock required behavior. Revisit
when humanoid camera compensation or AVC topology is otherwise being changed;
at that point, use the generic operator for the authored camera offset and
take care not to apply AVC's existing eye-offset compensation twice.

- Reuse the generic operator relationship for the camera offset.
- Keep head-anchor calibration and body-follow behavior as AVC
  specializations.
- Remove or bypass duplicate AVC eye-offset compensation.
- Decide whether AVC emits convenience topology or consumes authored topology.

### Phase 4: Complete TRS and live-update semantics

- Add full rotation coverage using a complete inverse transform.
- Define or reject non-unit scale.
- Verify runtime edits, reference rebinding, and teardown.

---

## 10. Required regression coverage

1. **Rigid GLTF:** changing camera-wrapper Y from `3` to `17` produces the
   corresponding inverse model displacement.
2. **Authored model offset preservation:** inverse-anchor application does not
   erase a model root offset such as `[0, 0.1, 0]`.
3. **Identity anchor:** an identity source leaves downstream placement
   unchanged.
4. **Explicit destination:** an uncompensated sibling model is unchanged.
5. **No AVC dependency:** generic inverse-anchor placement works without
   `AvatarControl` or a humanoid map.
6. **Tracked controllers:** controller world poses remain unchanged when the
   camera offset changes.
7. **Reference forms:** live-handle, GUID, and query sources resolve and
   round-trip consistently.
8. **Live source edit:** changing the source local transform invalidates and
   updates the downstream result according to the chosen policy.
9. **Invalid reference:** an unresolved source does not silently cause an
   identity-based jump.
10. **Cycle:** a source below the operator output is rejected without an
    infinite propagation loop.
11. **Humanoid mapped head:** existing head/body calibration remains stable and
    the camera offset is applied exactly once.
12. **Full inverse:** if rotation is supported, combined rotation and
    translation use a complete inverse rather than independent negation.

The `mittens-corp` example remains the visual proof scene. A large temporary
camera Y offset should make the inverse model movement unmistakable in XR.

---

## 11. Acceptance criteria

This task is complete when:

- an authored component can select a local anchor transform through
  `ComponentRef` and apply its inverse to an inherited transform stream,
- its destination is expressed by child topology rather than a remote mutation
  target or broadcast to pose-driver siblings,
- a wrapped `CameraXR` transform can use that operator with rigid, skinned
  non-humanoid, and humanoid models,
- the rendered XR camera remains based on the physical OpenXR eye pose,
- generic inverse-anchor behavior has no AVC or humanoid-map dependency,
- independently tracked hands/controllers remain outside the compensated
  branch,
- authored downstream transforms are preserved,
- humanoid head-anchor calibration remains an additional specialization,
- the camera offset is applied exactly once, and
- automated regression tests cover generic, car, and humanoid paths.
