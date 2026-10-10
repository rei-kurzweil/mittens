# Material descriptor caching for stable and frequently updated inputs

Status: planned; profile before selecting the replacement policy.

2026-10-10: feeds [stable material instances and family contracts](material-instances-and-family-contracts.md).
The target resolves stable handles to retained draw bindings, keeping animated
values out of descriptor cache identity. Hashing can remain for structural
program/pipeline lookup; measure value-key lookup, miss allocations, and eviction
separately rather than treating average O(1) HashMap lookup as a linear search.

## Problem

`VulkanoRenderer::get_or_create_material_set` currently caches immutable material
UBOs and descriptor sets in one 512-entry LRU keyed by material, texture,
filtering, quantization, and every Anime shading parameter bit. This bounds
cache-owned historical resources, but treats stable material identities and
continuously animated inputs as the same workload.

A slider, animation, OSC stream, or other frequent input can therefore allocate
a new uniform buffer and descriptor set for every distinct value. Once the cache
is full, each new key also finds the least-recently-used entry with a linear scan.
Eviction is GPU-safe because submitted command buffers retain `Arc` ownership,
but in-flight resources can outlive their cache entries.

The fixed capacity of 512 is not yet justified by workload measurements. It may
be harmless for mostly static scenes, too small for scenes with many stable
material/texture combinations, and unnecessarily large for a stream of values
that should not be cached by exact bits at all.

## Goal

Separate caching and update strategies according to input lifetime so stable
material state retains useful reuse while frequently changing scalar/vector
inputs use bounded, frame-safe storage without producing an unbounded allocation
rate.

## Investigation

1. Instrument per-frame and high-water values for material cache hits, misses,
   evictions, retained entries, UBO allocations, descriptor allocations, and
   resources retained only by in-flight submissions.
2. Measure at least:
   - a static mixed-material scene;
   - `examples/shading-models.mms` while continuously dragging every control;
   - a runtime animation producing more than 512 distinct parameter records;
   - desktop, stereo XR, and a scene with mirror or additional render views.
3. Attribute keys by stable fields and frequently updated fields. Determine
   whether exact float-bit keys provide useful reuse for each source.
4. Measure CPU time spent looking up and evicting entries, descriptor/UBO
   allocation time, memory high-water mark, and frame-time tails.

## Candidate design

Keep stable descriptor identity—pipeline layout, texture/sampler bindings, and
other infrequently changed resources—in a cache sized from measured scene
cardinality. Route known high-frequency parameter data through a separate
bounded mechanism, such as per-frame/ring-buffered uniform storage with slots
reclaimed only after the associated GPU future completes.

If a second LRU remains useful for quantized or recurring animated states, give
it an independently measured capacity and an `O(1)` recency implementation.
Do not assume that placing continuously unique float values in another LRU will
reduce allocation churn by itself.

The final design must account for descriptor indexing/dynamic offsets supported
by the target Vulkan baseline before choosing between reusable descriptor sets,
dynamic uniform offsets, descriptor pools, or frame-local arenas.

## Acceptance criteria

- Cache/resource ownership is bounded by an explicit policy plus the configured
  number of frames in flight.
- Frequently changing Anime parameters do not allocate one persistent UBO and
  descriptor set per distinct value.
- Stable material/texture combinations retain a measured useful hit rate.
- Lookup and eviction no longer require a linear scan on the hot miss path.
- Texture replacement still invalidates every descriptor that references the
  replaced image, without invalidating unrelated stable entries.
- Desktop, XR, mirror/multi-view, and device-loss/shutdown lifetimes remain safe.
- Before/after measurements and the chosen capacities are recorded here; 512 is
  retained only if the measurements justify it.

## Non-goals

- Redesigning the public MMS shading API.
- Recompiling pipelines for parameter-only changes.
- Folding the complete Materials v2 resource graph into this optimization.

## Relevant code

- `src/engine/graphics/material_cache.rs`
- `src/engine/graphics/vulkano_renderer.rs`
- `src/engine/graphics/pipeline_descriptor_set_layouts.rs`
- `docs/task/epic/renderer_optimisation.md`
- `docs/task/anime-shading-panel-and-live-shader-inputs.md`
