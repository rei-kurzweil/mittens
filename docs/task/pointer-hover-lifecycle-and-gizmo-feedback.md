# Task: pointer hover lifecycle and gizmo feedback

Date: 2026-08-08

Status: todo / design and implementation

## Goal

Allow interactive objects, renderables, transforms, panel rows, and gizmo handles to respond when a
pointer aims at them without requiring a trigger press.

Use the Twilight Rooftop avatar picker to validate desktop and XR hover first.
The [panel hover task](ui-button-hover-and-avatar-picker-feedback.md) defines
click parity, the initial sampling policy, the avatar picker appearance, and
the performance checks. Transform gizmos can use the same lifecycle for handle
feedback before drag start.

## Required behavior

- Desktop and XR pointers can acquire hover targets while idle.
- Hover state is per pointer; two pointers may hover different targets simultaneously.
- Enter, continued hover/move, and leave transitions are distinguishable.
- Triggering a drag does not accidentally retarget the active gesture.
- A captured drag keeps its original drag target even if hover moves elsewhere.
- Removing, disabling, hiding, or making a hovered target non-raycastable produces a clean leave.
- Consumers can react without rebuilding large authored UI subtrees every frame.

## Proposed lifecycle vocabulary

Candidate runtime events:

```text
PointerEnter  { pointer, raycaster, renderable, hit_point, ray }
PointerHover  { pointer, raycaster, renderable, hit_point, ray }
PointerLeave  { pointer, raycaster, renderable }
```

Names and event shape remain open. At minimum, enter and leave edges are required; a per-frame
`PointerHover` event should exist only if a consumer needs continuous hit points.

The event target should be the resolved interaction target while retaining the concrete hit
renderable in the payload. This distinction matters for handle subtrees and authored components
whose raycastable marker lives on an ancestor.

## Runtime ownership

Likely split:

- `PointerSystem`: pointer identity, active/idle sampling policy, and per-pointer hover target.
- `RayCastSystem`: current ray plus ordered hit facts, including idle casts requested for hover.
- hover lifecycle reducer: compare previous and current resolved target and emit transition edges.
- consumers such as `TransformGizmoSystem`: apply local visual feedback.

The new hit-independent `PointerRaySnapshot` seam helps expose idle pointer rays, but event-driven
raycasters currently need an explicit reason to cast. Hover therefore needs a deliberate sampling
policy rather than being inferred from trigger activation.

## Sampling and performance follow-ups

- Start with enabled pointers and live scoped hover consumers. Use the click
  candidate resolver for UI and emit only enter and leave edges.
- Share an interaction query when click, drag, or grip already needs the same
  ray. Keep hover-only sampling out of the raw hit event stream.
- Measure stationary-pointer reuse with invalidation for picking changes. Use
  the panel case and a dense moving scene to guide further sampling changes.

## Gizmo feedback follow-up

1. Resolve the hovered gizmo handle independently of active drag state.
2. Highlight only that handle's visual subtree.
3. Prefer brightness/emissive change for the first slice; optional scale enlargement must preserve
   the gizmo anchor and must update matching hit bounds.
4. Clear the effect on leave, disable, selection change, drag end, or subtree removal.
5. Define drag precedence: active handle styling wins over hover styling until release.

## Acceptance criteria

- [ ] An idle desktop pointer emits one enter edge when it first aims at a target and one leave edge
      when it departs.
- [ ] An idle XR controller pointer has equivalent behavior without pressing trigger or grip.
- [ ] Crossing directly from A to B emits leave(A) and enter(B) in a documented order.
- [ ] Two pointers retain independent hover targets.
- [ ] Hover target resolution respects interaction priority and pass-through policy.
- [ ] Captured drags remain targeted at their original renderable while hover may change.
- [ ] Hovering a gizmo handle visibly highlights only the intended axis/plane/ring.
- [ ] Hover enlargement, if enabled, keeps visible and raycast geometry synchronized.
- [ ] Removing a hovered subtree cannot leave stale visual state or component IDs.
- [ ] Tests cover desktop, XR, target crossing, zero hits, removal, simultaneous pointers, and
      hover-to-drag transitions.

## Non-goals for the first slice

- CSS-style `:hover` query syntax.
- General animation authoring for every hover effect.
- Dwell activation or gaze-click behavior.
- Replacing click and drag gesture events.

