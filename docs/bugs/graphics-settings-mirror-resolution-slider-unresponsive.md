# Graphics settings mirror resolution Slider stays on Authored in XR

Date: 2026-10-14
Status: open; headset failure reported, source-based cause identified;
real ray/gesture reproduction still required. No implementation in this review.
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

- The example wraps the entire importable panel in `T { Grabbable {} ... }`.
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

1. [ ] Reproduce with the real imported panel, its outer Grabbable wrapper,
   initialized slider visuals/layout, a controller-backed pointer, and actual
   ray hits. Check DragStart/Move reach the Slider before checking resolution.
2. [ ] Prefer an MMS-only boundary: put `Grabbable.off()` on the resolution
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
