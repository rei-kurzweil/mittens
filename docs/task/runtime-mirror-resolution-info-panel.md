# Task: Importable MMS graphics settings panel and mirror settings table

Date: 2026-10-12
Status: investigation and implementation planned; documentation only.
Architecture clarified 2026-10-12: MMS owns the importable panel factory;
the host-backed `GraphicsSettings.mirrors` built-in table exposes runtime
mirror controls. The asset and API described here are not implemented yet.

## Outcome

Build a reusable MMS `graphics_settings_panel` around the existing `info_panel`
factory, initially containing a mirrors on/off control, a mirror resolution
slider, and live readouts. Scenes import the exported factory from an ordinary
asset module and place its returned component subtree. Use it in mittens-corp
and mittens-corp-desktop. The panel can later grow controls for frame rate
versus image quality when those controls have working runtime APIs.

## Ownership and scope

- Keep the exported `graphics_settings_panel(options)` factory, layout, labels,
  toggle/slider callbacks, and accordion restore logic in
  `assets/components/ui/graphics_settings_panel.mms`. Follow the import/export
  and factory pattern used by `info_panel.mms` and `mouth_response_panel.mms`.
- Expose `GraphicsSettings` as a host-backed built-in MMS table. Initially its
  only entry is `mirrors`, which points to a table of mirror settings methods.
  It is available to scenes, imported factories, and callbacks without a
  settings component or a separate import of the API.
- Keep declarations and typed host dispatch in the existing RuntimeSpec/Mittens
  host boundary. Nested built-in table exposure needs investigation: current
  RuntimeSpec namespace APIs such as `File` and `JSON` are flat, and declaration
  names reject dots. Do not assume registering a namespace named
  `GraphicsSettings.mirrors` works. Add the smallest crate-owned runtime support
  needed for the requested table shape; no MMS type-annotation syntax is needed.
- Rust owns validation, authoritative runtime values, and renderer/resource
  changes. The UI must read actual applied values rather than maintain another
  independent graphics-settings state.
- Do not add a Rust `EditorPanel::Graphics`, hard-coded graphics panel rows,
  or new editor-context click handling. Explicit scenes attach the MMS panel
  factory as an ordinary authored component subtree.
- Prefer existing update paths for applying settings. If backend coordination
  warrants a `GraphicsSettingsSystem` (`graphics_settings_system.rs`), keep it
  responsible only for authoritative settings and applying changes. It must
  work without a panel and own no panel construction, layout, or input handling.
  A dedicated panel system is not required.
- Implement mirror control first. Do not build generic option registries,
  presets, persistence, automatic quality adjustment, or controls for settings
  that cannot yet change at runtime.

## Planned MMS API

The initial table shape and mutation method names are:

```mms
GraphicsSettings.mirrors.toggle(false)
GraphicsSettings.mirrors.quality(1024)
GraphicsSettings.mirrors.toggle(true)
```

| Method | Argument contract | Result |
| --- | --- | --- |
| `GraphicsSettings.mirrors.toggle(on_off)` | `bool`; explicitly set enabled/disabled rather than invert the current value | void/unit |
| `GraphicsSettings.mirrors.quality(q)` | `i32`; use the existing 64..=2048 clamp | void/unit |

These types describe host validation, not annotations to write in MMS source.
Both controls affect all mirrors in the active scene, including mirrors created
later and both mono/stereo viewer families. Initially mirrors remain enabled
and use their authored qualities until a runtime override is applied. Preserve
authored `Mirror.quality(...)` values rather than rewriting each component.
Repeated requests for the same value should cause no resource work.

Disable capture submission and associated publication when mirrors are off.
Define the surface's disabled appearance before implementation; do not leave
an apparently live reflection displaying a stale image. Prefer retaining valid
capture targets while temporarily disabled and refresh captures before a
re-enabled visible surface consumes them. A quality change while disabled must
be retained and applied when capture resumes without rendering in the meantime.

The panel needs to read authoritative enabled/quality state on creation,
after changes, and on accordion restore. Add a minimal readback contract under
`GraphicsSettings.mirrors`; its names and representation remain to be decided.
Resolve no override, mixed authored values, reset-to-authored behavior, and
scene-unload/reset lifetime before implementing that contract. Do not add
top-level entries or a `GraphicsSettings.toggle_mirrors` alias in the first slice.
The panel labels quality as **Mirror resolution preference**, since the value
does not by itself specify the complete capture dimensions.

Planned asset use follows ordinary MMS imports, for example from a scene in
`examples/`:

```mms
import { graphics_settings_panel } from "../assets/components/ui/graphics_settings_panel.mms"

let settings_panel = graphics_settings_panel({ root_name = "graphics_settings" })
T.position(2.0, 1.0, -2.0) { settings_panel }
```

`root_name` identifies the panel instance; the factory can provide default title
and styling. The caller owns its world placement. Final optional factory
options should follow the existing panel conventions.

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
   runtime API surface; expose the nested built-in table and its strict
   `toggle`/`quality` calls plus minimal state readback. Apply changes through the
   existing engine update boundary, not direct renderer calls from MMS UI code.
   Do not require scene rebuilding.
2. Define what the displayed resolution means. Quality is a base resolution
   preference, not necessarily a square extent. The label should distinguish
   the selected value from actual capture dimensions, for example
   `Mirror resolution preference: 1024 (capture 1536 × 1024)` for a 1.5-aspect mirror.
   If actual dimensions are not currently observable, add the smallest
   readout needed or label the selected value explicitly as the preference;
   do not display it as an exact square capture size.
3. Use scene-wide mirror settings through `GraphicsSettings.mirrors`. Define
   disabled surface behavior, override/default/mixed-value readback, handling
   for mirrors created later, and scene-unload/reset lifetime. Keep settings
   usable without the panel. Multiple aspect ratios still produce different
   capture extents; if displaying actual dimensions, report them per mirror
   when they differ.
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
  on/off control, resolution slider, and live readouts. Initialize from actual
  backend state, not factory-local graphics settings.
- Import its exported factory in both corp examples where it can be reached
  with XR pointers and desktop input without blocking the mirror or walking route.
- Wire the on/off control to `GraphicsSettings.mirrors.toggle(on_off)` and
  `SliderChanged` to `GraphicsSettings.mirrors.quality(q)`. Map the slider's
  selected discrete step to an integer quality before calling the host.
  Keep the enabled state and quality label synchronized with the applied
  backend state, including changes through another supported runtime path.
  State how differing initial
  mirror preferences are shown before the slider applies one common value.
- Restore the body with its current value after accordion minimize/restore;
  avoid duplicate listeners or retained deleted slider components.
- Runtime resolution control does not require implementing adaptive mirror
  LOD. Keep that separate plan's authored-ceiling semantics in mind.

## Acceptance

- The module can be imported and its factory used by another MMS module or
  example without adding a Rust panel type or dedicated panel system.
- Strict MMS evaluation accepts the nested table calls from scenes, factories,
  and callbacks, and rejects wrong argument counts/types consistently.
- Toggling mirrors off submits no mono/stereo mirror captures or publications.
  Toggling them back on refreshes reflections before use. Exercise repeated
  off/on requests, a quality change while off, and a mirror created while off.
- Changing the slider updates mirror sharpness and the label without a scene
  reload, on desktop and in XR. Both XR eyes remain correctly associated with
  their capture views.
- Exercise the minimum, maximum, current 1440 preference, rapid dragging,
  repeated changes, and panel minimize/restore. Invalid values are handled
  consistently by the runtime setter.
- Non-square mirrors preserve aspect ratio and show truthful dimensions or
  clearly named preferences. Mirror geometry and reflection orientation stay
  stable during target changes.
- Automated checks cover built-in table exposure, strict MMS host calls,
  toggle/quality behavior and readback, override handling, the imported panel
  factory, and unchanged capture-target reuse;
  a renderer smoke test verifies target/texture replacement and resource
  lifetime during repeated live changes.

## Related work

- [Mirror rendering improvement list](mirror-render-pass-status.md) — clipping,
  off-screen captures, and optional candidate tracking are separate from this
  MMS panel/API task.
- [Mirror component](../spec/mirror-component.md)
- [Mirror system](../spec/mirror_system.md)
- [Adaptive mirror detail](../desktop/adaptive-mirrors.md)
- [Mirror viewer-family captures](mirror-viewer-family-captures.md)
