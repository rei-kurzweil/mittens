# Refactor: introduce a PointerSystem

This doc proposes adding a dedicated `PointerSystem` as the runtime home for pointer behavior.

This is intentionally a **refactor note**, not a final spec.
The goal is to describe a cleaner system boundary for pointers before we move the result into `docs/spec`.

## Why this exists

Right now pointer behavior is split awkwardly across several places:

- `PointerComponent` exists as the scene-facing authored component
- `SystemWorld::register_pointer(...)` spawns a child `RayCastComponent`
- `RayCastSystem` infers ray source behavior from topology
- `GestureSystem` still assumes mouse-left for drag lifecycle

That means there is no single runtime place that answers:

- what kind of pointer is this?
- where does its ray come from?
- what input source should trigger click / drag / dwell for it?
- what runtime raycaster belongs to it?

Those questions are pointer questions, not raycast questions and not gesture questions.

## Proposed responsibility split

### PointerSystem

`PointerSystem` should own pointer interpretation.

Its job is to:

- discover and track `PointerComponent`s
- classify pointer topology / pose lineage
- resolve pointer ray source policy
- resolve pointer trigger policy
- own runtime pointer state
- create / maintain the pointer-owned `RayCastComponent`
- expose pointer-derived state to `RayCastSystem` and `GestureSystem`

Conceptually, `PointerSystem` should answer:

- "what pointer is this?"
- "what ray should it produce this frame?"
- "what trigger/button semantics does it use this frame?"

### RayCastSystem

`RayCastSystem` should become narrower.

Its job should be:

- consume ray requests / ray state
- intersect rays with eligible world geometry
- emit hit facts such as `RayIntersected`

It should not be responsible for inferring higher-level pointer meaning from topology.

### GestureSystem

`GestureSystem` should also become narrower.

Its job should be:

- consume pointer trigger state + hit facts
- maintain drag / click / hover lifecycle state
- emit `DragStart` / `DragMove` / `DragEnd`

It should not have to decide whether a pointer is mouse, head, controller, or fixed-camera.

## PointerSystem responsibilities in more detail

### 1) Pointer registration and owned runtime children

Today, pointer registration lives directly in `SystemWorld::register_pointer(...)`.

That logic should move behind `PointerSystem`.

Minimal behavior:

- when a `PointerComponent` is registered, ensure it owns exactly one runtime `RayCastComponent`
- cache the pointer ↔ raycaster relationship
- keep that relationship stable across topology changes where possible

### 2) Topology / lineage classification

`PointerSystem` should classify a pointer from its surrounding topology.

This is the logic we have started sketching already:

- controller / hand driver lineage
- desktop input / camera-driver lineage
- XR input / head-driver lineage
- camera-anchored fallback when no stronger driver exists
- generic transform fallback

Important rule:

- a `Pointer` may remain attached under a camera component
- a stronger outer driver ancestry can still win when trigger semantics are inferred

Example:

```text
InputComponent
└── Transform
    └── Camera3DComponent
        └── PointerComponent
```

Here the pointer can stay camera-local, while the outer desktop driver lineage still wins for trigger policy.

### 3) Ray source resolution

`PointerSystem` should determine how a pointer ray is formed.

Examples:

- desktop camera pointer → cursor through active camera
- fixed camera pointer → cursor through that camera anchor
- XR head pointer → head/camera-aligned ray
- controller pointer → parent-forward / aim-forward ray
- generic transform fallback → transform-forward ray

This lets `RayCastSystem` consume a resolved pointer ray instead of re-deriving topology intent itself.

### 4) Trigger policy resolution

`PointerSystem` should determine the trigger source paired with a pointer.

Examples:

- desktop input lineage → mouse button edges / cursor presence
- XR head lineage → dwell / confirm / runtime head-select action
- controller lineage → trigger / select / squeeze action
- fixed camera fallback with no stronger driver → desktop camera implies mouse trigger policy

This is likely the most important missing behavior today.

### 5) Runtime pointer state

A `PointerSystem` likely needs explicit runtime state per pointer.

For example:

- `pointer_id`
- owned `raycaster_id`
- resolved pointer kind / topology classification
- resolved ray source kind
- resolved trigger kind
- latest ray (if available)
- latest trigger state (`pressed`, `down`, `released`)
- whether screen-space cursor data exists for this pointer

This state can start as an internal cache and later become signal payloads if needed.

## Suggested data flow

A likely future frame flow is:

1. `InputSystem` / `OpenXRSystem` update raw device state
2. `PointerSystem` resolves pointer topology and per-pointer runtime state
3. `PointerSystem` requests or publishes pointer rays / trigger state
4. `RayCastSystem` performs intersections for those rays
5. `GestureSystem` consumes pointer trigger state + hit facts
6. editor / gizmo systems consume gesture events

That puts pointer interpretation in one place instead of splitting it across raycast and gestures.

## Relationship to current docs

This refactor note is intended to line up with:

- `docs/draft/pointer.md`
- `docs/task/refactor/gesture-refactor.md`
- `docs/task/refactor/raycast-driven-by-actions.md`

Roughly:

- `pointer.md` describes the authored model and high-level intent
- this doc describes where that behavior should live at runtime
- the gesture/raycast refactors describe how downstream systems become narrower once pointers are explicit

## Minimal incremental plan

### Step 1: introduce `PointerSystem`

Create a new system that initially does only:

- register pointers
- own child raycaster creation
- cache pointer ↔ raycaster mapping

No behavior change yet.

### Step 2: move topology classification into `PointerSystem`

Move the current pointer-topology classification out of `RayCastSystem`.

`PointerSystem` becomes the source of truth for:

- desktop lineage
- XR lineage
- controller lineage
- camera fallback

### Step 3: move trigger policy inference into `PointerSystem`

Add per-pointer trigger policy resolution.

This can still feed the current mouse-only `GestureSystem` at first via adapter logic.

### Step 4: make `RayCastSystem` consume pointer rays

Once pointer rays are explicit:

- remove topology inference from `RayCastSystem`
- make it consume resolved pointer rays or raycast requests

### Step 5: make `GestureSystem` consume pointer trigger state

Once trigger policy is explicit:

- remove hard-coded mouse-left assumptions from `GestureSystem`
- make drag lifecycle per pointer

## Open questions

- Should `PointerSystem` publish explicit signals such as `PointerRay` / `PointerTrigger`, or just maintain internal state queried by other systems?
- Should the owned `RayCastComponent` remain visible as a normal ECS component, or become a more internal runtime detail later?
- Should fixed-camera pointers default to desktop mouse trigger policy automatically, or require an explicit override in some cases?
- When multiple lineage cues exist, do we always want the priority order:
  - controller lineage
  - camera/head driver lineage
  - camera-anchored fallback
  - generic transform fallback

## Current recommendation

Yes: pointer behavior probably belongs in a dedicated `PointerSystem`.

That gives us a cleaner architecture:

- `PointerSystem` = pointer meaning
- `RayCastSystem` = ray → hit
- `GestureSystem` = hit + trigger → drag/click lifecycle

That is a much better fit for the direction we want than continuing to grow pointer logic inside `RayCastSystem`.
