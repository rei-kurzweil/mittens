# OpenXR render submission pipelining

Status: pipelining implemented through Phase 4; direct XR output added 2026-10-20. Headset/session verification and performance comparison remain pending.

Related:

- [Renderer futures and frame timing](../draft/renderer-futures-and-timing.md)
- [Compute-cached deformation XR performance regression](compute-cached-deformation-xr-performance-regression.md)
- [Opt-in System, MMS, Vulkano, and XR profiling](opt-in-system-mms-vulkano-xr-profiling.md)
- [Mirror viewer-family captures](mirror-viewer-family-captures.md)
- [OpenXR 1.1 swapchain image management](https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html#swapchain-image-management)

## Outcome

Keep XR rendering validation-clean while replacing the current sequence of per-consumer CPU waits
with GPU-ordered submissions and one bounded completion wait before the acquired OpenXR swapchain
image is released.

The current implementation uses runtime-owned final output images:

```text
mirror submissions -> eye 0 -> eye 1 -> completion-only submission/fence
       no wait       no wait  no wait             CPU wait once
                                                  release XR image
```

## Implementation status

- OpenXR creates single-sample stereo array images. Vulkano borrows their handles and caches a 2D view for each eye layer; the runtime retains image/allocation ownership.
- With MSAA off, eyes render directly into those views. With MSAA on, transient color resolves into them. Post-processing's final composite also writes directly into them.
- There are no separate single-sample eye-output images or final eye-image copies.
- Mirror captures and eyes extend `submission_future` and flush without intermediate CPU waits.
- An empty raw Vulkan submission on the exact shared graphics queue signals one reusable fence. Queue access uses Vulkano's external-synchronization guard.
- The fence is waited before release, then Vulkano's retained submission chain is retired. Pending image views retain the runtime swapchain and Vulkan device until that retirement, including failure/teardown paths.
- The wrapper checks acquire/wait ordering and Vulkan device identity before exposing an image to rendering. Failed waits/incomplete GPU work do not release an image or submit it for projection.
- The existing format-mismatch clear remains available because scene pipelines still use the window's color format.
- `CAT_OPENXR_DEBUG=1` reports image index, submissions and completion waits. CPU reports use `completion_wait` / "GPU completion wait" in place of the old copy duration.

The remaining acceptance work requires an OpenXR runtime and headset: validation-layer runs,
2,000-frame mirror-off/mirror-on captures, session stop/restart coverage, and before/after
performance reports.

This does not promise a zero-wait OpenXR frame loop. Core swapchain image release does not carry a
Vulkan semaphore or fence in `XrSwapchainImageReleaseInfo`. The application must not let the
runtime consume an image while application GPU work still references it. Unless a supported and
verified runtime synchronization mechanism provides that handoff, retain one completion wait
before `xrReleaseSwapchainImage`.

## Direct-output checks (2026-10-20)

- Focused graphics/OpenXR/mirror checks: 81 passed; the hardware-only test is ignored in that run.
- Hardware regression: passed on an NVIDIA GeForce GTX 1080. Three cached stereo images are reused across direct rendering, 4x MSAA resolves, and the actual final bloom-composite pipeline, with per-eye pixel readback. The same completion-only fence path is exercised. Khronos validation layers were unavailable; Vulkano's checks remained enabled.
- A wider library run encountered 51 failures and hanging audio checks. All 51 observed failures also reproduce on an unchanged HEAD snapshot. The hanging run was stopped.
- All-target compilation is blocked by the pre-existing `TransformParentComponent` import in `tests/editor_visualization_transform_regressions.rs`; the unchanged snapshot reproduces that error.
- Actual OpenXR acquire/release, headset visuals, session restart, and runtime validation-layer checks remain to be performed with a headset.

```bash
cargo test --lib -- engine::graphics engine::ecs::system::openxr_system mirror
cargo test --lib xr_renderer::tests -- --ignored --nocapture
```

## Historical serialized baseline

For a normal format-compatible OpenXR frame:

1. acquire and wait for one OpenXR swapchain image;
2. render all stereoscopic mirror captures;
3. signal a Vulkano fence and call `wait(None)` after every mirror capture;
4. build and submit eye 0, then call `wait(None)`;
5. build and submit eye 1, then call `wait(None)`;
6. record an Ash command buffer that copies both offscreen eye images into the acquired OpenXR
   array image;
7. submit the copy without a fence and call `queue_wait_idle`;
8. release the OpenXR image and submit the composition layer with `xrEndFrame`.

This is safe but introduces CPU/GPU bubbles. The CPU cannot build later eye work while the GPU
runs earlier eye work, and the whole graphics queue is drained for a copy whose completion can be
tracked with one fence.

The corresponding current entry points are:

- `VulkanoState::submit_xr_eye` in `src/engine/graphics/vulkano_renderer.rs`;
- `VulkanoState::submit_xr_mirror_captures` in
  `src/engine/graphics/vulkano_renderer.rs`;
- `submit_xr_completion_and_wait` in `src/engine/graphics/xr_renderer.rs`;
- the acquire/render/complete/release sequence in `src/engine/ecs/system/openxr_system.rs`.

## Original pipelining performance hypothesis

Removing intermediate waits should allow:

- the CPU to build eye 1 while eye 0 is executing;
- the GPU to begin the next queued command buffer without waiting for a CPU fence round trip;
- mirrors, both eyes, and the copy to remain one ordered queue workload;
- the final completion wait to cover only the remaining tail of the queued GPU work;
- the copy path to wait for one submission rather than idling the entire device or queue.

Total GPU work does not automatically decrease. The expected gains come from eliminating queue
bubbles, overlapping CPU command preparation with earlier GPU execution, and avoiding broad idle
operations.

## Required invariants

### One shared-resource ordering chain

Mirror and eye submissions that use renderer-wide deformation, material, descriptor, mesh, or
runtime-texture resources must extend `submission_future`. Do not replace the waits with
independent `sync::now` branches.

### OpenXR image ownership

An OpenXR swapchain image must be acquired and successfully waited before the application records
or submits writes to it. It must not be released until the application has completed the required
graphics work and will no longer access that image.

Track ownership by the actual OpenXR image index. Do not infer it from a window swapchain index or
eye number.

### Offscreen target ownership

Each per-eye MSAA/depth attachment, mirror capture, and reusable command-buffer slot must have a completion rule.
A target or command buffer must not be reset or overwritten while an earlier submission can still
use it.

The initial pipelined implementation may keep one set of offscreen targets because it performs one
final completion wait per XR frame. Any cross-frame overlap requires multiple completion-tracked
slots before that final wait can move later.

### Vulkano and raw Vulkan ordering

The Vulkano renderer and `xr_renderer.rs` currently submit to the same Vulkan graphics queue.
Confirm this with queue handles/family indices and keep all calls that access the queue externally
synchronized.

On one queue, Vulkan queue submission order can order the raw copy after the Vulkano mirror and
eye submissions. If a future implementation uses different queues, add an explicit Vulkan
semaphore dependency; host call order alone is not sufficient across queues.

### Vulkano tracking after raw work

The raw Ash copy is outside Vulkano's future graph. Keep the final Vulkano completion reference
alive until the raw-copy fence proves that all earlier same-queue work has completed. Then signal
and clean the tracked Vulkano future before reusing its resources.

Do not call `signal_finished` merely because commands were submitted. It is valid only after the
corresponding GPU completion has been established.

### Failure visibility

Validation errors, device loss, queue submission failure, fence failure, and OpenXR
acquire/wait/release errors must remain visible. Recovery may wait idle and reset tracking, but
that path must be explicitly counted and must not become steady-state behavior.

## Phase 0: capture the serialized baseline

Use the existing VR performance presets and counters before changing synchronization:

```bash
cargo run --release --example vtuber-secondary-motion -- \
  --vr-perf-case avatar_no_spring_no_mirror

cargo run --release --example vtuber-secondary-motion -- \
  --vr-perf-case avatar_no_spring_mirror
```

Capture at least:

- headset frames, elapsed time, and delivered FPS;
- mean, median, p95, and p99 headset frame time;
- frames exceeding the runtime display interval;
- `eye_render` and raw `copy` CPU durations;
- Vulkano queue submissions and CPU fence waits per XR frame;
- raw queue submissions and queue-idle waits per XR frame;
- mirror captures and XR eyes per frame;
- validation output and SteamVR compositor fall-through/flicker.

Record the expected baseline counts for the actual scene. With two eyes and no mirrors, the
current path should report two Vulkano fence waits and one raw queue wait per rendered XR frame.
Mirror-enabled counts should additionally expose one fence wait per mirror submission.

### Phase 0 verification

- Run the same release build and stable preset for every later comparison.
- Confirm counter deltas agree with the source-level wait sites.
- Preserve the generated report as the before result.
- Do not use window redraw FPS as XR FPS.

## Phase 1: make XR batch boundaries explicit

Refactor without changing wait behavior:

1. represent one logical XR render batch containing required mirrors and all active eyes;
2. separate command-buffer construction, queue submission, completion, and OpenXR release in the
   API names and profiling spans;
3. give the batch a monotonically increasing generation;
4. log the acquired OpenXR image index, offscreen slots, queue identity, submission count, wait
   count, and exceptional reset events behind the existing profiling/debug controls;
5. centralize queue external synchronization so Vulkano and Ash cannot submit concurrently.

### Phase 1 verification

- Render output and wait counts are unchanged from Phase 0.
- Each logical XR frame has exactly one batch generation.
- Both eyes and all required mirrors report the same batch generation.
- The trace proves that Vulkano and Ash use the expected queue.
- Vulkan validation remains clean through startup, steady rendering, and shutdown.

## Phase 2: remove the wait between XR eyes

Change eye submission so `submit_xr_eye_offscreen` flushes and stores a descendant of
`submission_future` without calling `wait(None)` after each eye.

Keep a temporary batch-completion operation after the final eye and before the raw copy. Waiting
for the latest descendant completes both eyes because they are in one linear submission chain.

Do not prebuild both eyes and wait to submit them as one large CPU batch unless measurement shows
that is preferable. Submitting eye 0 before building eye 1 permits eye 1 command preparation to
overlap eye 0 GPU execution.

### Phase 2 verification

- Both eyes render correctly with distinct views.
- The no-mirror preset falls from two eye fence waits to one batch fence wait per XR frame.
- No fence wait occurs between eye 0 submission and eye 1 command-buffer construction/submission.
- The raw copy still starts only after the temporary batch-completion wait.
- Eye 0 offscreen color, depth, MSAA, bones, and post-processing resources are not reused early.
- Validation is clean for at least 2,000 presented headset frames.
- Compare `eye_render`, total frame time, missed intervals, and delivered FPS with Phase 0.

If this phase produces no measurable improvement, retain its trace and determine whether command
recording, mirrors, deformation, or the final copy wait dominates before continuing.

## Phase 3: remove waits between mirror captures and eyes

Make mirror captures descendants in the same XR batch without calling `wait(None)` after each
capture. Schedule each required stereoscopic capture once per logical XR frame, followed by both
eyes.

Keep the one temporary Vulkano batch-completion wait before the raw copy.

### Phase 3 verification

- There is no CPU fence wait between mirror captures or between mirrors and eyes.
- There is exactly one Vulkano batch-completion wait before the raw copy.
- Each required mirror is captured once per viewer-family/capture slot, not once per eye.
- Mirror runtime textures sampled by later eye work have a declared dependency through
  `submission_future`.
- Mirror-off and mirror-on presets differ by the expected capture and submission counts.
- Validation is clean for at least 2,000 headset frames with mirrors on and off.
- Compare mirror cost, `eye_render`, total frame time, and missed intervals with Phases 0 and 2.

## Phase 4: replace queue idle with one final fence wait

Remove both the temporary pre-copy CPU wait and the copy submission's `queue_wait_idle`:

1. flush mirror and eye submissions without waiting;
2. submit the raw copy after them on the same externally synchronized graphics queue;
3. signal a reusable Vulkan fence from the raw copy submission;
4. wait for that fence before releasing the OpenXR swapchain image;
5. after the fence signals, clean the Vulkano submission chain and mark the raw command-buffer
   slot reusable;
6. reset the fence only when its previous submission is complete.

If the copy moves to another queue, signal a semaphore from the last Vulkano submission and wait
on it in the copy submission. The final copy fence is still the completion proof used before
OpenXR image release.

### Phase 4 verification

- Normal XR rendering performs zero `queue_wait_idle` and zero `device.wait_idle` calls.
- The complete mirror/eye/copy sequence has exactly one CPU completion wait per rendered XR frame.
- That wait is attached to the raw-copy fence and occurs before `xrReleaseSwapchainImage`.
- Submission tracing proves the copy follows the final eye on the same queue, or proves the
  cross-queue semaphore dependency.
- The OpenXR image index and raw command-buffer/fence slot are not reused before completion.
- Vulkano future retention and resource-use tracking remain bounded.
- Validation is clean for at least 2,000 frames, including mirror-enabled and continuously
  deforming scenes.
- Resize/session stop/restart and runtime loss do not leave fences, command buffers, or completion
  slots in an ambiguous state.
- Compare copy CPU time, final wait time, total frame time, missed intervals, and delivered FPS
  with all earlier phases.

## Phase 5: evaluate deeper pipelining only with evidence

After Phase 4, use timestamps and CPU spans to decide whether further work is justified.
Candidates include:

- multiple completion-tracked offscreen eye target generations;
- multiple raw copy command-buffer/fence slots keyed by OpenXR image index;
- acquiring more than one OpenXR image when the frame-loop design can usefully support it;
- direct rendering into OpenXR array layers to remove the offscreen copy (completed 2026-10-20);
- timeline semaphores for internal renderer milestones;
- a runtime- or extension-specific asynchronous release handoff, only when explicitly supported
  and verified.

Do not move the final completion wait past `xrReleaseSwapchainImage` based only on successful
testing on one runtime.

### Phase 5 verification

- State the measured bottleneck that motivates the selected experiment.
- Add an ownership diagram for every new slot domain.
- Verify the runtime-supported synchronization contract before relying on it.
- Compare against Phase 4, not only against the original serialized baseline.
- Revert the added complexity if it does not improve missed-frame rate or tail frame time.

## Exceptional recovery matrix

Exercise each of these independently:

| Event | Required result |
|---|---|
| Vulkano eye or mirror submission failure | Stop the batch; do not copy or release incomplete content as a successful frame |
| Raw copy submission failure | Do not treat the fence/slot as complete; recover queue/device state explicitly |
| Fence wait failure or device loss | Surface the failure and rebuild or end the XR session |
| OpenXR acquire/wait failure | Submit no writes to an unavailable image |
| OpenXR release failure | Preserve visible error state and follow session-loss handling |
| Session stop/restart | Complete or explicitly abandon tracked work before destroying XR resources |
| Format mismatch clear fallback | Use a fence rather than queue idle and apply the same release rule |

Exceptional `queue_wait_idle` or `device.wait_idle` calls are acceptable during proven recovery.
Count and trace them so they cannot silently enter steady-state rendering.

## Final acceptance criteria

- Mirrors, both eyes, and the XR copy are GPU-ordered without intermediate CPU waits.
- Normal format-compatible frames use one CPU completion wait before OpenXR image release.
- Normal rendering uses no queue-idle or device-idle wait.
- Both eyes use the correct view and projection and present without corruption or flicker.
- Mirror work is scheduled once per required logical capture.
- Shared deformation and runtime-texture resources remain ordered through
  `submission_future`.
- OpenXR images, offscreen targets, command buffers, fences, and future nodes have explicit,
  bounded ownership.
- Vulkan synchronization and resource-use validation are clean for at least 2,000 headset frames
  in mirror-off and mirror-on presets.
- Session stop/restart and failure recovery are validation-clean.
- Reports show queue submissions, CPU waits, queue-idle waits, and exceptional waits per frame.
- The before/after report demonstrates improved tail frame time, missed-frame rate, delivered FPS,
  or clearly documents why submission waits were not the limiting factor.
