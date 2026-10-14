# Info panel title-bar grab handle without blocking content interaction

Status: planned; whole-panel `Grabbable` experiment reverted after reproducing
the interaction conflict on 2026-09-11.

2026-10-14 implementation update: `info_panel.title` now accepts a string or a
component used as the actual title-bar root. `examples/graphics-settings.mms`
uses the component path with title-only `Grabbable.parent()` and an explicit
Draggable target. Tests verify the grab target is the panel root, Slider
geometry has no Grabbable ancestor, and custom header identity survives
minimize/restore. Its restore listener/selection guard travel with the moving
root during grip reparenting. String-title consumers keep their existing
non-Grabbable default header; the live gesture matrix below remains pending.

## Problem

An `info_panel` must be movable in desktop and XR without preventing interaction
with controls inside its body. Adding `Grabbable {}` to a transform surrounding
the entire accordion makes the panel movable, but its grab participation wins
over descendant pointer gestures: the Anime shading sliders can no longer be
selected or dragged reliably.

This is an input-ownership problem, not a slider problem. The surface eligible
to begin a panel grab overlaps the surfaces that must begin slider drags and
button clicks. Nesting interactive content beneath a grabbable transform is
therefore not sufficient evidence that event priority or descendant hit testing
will preserve the intended gesture.

The current `info_panel` remains non-`Grabbable`. Its title bar already has
`Draggable.target(...)`, which moves the named inner panel root for pointer-drag
interaction. This task adds physical grab behavior without broadening its hit
surface over the content.

## Desired behavior

- Only the visible title bar can initiate attachment-style panel grabbing.
- Slider tracks, slider thumbs, Reset, the accordion toggle, and future body
  controls remain independently clickable/draggable.
- Grabbing the title bar moves the title and body as one rigid panel.
- The body does not need to be a structural child of the grab-hit transform if
  an explicit transform relationship expresses the same motion ownership.
- Desktop pointer dragging and XR attachment-style grabbing coexist without two
  gesture owners responding to one press.
- Minimize, restore, removal during a gesture, and re-created body content remain
  safe.

## Candidate topologies to validate

### 1. Title bar uses `Grabbable.parent()`

Place the grab component on the title-bar transform and have it move the panel
motion root above it. The body remains a sibling beneath that motion root, so it
moves with the title without becoming part of the title's hit surface.

Conceptually:

```mms
T { // panel motion root
    T { // title bar; only this background is a grab hit target
        Grabbable.parent()
        Raycastable.enabled()
    }
    T { // body sibling
        Slider { ... }
    }
}
```

Verify how `Grabbable.parent()` resolves its target when the component is
materialized alongside layout-generated background geometry. It must move the
intended panel transform, not a private layout helper or outer layout slot.

### 2. Adjacent grab handle owns motion; panel follows explicitly

If attachment grabbing requires the grabbed transform itself to be the durable
motion authority, author a title-only grab handle adjacent to the visual/layout
tree. Route the visual panel through `TransformParent.target(handle)` so the
title and body inherit the handle's world transform without putting the body
beneath its interactive/grab hit region.

Conceptually:

```mms
T { // assembly / stable query scope
    T { name = "panel_grab_handle" Grabbable {} /* title-only hit geometry */ }
    TransformParent.target("../#panel_grab_handle") {
        T { // complete info_panel visual and layout tree }
    }
}
```

The exact selector/root syntax and initial-pose ownership must be proven against
the existing `TransformParent` semantics. Avoid a dependency cycle between the
handle, layout slot, and followed panel tree.

Prefer the first topology if it gives precise hit ownership and correct motion;
it is structurally simpler. Use the explicit follower topology only when the
grab system or layout-generated transforms make parent targeting ambiguous.

## Interaction rules

- Do not put a panel-sized invisible raycast/grab surface behind or in front of
  the body. Depth and interaction priority are not substitutes for disjoint hit
  geometry when controls can overlap it.
- The title bar and accordion toggle occupy distinct regions. The toggle must
  keep its click behavior and must not initiate a grab.
- A gesture chooses one owner at its start and keeps that owner until release.
  Moving across the body during a title grab must not activate controls; moving
  across the title during a slider drag must not grab the panel.
- Removing/minimizing content must cancel descendant slider gestures without
  cancelling an unrelated title grab incorrectly.

## Verification

1. Add a focused materialization test proving that the grab component belongs
   only to the title-handle topology, not the panel/body ancestor hit surface.
2. In `examples/shading-models.mms`, drag every slider and click Reset before and
   after moving the panel by its title.
3. In `examples/shading-models-xr.mms`, grab/release the title with each hand,
   then manipulate every slider with controller pointers.
4. Verify the accordion toggle still minimizes/restores without moving the panel.
5. Start a slider drag, cross the title, and release; then start a title grab,
   cross the body, and release. Confirm no ownership transfer mid-gesture.
6. Confirm world pose is preserved on grab start/release and that repeated grabs
   do not accumulate an offset.

## Acceptance criteria

- The info panel is attachment-grabbable exclusively through its title bar.
- All body controls retain their existing click and drag behavior.
- Title, toggle, body, and restored body move together with no visual separation.
- No extra panel-sized raycast geometry competes with descendant controls.
- The implementation works in both desktop and XR examples and has regression
  coverage for hit/gesture ownership and transform targeting.

## Relevant code

- `assets/components/ui/info_panel.mms`
- `assets/components/internal/ui/accordion.mms`
- `assets/components/ui/anime_shading_controls.mms`
- `src/engine/ecs/component/grabbable.rs`
- `src/engine/ecs/component/draggable.rs`
- `src/engine/ecs/component/transform_parent.rs`
- `src/engine/ecs/system/grabbable_system.rs`
- `src/engine/ecs/system/pointer_system.rs`
- `docs/task/style-pointer-events.md`
