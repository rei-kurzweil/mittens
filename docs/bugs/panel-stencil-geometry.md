# World and inspector panel stencil regions have incorrect geometry

## Status

Open bug / investigation note.

## Symptom

The stencil debug view shows panel clip regions with shapes that do not match the actual panel layouts.

Observed shapes:

- world panel clip region (green in stencil debug) is very wide, even though the panel content is tall and relatively narrow
- inspector panel clip region (also green in stencil debug) appears as a massive rectangle that is slightly taller than it is wide

These debug shapes do not correspond to the expected panel viewport rectangles.

## Expected behavior

The debug stencil shapes for both panels should closely match the visible panel viewport bounds.

Specifically:

- the world panel clip should be a tall, narrow rectangle aligned with the world panel content area
- the inspector panel clip should match the inspector content area rather than a large scene-sized rectangle
- clip debug geometry should reflect the same transforms and dimensions that actual stencil clipping uses

## Current repro

Repro scene / investigation setup:

- [examples/ui-layout.mms](../../examples/ui-layout.mms)
- stencil debug displayed via `render_graph.stencil_clip.debug`
- world panel currently remains opaque and not wrapped in overlay

The issue persists under that controlled experiment.

## Why this is a problem

Wrong clip geometry means either:

- the panel clip quad itself is sized/transformed incorrectly
- the wrong renderable is being registered as the clip source
- debug visualization is showing stale or mismapped stencil metadata

Any of those would make the panel clipping system untrustworthy and complicate further debugging of scroll/viewports.

## Likely cause

Possible failure modes include:

- the layout-managed `__bg` clip helper for panel content has the wrong transform or inherited scale
- world and inspector panels are registering a helper/renderable other than the intended content viewport as the stencil source
- panel clip geometry is being measured from the wrong layout item or from pre-layout dimensions
- debug batching is visualizing clip-source instances whose bounds no longer match the current ECS/layout state

## Investigation targets

- [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs)
- [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs)
- [src/engine/ecs/system/inspector_system.rs](../../src/engine/ecs/system/inspector_system.rs)
- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs)
- [src/engine/graphics/visual_world.rs](../../src/engine/graphics/visual_world.rs)
- [src/engine/graphics/vulkano_renderer.rs](../../src/engine/graphics/vulkano_renderer.rs)

Questions to answer:

- which concrete renderable handle is acting as the clip source for each panel?
- what transform/scale does that handle have at render time?
- does the panel content-slot `__bg` helper match the authored panel width/height after layout?
- is the debug view drawing the clip-source mesh with the same transform used in the stencil pass?

## Likely fix direction

The fix should make the panel clip source explicit and auditable:

- ensure the intended content-slot helper quad is the registered stencil source
- ensure its transform is updated after layout and before render extraction
- ensure debug visualization reads the same resolved geometry used by the stencil pass

If multiple helper quads exist for a panel subtree, the renderer should not guess which one is the clip viewport.
