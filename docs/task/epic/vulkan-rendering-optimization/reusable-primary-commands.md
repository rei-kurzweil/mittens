# Reusable primary command buffers

Status: graphics-primary prototype implemented; controlled cafeteria captures below.
Final regression checks are recorded with the implementation validation.
Baseline: `4a310ae8` plus the retained-upload prototype recorded in
[the history ledger](benchmark-history.md).

## First boundary

Separate a view into a dynamic scene/deformation primary, a retained bloom
extraction/filter primary, and a dynamic final-composite/debug primary. Submit
these in order in the existing future chain with one flush per view. The
refraction path that composites bloom before sampling scene color keeps its
existing inline recording. Desktop and XR share this implementation.

Camera and light descriptor sets are retained by their actual buffer ranges and
fallback texture view. Host-visible buffers are updated only when writable;
busy storage is replaced and the changed binding invalidates the recording.
Pose matrices are assumed to change continuously: staged bone uploads and
compute deformation stay dynamic. Cached extraction draws read the persistent
deformation output, whose contents change without changing its binding.

A complete extraction/filter primary avoids repeating Vulkano's per-draw
resource analysis for that chain. Automatic synchronization is still active:
Vulkano's primary builder emits conservative first-use barriers. A focused
submission wrapper forwards Unknown access queries to the actual preceding
future: Vulkano 0.35 otherwise loses provenance for a resource absent from its
last primary, including deformation output between eyes. Explicit denials are
never overridden. No queue-idle waits,
manual barrier replacement or additional per-view flushes are introduced.

## Invalidation and lifetime

The bounded per-view cache compares exact dependency records, rather than
assuming camera stationarity. Records cover batch structure, mesh buffer ranges
and index counts, instance buffer ranges, global/rig bindings, resolved reflection
textures, attachment identities/extents/formats, and recorded bloom settings.
Material revisions, texture view replacements and custom-program replacements
retire cached commands before selecting material snapshots. This prevents cache
ownership from retaining a succession of obsolete material revisions.

First-use image initialization is special. Vulkano records an Undefined initial
layout for an image's first recording. Replaying that recording after the image
reaches its ordinary layout is invalid. The first render with new targets or newly activated bloom layers stays in the
existing single-primary path to initialize the images. Subsequent renders can
split out the reusable steady-state primary. Recordings use `SimultaneousUse`; pending commands retain their own
resources when an entry is replaced or evicted.

## Validation and measurement plan

- Compare cached and fresh GPU output pixel-for-pixel while changing cameras,
  lights and joint matrices every frame.
- Assert both-eye cache hits during those changes; assert rebuilds when a bloom
  setting or mesh buffer changes.
- Recheck stereo, mirror and custom-material paths.
- Preserve stationary and moving-head cafeteria runs at 1920 x 1920 per eye,
  MSAA 4x with bloom, alongside the same-build fresh-recording control.
- Record cache hits/rebuilds/invalidations, CPU stages, GPU times and distributions.

The test-only `SHIP_PROFILE_REUSE_COMMANDS=0` control retains buffer/descriptor
improvements but records bloom inline as before. Results will be appended here
and linked from the ledger, including any regression from splitting primaries.

## First bloom-only checkpoint

The first controlled cafeteria pair (240 warmup, 480 samples, 1920 x 1920
per eye, bloom, MSAA 4x, GTX 1080) confirms two cache hits and zero rebuilds
per sampled frame. CPU recording/submission drops from 6.913 to 6.075 ms,
but GPU graphics rises from 13.638 to 15.515 ms and total elapsed rises from
23.602 to 24.944 ms. This boundary is a regression overall, despite avoiding
CPU recording work. Separate-process variation remains possible; added broad
cross-primary barriers and layout transitions are a candidate cost, not yet a
proven isolated explanation.

Preserved [enabled summary](data/bloom-primary-on-2026-11-06.json) and
[disabled summary](data/bloom-primary-off-2026-11-06.json), frame samples and logs
are adjacent. Next experiment: retain the complete eligible graphics primary,
with deformation uploads/compute in a small dynamic primary. That removes the
extra scene/bloom/composite image boundaries and avoids substantially more CPU
recording. Keep complex paths dynamic until their dependencies are supported.

## Broader graphics-primary experiment

An eligible view now records only uploads/deformation in a small fresh primary,
then executes a retained primary containing the scene, emission extraction,
filtering and final composite. Single-layer transparency uploads are prepared
before cache lookup, so their transforms keep updating on cache hits. Sorted
multi-layer transparency, mirror captures, scene-color sampling/refraction,
stencil debug and named post-process output publications keep fresh recording.

Recordings are indexed by view and output image. Their dependency keys include
actual upload ranges, descriptor identities, attachments, post-process config,
clear color and per-view reflection bindings. Structural draw-cache rebuilds and
changes in mesh storage, materials, textures or custom programs retire recordings.
Camera/light buffer contents and joint matrices do not invalidate commands.

The implementation still uses conservative automatic barriers. Its first-use
single-primary initialization applies to whole graphics recordings too. There
is no secondary command replay or bypass of Vulkano's safety checks.

## Complete-graphics measurements

Same binary for all four sequential captures; baseline `4a310ae8` plus the
retained-upload and primary-reuse implementation. Benchmark executable SHA-256:
`561422311b84d4b9e60cf987d6840143716e630263163a6c7b3f0fe7df50b679`.
Later formatting, test additions and disabling bloom-only production fallback do
not change the eligible graphics path measured here. GTX 1080; actual ship
cafeteria; 1920 x 1920 per eye; bloom/extraction on; MSAA 4x; 240 warmup and
480 sampled frames. Order: stationary on/off, moving-head off/on.

| Mean stereo-frame measurement | Stationary fresh | Stationary reused | Moving-head fresh | Moving-head reused |
| --- | ---: | ---: | ---: | ---: |
| CPU update | 4.730 ms | 4.387 ms | 4.675 ms | 4.383 ms |
| CPU preparation | 0.545 ms | 0.567 ms | 0.557 ms | 0.560 ms |
| CPU recording/submission | 7.056 ms | **0.999 ms** | 7.041 ms | **0.995 ms** |
| GPU graphics | 13.928 ms | 12.465 ms | 14.200 ms | 15.640 ms |
| Elapsed frame | 24.145 ms | **18.414 ms** | 24.306 ms | **21.549 ms** |
| Elapsed p95 | 24.987 ms | 19.403 ms | 25.330 ms | 22.351 ms |
| Approximate FPS from mean elapsed | 41.4 | **54.3** | 41.1 | **46.4** |

Both enabled captures have exactly two command-cache hits and zero rebuilds per
sampled stereo frame. All four captures still have 480 deformation dispatches,
8,160 deformation jobs and 960 view submissions. Joint matrices and deformation
are not frozen or removed. Draw counts are replayed into the profiling counters
when commands are reused, so those metrics still describe executed GPU work.

The CPU reduction is about **6.05 ms per stereo frame**. Both enabled means meet
the 22.0 ms goal, but moving-head p95 slightly exceeds the 22.22 ms 45 FPS cutoff.
Sequential-process GPU variation is substantial: do not attribute GPU differences
solely to CPU command reuse, or add GPU time to overlapping CPU stages. These are
offscreen captures, excluding live OpenXR, compositor and ALVR overhead. They do
not establish sustained headset performance. The parked-truck physics limitation
in the earlier fixture remains unchanged.

Artifacts, including full timing distributions, cache counters and projected
per-frame samples:

- Stationary: [fresh summary](data/scene-primary-stationary-off-2026-11-06.json), [fresh frames](data/scene-primary-stationary-off-2026-11-06.frames.json), [reused summary](data/scene-primary-stationary-on-2026-11-06.json), [reused frames](data/scene-primary-stationary-on-2026-11-06.frames.json).
- Moving head: [fresh summary](data/scene-primary-head-off-2026-11-06.json), [fresh frames](data/scene-primary-head-off-2026-11-06.frames.json), [reused summary](data/scene-primary-head-on-2026-11-06.json), [reused frames](data/scene-primary-head-on-2026-11-06.frames.json).
- Matching logs use the same filenames with `.log` beside the summaries.

`SHIP_PROFILE_REUSE_COMMANDS=0` disables command reuse without disabling retained
uploads/descriptors. `SHIP_PROFILE_REUSE_SCENE=0` selects the bloom-only experiment;
that boundary is not the production fallback because its total frame time
regressed. Unsupported views keep the existing single-primary recorder.

The current upload allocator replaces busy storage rather than waiting. Such
replacements correctly invalidate recordings; sustained reuse with additional
in-flight buffer versions still needs live-runtime measurement. Cache ownership
is bounded and does not replace the GPU's pending-use resource tracking.

## Completed validation

- Production `cargo check --lib` and 22 release visual-world CPU tests pass.
- [Animated-input pixel comparison](data/primary-reuse-animated-pixels-2026-11-06.log)
  passes for fresh recording, bloom-only reuse and complete-graphics reuse.
  Camera and joint matrices change across frames; a lit object also verifies a
  light-only change reaches cached draws. GPU pixels match fresh recording for
  every tested frame and eye. Cache hits and invalidation after bloom settings
  and mesh storage replacement are asserted.
- [GPU regression log](data/primary-reuse-gpu-validation-2026-11-06.log): stereo
  with/without MSAA, independent material revisions, first ship view, mirror
  orientation and individual freeze (5 headless tests); custom material snapshot
  reuse across 10,000 revisions; deformation/morph arenas; Surface Grid; retained
  uploads (4 additional tests). All pass.
- Preserved benchmark summaries have identical executed draw counts between
  fresh and reused runs; each capture retains active deformation and two view
  submissions per stereo frame.

Live headset/compositor testing, sorted-transparency/refraction reuse and a
buffer-version pool for sustained overlap remain subsequent work. These checks
use Vulkano's resource validation and GPU readback; no claim of a separate Vulkan
synchronization-layer audit is made.
