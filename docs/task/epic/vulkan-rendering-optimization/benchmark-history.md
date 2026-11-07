# Benchmark history and change ledger

Keep historical captures intact. Each optimization stage gets a report and new
artifact names, identifying its baseline revision, changed behavior, view, render
settings, warmup/sample counts, validation and limitations. An on/off control
within one implementation is preferable where practical. CPU-only, GPU-only and
end-to-end offscreen captures measure different things and are labelled separately.

## Reports in order

| Stage | Change / observation | Record |
| --- | --- | --- |
| Initial ship profiling and resolution/bloom controls | CPU systems, 1712/1920/2144 per-eye comparisons, GPU-only paired bloom | [Original ship report](../../ship-interior-performance-profile.md) |
| Parked pickup prototype | Reduced idle physics/updates, but freezes distant native physics; a known behavior bug | [Original report](../../ship-interior-performance-profile.md), [continuous-physics tracker](../../pickup-script-power-and-continuous-physics.md) |
| MMS event-binding lifecycle | Lazy vehicle bindings and native bypass; CPU update about 4.69 -> 3.92 ms in CPU-only probe | [Original report](../../ship-interior-performance-profile.md) |
| Renderer stage instrumentation | Split command preparation, recording, final build, submission and waits | [CPU recording capture](measurements.md) |
| Vulkano build instrumentation | About 1.99 ms access analysis and 0.385 ms finalization across both eyes | [Build internals](build-internals.md) |
| Cache invalidation counters | Found two texture-driven shared draw-cache rebuilds per stereo frame | [Counter baseline](cache-invalidation-baseline.md) |
| View-local mirror binding | Two structural rebuilds -> zero; instance-buffer allocations 10 -> 7 | [Mirror binding result](view-local-mirror-bindings.md) |
| Material key/retention cleanup | Fixed legacy bands; cache limit 16 -> 32 eliminated 36 material allocations/evictions per frame | [Material result](material-binding-variants.md) |
| Retained upload buffers, first prototype | Per-view/purpose host uploads and invariant rig descriptor reuse; sampled upload allocations 13 -> 0 | [Prototype result and controls](retained-upload-buffers.md) |
| Bloom-only primary reuse | Two hits per frame; CPU saving outweighed by higher GPU/elapsed time | [Rejected boundary and artifacts](reusable-primary-commands.md#first-bloom-only-checkpoint) |
| Complete eligible graphics-primary reuse | Dynamic deformation, retained scene/bloom/composite; CPU recording about 7 -> 1 ms | [Implementation, controls and results](reusable-primary-commands.md) |
| Recorder file split after `cbd83ae2` | Separate deformation/graphics recording; CPU record 0.964 ms, unchanged draws and cache hits | [Cleanup validation](reusable-primary-commands.md#recording-file-cleanup-after-cbd83ae2) |

## Selected stationary whole-frame historical captures

All rows below are the stationary cafeteria, GTX 1080, **1920 x 1920 per eye**,
bloom/emission extraction and MSAA 4x, 240 warmup and 480 sampled frames. These
are sequential separate-process records, not controlled timing deltas. GPU
execution overlaps CPU work; do not add its column to the CPU stages.

| Capture | CPU update | CPU preparation | CPU record/submit | GPU graphics, both eyes | Elapsed frame |
| --- | ---: | ---: | ---: | ---: | ---: |
| [Post-event-binding recording baseline](data/cafeteria-1920-bloom-2026-11-06.json) | 4.560 ms | 0.550 ms | 7.362 ms | 13.903 ms | 23.908 ms |
| [View-local reflection bindings](data/view-bindings-stationary-2026-11-06.json) | 4.384 ms | 0.557 ms | 7.126 ms | 14.055 ms | 23.876 ms |
| [Material cache/key cleanup, default 32](data/material-bindings-default-2026-11-06.json) | 4.443 ms | 0.550 ms | 6.921 ms | 12.505 ms | 22.191 ms |
| [Retained uploads disabled, same prototype](data/retained-uploads-off-2026-11-06.json) | 4.366 ms | 0.540 ms | 6.843 ms | 14.081 ms | 23.629 ms |
| [Retained uploads enabled, first prototype](data/retained-uploads-on-2026-11-06.json) | 4.353 ms | 0.546 ms | 6.815 ms | 13.765 ms | 23.294 ms |
| [Graphics primaries disabled](data/scene-primary-stationary-off-2026-11-06.json) | 4.730 ms | 0.545 ms | 7.056 ms | 13.928 ms | 24.145 ms |
| [Graphics primaries enabled](data/scene-primary-stationary-on-2026-11-06.json) | 4.387 ms | 0.567 ms | 0.999 ms | 12.465 ms | 18.414 ms |
| [Recorder file split](data/scene-primary-file-split-2026-11-06.json) | 4.226 ms | 0.538 ms | 0.964 ms | 13.551 ms | 19.254 ms |

The per-stage reports retain full distributions and raw/projected frame samples.
Separate-process GPU variation means a later row is not automatically proof of
a GPU regression or speedup. Counter changes directly identify eliminated work.
The current target is **22.0 ms or less**, with 22.22 ms the 45 FPS cutoff.
Offscreen captures exclude live OpenXR pacing, compositor and ALVR; validate
promising results on the headset before claiming sustained runtime performance.

Moving-head controls are recorded separately in the [primary-reuse report](reusable-primary-commands.md):
24.306 ms disabled versus 21.549 ms enabled (22.351 ms enabled p95).

## Pickup control power and continuous physics

After `5f477986`, pickups use explicit in-cab control power and always retain
configured native gravity/motion. This changes the workload of historical parked
truck captures. The [native-physics audit](../../native-velocity-sleeping-and-wake.md)
records CPU-only bridge controls and their code changes: 10.523 ms total with
physics active before a CombineMesh invalidation fix, 9.181 ms after that fix,
versus 4.328 ms with deliberately frozen physics. Dashboard mesh uploads fall
from 960 per 480 samples to zero. Truck-state snapshots confirm active physics
and supported orchard pose drift. Native sleeping remains unimplemented.
These CPU-only totals do not establish a new GPU frame time or headset FPS.

## Orchard with active physics (2026-11-07)

At `ae55468f`, [four orchard captures](orchard-active-physics.md) compare
stationary/moving-head and fresh/reused commands at 1920 per eye with bloom,
MSAA 4x, 240 warmup / 480 samples. Reused frames average **23.216 ms stationary**
and **25.150 ms moving-head**, with CPU recording near 1 ms but scene update near
9 ms. All captures keep native truck physics active. This is not the former
frozen-truck cafeteria workload and it does not meet the 45 FPS target.
[RoomSystem](../../room-system-distance-streaming.md) is a design proposal only.
