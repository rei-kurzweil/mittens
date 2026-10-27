# Sakura rooftop CPU profile

Date: 2026-10-26

Scene snapshot: these measurements precede per-floor building chunks, the
opt-in Lumae pedestal, three-pistol table and scoped projectile hit callbacks.
Rerunning the probe measures the current scene, explicitly spawning three Lumae
for NPC cases and none for baseline cases, rather than reproducing the exact
historical counts below.

The first controlled CPU measurements point to transform propagation and script
hit-event delivery, rather than BVH traversal or static contact, as the larger
costs. No collision or transform behavior was changed during this investigation.

## Method and limits

`examples/profile-sakura-cpu.rs` loads the actual rooftop MMS, materials, models,
combined geometry and collision zones. A CPU uploader supplies mesh/texture
handles so normal renderable and skin binding preparation completes, without
issuing GPU commands. The XR player is replaced by one gravity-driven capsule
that slides horizontally on the roof; the probe verifies that it lands at the
expected height. Presentation time advances at 75 Hz, with normal 120 Hz physics.

Each case runs in its own process, with four resource-preparation frames, 240
warmup frames and 480 sampled frames. Timings below exclude parsing, model/texture
loading and initial BVH construction. Native profiling is enabled. Cases run
sequentially. Host: Intel Core i5-10400, Rust 1.98.1, optimized release build.

These are CPU workload comparisons, **not headset frame times or FPS**. There is
no live tracking, primary-player avatar/IK, GPU deformation/drawing, OpenXR frame
wait, headset submission or vsync. `prepare` below is CPU render preparation;
update includes the probe's moving-capsule transform notification.

## Initial comparison

Average milliseconds per sampled frame:

| Case | Lumae NPCs | Update | CPU prepare | Sum |
| --- | ---: | ---: | ---: | ---: |
| Full city, before NPCs | 0 | 1.230 | 0.649 | 1.879 |
| Full city, running hair | 3 | 5.886 | 0.758 | 6.644 |
| Full city, hair paused after settling | 3 | 1.482 | 0.670 | 2.152 |
| Full city, all Collidables disabled | 3 | 5.855 | 0.762 | 6.617 |
| Full city, two continuous rays hitting the backing wall | 3 | 8.399 | 0.901 | 9.300 |
| Full city, two hit rays before NPCs | 0 | 3.754 | 0.924 | 4.678 |
| Simplified neighbors, before NPCs | 0 | 0.993 | 0.408 | 1.401 |

The full city has 954 static collidables and one enabled slide mover in this
probe. Simplified neighbors reduce the static count to 575. The full NPC case
has 12,803 ECS components and 57 hair chains. Disabling contact did not materially
improve update time; pausing secondary motion did.

### Contact and spatial queries

Warm static-contact passes averaged approximately 0.006 ms with running hair,
about 0.02 ms per presentation frame at 3.6 passes/frame. Neighborhoods contained
roughly 2–3 candidate surfaces per pass. Membership built the contact BVH once;
its static surfaces required no steady-state refits.

Two hit rays performed 960 queries over 480 sampled frames, with one hit each
and no fallback scans. Broad/narrow-phase query and sorting time together
averaged about 0.0031 ms/frame. Upward rays that missed still performed the same
number of queries, averaging about 0.0009 ms/frame, with zero hit events.

The renderable BVH also remained retained; two maintenance calls per frame do
not imply two rebuilds. The probe's moving capsule has no renderable, and the
NPC hair joints do not introduce moving raycast meshes.

### Spring transform propagation

For the running-hair case, existing per-frame timers measured:

| Stage | Average ms/frame |
| --- | ---: |
| Spring solver | 0.120 |
| Spring-driven transform propagation | 4.174 |
| Post-spring bone palette update | 0.042 |
| Spring visualization bookkeeping | 0.033 |

Linux `perf` sampling independently identified
`TransformSystem::update_transform_parent_dependents` as the largest hot symbol.
It calls `world.all_components()` to find transform-parent operators for each
changed spring root, even when there are no relevant dependents. Running all
57 chains repeatedly scans the entire scene. Pausing skips those notifications.
The native update path also uses this notification mechanism for ordinary pose
changes, so the live tracked player requires separate headset measurement.

### Hit delivery

Three repeated two-hit-ray runs averaged 8.40–8.53 ms/update. Keeping the native
scene and rays but closing the MMS session reduced this to 6.25–6.40 ms/update.
A separate upward-miss run, with MMS still active, averaged 6.31 ms/update.
Closing MMS without adding rays measured 6.25 ms/update. These later controls
had somewhat slower transform-propagation timing than the initial sweep, so
use the comparisons as workload evidence rather than exact isolated subtraction.

The BVH query itself is inexpensive. The larger hit-dependent cost is downstream
of it. `laser.mms` registers one global `RayIntersected` callback per projectile
slot. All three idle pistol slots receive both unrelated ray hits and reject them
inside MMS: six script callbacks per frame for these two rays. `perf` also shows
MMS expression cloning/destruction in the hit workload. The session-close control
removes all MMS delivery, including the display cars' ordinary FrameTick handlers;
the miss control retains those FrameTick handlers while avoiding hit callbacks.

## Query cadence in the current runtime

- The renderable BVH flushes changes twice per presentation frame, before motion
  and after controller/gizmo poses. Unchanged geometry does not rebuild it.
- RayCastSystem runs once per frame. Default event-driven pointers cast on
  trigger/grip press, hold and release, desktop press/drag, or explicit requests.
  Visible controller lasers alone do not force continuous hit tests. Weapon
  projectiles request a cast when launched. The probe deliberately adds two
  continuous rays as an active-ray workload.
- Sliding contact has two pose passes per presentation frame plus one pass per
  fixed 120 Hz physics substep: approximately 3.6 passes/frame at 75 Hz, or 3.3
  at 90 Hz. Each pass can perform additional local queries while resolving
  corrections, up to the six-iteration limit and final convergence check.
- Native avatar foot-support queries also share the collidable BVH. Those are
  outside this synthetic-capsule comparison.

## Current changes and follow-up priorities

Projectile hits now also reach a scoped handler on their own raycaster, without
duplicating surface/ancestor or global delivery. `laser.mms` uses that route;
idle projectile slots no longer enter MMS for unrelated pointer hits. The
historical hit-delivery timings above describe the earlier global subscriptions.
Transform-parent discovery has not been optimized yet.

1. Retain/index transform-parent dependents and batch spring pose propagation;
   avoid whole-world discovery for every hair-chain update.
2. Measure live tracking/IK and CPU/GPU submission on the headset. These results
   do not establish the cause of all pre-NPC headset slowness, and do not justify
   simplifying valid collision geometry yet.

### Remaining slowdown with springs disabled

The headset still slows after adding one or two Lumae with spring bones off.
The CPU-only paused comparison above does not measure that rendering workload.
Inspection of the current `assets/models/lumae.glb` finds two meshes, two
primitives, two materials, six textures, 30,200 vertices summed across primitives,
44,169 triangles and one skin with 146 joints. One primitive has 57 morph
targets; the other has none. These are asset counts, not measured per-frame work.
The material/primitive count alone does not suggest many separate draw calls;
geometry, shading, morph/skin work and CPU scene bookkeeping still need timing.

Next capture identical headset views with 0, 1 and 2 stationary Lumae, with
secondary motion disabled, after loading and caches have warmed. Compare CPU
update/render-preparation time, GPU eye/pass time, draw counts, deformation jobs,
and bone upload bytes. Repeat with the NPC meshes hidden while retaining their
hierarchies to distinguish rendering cost from the extra scene nodes. Check
whether unchanged skin/morph inputs reuse deformation output before considering
mesh simplification, material changes or another pose-baking path. Also measure
the tracked player's transform work as the world grows: its updates can still
scan the larger scene even when NPC springs are disabled.

## Reproduce

Run from the repository root:

```sh
CAT_PROFILE_SPATIAL=1 CAT_PROFILE_SYSTEMS=1 \
  cargo run --release --example profile-sakura-cpu -- full 480
```

Available cases: `baseline`, `baseline_rays`, `full`, `paused`, `no_contacts`,
`two_rays`, `rays_miss`, `no_callbacks`, `rays_no_callbacks`, `simple_city`.
Run cases sequentially to avoid CPU contention. Each prints a `PROFILE_BEGIN`
marker followed by the measured section and a `RESULT` line with mean/p50/p95
update time, CPU preparation time and spring-stage timing. Periodic spatial
summaries include contact candidates, BVH rebuild/refit counts and ray queries.

For native headset logs, retain the actual XR scene and enable the same timers:

```sh
CAT_PROFILE_SPATIAL=1 CAT_PROFILE_SYSTEMS=1 \
  cargo run --release -- load examples/sakura-drone-conversion.mms
```

Investigation logs and `perf` recordings are under `/tmp/mittens-rooftop-*`.
The periodic engine logs are also written under `docs/.debug/`.
