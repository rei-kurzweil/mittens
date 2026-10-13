# Task: Importable MMS graphics settings panel and mirror settings table

Date: 2026-10-12
Updated: 2026-10-13
Status: capture Toggle backend implemented; headset freeze/resume check pending.
Runtime quality integration remains planned.
Architecture confirmed 2026-10-13: MMS owns the importable panel factory;
`GraphicsSettingsSystem` owns authoritative capture-enabled state and receives
updates from the host-backed `GraphicsSettings.mirrors` built-in table.
The system, `toggle(bool)`, and `enabled()` readback are implemented. The
`quality(i32)` API is the next slice.

## Current UI-first slice

- `assets/components/panels/graphics_settings_panel.mms` exports
  `graphics_settings_panel(options)` around `info_panel`. Its native Toggle
  freezes/resumes mirror capture work through the authoritative backend. The
  stepped Slider remains a UI-only preview. Minimize/restore reads actual
  enabled state and retains the preview Slider value.
- `examples/graphics-settings.mms` imports the panel in an XR-only Rei(mu) scene,
  with microphone mouth animation, independent eye closure tracking, ambient
  eye direction animation, and the corp rig's shirt/bow secondary motion.
  A grabbable full-body mirror starts beside the standalone panel at quality 512.
  There is no desktop camera. The user confirmed the scene and UI are working;
  the user will now verify the capture Toggle's freeze/resume behavior.
- `assets/components/stage_with_boxes.mms` supplies the original
  secondary-motion-desktop floor and collision/contact zones, eight colored
  boxes, and three tripod lights. Both scenes now import it. The new example
  uses a black background, the gold kawaii stars, and the same bloom settings.
- The shared accordion's `emit_data` call is registered in the strict MMS runtime
  and dispatched through the Mittens host to queued `DataEvent` delivery.
  Runtime checks cover strict scene/import evaluation, preview control callbacks,
  and repeated real chevron close/open clicks through engine subtree removal
  and reattachment. Tests also cover typed settings calls outside edit mode,
  read-after-write, scene reset, frozen eye/family texture identities, startup
  while off, mirrors created while off, and reflected-view refresh on resume.
  Live GPU/headset image verification remains with the user.

Run the preview with:

```sh
cargo run --release -- load examples/graphics-settings.mms
```

The first slice below records implemented behavior; runtime quality remains planned.

## First focused backend slice: mirror capture toggle

Connect the existing panel's Toggle end to end so it pauses and resumes mirror
captures in `examples/graphics-settings.mms`. Keep the Slider as its current
UI-only preview; runtime quality changes are the next slice.

Implementation checklist:

- [x] Add `GraphicsSettingsSystem` to `SystemWorld`, initially owning one
  authoritative mirror capture-enabled flag, defaulting to true. Define its
  scene unload/reset behavior without adding persistence.
- [x] Expose `GraphicsSettings.mirrors.toggle(on_off)` through the typed MMS
  host boundary, plus `GraphicsSettings.mirrors.enabled()` readback.
  Add only the runtime support needed for this nested table. Define how a
  setter reaches the system and when subsequent readback sees its applied value.
- [x] Publish the applied enabled flag to mirror/render code. Gate capture
  work before target allocation, drawing, and image publication while retaining
  mirror records, targets, textures, and per-eye/family texture selection.
- [x] Connect the panel's Toggle to the host setter. Initialize and restore it
  from authoritative readback, and keep its displayed value synchronized with
  backend changes. The panel must not become the settings owner.
- [x] Verify repeated off/on changes through MMS and the panel in runtime tests, including
  minimize/restore, startup while off, and a mirror created while off.
- [ ] User headset check: last images remain visible per eye while off, fresh
  reflections resume on, and repeated Toggle/accordion changes stay correct.

The host leases `GraphicsSettingsSystem` during main-thread dispatch. Setters
apply synchronously, so `enabled()` immediately following `toggle(...)` reads
the new applied value. Changed values queue a `GraphicsSettingsChanged`
`DataEvent`; the panel then synchronizes its native Toggle with the silent
`Toggle.sync_value(bool)` method. Repeating an unchanged value emits no update.
Scene/session replacement and closing the live MMS session reset enabled to true.
This path works without `Editor` or `EditorUI` and uses no editor panel APIs.

This slice is complete when:

- Off produces zero new mirror captures/publications, and existing surfaces
  show their last published images with the correct eye/family bindings.
  Surfaces without a completed capture show the existing black placeholder.
- On captures fresh images before the mirror surface consumes them, reusing
  valid resources. The settings work with or without a panel and without reload.
- Focused runtime tests cover host argument validation, applied state/readback,
  capture gating, and Toggle restore. A renderer smoke check and headset check
  confirm freeze/resume and per-eye image retention in the existing example.

Quality overrides, Slider backend integration, clipping, visibility culling,
adaptive detail, and texture-allocation redesign are outside this slice.

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
  `assets/components/panels/graphics_settings_panel.mms`. Follow the import/export
  and factory pattern used by `info_panel.mms` and `mouth_response_panel.mms`.
- Expose `GraphicsSettings` as a host-backed built-in MMS table. Initially its
  only entry is `mirrors`, which points to a table of mirror settings methods.
  It is available to scenes, imported factories, and callbacks without a
  settings component or a separate import of the API.
- Keep declarations and typed host dispatch in the existing RuntimeSpec/Mittens
  host boundary. RuntimeSpec now supports nested namespace tables through
  `NamespaceBuilder::namespace(...)`, with each name segment validated
  separately. `GraphicsSettings` contains the `mirrors` table. No MMS
  type-annotation syntax or evaluator-specific graphics state is required.
- `GraphicsSettingsSystem` owns authoritative runtime values and validation.
  The MMS host receives settings calls and forwards their updates to this
  system through the existing engine update boundary. The UI reads actual
  applied values rather than maintaining another independent settings state.
- Do not add a Rust `EditorPanel::Graphics`, hard-coded graphics panel rows,
  or new editor-context click handling. Explicit scenes attach the MMS panel
  factory as an ordinary authored component subtree.
- `src/engine/ecs/system/graphics_settings_system.rs` provides
  `GraphicsSettingsSystem`, integrated with `SystemWorld`. It manages settings state and
  applies updates independently of any panel. Panel construction, layout,
  callbacks, and input composition remain in the MMS asset.
- Implement mirror control first. Do not build generic option registries,
  presets, persistence, automatic quality adjustment, or controls for settings
  that cannot yet change at runtime.

## Backend state and update flow

`GraphicsSettingsSystem` is the single authoritative owner of mirror capture
enabled state and, once the quality slice lands, the optional scene-wide quality
override. Defaults are
captures enabled and no quality override, so authored `Mirror.quality(...)`
values continue to apply. New mirrors use the current system state.

The planned path is:

```text
MMS GraphicsSettings.mirrors call
  -> typed RuntimeSpec/Mittens host dispatch
  -> synchronous settings update during main-thread host dispatch
  -> GraphicsSettingsSystem validates and applies the value
  -> mirror/render code consumes applied settings
```

Host calls must update this system rather than change renderer resources or
rewrite every mirror component directly. State readback also comes from this
system. The capture Toggle applies synchronously as described above. Apply
settings before the affected captures in a frame, and coalesce repeated/rapid
changes where needed.
Keep the existing frame/submission ordering; disabling captures does not cancel
GPU work already submitted.

Implement capture toggle and authoritative readback first, then connect the
quality override and Slider. The panel is an ordinary scene asset throughout.

## Planned MMS API

The initial table shape and mutation method names are:

```mms
GraphicsSettings.mirrors.toggle(false)
GraphicsSettings.mirrors.quality(1024)
GraphicsSettings.mirrors.toggle(true)
```

| Method | Argument contract | Result |
| --- | --- | --- |
| `GraphicsSettings.mirrors.toggle(on_off)` | Implemented: `bool`; explicitly set capture enabled/disabled | void/unit |
| `GraphicsSettings.mirrors.enabled()` | Implemented: no arguments; read authoritative applied capture state | bool |
| `GraphicsSettings.mirrors.quality(q)` | Planned: `i32`; use the existing 64..=2048 clamp | void/unit |

These types describe host validation, not annotations to write in MMS source.
Both controls affect all mirrors in the active scene, including mirrors created
later and both mono/stereo viewer families. Initially mirrors remain enabled
and use their authored qualities until a runtime override is applied. Preserve
authored `Mirror.quality(...)` values rather than rewriting each component.
Repeated requests for the same value should cause no resource work.

**Off pauses captures and keeps the last reflection visible.** Skip the entire
mirror capture pass and associated image publication before target allocation
or submission. Continue drawing the mirror surface with its already-published
texture; retain valid capture targets and texture bindings. No extra snapshot
copy, surface hiding, or material replacement is needed for this behavior.

Preserve separate textures and bindings for each viewer family and each XR
eye while off. The image is intentionally frozen: its reflection/parallax does
not track viewer motion until captures resume. A mirror or viewer family with
no completed capture uses the existing black placeholder until enabled.

When toggled on, refresh captures before the consuming surface draw. A quality
change while off is retained in `GraphicsSettingsSystem` and applied when
capture resumes, without allocating replacement capture targets or rendering
in the meantime. Repeated off/on requests must preserve resource reuse.

The panel needs to read authoritative enabled/quality state on creation,
after changes, and on accordion restore. Add a minimal readback contract under
`GraphicsSettings.mirrors`; enabled state uses `enabled()`, while quality
readback's names and representation remain to be decided.
Resolve no override, mixed authored values, reset-to-authored behavior, and
scene-unload/reset lifetime before implementing that contract. Do not add
top-level entries or a `GraphicsSettings.toggle_mirrors` alias in the first slice.
The panel labels quality as **Mirror resolution preference**, since the value
does not by itself specify the complete capture dimensions.

Asset use follows ordinary MMS imports, for example from a scene in
`examples/`:

```mms
import { graphics_settings_panel } from "../assets/components/panels/graphics_settings_panel.mms"

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
   `toggle`/`quality` calls plus minimal state readback. Route updates and reads
   through `GraphicsSettingsSystem`, applying changes at the existing engine
   update boundary. Renderer/resource changes follow the applied settings.
   Do not require scene rebuilding.
2. Define what the displayed resolution means. Quality is a base resolution
   preference, not necessarily a square extent. The label should distinguish
   the selected value from actual capture dimensions, for example
   `Mirror resolution preference: 1024 (capture 1536 × 1024)` for a 1.5-aspect mirror.
   If actual dimensions are not currently observable, add the smallest
   readout needed or label the selected value explicitly as the preference;
   do not display it as an exact square capture size.
3. Use scene-wide mirror settings through `GraphicsSettings.mirrors` and their
   authoritative `GraphicsSettingsSystem` owner. Preserve the frozen-texture
   off behavior above. Resolve override/default/mixed-value readback and
   scene-unload/reset lifetime. Keep settings usable without the panel, and
   keep mirror records/texture selection available even when captures are
   disabled. Multiple aspect ratios still produce different
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

- The panel asset already provides the on/off control, resolution slider, and
  readouts. Replace its preview state initialization with authoritative
  `GraphicsSettingsSystem` readback through the host API and connect its
  callbacks to that same backend.
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
- Settings updates and readback use `GraphicsSettingsSystem` with or without
  a panel. Both newly created mirrors and existing mirrors follow its applied
  capture-enabled state and quality override without changing authored quality.
- Toggling mirrors off submits no mono/stereo mirror captures or publications.
  It retains the last published image and correct per-eye/family bindings while
  the mirror surface continues to draw. No capture-target replacements are
  allocated while off. Toggling back on refreshes reflections before use.
  Exercise repeated off/on requests, head movement while frozen, a quality
  change while off, and a mirror/viewer family created while off (black until
  its first capture).
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
