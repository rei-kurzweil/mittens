# Bug: World Panel Does Not Follow Scene Selection From Clicked Geometry

## Summary

Selecting scene geometry directly in the viewport can place the transform gizmo on the expected object, but the world panel does not move its row selection to that same node.

This is visible in `bisket-vr-demo` when clicking the ground box.

## Reproduction

Use `examples/bisket-vr-demo.mms`.

1. Run the demo and open the editor panels.
2. Click the ground box in the scene viewport.
3. Confirm the transform gizmo appears on the ground box.
4. Check the world panel selection state.

## Expected

- Scene selection and world-panel selection should stay in sync.
- If clicking scene geometry causes the editor to target a node and show the gizmo on it, the world panel should select that same node's row.

## Actual

- The gizmo appears on the clicked ground box.
- The world panel does not move its visual selection to that node.

## Current Suspicions

- The editor's scene-selection path and the world-panel sync path are still partially separate.
- `EditorComponent.selected` or editor context is likely updating from scene clicks, but `sync_world_panel_selection(...)` is either:
  - not being triggered for that path
  - or failing to map the selected runtime/scene target back to a visible world-panel row
- Since world-panel selection now uses:
  - `selected_component = UI row`
  - `selected_payload = semantic target`
  the reverse sync path may still be assuming direct row/component identity in some cases.

## Relevant Areas

- [src/engine/ecs/system/editor_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_system.rs)
- [src/engine/ecs/system/editor_context_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_context_system.rs)
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)
  - `sync_world_panel_selection`
  - world/editor selection change handlers

## Notes

- This is distinct from the earlier world-panel click bug.
- World-panel row clicks now visibly select and route correctly; the remaining failure is the opposite direction: scene selection is not reflected back into the world panel.
