# Opt-in System, MMS, Vulkano, and XR Profiling

Date: 2026-07-23

## Summary

Add a `profiling` Cargo feature that compiles all instrumentation out of normal builds.
Profiling builds use `MITTENS_PROFILE` to select hierarchical areas or individual systems:

```bash
MITTENS_PROFILE=systems.secondary_motion,systems.skinned_mesh,systems.avatar_control,systems.transform_stream,systems.ik,mms,render,xr \
  cargo run --release --features profiling --example …
```

A macro should keep CPU instrumentation consistent, but remain a thin facade over a shared
profiler. Vulkan GPU timestamps require dedicated command-buffer and query-pool integration
rather than the CPU timing macro.

## Implementation changes

### Shared profiling facility

- Add a small internal `mittens-profile` workspace crate, included as an optional dependency by
  the engine and `meow-meow-script`.
- Provide RAII CPU scopes, counters, thread-safe aggregation, and periodic snapshots.
- Track calls, total time, average time, per-frame time, and maximum time.
- Aggregate MMS worker-thread and main-thread measurements together.
- Print live summaries to stderr and retain the engine's `docs/.debug` profile logs.

Add local facade macros such as:

```rust
profile_scope!("systems.secondary_motion");
profile_counter!("mms.host_call.spawn", 1);
```

- Under `#[cfg(feature = "profiling")]`, the macros create guards or update counters.
- Without the feature, they expand to nothing and do not evaluate their arguments.
- Cache `MITTENS_PROFILE` once per process.
- Selecting a parent such as `systems`, `mms`, `render`, or `xr` enables its descendants.
- Support `MITTENS_PROFILE=all`.

### ECS systems

- Instrument every top-level `SystemWorld` system invocation, including repeated stages.
- Include secondary motion, skinned meshes, avatar control, transform stream, transform
  propagation, and all IK systems explicitly.
- Distinguish repeated stages where useful, such as pre-pose and post-pose transform/skinning
  passes.
- Include signal processing and command-queue flushes so orchestration overhead is visible.
- Report both call averages and total time per frame, since some systems run more than once.
- Replace the current ad hoc system, XR, and spatial timers with the shared facility.
- Preserve secondary-motion correctness counters used by tests; move its timing counters behind
  the profiling feature.

### Meow Meow Script

Instrument both MMS execution paths:

- Standalone `meow-meow-script`:
  - tokenization
  - parsing
  - evaluation
  - component materialization
  - session evaluation
  - callback invocation
- Engine integration:
  - parsing
  - AST transforms
  - evaluation
  - host-call round trips, separated by call kind
  - runtime closures
  - signal handlers
  - component-tree spawning

Keep profiling at phase and host-call granularity rather than timing every AST expression.

### Vulkano and OpenXR CPU timing

Add CPU scopes for:

- swapchain acquisition and recreation
- pending runtime-texture updates
- mirror captures
- draw-cache and render preparation
- command-buffer construction
- window submission and presentation
- XR frame waiting
- XR tracking and input
- XR swapchain image acquisition
- per-eye rendering
- copying into the XR swapchain
- XR frame submission

### Vulkan GPU timing

- Use Vulkan timestamp query pools for the window render, mirror captures, each XR eye, and XR
  swapchain copies.
- Maintain a bounded ring of query slots for asynchronous window frames.
- Read completed window results without blocking; if every slot remains in flight, skip that
  frame's GPU sample rather than stalling rendering.
- Read XR eye and copy results after their existing synchronous completion points.
- Convert timestamp deltas using the physical device's timestamp period.
- Label CPU and GPU measurements distinctly in summaries.
- If timestamp queries are unsupported, emit one diagnostic and continue with CPU profiling.
- Initially measure major render submissions, not every material, draw call, or shader stage.

## Interfaces and configuration

Add these Cargo features:

- `mittens-engine/profiling`, forwarding to `meow-meow-script/profiling`
- `meow-meow-script/profiling` for standalone consumers

Examples:

```bash
MITTENS_PROFILE=all
MITTENS_PROFILE=systems
MITTENS_PROFILE=systems.secondary_motion,mms,xr
```

Keep `CAT_PROFILE_SYSTEMS` and `CAT_PROFILE_SPATIAL` as temporary compatibility aliases in
profiling builds. Document `MITTENS_PROFILE` as canonical.

This work does not change ECS, scripting-language, renderer, or application APIs.

## Reporting behavior

- Engine summaries report approximately every 120 rendered frames, matching the existing
  profiling cadence.
- Standalone MMS entry points report after each top-level evaluation so short-lived tools still
  produce results.
- Profiling is selected per process launch. Changing selectors while the application is running
  is out of scope.
- Normal builds have compile-time-zero profiling overhead.

## Test plan

- Test selector parsing, parent-category matching, `all`, invalid or empty selectors, and legacy
  aliases.
- Test scope recording across normal returns, early returns, errors, nesting, and multiple
  threads.
- Verify disabled macros do not evaluate their arguments and the optional profiler dependency is
  absent from default builds.
- Test snapshot aggregation and reset behavior.
- Test timestamp conversion and query-slot recycling with mocked results.
- Run default and profiling-feature checks and tests for the workspace and standalone MMS crate.
- On Vulkan hardware, verify individual systems can be selected independently and CPU/GPU window
  measurements are distinct.
- On OpenXR hardware, verify MMS, XR orchestration, per-eye GPU rendering, and XR-copy timing
  appear without changing rendered output.
- Verify unsupported GPU timestamps or unavailable XR do not prevent normal rendering.
