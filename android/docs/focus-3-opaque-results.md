# Focus 3: native MMS scene and 4x MSAA

Date: 2026-10-29. Device: VIVE Focus 3, `FA1722R00205`, Android 10/API 29,
Adreno 650 Vulkan 1.1.128, VIVE WAVE OpenXR 0.0.1 / Vulkan enable2.
The device clock still reports 2025-12-08; captured device timestamps are not
the development machine's date. No clock or system graphics settings changed.

## First scene: verified at 1x

Installed and launched `org.mittens.engine.dev/android.app.NativeActivity`
from `target/debug/apk/mitten-droid-custom-opaque.apk`:

```text
SHA256 433ab08dbc7df5cb9df5cdf056ef7835b04a2dc60266436caca2855f2e5b3b78
```

The real engine loads the embedded `android/assets/scene.mms`, updates a
Universe and prepares meshes/custom materials through the application uploader.
The user confirmed both cubes ("its a thing of beauty") and physical headset
translation/orientation. These host-supplied eye views provide 6DoF viewing;
engine InputXR/avatar/controller/hand integration is still absent.

The report records FOCUSED, `custom scene GPU complete`, and repeated
`mode=custom_opaque_scene GPU_complete_before_release=true` frames. It reports
one mesh, one custom pipeline, two retained material bindings and four draws /
four instances across both eyes. This is rendering/presentation evidence, not
an FPS benchmark or a complete graphics lifecycle test.

Output is R8G8B8A8_SRGB, 1024x1024 per eye, array layers=2, three runtime images,
1x color/depth. D16_UNORM depth is transient and lazily allocated. The runtime
recommends 1712x1712; the prototype deliberately retains its 1024px cap.

- [Raw 1x report](validation/focus-3-opaque-1x-2026-10-29.txt)
- [1x app/engine logs](validation/focus-3-opaque-1x-2026-10-29.log)

## Follow-up: 4x MSAA

User requested 4x MSAA after viewing the first scene. The Android scene now
creates 4x color and depth attachments and resolves color in the same subpass
directly into the single-sample runtime image. Multisample color/depth are
transient with CLEAR/DONT_CARE; the resolved XR output is DONT_CARE/STORE.
No separate resolve command, image copy or depth resolve is added.

The desktop GPU smoke check passed at 1x and 4x, including depth, stereo,
retained inputs, textures and batching. For the flat-color coverage test,
1x produced zero intermediate edge pixels and 4x produced 281, confirming
resolved coverage. Those counts are not performance measurements.
See the [selected headless output](validation/opaque-msaa-headless-2026-10-29.txt).

The Android build/alignment passed. Installed and launched:

```text
target/debug/apk/mitten-droid-custom-opaque-msaa4x.apk
SHA256 6f60f1bfbaa0c3fe04e1e38cb30cfd374bce36d2febaa99cd69714160f65bfff
```

Focus 3 creation logs confirm `samples=4`, `depth_lazy=true`, `msaa_lazy=true`,
and `in_pass_resolve=true`. The runtime output remains 1x, 1024x1024 per eye.
At the initial check the updated session reached SYNCHRONIZED; rendered-frame
and visual confirmation are tracked separately below.

- [Raw 4x report](validation/focus-3-opaque-msaa4x-2026-10-29.txt)
- [4x app/engine logs](validation/focus-3-opaque-msaa4x-2026-10-29.log)

- [ ] Confirm completed 4x custom scene frames on the headset.
- [ ] Confirm 4x appearance in both eyes and smoother geometric edges.
- [ ] Controlled background/resume, stop/relaunch and clean destruction.
- [ ] Profile actual CPU/GPU costs and sustained native cadence.

The desktop A/B demo retains an explicit 1x baseline on both renderers. It has
not been benchmarked in a visible desktop headset session. This change adds no
new controller, hand, deformation, mirror, bloom or audio functionality.
