# Editor settings mode buttons appear inert until the settings panel is focused

## Status

Inconclusive investigation.

## Symptom

In `examples/vtuber-desktop.mms`, clicking the editor settings mode rows:

- `Select`
- `3D Cursor`
- `Select + Cursor`

can appear to do nothing until the editor settings panel title bar is clicked first.

The user-visible effect is that the mode rows do not seem to take effect until the panel is
"focused", after which the same controls begin behaving normally.

This is currently only reported in `vtuber-desktop`. The same behavior has not been confirmed in
`vtuber-mirror-example`, which raises the possibility that the apparent no-op is scene-specific or
even perceptual:

- overlay / hit-surface behavior may differ between the examples
- fixed-camera framing and view angle may make the mode change harder to observe in
  `vtuber-desktop`
- the first click may be working, while the follow-up scene interaction is what looks wrong

## Repro Status

The original repro was written against `vtuber-desktop`, but it should now be treated as
example-specific and not yet generalized:

1. In `vtuber-desktop`, the mode change can look like a no-op until a title-bar click.
2. In `vtuber-mirror-example`, the same interaction is not currently known to fail.

So this is not yet strong evidence of a generic editor-settings focus bug.

## Expected behavior

Clicking a mode row inside the editor settings panel should immediately change the active editor
interaction mode, regardless of whether that panel was already focused.

## Observed behavior

In `vtuber-desktop`, the mode rows can appear to behave like a no-op until the settings panel is
focused through a title-bar click or equivalent panel-shell interaction.

That appearance may be misleading. It is still possible that:

- the row selection and interaction-mode change are already happening
- the scene used to verify the mode change is what differs
- overlay / camera presentation in `vtuber-desktop` is masking the effect

## Investigation notes

The mode-change reducer path itself does not appear to require panel focus:

- `SelectionChanged` on `#editor_settings_selection` is translated directly into
  `EditorContextEvent::InteractionModeChanged` in
  [src/engine/ecs/system/editor/context.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/context.rs:477)
- that event updates the shared editor context and editor components in
  [src/engine/ecs/system/editor/context.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/context.rs:552)
  and
  [src/engine/ecs/system/editor/context.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/context.rs:1091)
- there are already tests covering settings-selection to interaction-mode mapping in
  [src/engine/ecs/system/editor/context.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/context.rs:1474)

One earlier suspicion was a panel-focus routing mismatch around settings-panel clicks:

- panel click handling calls `focus_panel_from_descendant_click(...)` before
  `handle_editor_settings_panel_click(...)` in
  [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:294)
- `focus_panel_from_descendant_click(...)` focuses:
  - inspector instances
  - world
  - assets
  - paint
  - grid
- but it does **not** include the editor settings panel in
  [src/engine/ecs/system/editor/inspector_panel.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/inspector_panel.rs:1480)

That means a click inside settings content is not handled by the shared "focus this panel from any
descendant click" path that the other panels use.

The settings panel does try to repair panel focus indirectly when its own selection changes:

- settings `SelectionChanged` forces `#editor_panel_layout_selection` to the settings panel root in
  [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:552)

But if the first click is being interpreted primarily as an outer shell/focus gesture, or if the
inner settings selection is not winning on that first interaction, then the user would see exactly
this "first click focuses, second click works" behavior.

## Current interpretation

The old "missing settings panel in descendant-click focus path" theory is no longer sufficient by
itself:

- that path has since been updated
- focused tests cover direct settings-row selection and panel-layout focus handoff
- the reported symptom still appears scene-dependent

So the remaining issue may be outside generic settings-panel selection/focus handling.

More plausible current buckets are:

- `vtuber-desktop`-specific overlay or hit-surface interference
- camera/framing making mode changes hard to perceive
- a downstream scene-interaction difference after mode selection, rather than a failed row click

## Recommended next checks

1. Add a focused regression test for "first click on an unfocused settings mode row changes
   `EditorComponent.interaction_mode` immediately".
2. Compare `vtuber-desktop` and `vtuber-mirror-example` directly for:
   - row-selection events
   - panel-layout focus events
   - resulting `EditorContextState.interaction_mode`
3. Instrument whether the first row click produces:
   - panel-layout `SelectionChanged`
   - settings `SelectionChanged`
   - both, and in what order
4. Verify whether the apparent failure is actually in the follow-up scene interaction, not in the
   settings row click itself.
5. Inspect whether the first click is hitting panel-shell renderables instead of the row
   option/raycastable subtree.

## Notes

This is related to, but distinct from:

- [cursor-3d-mode-does-not-respond-to-editor-clicks.md](/home/rei/_/cat-engine/docs/bugs/cursor-3d-mode-does-not-respond-to-editor-clicks.md:1)

That bug is about scene-click behavior after mode selection.
This bug is about the settings-panel mode controls themselves appearing to require focus before the
mode change takes effect.
