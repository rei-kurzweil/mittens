# Sakura rooftop CPU and GPU profiling

Initial CPU investigation: 2026-10-26. Stereo follow-up: 2026-10-27.

The [controlled stereo follow-up](#2026-10-27-controlled-stereo-follow-up-with-springs-disabled)
identifies repeated global morph staging and per-material binding eviction while
springs are disabled. The earlier CPU-only results below cover running springs
and explain a separate transform-propagation bottleneck.

The [first optimization follow-up](#2026-10-27-first-optimization-reuse-the-immutable-morph-arena)
records the retained morph-buffer change and its before/after validation.

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
The subsequent [transform index and spring-contact slice](spring-world-collision-and-transform-parent-index.md)
implements retained transform-parent discovery; measurements above remain the earlier baseline.

1. Retain/index transform-parent dependents and batch spring pose propagation;
   avoid whole-world discovery for every hair-chain update.
2. Measure live tracking/IK and CPU/GPU submission on the headset. These results
   do not establish the cause of all pre-NPC headset slowness, and do not justify
   simplifying valid collision geometry yet.

### Remaining slowdown with springs disabled

The headset still slows after adding one or two Lumae with spring bones off.
The CPU-only paused comparison above does not measure that rendering workload.
Inspection of the current `assets/components/characters/lumae/lumae.glb` finds two meshes, two
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

## 2026-10-27: controlled stereo follow-up with springs disabled

The larger motion-dependent cost is a repeated **global morph-delta upload**,
not bone-palette eviction. A separate **material binding cache does thrash**.
Repeated Lumae already share immutable GPU meshes/textures and use instanced
opaque draws. This pass adds measurement tools and counters; it does not change
resource lifetime, cache capacity, batching, or deformation behavior.

### Workload and instrumentation

Base revision: `afa0e172`, plus this diagnostic instrumentation. The ignored
`profile_sakura_lumae_stereo` fixture loads the actual Sakura scene with 0, 1 or 2
Lumae, disables secondary motion, and renders both eyes at 1440×1600 with 4× MSAA
and the scene's bloom. A separate mitten model stands behind the camera. Static
cases leave its pose unchanged; moving cases rotate its head joint by a small
sine wave through the normal transform/palette path. The same simulated 75 Hz
sequence is used for every run. NPCs stay in the same relaxed pose at fixed,
visible positions. Live tracking, IK, microphone and compositor pacing are not
part of this automated fixture. After loading, idle weapon audio is retired so
its real-time clock cannot replace the deterministic test clock.

Each case runs in a fresh process: 240 warmup frames, 600 measured frames, three
repetitions. Repetitions alternate case order and run sequentially. Loading,
GPU uploads and pipeline creation are reported separately. The hidden control
suppresses only NPC graphics draws, retaining their ECS nodes, visual instances,
mesh/morph arenas and bone/deformation allocations. Saved images verify the
visible and suppressed controls.

CPU timers cover driver transform notification, update, palette calculation,
render preparation, command recording/submission, morph staging and completion
wait. Test-only Vulkan timestamps divide each eye into transfer/deformation and
graphics/post-processing intervals. The fixture waits the already-submitted
second-eye fence to bound frames in flight and safely reuse timestamp queries;
it adds no new queue submissions. This is an offscreen workload comparison,
**not headset FPS**. GPU and CPU columns overlap and should not be summed.

### Results

Milliseconds below are the median of the three per-run medians. Frame p95 is
the median of the three per-run p95 values. MB uses decimal bytes. Uploads are
averaged over all measured frames; the sine wave occasionally produces an
unchanged pose, so a moving run has deformation work on 596 of 600 frames.

| Case | Frame median ms | Frame p95 ms | CPU morph staging ms | GPU transfer/deform ms | GPU graphics ms | Morph MB/frame |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0-static | 8.27 | 8.88 | 0.00 | 0.00 | 3.05 | 0.00 |
| 0-moving | 12.43 | 12.87 | 2.68 | 0.64 | 3.59 | 7.61 |
| 1-static | 8.45 | 8.88 | 0.00 | 0.00 | 3.24 | 0.00 |
| 1-moving | 15.59 | 16.21 | 5.26 | 1.22 | 3.31 | 15.16 |
| 2-static | 8.56 | 9.04 | 0.00 | 0.00 | 3.30 | 0.00 |
| 2-moving | 16.82 | 17.57 | 5.34 | 1.24 | 4.39 | 15.16 |
| 2-moving-hidden | 15.60 | 16.28 | 5.31 | 1.23 | 3.19 | 15.16 |

Removing NPC draws recovers about 1.22 ms in the two-avatar moving case, while
about 3.17 ms remains above the moving baseline. CPU morph staging alone grows
by about 2.66 ms; GPU transfer/deformation grows by about 0.59 ms. Their size and
the suppressed-draw control identify the transfer path as the first fix to try.
This does not measure hardware cache misses or establish their absence.

### Resource sharing, palettes and draw batches

- One and two Lumae both use exactly two unique NPC GPU mesh handles and the same
  material/texture bindings. Their morph data is registered once: the global
  morph arena is 7,657,152 bytes without Lumae and 15,257,760 bytes with either
  one or two. A second Lumae does not duplicate its 7,600,608-byte morph payload.
- All stationary cases have zero deformation jobs, bone uploads, morph-delta
  uploads and deformation allocations after warmup. Moving cases have **zero
  dirty NPC jobs**. Only the driver deforms: about 16.89 primitive jobs/frame,
  independent of NPC count.
- Moving cases upload the same approximately 153,466 bone bytes/frame for all
  NPC counts. Palette CPU time stays near 0.041–0.042 ms/frame. No bone-buffer or
  deformation-output growth occurs in any measured case. The output allocation
  grows at setup from 1 MiB to 2 MiB for two NPCs, then stays retained.
- One NPC uses four NPC graphics draws per stereo frame (two primitives × two
  eyes). Two NPCs also use four draws, now with batch size two, drawing eight
  primitive instances. Neither uses sorted multi-layer transparency. The hidden
  control submits no NPC graphics draws. There is no missing instancing feature
  to implement for this case.

### Repeated immutable morph staging

`record_dirty_deformations` correctly returns early when no instance needs
work. When any job exists, however, it clones `deformation_morph_cpu`, allocates
new staging and device buffers, and copies the **whole global arena**, even
though these deltas are immutable and the NPCs are not dirty. In this fixture
there are no active driver morph targets, but the full arena is still copied.

The GPU morph buffer already created during `upload_mesh` is retained on the
renderer; the frame path creates another local buffer instead of reusing it.
Moving cases make approximately 8.94 deformation buffer allocations/frame (nine
per active frame), including the full morph staging/device pair. The delta array
is transferred only on the first eye; this is not duplicate skinning per eye.

Nsight independently supports the CPU timing: its dominant moving-case leaf
symbol is `Buffer::from_iter<GpuMorphDelta, ...>`, with 1,722 sampled leaves out
of 8,912 for zero NPCs and 3,393 out of 11,047 for two. The sample counts describe
profiled runs and include process threads; use the unprofiled table for timings.

### Material binding eviction

The legacy renderer-wide cache (capacity 512) has zero steady-state misses or
evictions. The per-material binding caches are separate and hold only **16
resource variants**. The shared pistol/avatar material's cache remains full and
cycles through more variants than fit:

| Case family | Binding misses/frame | Binding evictions/frame | New material UBO/descriptor snapshots/frame |
| --- | ---: | ---: | ---: |
| No NPCs | 38 | 38 | 38 |
| One or two visible NPCs | 42 | 42 | 42 |
| Two NPCs, draws suppressed | 38 | 38 | 38 |

This churn exists even in completely static scenes and is distinct from bone
palettes and mutable material revisions. This pass counts it but does not isolate
its CPU cost with a changed-capacity experiment. It is a separate follow-up.

### Nsight support and capture overhead

Nsight Systems **2025.3.2.474** successfully captured Vulkan API calls, process
CPU samples and OS runtime activity on the GTX 1080. The automated controller
starts recording after the fixture's warmup barrier and stops before teardown.
SQLite exports contain 288,200–336,748 Vulkan API events over 600 measured frames;
there are no startup pipeline-creation events in the warmed captures. GPU workload
rows were not produced for this offscreen configuration, so GPU timings above
come from the fixture's Vulkan timestamp queries. Hardware utilization metrics
remain unavailable on this Pascal GPU. Kernel CPU samples are restricted by
`perf_event_paranoid=2`; user-space CPU sampling works. No driver or kernel
permission setting was changed.

The captured frame medians were 8.65, 10.06, 13.17 and 17.25 ms for 0-static,
2-static, 0-moving and 2-moving respectively: approximately 4.6%, 17.6%, 5.9%
and 2.5% above the corresponding unprofiled repetition medians. Profiling
perturbs the workload; captured timings are not substituted into the result table.

### Artifacts and reproduction

The diagnostic fixture is private to test builds, including draw suppression,
per-component dirty-job attribution and timestamp queries. Additive renderer
counters for morph-delta uploads, deformation allocations and buffer growth also
appear in real XR performance reports. Existing weight-upload counters now count
the actual active-morph staging payload, including the required empty placeholder.

```sh
cargo test --release --lib profile_sakura_lumae_stereo --no-run
python3 scripts/profile_sakura_lumae.py \
  --binary <test-executable-printed-by-Cargo> \
  --output /tmp/sakura-stereo-profile --capture
```

The runner records the executable SHA-256 and frame counts in `run-config.json`,
refuses to mix incompatible runs, and can resume completed compatible cases.
Individual `--case 2-moving-hidden`, `--repeat`, `--warmup`, `--frames`, and
`--capture-only` options support focused validation. Capture defaults to the
installed `~/.local/bin/nsys-2025.3`; override with `--nsys`.

Measured artifacts: `/tmp/mittens-sakura-lumae-gpu-20261027/`. Each case has a
summary JSON, all frame samples in `.frames.json`, and a log. The visible and
hidden controls have PNG previews. Four `*-nsight.nsys-rep` captures and matching
SQLite exports retain the warmed traces. `results.json` indexes all 25 runs.
The earlier short probes under `/tmp/mittens-sakura-profile-*-smoke*` validate
instrumentation and are not part of the result table.

### Recommended follow-up

1. Reuse the immutable GPU morph arena uploaded during mesh loading; update it
   when the arena grows instead of rebuilding it for each deformation job batch.
   Keep per-instance active weights, poses and output ranges independent. Verify
   distinct poses/morph weights and repeat this matrix to quantify the change.
2. Investigate the 16-entry per-material binding cache with the measured resource
   working set. Retain enough structural bindings or choose a lifetime policy
   that avoids steady-state eviction while remaining bounded.
3. Retain the separate transform-parent scan optimization investigation. A single
   driven joint's transform notification grows only from about 0.225 to 0.239 ms
   here, but full live tracking/IK updates many more joints.
4. Sharing identical already-posed vertex output is a possible later memory
   optimization; ordinary instancing already works and the measured NPC output
   is not being recomputed each frame.

Live XR confirmation uses `examples/profile-sakura-xr.rs`, with the real tracked
player, springs disabled, the original scene placements, 4× MSAA and no desktop
window. It records ten seconds of warmup followed by fifteen seconds of samples:

```sh
cargo run --release --example profile-sakura-xr -- 0 /tmp/sakura-xr-0
cargo run --release --example profile-sakura-xr -- 2 /tmp/sakura-xr-2
```

Keep the same view toward the left roof edge/graphics panel, gently move head
and hands, and avoid locomotion or firing. Runtime extent, refresh rate, frame
statistics and renderer counters are saved by the existing XR perf collector.
Live confirmation results are recorded separately below.

### Live XR confirmation: completed

With the user's headset assistance, both timed runs completed successfully on
SteamVR/OpenXR 2.16.7, GTX 1080, 1940×1940 per eye, 4× MSAA. These are one
15-second capture per case after ten seconds of warmup, with natural head/hand
motion and the original roof placements. They confirm the transfer behavior,
not a precisely controlled FPS delta.

| Metric | 0 Lumae | 2 Lumae |
| --- | ---: | ---: |
| Sampled frames | 562 | 511 |
| Mean headset frame ms | 26.739 | 29.380 |
| Median headset frame ms | 26.675 | 26.862 |
| p95 headset frame ms | 27.281 | 40.253 |
| Arithmetic average FPS | 37.398 | 34.036 |
| Mean update before XR ms | 8.323 | 9.143 |
| Mean eye-render CPU ms | 6.698 | 9.368 |
| Mean GPU completion wait ms | 3.039 | 4.072 |
| Mean post-secondary palette CPU ms | 0.036 | 0.039 |
| Morph-delta bytes/frame | 7,657,152 | 15,257,760 |
| Bone bytes/frame | 154,496 | 154,496 |
| Deformation jobs/frame | 17 | 17 |
| Dirty vertices/frame | 33,779 | 33,779 |
| Deformation buffer allocations/frame | 9 | 9 |
| Bone/output buffer resizes | 0 | 0 |

The extra 2.670 ms in eye-render CPU time is consistent with the automated
morph-staging increase. Unchanged job counts and bone traffic confirm that the
NPCs do not need to be animated for the extra immutable morph payload to be
transferred. Additional live update cost (about 0.82 ms) also leaves room for the
tracked player's scene-size-dependent transform/IK work, beyond the single-joint
automated control.

SteamVR reported different average predicted display intervals during the two
runs (22.341 and 26.701 ms). Those values are not an independent measurement of
the physical panel's refresh setting. Do not interpret the corresponding report
"target refresh" or "frames exceeding display interval" percentages as a
controlled change in headset mode. Compositor pacing and non-identical human
motion limit interpretation of the FPS/tail-time comparison. Runtime dropped and
reprojected frame counts are unavailable. Real XR per-eye GPU timestamps and
material-binding eviction counts were not collected; the headless fixture
provides those separate diagnostics.

Reports and complete logs are in the artifact root's `xr-0/`, `xr-2/`, `xr-0.log`
and `xr-2.log`. The timed applications exited normally and left SteamVR running.

### Next CPU profiling pass: the remaining stereo submission cost

The zero-Lumae run's **6.698 ms eye-render CPU time covers both eyes together**.
In `OpenXRSystem`, that timer wraps XR image import, mirror-capture submission
(disabled for these runs), and the loop calling `submit_xr_eye`. It includes CPU
preparation, command recording and submission; GPU completion waiting has its
own timer. It is not a measurement of GPU execution or a demonstrated minimum
rendering cost.

First address the two measured sources of repeated work: immutable morph staging
and per-material binding eviction. Both also affect the zero-Lumae baseline.
Then split the remaining eye-render CPU time into:

- Resource/material synchronization and draw preparation, including sorting.
- Instance-buffer creation and uploads.
- Vulkan command recording, including Vulkano resource tracking and validation.
- Driver queue submission.

The existing Nsight samples include Vulkano's `AutoSyncState::add_command`, which
makes command recording a useful next target, but does not yet quantify how much
of the baseline it explains. Compare the two eyes' CPU scopes to identify repeated
view-independent preparation or material synchronization. Preserve work that
depends on each camera, such as transparency ordering. Deformation already runs
only for the first eye in the controlled fixture; do not assume skinning is
duplicated.

Repeat the same static/moving, visible/hidden, and avatar-count comparisons after
each change, then confirm in the headset with the same view and settings. Keep
CPU submission, GPU execution and completion waiting separate. The scene update
cost, including transform/IK propagation, remains a separate investigation from
this eye-render timer. No optimization in this plan is included in this checkpoint.

The [material pipeline and multiview slice plan](material-pipeline-multiview-slices.md)
details the proposed camera/pipeline migration, per-slice smoke/performance gates,
and an audit of profiling instrumentation overhead outside captures.

### Verification of this diagnostic change

- All 21 automated unprofiled case runs and four warmed Nsight captures completed;
  the fixture enforces stationary NPC reuse, correct hidden-draw suppression,
  deterministic clock ownership, and stable post-warmup bone/output capacities.
- Eighteen native deformation tests, eleven Sakura scene regressions, three XR
  perf collector tests, and both existing headless stereo/material GPU tests pass.
- The normal library check and release XR example build pass. Formatting, Python
  syntax and whitespace checks pass.
- No optimization was applied during this study. The existing bone allocator,
  immutable morph upload behavior and material-cache capacities are preserved.

## 2026-10-27: first optimization, reuse the immutable morph arena

For a concise explanation of the data flow and cache relationship, see
[morph buffer reuse and the skinning cache](../analysis/morph-buffer-reuse-and-skinning-cache.md).

### Change and lifetime behavior

`record_dirty_deformations` now binds the immutable `deformation_morph_buffer`
already uploaded by `upload_mesh`. It no longer clones the CPU morph arena,
creates a staging/device buffer pair, or records that full-arena transfer for
each dirty deformation batch. Active morph weights, bones, jobs, workgroups and
per-instance output ranges keep their existing independent behavior.

Loading a new mesh with morph targets still extends the CPU arena, uploads a new
immutable GPU arena, waits for that upload, and replaces the renderer's retained
reference. Previously recorded/submitted descriptors keep the old allocation
alive through their own references. This does not introduce in-place writes to a
buffer that older GPU work may still read. Optimizing mesh-load upload growth is
outside this change.

Skin-only scenes allocate one reusable zero delta entry because Vulkan storage
descriptors cannot reference empty buffers. Loading the first real morph mesh
replaces that placeholder. Upload/allocation counters now include morph arena
creation at mesh loading and the initial placeholder; steady-state comparisons
start after loading and warmup. Each active deformation batch avoids two buffer
allocations. The removed per-frame morph-staging timer reports zero because that
stage is gone, rather than timing loading or the retained-buffer lookup.

The material binding policy, transform propagation, spring solver, instancing,
shaders and stereo recording strategy are unchanged.

### Correctness checks

The new ignored Vulkan test
`morph_arena_reuse_preserves_weights_poses_and_recorded_generations` reads back
actual production compute output. It verifies:

- Skin-only deformation and repeated reuse of its zero placeholder.
- Zero deformation jobs/allocations on an unchanged warm frame.
- Two instances sharing one mesh with different poses, multiple target weights,
  negative weights, and position/normal deltas.
- Arena growth from an additional mesh with a nonzero target offset.
- Old and new command recordings returning their own expected results when the
  arena grows before the older command is submitted.
- Old arena retention through recorded work, then release after completion.

This GPU test, eighteen native deformation tests, eleven Sakura scene tests,
three XR perf collector tests and two existing headless stereo/material tests
pass: **35 focused tests**. The library check, release headset example build,
formatting and whitespace checks pass. No headset demo is launched by these tests.

### Controlled performance comparison

The final comparison runs the saved old and new release test binaries sequentially
in pairs for each case. It uses all seven existing cases, 240 warmup frames,
600 measured frames, three independent repetitions, and reverses case/binary
order on the middle repetition. Both binaries use the same scene/assets, fixed
camera/poses, 1440x1600 per eye, 4x MSAA, bloom, springs disabled, timestamps and
completion policy. No build, native/GPU regression test, or Nsight capture runs
concurrently with the final timing matrix.

The initial exploratory before-only runs overlapped compilation and are excluded.
The final artifact root is `/tmp/mittens-sakura-morph-comparison-20261027/`, with
`before/` and `after/` summaries, per-frame samples, logs, previews and executable
SHA-256 manifests. `/tmp/mittens-compare-morph.py` records the exact paired schedule;
`/tmp/mittens-sakura-morph-comparison.log` records its progress. The existing
`scripts/profile_sakura_lumae.py` provides the individual case runner.

All **42 unprofiled runs** completed. Values below are the median of three
per-run medians; p95 is likewise the median of the three per-run p95 values.
These are headless CPU/GPU workload times, not headset FPS.

| Case | Frame median before ms | After ms | Frame p95 before ms | After ms |
| --- | ---: | ---: | ---: | ---: |
| 0-static | 8.22 | 8.55 | 8.84 | 9.06 |
| 0-moving | 12.08 | 8.75 | 12.61 | 9.34 |
| 1-static | 8.64 | 8.76 | 9.12 | 9.49 |
| 1-moving | 16.19 | 9.21 | 17.20 | 9.77 |
| 2-static | 8.58 | 8.67 | 9.29 | 9.19 |
| 2-moving | 16.71 | 9.38 | 17.37 | 10.01 |
| 2-moving-hidden | 15.85 | 8.96 | 16.52 | 9.57 |

| Moving case | CPU record/submit before -> after ms | CPU morph staging before -> after ms | GPU transfer/deform before -> after ms | Morph MB/frame before -> after |
| --- | ---: | ---: | ---: | ---: |
| 0 Lumae | 6.595 -> 3.794 | 2.686 -> 0 | 0.639 -> 0.047 | 7.606 -> 0 |
| 1 Lumae | 9.484 -> 3.994 | 5.402 -> 0 | 1.228 -> 0.046 | 15.156 -> 0 |
| 2 Lumae | 9.357 -> 3.928 | 5.354 -> 0 | 1.232 -> 0.047 | 15.156 -> 0 |

Moving-frame medians improve by approximately 28%, 43% and 44% for 0, 1 and 2
Lumae respectively. Static controls have no expected benefit and drift upward
by about 0.09-0.33 ms. There is no claim that this fixes all baseline CPU time.
Graphics GPU time remains similar; for 2-moving it is 3.444 -> 3.580 ms. The
large repeatable improvement is removal of CPU staging and GPU transfer work.

All 21 pairs have exactly matching deformation jobs, dirty vertices, bone/weight
bytes, workgroups, NPC draw batches, submissions and material binding misses and
evictions. Cache hit counts differ between processes and are not asserted equal.
Every new case has zero steady-state morph-delta upload bytes. Moving cases
retain about 16.887 jobs and 153,466 bone bytes per sampled frame, with zero NPC
dirty jobs and zero bone/output resizes. Buffer allocations fall from nine to
seven per active batch (8.94 -> 6.953 per sampled frame because four frames have
no pose change). The 38/42 per-frame material-binding evictions remain unresolved.

Visible/hidden previews retain identical scene pixels outside a handful of
diagnostic text pixels: 10 changed pixels for 0-static and five each for 2-static
and 2-moving-hidden, out of 2,304,000 pixels per image. These differences are
confined to the diagnostic text region. Numeric GPU readback checks separately
verify nonzero morph deformation, which the stationary-avatar screenshots alone
cannot establish.

`comparison.json` aggregates the matrix; `pair-invariants.txt` records matching
workload counters and `preview-comparison.json` records the image differences.
Nsight follow-up and headset confirmation are recorded separately below.

### Warmed Nsight confirmation

One additional 2-moving capture completed after the timing matrix, using the same
240/600-frame configuration and Nsight Systems 2025.3.2.474. Its median frame time
is 10.079 ms versus the unprofiled repetition median of 9.384 ms (about 7.4%
capture overhead); it is excluded from the performance tables. The trace and
SQLite export are under the artifact root's `capture-after/` directory.

The warmed trace contains 327,290 Vulkan API events and no graphics/compute
pipeline creation. There are **zero sampled leaf calls to
`Buffer::from_iter<GpuMorphDelta, ...>` out of 6,781 leaves**, compared with
3,393 out of 11,047 in the earlier pre-optimization 2-moving trace. The same query
matches the old mangled Rust symbol, verifying that the zero is not a demangling
or name-filter mismatch. This independently confirms removal of the staging
hotspot. These sample counts come from different-duration captures; use the
unprofiled matrix for timing comparisons. Vulkano's `AutoSyncState::add_command`
is now the largest sampled leaf (485 samples), a useful target for the separate
command-recording investigation rather than a quantified multiview benefit.
`capture-after/nsight-analysis.json` retains these counts and leading symbols.

### Headset confirmation: completed after ALVR client restart

After the user's explicit ready signal, the first zero-Lumae launch reached only
`SYNCHRONIZED`, with no active controller profiles or visible/focused session.
The user reported a black screen, and the application was stopped. No warmup or
sampling began and no report was produced. Its log is retained as
`xr-0-attempt1-black.log`; this attempt is excluded from all measurements.

The user restarted the headset's ALVR streaming client and requested a retry.
Both subsequent runs reached `VISIBLE` and `FOCUSED`, acquired active Oculus Touch
controller profiles with aim/grip tracking, and completed ten seconds of warmup
plus fifteen seconds of sampling. Both applications exited normally. Settings
match the earlier captures: SteamVR/OpenXR 2.16.7, GTX 1080, 1940x1940 per eye,
4x MSAA, springs and mirror off, with the real tracked player and natural
head/hand movement. The ALVR connection recovery is not a renderer code change.

| Metric | 0 Lumae before | 0 Lumae after | 2 Lumae before | 2 Lumae after |
| --- | ---: | ---: | ---: | ---: |
| Sampled frames | 562 | 556 | 511 | 563 |
| Mean eye-render CPU ms, both eyes | 6.698 | 3.984 | 9.368 | 4.056 |
| Mean update before XR ms | 8.323 | 8.927 | 9.143 | 9.316 |
| Mean GPU completion wait ms | 3.039 | 4.398 | 4.072 | 2.708 |
| Mean frame submit ms | 7.485 | 8.409 | 5.543 | 9.357 |
| Mean post-secondary palette CPU ms | 0.036 | 0.039 | 0.039 | 0.038 |
| Mean headset frame ms | 26.739 | 26.979 | 29.380 | 26.666 |
| Median headset frame ms | 26.675 | 26.673 | 26.862 | 26.669 |
| p95 headset frame ms | 27.281 | 27.450 | 40.253 | 27.100 |
| Arithmetic average app FPS | 37.398 | 37.066 | 34.036 | 37.501 |
| Morph-delta bytes/frame | 7,657,152 | 0 | 15,257,760 | 0 |
| Deformation buffer allocations/frame | 9 | 7 | 9 | 7 |
| Bone bytes/frame | 154,496 | 154,496 | 154,496 | 154,496 |
| Deformation jobs/frame | 17 | 17 | 17 | 17 |
| Dirty vertices/frame | 33,779 | 33,779 | 33,779 | 33,779 |
| Bone/output resizes | 0 | 0 | 0 | 0 |

The headset confirms zero steady-state morph-delta traffic and the expected two
fewer allocations per active batch. Eye-render CPU time drops by 2.714 ms without
NPCs and 5.312 ms with two, consistent with the removed staging cost in headless
tests. The difference between the new 0- and 2-Lumae eye-render means is only
0.072 ms, compared with 2.670 ms in the earlier captures. That is evidence that
the large motion-dependent rendering penalty was addressed, not proof that NPC
rendering or their ECS nodes are free.

Application frame cadence still sits around 26.7 ms / 37.5 FPS. The roughly 9 ms
CPU update and runtime submission/pacing remain significant. Do not equate the
eye-render CPU reduction with an equal end-to-end frame reduction: the warm runs
also differ in GPU completion wait and frame-submit time. The newly reported
predicted intervals are 22.742 and 22.222 ms; earlier runs reported 22.341 and
26.701 ms. These are not independent physical refresh measurements. Natural
movement, view differences, one short capture per condition and runtime pacing
limit FPS/tail comparisons. Actual compositor dropped/reprojected frame counts
and live per-eye GPU timestamps remain unavailable.

The successful updated reports are
`xr-0/20261027-205332Z__avatar_no_spring_no_mirror.md` and
`xr-2/20261027-205408Z__avatar_no_spring_no_mirror.md` under the final artifact
root. `xr-comparison.json` records the old/new report values, and the matching
`xr-0.log` / `xr-2.log` retain session/tracking events. No further headset demo
is running. Next retain the material binding-cache follow-up and the separate
transform/IK update investigation; multiview remains a later measured slice.
