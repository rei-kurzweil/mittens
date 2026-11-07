# Vulkan rendering optimization

Reduce CPU time spent preparing and recording rendering while preserving the
ship's appearance, bloom, MSAA 4x, mirror behavior and XR correctness. Start with
the desktop Vulkan backend and measure the actual ship scene. Android renderer
features are outside this epic's initial scope.

The immediate target is the cafeteria at 1920 x 1920 **per eye**. Sustained
45 FPS requires a 22.22 ms frame budget. Earlier measurements, before the event
binding refactor, showed 5.458 ms scene update, 0.578 ms render preparation,
7.322 ms renderer recording/submission and 12.370 ms GPU graphics across both
eyes. CPU and GPU overlap: those columns must not be added together. The
previously discussed 12.59 ms active CPU total was an estimate, not a new
end-to-end measurement.

## Work and evidence

- [Benchmark history and change ledger](benchmark-history.md): chronological
  reports, preserved captures and the implementation changes behind each stage.
- [Orchard with active physics](orchard-active-physics.md): 23.22 ms stationary /
  25.15 ms moving-head with cached graphics, native truck physics active, and
  preserved fresh-recording controls; still above the 45 FPS frame budget.
- [RoomSystem design proposal](../../room-system-distance-streaming.md): graph-hop
  distance streaming and proposed MMS syntax; not implemented.
- [CPU recording measurements](measurements.md): instrument and measure resource
  setup, draw preparation, individual recording phases, Vulkano finalization,
  queue submission and completion waits separately.
- [Inside Vulkano build](build-internals.md): synchronization analysis accounts
  for 1.990 ms and resource-usage finalization for 0.385 ms; native command
  emission takes 0.748 ms across both eyes. Includes the temporary probe and control.
- [Cache invalidation baseline](cache-invalidation-baseline.md): first counter
  slice finds two texture-driven rebuilds per stereo frame, second-eye buffer
  refreshes and recurring mutable material snapshot allocations.
- [View-local mirror bindings](view-local-mirror-bindings.md): the first cache
  fix eliminates those per-eye structural rebuilds and reduces instance-buffer
  allocations from 10 to 7 per stereo frame, preserving stereo/frozen reflections.
- [Material binding variants and retention](material-binding-variants.md): fixed
  legacy bands, a standard Surface Grid definition and a measured cache-limit
  fix eliminating 36 recurring material snapshot allocations per frame.
- [Reusable primary command buffers](reusable-primary-commands.md): retained
  eligible graphics commands with continuously updated camera/light/joint data;
  CPU recording about 7 -> 1 ms, 18.41 ms stationary and 21.55 ms moving-head
  mean offscreen frames at 1920 per eye with bloom/MSAA 4x. Includes the rejected
  bloom-only boundary and preserved controls.
- [Persistence and command reuse](persistence-and-command-reuse.md): inventory
  what already persists, what changes each frame, and the invalidation and
  lifetime rules needed to switch incrementally.
- [Next optimization slices](next-steps.md): concrete work order, acceptance
  criteria and measured targets after the build-internals capture.
- [Pass dependencies and explicit synchronization](pass-dependencies-and-synchronization.md):
  existing MaterialBase/capability contracts, image/buffer dependencies and the
  lower-level Vulkano integration decision.
- [Clipping and command reuse](clipping-and-command-reuse.md): preserve ordered
  stencil operations while compiling direct slots and reusable operation ranges.
- [Existing ship profiling report](../../ship-interior-performance-profile.md):
  previous CPU, resolution, bloom and truck/event-binding experiments.
- [Truck script power and continuous physics](../../pickup-script-power-and-continuous-physics.md):
  separate vehicle MMS activity from native motion and investigate post-visit CPU
  cost. Current parking freezes physics; keep that limitation visible in comparisons.

## Intended outcome

Build stable draw lists less often, retain reusable GPU resources, and avoid
rerecording unchanged command sequences. Keep current camera/pose data fresh.
Every rendered frame still submits GPU work and draws the current scene; command
reuse does not mean skipping unchanged objects in a freshly cleared image.
Reducing draw calls or queue submissions is a separate measured opportunity.

The renderer already has draw-stream and instance-buffer caches. This is an
incremental extension of existing behavior, not a replacement of every rendering
path or a presumption that all lists are rebuilt every frame.

## Slices

1. **Measurement baseline (completed).** Split recording CPU timings, record
   a post-event-binding cafeteria run, retain raw samples, and reconcile stage
   totals. Split Vulkano's final build into internal phases with an isolated
   dependency probe. No command caching or scheduling changes in this slice.
2. **Persistence inventory (drafted).** Classify buffers, descriptors, pipelines, draw
   streams, command buffers and attachments. Define dirty generations and
   frame-in-flight ownership before changing reuse.
3. **Invalidation counters and retained pass plan.** Count actual stream/cache
   rebuilds and compile current material/configuration dependencies into pass
   resource roles. Preserve view-dependent sorting and clip order.
4. **Stable resources and recorded-command reuse prototype.** Retain per-slot
   bindings for a small candidate. Compare compatible full primary reuse with
   retained secondaries, including their remaining parent analysis cost.
5. **Explicit synchronization feasibility and pilot.** Resolve lower-level
   recording/submission ownership, then validate a small renderer-owned pass
   chain. Extend to opaque/deformation work after correctness and timing evidence.
6. **Clip-program compilation.** Measure a clipping-heavy fixture, precompute
   stream-local slots and evaluate balanced reusable ranges. Preserve current
   masking behavior; the cafeteria capture has no measured stencil-mask draws.
7. **Parallel recording experiment, if still justified.** Use independent worker
   command pools and substantial secondary-command-buffer batches. Measure CPU
   savings, scheduling overhead and GPU effects before adopting it.
8. **Stereo/submission opportunities.** Evaluate redundant per-eye setup and
   submission counts separately. Multiview or different frame scheduling would
   need their own explicit design and correctness checks.

Each slice should produce an independently reviewable change and before/after
measurements. Keep extracted modules focused and below the repository's source
file size targets. Preserve direct/batched hot paths; splitting modules must not
add per-draw allocation or resource creation.

The detailed [work order](next-steps.md) separates measurement, enabling resource
persistence, reuse and engine-owned synchronization. It supersedes the initial
allocation-first ordering: camera/light/global setup measured only 0.030 ms,
while synchronization analysis/finalization measured about 2.376 ms.

## Validation

Use the same scene, viewpoint, resolution, MSAA, bloom configuration, warmup and
sample counts. Record GPU model and sample distribution as well as averages.
Report scene update, preparation, recording, submission, GPU and completion wait
separately. Offscreen timings exclude headset pacing, compositor and ALVR.

Check normal rendering, moving camera, animated avatars, streaming/unloading,
destruction, material/texture changes, frozen/enabled mirrors and resize/MSAA
changes as they become relevant to a slice. Do not change rendering quality to
claim a CPU optimization. Confirm frame-in-flight safety before retaining or
mutating resources referenced by reusable commands.
