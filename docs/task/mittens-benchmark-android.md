# mittens-benchmark: Android benchmark app and adb harness

Date: 2026-11-23
Status: proposed. Nothing here is implemented; no results yet.

A dedicated Android build that runs a fixed benchmark suite on the Focus 3 and
sends its results back to a host script over adb. The first suite compares
bloom variants with and without fragment density maps (FDM), including FDM on
bloom's intermediate images and subsampled intermediates. Everything needed to
build and run it from another machine with adb access to the headset is in
this doc or linked from it.

Related:

- [Fast bloom research](bloom-performance-desktop-and-mobile.md): the bloom
  questions this suite answers, including
  [foveation and subsampled intermediates](bloom-performance-desktop-and-mobile.md#3-foveation-and-subsampled-intermediates-xr2).
- [MMS `Benchmark` API](../spec/benchmark.md): the measurement intervals this
  builds on.
- [GPU benchmarks](gpu-benchmarks.md) (results log) and
  [GPU benchmark runbook](gpu-benchmark-runbook.md) (agent handoff).
- [Perlin noise benchmark](perlin-noise-gpu-benchmark.md): the existing
  `--features noise-bench` scene. Its logcat-only flow is the pattern this
  replaces with a structured harness.
- [Android renderer scope](../../android/docs/opaque-renderer.md),
  [Adreno 650 optimization epic](../../android/docs/task/epic/adreno650_rendering_optimization/README.md),
  [FDM device check](../../android/docs/validation/focus-3-vulkan-foveation-2026-11-02.txt).

## Why a separate app

The noise benchmark showed the existing route works: a feature-gated MMS scene
in Mittens Host, `Benchmark.start/stop`, and results printed to logcat. It does
not scale to a suite with many cases and two builds:

- Results are scraped from logcat, which rotates and drops lines. There is no
  authoritative results file, no schema and no device/thermal metadata.
- It is installed over the development app (`org.mittens.engine.dev`), and it
  is a debug build with `CARGO_PROFILE_DEV_OPT_LEVEL=3`, not a release build.
- It runs with live head tracking, so pixel coverage changes with head pose.
- `Benchmark` reports mean/min/max only; the bloom research needs p50/p95/p99,
  per-stage GPU time and missed frames.

`mittens-benchmark` is a separate APK (package `org.mittens.benchmark.*`,
launcher label **Mittens Benchmark**) that installs alongside Mittens Host and
ALVR. It reuses the Android host, renderer and engine; it does not fork them.
It loads only the benchmark scene: no editor UI, collision or controller
locomotion.

## Two APKs: FDM and no-FDM

| APK | Package | Vulkan FDM extensions | Runs |
|---|---|---|---|
| `mittens-benchmark-fdm` | `org.mittens.benchmark.fdm` | enabled | every case, including foveation Off/Low/Medium/High and FDM on intermediates |
| `mittens-benchmark-nofdm` | `org.mittens.benchmark.nofdm` | never enabled | every case that does not need FDM |

Both implement the same bloom variants from the same source.

The primary foveation comparison happens **inside the FDM APK**, switching
quality at runtime between interleaved windows. That keeps both sides in the
same thermal and clock session, which matters more than anything else for
comparability. Foveation Off there already records a pass without a density map
attachment (`android/src/renderer/foveation.rs`), but the device still has the
extension enabled.

The no-FDM APK is the **control**. Its "bloom on, foveation off" cases should
match the FDM APK's foveation-Off cases within noise. If they don't, enabling
the extension or its render pass variants has a cost of its own, and that is a
result. It is also the fallback if the FDM path fails on a driver update.

Running both APKs doubles headset time. Run the no-FDM control once per
renderer change, not for every exploratory run.

### Build structure

cargo-apk reads the package id from `[package.metadata.android]`, which can't
vary by feature. Proposed layout:

- Make `mitten-droid`'s host/renderer reusable: `crate-type = ["cdylib",
  "rlib"]`, with `android_main` kept in the existing app's entry module.
- Add two thin workspace crates, `android/benchmark/fdm/` and
  `android/benchmark/nofdm/`. Each has its own `Cargo.toml` (package id,
  label, `debuggable = true`) and a small `lib.rs` that calls the shared host
  with a benchmark scene and its FDM policy.
- Add `android/packaging/build-benchmark-apk.sh [fdm|nofdm|both]`, wrapping the
  same NDK/CMake/loader preparation as `build-apk.sh` and building
  `--release`.

The renderer must take "FDM disabled" as an explicit device-creation choice,
not by failing to find the extension. Report it as `fdm=disabled-by-build` in
the device record, never as unsupported.

`debuggable = true` is required for `run-as` (see the harness). It mainly
affects Java/ART, not our native code, but record it with every result.

## The benchmark scene

`android/examples/benchmark-bloom.mms`, embedded by both APKs:

- The lean Sakura stage plus a fixed set of emissive sources: thin LED strips,
  bright points, a broad sun glow behind a silhouette, emissive text.
- **Locked camera.** The renderer uses fixed per-eye view matrices from the
  suite config instead of the tracked head pose, so every case renders the
  same pixels. Projections stay the runtime's, so extent and FOV are real.
  What the wearer sees will be wrong while it runs; that's expected.
- The scene drives the suite from a `FrameTick` state machine, like
  `android/examples/noise-bench.mms`: settle, then per case warm up, run
  `Benchmark.start`, measure, run `Benchmark.stop`. Case order rotates each
  round and reverses on odd rounds.
- The case list, rounds and timings come from `suite.json` (below), read at
  startup, so changing a run needs no rebuild.

## Suite: bloom × foveation

Not a full factorial. Change one dimension at a time from a reference config,
then combine the winners.

Reference config: 1712×1712 per eye (the runtime recommendation; record what
the runtime actually reports), 4x MSAA, RGBA8 bloom intermediates, half-res
start, 4 pyramid levels, bilinear downsample/upsample, composite pass.

| Dimension | Values | Notes |
|---|---|---|
| Bloom | off, on | "off" is the cost reference for every bloom case. |
| Main-pass foveation | off, low, medium, high | FDM APK only for non-off. |
| Glow source | MRT, redraw | MRT: emissive is a second color attachment of the main pass, stored. Redraw: only emissive geometry drawn again into a half-res target. |
| Blur kernel | bypass, paired-tap Gaussian | Bypass means resizing with bilinear filtering only (see the bloom doc). |
| Pyramid levels | 3, 4, 5 | |
| Intermediate format | RGBA8, B10G11R11, RGBA16F | Only formats the probe reports as usable (see prerequisites). |
| FDM on intermediates | none, expanded, subsampled | FDM APK only. "Expanded" attaches a density map to extraction and the first downsample/blur passes. "Subsampled" also creates those images with `VK_IMAGE_CREATE_SUBSAMPLED_BIT_EXT`. |
| Composite | load pass, previous-frame | Load: a pass that loads the XR image and adds bloom. Previous-frame: add last frame's pyramid in the main pass's final draw, with no extra pass. The latter lags under head rotation, so it needs a motion check before it counts. |

Defaults: 10 s settle, 3 s warmup, 8 s measured per window, 4 rounds. Cool
down between APKs (see the run protocol).

**Visual capture.** After the timed rounds, each case renders one frame with
the eye images read back to files, outside any timed window. The harness pulls
them, so blockiness, halos and thin-LED loss can be compared side by side.
Readback never runs during a measured window.

## Measurements

What each measured window records:

| Metric | Source | Status |
|---|---|---|
| `gpu_frame_ms` | Existing timestamps spanning both eye passes. | exists; must be extended to span bloom passes too |
| Per-stage GPU ms | One timestamp pair per stage: main pass, source, each pyramid level, composite. Grow the query pool in `android/src/renderer/gpu_timing.rs`. | new |
| p50/p95/p99 | Fixed log-spaced histogram per label, still constant space as `docs/spec/benchmark.md` requires. | new; update the spec |
| `cpu_update_ms` | Existing. | exists |
| Record and submit CPU ms | The existing `FRAME_PROFILE` fields, recorded per window. | exists in logs only |
| Missed frames | Deltas between successive predicted display times compared with the predicted display period. | new; an estimate, not compositor truth |
| Applied state | Foveation applied, FDM pass/intermediate mode, formats, extents, sample count. | new |

Compare cases by their delta against the bloom-off case **in the same run and
APK**, as the noise benchmark does, not by absolute numbers across runs.

## Results over adb

The app writes results in two places:

1. **`files/results.jsonl`** in the app's private files directory. This is
   authoritative: one JSON object per measured window, flushed after each
   write. The harness reads it with `adb exec-out run-as <pkg> cat
   files/results.jsonl`, which is why the APK is debuggable.
2. **Logcat lines prefixed `MITTENS_BENCH `** for progress: `ready`,
   `case_start`, `result` (the same JSON), `capture`, `done` and `error`. The
   harness uses these to follow the run and to detect hangs. It never treats
   logcat as the only copy of a result.

Result record (schema version 1):

```json
{
  "schema": 1, "suite": "bloom-fdm", "apk": "fdm", "commit": "<hash>",
  "build": "release", "debuggable": true,
  "case": "bloom-on/fov-high/src-mrt/kernel-bypass/lv4/rgba8/inter-expanded/comp-load",
  "round": 2, "order_index": 5,
  "extent": [1712, 1712], "msaa": 4,
  "foveation": { "requested": "high", "applied": "high", "intermediates": "expanded" },
  "window_ms": 8000.0, "frame_count": 600, "missed_frames_est": 0,
  "gpu_frame_ms": { "samples": 600, "mean": 0, "p50": 0, "p95": 0, "p99": 0, "max": 0 },
  "gpu_stage_ms": { "main": {}, "source": {}, "level0": {}, "composite": {} },
  "cpu_update_ms": {}, "record_ms": {}, "submit_wait_ms": {},
  "thermal": { "battery_c": 0, "gpu_c": null }
}
```

Numbers above are placeholders. A metric with no samples is `null`, never 0.

`suite.json` goes the other way: the harness pushes it with `adb shell run-as
<pkg> sh -c 'cat > files/suite.json'` before launching. If it's missing, the
app falls back to its embedded default suite and logs that it did.

## Host harness: `scripts/android_benchmark.py`

Python 3, standard library plus `adb` on `PATH`; no Rust needed to *run* it.

```sh
scripts/android_benchmark.py build   [fdm|nofdm|both]   # wraps build-benchmark-apk.sh
scripts/android_benchmark.py install [fdm|nofdm|both]
scripts/android_benchmark.py run     [fdm|nofdm|both] --suite bloom-fdm [--rounds N] [--cases GLOB]
scripts/android_benchmark.py summarize <results dir>
```

`run` does, per APK:

1. Check exactly one authorized device and record `ro.product.model`,
   `ro.build.fingerprint`, the APK version and commit, and battery level,
   charging state and temperature (`dumpsys battery`). Also capture `dumpsys
   thermalservice` where available.
2. Wait for cooldown: battery temperature at or below `--cool-c` (default
   35 °C), with a timeout. Refuse to start below 40% battery unless `--force`.
3. Push `suite.json`, clear logcat, `am start -W -n <pkg>/android.app.NativeActivity`.
4. Wait for `MITTENS_BENCH ready`, which the app logs only after the OpenXR
   session is FOCUSED. If the headset isn't worn, it times out with a message
   to put it on or cover the proximity sensor.
5. Stream `MITTENS_BENCH` lines and show progress. Stop on `done`, `error`, or
   no progress for `--stall-s` seconds.
6. Pull `results.jsonl` and the capture images, save the full logcat, record
   thermal state again, then `am force-stop`.

Output goes to
`docs/task/results/mittens-benchmark/<date>-<suite>-<short commit>-<apk>/`:
`device.txt`, `suite.json`, `results.jsonl`, `logcat.txt`, `captures/` and
`summary.md`.

`summarize` groups by case across rounds and reports, per case, the mean and
spread of each metric and its delta against the bloom-off case. It writes
`summary.md` with a table that can be pasted into
[GPU benchmarks](gpu-benchmarks.md). With both APKs present, it adds the
FDM/no-FDM control comparison. It flags cases whose spread across rounds
exceeds their delta, and any window with missed frames.

## Running it on another machine

1. **Toolchain** (to build): follow [Android host
   bootstrap](../../android/docs/host-bootstrap.md#build-the-apk): Rust target
   `aarch64-linux-android`, Android SDK and platform tools, NDK
   `28.1.13356709`, CMake `3.30.3`, Java/keytool, and cargo-apk `0.10.0` from
   the `zarik5/cargo-apk` fork at `0fd3126d`. Set `ANDROID_HOME`. To *run*
   only, `adb`, Python 3 and prebuilt APKs are enough.
2. **Headset**: developer mode and USB debugging on, the adb key authorized,
   and `adb devices` showing exactly one device. Charge above 40%. Note
   whether it's on the charger, because charging changes thermals.
3. **During a run**: wear the headset or cover the proximity sensor, keep it
   still on a stand facing the same way, and don't touch the controllers.
   Close other XR apps first.
4. **After**: commit the results directory, add rows to
   [GPU benchmarks](gpu-benchmarks.md), and fill in the focus3 driver in its
   Machines table from `device.txt`.

## Prerequisite probes

Before the subsampled cases mean anything, extend the startup device report:

- `VK_EXT_fragment_density_map2` properties: `subsampledLoads`,
  `subsampledCoarseReconstructionEarlyAccess`, `maxSubsampledArrayLayers` and
  `maxDescriptorSetSubsampledSamplers`. The extension is advertised (2026-10-29
  probe), but its properties weren't recorded.
- Already recorded on 2026-11-02: `fragmentDensityMapNonSubsampledImages=1`,
  density texel size 32×32 to 256×256, no dynamic density map.
- Which sampler settings a subsampled sampler allows, especially linear
  filtering, which bloom downsampling relies on. The Vulkan sample uses nearest
  filtering. If only nearest is allowed, the subsampled cases need a different
  downsample filter, or they're dropped.
- The Focus 3 driver doesn't advertise `VK_KHR_fragment_shading_rate`, so FDM
  is the only variable-rate shading option there.
- For B10G11R11 and RGBA16F: color attachment, blend, sampled-image and
  linear-filter support. `maxColorAttachments` and attachment-size limits for
  the MRT source.

Cases needing something the device lacks are reported as `unsupported` with
the reason, never silently skipped or swapped for another case.

## Slices

1. **App shell, harness and baseline suite.** The two crates, the build
   script, `suite.json`, the locked camera, `results.jsonl` and logcat
   records, and the harness's `run` and `summarize`. Suite: no bloom,
   foveation off/low/medium/high. Useful immediately: it gives the first
   measured foveation numbers, which don't exist yet.
2. **Measurement extensions.** Histogram percentiles, per-stage timestamps,
   missed-frame estimate and the prerequisite probes. Update
   `docs/spec/benchmark.md` for the histogram.
3. **Bloom prototype.** Bloom is new scope for the Android renderer. Put it in
   a focused `android/src/renderer/bloom.rs` module, enabled only by the
   benchmark builds, not the default APK. Start with the reference config plus
   the source and composite variants.
4. **FDM on intermediates**, expanded and then subsampled.
5. **No-FDM control run and visual review**, then record results and
   recommendations in the bloom doc.

Constraints from `AGENTS.md`: no legacy material pipelines or silent fallbacks.
Bloom passes must not add per-draw allocations or per-frame pipeline creation.
Transient attachments stay transient. The default APK's render path stays
unchanged unless a later decision enables bloom there.

## Completion criteria

- [ ] Both APKs build with one command on a machine set up per the bootstrap
  doc, and install alongside Mittens Host.
- [ ] One `run both` produces the results directory with no manual log
  scraping, and `summarize` produces a delta table.
- [ ] A rerun from cold reproduces the bloom-off/bloom-on delta within the
  spread recorded in `summary.md`.
- [ ] The FDM/no-FDM control comparison is recorded.
- [ ] Results and a recommendation for the XR2 bloom route, including whether
  FDM and subsampled intermediates are worth it, are written up in the bloom
  doc.
