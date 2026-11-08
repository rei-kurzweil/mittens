# Custom-first opaque ordering experiment

Status: retained as an interim policy after VR smoke testing; opaque bands pending.

Checkpoint before the change: `ae833df2` (LambertToon materials, GPU diagnostics
and profiling reports). This follow-up retains the trial as an interim policy;
near/middle/far opaque bands remain unimplemented.

## Change

Within each regular opaque stencil scope, custom programs now sort before
built-in programs. Existing material/texture/mesh grouping is retained inside
both groups. Classification uses the registered definition and checks that its
static/deformed custom program is actually active; inherited custom values do
not turn an explicit built-in program into a custom program.

Only the opaque sort key in `visual_world/draw_cache.rs` changes. No new passes,
material pipelines, per-draw allocations or MaterialBase API are introduced.
Sorting still occurs only on retained-stream rebuilds. The default application
runs the experiment, without setting a profiling environment variable.

Background, cutout, transparent and overlay ordering are unchanged. Clip entry,
source rendering and exit stay under the existing stream builder. Emission
extraction follows the opaque order, with its existing depth test and contents.
Possible coplanar ordering differences and hidden custom surfaces drawn before
nearby built-in walls are reasons to inspect multiple views before adoption.

## Layer terminology

Proposed near/middle/far opaque bands would share the normal foreground depth
buffer and retain depth tests/writes; they would be scheduling hints, not visual
layer overrides. They are not implemented in this experiment.

`Background` is a separate renderer phase. Plain background draws do not write
depth; occluded/lit background can write depth for self-occlusion. Foreground
rendering clears that depth, so background does not occlude regular opaque
geometry. Far opaque room walls are therefore different from Background.

The user's intended hints could be near for the interior of an occupied truck,
middle for its exterior, and far for room geometry. Those remain authored hints:
a wall can be closer than an exterior truck in some views. The present trial
uses material kind as a rough proxy to measure the effect before adding bands.

## Validation and measurement

- Three targeted tests cover active static/deformed custom programs, inherited
  bindings with built-in overrides, batching/cache reuse, clip operation order,
  and unchanged non-opaque phases.
- Benchmark actual Lambert truck material and existing character/tile shaders;
  no diagnostic binary-gate shader substitution or special truck priority.
- Local cafeteria, Mitten player/Miku vendor, one truck, background rooms loaded,
  mirror off; walking and driving, 1920×1920 per eye, MSAA 4x, bloom/emission on.
- Two captures per motion and revision; 240 warmup / 480 measured frames. Before
  sequence walking/driving/driving/walking, after sequence reversed. Per-pass
  timestamps and opaque GPU statistics enabled identically, native truck order.
- Keep before/after raw captures and source/executable hashes. Verify camera,
  movement, draw counts, vertex work and cached-command reuse. No live
  OpenXR/compositor/ALVR pacing is measured.


## Results (2026-11-08)

Both repetitions improve in both motion cases. The following means and p95s pool
960 frames per motion/revision; p95 is computed from all frames, not averaged
from individual-run percentiles. Times are milliseconds, fragment invocations
millions across both eyes.

| Motion | CPU update before → after | GPU graphics before → after | Elapsed mean before → after | Elapsed p95 before → after | Opaque fragments M before → after |
|---|---:|---:|---:|---:|---:|
| walking | 5.795 → 5.857 | 10.871 → 9.943 | 18.011 → 17.174 | 20.504 → 18.591 | 17.332 → 15.781 |
| driving | 6.930 → 6.963 | 15.823 → 12.038 | 24.164 → 20.397 | 26.092 → 22.260 | 26.166 → 20.070 |

Per-capture values, preserving startup/run variation:

| Revision | Motion | Repeat | CPU update | GPU graphics | GPU opaque | Opaque fragments M | Elapsed mean | Elapsed p95 |
|---|---|---|---:|---:|---:|---:|---:|---:|
| before | walking | 1 | 5.849 | 10.330 | 8.568 | 16.585 | 17.536 | 19.622 |
| before | driving | 1 | 6.936 | 15.763 | 14.037 | 26.055 | 24.110 | 26.360 |
| before | driving | 2 | 6.925 | 15.883 | 14.135 | 26.276 | 24.217 | 25.917 |
| before | walking | 2 | 5.740 | 11.412 | 9.609 | 18.079 | 18.485 | 20.687 |
| after | walking | 2 | 5.957 | 10.129 | 8.367 | 16.190 | 17.470 | 19.058 |
| after | driving | 2 | 6.944 | 12.136 | 10.368 | 20.321 | 20.479 | 21.906 |
| after | driving | 1 | 6.981 | 11.939 | 10.158 | 19.818 | 20.316 | 22.884 |
| after | walking | 1 | 5.757 | 9.757 | 8.000 | 15.372 | 16.877 | 18.156 |

The unchanged scene's camera matrices, player/truck poses, mount/power/mirror
states, draw counts, component counts and input vertex work match for every
before/after frame pair. Graphics command reuse remains two hits per frame,
zero rebuilds; there are no steady-state material allocations. Custom truck
batch positions move from roughly 285–305 to 167–187 (not necessarily first
among other custom programs). The all-custom-first policy reduces actual opaque
fragment work; its benefit is not evidence that a custom shader is intrinsically
cheaper or that custom-first is optimal from every view.

Driving's mean is below the 22.22 ms / 45 FPS budget, but its p95 remains above
that budget. These headless diagnostic timings include per-pass/statistics query
instrumentation and exclude live XR/compositor/ALVR pacing. Do not claim sustained
45 FPS on the headset, at 2144/eye, or in the full orchard variant from this test.

This supports trying authored opaque bands, rather than globally assuming that
custom material identity means near geometry. VR smoke testing should include
inside/outside the truck, views into the room from its doors, tiled passages,
characters, and windows/mirrors as applicable. Inspect coplanar surfaces as well
as speed. A later policy can keep near/middle/far depth-tested while preserving
batching and retained command reuse; no such API is added here.

## Validation and reproduction

- `cargo test --release --lib custom_first_opaque`: 3 passed.
- Release test executable filtered to `engine::graphics::visual_world`: 25 passed,
  including the new tests.
- `cargo build --release --bin mittens-engine`: passed; the VR smoke-test binary
  is `target/release/mittens-engine`.
- Eight successful real-scene stereo captures (four before, four after).
- `git diff --check`: passed. Existing compiler/asset-scan warnings remain;
  no new material compilation/pipeline errors occurred.

Run your usual ship source with the rebuilt release binary, for example:

```sh
target/release/mittens-engine load examples/local/ship_interior.mms
```

Raw ignored captures:
`examples/local/benchmarks/cafeteria-1920-{walking,driving}-custom-first-{before,after}-r{1,2}-2026-11-08.*`.
The `custom-first-opaque-{before,after}-2026-11-08.manifest.json` files retain
execution order, counters, source/executable hashes and GPU process snapshots.
The driver script is copied alongside them; source order before the experiment
is the committed file at `ae833df2`. No restricted assets are committed.

