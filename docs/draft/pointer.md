# Pointer authoring draft

This document proposes a cleaner scene-facing model for `PointerComponent`.

It does **not** fully describe the current implementation in `src/`.

The current codebase now authors `Pointer {}` directly and lets the runtime spawn/own a child
`RayCastComponent`, but the higher-level source/trigger policy described here is still only
partially implemented.
This draft proposes the intended long-term authored/runtime relationship:

- authors place `Pointer {}` under a transform-like pose source
- the engine spawns/owns the corresponding `RayCastComponent` as a child of the pointer
- desktop and XR use the same authored shape
- pointer behavior is resolved from the **pose lineage** and the **action-trigger source**, not merely from whatever camera happens to be nearby in topology

In most scenes that lineage is a true **pose driver** (desktop rig transform, XR head transform,
controller transform). Some scenes, such as a fixed-camera desktop editor view, do not have a
separate driving transform at all. In that case the lineage needs a camera-anchored fallback.

## 1. Motivation

Current authoring is awkward because “make this thing behave like a pointer” requires two authored concepts:

- a `RayCastComponent`
- a `PointerComponent`

That leaks low-level raycast plumbing into scene authoring.

What authors usually mean is simpler:

- “this camera/head/controller should act like a pointer”
- “this pose source should supply the pointer ray, and this input source should drive click / drag / dwell”

So the authored component should be `Pointer`, not `RayCast`.

## 2. Proposed authored topology

### Desktop camera pointer

```text
I {
    T {
        C3D {}
        Pointer {}
    }
}
```

### Fixed camera pointer

```text
T {
    C3D {
        Pointer {}
    }
}
```

This is the important fallback case for scenes like `vtuber-desktop`:

- there may be no separate input-driven camera rig transform
- the camera is effectively fixed in place
- the pointer still needs a stable pose anchor and desktop mouse trigger pairing

In that situation, nesting `Pointer` under the camera is the clearest authored signal that the
camera itself is the pointer anchor.

This camera-local shape is also intentionally stable.
If the subtree is later wrapped by a higher-level driver such as `I { ... }` or `InputXR { ... }`,
the `Pointer` does **not** need to move.
The outer driver ancestry can win for trigger inference while the pointer remains camera-local.

### XR head pointer

```text
InputXR {
    T {
        CXR {}
        Pointer {}
    }
}
```

### XR controller / hand pointer

```text
CTLXR {
    T {
        Pointer {}
    }
}
```

The shared authored meaning is:

- `Pointer` marks “this pose source should produce an interaction pointer”
- the engine is responsible for creating and wiring the raycaster details

## 3. Pose driver and action trigger diagrams

The important distinction is:

- **pose lineage** decides where the pointer ray originates and points
- **action trigger** decides when that pointer should begin / continue / end interaction

Those may be related, but they are not the same concept.

### Desktop mouse pointer

```text
InputComponent / desktop controls
└── TransformComponent            ← pose driver for the camera rig
    ├── Camera3DComponent
    └── PointerComponent          ← authored pointer
        └── RayCastComponent      ← runtime helper

Action trigger source:
- desktop mouse buttons / cursor motion
```

The camera rig transform provides the pointer pose.
Mouse input provides the interaction trigger.

An important refinement is that this does not require `Pointer` to sit directly on the driver node.
It is valid for `Pointer` to remain nested under the camera while an outer driver ancestry still
wins when deciding which trigger source belongs to that pointer.

### Fixed camera desktop pointer

```text
TransformComponent            ← static scene anchor
└── Camera3DComponent
    └── PointerComponent      ← authored pointer
        └── RayCastComponent  ← runtime helper

Action trigger source:
- desktop mouse buttons / cursor motion
```

There is no distinct pose-driver component here.
The camera itself is the anchor that defines the pointer ray.

If this subtree is later wrapped by a desktop or XR driver, the pointer can remain attached to the
camera.
In that case:

- the camera-local attachment still communicates the ray anchor
- the outer driver ancestry becomes the stronger cue for trigger pairing

### XR head / gaze pointer

```text
InputXRComponent
└── TransformComponent            ← pose driver for head / HMD
    ├── CameraXRComponent
    └── PointerComponent          ← authored pointer
        └── RayCastComponent      ← runtime helper

Action trigger source:
- gaze dwell timer, or
- explicit head-pointer confirm action, or
- future runtime-specific head interaction signal
```

The head pose provides the pointer ray.
The trigger might be a dwell threshold rather than a button.

### XR controller / hand pointer

```text
ControllerXRComponent / hand driver
└── TransformComponent            ← pose driver for controller / hand
    └── PointerComponent          ← authored pointer
        └── RayCastComponent      ← runtime helper

Action trigger source:
- controller trigger / squeeze / select action
```

The controller transform provides the pointer ray.
The controller buttons provide the interaction trigger.

### Why this matters

These examples show why “find the nearest camera” is not the right mental model.

What matters is:

1. which ancestor chain defines the pointer pose
2. which input source is paired with that pointer for action lifecycle

So the engine should resolve a pointer from its **pose lineage**, not from incidental nearby components.

The refinement is:

- first prefer a true pose-driver lineage
- if none exists, allow a camera-anchored fallback when `Pointer` is nested under `Camera3D` or `CameraXR`
- camera-local pointer placement does not block a stronger outer driver ancestry from winning later

## 4. Proposed runtime topology

At runtime, a pointer should own a raycaster child.

Conceptually:

```text
Transform / pose source
└── PointerComponent
    └── RayCastComponent
```

This keeps the raycaster as an implementation detail of the pointer layer.

The important authored/runtime split is:

- authored API: `Pointer`
- runtime helper: `RayCastComponent`

## 5. Source resolution

The pointer’s ray source should be inferred from the surrounding topology.

More specifically: it should be inferred from the **pose lineage above the pointer**.

That means the engine should primarily ask:

- what transform lineage is driving or anchoring this pointer?
- is that lineage associated with a desktop camera rig, XR head rig, controller/hand rig, or a fixed camera anchor?

And importantly:

- does a stronger outer driver ancestry exist above a camera-local pointer subtree?

It should **not** primarily ask:

- what camera happens to be nearby somewhere else in the subtree?

### Camera-backed pointer

If the pointer is an immediate or indirect child of a transform lineage that is acting as a camera/head pose driver, the pointer uses that lineage as a camera/head-aligned pointer.

Examples:

- `C3D + Pointer` under the same transform → desktop camera pointer
- `CXR + Pointer` under the same transform → XR head pointer

Possible generalized shape:

```text
pose-driver transform
├── camera component
└── ...
    └── Pointer
```

### Camera-anchored fallback pointer

If no pose-driver transform lineage exists, but the pointer is nested under a camera component,
the camera acts as the pointer anchor.

Examples:

- `T { C3D { Pointer {} } }` → fixed desktop camera pointer
- `AVC { CXR { Pointer {} } }` → head/camera-anchored pointer when the camera is the clearest authored anchor

Possible generalized shape:

```text
transform
└── camera component
    └── ...
        └── Pointer
```

This fallback should be lower priority than a true driver lineage.
If both are present, the engine should prefer the explicit driving transform ancestry.

That means a subtree like this is valid and stable:

```text
Input / InputXR / other driver
└── Transform
    └── Camera
        └── Pointer
```

The `Pointer` may remain attached to the camera.
The surrounding driver ancestry can still win when choosing trigger semantics.

### Controller-backed pointer

If the pointer is an immediate or indirect child of a controller/hand pose-driver lineage, the pointer uses that lineage’s forward ray.

Example:

- `CTLXR { T { Pointer {} } }` → controller pointer

Possible generalized shape:

```text
controller / hand driver
└── pose-driver transform
    └── ...
        └── Pointer
```

### Trigger source pairing

Separately from pose resolution, the engine needs to pair each pointer with an interaction trigger source.

Examples:

- desktop camera pointer → mouse button state
- fixed desktop camera pointer → mouse button state
- XR head pointer → gaze dwell or confirm action
- XR controller pointer → controller trigger/select state

The trigger pairing is allowed to depend on camera ancestry even when pose fallback does.
For example, a `Pointer` nested under `Camera3D` with no explicit driver still clearly implies:

- cursor-bearing desktop pointer ray
- mouse-driven gesture lifecycle

But if that same camera-local pointer subtree is wrapped by a stronger driver ancestry, the
trigger pairing should follow the stronger outer lineage rather than the local camera attachment.

Examples:

- `I { T { C3D { Pointer {} } } }` → desktop mouse trigger wins
- `InputXR { T { CXR { Pointer {} } } }` → XR head/gaze trigger policy wins
- `ControllerXR { T { C3D { Pointer {} } } }` → controller trigger policy wins, even if the camera remains the local pointer anchor

This pairing is conceptually part of the pointer model even if the first implementation keeps gesture triggering elsewhere.

## 6. Does it matter whether Pointer is attached to a camera?

Only indirectly.

What matters more precisely is whether the pointer sits under a **camera/head pose lineage**.

That lineage may be either:

- a real driving transform, or
- a camera-anchored fallback when no such driver exists

`Pointer` should stay generic.

What changes by attachment context is:

- where the ray origin comes from
- how the ray direction is derived
- whether screen-space cursor data exists
- which input/button source should drive gesture start/end in the future

So the draft position is:

- `Pointer` itself remains generic and cross-platform
- camera-vs-controller differences are inferred by the engine from pose lineage and trigger pairing
- camera nesting matters specifically as a fallback authored cue when no better pose driver exists

## 7. Resolution order proposal

When resolving a pointer, the engine should use a stable priority order:

1. **Controller / hand driver lineage**
    - `Pointer` under a transform driven by `ControllerXRComponent` or equivalent hand driver
    - ray source: parent forward / controller forward
    - trigger: controller select / trigger / squeeze

2. **Camera/head pose-driver lineage**
    - `Pointer` under a transform lineage that also carries a camera/head role
    - ray source: camera/head-aligned
    - trigger: mouse buttons for desktop, dwell/confirm for head pointers

3. **Camera-anchored fallback**
    - no explicit driver lineage found, but `Pointer` is nested under `Camera3D` or `CameraXR`
    - ray source: that camera's pose anchor
    - trigger: inferred from camera kind (`Camera3D` → mouse, `CameraXR` → dwell/confirm/runtime action)

4. **Generic transform fallback**
    - `Pointer` is under a transform but under neither controller nor camera lineage
    - ray source: transform forward
    - trigger: explicitly configured later, or none by default

This gives fixed-camera scenes a principled place in the model without making "nearest camera"
the primary rule.

The key authored consequence is:

- `Pointer` does not need to be moved just because a new outer driver is introduced
- outer pose-driver ancestry can override trigger inference without rewriting the camera-local subtree

## 8. Relationship to gestures

This draft intentionally lines up with the longer-term gesture direction already discussed elsewhere:

- gestures should become pointer-driven rather than mouse-special-cased
- drag state should eventually be tracked per pointer
- screen-space gesture fields are optional and only present for desktop-style pointers

This means `Pointer` becomes the natural authored bridge between:

- desktop mouse/camera interaction
- XR head pointers
- XR controller/hand pointers

## 9. Proposed authoring semantics

### `Pointer {}`

Minimal form:

- enables pointer behavior with default engine policy

Possible future fields, not specified yet:

- enabled/disabled
- max distance
- cast policy (`continuous`, `event-driven`, etc.)
- handedness / source hint overrides
- gesture policy overrides

This draft intentionally does **not** standardize those options yet.
The key proposal is the topology and ownership direction.

## 10. Current implementation vs this draft

Current codebase:

- `PointerComponent` exists
- `Pointer {}` is authored directly
- the runtime spawns/owns a child `RayCastComponent`

This draft proposes:

- `Pointer` should be the authored entry point
- `RayCastComponent` should be spawned as pointer-owned runtime machinery

And specifically:

- pointer source should be resolved from pose lineage, with a camera-anchored fallback
- interaction lifecycle should ultimately be paired with the relevant trigger source for that pointer

## 11. Recommended implementation direction

When implementation work begins, the first useful step is:

1. register `PointerComponent` in MMS authoring as a first-class component
2. on pointer registration, spawn/maintain a child `RayCastComponent`
3. infer source mode from pose lineage:
    - desktop camera rig lineage
    - XR head / camera lineage
    - controller / hand lineage
    - fixed camera anchor lineage when no driver exists
4. keep current low-level `RayCastComponent` support internally for systems/runtime plumbing
5. later, pair each pointer with the appropriate trigger source:
    - mouse buttons
    - controller trigger/select
    - gaze dwell / confirm

That would let scenes author:

- desktop pointers
- XR head pointers
- XR controller pointers

with one consistent component shape.