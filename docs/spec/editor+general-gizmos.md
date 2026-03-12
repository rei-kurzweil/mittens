# Editor + general gizmos

This document describes the current “editor root + gizmos” model and how it scales to multiple gizmo types.

## Goals

- An **editor root** owns editor-only tools (gizmos) and a set of editable objects.
- Clicking an object under the editor root should:
  - Detach the transform gizmo from any previous selection
  - Attach it to the clicked target so subsequent drags manipulate that target
- Support multiple gizmo types in the future (Transform / Color / Text / etc.) without changing the interaction pipeline.

## Core concepts

### `EditorComponent`

- `EditorComponent` is a marker for an editor subtree root.
- Objects that should be selectable/editable are parented under this subtree.
- Editor-owned gizmos (currently: a single `TransformGizmoComponent`) should also live somewhere under this subtree.

### Gizmo components

- `TransformGizmoComponent` is a transform manipulation tool.
- It is reparented under the selected target’s `TransformComponent`.
- It spawns its own visual/interactive subtree on init (handles, rings, etc.).

### Coordinate spaces

Translation and rotation gizmos may operate in either **World** or **Local** coordinate space (independently). See:

- [docs/spec/editor-gizmo-coord-spaces.md](docs/spec/editor-gizmo-coord-spaces.md)

## Interaction flow (signals)

The engine input pipeline is signal-driven:

1. `RayCastSystem` emits `SignalValue::RayIntersected` facts.
2. `GestureSystem` consumes ray hits + input and emits drag facts:
   - `SignalValue::DragStart`
   - `SignalValue::DragMove`
   - `SignalValue::DragEnd`
3. `EditorSystem` consumes `DragStart` and emits an action:
   - `SignalValue::Attach { parents, child }`
4. `ActionSystem` handles `Attach` by mutating topology and emitting:
   - `SignalValue::ParentChanged`
5. `TransformGizmoSystem` consumes drag facts and mutates the currently-attached target transform.

Key detail: editor-driven reparenting goes through `Attach` so topology changes remain consistent with the rest of the engine (e.g. `ParentChanged` emission, init/refresh behavior).

## Current behavior (selection)

On `DragStart`:

- Find the nearest `EditorComponent` ancestor of the clicked renderable.
  - If none: do nothing.
- Ignore clicks on gizmo handles (anything with a `TransformGizmoComponent` ancestor).
- Resolve the clicked object’s nearest `TransformComponent` ancestor.
  - If none: do nothing.
- Resolve the editor’s `TransformGizmoComponent` (cached on `EditorComponent`, with a subtree search fallback).
- Emit `Attach` to reparent the gizmo under the clicked `TransformComponent`.

The gizmo then manipulates the newly selected target.

## Generalizing to multiple gizmo types

The intended scaling model is:

- Each gizmo type has:
  - A component (e.g. `ColorGizmoComponent`, `TextGizmoComponent`)
  - A system that:
    - Spawns visuals
    - Consumes drag facts
    - Emits or performs mutations
- `EditorSystem` becomes the selection “router”:
  - It decides *which* gizmos to reattach on click (maybe based on an editor mode, per-gizmo enable flags, etc.)
  - It emits `Attach` actions for the chosen gizmo(s)

Possible next steps:

- Add an explicit editor mode component/state (Translate/Rotate/Scale, or GizmoType selection).
- Add per-gizmo enable flags under `EditorComponent`.
- Add a dedicated “selection changed” fact signal if other systems need to react.

## Serialization notes

- The component codec recognizes both the new transform gizmo type names (`transform_gizmo*`) and the legacy names (`gizmo*`) for backward compatibility.
- Runtime-only editor caches (like the resolved gizmo id stored on `EditorComponent`) are not serialized.
