# Desktop OpenXR renderer comparison

Select `lean` (the new classic-pass opaque renderer) or `desktop` (the existing
Mittens Vulkano scene renderer). Both run through **the same host-owned OpenXR
session/frame loop**, the runtime's recommended stereo swapchain size, the same
custom material and cube grid, 1x sampling, and direct XR output. Neither run
creates a window, mirror, bloom, or a second engine-owned XR session.

From the repository root, with your desktop OpenXR runtime/headset running:

```sh
scripts/run_xr_renderer_compare.sh --renderer desktop --output /tmp/xr-desktop-90.csv
scripts/run_xr_renderer_compare.sh --renderer lean --output /tmp/xr-lean-90.csv
```

Each run waits for visible, valid eye poses and compiled custom materials, then
warms up for 3 seconds and records 20 seconds. The grid has 144 cubes sharing
one custom material instance. Increase `--grid 24` identically in both runs for
576 cubes. Keep runtime resolution scale, refresh, reprojection settings, scene,
grid size, and other GPU workloads constant. Run both orders to check for
warmup/thermal/runtime hysteresis. The script builds in release mode; debug
builds are unsuitable for drawing performance conclusions.

The helper honors `CMAKE` and otherwise uses the SDK's CMake 3.30.3 if installed.
On other machines, provide a CMake version compatible with the engine's locked
shaderc-sys 0.8.3 (this checkout cannot build it with the system CMake 4.4).

For 75 Hz, set the headset's refresh rate in the runtime, then run both again:

```sh
scripts/run_xr_renderer_compare.sh --renderer desktop --budget-hz 75 --output /tmp/xr-desktop-75.csv
scripts/run_xr_renderer_compare.sh --renderer lean --budget-hz 75 --output /tmp/xr-lean-75.csv
```

`--budget-hz` labels the intended frame budget; it does **not** change the
headset refresh rate. Other options: `--seconds N`, `--warmup N`, `--grid N`
(1..64), `--startup-timeout N` (default 60 seconds), and `--help`. No rendered
scene before the startup timeout requests session stop and produces no FPS
result. Both use recommended per-eye resolution with no 1024px
Android probe cap. The shared swapchain constructor currently takes its extent
from the first stereo view, as the existing engine does. The exact extent,
format, runtime, build profile, and renderer are printed at startup.

## Interpreting results

The terminal prints application submission Hz and active-frame p50/p95/p99,
plus the number of frames over the selected budget (11.11ms at 90Hz, 13.33ms
at 75Hz). CSV includes:

| Column | Meaning |
| --- | --- |
| `wait_frame_ms` | Time blocked in `xrWaitFrame`; excluded from active-frame work. |
| `update_ms`, `prepare_ms` | Engine update and shared upload/draw preparation. |
| `render_and_gpu_complete_ms` | CPU recording, queue submission, and fence completion for both eyes. **Not a GPU timestamp.** |
| `end_frame_ms` | Projection-layer submission to OpenXR. |
| `active_frame_ms` | Wall time after `xrWaitFrame` returns through `xrEndFrame`, including image acquisition/wait. |
| `app_interval_ms` | Wall time between rendered application iterations. |
| `predicted_period_ms`, `predicted_step_ms` | Runtime-reported application display period and predicted timestamp progression. Neither proves native refresh. |

Use the runtime's frame-timing overlay to confirm new presented frames,
reprojection and dropped frames. A 45/90 or 37.5/75 pattern is consistent with
half-rate pacing, but a timing report alone does not establish its cause.
The [Khronos frame submission guide](https://github.com/KhronosGroup/OpenXR-Guide/blob/main/chapters/frame_submission.md)
explains frame pacing and why application work must fit the native budget.

The small scene isolates the renderer; it does not reproduce avatar skinning,
tracking, physics, scripting or mirror costs in larger demos. The new backend
deliberately supports fewer features. A speedup is not evidence that classic
render passes or hardware tiling alone caused it. No speedup is claimed until
both complete on the same desktop runtime/headset and their output is checked.

The intended headset workload later includes skinning, morph targets,
anime-complexity custom shaders and mirrors, targeting at least 90Hz. Bloom and
transmission can stay disabled. Extend this comparison with those representative
features as they land; a static cube result does not establish the eventual
animated/mirrored workload's cadence.

## Prototype source layout

This executable directly compiles `android/src/renderer/` and the common Ash
device bootstrap, so desktop exercises the actual new backend, not a copy.
Android/JNI/lifecycle code is excluded. Its host modules live here; the existing
renderer is initialized through its public two-stage XR graphics/scene API.
If retained beyond this comparison, move the portable renderer/device contract
to a shared engine module or library rather than making the engine depend on
the Android application. Selection currently applies to this demo only.

The shared [GPU smoke harness](../../android/validation/renderer-smoke/) verifies
stereo pixels, depth, material parameter revisions, batching and stable resource
caches without a headset. Headset validation and actual performance measurement
remain separate.
