# Task: XrSettingsSystem and importable MMS floor-correction panel

Date: 2026-10-13.

Status: feasibility investigated; implementation proposed. Documentation only.

## Outcome

Provide an `XrSettingsSystem` backend, analogous to `GraphicsSettingsSystem`,
and an explicitly imported `xr_settings_panel(options)` MMS factory. Its first
setting is a live floor-height correction slider that can be operated in XR
without reloading the scene. Examples choose whether and where to attach it.
The setting remains effective when the panel is minimized, removed, or absent.

Retain the user's longer-term direction: a settings launcher/control panel with
Windows XP-era icon-and-label organization, using Mittens' editor/paint visual
language. Graphics and XR settings open as separate panels. Placeholder icons
with labels beneath them are sufficient initially; final icon design is later.
The launcher does not gate the floor-correction slice.

## Feasibility findings

| Capability | Current state | Required work |
| --- | --- | --- |
| Import a panel factory from an MMS asset | Implemented | Add the XR asset |
| Explicit scene placement or `mount.attach(panel)` | Implemented | Choose placement in a focused example |
| Native slider, pointer drag, numeric readout | Implemented | Author one setting row |
| Minimize/restore dynamic panel content | Implemented | Rebuild controls from backend readback |
| Typed nested settings API | Implemented for graphics | Register and dispatch `XrSettings` methods |
| Authoritative settings without a UI | Implemented for graphics | Add `XrSettingsSystem` and lifecycle wiring |
| Live XR floor correction | Absent | Integrate consistent tracking-to-world correction |
| Settings launcher with icons and labels | UI primitives exist; launcher absent | Later MMS factory and panel lifecycle policy |

The panel can be ordinary scene content; it does not require `EditorUI`, a new
Rust editor-panel variant, or a special panel loader. Static imports supply
factories that can also construct a panel in a later callback and attach it to
an existing scene mount. Do not add arbitrary runtime source evaluation just
to open a known settings asset.

### Existing panel/API examples

- [graphics_settings_panel.mms](../../assets/components/panels/graphics_settings_panel.mms)
  exports a factory around `info_panel`; its mirror Toggle changes real backend
  state. Its resolution Slider remains UI-only, so copy the backend-backed
  Toggle flow rather than treating that Slider as an implemented settings API.
- [graphics-settings.mms](../../examples/graphics-settings.mms) imports and places
  that panel as an independent authored subtree, with an XR avatar and mirror.
- [info_panel.mms](../../assets/components/ui/info_panel.mms) provides the title,
  styling, drag/accordion shell, and `info_panel_body` restoration path.
- [mouth_response_panel.mms](../../assets/components/ui/mouth_response_panel.mms)
  demonstrates native `SliderChanged`, signed numeric formatting, live setters,
  and silent `Slider.sync_value(...)` updates without a callback loop.
- [GraphicsSettingsSystem](../../src/engine/ecs/system/graphics_settings_system.rs)
  owns authoritative settings, suppresses unchanged notifications, and defines
  reset behavior independently of panel content.
- [runtime_config.rs](../../src/scripting/runtime_config.rs) registers the typed
  nested `GraphicsSettings.mirrors` namespace;
  [host.rs](../../src/scripting/host.rs) leases the backend and dispatches calls.
  The same infrastructure can expose `XrSettings`; no evaluator redesign is
  necessary.

## Meaning of floor correction

This setting corrects the application's interpretation of the XR runtime floor.
It moves the composed HMD, tracked hands, and rendered eyes together relative to
the grounded virtual environment. It does not edit the stage/floor geometry,
resize the avatar, calibrate knee posture, or change OpenXR's external room setup.

Recommended first convention, in raw STAGE metres:

```text
c = physical-floor Y in the runtime's reported STAGE coordinates
corrected_stage_position = raw_stage_position - [0, c, 0]

c = +0.15 m: lower the composed tracked poses by 15 cm
c = -0.15 m: raise the composed tracked poses by 15 cm
c = 0: preserve current behavior
```

Make the sign visible in the UI: label the row "Floor correction", display
signed centimetres, and explain that positive values lower the tracked view and
hands. A physical floor measured at raw Y=+0.15 should map to corrected Y=0.
Do not infer the value automatically from avatar soles, head height, or the
first startup pose.

This can compensate a tracking-floor error, but an avatar eye-to-sole mismatch
can still remain. Keep it distinct from
[AVC standing-height/sole calibration](avc-vr-standing-height-and-sole-floor-calibration.md).
Use raw controller/head diagnostics to identify which correction is needed.

## Backend ownership and draft API

Add `src/engine/ecs/system/xr_settings_system.rs` and a `SystemWorld.xr_settings`
field, following the role of `GraphicsSettingsSystem`. The backend owns the
validated correction, default zero, change revision/notification, and reset
policy. `OpenXRSystem` consumes an applied settings snapshot; panel closures
must not become the settings owner.

Proposed MMS API; these calls do not exist yet:

```mms parse-only
XrSettings.floor.set_correction_meters(0.15)
let correction = XrSettings.floor.correction_meters()
XrSettings.floor.set_correction_meters(0.0)
```

Keep `XrSettingsSystem` as the Rust backend name explicitly requested by the
user. The draft public table is `XrSettings`, parallel to `GraphicsSettings`.
Register finite numeric setter input and numeric readback in the existing typed
RuntimeSpec namespace. Use the host lease to reach the system from scene
evaluation, imported factories, and event callbacks, with no edit-mode dependency.

Recommended initial interval is -0.50 to +0.50 m, with a UI step of 0.005 m.
Validate in the backend too; reject non-finite or out-of-range API input without
changing state. Use the same range for the Slider. Coalesce rapid updates and
emit `XrSettingsChanged` only when the canonical value changes. Multiple panels
read one backend and synchronize current controls silently.

As with graphics, setters may update canonical settings synchronously so a
following getter sees the accepted value. Pose consumers must share a coherent
applied revision: a late UI callback must not make rendering use a new correction
while the avatar/hands still use the old one. Define the application boundary
explicitly and show the setting as accepted until that pose frame is presented;
never claim readback is a new raw OpenXR measurement.

First-slice scope is the local XR session/scene, not a per-avatar setting or a
multi-user preference service. Follow GraphicsSettingsSystem's default-reset
policy on scene/session replacement and close, documenting this behavior.
Panel deletion/minimization and temporary headset tracking loss do not reset it.
Changing the runtime's room calibration or reference space should be diagnosed;
do not silently derive another correction. Saved personal/runtime profiles and
cross-scene persistence are follow-up decisions.

## XR composition: the main implementation work

[openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs) currently creates
identity `STAGE`, caches raw poses, and composes them through separate paths:

1. `tick_with_queue`: headset through the tracked transform's parent world frame.
2. `tick_with_queue`: controllers/hand-root poses through the active XR rig origin.
3. `render_xr`: each eye through `xr_rig_origin_world`, published to VisualWorld.
4. Projection submission: original located eye poses in the original reference space.

The correction belongs to the shared **application tracking-to-world mapping**:

```text
world_from_tracked_pose = base_rig_world * T(0, -c, 0) * raw_stage_pose
```

Use that relationship once for every headset/controller/hand/eye consumer.
Preserve raw caches for diagnostics, and preserve located projection-view poses
with their original OpenXR space at submission. Do not correct raw poses and
also correct the rig origin. The correction is in STAGE metres before the rig
mapping; supported rig scale therefore transforms it consistently. Define the
supported upright/yaw/scale contract and diagnose unsupported frames.

Keep the existing rig origin used for locomotion/grounding independently
identifiable. AVC's capsule samples Y from the pre-tracking locomotion transform;
changing `c` must not change that source, its measured shape, or its bottom offset.
Putting an ordinary correction T above `InputXR` risks making the correction
part of that sampled Y. A correction on the outer physics root can be undone by
contact. A CXR-only wrapper does not move the rendered eyes on the current
InputXR path and would leave independent hands inconsistent.

Audit all uses of rig-origin composition before changing the helper. The existing
headset path and controller/render paths differ, so changing only
`xr_rig_origin_world` is insufficient. Support the active CXR path without AVC as
well as the current humanoid scene. Reuse the engine's existing single active
XR rig policy; adding independent multi-rig tracking is out of scope.

Scheduling must cover an update made by a slider callback after the XR pose tick.
Either consume one latched revision across the next update/render pose frame or
refresh affected headset/controller/AVC transforms before rendering the new
revision. Do not accept a permanent frame mismatch or filter only the camera.
Test existing raw-pose cache timing separately from correction-revision timing.

Changing only floor correction should not alter grounded state, inject Velocity,
rebuild the capsule, move static geometry, or change the movement root. Physical
crouch/stand remains free. Mounting/dismounting must retain the selected setting,
apply it once, and preserve existing vehicle anchor/input authority.

## Importable panel asset and explicit attachment

Proposed asset: `assets/components/panels/xr_settings_panel.mms`.
Export `xr_settings_panel(options)`; require a unique `root_name`, with optional
title/theme following existing panel conventions. The caller owns world placement.

Draft inclusion from an example; the asset does not exist yet:

```mms parse-only
import { xr_settings_panel } from "../assets/components/panels/xr_settings_panel.mms"

T.position(-1.28, 2.3, -1.0) {
    Grabbable {}
    xr_settings_panel({ root_name = "xr_settings" })
}
```

The same factory can be attached later to a caller-owned mount:

```mms parse-only
let panel = xr_settings_panel({ root_name = "xr_settings" })
settings_mount.attach(panel)
```

The panel contains one Slider, a signed centimetre readout, concise sign guidance,
and a reset-to-zero action. On creation/restoration it reads the backend. On
`SliderChanged` it calls the typed setter; `SliderCommitted` is available for a
future explicit commit/persistence policy. Use existing raycast priority and
thumb-travel conventions so XR lasers operate it like the graphics/mouth panels.

Rebuild deleted accordion content from backend state, resolve live control
handles within the panel instance, and silently sync external changes. Avoid
duplicate listeners and stale body handles after repeated minimize/restore or
launcher-driven deletion/recreation. Unique instance roots must isolate controls
even if more than one panel observes the same setting.

## Future launcher notes

Create an MMS settings-launcher factory later, with Graphics and XR entries.
Each clickable cell has an icon above its label, matching older Windows control
panels/desktop organization and Mittens' paint/editor styling. Reuse
[icons.mms](../../assets/components/icons.mms) or simple placeholder geometry;
final graphics/XR icon artwork is undecided.

An initial fixed two-entry layout can use existing flex rows/columns; it does not
require a new responsive grid or OS window manager. Opening an entry attaches
its imported factory to an explicit panel mount. Define duplicate-open, focus,
placement, close/minimize, and listener cleanup policy in that later task. An
already-open panel should be reused/revealed rather than spawning another copy.
The first XR panel remains explicitly importable without this launcher.

## Delivery and acceptance

1. Add `XrSettingsSystem`, typed namespace/host lease, validation, readback,
   notifications, and lifecycle reset. Test setters without any panel or XR
   hardware; invalid calls leave state unchanged.
2. Implement correction composition and coherent application timing. Synthetic
   nonzero poses prove +0.15 m lowers head, both hands, and both eyes by the
   corresponding mapped amount, preserving rotations and inter-eye separation.
   Reset returns to the original poses without accumulation. Raw cached poses
   and submitted projection poses/reference space remain unchanged.
3. Integrate with grounded AVC and a CXR rig without AVC. Repeated adjustments,
   crouch/stand, gamepad motion, gravity/contact, teleport, and mount/dismount
   must not make the capsule or movement-root Y chase the setting. Verify scale
   and the declared supported rig frame separately.
4. Add the reusable MMS panel and a focused `examples/xr-settings.mms` scene,
   reusing the graphics-settings example's stage/avatar/mirror fixtures. Verify
   explicit import and later attach under strict RuntimeSpec evaluation,
   slider/reset behavior, external backend updates, two panels, minimize/restore,
   and removal/recreation. No new Rust editor-panel type is required.
5. Headset acceptance: operate the slider with hand/controller rays, adjust in
   both directions, compare a physical-floor controller landmark to virtual
   support, and confirm eye/hand/avatar alignment and stable world contact.
   Keep real floor error separate from avatar proportion/sole calibration.

Investigation verification: `cargo test --lib graphics_settings_` passed all four
existing tests on 2026-10-13. They cover strict imported-panel evaluation,
backend/control/restore behavior, setter readback/reset, and argument validation.
They verify the reusable precedent, not the proposed XR correction. No new code,
assets, UI, runtime API, or headset verification was created in this task.

## Related work

- [Runtime graphics-settings panel/backend](runtime-mirror-resolution-info-panel.md)
- [AVC capsule Y independent of HMD tracking](avc-capsule-before-hmd-tracking.md)
- [AVC crouch IK](avc-foot-penetration-crouch-ik.md)
- [XR camera-wrapper compensation](xr-camera-offset-pose-driver-compensation.md)
  describes content anchoring; that mechanism has different scope from this
  tracking-floor correction.
