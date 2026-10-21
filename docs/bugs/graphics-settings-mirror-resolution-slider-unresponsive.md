# Graphics settings mirror resolution Slider stays on Authored in XR

Date: 2026-10-14
Status: MMS control boundaries implemented; automated controller ray/trigger
regression passes. Headset verification and initial Authored-label ambiguity
remain open.
2026-10-14 follow-up: the whole-panel grab marker was removed. The example now
supplies a custom component title with title-only `Grabbable.parent()`;
automated ownership checks confirm the Slider is outside that grab scope.
Headset input/image checks and the initial Authored-label ambiguity remain open.
2026-10-20 follow-up: the Mittens Corp scenes reintroduced whole-panel grab
wrappers, blocking both mirror resolution and XR floor correction Sliders.
The shared graphics and XR settings prefabs now put `Grabbable.off()` on their
control containers. Labels and title bars retain the panel's grip target.
`settings_panels_in_grabbable_wrappers_allow_xr_slider_drags` reproduces the
original failure and passes with the boundaries: it lays out the real imported
prefabs, casts controller rays against their geometry, presses/moves/releases
the trigger, and verifies backend values and readouts before and after restore.
It also checks that slider input leaves the panel stationary and that title
geometry still resolves the panel's grab target. No setter or drag events are
injected directly.
Example: `examples/graphics-settings.mms`.
Task: [runtime graphics controls](../task/runtime-mirror-resolution-info-panel.md).

## Observed behavior and label meaning

Secondary-motion and mirror Toggles work, but the resolution readout says
`Authored` and attempts to change the Slider do not apply a resolution.

`Authored` means `GraphicsSettings.mirrors.quality_override()` returned null.
MirrorSystem therefore uses each Mirror component's authored quality; this
example has `Mirror.quality(512)`. It is a default-state label, not a disabled
control. The Slider should call `GraphicsSettings.mirrors.quality(q)` and show
the applied integer after a changed step. This explanation does not excuse
the reported inability to use the control.

The initial thumb is at step 3 (512) even with no override. Selecting that same
step emits no SliderChanged because the native value did not change. That is
an additional first-use ambiguity, particularly with mixed authored qualities;
it does not explain failure to drag to another step.

## Source-based gesture conflict

- The reported version wrapped the entire panel in `T { Grabbable {} ... }`.
- [GestureSystem](../../src/engine/ecs/system/gesture_system.rs) rejects an
  XR trigger drag candidate when it resolves a Grabbable owner and no
  Draggable owner. Grip grabbing is a separate path.
- The Slider track/thumb have enabled Raycastable markers, but no Draggable
  marker or local Grabbable-off boundary. Their ancestry reaches that wrapper.
  The accordion title's Draggable is on a separate title subtree and does not
  authorize dragging the Slider.
- [SliderSystem](../../src/engine/ecs/system/slider_system.rs) starts pointer
  editing on trigger `DragStart`. The skipped Slider therefore receives no
  start event for that interaction, and no SliderChanged reaches the MMS host.
  Raising interaction priority does not overcome this eligibility filter.
- Click targets are chosen independently, so working Toggle clicks do not
  demonstrate that Slider drag acquisition works. A different scene hit may
  supply the drag context while a control still wins the click.

The source conflict matches the report; confirm it with actual hit/gesture
traces before declaring the full XR failure fixed. Also verify track placement
and hit surfaces through the real layout/transform stage.

## Coverage gap and focused fix plan

The previous panel test directly queued `SliderSet { emit_changed: true }`.
It proved callback/backend/readback/restore behavior, while bypassing ray hits,
XR trigger acquisition, drag mapping, and this grabbable-ancestor rule.

1. [x] Reproduce with the real imported panel, its outer Grabbable wrapper,
   initialized slider visuals/layout, a controller-backed pointer, and actual
   ray hits. Check DragStart/Move reach the Slider before checking resolution.
2. [x] Prefer an MMS-only boundary: put `Grabbable.off()` on the resolution
   control's containing Transform, or limit the panel's grab marker to its
   title/handle. The current grabbable resolver stops at the nearest disabled
   marker. Preserve title dragging and intentional grip movement.
3. [ ] If sliders inside grabbable containers should work generally, review
   a narrowly defined slider/control exception to trigger acquisition. Adding
   Draggable blindly can also activate transform dragging; do not turn the
   Slider into a movable scene object just to pass that filter.
4. [ ] Clarify the initial readout, e.g. `512 (scene default)` for a verified
   uniform scene value, with truthful mixed/no-mirror states. Keep the runtime
   override optional and authored qualities unchanged. Provide an explicit
   way to apply/reset a preference if choosing the current thumb is unchanged.

Do not debug GPU resizing first while the setter is never reached. After input
works, validate per-eye resizing, rapid dragging, quality changes while captures
are frozen, and fresh images on resume using the existing task's live checks.
Cover minimize/restore and the panel's
[selection barrier](graphics-settings-panel-controls-selectable.md) too.
