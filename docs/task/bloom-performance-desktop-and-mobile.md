# Research: fast bloom for desktop and XR2-class GPUs

Date: 2026-10-29
Status: open research/backlog; no new bloom implementation or benchmark result.
Checkpoint: `731a54ac` (custom opaque renderer and desktop XR comparison).

Related: [existing bloom pyramid](opt-in-bloom-pyramid.md),
[implemented bloom controls](sakura-scene-polish-bloom-and-can-physics.md),
[emission selection/lighting policy](material-lighting-emission-and-extraction-policy.md),
[custom emission capability proof](material-emission-capabilities-and-led-strip.md),
[mobile renderer scope](../../android/docs/opaque-renderer.md).

## Outcome

Find a **very fast bloom implementation that looks good enough**, with measured
cost/quality trade-offs for two targets:

1. Desktop GPUs: reduce the current bloom cost and determine which controls
   should expose useful quality/performance choices.
2. Snapdragon XR2 / Adreno 650 and similar mobile GPUs: identify a practical
   low-bandwidth bloom path for stereo rendering on the Vive Focus 3.

Physical realism and a perfect Gaussian are optional. Favor an attractive,
stable glow with low frame cost and bounded memory. The headset's eventual
animated, anime-complexity, mirrored scene targets at least 90Hz; bloom must
have a measured incremental budget within that workload. This ticket does not
claim that budget is available, or promise 90Hz from a standalone effect test.

This is a follow-up research task. The first mobile renderer remains
opaque/albedo-only; do not add bloom to its current slice as part of this ticket
creation. Keep source/style decisions separate from algorithm and precision.

## Questions to answer

### 1. Filtering, passes and bandwidth

- Are we exploiting **bilinear texture filtering to combine weighted taps**,
  rather than merely using a linear sampler? Derive correct offsets/weights
  and check texel-center alignment, boundaries and normalization. The current
  radius-scaled, spaced taps cannot simply be assumed equivalent to paired
  adjacent texels at every radius.
- Compare a small separable Gaussian with paired taps against a downsample /
  upsample dual-filter or tent pyramid. Match halo size and intensity when
  judging cost; allow an approximate kernel if it looks good in motion.
- Establish whether larger radii are cheaper and more stable through coarser
  levels than widely spaced full-resolution samples. Investigate sampling
  artifacts on thin LEDs, bright points, text and moving emissive edges.
- Audit actual shader compilation before optimizing kernel arithmetic: are
  weights/loops already folded or unrolled? Consider fixed kernels or bounded
  cached variants only where they improve measured cost.
- Measure fullscreen fragment passes versus a focused compute variant, including
  memory traffic, synchronization and occupancy. Do not assume compute wins on
  desktop or Adreno, or require one implementation for every GPU.
- Compare half/quarter-resolution starts, level count and per-level tap counts.
  Investigate combining compatible extraction/downsample/filter/composite work
  and avoiding unnecessary full-resolution intermediate images.
- For a tiler, draw the resource lifetime and load/store/resolve graph. Identify
  outputs that must survive for later texture sampling and depth that can be
  discarded. Do not assume a multi-pixel blur remains tile-local just because
  it is expressed as subpasses. Check attachment pressure, MSAA resolves,
  off-chip reads/writes and render-pass breaks on the actual driver.

### 2. Glow sources, HDR and artistic intent

**Which surfaces glow** is an authoring choice. **How much dynamic range and
precision their buffers retain** is a separate implementation/quality choice.
Do not silently turn an emissive-only style into whole-scene bright-pass bloom,
or require full-scene HDR solely to support selected emissive sources.

Shortlist and compare these routes rather than implementing every combination:

| Route | Research question |
| --- | --- |
| LDR scene + selected LDR emissive source | How far can inexpensive bounded/normalized emission go before clipping, banding or hue loss becomes unacceptable? |
| LDR scene + higher-range emissive source | Can we retain useful emission range without paying for a full HDR scene target and its entire composition path? |
| HDR scene + selected emission and/or scene bright-pass | What visual benefit justifies the format, extraction, exposure/tone-mapping and bandwidth costs? |

- Compare suitable linear UNORM, packed floating RGB (for example
  `B10G11R11_UFLOAT_PACK32`) and FP16 intermediates. Query actual attachment,
  filtering, blending and storage-image capabilities for each proposed use;
  do not assume packed-float or storage support on the installed Focus 3 driver.
- Record the scene color, emission source, pyramid and final XR formats
  independently. Audit clamping, linear/sRGB conversions, exposure, hue and
  composition order; an SDR output does not by itself settle intermediate HDR.
- Compare a separate extraction draw against MRT output or another compatible
  source-generation route. Include repeated geometry/rasterization, material
  variants, extra attachments, depth preservation, resolves and bandwidth in
  the total cost. Reuse cached deformation across emission/eye/mirror views.
- Preserve authored unlit/lit surface appearance, material emission capability
  and extraction opt-in/out semantics. Coordinate with the existing emission
  policy task rather than creating a second material flag/registry contract.
- Define the acceptable approximations explicitly: highlight clipping, broad
  versus tight halos, limited range or reduced resolution can be good trade-offs
  if they preserve the intended style. Physically based radiance is not required.

## Checked starting point

Source audit at `731a54ac`; these are code observations, not measured timings:

- [The blur shader](../../assets/shaders/post-process-bloom-blur.frag) specifies
  17 weighted `texture()` samples per one-dimensional pass, with offsets scaled
  by radius. It does not explicitly construct paired adjacent Gaussian taps.
- [PostProcessingRenderer](../../src/engine/graphics/post_processing.rs) already
  uses linear min/mag filtering with clamp-to-edge. It has half-resolution and
  pyramid support; simply enabling linear filtering is not new optimization.
- The [existing runtime slice](sakura-scene-polish-bloom-and-can-physics.md)
  exposes 1–6 active pyramid levels with normalized per-scale merging. A bounded
  pool of coarse image pairs stays allocated to avoid reallocating on changes.
  Measure retained memory versus runtime allocation savings before replacing it.
- `build_target_set` currently passes its caller-supplied `color_format` to
  main color, extraction source and blur targets. Record actual formats and
  determine whether scene and bloom precision should be independent; do not
  describe the existing pipeline as necessarily HDR.
- `VulkanoRenderer::record_bloom_passes` returns before extraction/filtering
  for nonpositive intensity or absent eligible emission. Audit remaining
  preparation/allocation work with bloom disabled or no sources.
- Effective blur radius/resolution can come from an enabled emissive BlurPass
  ahead of Bloom settings. It is not automatically a second serial blur.
- The new mobile backend currently has no bloom/emission pass. The Focus 3
  [driver probe](../../android/docs/focus-3-vulkan-results.md) established its
  classic-pass requirement; it did not benchmark bloom.

## Investigation and measurement checklist

- [ ] Map the desktop pipeline: extraction, downsampling, filtering, merging,
  composition, image formats/extents, MSAA, depth dependencies and resource reuse.
- [ ] Capture an actual baseline with bloom off and with today's bloom on;
  establish incremental CPU and GPU cost, allocations and retained image memory.
- [ ] Prototype a small shortlist from the questions above in focused modules.
  Compare one variable at a time before selecting combined desktop/mobile presets.
- [ ] Collect GPU timestamps where supported with the queue's timestamp limits
  and period validated. Also record CPU preparation/recording, wall time through
  GPU completion and total active XR frame time. Label combined wall time clearly;
  it is not an isolated GPU duration. Exclude `xrWaitFrame` from active work.
- [ ] Measure desktop and the real Focus 3 separately. Desktop results, vendor
  slides and tap-count arithmetic cannot establish Adreno performance. Record
  firmware/driver, GPU, build revision/profile, clock/thermal state and method.
- [ ] Use release builds, fixed scene/camera/exposure/halo size, equal settings
  and warmup/sample windows; repeat both run orders and save raw timings and
  images/video. Report p50/p95/p99, variance, stage costs and incremental bloom
  time even when displayed FPS stays at the same runtime pacing step.
- [ ] Lock and report actual per-eye resolution, MSAA, view count and formats.
  The local ALVR configuration is 2144x2144 per eye at 90Hz; the initial desktop
  demo received a scaled 1892x1892 recommendation. Use equal explicit extents
  for compared runs. Record the standalone runtime's actual recommended extent
  separately. A pixel-count change is not a measured speedup.
- [ ] Confirm new presented-frame cadence, reprojection/missed frames and
  sustained behavior with the headset worn/visible. The current comparison
  startup logs contain no visible-frame performance samples. No hypothetical
  frame-time examples or frame-budget calculations count as benchmark results.
- [ ] Exercise ordinary stereo, a representative animated avatar, and mirrors
  as those features become available. Decide whether mirrors apply bloom in
  their captures, after composition, or another explicitly supported route;
  include repeated effect cost and avoid unintended double bloom.

Visual checks: thin colored LEDs and points, broad sun glow behind foreground
silhouettes, saturated highlights, dark gradients, flat stylized/anime surfaces,
depth occlusion, viewport edges, small/odd-sized targets and head/camera motion.
Check stereo consistency, shimmer/flicker, banding, brightness stability across
levels/resolution, and sensible behavior with no emission or zero intensity.
Include supported cutout/background semantics in the desktop reference; reject
unsupported phase combinations in a mobile prototype rather than changing them.

## Configuration proposal to produce

Investigate exposing a small set of meaningful trade-offs through the existing
graphics settings owner/MMS controls. Keep artistic intensity/radius/source
selection separate from quality/cost. Candidate choices include a quality
preset, base resolution, pyramid depth/kernel family, and precision/HDR policy;
do not expose every implementation knob without a demonstrated authoring need.

Specify defaults per backend/capability, effective-setting queries, unsupported
choices/fallback reporting, and whether each change is live, allocates/resizes
resources or needs reinitialization. Bound pipeline variants and retained images;
preserve authored settings when globally disabled, and skip unused work. Preserve
existing scenes/defaults unless a deliberate migration is documented. Show a
measured quality/cost table so users can choose for a reason.

## Completion criteria

- [ ] A short research report links primary sources, states tested hypotheses
  and limitations, and compares a reproducible baseline with the selected
  desktop and mobile candidates, including raw evidence and visual comparisons.
- [ ] Recommend a good-enough fast desktop route and a practical XR2-class route,
  or explain with measurements why a candidate fails the mobile budget. Document
  differences instead of assuming one universal winner.
- [ ] Decide scene/source precision and emission-selection semantics independently,
  with explicit accepted artifacts and a configuration proposal.
- [ ] Set an incremental bloom budget from the representative workload and
  record whether it leaves room for native 90Hz, deformation and mirrors. Mark
  unavailable hardware/features and untested combinations as pending.
- [ ] File focused implementation/validation slices with module ownership,
  resource lifetime and regression gates. Research completion does not imply
  the new renderer already supports bloom or that the 90Hz target is achieved.

## Initial primary-source reading

- [RasterGrid: efficient Gaussian blur with linear sampling](https://www.rastergrid.com/blog/2010/09/efficient-gaussian-blur-with-linear-sampling/):
  original explanation of reducing texture lookups through weighted linear
  sampling. Validate the math against our kernel/spacing; historical performance
  numbers are not transferable to today's desktop or XR2.
- [Arm/Nordeus: post-processing effects for mobile](https://developer.arm.com/community/arm-community-blogs/b/mobile-graphics-and-gaming-blog/posts/post-processing-effects-for-mobile-at-gdc18):
  source-selection, precision/bandwidth trade-offs and dual filtering as research
  candidates. This is Mali-oriented evidence, not an Adreno 650 benchmark.
- [Khronos: render-pass attachment use](https://docs.vulkan.org/samples/latest/samples/performance/render_passes/README.html):
  load/store and transient attachment guidance. Apply it to each resource's real
  consumers; sampled blur outputs need preservation beyond their producer pass.
