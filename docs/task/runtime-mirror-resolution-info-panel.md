# Task: MMS graphics settings panel with runtime mirror resolution

Date: 2026-10-12
Status: investigation and implementation planned; documentation only.
Architecture clarified 2026-10-12: MMS owns the panel; a host-backed graphics
namespace exposes runtime settings.

## Outcome

Investigate the smallest supported way to change mirror capture resolution at
runtime. Build a reusable MMS `graphics_settings_panel` around the existing
`info_panel` factory, initially containing only a mirror resolution slider and
live readout. Use it in mittens-corp and mittens-corp-desktop. The panel can
later grow controls for frame rate versus image quality when those controls
have working runtime APIs.

## Ownership and scope

- Keep the panel, layout, labels, slider callbacks, and accordion restore logic
  in `assets/components/ui/graphics_settings_panel.mms`.
- Expose a host-backed MMS object/namespace such as `graphics` for reading and
  changing actual graphics settings. Reuse RuntimeSpec namespace APIs and the
  existing host dispatch; MMS already uses this mechanism for `File`, `JSON`,
  and audio device APIs. No new language feature is required.
- Rust owns validation, authoritative runtime values, and renderer/resource
  changes. The UI must read actual applied values rather than maintain another
  independent graphics-settings state.
- Do not add a Rust `EditorPanel::Graphics`, hard-coded graphics panel rows,
  or new editor-context click handling. Explicit scenes attach the MMS panel
  factory as an ordinary authored component subtree.
- Implement mirror control first. Do not build generic option registries,
  presets, persistence, automatic quality adjustment, or controls for settings
  that cannot yet change at runtime.

Illustrative API shape, **not implemented or a final naming commitment**:

```mms
let current = graphics.get_mirror_quality()
graphics.set_mirror_quality(1024)
```

Prefer a scene-wide runtime override so mirrors created later follow the same
setting, while their authored `Mirror.quality(...)` values remain intact.
Decide how no override, mixed authored values, resetting to authored values,
and scene unload are represented before committing the getter's contract.
The panel's first control can label this value **Mirror resolution** while the
host API reuses the existing quality terminology.

## Existing foundation

- `src/engine/ecs/component/mirror.rs`: `Mirror.quality(N)` initializes the
  resolution preference, clamped to 64..=2048, default 512.
- `src/engine/ecs/system/mirror_system.rs` reads live component quality and
  publishes `resolution_scale = quality / 1024` to VisualWorld.
- `src/engine/graphics/vulkano_renderer.rs` derives the actual capture width
  and height from that scale and mirror aspect ratio.
  `ensure_mirror_offscreen_targets` already compares extent, format, and view
  count before recreating targets. Verify the complete update/publication path
  before assuming a live component edit is sufficient.
- Runtime texture publication also derives its sampled image extent from the
  capture image in `ensure_runtime_texture_target`. That function currently
  allocates a fresh publication image each capture; this is existing behavior,
  distinct from offscreen capture-target reuse. Record the baseline and verify
  resize/lifetime correctness without expanding this task into an unrelated
  texture-allocation redesign.
- `VulkanoRenderer::set_msaa_mode` currently rejects changes after renderer
  initialization. An MSAA slider/toggle would require separate runtime work;
  a graphics panel does not make every authored setting live automatically.
- `assets/components/ui/info_panel.mms` provides the existing panel factory
  and accordion restore behavior. `mouth_response_panel.mms` demonstrates
  sliders, `SliderChanged` callbacks, and readouts in this panel system.

## Investigation and decisions

1. Trace quality from a runtime edit through MirrorSystem, VisualWorld, target
   allocation, and texture/material publication. Inspect the current MMS
   runtime API surface; expose a narrow graphics namespace getter/setter using
   the existing `quality` terminology and clamp. Apply changes through the
   existing engine update boundary, not direct renderer calls from MMS UI code.
   Do not require scene rebuilding.
2. Define what the displayed resolution means. Quality is a base resolution
   preference, not necessarily a square extent. The label should distinguish
   the selected value from actual capture dimensions, for example
   `Mirror resolution: 1024 (capture 1536 × 1024)` for a 1.5-aspect mirror.
   If actual dimensions are not currently observable, add the smallest
   readout needed or label the selected value explicitly as the preference;
   do not display it as an exact square capture size.
3. Use one scene-wide mirror control through the host-backed graphics object.
   Define override/default/mixed-value behavior and handling for mirrors created
   later. Multiple aspect ratios still produce different capture extents; show
   actual dimensions per mirror when they differ.
4. Choose discrete resolution steps within the existing range, initially
   considering 64, 128, 256, 512, 1024, 1440, and 2048 so the current corp
   setting remains representable. Apply only changed values; coalesce rapid
   slider motion if necessary to avoid allocating a new target for every
   pointer event.
5. Verify safe replacement of GPU targets, descriptors, and runtime textures
   while frames are in flight. Old resources must remain alive as required,
   then be released; returning to a previous setting must not accumulate
   orphaned captures. Preserve mono and stereo viewer-family behavior.

## Panel implementation

- Build `graphics_settings_panel.mms` around `info_panel`, with a mirror
  resolution slider and live readout. Initialize from current mirror state.
- Include it in both corp examples where it can be reached with XR pointers
  and desktop input without blocking the mirror or walking route.
- Wire `SliderChanged` to the graphics host setter. Keep the label
  synchronized with the clamped/applied value, including
  changes through another supported runtime path. State how differing initial
  mirror preferences are shown before the slider applies one common value.
- Restore the body with its current value after accordion minimize/restore;
  avoid duplicate listeners or retained deleted slider components.
- Runtime resolution control does not require implementing adaptive mirror
  LOD. Keep that separate plan's authored-ceiling semantics in mind.

## Acceptance

- Changing the slider updates mirror sharpness and the label without a scene
  reload, on desktop and in XR. Both XR eyes remain correctly associated with
  their capture views.
- Exercise the minimum, maximum, current 1440 preference, rapid dragging,
  repeated changes, and panel minimize/restore. Invalid values are handled
  consistently by the runtime setter.
- Non-square mirrors preserve aspect ratio and show truthful dimensions or
  clearly named preferences. Mirror geometry and reflection orientation stay
  stable during target changes.
- Automated checks cover strict MMS host calls, getter/setter behavior,
  override handling, the authored panel, and unchanged capture-target reuse;
  a renderer smoke test verifies target/texture replacement and resource
  lifetime during repeated live changes.

## Related work

- [Mirror component](../spec/mirror-component.md)
- [Mirror system](../spec/mirror_system.md)
- [Adaptive mirror detail](../desktop/adaptive-mirrors.md)
- [Mirror viewer-family captures](mirror-viewer-family-captures.md)
