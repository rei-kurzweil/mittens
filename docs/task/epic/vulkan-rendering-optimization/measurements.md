# Cafeteria CPU recording measurements

Measured 2026-11-06 on GTX 1080, release build, after the event-binding refactor.
Checkpoint commit: `645885e7`; the event-binding work and instrumentation were
uncommitted during capture. The source extraction moves the per-view command
builder and XR submission into focused files; this slice introduces no resource
reuse, command caching, draw-order or scheduling changes.

## Conditions

- Actual `ship_interior.mms`, cafeteria fixture, stationary synthetic XR views.
- **1920 x 1920 per eye**, bloom including emissive extraction, MSAA 4x.
- 240 warmup frames, 480 measured stereo frames. 16,444 components and
  2,017 visual instances. Zero mirror captures and 960 eye submissions.
- Ordinary test hooks collect GPU timestamps; additional CPU stage hooks run
  only when a test profile exists and compile out of production builds.
- These are offscreen timings, excluding live headset pacing, compositor,
  ALVR and microphone capture. CPU elapsed stages include any scheduling/driver
  stalls inside them. GPU clocks were not locked.

## Whole-frame result

| Stage | Mean ms | p95 ms |
| --- | ---: | ---: |
| Scene update | 4.560 | 5.157 |
| Render preparation | 0.550 | 0.598 |
| Renderer recording/submission | 7.362 | 7.797 |
| Completion wait | 11.420 | 11.755 |
| GPU graphics, both eyes | 13.903 | 14.080 |
| GPU deformation, both eyes | 0.044 | 0.045 |
| Total elapsed frame | 23.908 | 24.643 |

Active CPU work totals **12.473 ms** in this run. GPU execution overlaps CPU
recording; do not add the GPU columns to CPU work. Total elapsed is about
41.8 FPS equivalent, short of the 22.22 ms budget for 45 FPS.
This is a measured post-refactor frame, replacing the earlier 22.55 ms estimate
for this capture. GPU graphics is 13.903 ms here versus 12.370 ms in the earlier
separate-process run; this capture does not isolate a cause for that variation
and must not be presented as proof of a GPU regression from event bindings.

## Renderer CPU breakdown

All stages below are summed across both eyes per frame. They partition the
recording/submission bucket; they are **CPU times**, including the post-process
recording row, not GPU pass timings.

| CPU stage | Mean ms | p95 ms | Share of recording/submission |
| --- | ---: | ---: | ---: |
| `command_buffer_build` | 3.462 | 3.705 | 47.0% |
| `opaque_record` | 1.508 | 1.585 | 20.5% |
| `postprocess_record` | 0.530 | 0.567 | 7.2% |
| `background_record` | 0.524 | 0.565 | 7.1% |
| `queue_submit_flush` | 0.503 | 0.570 | 6.8% |
| `draw_cache_and_transparency_sort` | 0.477 | 0.513 | 6.5% |
| `deformation_record` | 0.136 | 0.153 | 1.8% |
| `instance_buffers` | 0.070 | 0.080 | 1.0% |
| `material_sync` | 0.036 | 0.041 | 0.5% |
| `camera_lights_global_descriptors` | 0.030 | 0.053 | 0.4% |
| `cutout_record` | 0.026 | 0.030 | 0.4% |
| `builder_create` | 0.017 | 0.046 | 0.2% |
| `transmission_transparency_overlay_record` | 0.016 | 0.018 | 0.2% |
| `record_finish` | 0.008 | 0.009 | 0.1% |
| `rig_buffer_descriptor` | 0.007 | 0.019 | 0.1% |
| `xr_target_camera_setup` | 0.002 | 0.003 | 0.0% |
| `attachment_setup` | 0.002 | 0.002 | 0.0% |
| `draw_streams_and_transmission_buffers` | 0.001 | 0.002 | 0.0% |
| `debug_record` | 0.000 | 0.001 | 0.0% |
| `outline_record` | 0.000 | 0.000 | 0.0% |

The mean sum of instrumented stages is **7.356 ms**, leaving
**0.006 ms** of wrapper/timing/cleanup overhead relative to the outer
7.362 ms timer. Raw per-frame records permit independent reconciliation;
stage p95 values are not additive.

### Stage boundaries

- `material_sync`: runtime resource retirement/updates, material sync and mirror
  texture retargeting.
- `draw_cache_and_transparency_sort`: view selection, mesh vertex-count map and
  deformation-range synchronization, `prepare_draw_cache()`, per-view transparent
  sorting, and dirty-state collection. Its cost is **not** proof that the opaque
  draw cache rebuilt; a rebuild counter is a follow-up.
- `draw_streams_and_transmission_buffers`: per-pass stream selection, mirror
  exclusion streams where applicable, transmission/refraction buffer preparation,
  and optional debug-batch preparation.
- `instance_buffers`: opaque/outline/background/cutout/overlay instance-buffer
  cache handling, emissive buffer preparation and optional debug instances.
- `attachment_setup`: rendering attachment state and viewport description.
- `camera_lights_global_descriptors`: camera/light buffers, foreground/background
  and optional refraction global descriptor setup.
- `builder_create`: creating the primary `AutoCommandBufferBuilder`.
- `deformation_record`: initial timestamp hooks and dirty compute recording.
- `rig_buffer_descriptor`: rig descriptor and placeholder lighting setup.
- `background_record`: opening rendering, viewport/scissor, background draws and
  foreground depth clear. Outline, opaque and cutout rows follow individually.
- `transmission_transparency_overlay_record`: refraction setup/copies, possible
  pre-refraction bloom, transmission, transparent draws, early overlays, closing
  rendering and optional texture publication. Empty branches still have overhead.
- `postprocess_record`: remaining emission extraction, blur, deferred overlays,
  final composite and post-process runtime texture publication. For scenes with
  pre-refraction bloom, its earlier work belongs to the preceding row.
- `debug_record`: optional stencil debug rendering/publication.
- `record_finish`: final timestamp recording and marking deformations clean.
- `command_buffer_build`: the final **`cbb.build()`**, including Vulkano's command
  finalization/resource tracking/barrier work. Further sampling is needed to
  identify which functions inside it dominate.
- `xr_target_camera_setup`: output/target selection and camera/view setup outside
  the command-building method.
- `queue_submit_flush`: prior-future chain setup, `then_execute`, and fence/flush.
  This is the complete submission wrapper, not a pure `vkQueueSubmit` timer.

## Findings and next slice

**Final `build()` dominates at 3.462 ms (47.0% of recording/submission).** Opaque
recording follows at 1.508 ms (20.5%). Together these consume 4.970 ms across both
eyes. The capture records 1,484 instrumented geometry draw calls, including 300
emission draws. Fullscreen commands are outside that geometry counter.

Camera/light/global descriptor setup costs only 0.030 ms here. Retaining those
resources alone cannot explain a large win for this scene. Background recording
and post-process recording are about 0.52 and 0.53 ms respectively; those are
also candidates for recording reuse, not evidence that bloom costs 0.53 ms GPU.

The next measurement should sample stacks inside final `build()` and the opaque
recording helper, and count draw-cache rebuilds/cache hits and descriptor misses.
The subsequent [build internals capture](build-internals.md) completes the first
part with temporary phase timers: most inner build time is synchronization
analysis and resource-usage finalization. Opaque-helper sampling and cache/miss
counters remain follow-up work.
Use that evidence to choose a **small stable opaque-group command reuse
prototype**. The persistence design must address buffer/descriptor identities
and view-dependent state before retaining any commands. Do not assume secondary
command buffers eliminate Vulkano finalization overhead without measuring it.

Scene update remains an aggregate 4.560 ms here; this capture specifically splits
the renderer bucket and does not yet assign all scene-system CPU costs. GPU pass
breakdown and parallel-recording experiments also remain follow-up work.

## Artifacts and reproduction

- [Summary JSON](data/cafeteria-1920-bloom-2026-11-06.json)
- [480 raw frame samples](data/cafeteria-1920-bloom-2026-11-06.frames.json)

```sh
SHIP_PROFILE_CASE=full SHIP_PROFILE_ROOM=cafeteria \
SHIP_PROFILE_WIDTH=1920 SHIP_PROFILE_HEIGHT=1920 \
SHIP_PROFILE_WARMUP=240 SHIP_PROFILE_FRAMES=480 \
SHIP_PROFILE_OUTPUT=/tmp/ship-cafeteria-record-stages.json \
cargo test --release --lib profile_ship_stereo -- --ignored --nocapture
```

The fixture writes both the summary and a sibling `.frames.json` file. This
capture passed, including its assertions that the scene's GLTFs had loaded.
Production `cargo check --lib` also passed. `git diff --check` and formatting
checks on the extracted/instrumented modules passed. No new recording reuse or
parallel recording implementation is included in these results.
