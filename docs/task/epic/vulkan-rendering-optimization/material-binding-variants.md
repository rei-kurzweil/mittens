# Fixed legacy bands, named Grid inputs and material binding retention

Status: implementation and controlled capacity capture. Prior checkpoint is
`d7e265b8`, which saves the cache counters and view-local mirror optimization.

## Scope and current rendering behavior

The shared Vulkan renderer serves desktop and XR. The previous optimization
reused **scene draw lists**, not recorded command buffers. It avoided repeating
opaque/background/outline/cutout/emission/transmission/single-transparent/overlay
batch and clip-program preparation just to select an eye's reflection image.
Real structural mutations still invalidate those lists. Multi-layer transparency
still sorts per view and command buffers are still freshly recorded.

This slice removes obsolete lighting quantization variants, provides an ordinary
Surface Grid material and fixes a measured material-binding cache working-set
problem. It does not implement explicit synchronization or command-buffer reuse.

## Binding identity and fixed lighting bands

A descriptor set refers to a material-values buffer, texture image view and
sampler. A texture image can be referenced by many bindings and compatible
pipelines without copying its pixels. A cache miss here creates a binding/snapshot
pair, not another copy of the texture image.

The built-in mutable material path previously cached tuples of material program,
texture, filtering and authored quantization bits within each material instance.
The immutable descriptor cache also included quantization bits. Custom materials
use a separate snapshot path; the old 16-entry limit did not describe all custom
material behavior.

`MaterialResourceKey` now identifies program, texture, filtering and, only for
the still-native Grid pipeline, signed spacing. **Ordinary material keys have
no configurable lighting-band value.** Legacy Toon UBOs use three bands. The old
authoring scalar can still deserialize, but changing it on an ordinary surface
does not split batches, change its band count or invalidate its draw list.
Actual material inputs, textures, sampling modes and mutable revisions keep
their existing distinct identities/lifetime semantics.

Native Grid historically repurposes that uniform slot for spacing and coordinate
mode. Its grid-only spacing key is retained until the native runtime migrates;
removing it without that distinction would break grid visuals. Texture replacement
and retirement now match typed binding keys by texture identity.

## Standard Surface Grid definition

[Grid Surface migration tracker](../../grid-surface-material-migration.md) describes
`assets/components/materials/grid_surface.mms`, its `MaterialBase()` definition,
named inputs and `examples/grid-material.mms`. The new shader uses the standard
Surface interface and flat albedo/coverage; it has no lighting-quantization input
or special offscreen-image requirements. Two independently valued materials in
the example share one compiled definition.

The asset is usable on ordinary planes now. Automatically switching native
editor Grid visuals is a separate integration task because their arbitrary-plane
coordinates and dynamic owner transforms must retain their current semantics.
No new material base, legacy fallback or Android renderer feature is introduced.

## Confirmed working-set churn

The software LRU bound was sixteen distinct built-in material resource bindings
per material instance. If a shared environment material repeatedly visits more
variants than the cache retains, earlier bindings are evicted and recreated on
the next cycle, while their texture images remain loaded.

An invocation-local test override compares limits 16 and 32 in the same compiled
renderer, after quantization cleanup and the view-local mirror fix. Conditions:
cafeteria, stationary synthetic XR, 1920 x 1920 per eye, bloom/emission extraction,
MSAA 4x, GTX 1080, 240 warmup frames and 480 measured stereo frames per run.

| Mean count per stereo frame after warmup | Limit 16 | Limit 32 |
| --- | ---: | ---: |
| Built-in mutable binding misses | 36 | **0** |
| Built-in mutable binding evictions | 36 | **0** |
| Mutable UBO/descriptor pair allocations | 36 | **0** |
| Mutable UBO uploads | 36 | **0** |

Thus the allocation churn is eviction-related, rather than evidence that the
texture images are copied or that every pipeline needs a new binding. The
**production default is now 32**, still bounded and allocated on demand.
The eight retained revisions per binding limit is unchanged. Larger future
working sets can still exceed this bound; broader memory-budgeted retention is
not implemented by this constant adjustment.

The test-only `SHIP_PROFILE_MATERIAL_BINDINGS` override permits reproduction of
the old sixteen-entry case. It is not a production setting or MMS feature.
Timing summaries are retained for context, but these separate processes do not
isolate GPU clock variation. The counters directly establish the removed churn;
they do not imply that the whole ship now sustains 45 FPS.

## Validation and next work

- Binding-key tests: band edits produce the same ordinary key; grid spacing,
  texture and filtering changes remain distinct.
- UBO tests: legacy lighting stays at three bands; native Grid signed spacing
  is preserved.
- Draw-stream tests: band edits do not split/invalidate ordinary batches.
- Existing visual-world, material-cache and native Grid tests pass.
- The MMS Grid example resolves to one shared Surface definition with independent
  named input values, and its typed fragment uniform layout compiles.
- GPU Surface Grid test renders both eyes and verifies pixel changes for spacing
  and coordinate-mode updates using the generic custom pipeline.
- GPU snapshot-retention and runtime texture retirement tests pass.

Next: retain safe per-frame/view instance-buffer identities and define pass-level
resource dependencies before attempting retained command buffers. The material
bindings now remain stable for the measured working set, but geometry/instance
updates still produce new first-eye buffers. Native Grid-to-Surface integration
and truck script power/continuous physics remain separately tracked work.

## Capture artifacts and reproduction

- [Sixteen-entry summary](data/material-bindings-16-2026-11-06.json)
- [Sixteen-entry frame counters](data/material-bindings-16-2026-11-06.frames.json)
- [Thirty-two-entry summary](data/material-bindings-32-2026-11-06.json)
- [Thirty-two-entry frame counters](data/material-bindings-32-2026-11-06.frames.json)

Use the `profile_ship_stereo` variables from the counter baseline with
`SHIP_PROFILE_MATERIAL_BINDINGS=16` or `32`. Both controls use identical compiled
code and differ only in that test override. Without the override, the production
and test default is now thirty-two. The new miss/eviction counters aggregate the
built-in per-material binding caches, separately from the immutable cache.

The final [default-policy capture](data/material-bindings-default-2026-11-06.json),
with no test override, confirms limit 32 and zero binding misses, evictions,
material allocations/uploads and structural draw-cache rebuilds across its
480 sampled stereo frames. Production `cargo check --lib` and the final MMS
Grid Surface integration test also passed. New Rust modules pass focused
format checks; local documentation links and `git diff --check` pass.
