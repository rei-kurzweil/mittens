# XR-only desktop startup

Run Mittens without a desktop window before testing the Android host:

```sh
cargo run --release --example openxr-only
cargo run --release --example openxr-only -- --seconds 30 --no-msaa
```

Start the desktop OpenXR runtime and connect the headset first. The example
loads an embedded MMS scene with two colored cubes and a floor. It needs no
model downloads, script imports, audio input, or stdin REPL. `--seconds` bounds
the frame loop after initialization; without it, stop through the runtime or
Ctrl+C. Initialization and runtime calls can still block independently of that
limit.

`Universe::init_renderer_for_xr()` initializes OpenXR and obtains its Vulkan
requirements. `RendererContext` creates one Vulkan instance/device/queue,
selecting the physical device requested by OpenXR before creating the logical
device. The XR system creates its session and single-sample stereo swapchain;
Mittens then creates scene pipelines using that swapchain's actual color format.

This path creates no Winit event loop, window, Vulkan surface, or desktop
swapchain. Surface/presentation extensions are not requested by the engine
unless the runtime explicitly requires them. Per-eye frame slots do not depend
on a desktop swapchain. The ordinary windowed entry point remains available.

The example calls the existing `Universe::update`/`render` methods.
`OpenXRSystem::prepare_frame` performs `xrWaitFrame` and locates the predicted
views before the pose systems run. Rendering writes/resolves directly into the
acquired XR image layers, waits for completion, and releases/submits them.
While the session is stopped or waiting for READY, the host sleeps briefly
between event polls. EXITING, LOSS_PENDING, instance loss, and fatal rendering
failure terminate the example; STOPPING alone permits a subsequent restart.

Winit is still a Cargo dependency: desktop APIs and the shared keyboard/mouse
input types reference it. This establishes window-free execution, not a build
with Winit removed. Android loader/activity initialization, packaging, an
advertised-binding negotiation/enable2 path, and actual Focus 3 feature testing
remain separate work.

For GPU verification without an OpenXR runtime or display server:

```sh
cargo test --lib headless_scene_draws_stereo_without_surface_extensions -- --ignored --nocapture
```

That test uses the full scene pipelines and owned stereo images. It checks
both eye layers and repeated frame-slot reuse with pixel readback, with MSAA
off and requested at 4x. It asserts that no Vulkan surface or swapchain
extension was enabled. It does not validate OpenXR session lifecycle or headset
presentation; use the example for those checks.

## Validation on 2026-10-20

The desktop `openxr-only --seconds 5` run created a Vulkan session with
SteamVR/OpenXR 2.16.7, reached FOCUSED, used 4x MSAA, and exited with status 0.
Headset presentation was also confirmed by the user. The window-free GPU test
passed on a GeForce GTX 1080 with MSAA off and at 4x, including stereo pixel
readback. Seventy graphics/OpenXR library tests and the embedded-scene test
passed. The existing windowed `runtime-spec-emissive-cubes` example also reached
the present loop and completed window rendering with bloom and 4x MSAA; the
user confirmed its desktop presentation. The larger `ui-layout` smoke reached
its first redraw before its 30-second timeout, so it did not establish scene
rendering within that run. Explicit session stop/restart and Android/Focus 3
standalone execution remain untested.
