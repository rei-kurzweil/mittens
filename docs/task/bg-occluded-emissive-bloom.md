# Task: Bloom for occluded+lit background emissives

Date: 2026-07-08

Status: planning only.

This is a `docs/task` note only. No `src/` changes are proposed here yet.

## Goal

Allow emissive objects routed through `BG.with_occlusion_and_lighting()` to participate in bloom, while keeping plain `BG{}` excluded and preserving the current foreground bloom behavior.

The intended effect is opt-in:

1. Plain `BG{}` remains authored-backdrop behavior and does not contribute to bloom.
2. `BG.with_occlusion_and_lighting()` emissive renderables may contribute to bloom.
3. Foreground opaque/cutout geometry should occlude that background-derived bloom.
4. When no eligible background emissives exist, the renderer should pay effectively no extra cost.

## Current behavior

Today the renderer splits background instances away from the normal foreground draw lists:

- `RenderableSystem` marks inherited background state on instances.
- `VisualWorld::prepare_draw_cache()` puts background instances into `background_order` or `background_occluded_lit_order`.
- Those instances are excluded from the normal `draw_order` and `cutout_order`.

Bloom extraction does **not** read bright pixels back from the already-rendered main color target. Instead, the renderer opens a separate emissive extraction pass and redraws only:

- `emissive_draw_batches()` (derived from `draw_order`)
- `emissive_cutout_batches()` (derived from `cutout_order`)

That means emissive objects in background phases can render with emissive materials in the main color pass, but they never enter the bloom source texture.

## Desired behavior

Add a second opt-in emissive source for the background-occluded-lit phase only.

- Eligible source set:
  - instances marked `background_occluded_lit == true`
  - emissive material / emissive intensity active
- Ineligible source set:
  - plain `background == true` without `background_occluded_lit`
  - overlay-only emissives
  - transparent multi-layer background behavior (out of scope for this task)

The background-emissive contribution should go into the same bloom source image as the existing foreground emissive extraction so the rest of the blur/composite pipeline stays unchanged.

## Zero-overhead requirement

This task should not add ongoing overhead when a scene has no eligible background emissives.

Concretely:

1. `VisualWorld` should not build or maintain extra background-emissive batch data unless at least one instance in `background_occluded_lit_order` is emissive.
2. The renderer should not allocate extra bloom targets; it should reuse the existing bloom source / blur targets.
3. The renderer should not open an additional extraction rendering scope unless both are true:
   - bloom is enabled for the active render graph, and
   - at least one eligible background emissive batch exists.
4. The normal no-background-emissive path should remain the current foreground-only extraction path.

“Effectively no extra cost” here means:

- no extra GPU pass when unused
- no extra instance buffer uploads when unused
- no repeated per-frame work beyond a cheap boolean/count check during draw-cache prep and render submission

## Proposed approach

### 1. Track a dedicated emissive subset for `background_occluded_lit`

Extend `VisualWorld` with background-occluded-lit emissive metadata parallel to the existing foreground emissive metadata:

- `background_occluded_lit_emissive_order`
- `background_occluded_lit_emissive_batches`
- a cheap presence/count query such as `has_background_occluded_lit_emissive()`

Important constraint:

- Only build these vectors when `background_occluded_lit_order` contains at least one emissive instance.
- Otherwise leave them empty and avoid any extra derived work.

This keeps the hot path cheap for the common case where `BG.with_occlusion_and_lighting()` is used for non-emissive scenery or not used at all.

### 2. Reuse the existing bloom source target

Do not create a second bloom texture for background content.

Instead:

- keep the current foreground emissive extraction into `bloom_source`
- if background-occluded-lit emissive content exists, append its extraction into the same `bloom_source`
- then run the existing blur/composite stages unchanged

This keeps the post-processing contract simple and avoids adding a second blur/composite path.

### 3. Use depth semantics that preserve foreground occlusion

The background-emissive extraction must be suppressed by foreground opaque/cutout depth where foreground geometry covers the source object.

The intended recording order is:

1. main scene render as today:
   - background
   - background occluded+lit
   - depth clear
   - foreground opaque/cutout/transparent
2. existing foreground emissive extraction into `bloom_source`
3. conditional background-occluded-lit emissive extraction into the same `bloom_source`
4. blur + final composite

For step 3, use the current post-foreground depth attachment so foreground opaque/cutout depth can reject covered background emissives.

Pipeline state for the background-emissive extraction should be:

- emissive fragment output identical to the current emissive extraction shader
- depth compare `LessOrEqual`
- depth write enabled

Reasoning:

- foreground depth already exists and should occlude covered background sources
- depth writes allow multiple background-occluded-lit emissives to self-occlude correctly during extraction
- plain background objects remain excluded by construction

### 4. Keep authoring/API unchanged

Do not add a new component or MMS flag for this task.

The opt-in contract is:

- `Background.with_occlusion_and_lighting()`
- emissive material/intensity on the renderable
- render graph bloom enabled

That is enough to opt a background object into bloom participation.

## Expected implementation shape

### CPU-side classification

- Extend draw-cache prep so `background_occluded_lit_order` can cheaply derive an emissive subset.
- Avoid copying or rebuilding the subset if no emissive background-occluded-lit instances exist.
- Keep plain background and foreground list construction unchanged.

### Renderer-side extraction

- Reuse the existing bloom source image and blur targets.
- Add a conditional extraction step for background-occluded-lit emissive batches.
- Use `Load` on the bloom color attachment when appending after foreground extraction.
- Skip the step entirely if the new subset is empty.

### Pipeline setup

- Add background-emissive extraction pipelines only if the existing prepass pipelines cannot safely express the required depth-write-on behavior.
- Prefer sharing shader modules and as much pipeline setup as possible with the existing emissive prepass path.

## Edge cases

- **Bloom disabled**: no background-emissive extraction state or pass should run.
- **Render graph absent / post-process inactive**: no behavior change.
- **Eligible background emissives but no foreground emissives**: the bloom source should still be produced from the background-emissive extraction alone.
- **Foreground occludes source fully**: no visible bloom should leak through.
- **Multiple eligible background emissives at different depths**: they should self-occlude consistently.
- **Plain `BG{}` emissive sky cards**: continue to render emissive-looking color in main color, but contribute nothing to bloom.

## Test plan

1. Draw-cache classification:
   - emissive instances under `BG.with_occlusion_and_lighting()` appear in the new subset
   - plain `BG{}` emissives do not
   - non-emissive `BG.with_occlusion_and_lighting()` does not create subset work
2. Renderer behavior:
   - eligible background emissive blooms when bloom is enabled
   - the same instance stops blooming when downgraded to plain `BG{}`
   - foreground opaque geometry suppresses covered portions of that bloom
   - two eligible background emissives self-occlude correctly
3. Regression coverage:
   - existing foreground emissive bloom remains unchanged
   - overlay behavior remains unchanged
   - emissive-pass and bloom runtime texture publication still work
4. Overhead checks:
   - no extra extraction render pass when the new subset is empty
   - no extra instance-buffer path for background-emissive extraction when empty

## Relevant files

- `src/engine/graphics/visual_world.rs`
- `src/engine/graphics/vulkano_renderer.rs`
- `src/engine/graphics/vulkano_cbb.rs`
- `src/engine/graphics/post_processing.rs`
- `docs/spec/render-phases.md`

## Related

- [docs/spec/render-phases.md](/home/rei/_/cat-engine/docs/spec/render-phases.md)
- [docs/spec/render-graph-post-processing.md](/home/rei/_/cat-engine/docs/spec/render-graph-post-processing.md)
- [docs/bugs/bloom-stencil-clipping.md](/home/rei/_/cat-engine/docs/bugs/bloom-stencil-clipping.md)
