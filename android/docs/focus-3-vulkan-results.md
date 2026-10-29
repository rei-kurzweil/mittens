# Focus 3 OpenXR/Vulkan results

Verified 2026-10-29 on VIVE Focus 3, Android 10/API 29, build display ID
`QKQ1.210528.001 release-keys`, incremental `11634`. The user confirmed the
requested triangle/eye outputs through the headset after successful frame
submission. This is a local native Vulkan prototype, not a Mittens scene.

## Established results

| Item | Observed result |
| --- | --- |
| Android loader and instance | Khronos loader 1.1.36 initialized; Android OpenXR instance and HMD system succeeded. |
| Runtime | VIVE WAVE 0.0.1, system `WAVE:SUE`, orientation and position tracking advertised. |
| Vulkan binding | Both bindings advertised; `XR_KHR_vulkan_enable2` actually used and successful. Legacy binding was not exercised. |
| Runtime Vulkan API range | 1.0.0–1.2.203. This range is distinct from the GPU's supported API. |
| Runtime-selected GPU | Adreno (TM) 650, Vulkan 1.1.128, raw driver version 2149855232. |
| Dynamic rendering | `VK_KHR_dynamic_rendering` absent from the device extension list; unavailable on this driver/API. Dynamic-rendering local-read extension also absent. |
| Rendering path | Classic render pass with per-eye framebuffer/views; Vulkan instance/device and XR session creation succeeded. |
| XR output | R8G8B8A8_SRGB, 1024x1024, two array layers, 1x samples, three runtime images. |
| Frame submission | IDLE → READY → SYNCHRONIZED → VISIBLE → FOCUSED. Clear and triangle milestones logged, followed by at least frame 900. |
| User confirmation | User answered “ya i see them” to the request to check the RGB triangle in both eyes with distinct reddish/blue backgrounds. |
| Multiview | Advertised and feature supported; maximum view count 6. The prototype renders each eye separately. |
| Compute | Graphics+compute queue; workgroup invocation limit 1024 and per-axis sizes [1024, 1024, 64]. Basic audited deformation limits passed, without executing engine deformation shaders. |
| Formats and samples | D32_SFLOAT_S8_UINT and D24_UNORM_S8_UINT attachment queries succeeded. Queried color/depth attachments support 1x/2x/4x; XR output remains 1x. |
| Texture compression | ASTC LDR and ETC2 supported; BC compression unsupported. |
| Memory | A lazily allocated device-local memory type is exposed. |

## Evidence

- [OpenXR instance/HMD diagnostic report](validation/focus-3-openxr-instance-2026-10-29.txt)
- [OpenXR diagnostic logs](validation/focus-3-openxr-instance-2026-10-29.log)
- [Vulkan capability/session/frame report](validation/focus-3-vulkan-probe-2026-10-29.txt)
- [Vulkan process logs](validation/focus-3-vulkan-probe-2026-10-29.log)
- [APK build hashes](validation/probe-apk-build-2026-10-29.txt)

The device log clock differs from the development machine; use the host test
date above. No firmware, driver, or persistent device settings were changed.

## Renderer decision

Mittens needs a classic render-pass/framebuffer and compatible pipeline path
for this installed driver. Its existing dynamic-rendering requirement cannot
be used unchanged. The small Ash prototype proves the Android/OpenXR/Vulkan
route and the classic graphics path; it does not supply that backend to the
full Vulkano engine renderer yet.

This requirement follows from the measured API/extension support. A tiled
GPU does not inherently require classic `VkRenderPass` objects: dynamic
rendering also defines attachment load/store operations and rendering scopes
that a tiler can use. Preserve shared scene/material/draw preparation while
adding compatible pass/pipeline recording. See the
[Khronos tile-rendering guide](https://docs.vulkan.org/guide/latest/tile_based_rendering_best_practices.html).

## Remaining validation

Graphics-active background/resume, session stop/start, physical headset
removal, normal shutdown/relaunch, and sustained timing/thermal behavior are
not yet established. The earlier lifecycle-only host tests do not cover these
graphics paths. Full Mittens scene execution, compute deformation, stencil UI,
MSAA, controllers, passthrough, assets, and audio remain subsequent work.
