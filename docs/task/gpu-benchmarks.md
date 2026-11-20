# GPU benchmarks

Running log of GPU benchmarks: what exists, how to run each one, and every
recorded result. Add a row per run; never overwrite old rows. Record the
commit, machine, and anything that differs from the defaults (resolution,
MSAA, foveation, thermal state, build profile).

Task-specific design notes live in the linked task docs; results live here.

## Machines

| Name | GPU | Driver / OS | Notes |
|---|---|---|---|
| desktop-1080 | NVIDIA GTX 1080 | _fill in_ | desktop Vulkan (vulkano renderer) |
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
| | | desktop-1080 | 1920×1080 | | | | | | |
| | | desktop-1080 | 2448×2448 | | | | | | |

Δ = `delta_vs_flat_ms` (both eyes) from `/tmp/noise-profile.json`.

### Noise: texture vs procedural, focus3

| Date | Commit | Machine | Extent/eye | flat ms | rgba8 Δ | astc4x4 Δ | astc8x8 Δ | perlin-1 Δ | fbm6 Δ | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| | | focus3 | 1712×1712 | | | | | | | |

Δ = mean `gpu_frame_ms` of the case minus `flat`, averaged over rounds.

## Work log

- 2026-11-20: Created. Desktop noise harness and XR2 noise scene added; neither
  has been run yet. Authoring machine has no Rust toolchain or headset, so
  both are unbuilt.
