# Ship interior performance investigation

Date: 2026-11-06. Scene checkpoint: `9cdab2d2`.

The current measurements identify vehicle-related CPU work as the largest
removable simulation cost. The initial investigation adds standalone profiling probes. The activation
follow-up below changes the ship MMS; engine behavior, editor state and material
defaults are unchanged.

## Resolution comparison

Full scene with bloom enabled, GTX 1080, MSAA 4x. Resolution is **per eye**;
all timings are average milliseconds per stereo frame.

| View | Per-eye resolution | CPU update | GPU graphics, both eyes | Total elapsed frame | Sampled frames |
| --- | --- | ---: | ---: | ---: | ---: |
| Bridge | 1712 x 1712 | 13.577 | 10.248 | 30.172 | 120 |
| Bridge | 2144 x 2144 | 14.152 | 11.189 | 31.876 | 480 |
| Cafeteria | 1712 x 1712 | 13.481 | 10.836 | 30.594 | 120 |
| Cafeteria | 2144 x 2144 | 13.889 | 16.781 | 37.180 | 480 |

The 1712 runs use 120 warmup frames; the 2144 confirmation runs use 240.
These are separate process captures with different sample lengths, not a
matched-duration resolution sweep. GPU execution overlaps CPU recording and
waiting; do not add the CPU and GPU columns. Total excludes ALVR encoding,
transport and live headset pacing. Detailed controls and variability follow below.

## Method

The CPU probe loads `examples/ship_interior.mms` and its actual imported modules,
models, collision and materials. A CPU uploader publishes real render bindings
without GPU allocation. A stationary synthetic tracked head exercises the XR
avatar in the bridge at 75 simulated frames/s. A disabled oscillator replaces
the microphone. Four preparation frames and 240 warmup frames precede 480
samples. No weapons are fired and no damage jobs are submitted.

CPU results exclude live input/audio capture, GPU drawing, OpenXR pacing,
stream encoding and ALVR transport. They are workload comparisons, not headset
FPS. The GPU probe renders two offscreen eyes sequentially, with timestamps,
using the actual scene and MSAA 4x. Its default target is **1712 x 1712 per eye**,
following the user's headset target; width and height can be overridden to
match the runtime's reported swapchain extent. It does not establish which
resolution a particular ALVR preset chooses.

Host: Intel Core i5-10400, NVIDIA GTX 1080, Rust 1.98.1 release builds.
Cases run sequentially without concurrent compilation or other probes. The
first exploratory GPU runs used 1440 x 1600 per eye; those are superseded by
the corrected-resolution comparisons recorded below.

## Verified local resolution after the captures

Local ALVR `session.json` configures both the stream and emulated headset at
2144 x 2144 per eye. The Focus 3 physical panels are 2448 x 2448 per eye.
SteamVR applies another render scale: the latest Mittens OpenXR log
(`xrclient_mittens-engine.txt`, 2026-11-05 23:32:58) reports
`recommendedImageRect: 1892 1892`, which the engine uses for its swapchain.
The 1712 x 1712 captures below were an assumed comparison target, **not**
verification of ALVR's active application resolution. For a matching repeat
use `SHIP_PROFILE_WIDTH=1892 SHIP_PROFILE_HEIGHT=1892`, or the newly reported
runtime size if SteamVR scaling changes. CPU-only findings are unaffected.

## CPU comparisons

Average milliseconds per sampled frame. Controls are independent; differences
are not additive. The full result is the confirmation run.

| Case | Update | CPU preparation | Total | Mesh uploads/frame |
| --- | ---: | ---: | ---: | ---: |
| full | 12.766 | 1.032 | 13.798 | 2.00 |
| no_input_controller | 9.065 | 0.996 | 10.061 | 2.00 |
| parked_vehicles | 7.711 | 0.526 | 8.237 | 0.00 |
| no_callbacks | 8.722 | 1.055 | 9.778 | 2.00 |
| no_contacts | 9.439 | 1.345 | 10.785 | 3.14 |
| no_springs | 11.278 | 1.005 | 12.282 | 2.00 |
| no_avatar | 12.149 | 1.027 | 13.176 | 2.00 |
| no_vehicles | 4.687 | 0.271 | 4.959 | 0.00 |
| no_destroyables | 12.590 | 0.990 | 13.580 | 2.00 |

The full scene contains 16,432 components, 2,017 visual instances, three
pickups, the vendor Lumae and the XR Mitten avatar. Tree accordions remain
commented out as authored. The no-vehicle control removes three pickup roots
after materialization, retaining the surrounding rooms/vendor. Closing MMS
retains native objects and velocities but freezes scripted behavior. Disabling
contacts lets objects fall and therefore changes their poses; use that control
only as evidence about contact-related work, not a like-for-like rendering view.
The parked-vehicle control disables pickup gravity and clears linear velocity
before warmup; it is diagnostic, not a proposal to remove vehicle gravity.

## Confirmed CPU findings

1. The two orchard dashboard batches are re-baked and uploaded every frame.
   Upload traces identify `pickup_dashboard_gauge_mesh`, each with 963 vertices
   and 1,728 indices (576 triangles). Both are `CombineMesh.keep_transforms()`.
   The cache fingerprint hashes each source's **world** matrix, so movement of
   the whole truck invalidates an otherwise rigid cluster. The parked-vehicle
   control eliminates these uploads; the hotdog truck's settled cluster stays
   cached.
2. Omitting `ship_input_controller()` saves several milliseconds while keeping
   the trucks. Its FrameTick handler performs a global `query_all("#pickup_truck")`
   and a patrol lookup, even for unmounted trucks with no patrol instantiated.
   Native MMQ evaluation traverses the live component tree and allocates child
   lists. The control removes the input controller's other event handlers too,
   but they receive no synthetic input events in this probe.
3. Moving vehicles cause additional transform, contact and render-maintenance
   work. Freezing pickup gravity roughly halves the baseline update cost while
   retaining the same scene geometry. Removing all three trucks reduces the
   combined update/preparation workload to about 5 ms. Exact causes within the
   remaining native vehicle work need a more focused follow-up.
4. Removing destruction markers is a much smaller change than removing the
   trucks. There are no active CSG cuts in these runs; CSG worker time cannot
   explain this idle-scene workload.

A separate Linux perf capture (99 Hz, 1,549 samples, zero lost samples)
corroborates transform propagation, MMQ traversal, CombineMesh reconciliation,
light updates, mirror-request bookkeeping and triangle BVH building as active
CPU paths. Collection starts after a six-second delay to exclude startup and
warmup. These are sampled symbols, not isolated per-system timing estimates.
The native/system controls above provide the stronger evidence for priorities.

## Stereo GPU comparisons

GTX 1080, **1712 x 1712 per eye**, MSAA 4x, 120 warmup frames and 120
sampled frames per process. Averages in milliseconds:

| View / control | Update | CPU preparation | Record / submit | GPU graphics (both eyes) | Total, including waits |
| --- | ---: | ---: | ---: | ---: | ---: |
| bridge / full | 13.577 | 1.408 | 7.426 | 10.248 | 30.172 |
| bridge / no_bloom | 13.244 | 1.372 | 6.292 | 7.783 | 26.568 |
| bridge / avatar_hidden | 13.234 | 1.334 | 7.363 | 8.297 | 27.716 |
| bridge / environment_hidden | 12.479 | 1.317 | 1.211 | 0.741 | 15.488 |
| cafeteria / full | 13.481 | 1.358 | 7.466 | 10.836 | 30.594 |
| cafeteria / no_bloom | 13.559 | 1.392 | 6.356 | 9.560 | 28.766 |

GPU graphics is the sum of the timestamped drawing intervals for both eyes.
It overlaps CPU command recording and waiting; do **not** add this column to
the CPU columns. Total is measured elapsed time for the offscreen frame, not
a predicted ALVR FPS. These initial single-process comparisons have normal
clock/thermal variation; repeat an affected pair before judging a small change.

The bridge's representative stereo frame records 1,484 instrumented geometry
draw calls: 790 opaque/other, 380 surface batches, 300 emission, 12 cutout and
2 single-transparent calls. This excludes uninstrumented background/fullscreen
commands. CPU command recording/submission is approximately 7.4 ms.

GPU deformation averages approximately 0.05 ms; it is a small part of the
measured frame. Immutable morph deltas upload zero bytes after warmup. The
retained deformation path is doing active avatar work, but its shader duration
is not the main GPU expense in this view. Hiding the player avatar's graphics
reduces bridge graphics time from 10.25 to 8.30 ms while retaining animation
and deformation. Hiding environment geometry reduces it to 0.74 ms and command
recording to 1.21 ms, while CPU update remains expensive.

Bloom is measurable: disabling it reduces total elapsed time by roughly
1.8–3.6 ms in these captures, but leaves the large vehicle CPU costs.
All six corrected-resolution runs report **zero mirror captures**. A frozen
mirror still has CPU system bookkeeping; this result specifically excludes
mirror render passes as the source of these timings.


## 2144 x 2144 per-eye follow-up

User-requested higher-resolution benchmark, GTX 1080, MSAA 4x. Five
confirmation processes run sequentially, each with 240 warmup frames and 480
sampled frames. Averages in milliseconds:

| View / control | Update | CPU preparation | Record / submit | GPU graphics (both eyes) | Total, including waits |
| --- | ---: | ---: | ---: | ---: | ---: |
| bridge / full | 14.152 | 1.401 | 7.680 | 11.189 | 31.876 |
| bridge / no_bloom | 13.432 | 1.366 | 6.206 | 10.583 | 29.514 |
| bridge / avatar_hidden | 13.727 | 1.395 | 7.529 | 11.281 | 31.404 |
| cafeteria / full | 13.889 | 1.384 | 7.617 | 16.781 | 37.180 |
| cafeteria / no_bloom | 13.880 | 1.415 | 6.426 | 15.814 | 35.508 |

The elapsed frame totals include waits and exclude ALVR encoding/transport and
live headset pacing. GPU drawing overlaps CPU work; do not add the GPU column
to CPU times. All runs submit two eyes and perform zero mirror captures.
These use the existing resolution overrides; no scene or runtime settings changed.

Short 120-frame exploratory captures preceded confirmation. Cafeteria full
measured 14.584 ms GPU initially versus 16.781 ms in confirmation; its no-bloom
control measured 17.426 versus 15.814 ms. Frames within each process were stable,
but cross-process differences are large enough that small deltas need caution.
At this resolution the bridge avatar-hidden control averages 11.281 ms GPU
versus 11.189 ms full, so the earlier 1712-resolution avatar saving should not
be generalized. The longer bloom controls reduce elapsed frame time by about
2.36 ms in the bridge and 1.67 ms in the cafeteria; they leave the expensive
CPU update intact.

[2144-resolution machine-readable results](../analysis/ship-interior-profile-2144-2026-11-06.json)
include the full confirmation summaries and exploratory timing means.

### Setting a custom ALVR size

ALVR 20.14.1 supports `Absolute { width, height }` for both resolution settings.
`Emulated headset view resolution` sets the per-eye default reported to
SteamVR for application rendering; `Transcoding view resolution` sets the
encoded/decoded stream size separately. For a square 1920 target, choose
Absolute, set width 1920 and explicitly enable/set height 1920. Restart SteamVR
after changing either resolution setting. [Versioned ALVR settings](https://github.com/alvr-org/ALVR/blob/v20.14.1/alvr/session/src/settings.rs#L712-L721).

SteamVR applies its own global/per-application render scale. Select custom/manual
resolution there and check the displayed per-eye dimensions; verify the final
OpenXR recommended size after restart. Setting ALVR's base to 1920 does not
guarantee that applications receive 1920 if SteamVR scaling remains active.
The existing local configuration demonstrates this distinction: ALVR 2144,
latest Mittens OpenXR recommendation 1892. No ALVR or SteamVR configuration was
modified for this investigation.

## Truck activation follow-up

The ship now opts all three trucks into `pickup_activity.mms`. Native zone
enter/exit events update a per-truck state object. The inner sensor wakes it;
the outer sensor retains activity through a deadband. A mounted driver or NPC
throttle command pins activity. Distant trucks initially settle under gravity
and park only after `Velocity.grounded()` reports support. Parking turns gravity
off, resets velocity and deletes the short component-owned support-check
animation. Sleeping trucks have no MMS polling callback. Sensors, collision,
geometry and destruction remain available.

This uses existing MMS/host APIs. The reusable pickup keeps its ordinary physics
behavior unless its fourth `proximity_activation` argument is true. The ship
passes true for its orchard and hotdog pickups. Parked trucks hold position
until woken, including if supporting geometry is removed while they sleep;
general physics sleeping/wake-on-support-change remains future engine work.

`ship_input_controller` receives the driver from mount events. Unoccupied
FrameTick skips truck/patrol searches. Mounted frames still validate the driver
against live trucks before calling it; removing a mounted truck clears that
reference safely. Only explicit `patrol_enabled=true` enables patrol lookup.
XR analog axes skip the weapon searches that cannot consume those events.

CPU probe, 480 sampled frames, same stationary bridge fixture:

| Case | CPU update | CPU preparation | Combined CPU | Mesh uploads/frame |
| --- | ---: | ---: | ---: | ---: |
| Before activation | 12.766 | 1.032 | 13.798 | 2 |
| After activation | 4.685 | 0.560 | 5.244 | 0 |

A prior clean after-run measured 5.376 ms combined CPU; the confirmation above
measures 5.244 ms. The visual instance count stays **2,017**. The orchard
dashboards remain cached while parked; their underlying moving-batch cache
invalidation is unchanged and still needs a separate fix for awake trucks.

2144 x 2144 per eye, bloom enabled, MSAA 4x, 240 warmup / 480 sampled frames:

| View / state | CPU update | GPU graphics, both eyes | Total elapsed frame |
| --- | ---: | ---: | ---: |
| bridge / Before | 14.152 | 11.189 | 31.876 |
| bridge / After | 5.519 | 11.520 | 22.485 |
| cafeteria / Before | 13.889 | 16.781 | 37.180 |
| cafeteria / After | 5.530 | 15.087 | 26.235 |

The same offscreen/cross-process limits above apply. These are inactive-vehicle
views, not measurements of driving/mounted workloads. GPU timings vary between
processes; the consistent gain is CPU update and removal of idle mesh uploads.
All updated captures have zero mirror passes.

Validation: proximity wake/deadband/exit; spawn falling before parking; manual
on/off and automatic mode; NPC wake and stop; mounting, driving, steering,
braking, XR stick input and dismounting; removing a mounted truck; unchanged
ordinary-physics support removal; hydraulic transitions; imported destruction.
No callback errors appear in the lifecycle or updated full CPU captures.

[Activation measurements](../analysis/ship-interior-activation-2026-11-06.json).

### 1920 x 1920 after activation

Fresh sequential runs at **1920 x 1920 per eye**, bloom enabled, MSAA 4x,
240 warmup / 480 sampled frames. The preceding attempt was discarded because
the user reported using the GPU during capture; these results use only the
restarted `/tmp/ship-activation-1920-clean-*` files.

| View | Average frame ms | p95 frame ms | Average GPU ms, both eyes | Uncapped equivalent |
| --- | ---: | ---: | ---: | ---: |
| bridge | 20.192 | 20.908 | 9.211 | 49.5 FPS |
| cafeteria | 23.321 | 23.987 | 12.370 | 42.9 FPS |

45 FPS requires 22.222 ms. The bridge clears that offscreen budget, including
at p95, with about 2.0 ms of average headroom. The cafeteria exceeds it by
about 1.1 ms on average. These remain stationary inactive-vehicle views and
exclude live tracking/input changes, ALVR encoding/transport and headset
pacing. Clearing the offscreen budget does not establish actual headset FPS.
Full summaries are in the activation measurements JSON linked above.

### CPU/GPU breakdown: 1920 versus 2144 after activation

Both resolutions use the activation change, bloom, MSAA 4x, 240 warmup frames
and 480 samples. All numbers below are average milliseconds per stereo frame.

| View | Per-eye size | CPU update | CPU preparation | CPU record/submit | CPU completion wait | GPU graphics, both eyes | Total elapsed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| bridge | 2144 x 2144 | 5.519 | 0.574 | 7.274 | 9.105 | 11.520 | 22.485 |
| bridge | 1920 x 1920 | 5.542 | 0.582 | 7.351 | 6.702 | 9.211 | 20.192 |
| cafeteria | 2144 x 2144 | 5.530 | 0.583 | 7.447 | 12.661 | 15.087 | 26.235 |
| cafeteria | 1920 x 1920 | 5.458 | 0.578 | 7.322 | 9.949 | 12.370 | 23.321 |

CPU active work (update + preparation + recording/submission) remains about
13.4 ms at either size. CPU completion-wait time is waiting for rendering, not
additional simulation work. GPU graphics overlaps CPU command recording; it
is not an extra column to add to the CPU stages. GPU deformation adds only
about 0.046 ms across both eyes. The sum of CPU active work and completion wait
closely accounts for total elapsed time, apart from profiling/query overhead.

1920 has 19.8% fewer pixels than 2144. GPU graphics drops 20.0% in the bridge
and 18.0% in the cafeteria, saving approximately 2.3 / 2.7 ms of GPU time.
Total elapsed frame time drops approximately 2.3 / 2.9 ms. The CPU update
workload is essentially unchanged; the lower resolution shortens GPU waits.
This comparison uses separate processes, subject to the variability caveat above.

## GPU-only bloom comparison at 1920 and 2144

These paired measurements supersede comparisons that subtract bloom-on and
bloom-off timings from different processes. The optimized scene stays loaded
with identical geometry/material identities. Each process measures four blocks
in **on / off / off / on** order, with 120 warmup frames then 240 samples per
block (480 samples and 240 warmup frames per state). GTX 1080, MSAA 4x.

Average GPU **graphics** milliseconds summed across both eyes; no CPU time,
completion waits or GPU deformation are included:

| View | Per-eye size | Bloom on | Bloom off | On minus off |
| --- | --- | ---: | ---: | ---: |
| bridge | 1920 x 1920 | 9.210 | 8.713 | 0.498 |
| bridge | 2144 x 2144 | 11.309 | 10.693 | 0.616 |
| cafeteria | 1920 x 1920 | 13.487 | 12.963 | 0.524 |
| cafeteria | 2144 x 2144 | 16.946 | 16.588 | 0.358 |

Bloom contributes roughly **0.4–0.6 ms across both eyes** in this probe.
Most GPU graphics time remains in rendering the scene. Bloom-off disables the
example's complete post-processing graph, including the emission pass; this
is not an isolated blur-kernel timer or a measurement of only the settings UI's
bloom-intensity toggle. GPU deformation remains outside this table.

ABBA controls ordering and scene identity, but does not lock GPU clocks. The
2144 cafeteria on-block means drift from 16.684 to 17.209 ms, so its 0.358 ms
delta should be read as an approximate small cost. Block summaries and p95
values are retained in the [paired measurements](../analysis/ship-interior-bloom-paired-2026-11-06.json).
Earlier separate-process controls reversed the cafeteria on/off ordering; do
not interpret those differences as an isolated bloom cost.

Reproduce one paired run after building the profiling test:

```bash
SHIP_PROFILE_CASE=paired_bloom SHIP_PROFILE_ROOM=bridge \
SHIP_PROFILE_WIDTH=1920 SHIP_PROFILE_HEIGHT=1920 \
SHIP_PROFILE_WARMUP=240 SHIP_PROFILE_FRAMES=480 \
SHIP_PROFILE_OUTPUT=/tmp/ship-bloom-paired.json \
  cargo test --release --lib profile_ship_stereo -- --ignored --nocapture
```

Repeat with `cafeteria` and/or 2144 dimensions, sequentially. In paired mode,
frame/warmup counts are totals **per bloom state**, split across two blocks.

## Idle controller and proximity-sensor overhead

A follow-up isolates whether the remaining idle controller callback and native
proximity observation are inexpensive. Each row averages two fresh 480-frame
CPU-only runs, after 240 warmup frames. Controller and sensor experiments use
separate full-scene baselines; there is ordinary cross-process timing variation.
All trucks stay loaded and parked, with the same 2,017 visual instances.

| Experiment | Control | Average CPU update | Average CPU preparation | Combined |
| --- | --- | ---: | ---: | ---: |
| controller | full | 4.418 | 0.533 | 4.951 |
| controller | no_input_controller | 3.883 | 0.499 | 4.382 |
| sensors | full | 4.500 | 0.540 | 5.041 |
| sensors | no_vehicle_sensors | 4.360 | 0.535 | 4.895 |

Removing the complete input controller saves approximately **0.54 ms of CPU
update** on average (individual paired differences 0.39–0.68 ms). Therefore
the current early-return FrameTick handler is not free, even though it no longer
queries trucks while unoccupied. This control removes the controller's event
subscriptions too; with no raw input events, FrameTick delivery is its steady
script workload. The difference is not an isolated interpreter instruction timer.

Disabling the six truck sensors saves approximately **0.14 ms of CPU update**
on average (paired differences 0.05–0.23 ms). This includes their native overlap
observation in this scene, not only shape intersection arithmetic. The native
observer currently scans enabled sensors/movers and checks their pairs; this
small-scene result is not a guarantee that arbitrarily many sensors scale freely.
The sensor control mutates event flags only after initial settling.

### Input lifetime boundary

Native headset/controller sampling remains needed for walking, hands and tools.
Vehicle-specific driving updates should run only for a mounted player or active
NPC driver, independently of cheap proximity wake/sleep observation.

MMS `on_global` currently registers a callback and returns null; there is no
MMS global unsubscribe operation. A guard in its callback is not removal. The
current controller consequently retains its FrameTick subscription while idle.
A component-owned input/driver subscription that is created on mount and
removed on dismount would give the requested lifecycle. That needs native host
lifetime support; this audit does not add a language/API feature or replace
frame-driven steering with a clock-dependent animation timer. Mounting remains
a separate responsibility from vehicle control.

Even without the entire input controller, CPU update remains about **3.9 ms**
in this probe, plus render preparation. The previously measured ~7 ms CPU
render command recording is a separate whole-scene cost, not idle truck input.
The ~1 ms native secondary-motion stage and other simulation/scene systems
remain active while trucks are parked.

[Raw summaries](../analysis/ship-interior-idle-controller-2026-11-06.json).
The CPU probe now accepts `no_vehicle_sensors` for this diagnostic.

## Next slices

1. Idle truck/patrol queries are now gated as recorded above. Mounted-driver
   validation still uses a live selector lookup; optimize that occupied path
   separately while preserving streamed-removal safety.
2. Fix CombineMesh invalidation so ancestor-only movement keeps a rigid batch
   cached, while actual source-relative changes still rebuild it. Cover both
   cases with upload-count tests; do not hide this by disabling truck gravity.
3. Investigate resting truck motion and the cost of transform notifications,
   including light/mirror maintenance and transformed triangle BVHs. Preserve
   intended sliding, support and driving behavior.
4. Measure scene draw submission/batching and visible-region work after the CPU
   fixes. Keep the existing bloom/MSAA preferences during optimization.
5. Confirm the improvements with live headset input and the runtime-reported
   per-eye extent, including OpenXR waits and ALVR pacing.

## Reproduction

```bash
cargo build --release --example profile-ship-cpu
CAT_PROFILE_SYSTEMS=1 CAT_PROFILE_SPATIAL=1 \
  target/release/examples/profile-ship-cpu full 480
# Repeat with the case names from the table, one process at a time.
# Optional, deliberately intrusive diagnostics (not a timing run):
SHIP_PROFILE_TRACE_UPLOADS=1 \
  target/release/examples/profile-ship-cpu full 120

cargo test --release --lib profile_ship_stereo --no-run
SHIP_PROFILE_ROOM=bridge SHIP_PROFILE_CASE=full \
SHIP_PROFILE_WIDTH=2144 SHIP_PROFILE_HEIGHT=2144 \
SHIP_PROFILE_WARMUP=240 SHIP_PROFILE_FRAMES=480 \
SHIP_PROFILE_OUTPUT=/tmp/ship-stereo.json \
  cargo test --release --lib profile_ship_stereo -- --ignored --nocapture
```

GPU rooms: `bridge`, `cafeteria`, `orchard`. GPU controls: `full`, `no_bloom`, `paired_bloom`,
`avatar_hidden`, `environment_hidden`, `no_callbacks`. Hidden controls suppress
graphics submissions while retaining simulation and deformation preparation.
GPU warmup/sample lengths default to 120 frames each and accept
`SHIP_PROFILE_WARMUP` / `SHIP_PROFILE_FRAMES` overrides. The frozen washroom
mirror issues no captures in these bridge/cafeteria views.

Machine-readable summaries: [profile data](../analysis/ship-interior-profile-2026-11-06.json).
Raw exploratory logs and perf captures are under `/tmp/ship-*` and are not
repository artifacts. The initial capture files retain the pre-activation baseline; activation
measurements are recorded separately above.
