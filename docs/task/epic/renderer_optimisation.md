# Renderer optimisation

Status: active tracker; current work is validating the compute-deformation cutover and XR
submission pipeline, then removing the measured spring-visualization command-flush bottleneck.

## Purpose

Track measured performance work in `src/engine/graphics`, including `VisualWorld` cache
preparation, command-buffer construction, GPU resource uploads, submission, synchronization, and
render-thread architecture.

The objective is lower and more predictable CPU frame time without trading away rendering
correctness. Optimizations should record a baseline, isolate the cost being changed, and retain
before/after evidence.

## Working rules

- Measure before and after; distinguish CPU work, driver work, and time blocked on the GPU.
- Prefer removing work and allocations before adding concurrency.
- Keep ordinary window, XR, mirror, clipping, transparency, skinning, and post-processing behavior
  covered when their paths are touched.
- Avoid long-lived references into mutable `VisualWorld` vectors. Continue using canonical
  instance storage with lightweight indices unless a separate snapshot boundary is deliberately
  introduced.
- Record regressions or rejected approaches in the relevant task rather than silently removing
  them from this tracker.

## Execution order

### Phase 1: make render streams the main-phase source of truth

Implement
[render streams as the single source for clip-capable phases](../render-stream-single-source.md)
as one cohesive refactor:

1. Detect whether any stencil clip exists once during dirty draw-cache preparation and pass that
   fact into stream construction.
2. Add the no-clip construction path: emit only `RenderOp::DrawBatch` entries directly from each
   sorted phase order, without depth discovery, clip-source scans, per-depth groups, or DFS.
3. Keep opaque, cutout, single-layer transparent, and overlay rendering exclusively on their
   render streams, then remove the corresponding legacy flat batch caches, accessors, and rebuild
   work.
4. Make ordinary window and XR views borrow cached stream operations and instance indices instead
   of copying them with `to_vec()`.
5. Keep owned filtered stream construction only for views with an actual instance exclusion, such
   as mirror self-exclusion, and remove obsolete code exposed by the consolidation.

`DrawBatch` remains the batching payload inside `RenderOp::DrawBatch`. Background,
background-occluded-lit, emissive/bloom extraction, and per-view multi-layer transparency retain
their specialized flat batches during this phase.

Focused unit tests should be added or adjusted alongside these edits, but treat steps 1–5 as one
implementation unit before running the full validation gate. This avoids repeatedly exercising
window/XR/mirror examples against intentionally intermediate cache shapes.

### Phase 1 validation gate

After steps 1–5 are complete:

- Run the existing and new `VisualWorld` stream tests, including no-clip, nested clips,
  cross-phase clip sources, and mirror exclusion.
- Run `cargo check` and the relevant graphics tests.
- Compare representative unclipped and clipped window scenes.
- Verify XR and mirror examples where hardware/runtime access is available.
- Record cache-build time, allocations, copied stream bytes, operation counts, and instance counts
  before and after. Capture the pre-change baseline before implementation when no equivalent
  measurement already exists.

Do not begin CPU culling until this gate passes, so culling is built on one authoritative stream
representation.

Gate result: passed on 2026-07-25. Automated stream tests and checks passed, revision-comparison
measurements were recorded, and representative unclipped, clipped, scrolling, mirror, and XR
rendering were visually confirmed.

### Deferred: omit fully outside clipped content

Implement
[event-driven CPU culling for flat stencil clips](../event-driven-stencil-clip-culling.md):

1. Capture the pre-culling `scrolling` baseline: overlap-test count, emitted instances,
   instance-buffer bytes, draw instances, and CPU draw-cache preparation time.
2. Add clip-to-members and renderable-to-ancestor-clips indexes to `ClippingSystem`.
3. Reuse `VisualWorld::update_model`'s existing instance lookup to identify only changed clipped
   content and clip sources during transform propagation.
4. Pass that synchronous, deduplicated change batch to `ClippingSystem` after final world matrices
   settle; do not use reactive signals or a transform-dependency index.
5. Perform conservative clip-local 2D bounds rejection and exclude culled instances before stream
   construction.
6. Validate nested clips, topology changes, transform streams, transform-parent dependents,
   visibility restoration, and rotated 3D UI.

This phase is paused before baseline capture. Resume it only after the current
[GPU-cached deformation and morph-target epic](gpu-cached-deformation-and-morph-targets.md)
reaches an explicit stopping point or profiling changes the priority.

### Deferred: reduce active-stencil recording cost

Create focused follow-up tasks, with measurements, for:

- storing the resolved instance-buffer slot in `EnterClip` and `ExitClip` operations instead of
  calling `stream_instances.iter().position(...)` while recording;
- omitting a clip's stencil operations from phases that have no visible content requiring that
  clip.

### Current direction: validate compute-cached deformation and XR submission

The source cutover implements the first three parts of
[GPU-cached deformation and morph targets](gpu-cached-deformation-and-morph-targets.md):

1. Move repeated graphics-stage skinning to an event-driven compute pass.
2. Cache mesh-local position and normal once per dirty deformation generation.
3. Reuse that output across desktop, mirror, extraction, and XR-eye consumers.
4. Later, extend the same pass with glTF morph targets and selected-instance editor controls.

Items 1–3 are implemented in source, and graphics-stage skinning has been removed. The Phase 1
gate remains open for GPU readback/consumer parity, unchanged-frame proof, buffer lifetime and
frames-in-flight safety, required-hardware validation, and before/after GPU timings.

The XR path discovered during this cutover has also advanced: duplicate per-eye mirror scheduling
was removed, and
[OpenXR submission pipelining](../openxr-render-submission-pipelining.md) is implemented through
Phase 4. Validation-layer headset runs, session restart coverage, and before/after performance
reports remain.

This work continues to supersede CPU clip culling. Completed render-stream work and its
measurements remain the foundation and historical record; clipping tasks remain planned but
deferred.

### Broader renderer work

Use profiling results to order instance-buffer reuse, upload batching, submission/wait reduction,
and renderer-thread work. Do not make renderer threading a prerequisite for the cache and clipping
improvements above.

The corrected XR deformation measurements also isolated
[spring-bone visualization command-flush performance](../spring-bone-visualization-command-flush-performance.md)
as an adjacent CPU-update bottleneck. Marker calculation takes about `0.08 ms`, while executing
the per-marker transform intents through the generic signal and transform path takes about
`66 ms`. Address that measured batching problem independently of compute deformation and the
secondary-motion solver.

## Effort tracker

| Effort | Status | Target | Evidence / outcome |
| --- | --- | --- | --- |
| [Render streams as the single source for clip-capable phases](../render-stream-single-source.md) | Complete | Remove duplicate phase caches and common-path per-view stream copies | Validation gate passed; revision comparison records a 50.0% unclipped cache-build reduction, 41.4% fewer allocation calls, and zero common-path stream-copy bytes |
| [GPU-cached deformation and morph targets](gpu-cached-deformation-and-morph-targets.md) | Active; Phase 1 source implemented, validation open | Evaluate dirty deformation once and reuse it across passes/views, then add morph targets and editor controls | Compute cutover and graphics-stage skinning removal landed in `ef592dc`; focused deformation tests pass; GPU parity, lifetime, hardware, and before/after evidence remain |
| [OpenXR render submission pipelining](../openxr-render-submission-pipelining.md) | Implemented through Phase 4; verification open | Remove intermediate mirror/eye waits while preserving OpenXR image ownership | Source now GPU-orders mirrors, both eyes, and raw copy with one final fence wait; headset validation and performance reports remain |
| [Spring-bone visualization command-flush performance](../spring-bone-visualization-command-flush-performance.md) | Open; measured | Batch retained debug-marker transform updates without repeating generic world dependency work per marker | Corrected XR baseline: visualization calculation `0.082 ms`, post-pose command flush `66.345 ms`, visualization-on FPS `8.389` |
| [Event-driven CPU culling for flat stencil clips](../event-driven-stencil-clip-culling.md) | Deferred; baseline not captured | Keep clip membership indexed and omit fully outside content without per-frame scans | Resume with the pre-culling `scrolling` workload baseline |
| [Opt-in system, MMS, Vulkano, and XR profiling](../opt-in-system-mms-vulkano-xr-profiling.md) | Planned | Establish selectable CPU/GPU measurements for optimization work | Pending |
| [Renderer thread refactor](../refactor/renderer-thread.md) | Design | Move recording/submission work off the simulation thread | Requires profiling and a decided snapshot/command boundary |
| [Renderer CPU-time complexity](../../analysis/renderer-cpu-time-complexity.md) | Analysis | Submission, future cleanup, waits, uploads, and threading opportunities | Existing investigation and candidate mitigations |

## Candidate queue

Candidates become standalone tasks before implementation when they affect ownership,
synchronization, render ordering, or public interfaces.

### Cache and command preparation

- Measure and reduce repeated per-phase instance-buffer allocation/upload, especially emissive
  subset buffers that are currently constructed during render-view preparation.
- Audit `VisualInstance` value copies in sorting and cache construction; use borrows where they
  prevent large semantic copies, then verify generated behavior with profiling.
- Measure per-view transparent sorting and determine whether unchanged view/instance state can
  reuse results.

### Resource and descriptor work

- [Material descriptor caching for stable and frequently updated inputs](../material-descriptor-cache-update-frequency.md):
  measure hit rates and allocation churn, then separate stable descriptor identity from
  high-frequency parameter storage.
- Batch mesh and texture uploads and keep synchronous upload waits out of active frame rendering.
- Audit transient uniform, storage, and instance buffers for safe per-frame reuse.

### Submission and synchronization

- Count submissions and explicit waits per window frame, XR frame, eye, mirror capture, and upload
  burst.
- Validate the implemented one-wait XR submission pipeline and quantify its effect on missed
  frames and tail frame time.
- Keep frames in flight bounded and measure Vulkano future cleanup separately from GPU waits.

### Parallelism

- Decide the renderer-thread ownership and messaging boundary only after profiling establishes the
  amount of simulation-thread time that can be hidden.
- Consider parallel secondary-command-buffer recording only if command recording remains a
  significant measured cost after allocation and cache cleanup.

## Measurement record

For each completed optimization, update its tracker row with:

- workload and build profile
- window/XR/mirror configuration
- CPU frame-time or scoped timing before and after
- allocation, upload, draw, or submission counts relevant to the change
- GPU timing when the optimization may shift rather than remove work
- hardware/runtime and date

## Definition of done for an effort

- The task has explicit correctness and performance acceptance criteria.
- Relevant automated tests and `cargo check` pass.
- Representative window rendering is verified; XR and mirror rendering are also verified when
  the changed path affects them.
- Before/after evidence is recorded in the task or linked artifact.
- This tracker is updated with the result and any follow-up work.

### Procedural mesh module follow-up

Move closed primitives, planar primitives, and debug wireframes out of
`graphics/mesh.rs` into mesh-family modules. Preserve mesh handles and
cached buffers. Compare generated topology and mesh-generation time so the
split adds no allocations or work to draw recording. Run mesh and headless
stereo rendering tests after the split.
