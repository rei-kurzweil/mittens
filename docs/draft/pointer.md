# Pointer authoring draft

This document proposes a cleaner scene-facing model for `PointerComponent`.

It does **not** describe the current implementation in `src/`.

Today, `PointerComponent` is a small opt-in marker attached alongside or under an already-authored `RayCastComponent`.
This draft proposes flipping that relationship for authoring:

- authors place `Pointer {}` under a transform-like pose source
- the engine spawns/owns the corresponding `RayCastComponent` as a child of the pointer
- desktop and XR use the same authored shape
- pointer behavior is resolved from the **pose-driver ancestry** and the **action-trigger source**, not merely from whatever camera happens to be nearby in topology

## 1. Motivation

Current authoring is awkward because “make this thing behave like a pointer” requires two authored concepts:

- a `RayCastComponent`
- a `PointerComponent`

That leaks low-level raycast plumbing into scene authoring.

What authors usually mean is simpler:

- “this camera/head/controller should act like a pointer”
- “this pose driver should supply the pointer ray, and this input source should drive click / drag / dwell”

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

- **pose driver** decides where the pointer ray originates and points
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

So the engine should resolve a pointer from its **driver lineage**, not from incidental nearby components.

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

More specifically: it should be inferred from the **pose-driver ancestry above the pointer**.

That means the engine should primarily ask:

- what transform lineage is driving this pointer?
- is that lineage associated with a desktop camera rig, XR head rig, or controller/hand rig?

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
- XR head pointer → gaze dwell or confirm action
- XR controller pointer → controller trigger/select state

This pairing is conceptually part of the pointer model even if the first implementation keeps gesture triggering elsewhere.

## 6. Does it matter whether Pointer is attached to a camera?

Only indirectly.

What matters more precisely is whether the pointer sits under a **camera/head pose-driver lineage**.

`Pointer` should stay generic.

What changes by attachment context is:

- where the ray origin comes from
- how the ray direction is derived
- whether screen-space cursor data exists
- which input/button source should drive gesture start/end in the future

So the draft position is:

- `Pointer` itself remains generic and cross-platform
- camera-vs-controller differences are inferred by the engine from pose-driver ancestry and trigger pairing

## 7. Relationship to gestures

This draft intentionally lines up with the longer-term gesture direction already discussed elsewhere:

- gestures should become pointer-driven rather than mouse-special-cased
- drag state should eventually be tracked per pointer
- screen-space gesture fields are optional and only present for desktop-style pointers

This means `Pointer` becomes the natural authored bridge between:

- desktop mouse/camera interaction
- XR head pointers
- XR controller/hand pointers

## 8. Proposed authoring semantics

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

## 9. Current implementation vs this draft

Current codebase:

- `PointerComponent` exists
- it is authored/attached manually next to a `RayCastComponent`
- MMS does not yet expose the desired high-level pointer authoring flow

This draft proposes:

- `Pointer` should be the authored entry point
- `RayCastComponent` should be spawned as pointer-owned runtime machinery

And specifically:

- pointer source should be resolved from pose-driver ancestry
- interaction lifecycle should ultimately be paired with the relevant trigger source for that pointer

## 10. Recommended implementation direction

When implementation work begins, the first useful step is:

1. register `PointerComponent` in MMS authoring as a first-class component
2. on pointer registration, spawn/maintain a child `RayCastComponent`
3. infer source mode from pose-driver ancestry:
    - desktop camera rig lineage
    - XR head / camera lineage
    - controller / hand lineage
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