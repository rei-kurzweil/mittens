# Grid panel does not refresh after placing a grid with the paint panel Grid Tool

## Status

Open bug / investigation.

## Symptom

When using the paint panel `Grid Tool` to place a grid into the scene, the `grid_panel` does not
update to show the new grid immediately.

The new grid only appears in the panel after some other grid-panel-specific action happens, such as
clicking `Add Grid`.

## Repro

1. Open an editor scene with the Paint panel and Grid panel visible.
2. Focus the Paint panel.
3. Select `Grid Tool`.
4. Place a grid into the scene using the normal paint/grid placement interaction.
5. Observe that the grid exists in the scene.
6. Observe that the `grid_panel` list does not update yet.
7. Click `Add Grid`.
8. Observe that the panel list now refreshes and the previously placed grid appears.

## Expected behavior

Placing a grid via the paint panel should refresh the `grid_panel` immediately.

The panel should reflect the current set of editor grids without requiring an unrelated
panel-specific action to force a rerender.

## Actual behavior

The panel model appears stale until a later action causes a grid-panel rerender path.

This suggests grid creation through the paint placement path is not emitting or triggering the same
refresh/rebuild path that the explicit `Add Grid` button uses.

## Likely root cause

The paint/grid placement path creates the grid preview / final grid, but the shared editor UI layer
does not appear to rerender the grid panel afterward.

Relevant paths:

- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:1229)
  starts `GridTool` placement with `start_grid_preview_session(...)`
- [src/engine/ecs/system/editor_paint_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:1280)
  creates the grid preview via `spawn_default_grid_for_editor(...)`
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2463)
  owns the current grid-panel click handling and rerender behavior
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2690)
  rerenders grid-panel content from editor context

Today, `Add Grid` has an explicit rerender path, but paint-driven grid placement appears not to
flow through the same refresh mechanism.

## Investigation targets

- `EditorPaintSystem`
  - where the grid preview becomes a committed grid
  - whether any panel refresh or data event is emitted at commit time
- stopgap adapter / shared panel runtime
  - whether grid panel updates are only tied to direct grid-panel clicks
- editor context / workspace events
  - whether there is a missing generic "editor content changed" or "grid set changed" signal

## Likely implementation direction

Do not special-case this as "click Add Grid after paint."

Instead:

1. identify the point where a grid-tool placement is committed
2. emit a shared refresh/reducer event for editor grid content
3. have the grid panel rerender from that shared event path, not only from direct grid-panel UI
   actions

## Related

- [docs/task/grid-tool-and-surface-placement-followups.md](/home/rei/_/cat-engine/docs/task/grid-tool-and-surface-placement-followups.md:233)
- [docs/task/grid-panel-and-grid-inspector.md](/home/rei/_/cat-engine/docs/task/grid-panel-and-grid-inspector.md:475)
