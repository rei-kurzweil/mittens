# Focus 3 Vulkan/OpenXR probe

This specifies the first Android executable to implement. It is not a completed test or a claim about installed headset capabilities.

## Build and launch

Use the local ALVR Android packaging/lifecycle setup as a reference. Give the probe a Mittens package identity, an arm64 native activity entry point, logcat logging, and the matching OpenXR loader. Include OpenXR runtime discovery declarations and required HTC activity metadata. Request microphone/tracking permissions only as their tests are introduced.

Initialize the Android context before libraries that use it. Initialize the OpenXR loader before enumerating runtime extensions, and enable Android instance creation. Log failures with the exact API name/result. Keep application Vulkan capabilities distinct from OpenXR extensions and runtime requirements.

## Capability report

Persist/log a report with:

| Category | Fields |
| --- | --- |
| Device | Model, Android/API version, firmware/build identifier, ABI. |
| Runtime | OpenXR name/version, HMD system, view configurations, environment blend modes, enumerated instance extensions. |
| Graphics binding | `XR_KHR_vulkan_enable` and `XR_KHR_vulkan_enable2` availability; selected path; runtime min/max Vulkan API version and runtime-required extensions for that path. |
| Vulkan device | Runtime-selected physical device, device/driver/API versions, device extensions/features, graphics/compute queue families. |
| Render APIs | `dynamicRendering`, its core/extension availability, multiview feature and maximum view count; optional dynamic-rendering local-read support if exposed. |
| Limits | Per-stage storage-buffer and total resource limits, compute workgroup sizes/invocations/counts, uniform/storage buffer ranges, sampled-image limits. |
| Images | XR formats, recommended/max per-view extent/sample counts; supported color and depth/stencil format/usage/sample combinations, depth/stencil resolve modes where used. |
| Memory/assets | Device/local heaps and lazy memory types, ASTC/ETC2/BC texture support. |
| Optional integrations | HTC passthrough, Focus 3 controller interaction, hand and eye extensions; available performance/refresh/foveation extensions. |

Choose the runtime-selected physical device and an API version satisfying the runtime and engine. For `enable2`, use its prescribed Vulkan creation route. Required extensions must be enabled completely, with a diagnostic failure for an unsupported requirement.

## Incremental acceptance tests

1. Create a Vulkan-bound OpenXR session. Record which binding succeeded and the exact result of failures.
2. Create a supported **single-sample** stereo color swapchain. Enumerate the runtime-owned images; attach views without assuming ownership of the images or their memory.
3. Acquire/wait an XR image, clear each eye to a distinct color, ensure rendering completes and the required layout is restored, release, and submit projection views. Honor frame `should_render` and the session state machine.
4. Draw a triangle using the selected rendering API. Test dynamic rendering if supported; otherwise use a classic render pass. This establishes basic graphics compatibility before introducing Mittens' compute and material requirements.
5. Test a local floor/reference space and Focus 3 controller poses/actions.
6. Add HTC passthrough underlay and projection-layer alpha if needed. Test transparent clear, layer order, blend flags/mode, and object visibility rather than inferring passthrough success from session creation.
7. Exercise headset removal, app background/foreground, pause/resume, session stop/start, and clean destruction. Record whether Vulkan/session recreation is required.

Keep the first visual test independent of audio, eye accessories, scripting, mirrors, and post-processing. Add those tests after the basic Vulkan session works.

## Decisions from the result

- Vulkan session succeeds and dynamic rendering works: extract Mittens' XR-only initialization and reuse its base dynamic rendering path.
- Vulkan session succeeds without dynamic rendering: implement classic render-pass/framebuffer and pipeline variants while keeping shared scene preparation.
- Mittens' deformation limits fail: make that subsystem optional or lower its resource requirements before expecting an ordinary static scene to run.
- Vulkan binding/session fails: preserve logs and firmware/loader identity; investigate that failure before choosing a different graphics backend.
- Multiview or optional HTC/performance features are absent: retain a working per-eye/core input baseline and treat the missing feature independently.

After first scene bring-up, measure steady-state CPU/GPU times and missed frames, then repeat under sustained thermal load. At a 90 Hz display cadence one interval is about 11.1 ms; leave time for runtime/compositor work and measure the actual application budget.
