# Renderer CPU-time complexity (Vulkano futures, flush, fences)

When profiling the engine, you may see time dominated by calls like:

- `GpuFuture::then_signal_fence_and_flush`
- `GpuFuture::cleanup_finished`

This document explains what that typically means, where we call these today, and what options exist to reduce wall-time and/or move work off the main thread.

## TL;DR

- Yes: those methods are in **Vulkano** (they’re methods on the `GpuFuture` trait).
- `then_signal_fence_and_flush()` does **submission work** (and driver work via `vkQueueSubmit`). It may also appear “expensive” if it performs validation/tracking or hits internal locks.
- If you ever follow it with `.wait(...)`, the samples can include **time waiting for the GPU** (blocking in a fence wait).
- `cleanup_finished()` walks future chains and polls fences with a zero-timeout; it is necessary for releasing resource locks and memory, but can become noticeable if you have many futures, many in-flight frames, or long chains.
- Multi-threading can help with **command buffer recording** and **decoupling render submission from simulation**, but it cannot eliminate driver submission costs or GPU waits; it can only hide them.

## Where we call these in cat-engine

### Window swapchain rendering

We submit the per-frame command buffer and keep the resulting `GpuFuture` around in `images_in_flight`. We do not block the frame on a wait here, but we do call `cleanup_finished()` each frame to allow Vulkano to release resources.

Call sites:

- `render_visual_world` submission:
  - `then_signal_fence_and_flush()` in `src/engine/graphics/vulkano_renderer.rs`
- `cleanup_finished()` on in-flight futures:
  - per-frame loop in `src/engine/graphics/vulkano_renderer.rs`

This design is the “normal” Vulkano pattern: submit, keep a future per swapchain image, periodically call `cleanup_finished()`.

### XR offscreen rendering

In XR offscreen rendering, we currently do:

```
... then_signal_fence_and_flush()? .wait(None)?
```

That `.wait(None)` is a **full GPU sync point** for that submission. If this runs per-frame (or per-eye per-frame), it can dominate CPU profiles even if the CPU is mostly blocked.

### Texture uploads

Our texture upload helpers also perform a flush + wait:

- `src/engine/graphics/vulkano_texture_upload.rs`

We also do the same pattern for mesh uploads (staging -> device-local buffer copy):

- `src/engine/graphics/vulkano_renderer.rs` (`upload_mesh`)

This is fine for “load at startup”, but it will be very noticeable if used during gameplay frames or in bursts.

## What Vulkano is doing (conceptually)

### `then_signal_fence_and_flush()`

In Vulkano 0.35.x this is implemented as:

- `then_signal_fence()` → wraps the previous future with a `FenceSignalFuture`
- `flush()` on that future → causes the submission chain to be turned into an actual `vkQueueSubmit`

Key point: it’s not “just a method call”. It is usually the point at which we:

- build the final submission list (semaphores, waits, command buffers)
- take/track resource locks for correctness (buffer/image usage tracking)
- call into the Vulkan driver to submit work

If you profile CPU time inside this call, it can be:

- CPU overhead from Vulkano’s tracking + Rust-side work
- driver overhead in `vkQueueSubmit`
- (indirectly) time waiting on driver-internal synchronization

### `cleanup_finished()`

Vulkano futures form a chain. `cleanup_finished()`:

- tries to non-blockingly check if the fence is already signaled (`wait(timeout=0)` internally)
- if signaled, it “signals finished” on previous futures and drops state
- otherwise it recurses into the previous future so resources can be released as soon as possible

This keeps memory bounded and prevents resources from remaining “locked” forever.

If `cleanup_finished()` is hot:

- you may be calling it very often (many futures)
- your future chains may be long
- you may have lots of in-flight submissions
- each call may still involve syscalls/driver calls for fence status queries

## Interpreting the profiler output correctly

A flamegraph that shows `then_signal_fence_and_flush` at the top does **not** automatically mean “Rust is doing tons of work there”. It can also mean:

- you called `.wait(...)` shortly after and the thread is blocked
- the GPU is the limiting factor and the CPU is waiting at the sync boundary

How to disambiguate:

- In `perf report`, look for time in syscalls like `futex`, `nanosleep`, or driver calls.
- Compare with GPU utilization (e.g. `nvidia-smi dmon`, `intel_gpu_top`, etc.).
- Temporarily remove (or gate) `.wait(None)` in XR paths and see if the “CPU time” shifts.

## Can we multi-thread `VulkanoRenderer`?

### 1) Put the renderer on a dedicated thread (high value)

Goal: hide submission/driver time behind simulation work.

Approach:

- Run simulation/ECS on main thread.
- Send a compact per-frame “render packet” to a render thread (camera matrices, draw batches, GPU handles).
- The render thread builds the command buffers and submits.

Pros:

- big wins if the main thread is currently blocked in rendering submission
- cleaner frame pacing if simulation is heavy

Cons:

- requires designing a snapshot/transfer boundary for `VisualWorld`
- requires careful lifetime management of GPU handles (but most Vulkano handles are `Arc<...>`)

This does **not** reduce total CPU cycles spent in `vkQueueSubmit`, but it can reduce *frame time* on the simulation thread.

### 2) Parallelize command buffer recording (medium/high complexity)

Vulkan supports multi-threaded command buffer recording. Vulkano can support this too, as long as you record separate command buffers in separate builders.

Typical pattern:

- Record multiple **secondary** command buffers in parallel (per-pass, per-layer, per-tile, etc.).
- Record a small primary command buffer that just executes those secondary buffers.

Potential blockers:

- shared allocators/caches inside Vulkano may serialize via locks
- descriptor set allocation and pipeline layout caching can contend
- your renderer architecture must support splitting work into independent chunks

This can reduce the “build command buffers” portion, but will not reduce submission cost.

### 3) Multi-threading doesn’t help if we are *waiting*

If the hot path is actually `.wait(None)` (or a fence wait hidden in a future behavior), then multi-threading can only help by:

- moving that wait off the simulation thread
- increasing pipelining (more frames in flight) to avoid waiting entirely

But if you truly must wait before proceeding (hard sync requirement), you can’t outrun it.

## Quick mitigations to try first

1) Avoid per-frame `.wait(None)` in XR rendering

- Instead, submit work and keep a future like we do for the window swapchain.
- Only wait if/when the CPU actually needs the result (e.g. readback).

2) Batch uploads (and don’t upload mid-frame)

- Gather texture uploads and submit them together.
- Consider a background loader thread that produces staging buffers; submit on render thread.

3) Reduce the number of submits per frame

- Fewer queue submits means fewer calls through `then_signal_*_and_flush`.
- Combine passes where possible.

4) Keep “frames in flight” bounded and consistent

- Our swapchain path already does per-image futures; keep it at swapchain image count.
- Don’t create additional long-lived futures unnecessarily.

## Next investigation steps (actionable)

- Confirm whether XR rendering is calling `.wait(None)` per-eye per-frame, and measure impact by temporarily gating that wait.
- Count queue submissions per frame (window + XR + uploads) and compare to CPU hotspots.
- If `cleanup_finished()` remains hot with only ~3 in-flight futures, inspect contention (locks in Vulkano allocators) and consider batching/deferring cleanup (e.g. cleanup once per N frames) as an experiment.

## Appendix: “where in Vulkano are these implemented?”

In Vulkano 0.35.x, the strings appear under:

- `vulkano/src/sync/future/mod.rs` (trait + `then_signal_fence_and_flush`)
- `vulkano/src/sync/future/fence_signal.rs` (fence future + cleanup logic)

You can locate them locally with ripgrep over your Cargo registry.
