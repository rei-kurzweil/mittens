# Adreno 650 rendering optimization

Status: proposed optimization epic. Source baseline reviewed 2026-11-11;
this document does not implement renderer changes or establish a performance result.

Target: standalone Vive Focus 3, XR2 / Adreno 650, using `mitten-droid`'s
application-owned Vulkan renderer. PC OpenXR through SteamVR/ALVR uses the
desktop Vulkano backend and needs separate measurements.

## Goal and scope

Reduce CPU scene preparation, instance upload, and command recording costs while
preserving the GPU's tile-local rendering opportunities. Measure improvements on
the headset at matched resolution, MSAA, foveation, scene, and visual quality.
The broader Android target is at least 90 Hz; that is a target, not a demonstrated
result. Keep CPU work and GPU work individually below the cadence budget with
headroom, rather than adding overlapping timings into a misleading frame total.

Keep the generic custom material definition path and currently supported
opaque/albedo `Surface` base. Do not introduce legacy material pipelines or silent
fallbacks. Skinning, morphs, mirrors, bloom, and additional material bases remain
explicit feature work; this epic should accommodate their future dependencies
without implementing them as prerequisites.

## Current baseline

| Area | Current behavior |
|---|---|
| Shared scene preparation | Desktop and Android consume the same `VisualWorld` draw batches, render streams, sorting keys, and opaque buckets. |
| CPU draw cache | Already retained while clean; a global dirty flag currently rebuilds the cached phases together. |
| Android instance data | Every frame copies ordered model/color data into a packed array and uploads the full used range; buffer capacity is retained. This is data repacking, not another membership sort. |
| GPU resource caches | Meshes, textures, material bindings, and pipelines persist. These are Android-specific caches, separate from desktop recording caches. |
| Command recording | One primary command buffer is reset and recorded every frame, including two eye passes and two batch loops. Both eyes already use one queue submission. |
| Synchronization | One in-flight submission, with a blocking fence wait before releasing the XR image. |
| Attachments | Classic render pass per eye; transient depth/MSAA attachments, preferred lazy memory, and in-subpass resolve directly to XR color. |
| Multiview | Queried and reported, but not enabled or used by the scene renderer. The recorded Focus 3 probe reports support with a maximum of six views. |

See [renderer implementation and limits](../../../opaque-renderer.md),
[recorded device capabilities](../../../focus-3-vulkan-results.md), and
[frame recording source](../../../../src/renderer/mod.rs).

These are complementary opportunities, not one cache:

| Retain or reduce | What it avoids | What can still change every frame |
|---|---|---|
| CPU draw membership/order | Reclassification and sorting of unchanged geometry | Camera, transforms, colors, lights |
| Stable instance layout and dirty uploads | Repacking/uploading unchanged instance records | Changed records and camera/light data |
| Vulkan command recordings | Repeating unchanged binds and draw commands | Synchronized contents of stable buffers |
| Stereo work through multiview | Recording the same draw sequence separately for each eye | Two eye projections and rasterization |

## Work slices

### 1. Establish a measured headset baseline

Record CPU update, shared render preparation/cache rebuild, instance packing and
upload, command recording, submission, and fence wait separately. Record GPU
timestamps where supported, XR missed frames, and sustained thermal behavior.
Fence wait includes GPU backpressure; it is not all CPU computation.

Include draw/batch counts, uploaded bytes, cache rebuilds/hits, resource growth,
and pipeline creation counts. Preserve historical runs with revision, changes,
actual per-eye extent, cadence, MSAA, foveation, scene, and warm-up conditions.
Report median and tail timings, not just a single average. Desktop checks remain
useful for correctness but do not establish Adreno performance.

### 2. Narrow shared draw-cache invalidation

Track which render phases actually need membership/order rebuilding. Camera or
buffer-value changes should not invalidate static draw membership unnecessarily.
Keep existing ordering and clipping semantics intact wherever supported.

This is separate from the proposed retained sort-key/group storage work. Begin
with conservative phase invalidation and measure rebuild frequency before adding
more retained structures. Shared changes benefit both backends and require
desktop regression checks too.

### 3. Retain instance layout and update mutable data

Keep a mapping from draw instances to stable packed ranges while membership/order
is unchanged. Update changed transforms, colors, and opacity without reconstructing
the full array. Camera-only movement should not require rewriting static instances.
Measure dirty-range bookkeeping against a full contiguous upload for small scenes
and scenes where most instances change.

Preserve capacity and resource identities where practical. Buffer growth and
binding changes must explicitly invalidate affected recordings. Future joint or
deformation updates should similarly change data without invalidating unrelated
draw commands; they are not supported by the current Android slice.

### 4. Prototype retained Vulkan recordings

Compare caching complete primary recordings against a small per-frame primary
that executes retained secondary draw groups inside the existing classic pass.
Choose using measurements; do not target an arbitrary count of three primaries.
Android currently has no deformation pass requiring the desktop's compute split.

Cache compatible variants for acquired XR images/framebuffers and, when needed,
frame slots. Separate retained command-pool lifetimes from transient resets and
retire resources only after their submissions finish. Start with the existing
single-in-flight synchronization model.

A classic render pass cannot begin in one primary and end in another. Avoid
ending/reopening passes or adding intermediate images just to separate recording
responsibilities: that can force tile stores/reloads. Secondary recordings can
keep draw groups within one physical pass, but many tiny command buffers may add
overhead. See [Khronos command-buffer guidance](https://docs.vulkan.org/samples/latest/samples/performance/command_buffer_usage/README.html).

### 5. Add and compare an explicit Vulkan multiview path

The existing XR swapchain already has two array layers. Enable the supported
multiview device feature and build a classic stereo render pass with view mask
`0b11`, layered attachment views, and one draw loop. Preserve the two-eye path as
an explicit comparison and compatibility option.

Use stereo camera data selected with `gl_ViewIndex`. Handle fragment shaders that
read camera/view data for rim or other view-dependent effects too; changing only
the vertex shader would leave an incorrect eye-dependent material interface.
Validate shader targets, background rotation, depth, 1x/4x MSAA resolve, and both
XR array layers. The current per-eye foveation maps need a supported layered
arrangement or an explicitly reported limitation, not a silent loss of foveation.

Multiview can reduce repeated recording and some geometry/setup work. It does not
halve fragment shading, eliminate instance uploads, or reduce today's submission
count from two to one: it is already one. See [Vulkan multiview](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_multiview.html).

### 6. Revisit frame overlap only after reuse is correct

Investigate reducing the blocking wait or introducing more in-flight resources
as separate work. Preserve OpenXR image acquire/wait/release rules and GPU-safe
buffer writes. Measure latency, memory, and throughput together; more frame slots
are not automatically better for XR. Reuse does not require this change first.

## Invalidation and attachment rules

| Change | Intended response |
|---|---|
| Camera/light/instance values in compatible stable buffers | Update synchronized data; retain compatible recordings. |
| Draw membership, order, batch size, or bucket changes | Rebuild affected CPU phases and dependent recordings. |
| Mesh buffers, descriptor identities, pipeline/layout changes | Invalidate dependent recordings; retain unrelated resources. |
| Extent, formats, MSAA, foveation attachment configuration | Select/rebuild compatible pass, pipeline, and recording variants. |
| Different acquired XR image | Select its compatible framebuffer/recording variant. |
| Resource destruction or buffer growth | Retire safely and invalidate references to replaced resources. |

Continue clearing/discarding transient depth and multisample color, resolving
directly into XR color, and avoiding unnecessary copies or intermediate targets.
Near/Middle/Far opaque buckets are ordering groups sharing depth, not separate
physical render passes. Keep the classic-pass path for the measured Focus 3
driver; dynamic rendering is not a prerequisite for these optimizations.

## Validation and completion criteria

- Compare unchanged baseline and each slice independently, then their combination.
  Use matched per-eye extent, MSAA, foveation, scene, and sustained run conditions.
- Exercise static scenes, camera movement, dynamic transforms/colors, material
  updates, additions/removals, buffer growth, and scene replacement.
- Compare both eyes for correct projection, background, view-dependent shading,
  supported transparency, MSAA edges, and foveation. Test pause/resume and resource
  retirement with validation enabled where available.
- Keep correctness checks distinct from performance captures. Record CPU/GPU
  improvements, tail regressions, memory costs, and remaining bottlenecks before
  enabling a new path by default.

Related desktop work: [Vulkan rendering optimization epic](../../../../../docs/task/epic/vulkan-rendering-optimization/README.md).
Reuse the architectural lessons, not desktop timings or an assumed identical
command-buffer structure.
