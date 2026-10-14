# Standalone graphics settings controls create scene-selection gizmos

Date: 2026-10-14
Status: MMS selection barrier implemented; automated coverage passes,
headset verification pending.
Example: `examples/graphics-settings.mms`.

## Observation and expected behavior

Clicking graphics settings controls operates them but also puts a transform
gizmo on what was clicked. The editor UI does not exhibit this behavior.
This is a generic authored panel, usable outside edit mode. Its contents are
controls, not scene-selection targets; retain their raycast/click/drag behavior
and the panel's deliberate movement affordances.

## Current difference from editor UI

[graphics_settings_panel.mms](../../assets/components/panels/graphics_settings_panel.mms)
previously had no Selectable-off boundary. The graphics factory now attaches
one to its inner panel root, which moves with a title grip and remains an
ancestor of controls and recreated bodies. This does not change all shared
info-panel consumers.

[editor_scene_hit](../../src/engine/ecs/system/editor_scene_hit.rs) explicitly
rejects hits below `editor_runtime_ui_root` and checks Selectable-off on each
ancestor or its immediate children. The standalone panel has neither that
editor-specific root nor an explicit off marker. It can therefore resolve as
scene geometry; this resolver even permits a hit with no enclosing Editor.
The exact handler producing the reported gizmo still needs event tracing.

## Focused fix plan

- [x] Add `Selectable.off()` at the graphics factory's stable moving panel
  root so title, Toggles, Slider, generated style backgrounds, and restored
  bodies all inherit it. Keep this scoped to the graphics panel initially;
  changing every shared info-panel consumer needs a separate compatibility
  check. A parent marker survives body removal/recreation.
- [x] Keep Raycastable enabled for UI events. Selectable-off does not mean
  Raycastable-off, and an explicit Grabbable/Draggable can remain a movement
  affordance without making controls editor-selection targets.
- [x] Assert the real world-scene resolver rejects hits on every control and
  background after initialization and after accordion restore. Verify the
  settings callbacks still run and no gizmo/scene-selection change occurs.
- [ ] Verify selection exclusion and control input in the headset, including
  while gripping the custom title and after accordion restore.
- [ ] Align paint's sidecar-aware exclusion in the
  [self-hit color bug](xr-color-tool-targets-own-laser-and-avatar.md) so the
  same marker protects this panel from color-tool edits when editor-authored.
  The current example's panel lies outside ED; do not assume its reproduction
  follows the same scoped paint-handler path as the avatar.

This is separate from the
[Slider's grabbable-ancestor conflict](graphics-settings-mirror-resolution-slider-unresponsive.md).
Selectable-off alone does not fix XR trigger drag eligibility.
