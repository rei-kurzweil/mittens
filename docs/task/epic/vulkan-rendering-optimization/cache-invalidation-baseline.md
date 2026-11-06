# Cache invalidation counters: first implementation slice

Status: saved baseline from the instrumentation-only stage, before the
[view-local mirror binding fix](view-local-mirror-bindings.md). Renderer cache
behavior was unchanged during these baseline captures.
Checkpoint before this slice: `80419d7d`.

## What is measured

Opt-in test profiling records the existing mutation paths that mark draw
structure or instance data dirty. Fixed-size counters compile out of production
builds. Counters distinguish invalidation requests from actual cache rebuilds;
several requests can be consumed by one rebuild. They count writes/request calls,
not necessarily changes in value. An existing setter that marks dirty even when
its value is unchanged continues doing so in this measurement slice.

Per-frame samples include:

- Draw-cache prepare calls, actual rebuilds, hits and contributing setter names.
- Instance-data writes by setter, separately from topology invalidation.
- Both eyes' draw-cache rebuild/data-dirty flags and six buffer refresh decisions.
- Successful instance-buffer helper allocations and instance records written.
- Existing immutable material-cache hit/miss counters and mutable material
  snapshot allocation/upload/reuse counters, as frame deltas.
- The rendered first-eye view matrix, with an assertion that the moving-head
  workload changes that matrix across the sample window.

Counters are diagnostic: they are not new public dirty-generation or cache APIs.
The mutable material allocation counter represents UBO/descriptor snapshot pairs;
it is not a count of every renderer descriptor allocation. Instance-buffer helper
counts exclude unrelated camera/light, deformation and post-process buffers.

## Capture conditions

Actual cafeteria scene, GTX 1080, 1920 x 1920 **per eye**, bloom including emission
extraction, MSAA 4x, 240 warmup frames and 480 measured stereo frames per run.
Compare stationary synthetic XR views with sinusoidal horizontal head movement
of amplitude 0.35 m. Scene avatars and native systems keep updating in both runs.
The moving run verifies the rendered camera changes, not only the injected pose.

This scene still uses the existing truck proximity-physics parking prototype.
The [truck tracker](../../pickup-script-power-and-continuous-physics.md) records
why that needs changing; no vehicle physics behavior changed for this slice.
These are offscreen probes, with no headset pacing/compositor/ALVR. Separate-run
GPU times are context, not a controlled performance comparison between motions.

## Results

Mean counts per stereo frame; the principal counts below are constant across
all sampled frames in both runs.

| Counter | Stationary | Moving head |
| --- | ---: | ---: |
| Draw-cache prepare calls | 2 | 2 |
| Actual draw-cache rebuilds | **2** | **2** |
| Cache hits | **0** | **0** |
| Texture-update invalidation requests | 3 | 3 |
| Rebuilds with texture-update cause | 2 | 2 |
| Model update writes | 109.4 | 109.4 |
| Instance-buffer helper allocations | **10** | **10** |
| Instance records written by those helpers | 4,334 | 4,334 |
| Immutable material cache misses | 0 | 0 |
| Mutable material snapshot allocations | **36** | **36** |
| Mutable material snapshot uploads | 36 | 36 |

Model write count varies across frames (median/p95 127); it is not a claim that
109.4 transforms actually differ each frame. Mutable snapshot reuse also occurs;
see raw summaries for its count, rather than treating all material bindings as
misses.

The per-eye samples show:

| Eye | Draw cache rebuilt | Instance data dirty | Buffer refresh requests |
| --- | --- | --- | --- |
| First | true | true | all six flags true |
| Second | true | false | all six flags true |

Refresh flags are ordered opaque, outline, plain background, lit/occluded
background, cutout and overlay. Refreshing an optional empty pass can allocate
nothing. The successful allocation counter is the actual helper allocation
count, not the sum of those flags.

## Concrete next targets

### Separate view-specific mirror bindings from shared draw topology

All measured rebuilds are attributed to `update_texture`. Source inspection
identifies `retarget_mirror_surface_textures_for_render_view()` before each
view's draw-cache preparation: it writes the appropriate reflection texture into
the shared visual instance. `update_texture()` unconditionally marks the draw
cache dirty, including when assigning the same handle. Texture-system updates
also call this setter; add caller-level attribution if needed to distinguish all
three requests directly.

This means even a frozen mirror can participate in per-view binding changes
without capturing a new image. The second eye then refreshes buffers because
`draw_cache_rebuilt` is true despite instance data no longer being dirty.

Proposed focused follow-up:

1. Add an equality fast path for genuinely identical texture assignments, with
   tests for actual handle changes. Measure it separately; it cannot eliminate
   legitimate left/right texture differences by itself.
2. Design view-local resolved texture bindings, or compatible per-view cached
   streams, so switching an eye's reflection image does not repeatedly mutate
   shared scene topology. Keep the correct eye's frozen image and batch keys.
3. Separate topology/instance-buffer invalidation from binding-only changes
   where the recorded interface permits. Preserve mirror exclusion and runtime
   texture replacement/retirement.
4. Measure cache hits and second-eye buffer allocations before extending command
   reuse. Do not suppress the dirty flag while using a stale descriptor/batch.

### Attribute mutable material snapshot churn

The existing material counters show 36 UBO/descriptor snapshot allocations and
uploads every sampled frame, despite many retained snapshot hits. This does not
yet identify which material IDs/revisions/textures cause them or prove a leak.
Add per-source/reason tracing in a diagnostic capture: revision/definition changes,
resource identity, snapshot retention and eviction/reuse decisions. Choose any
resource-persistence fix from that evidence. Immutable descriptor cache misses
are zero in these captures.

### Continue pass planning and synchronization work

The current 1.990 ms synchronization-analysis finding still applies as a separate
recording target. This slice gives a specific shared-state invalidation issue to
address before assuming stable commands can be retained. Follow the
[dependency plan](pass-dependencies-and-synchronization.md) and measure the
primary-analysis cost for any retained secondary prototype.

## Source organization and validation

Before adding hooks, extracted retained draw preparation and instance mutation
methods from oversized `visual_world.rs` into focused `visual_world/draw_cache.rs`
and `instance_updates.rs`. Extracted instance-buffer helpers from the oversized
renderer into `vulkano_instance_buffers.rs`. Calls remain direct; no production
per-draw maps, allocations or runtime dispatch were added by these splits.

- Release visual-world suite: **18 passed**, including current clipping stream
  tests and two new diagnostic-causality tests.
- Diagnostic tests verify model updates hit the structural cache, repeated color
  invalidations coalesce into one rebuild, and resetting measurements preserves
  pending rebuild attribution.
- Production `cargo check --lib` passed.
- Both final GPU captures passed, including GLTF-load assertions and the
  moving-head rendered-camera assertion.
- Formatting, local documentation links and `git diff --check` passed.

## Reproduction and artifacts

Use the existing `profile_ship_stereo` fixture variables from
[the baseline](measurements.md), adding `SHIP_PROFILE_MOTION=stationary` or `head`.
The default is stationary. Summary `cache_counts` uses **count** units with
`mean`/`median`/`p95`, while `timing` and `cpu_record_stages_ms` retain milliseconds.
The full local `.frames.json` also stores view decisions and first-eye matrices.

Retained artifacts contain the summary and projected per-frame counter/view data
for each run. The projection omits repeated draw histograms and timing details
already summarized; it does not average or discard individual counter samples.

- [Stationary summary](data/cache-stationary-2026-11-06.json)
- [Stationary raw counter/view samples](data/cache-stationary-2026-11-06.frames.json)
- [Moving-head summary](data/cache-head-2026-11-06.json)
- [Moving-head raw counter/view samples](data/cache-head-2026-11-06.frames.json)
