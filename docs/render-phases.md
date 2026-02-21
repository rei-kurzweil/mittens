# Rendering phases (virtual passes) — current implementation

This doc describes cat-engine’s current “render graph” as implemented today.

Key idea: we use **dynamic rendering** (one rendering scope), but we still structure drawing into **virtual phases** by recording draw commands in a specific order with different pipeline states.

Primary references:
- `src/engine/graphics/vulkano_renderer.rs` (`build_draw_batches_command_buffer`)
- `src/engine/graphics/vulkano_cbb.rs` (`VulkanoState::record_*_draws` helpers)
- `src/engine/graphics/visual_world.rs` (draw cache + per-phase instance lists)

---

## Phase overview (actual order)

Within one `begin_rendering`/`end_rendering` scope, the renderer records draws in this order:

1. **Background** (instanced)
   - Draw list: `VisualWorld::background_batches()`
   - Pipeline: `pipeline_*_transparent` (depth write OFF)
   - Use-case: skyboxes / scene dressing that must never occlude the foreground.

2. **Background occluded+lit** (instanced)
   - Draw list: `VisualWorld::background_occluded_lit_batches()`
   - Pipeline: `pipeline_*` (depth write ON)
   - Use-case: “background world” that self-occludes and is lit, but still shouldn’t occlude the main scene.

3. **Clear depth**
   - Depth is cleared after background so the background depth buffer contents cannot occlude foreground geometry.

4. **Opaque** (instanced)
   - Draw list: `VisualWorld::draw_batches()`
   - Pipeline: `pipeline_*` (depth write ON)

5. **Cutout / alpha-to-coverage** (instanced, optional)
   - Draw list: `VisualWorld::cutout_batches()`
   - Pipeline: `pipeline_*_cutout`
   - Enabled by: `TransparentCutoutComponent`.

6. **Transparent single-layer** (instanced)
   - Draw list: `VisualWorld::transparent_single_draw_batches()`
   - Pipeline: `pipeline_*_transparent` (depth write OFF)
   - Fast path for transparency that doesn’t require strict per-layer sorting.

7. **Transparent multi-layer** (sorted, drawn one-by-one)
   - Draw list: `VisualWorld::transparent_multi_draw_batches()`
   - Pipeline: `pipeline_*_transparent` (depth write OFF)
   - Important: instances are **sorted back-to-front per eye** and then drawn **one draw per instance** for correct blending.

---

## How instances get classified into phases

Classification and batching live in `VisualWorld::prepare_draw_cache()`.

Per `VisualInstance` flags:
- `background` and `background_occluded_lit` route an instance into the two background phases.
- `transparent_cutout` routes an instance into the cutout phase.
- Otherwise, transparency is determined conservatively:
  - transparent if `opacity < 0.999` or `color.a < 0.999`
  - if transparent and `multiple_layers == false` → transparent single-layer
  - if transparent and `multiple_layers == true` → transparent multi-layer

Note: texture alpha is not currently used for pass selection.

---

## Component → phase mapping (practical)

This section answers: “what do I attach where to make something draw in a given phase?”

### Background routing

- `BackgroundComponent` (ancestor)
   - Any renderable under a `BackgroundComponent` is treated as **background** (excluded from normal opaque/transparent lists).
   - If the nearest background ancestor was constructed with `with_occlusion_and_lighting()`, the instance is routed to **Background occluded+lit**; otherwise it’s routed to the **plain Background** phase.

Nuance: background is decided by nearest ancestor “wins” logic (background status is inherited).

### Cutout routing (alpha-to-coverage)

- `TransparentCutoutComponent` (ancestor or immediate child override)
   - If enabled for a renderable (directly on the renderable or inherited from ancestors), it routes to the **Cutout** phase.
   - Cutout instances are excluded from opaque/transparent lists.

### Opaque vs transparent routing

Transparency is conservative and based on per-instance values stored in `VisualWorld`:

- `ColorComponent` (ancestor or immediate child override)
   - If effective `rgba.a < 0.999`, the instance is considered transparent.

- `OpacityComponent` (ancestor or immediate child override)
   - If effective `opacity < 0.999`, the instance is considered transparent.
   - `multiple_layers: bool` decides which transparent path is used:
      - `false` → **Transparent single-layer** (instanced, fast)
      - `true` → **Transparent multi-layer** (sorted, drawn one-by-one)

Nuance: `ColorComponent`/`OpacityComponent` are **inherited from ancestors** when not present directly on the renderable (this is used heavily by text and grouped UI-ish trees).

### Which component actually creates a drawable instance?

- `RenderableComponent` is what becomes a `VisualWorld` instance.
- The renderer never walks ECS directly; it only consumes the `VisualWorld` snapshot built by systems.

### Where the inheritance rules live

The “effective” per-instance values (background, cutout, color, opacity, multiple_layers) are computed while syncing ECS → `VisualWorld`, primarily in:

- `src/engine/ecs/system/renderable_system.rs` (inherited background/color/opacity/cutout helpers)
- `src/engine/graphics/visual_world.rs` (phase classification in `prepare_draw_cache`)

---

## Pipeline variants (toon vs skinned)

Most phases have **two pipeline variants**, selected per batch by `MaterialHandle`:
- `MaterialHandle::TOON_MESH` (non-skinned)
- `MaterialHandle::SKINNED_TOON_MESH` (skinned)

Selection + vertex buffer binding happens in `vulkano_cbb.rs`:
- Skinned pipeline expects a second vertex buffer binding for skin attributes (`GpuSkinVertex`).
- If a batch says `SKINNED_TOON_MESH` but the mesh has no skin vertex buffer, that batch is skipped.

---

## Practical tips / gotchas

- If your “background” appears to occlude the main scene, ensure the draw is actually classified as `background`, and remember the renderer clears depth only if `any_background` is present.
- If a skinned mesh renders as invisible:
  - check its `MaterialHandle` is `SKINNED_TOON_MESH`
  - check the uploaded GPU mesh includes skin vertices (glTF import should provide JOINTS/WEIGHTS)
- Transparent multi-layer correctness comes from the one-by-one draw loop; don’t try to re-enable instancing for that phase unless you accept incorrect blending.

---

## Where to change things

- Add/change phases/order/state: `src/engine/graphics/vulkano_renderer.rs` (`build_draw_batches_command_buffer`).
- Add new phase-specific pipelines: `src/engine/graphics/vulkano_renderer.rs` pipeline creation and `VulkanoState` fields.
- Change classification/batching: `src/engine/graphics/visual_world.rs` (`prepare_draw_cache`, `prepare_transparent_multi_draw_cache_for_eye`).
