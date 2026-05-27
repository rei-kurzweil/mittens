# World and inspector panels render through the MMS stencil view instead of their own clip regions

## Status

Open bug / investigation note.

## Symptom

The editor world panel and inspector panel are only visible through the MMS-authored scrolling viewport's stencil-clipped region.

In the stencil debug view:

- the MMS-authored scrolling viewport shows up as red
- the world/inspector panel clip regions show up as green
- panel content is visible through the red MMS viewport instead of only through the panel-local green clip regions

This makes the panels appear as though they are being masked by the wrong clip source.

## Expected behavior

Each panel should be visible through its own stencil clip region.

Specifically:

- the world panel should be clipped by the world panel viewport
- the inspector panel should be clipped by the inspector panel viewport
- the MMS-authored scrolling view should clip only its own content subtree
- unrelated clip subtrees should not mask editor panels

## Current repro

Repro scene and investigation context:

- [examples/ui-layout.mms](../../examples/ui-layout.mms)
- world panel currently left on the opaque path with no overlay wrapper
- issue still reproduces in that configuration

## Why this is a problem

This breaks stencil ownership semantics.

Clip regions are supposed to define local viewport boundaries, but the observed behavior suggests stencil refs or clip-source ordering are crossing subtree boundaries.

As a result:

- editor panels do not respect their own clip topology
- the MMS viewport interferes with unrelated panel rendering
- stencil debug output becomes misleading because the visible routing does not match the apparent clip geometry

## Likely cause

This likely means one of the following is still wrong:

- panel renderables are inheriting the wrong stencil ref
- stencil clip/source assignment is correct in ECS state but wrong in phase-local draw order
- clip-source writes and clipped-content reads are not isolated correctly between render streams or subtrees
- a later pass is sampling/combining panel content in a way that effectively reuses the MMS clip result

## Investigation targets

- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs)
- [src/engine/ecs/system/renderable_system.rs](../../src/engine/ecs/system/renderable_system.rs)
- [src/engine/graphics/visual_world.rs](../../src/engine/graphics/visual_world.rs)
- [src/engine/graphics/vulkano_renderer.rs](../../src/engine/graphics/vulkano_renderer.rs)
- [src/engine/ecs/system/inspector_system.rs](../../src/engine/ecs/system/inspector_system.rs)

Questions to answer:

- which stencil ref do the world-panel and inspector-panel renderables actually carry in the failing frame?
- does the phase-local stencil-clip ordering match the intended subtree ownership?
- are panel clip quads emitted into the same logical stencil space as the MMS-authored viewport when they should be independent?
- does any pass consume panel content after the wrong clip has already been established?

## Likely fix direction

The fix should restore strict subtree-local stencil ownership:

- clip-source registration order must match topology semantics
- each renderable should resolve to the nearest enclosing clip source only
- later render phases must not accidentally reuse stencil state from unrelated clip subtrees

This should be fixed at the renderer / visual-world boundary, not papered over in panel authoring.
