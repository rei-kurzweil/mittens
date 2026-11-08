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

## Local cafeteria walking/driving (2026-11-07)

At `2887a7b1` plus test-only profiling support, the ignored local variant
(no orchard, one pickup, Mitten player and Miku vendor) was measured at 1920 per
eye with bloom/emissive extraction and MSAA 4x. Bridge, barracks and washrooms
remain loaded; the washroom mirror remains disabled. Each clean capture uses
240 warmup / 480 samples and verifies actual native locomotion or mounted driving.

[Walking/driving report](local-cafeteria-walking-driving.md): walking **18.238 ms**
mean / **20.395 ms p95** (54.8 FPS estimate), driving **30.537 ms** mean /
**31.891 ms p95** (32.7 FPS estimate). CPU scene update is 5.486 versus 13.718 ms;
GPU graphics is 11.547 versus 15.362 ms. Both reuse graphics primaries without
rebuilds. This is not a vehicle-subsystem isolation or a RoomSystem result.

Earlier captures made while the user's live app was still open were discarded;
only the restarted `clean-r1` captures are reported. Private raw artifacts stay
under ignored `examples/local/benchmarks/`; aggregate results are retained here.
Offscreen timing excludes headset/compositor/ALVR overhead.

### GPU attribution follow-up

[Seven controlled captures](local-cafeteria-gpu-attribution.md) retain 1920/eye,
bloom/emission, MSAA 4x and loaded background rooms. With pass timestamps, natural
walking/driving GPU graphics is 11.529/15.927 ms, almost entirely differentiated
by opaque rendering (9.773/14.187 ms); postprocess stays near 0.56 ms. Driving
with the walking render camera costs 9.223 ms GPU despite CPU update 13.610 ms.

From the identical natural driving route/view, hiding the combined body reduces
GPU graphics to 10.780 ms; hiding the player reduces it to 14.957 ms. Replacing
only pickup paint shading with a temporary plain-color shader retains all draw
counts/geometry and reduces GPU graphics to **11.948 ms** (3.979 ms saved).
Per-frame camera and truck/player poses match exactly. This localizes substantial
cost to material shading in the cab view; it does not isolate individual shader
terms or solve the independent CPU update bottleneck. Production materials are
unchanged. Invalid first avatar/material diagnostic attempts are excluded.

### Pickup Anime material (2026-11-07)

At the user's request, both pickup prefabs now use `pickup_anime_material()`
with `MaterialDefinition.anime()`, character-style shading and rim strength
**1.0 versus character 0.42** (2.38x), with matching rim power 3.5. Shared
character/environment presets are unchanged. Truck paint no longer uses the
custom painted-metal shader. The preceding correction also removed matcap from
the retained metal material used by appliances.

A new 1920/eye, MSAA 4x, bloom/emission-on driving capture (240 warmup/480
samples, pass timestamps) averages **11.739 ms GPU graphics**, **13.586 ms CPU
update**, **26.714 ms total**, **28.767 ms p95** (37.4 FPS estimate). Opaque GPU
is 10.072 ms. Two graphics-primary cache hits and zero rebuilds/frame remain.
Route, mount/power/mirror state and rendered view matrices match the prior
natural-driving capture exactly. This is improved GPU cost, not yet a 45 FPS
result. CPU update remains the separate unresolved bottleneck.

Raw captures and source hashes remain ignored at
`examples/local/benchmarks/cafeteria-1920-driving-anime-rim-2026-11-07.{json,frames.json,log}`
and `anime-rim-manifest-2026-11-07.json`. Offscreen timing still excludes
OpenXR/compositor/ALVR overhead; prior results are retained unchanged.

### Character directional-fill experiment (2026-11-07)

Ship avatar/face presets now share a separate custom Anime fragment definition.
For affecting lights above 25 degrees relative to world-up, a failed `N·L > 0`
gate floors the ramp input at 1.0; the rim remains independent and unchanged.
See [material controls and semantics](../../../how_to/character-directional-fill-material.md).
Global Anime, environment and truck presets are unchanged by this experiment.

Enabling this on skinned characters required custom cached-deformation program
selection/pipelines using the existing cached-skinned vertex ABI. Custom material
parameter storage also needed the standard bounded per-texture binding cache;
the previous flat eight-snapshot pool failed with the avatar's texture count.
No new MMS syntax, host calls, skinning dispatches or joint-update model were added.
Snapshot tests were extracted to a focused source file and extended to cover
more than eight textures and bounded eviction with retained descriptors.

The corrected `walking-character-directional-fill-v3` capture uses the local
scene, 1920/eye, MSAA 4x, bloom/emission, 240 warmup / 480 samples, pass timestamps.
It averages **17.672 ms total**, **19.865 ms p95** (56.6 FPS estimate), **5.465 ms
CPU update**, **10.971 ms GPU graphics** and **0.0458 ms GPU deformation**. All
background rooms remain loaded, mirror disabled, one truck control-off, normal
walking input. Two graphics-primary hits and zero rebuilds/frame remain. The
sampled stereo draw counts match the original walking scene, including all
636 opaque calls / 3,166 opaque instances; there are no custom-material errors.
This is not an isolated shader speedup: the truck material and backend also
changed since the original walking baseline, and GPU/process variation remains.

Two earlier attempts are invalid/excluded: the first lacked a custom geometry
program and omitted skinned draws; v2 failed before sampling on the flat snapshot
limit. Raw v3 summary/frame samples/log, source hashes and validation logs remain
under ignored `examples/local/benchmarks/`. The seven material-system tests, four
material-registry tests and two Vulkan snapshot/static-room tests passed after
the fixes. Offscreen timing excludes live OpenXR/compositor/ALVR overhead.

### Vehicle CPU drill-down after checkpoint `c11264f8` (2026-11-07)

[CPU attribution and temporal report](../../ship-driving-cpu-profile.md) uses
opt-in per-frame update/host/callback timing. Fresh walking/driving CPU update
is 5.529/13.966 ms; native system tick is 5.526/5.744 ms and callback servicing
0.001/8.200 ms. Longer driving logs show callback cost growing with stable world,
query and callback counts. The 120 Hz physics substep cadence adds a separate
one/two-step timing band to the 75 Hz fixture.

A per-callback capture attributes 4.812 ms/frame to the vehicle FrameTick handler
and 3.079 ms/frame to two settings-panel global DataEvent listeners rejecting
VelocityChanged events. A fresh control pair bypassing only those listeners
reduces CPU update **13.972 -> 10.962 ms**, with every pose/view identical.
Production event delivery and vehicle scripts remain unchanged. Native query
work accounts for about 0.7–0.8 ms; remaining VM work and its drift need further
attribution. A vehicle-FrameTick bypass attempt failed motion validation and is
excluded. All valid raw captures remain ignored under examples/local/benchmarks.

### Shared immutable MMS function bodies (2026-11-07)

Following the [CPU attribution](../../ship-driving-cpu-profile.md), userspace
sampling identified repeated AST cloning/dropping. Runtime function bodies now
share immutable code with `Arc<BlockStatement>` instead of copying that code on
every function-value/captured-environment clone. No script behavior, subscription
filtering, vehicle control or material change is part of this optimization.

At 1920/eye, bloom/emission, MSAA 4x, 240 warmup/480 samples, full driving CPU
update falls **13.972 -> 6.575 ms** and callback service **8.154 -> 0.890 ms**.
All 7.058 callbacks/frame, draw counts, truck/player poses and camera matrices
remain. Driving elapsed is **20.164 ms mean / 21.886 ms p95** (49.6 FPS estimate);
walking is **17.845 / 19.867 ms**. GPU graphics is 12.339 ms driving / 11.169 ms
walking; GPU differences include run variation, not a GPU optimization.

A longer 1,200-sample driving capture averages **19.885 ms / 21.444 ms p95**
(50.3 FPS estimate), CPU update 6.567 ms and callback service 0.885 ms. The earlier
callback-time growth is absent across this 16-second synthetic window. Raw
captures remain ignored under examples/local/benchmarks with stems containing
`cpu-shared-function-body` and source/test provenance in its manifest.

The typed-table selector work is explicitly independent and still proposed.
All settings callbacks remain enabled; their combined cost is now 0.117 ms/frame,
so the old 3 ms settings-bypass opportunity must not be counted again. No live
OpenXR/compositor/ALVR or full tracked-ship 45 FPS claim follows from these runs.
