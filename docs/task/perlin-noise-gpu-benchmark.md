# Perlin noise GPU benchmark: baked texture vs procedural

Question: in a custom material fragment, how much does procedural Perlin noise
cost compared with one sample of a baked noise texture? Measure on the desktop
GTX 1080 first, then on the XR2 (Focus 3) with the Android renderer.

Results are logged in [GPU benchmarks](gpu-benchmarks.md). Feeds the smoke
ribbon work in
[curved smoke ribbons and spin blur](curved-smoke-ribbons-and-spin-blur.md).

## Fixture

- Shader: `assets/shaders/perlin-noise-benchmark.frag`. The harness prepends
  `NOISE_MODE` / `NOISE_OCTAVES`:
  `0` flat colour (baseline), `1` one texture sample, `2` one Perlin octave,
  `3` Perlin fBm (default 6 octaves). Integer hash (no `fract(sin)`), screen
  pixel coordinates; texture sampled 1:1 texel per pixel with repeat
  addressing; procedural base cell 128 px matches the baked texture's layout.
- Textures: `assets/images/noise/perlin-1024.png` (uploads as RGBA8),
  `perlin-1024.dds` (BC7, desktop only) and `perlin-1024-astc{4x4,8x8}.astc`
  (ASTC LDR, Android only; GTX 1080 has no ASTC).
- Every variant declares and binds the texture so descriptor setup is
  identical; the harness asserts the compiled SPIR-V sample count per case.

## Desktop (GTX 1080)

```sh
cargo test --release --lib noise_profile -- --ignored --nocapture
# optional: NOISE_PROFILE_OUTPUT=/tmp/noise-profile.json NOISE_PROFILE_WARMUP=24
#           NOISE_PROFILE_FRAMES=80 NOISE_PROFILE_REPEATS=6 NOISE_PROFILE_OCTAVES=6
```

Harness: `src/engine/graphics/vulkano_noise_profile.rs` (test-only module of
the existing Sakura profile fixture, same structure as the twilight moon
profile). Cases: `flat`, `texture-rgba8`, `texture-bc7`, `perlin-1-octave`,
`perlin-fbm`. Both eyes, 4x MSAA, full-coverage background sphere, at
1920×1080 and 2448×2448 per eye. Order rotates/reverses each round. Reports GPU
graphics-pass timestamps (incl. clear + resolve), `delta_vs_flat_ms` and
`delta_ns_per_pixel` per case to `/tmp/noise-profile.json`.

Not a whole-frame/FPS measure: no swapchain pacing, CPU load or post.

- [x] Shader compiles for all four modes (`glslangValidator -V`); unused
      sampler is retained in non-texture modes.
- [ ] Harness compiles (not built: no Rust toolchain on the authoring machine).
- [ ] Run on GTX 1080; record results below.
- [ ] Optional: capture one frame per case to eyeball that texture and fBm look
      alike (twilight profile has a capture helper to copy).

## XR2 (Focus 3, Android renderer)

Scene: `android/examples/noise-bench.mms`, built with
`--features noise-bench` (run steps in [GPU benchmarks](gpu-benchmarks.md#xr2-noise-run)).
A `FrameTick` state machine attaches one background-sphere case at a time,
warms up, and brackets `Benchmark.start/stop`; results print to logcat. The
Android installer writes `perlin-noise-benchmark-mode<N>.frag` variants because
MMS cannot prepend defines. Cases: `flat`, `texture-rgba8`, `texture-astc4x4`,
`texture-astc8x8`, `perlin-1-octave`, `perlin-fbm6`; 1712² per eye, 4x MSAA,
foveation off.

Supporting changes:
- ASTC textures: `.astc` (astcenc container) → `CatEngineTextureFormat::Astc`,
  `TextureUploader::upload_texture_astc` (default: explicit "not supported"
  error, so desktop rejects ASTC rather than decoding it). Android enables
  `textureCompressionASTC_LDR` at device creation and uploads one level.
- Android forwards stdout/stderr to logcat (`stdio ...`), so MMS `print` and
  engine diagnostics such as `[TextureSystem]` errors are visible.

- [x] Android custom materials support set=1 binding=1 textures + inputs.
- [x] ASTC upload path (Android) and `.astc` loader (engine).
- [x] MMS benchmark scene + Android feature/asset bundle.
- [ ] Build APK; run on headset; record results in [GPU benchmarks](gpu-benchmarks.md).
- [ ] Decide texture vs procedural for smoke ribbons on XR2.
- [ ] ASTC currently uploads one level (no mips). Fine here (1:1 sampling);
      general ASTC assets need a mip chain (astcenc per level or KTX2).

## Results

Logged in [GPU benchmarks](gpu-benchmarks.md#results).

## Android renderer audit (2026-11-20)

Custom materials are close to desktop parity. Both use the same shaderc-compiled
fragment, MeshSurfaceV1 varyings (loc 0–6), the reflected std140 inputs block
with live `set_input`, and one base texture at set=1 binding=1 (Android: three
filter samplers; RGBA8 with a CPU-generated mip chain). Both support
single-layer alpha blending with depth writes off.

Differences that matter:

| Area | Desktop | Android |
|---|---|---|
| Culling | none | back faces culled except background (`android/src/renderer/pipeline.rs` ~126) |
| Unsupported surface | per-feature pipelines | whole frame fails: emissive, cutout, outline, bones/deformation, stencil, clip, multi-layer transparency, post, mirrors, or any surface without a custom `Material` (`android/src/renderer/mod.rs` ~191–235) |
| Textures | RGBA8, BC7 (top mip only) | RGBA8 + mips; BC7 rejected |
| Asset delivery | filesystem | fixed embedded list (`android/src/demo_assets.rs` ~23–77); new shaders/textures must be added |
| Stereo | two eye submissions | two render passes per frame, no multiview |
| Foveation | — | `VK_EXT_fragment_density_map` Low/Medium/High |
| GPU timing → `Benchmark` | not wired (`gpu_frame_ms` null) | top/bottom timestamps over both eye passes |
| Time | `time_seconds` input pushed by script | same |

Tiling: 4x MSAA colour CLEAR/DONT_CARE resolved in-subpass to the XR image,
transient depth (D16 preferred, CLEAR/DONT_CARE), lazily allocated memory when
available. Current status docs: `android/docs/opaque-renderer.md` (no foveation
section yet) and
`android/docs/task/epic/adreno650_rendering_optimization/README.md` (newest).

For the XR2 benchmark scene: background sphere with a custom material only (no
lights, emissive, or GLTF legacy materials); add the benchmark shader and
`perlin-1024.png` to the embedded asset list; keep MSAA, foveation and
resolution fixed. Android numbers cover the whole frame of both eye passes, not
the isolated graphics pass, so compare deltas versus `flat`, not absolutes with
desktop.
