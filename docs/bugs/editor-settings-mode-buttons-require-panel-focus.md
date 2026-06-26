# Editor settings mode buttons appear inert until the settings panel is focused

## Status

Open bug / investigation.

## Symptom

In `examples/vtuber-desktop.mms`, clicking the editor settings mode rows:

- `Select`
- `3D Cursor`
- `Select + Cursor`

can appear to do nothing until the editor settings panel title bar is clicked first.

The user-visible effect is that the mode rows do not seem to take effect until the panel is
"focused", after which the same controls begin behaving normally.

## Repro

1. Run `vtuber-desktop`.
2. Open the shared editor UI.
3. Without first clicking the editor settings panel title bar, click one of:
   - `Select`
   - `3D Cursor`
   - `Select + Cursor`
4. Try the corresponding scene interaction.
5. Observe that the mode change appears not to take effect.
6. Click the editor settings title bar to focus the panel.
7. Click the same mode row again.
8. Observe that the mode now behaves as expected.

## Expected behavior

Clicking a mode row inside the editor settings panel should immediately change the active editor
interaction mode, regardless of whether that panel was already focused.

## Actual behavior

The mode rows can behave like a no-op until the settings panel is focused through a title-bar
click or equivalent panel-shell interaction.

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

The stronger current suspicion is a panel-focus routing mismatch around settings-panel clicks:

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

## Likely root cause

The editor settings panel is missing from the common descendant-click panel-focus path.

That leaves settings behavior split across two mechanisms:

- shell/title-bar focus through panel-layout selection
- row selection through `#editor_settings_selection`

Other panels get a shared focus handoff on any descendant click; settings currently appears to rely
on narrower follow-up paths.

## Recommended next checks

1. Add a focused regression test for "first click on an unfocused settings mode row changes
   `EditorComponent.interaction_mode` immediately".
2. Instrument whether the first row click produces:
   - panel-layout `SelectionChanged`
   - settings `SelectionChanged`
   - both, and in what order
3. Extend `focus_panel_from_descendant_click(...)` to include
   `#editor_settings_panel_root`, then verify whether that alone fixes the repro.
4. If not, inspect whether the first click is hitting panel-shell renderables instead of the row
   option/raycastable subtree.

## Notes

This is related to, but distinct from:

- [cursor-3d-mode-does-not-respond-to-editor-clicks.md](/home/rei/_/cat-engine/docs/bugs/cursor-3d-mode-does-not-respond-to-editor-clicks.md:1)

That bug is about scene-click behavior after mode selection.
This bug is about the settings-panel mode controls themselves appearing to require focus before the
mode change takes effect.
