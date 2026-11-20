# GPU benchmark runbook (agent handoff)

Step-by-step instructions for an agent on a machine with the GPU and/or the
Focus 3 over adb. Results and the benchmark list live in
[GPU benchmarks](gpu-benchmarks.md); design notes in
[perlin noise GPU benchmark](perlin-noise-gpu-benchmark.md).

The desktop harness, ASTC decoder test and rocket tests passed on the GTX 1080
host at `8e08010e`. Record any fixes in the benchmark work log. Commit directly
to `main`.

Benchmark implementation files:

| Piece | Files |
|---|---|
| Desktop noise harness | `src/engine/graphics/vulkano_noise_profile.rs` (included from `vulkano_sakura_profile.rs` as `mod noise`) |
| ASTC loader | `src/engine/ecs/system/texture_system.rs` (`decode_astc`), `src/engine/ecs/component/texture.rs`, `TextureUploader::upload_texture_astc` in `src/engine/graphics/mod.rs` |
| Android ASTC upload | `android/src/renderer/resources.rs` (`upload_astc`, `upload_levels`), `android/src/renderer/mod.rs`, `android/src/graphics_device.rs` (`astc_ldr` feature) |
| Android stdio → logcat | `android/src/activity.rs` (`forward_stdio_to_logcat`, uses `libc`) |
| Android bench scene | `android/src/demo_assets.rs` (`DemoScene::NoiseBench`), `android/src/frame_renderer.rs`, `android/examples/noise-bench.mms` |
| Rocket trail | `assets/components/projectiles/rocket.mms`, `src/scripting/rocket_motion_tests.rs` |

## 0. Sync and sanity check

```sh
git pull --ff-only
git log --oneline -1          # record this commit in every result row
ls assets/images/noise/perlin-1024.png \
   assets/textures/noise/perlin-1024.dds \
   assets/textures/noise/perlin-1024-astc4x4.astc \
   assets/textures/noise/perlin-1024-astc8x8.astc
```

All four files are committed. Regenerate only if missing
(see `assets/textures/noise/README.md`).

## 1. Unit tests (any machine with cargo)

```sh
cargo test --lib astc_tests
cargo test --lib rocket_
```

Both must pass before benchmarking. The rocket tests check that the projectile
owns a `sta_rocket_trail` child and that hits/expiry still work.

## 2. Desktop benchmark (GTX 1080)

```sh
cargo test --release --lib noise_profile -- --ignored --nocapture
```

- Runs 5 cases (`flat`, `texture-rgba8`, `texture-bc7`, `perlin-1-octave`,
  `perlin-fbm`) at 1920×1080, 1920×1920 and 2448×2448 per eye, 4x MSAA, both eyes.
- Writes `/tmp/noise-profile.json`. Env overrides: `NOISE_PROFILE_OUTPUT`,
  `NOISE_PROFILE_WARMUP` (24), `NOISE_PROFILE_FRAMES` (80),
  `NOISE_PROFILE_REPEATS` (6), `NOISE_PROFILE_OCTAVES` (6).
- Copy the JSON to `docs/task/results/noise-profile-desktop-1080-<date>.json`.
- Fill a row per extent in **Noise: texture vs procedural, desktop** in
  [GPU benchmarks](gpu-benchmarks.md), using `delta_vs_flat_ms`.
- Fill the desktop-1080 driver version in the Machines table
  (`nvidia-smi` or `vulkaninfo --summary`).

### GTX 1080 findings and texture audit

The 2026-11-20 rerun used driver 580.159.04 and 480 measured stereo frames per
case at each extent. Flat draws a constant gray sphere and includes the graphics
pass clear and 4× MSAA resolve. The other columns give additional milliseconds
over flat for both eyes together. [Raw timings](results/noise-profile-desktop-1080-2026-11-20.json)

| Extent per eye | Flat (ms) | RGBA8 Δ | BC7 Δ | One octave Δ | Six octaves Δ |
|---|---:|---:|---:|---:|---:|
| 1920×1080 | 0.269 | 0.051 | 0.016 | 0.052 | 0.648 |
| 2448×2448 | 0.725 | 0.211 | 0.068 | 0.163 | 1.885 |

At the usual 1920×1920 XR extent, a separate run measured 0.454 ms flat.
RGBA8 added 0.121 ms, BC7 added 0.035 ms, one octave added 0.101 ms, and
six octaves added 1.168 ms for both eyes. [XR extent timings](results/noise-profile-desktop-1080-xr-square-2026-11-20.json)

At the two extents in the table, BC7 saved 0.035 and 0.143 ms respectively over RGBA8 for one full-screen
sample per fragment. One octave and RGBA8 cost about the same at 1080p. At the
larger extent one octave was cheaper than RGBA8. Both texture formats beat six
octaves. Octaves two through five and Android ASTC need their own measurements.

The following static inventory uses source references and image dimensions.
Sizes are RGBA8 base-level estimates before mipmaps, per asset if loaded.
Actual frame cost depends on visible coverage and sampling.

| Where RGBA8 is used | Base-level size | Why it matters |
|---|---:|---|
| Embedded images in the PC Rei hoodie, Lumae, and Mitten GLBs | 196, 192, and 81 MiB per model | RGBA8 is the fallback. The importer can use per-image sidecars generated below. Bisket also contains 81 MiB. [Importer](../../src/engine/ecs/system/gltf_system.rs) |
| Optional local rooftop roster assets | Succubus 124 MiB, Miku 41 MiB | Based on the local GLBs available during this audit. |
| Midnight Alleyway panorama | 83.3 MiB | The 11369×1920 JPEG is one large standalone candidate. [Scene](../../examples/midnight-alleyway.mms) |
| Sky cloud mask, grass, pool tiles, chrome matcap, cherry blossoms | 2 MiB sky, 1 MiB each for grass, tiles and matcap, 0.25 MiB blossoms | Skies and tiled surfaces can cover much of an eye. Check packed normals and alpha-cutout foliage for artifacts at a distance. |
| Grocery label and lid, muzzle flashes, pickup tread | 2 and 1 MiB for groceries, 4 MiB per flash, 0.125 MiB for tread | The four tires share one uploaded image by URI. [Texture loading and cache](../../src/engine/ecs/system/texture_system.rs) |

Start desktop A/B tests with the panorama or a high-coverage tiled surface.
For avatar comparisons, bake a model’s embedded images with the command below. Preserve color space, alpha, packed data channels
and mip filtering when comparing assets. The current RGBA8 upload builds a mip
chain, while the BC7 and raw ASTC upload paths use a single level. Desktop font
atlases and the noise benchmark already have DDS assets. Render targets and
runtime-updated images have different write requirements from static sampled
art. [Desktop upload](../../src/engine/graphics/vulkano_texture_upload.rs) ·
[Android upload](../../android/src/renderer/mod.rs)

### Bake GLB textures for desktop and Android

From the repository root, create a local virtual environment and install the encoder once.

```sh
python -m venv tools/texture/venv
tools/texture/venv/bin/python -m pip install Pillow ispc_texcomp==1.0.1
```

Run the baker with that interpreter.

```sh
tools/texture/venv/bin/python scripts/compress_glb_textures.py assets/models/mitten.glb
```

Pass multiple GLBs or use `--format bc7` or `--format astc` for a single
target. Keep the resulting `mitten.glb.textures/` directory beside the GLB
when deploying, including on Android. Each filename includes the image index
and a checksum of the embedded PNG or JPEG. Re-run the baker after re-export.
The importer falls back to the embedded image if the matching sidecar is
missing or its upload fails. The optional files add disk space and the importer
still decodes the original images on the CPU. Both generated formats use one
16-byte block per 4×4 texels, with the same linear UNORM sampling as the
existing RGBA8 upload. Check distant surfaces for aliasing with single-level
textures.

### Avatar texture memory and timing

These are texel and block payloads calculated from the embedded image
dimensions and generated sidecars. BC7 and ASTC 4×4 have the same payload size.
The full-mip compressed column is a size estimate for a future upload path.

| GLB | RGBA8 base | RGBA8 full mip chain | Current compressed base | Compressed full mip chain |
|---|---:|---:|---:|---:|
| Mitten | 81 MiB | 108 MiB | 20.25 MiB | 27 MiB |
| Lumae | 192 MiB | 256 MiB | 48 MiB | 64 MiB |
| Combined | 273 MiB | 364 MiB | 68.25 MiB | 91 MiB |

The current upload paths save 295.75 MiB of texture payload combined, about
81%. Comparing equal mip coverage gives about 75% savings.

The Sakura stereo fixture uses real scene draws at 1920×1920 per eye with
4× MSAA in texture comparison mode. It reports image count and the sum of
Vulkan image memory requirements in bytes by format. The sums cover textures
in the renderer's texture table. Render targets, other resources and allocator
pool overhead are outside those totals. The test-only toggle skips GLB
sidecars while retaining the scene's existing standalone texture formats.

```sh
SAKURA_PROFILE_COMPARE_TEXTURES=1 SAKURA_PROFILE_LUMAE=1 \
SAKURA_PROFILE_WARMUP=240 SAKURA_PROFILE_FRAMES=160 \
SAKURA_PROFILE_RGBA_ONLY=1 SAKURA_PROFILE_OUTPUT=/tmp/sakura-rgba.json \
cargo test --release --lib profile_sakura_lumae_stereo -- --ignored --nocapture

SAKURA_PROFILE_COMPARE_TEXTURES=1 SAKURA_PROFILE_LUMAE=1 \
SAKURA_PROFILE_WARMUP=240 SAKURA_PROFILE_FRAMES=160 \
SAKURA_PROFILE_OUTPUT=/tmp/sakura-bc7.json \
cargo test --release --lib profile_sakura_lumae_stereo -- --ignored --nocapture
```

GTX 1080, driver 580.159.04, at `dfe36eff` plus the sidecar and profiler
changes. Each result is the median of three runs of 160 measured frames.
Mitten is loaded behind the camera in both cases. Lumae is visible. With
Lumae present, both formats produced the same four NPC draw calls in the
sample frame and 320 deformation jobs over each measured run.

| Scene | Image requirements, RGBA8 → BC7 | Saved | Stereo GPU graphics, RGBA8 → BC7 | Scene load and upload, RGBA8 → BC7 |
|---|---:|---:|---:|---:|
| Sakura + Mitten | 148.46 → 60.66 MiB | 87.80 MiB | 4.152 → 4.155 ms | 1.22 → 0.85 s |
| Sakura + Mitten + Lumae | 404.47 → 108.66 MiB | 295.81 MiB | 4.161 → 4.256 ms | 2.71 → 1.28 s |

The stereo graphics timings overlap across repetitions. With Lumae, the
run medians span 4.14–4.46 ms for RGBA8 and 4.16–4.37 ms for BC7. The
measured gains are in texture memory and startup. Lumae accounts for
208.01 MiB of the measured savings. The test measures the current paths
with and without a mip chain. Android ASTC figures above are payload
estimates, pending a headset run.

## 3. XR2 benchmark (Focus 3 over adb)

Prerequisites: [Android host bootstrap](../../android/docs/host-bootstrap.md)
(aarch64-linux-android target, SDK/NDK, CMake, cargo-apk fork).

```sh
adb devices -l                       # must say "device"
export ANDROID_HOME="$HOME/Android/Sdk"
CARGO_PROFILE_DEV_OPT_LEVEL=3 android/packaging/build-apk.sh --features noise-bench
adb install --no-incremental -r target/debug/apk/mitten-droid.apk
adb logcat -c
adb shell am start -W -n org.mittens.engine.dev/android.app.NativeActivity
adb logcat -v threadtime 'mitten-droid:I' 'AndroidRuntime:E' 'libc:F' '*:S' \
  | tee android/docs/validation/noise-bench-<date>.txt
```

- Ask the human to put the headset on (or cover the proximity sensor) and
  hold still for the whole run. If nobody is wearing it, the XR session may
  not reach focused/visible and no frames will be timed.
- About 4 minutes: 10 s settle, then 6 cases × 4 rounds (3 s warmup, 6 s
  measured). Stop logcat after the `noise-bench done` line.
- Each window logs `stdio [mms] noise-bench result {...}` with `case`, `round`
  and `gpu_frame_ms` mean/min/max (both eye passes, whole frame).
- For each case, average the mean over the 4 rounds and subtract `flat`. Fill
  **Noise: texture vs procedural, focus3** in [GPU benchmarks](gpu-benchmarks.md).

Fixed settings: 1712×1712 per eye, 4x MSAA, foveation off. Don't change them.

### Checks while it runs

- `TextureSystem` / `ASTC` lines: both `.astc` files must load. An "ASTC
  textures are not supported" error means `astc_ldr` wasn't enabled on the
  device. Report it rather than falling back to RGBA8.
- No `stdio` lines at all: the stdio→logcat forwarding failed. The scene may
  still be running; check `adb logcat` unfiltered for `[mms]`.
- A frame failure naming an unsupported surface: the Android renderer only
  supports custom-material opaque/albedo surfaces (see `AGENTS.md`). Don't add
  a legacy fallback; report which surface it was.

## 4. Report back

Commit to `main`:

- result rows and the work-log entry in `docs/task/gpu-benchmarks.md`
  (date, commit, machine, any deviation, any compile fixes);
- the raw JSON and logcat capture;
- the compile fixes, as a separate commit.

In the summary, answer the question the benchmark exists for: on each GPU,
how much one texture sample (RGBA8 / BC7 / ASTC 4x4 / ASTC 8x8) costs versus
one octave and six octaves of procedural Perlin, as ms per frame over `flat`.
