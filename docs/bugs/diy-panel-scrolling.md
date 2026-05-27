# DIY panel clipping works but scrolling does not

## Status

Open bug / follow-up investigation.

## Symptom

In the focused repro scene, stencil clipping now works correctly, but scroll interaction does not move the container content.

The affected viewport is the yellow `container` content area in the DIY panel repro.

Current state:

- content in the `container` is clipped correctly when `overflow("scroll")` is enabled
- the same repro confirms stencil refs and clip-source routing are now working
- however, scroll input does not visibly scroll the clipped content

## Repro

- [examples/diy-panel.mms](../../examples/diy-panel.mms)
- [examples/diy-panel.rs](../../examples/diy-panel.rs)

Scene notes:

- `diy_panel_demo` contains a `container` node with `overflow("scroll")`
- that `container` is the yellow content area in the panel body
- routed rows and attached-at-runtime rows appear in the container
- clipping works
- scrolling does not

## Expected behavior

When the pointer/wheel/drag scroll path targets the `container`, the content subtree should move within the clipped viewport while the viewport itself remains fixed.

## Actual behavior

The viewport clips correctly, but content remains stationary.

## Why this matters

This means the stencil/clip fix is not sufficient for functional scroll containers:

- visual clipping works
- interaction semantics for scroll containers still fail
- editor panels and MMS layout examples can look correct but remain unusable for overflow content

## Likely investigation targets

- [src/engine/ecs/system/scrolling_system.rs](../../src/engine/ecs/system/scrolling_system.rs)
- [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs)
- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs)
- [src/engine/ecs/system/pointer_system.rs](../../src/engine/ecs/system/pointer_system.rs)
- [src/engine/ecs/system/raycast_system.rs](../../src/engine/ecs/system/raycast_system.rs)

## Questions to answer

- does the generated scroll drag surface receive pointer hits in the repro?
- is wheel input reaching the `ScrollingSystem` for the `container`?
- does scroll state update but fail to affect transform/layout output?
- does the layout-owned scroll helper target the correct subtree after the sibling clip refactor?

## Current context

Recent stencil work fixed a separate bug where clip-source renderables were carrying the wrong stencil ref and deferred overlay rendering could clear stencil state.

This bug remains after those fixes, so it should be investigated as a scrolling/input/layout issue rather than another stencil-visibility issue.
