# Task: size a Grabbable for comfortable holding

Status: planned. Design only; no grab scale behavior is implemented by the Rei(mu) panel work.

## Performer workflow

During an XR recording, a large world-space control panel should approach the
hand and become a readable, tablet-sized object. It must stop obscuring the
performer and the mirror while held. On an ordinary release, it should smoothly
return to its authored world size. The first fixture is the three-slider
Rei(mu) mouth-response panel in `examples/rei(mu).mms`.

## Proposed authoring surface

Prefer an optional builder on `Grabbable`, for example
`Grabbable.held_size_axis("x", 0.20) {}`. The name and exact syntax need an API
review. The value is a positive world-space metre length for one selected axis
of the grabbed rigid subtree's visible bounds, not a raw transform scale or a
screen-space pixel count. A 0.20 m width is a starting tablet-sized fixture;
validate it with the actual panel and hands before treating it as a default.
Unconfigured `Grabbable {}` keeps its present behavior.

At grab start, measure the selected subtree bounds and compute one uniform
scale multiplier from `target_length / current_axis_length`. Apply that
multiplier relative to the object's captured world scale, preserving authored
aspect ratio and existing nonuniform proportions. Define the measured subtree
so incidental pointer/interaction geometry and generated bounds do not enlarge
the target. An empty, zero-sized, or nonfinite axis must reject the resize
cleanly and still allow an ordinary grab.

The scale should ease toward the held size during the same approach to the hand
that already smooths grab placement. Placement clearance should use the current
scaled bounds each frame so the panel does not pass through the hand or jump as
its size changes. Capture the pregrab world transform and size before
reparenting; parent scale must not change the intended world-space target.
Releasing outside an attachment zone restores that captured size smoothly in
world space while the object retains a coherent released pose. Regrabbing
during restoration starts from its current visible size and reaches the same
held target without an abrupt scale change.

Keep grab motion and resize in one active-grab lifetime. The scale transition
must be cancellable on deletion, failed grab, pointer loss, or scene reload.
Serialization should retain the authored held-size policy, never a transient
half-scaled grab state. Do not resize the avatar, its hand, or a shared asset.

## Interaction and dependencies

The panel's slider gestures must remain independently usable. The existing
[title-bar grab-handle task](info-panel-title-bar-grab-handle.md) must establish
the panel's physical grab surface before using this policy on it; a
panel-wide `Grabbable` already conflicts with slider input. Hand placement and
clearance belong with [bounds-aware grab placement](grab-hand-relative-bounds-placement.md).
Release into a body mount point has a different scale lifetime and is specified
in [held-item body attachment](held-item-body-attachment.md).

## Acceptance

1. A configured panel approaches either XR hand while shrinking smoothly to
   the authored world-space width; its whole visible body, including sliders,
   preserves its layout and aspect ratio.
2. A normal release smoothly restores the panel's captured size. Repeated
   grabs and releases do not accumulate scale or move it unexpectedly.
3. Parent transforms with nonunit or nonuniform scale still yield the requested
   world-space width, and placement clearance follows the changing bounds.
4. Slider drag, accordion toggle, and title-bar grab choose one gesture owner
   each; none activates another while crossing its hit area.
5. Unconfigured, invalid-bounds, interrupted, and removed-object cases retain
   safe existing grab/release behavior.
