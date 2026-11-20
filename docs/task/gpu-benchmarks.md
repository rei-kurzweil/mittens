# GPU benchmarks

Running log of GPU benchmarks: what exists, how to run each one, and every
recorded result. Add a row per run; never overwrite old rows. Record the
commit, machine, and anything that differs from the defaults (resolution,
MSAA, foveation, thermal state, build profile).

Task-specific design notes live in the linked task docs; results live here.
To hand a run to an agent on the GPU/headset machine, give it the
[GPU benchmark runbook](gpu-benchmark-runbook.md).

## Machines

| Name | GPU | Driver / OS | Notes |
|---|---|---|---|
| desktop-1080 | NVIDIA GTX 1080 | NVIDIA 580.159.04 / EndeavourOS | desktop Vulkan (vulkano renderer) |
| focus3 | Adreno 650 (XR2), HTC Focus 3 | _fill in_ | Android renderer, classic passes, two eye passes |

## Benchmarks

| Benchmark | Platform | Task doc | Command |
|---|---|---|---|
| Noise: texture vs procedural | desktop | [perlin noise GPU benchmark](perlin-noise-gpu-benchmark.md) | `cargo test --release --lib noise_profile -- --ignored --nocapture` |
| Noise: texture vs procedural | focus3 | [perlin noise GPU benchmark](perlin-noise-gpu-benchmark.md) | see [XR2 noise run](#xr2-noise-run) |
| Twilight moon: sampled vs analytic | desktop | — | `cargo test --release --lib profile_twilight -- --ignored --nocapture` |
| Sakura stereo profile | desktop | — | `cargo test --release --lib profile_sakura_lumae_stereo -- --ignored --nocapture` |
| Android multiview A/B | focus3 | [Android multiview](android-multiview.md) | planned |

### What the numbers mean

- **Desktop profile harnesses** (`src/engine/graphics/vulkano_*_profile.rs`):
  GPU timestamps around the graphics pass for both eyes, including clear and
  4x MSAA resolve. Not whole-frame time; no pacing or CPU load. Desktop's MMS
  `Benchmark` API reports no GPU time (see [benchmark spec](../spec/benchmark.md)).
- **Android `Benchmark.start/stop`**: `gpu_frame_ms` is top-to-bottom-of-pipe
  timestamps spanning both eye passes of a whole frame (including resolve).
  Compare cases by their delta against a baseline case in the same run, not
  against desktop absolutes.

### XR2 noise run

```sh
# Build (from repo root; prerequisites in android/docs/host-bootstrap.md)
export ANDROID_HOME="$HOME/Android/Sdk"
CARGO_PROFILE_DEV_OPT_LEVEL=3 android/packaging/build-apk.sh --features noise-bench
adb install --no-incremental -r target/debug/apk/mitten-droid.apk
adb logcat -c
adb shell am start -W -n org.mittens.engine.dev/android.app.NativeActivity
# Put the headset on (or keep the proximity sensor covered) and hold still.
adb logcat -v threadtime 'mitten-droid:I' '*:S' | grep -E 'noise-bench|TextureSystem|ASTC'
```

The scene (`android/examples/noise-bench.mms`) settles 10 s, then runs 6 cases
× 4 rounds (3 s warmup, 6 s measured each, about 4 min total) at
1712×1712 per eye, 4x MSAA, foveation off. Each window prints
`stdio [mms] noise-bench result {...}` with `gpu_frame_ms` mean/min/max, then
`noise-bench done`. Save the full log next to the results (e.g.
`android/docs/validation/noise-bench-<date>.txt`).

## Results

### Noise: texture vs procedural, desktop

| Date | Commit | Machine | Extent/eye | flat ms | rgba8 Δ | bc7 Δ | perlin-1 Δ | fbm6 Δ | Notes |
|---|---|---|---|---|---|---|---|---|---|
| 2026-11-20 | `8e08010e` | desktop-1080 | 1920×1080 | 0.269 | 0.051 | 0.016 | 0.052 | 0.648 | Default settings |
| 2026-11-20 | `8e08010e` | desktop-1080 | 2448×2448 | 0.725 | 0.211 | 0.068 | 0.163 | 1.885 | Default settings |

Δ = `delta_vs_flat_ms` (both eyes) from `/tmp/noise-profile.json`.

### Noise: texture vs procedural, focus3

| Date | Commit | Machine | Extent/eye | flat ms | rgba8 Δ | astc4x4 Δ | astc8x8 Δ | perlin-1 Δ | fbm6 Δ | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| | | focus3 | 1712×1712 | | | | | | | |

Δ = mean `gpu_frame_ms` of the case minus `flat`, averaged over rounds.

## Work log

- 2026-11-20 — Ran the desktop noise benchmark on `8e08010e` with release defaults. Discarded the first capture after concurrent GPU use was reported. Before the rerun the GPU reported 1–3% utilization in P8. Each case contributed 480 measured stereo frames per extent. The ASTC and release rocket tests passed. The benchmark compiled without changes. [Raw rerun timings](results/noise-profile-desktop-1080-2026-11-20.json)

- 2026-11-20: Added the [runbook](gpu-benchmark-runbook.md) for agent-run benchmarks.
- 2026-11-20: Multiview A/B plan recorded in [Android multiview](android-multiview.md).
- 2026-11-20: Created. Desktop noise harness and XR2 noise scene added; neither
  has been run yet. Authoring machine has no Rust toolchain or headset, so
  both are unbuilt.
