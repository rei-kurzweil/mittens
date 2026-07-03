# Refactor: pointer trigger policy and auto-casting

This note captures a runtime problem that shows up clearly in `vr-input.mms`:

- desktop mouse selection should be driven by the desktop camera pointer
- XR head / controller pointers should **not** automatically cast just because the desktop mouse was clicked

Right now those two behaviors are mixed together.

## Current problem

Today, every `PointerComponent` owns a child `RayCastComponent::event_driven()`.

Then `RayCastSystem::tick_with_queue(...)` does this:

- iterate all registered raycasters
- for each event-driven raycaster, cast on desktop left-mouse press

So a mouse click does not mean:

- "cast the desktop editor pointer"

It actually means:

- "cast every event-driven pointer in the world this frame"

That is acceptable in simple desktop scenes with a single pointer, but it breaks scenes that also contain XR or controller pointers.

## Symptom in `vr-input.mms`

`vr-input.mms` contains multiple pointers:

- desktop `Camera3D` pointer
- avatar-local `CameraXR` pointer
- left controller pointer
- right controller pointer

Those all become event-driven raycasters.

When the user clicks the desktop mouse, all of them cast.

`GestureSystem` then consumes the globally nearest `RayIntersected`, not the “desktop-intended” one. That allows an XR/head/controller ray to outrank the desktop cursor ray.

This is why avatar bone selection in `vr-input.mms` can fail even though the same avatar topology works in `vtuber-desktop.mms`.

## Root cause

We currently conflate three separate questions:

1. does this pointer exist?
2. what ray does it emit?
3. what trigger/input source is allowed to request a cast for it?

Question (3) is the missing piece.

The runtime has a notion of pointer topology / pose lineage, but cast triggering is still implicitly tied to desktop mouse input inside `RayCastSystem`.

That means XR pointers inherit a desktop trigger policy by accident.

## Desired rule

Automatic casting should be gated by pointer trigger policy, not just by `RayCastMode::EventDriven`.

Practical rule:

- desktop camera pointers may auto-cast from desktop mouse input
- XR head pointers do **not** auto-cast from desktop mouse input
- XR controller pointers do **not** auto-cast from desktop mouse input

So the future meaning of a mouse click should be closer to:

- "request a cast for desktop mouse-driven pointers"

not:

- "request a cast for every event-driven pointer"

## Near-term refactor direction

Before implementing dwell click or richer pointer actions, we should first stop automatic desktop mouse casting for non-desktop pointers.

The cleanest route is:

1. keep pointer registration / owned raycaster creation as-is for now
2. add a resolved trigger policy per pointer
3. only request casts automatically for pointers whose trigger policy is desktop mouse

This can be implemented either by:

- moving cast-request generation into `PointerSystem`, or
- teaching `RayCastSystem` to skip auto-casting for non-desktop pointer kinds as a temporary step

The first option is the cleaner architecture.

## Relationship to later XR dwell click

XR head selection should be added later as an explicit feature, not as a side-effect of desktop mouse clicks.

Planned direction:

- introduce a component attachable to `InputXRComponent`
- that component configures dwell-click / head-select behavior
- when present, it becomes the producer of cast requests / click intents for the XR head pointer

Important implication:

- **do not** preserve current XR auto-casting behavior for “compatibility”
- current behavior is not a feature; it is the bug

## Proposed future trigger-policy buckets

At minimum, runtime pointer trigger policy should distinguish:

- `DesktopMouse`
- `XrHeadDwell`
- `XrControllerSelect`
- `Disabled`

Early implementation only needs the first bucket wired end-to-end.

That is enough to fix desktop editor selection in mixed desktop/XR scenes.

The XR-specific buckets can be added later without reintroducing desktop-trigger leakage.

## Why this should happen before other editor fixes

As long as every pointer auto-casts on mouse press, editor selection in mixed scenes is fundamentally ambiguous.

That means other fixes can look flaky or scene-dependent because the wrong raycaster can still win globally.

Stopping unintended auto-casts first gives the editor a stable selection input again.

## Related docs

- `docs/task/refactor/pointer-system.md`
- `docs/task/refactor/raycast-driven-by-actions.md`

This note is intentionally a refactor doc because it describes the runtime migration step that should happen before the final trigger semantics move into `docs/spec`.
