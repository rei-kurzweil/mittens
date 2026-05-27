# Pointer → Input → Ray → Gesture

This document describes the interaction pipeline from authored `PointerComponent` topology through ray requests, hit testing, gesture generation, and editor/gizmo consumption.

It exists to unify behavior that was previously split across:

- `docs/spec/input-intent-data-flow.md`
- `docs/spec/bvh-and-raycast.md`
- `docs/spec/gestures-and-gizmos.md`
- `docs/spec/system/editor.md`

## Scope

This is the interaction flow for pointer-driven selection/manipulation.

It answers:

- what kinds of pointers the runtime recognizes
- what trigger/input modes are supported today
- how a pointer turns into a raycast
- how a ray hit turns into a drag gesture
- where desktop and XR behavior intentionally differ

## High-level flow

```text
PointerComponent topology
  → pointer/raycaster registration
  → trigger policy / cast request
  → RayCastSystem
  → RayIntersected
  → GestureSystem
  → DragStart / DragMove / DragEnd
  → EditorSystem / TransformGizmoSystem
```

More concretely:

1. `PointerComponent` exists in authored topology.
2. Runtime ensures the pointer owns a `RayCastComponent`.
3. Some input/trigger source requests a cast for that pointer.
4. `RayCastSystem` resolves the ray origin/direction from topology.
5. `RayCastSystem` emits `RayIntersected` facts.
6. `GestureSystem` combines hit facts with trigger edges to produce drag events.
7. `EditorSystem` may treat `DragStart` as selection.
8. `TransformGizmoSystem` consumes drag events to manipulate a target transform.

## Runtime pieces

### PointerComponent

`PointerComponent` is the authored scene-facing marker for “this subtree owns an interaction pointer”.

Authoring examples:

- desktop camera pointer
- XR camera/head pointer
- controller pointer
- fixed-camera scene pointer

### Pointer-owned RayCastComponent

At runtime, a pointer owns a child `RayCastComponent`.

Current responsibility split:

- `PointerSystem` ensures the runtime raycaster exists
- `RayCastSystem` performs hit testing
- `GestureSystem` turns hit facts into drag lifecycle events

## Pointer source modes

The runtime distinguishes pointer source behavior from pointer trigger behavior.

### Source mode: cursor-through-camera

Used for desktop-style camera pointers.

Behavior:

- ray is built from the current window cursor position
- ray passes through the active camera projection/view
- this is the expected editor/desktop selection path

This is the only fully supported editor interaction path today.

### Source mode: parent-forward

Used as the generic fallback for non-cursor pointers.

Behavior:

- ray origin comes from the nearest ancestor transform
- ray direction is the transform’s forward axis (`-Z` in engine convention)

This is the current path for controller-like and non-desktop pointers unless a cursor-through-camera desktop anchor is detected.

### Camera-local pointer nuance

A `PointerComponent` may remain attached under `Camera3DComponent` or `CameraXRComponent`.

That local attachment does not necessarily mean its trigger semantics are “camera-owned”. A stronger outer input lineage may still determine how the pointer is triggered.

Examples:

- desktop input → transform → camera → pointer
- XR input → avatar control → camera XR → pointer

In both cases, the pointer can stay camera-local while trigger policy is still derived from the enclosing driver lineage.

## Trigger/input modes

Trigger policy answers a different question from source mode:

- source mode: where does the ray come from?
- trigger mode: when is this pointer allowed to request a cast / click / drag lifecycle?

### Supported today

#### Desktop mouse

Desktop mouse is the only supported pointer trigger mode today.

Meaning:

- mouse press starts drag selection/interaction
- mouse hold continues drag
- mouse release ends drag
- editor selection and gizmo dragging are built around this mode

### Not yet supported as real trigger modes

#### XR head dwell

Planned, but not implemented yet.

Expected future direction:

- configured by a component attached under `InputXRComponent`
- produces cast/click requests for a head pointer without using desktop mouse input

#### XR controller select/trigger

Planned, but not implemented yet.

Expected future direction:

- controller input becomes the producer of cast requests / click edges
- controller pointer rays should not depend on desktop mouse state

## Important current limitation

Today, event-driven raycasters are still implicitly coupled to desktop mouse input inside `RayCastSystem`.

That means the implementation currently behaves as if:

- every event-driven pointer may cast on desktop mouse press

This is broader than intended.

Desired behavior is narrower:

- only desktop mouse-driven pointers should auto-cast from desktop mouse input
- XR head/controller pointers should wait for their own trigger producers

This limitation is the reason mixed desktop/XR scenes can currently produce incorrect selection results.

## Current supported interaction matrix

| Pointer topology | Ray source | Trigger mode | Support status | Notes |
|---|---|---|---|---|
| Desktop camera pointer | Cursor-through-camera | Desktop mouse | Supported | Primary editor path |
| Fixed-camera desktop pointer | Cursor-through-camera fallback | Desktop mouse | Supported | Useful in non-avatar desktop scenes |
| Generic transform pointer | Parent-forward | Desktop mouse | Partial | Source works, but not the main editor path |
| XR head pointer | Parent-forward / camera-local XR lineage | XR dwell | Not yet supported | Future `InputXR`-attached config |
| XR controller pointer | Parent-forward | Controller trigger/select | Not yet supported | Future XR action path |

## Ray request semantics

Conceptually, raycasting should be request-driven.

A pointer should cast because some producer requested it:

- desktop mouse interaction
- future XR dwell
- future controller trigger
- tool/system-driven request

This is distinct from “the pointer exists”.

The long-term direction is:

- pointer existence does not imply continuous casting
- per-frame casting policy is not owned by `RayCastSystem`
- `RayCastSystem` should answer hit-test queries when requested

## RayCastSystem role

`RayCastSystem` should be thought of as the intersection stage.

Its job:

- resolve the ray for a casting pointer
- intersect against raycastable renderables
- emit `RayIntersected`

It should not be the long-term owner of pointer trigger policy.

## GestureSystem role

`GestureSystem` consumes hit facts plus trigger edges.

Today it is still desktop-mouse oriented:

- `DragStart` on left mouse press when a hit exists
- `DragMove` while dragging
- `DragEnd` on left mouse release

Important nuance:

- gestures are downstream of ray hits
- if the wrong pointer is allowed to cast, gestures can attach to the wrong renderable even when the editor logic itself is correct

## Editor and gizmo consumption

### EditorSystem

`EditorSystem` listens for `DragStart` under an editor subtree.

It does not decide which pointer should have been active. It trusts the incoming drag fact and resolves the nearest transform ancestor from the hit renderable.

### TransformGizmoSystem

`TransformGizmoSystem` consumes drag events and binds the drag to the captured raycaster.

It also supports route-upward proxy transforms for cases like glTF visualization handles.

So if the wrong pointer wins at the raycast stage, the downstream systems will faithfully follow that wrong choice.

## Editor-selectable topology vs pointer triggering

Do not confuse these two concepts:

- **raycastable topology**: whether an object is eligible to be hit at all
- **pointer triggering**: whether a pointer is allowed to cast this frame

Editor auto-raycastable wrappers affect the first question.

Desktop/XR pointer trigger policy affects the second question.

These are independent concerns.

## Practical summary

### What works today

- desktop pointer selection
- desktop gizmo dragging
- camera-local desktop pointers
- editor subtree selection with explicit or editor-materialized raycastability

### What is only partially working today

- mixed desktop/XR scenes with multiple pointers
- generic transform-forward event-driven pointers using desktop mouse input

### What is intentionally deferred

- XR head dwell click
- XR controller trigger/select gesture flow
- final request-driven cast pipeline with pointer-owned trigger policy

## Related docs

- `docs/spec/input-intent-data-flow.md`
- `docs/spec/bvh-and-raycast.md`
- `docs/spec/gestures-and-gizmos.md`
- `docs/spec/system/editor.md`
- `docs/refactor/pointer-trigger-policy-and-auto-casting.md`
- `docs/refactor/raycast-driven-by-actions.md`
