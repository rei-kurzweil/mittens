# Shared graphics settings and XR foveation preferences

The reusable panel is
[`graphics_settings_panel.mms`](../../assets/components/panels/graphics_settings_panel.mms).
Sakura drone conversion and other examples import the same factory. Its controls
use the `GraphicsSettings` built-in table; they do not own independent applied
settings.

## XR foveation

```mms parse-only
GraphicsSettings.foveation.quality("low")
let requested_quality = GraphicsSettings.foveation.quality_level()
let backend_status = GraphicsSettings.foveation.status()
let last_applied_quality = GraphicsSettings.foveation.applied_quality()
```

Accepted values are `off`, `low`, `medium`, and `high`, describing **how much
fixed foveation** is requested. Low means a mild reduction toward the edges;
high means more foveation. The default is off. Invalid names are rejected without
changing the setting. Readback immediately returns the accepted preference.

The shared panel has a stepped quality slider and a grey, noninteractive eye
tracking control fixed to Off. Eye tracking has no setter or active toggle in
this slice.

The Android Vulkan renderer uses `VK_EXT_fragment_density_map` when the GPU
supports both `fragmentDensityMap` and `fragmentDensityMapNonSubsampledImages`,
plus an RG8 density image with the required format/usage support. Other devices
report unavailable and keep the ordinary pass. Subsampled-image reconstruction
is separate work; this implementation does not assume every advertised density-map
extension permits direct rendering into OpenXR swapchain images.

The maps are fixed around each eye's forward direction, using its asymmetric
projection rather than assuming the optical center is the center of the image.
The profiles request the following angular cones (half angles from forward):

| Amount | Full density | Half density on each axis | Quarter density on each axis |
| --- | --- | --- | --- |
| Off | Entire image | — | — |
| Low | Up to 35° | Beyond 35° | — |
| Medium | Up to 30° | 30–45° | Beyond 45° |
| High | Up to 20° | 20–35° | Beyond 35° |

Actual fragment sizes are implementation-defined and may be clamped. The density
map is RG8_UNORM, with values 255, 127 and 63; rounding down allows reciprocal
shading densities of at least 1, 2 and 4. This is the amount of foveation, not
an overall rendering resolution setting. Eye tracking remains disabled.

`status()` returns `unavailable`, `pending`, `accepted`, or `failed`.
`applied_quality()` returns the last configuration applied by the rendering backend,
or null before any successful configuration. A failed change preserves the previous
pass and applied value; the requested preference remains available separately.
Acceptance does not promise a GPU speedup. The panel displays this feedback and
remains synchronized with the built-in table.

The renderer owns the backend in `android/src/renderer/foveation.rs`. Static maps
are uploaded and fenced before command recording, because the driver may read
them during `vkCmdBeginRenderPass`. A map/pass is rebuilt only on quality or eye
projection changes. Off switches to the original pass, with no density attachment;
re-enabling an unchanged cached profile reuses its resources. Pipelines are cached
separately for ordinary and density-map passes. There are no per-draw map uploads,
intermediate eye targets, or reconstruction copies. MSAA color/depth retain their
transient/lazy allocation policy and in-pass resolve into the runtime image.

The earlier HTC API experiment is removed: HTC documents its OpenXR foveation
feature as [OpenGL ES only](https://dl.vive.com/SDK/openxr/vive_xr_performance_tunning_guide.pdf)
(page 17), despite the runtime returning success for our Vulkan requests. This
backend uses Vulkan capabilities, independent of the headset vendor. See the
[Khronos density-map sample](https://docs.vulkan.org/samples/latest/samples/extensions/fragment_density_map/README.html)
for the static-map and non-subsampled-image contracts.

Changing quality uses the existing `GraphicsSettingsChanged` event. All open
panels synchronize without emitting extra user-change events. Minimizing removes
the panel body but preserves the preference in `GraphicsSettingsSystem`; restoring
reads the latest value, including changes made by another panel or MMS callback.
Scene/session reset restores the default. Saved user profiles are separate work.

Bloom, mirrors, secondary motion, and foveation share the same built-in table,
engine-thread host service and notification path. Their MMS registration and
dispatch live in `src/scripting/graphics_settings_api.rs`; extracting that
responsibility avoids expanding the oversized host/runtime configuration files.

Headless regression coverage checks the built-in setter/getter, invalid levels,
two-panel synchronization, minimize/restore, disabled eye-tracking interaction
and host feedback. Renderer tests check profile ordering, UNORM encoding, rounded
map extents and per-eye asymmetric projection. The opt-in GPU check below uses the
real renderer and reads `gl_FragSizeEXT` back from both eye images at 1x and 4x MSAA,
checking Off/Low/Medium/High, disable/re-enable and unchanged-request suppression.
The desktop Vulkan smoke verifies panel rendering and movable-window materials.

## Headset A/B test

Build an optimized development-signed APK:

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=3 android/packaging/build-apk.sh --features shell-demo
adb install --no-incremental -r target/debug/apk/mitten-droid.apk
adb shell am start -S -n org.mittens.engine.dev/android.app.NativeActivity
```

The headset shell starts at Off and places a compact version of this same panel
to the left of Terminal, clear of the tree. It supplies an explicit custom UI
material and requests `foveation_only = true`, so unsupported bloom/mirror controls
are not offered by the current Android renderer. Full desktop panels remain unchanged.
The factory accepts an optional custom `material` retained on its movable root.

Wear the headset and keep resolution (currently 1712x1712 per eye), refresh rate,
panel visibility, scene content and head position constant. Record at least eight
seconds at Off, then Low, then Off again. Compare peripheral text quality as well
as GPU eye-pass time; matching FPS alone does not establish whether shading work
changed. The eye-tracking switch remains disabled.

`files/xr-vulkan-probe.txt` logs `XR_FOVEATION` on request changes, including backend status and any setup error. `FRAME_PROFILE` lines include `foveation_applied`; changing the
applied level clears the measurement window and repeats warmup so averages do
not mix levels. A connected but unworn headset may remain synchronized without
rendering; that is not a completed live test. Save the GPU readback and timing
samples before claiming on-device effectiveness.


For an opt-in hardware check before the normal scene starts:

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=3 android/packaging/build-apk.sh --features shell-demo,foveation-validation
```

Install and launch as above, then inspect `FOVEATION_GPU_CHECK` in the report.
The check uses an offscreen substitute for the stereo swapchain and the same eye
pass and density-map code. It reports actual coarse/full fragment pixel counts;
API acceptance alone cannot pass this check. This diagnostic is excluded from
normal builds. Keep performance measurements on a normal build.

The [Focus 3 GPU readback record](../../android/docs/validation/focus-3-vulkan-foveation-2026-11-02.txt)
confirms actual coarse fragments at the headset's 1712x1712 eye resolution. At 4x
MSAA, Low produced about 45% coarse output pixels and High about 78–80% in the
diagnostic scene. These are pixel counts, not measured performance savings.
A 512x512 target stayed at full density throughout; tile/filter decisions make
small-target tests unsuitable for demonstrating these profiles on this driver.


For live visual diagnosis, build with `--features shell-demo,foveation-visualization`.
This replaces only the shell sky shader with a custom shader that reads actual
`gl_FragSizeEXT`: green = maximum axis 1, yellow = 2, red = 4.
Anisotropic fragments use their largest dimension for this color key.
The UI remains normally shaded and the quality slider controls the actual pass.
Off should leave the sky green; higher amounts should introduce coarse-color
regions. This mode is diagnostic, requires a density-map-capable GPU, and is
excluded from normal builds. Reinstall a normal build to restore the Sakura sky.
The new `assets/components/platforms/terraced_garden.mms` prefab places a railed
upper deck, descending ramp, and lower landing/tree across from the original
shell terrace, providing additional scene detail for visual comparisons.


Normal shell builds also include a framed checkerboard on the upper deck of the
opposite terrace (`assets/components/mitten_droid/checkerboard.mms`). Its three
bands use 16, 48, and 128 checks across the width, independent of the quality
setting. Compare Off and High with a fixed head position, viewing the board
through the center and then the peripheral portion of the display. Fine checks
can alias even with Off; compare changes between settings rather than treating
aliasing by itself as evidence of foveation. The board leaves all other scenery
and windows present; the normal build restores the pink sky.
