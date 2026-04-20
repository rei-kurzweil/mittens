# Stencil Clip Sync Performance Analysis

## Status

Working note for incremental cleanup.

This area likely needs several passes rather than a single refactor.

## Goal

Reduce broad world scans and repeated clip/renderable matching work in stencil clip synchronization while preserving the current stencil semantics:

- clip source stores parent-depth ref
- clipped content stores inside-depth ref
- `is_stencil_clip` distinguishes clip-source interpretation from normal clipped-content interpretation

## Current progress

Recent correctness fixes:

- clip-source refs no longer use the clipped-content depth by mistake
- renderable stencil sync preserves clip-source refs instead of overwriting them with normal content refs
- `resync_stencil_state()` no longer clears every renderable in the world after renderable flush
- post-flush resync is now scoped to active clip subtrees instead of a full renderable sweep

Relevant code:

- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L894-L1008)
- [src/engine/graphics/visual_world.rs](../../src/engine/graphics/visual_world.rs)

## Remaining hotspots

### 1. Global scan for all stencil clips

Current code still gathers all clip components via `world.all_components()` and a type filter:

- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L948-L960)

That is much smaller than sweeping all renderables, but it is still a whole-world scan.

### 2. Clip lookup per renderable sync

`sync_renderable_stencil_ref()` currently uses `stencil_clip_for_renderable_component()` to determine whether a renderable is itself a clip source.

That helper currently scans all `StencilClipComponent`s and compares their resolved renderable target:

- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L894-L926)

This is correct but not cheap, especially when syncing many renderables in a subtree.

### 3. Repeated ancestor walks for depth resolution

Both of these do parent-chain walks:

- `stencil_ref_for_renderable()`
- `stencil_ref_for_clip()`

See:

- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L970-L1008)

These walks are probably acceptable at current scale, but they are still repeated work.

### 4. Repeated renderable discovery for clip helpers

Clip helpers resolve through:

- `find_stencil_clip_renderable_component()`
- `find_stencil_clip_renderable_handle()`

See:

- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L1152-L1184)

This is simple and robust, but it recomputes relationships instead of using cached ownership/index data.

## Why the current state is still acceptable

The recent change removed the worst-case broad reset of:

- unregister clip on every renderable
- reset every renderable's stencil ref to zero
- rebuild clip state from scratch for the whole world

That was the most obviously expensive path.

The remaining scans are smaller and more localized, so the system is in a better intermediate state now.

## Likely future passes

### Pass 1 — index clip components

Add a lightweight `SystemWorld` index for active `StencilClipComponent`s so resync does not start from `world.all_components()`.

Potential shape:

- set of active clip component ids
- maintained on register/unregister/remove

### Pass 2 — map clip component -> renderable component

Cache the resolved clip renderable for layout-owned and normal clip components.

Potential shape:

- `HashMap<ComponentId, ComponentId>` for clip component to renderable component
- invalidated only when relevant topology changes

### Pass 3 — map renderable component -> owning clip component

Avoid scanning all clips from `stencil_clip_for_renderable_component()`.

Potential shape:

- reverse map for renderable component to clip component
- especially valuable for layout-owned sibling clip helpers

### Pass 4 — precompute clip depth / scope metadata

If ancestor walks become hot, cache per-scope or per-component clip depth metadata.

This should come after the simpler indexing wins.

## Constraints / correctness notes

Any optimization must preserve these semantics:

- clip-source renderables use parent-depth ref
- clipped descendants use inside-depth ref
- layout-owned sibling clip helpers still resolve to the paired sibling `__bg`
- subtree-local ownership must remain correct; avoid reintroducing cross-subtree leakage

## Open questions

- Should clip indexing live in `SystemWorld`, `RenderableSystem`, or a dedicated clip sync helper?
- Are topology changes frequent enough that caching needs explicit invalidation hooks?
- Would a reverse map from renderable -> clip owner also help debug tooling and tests?
- Should `VisualWorld` eventually carry an explicit clip-owner handle instead of only `stencil_ref` + `is_stencil_clip`?

## Next step

Start with the cheapest structural improvement:

1. maintain an active clip-component index
2. replace full `all_components()` scans for stencil clips
3. measure whether the reverse renderable->clip lookup is still worth doing afterward
