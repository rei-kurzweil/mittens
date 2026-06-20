# Grid panel does not refresh after placing a grid with the paint panel Grid Tool

## Status

Partially fixed on 2026-06-20, with follow-up verification still needed.

## Symptom

When using the paint panel `Grid Tool` to place a grid into the scene, the `grid_panel` does not
update to show the new grid immediately.

The new grid only appears in the panel after some other grid-panel-specific action happens, such as
clicking `Add Grid`.

The same stale-panel behavior also affects the startup default grid: the editor may ensure a
default hidden grid exists, but its row still does not appear until a later panel action forces the
list to rebuild.

There is also a related delete/runtime cleanup bug: deleting grid rows can leave live grid visuals
and hit state behind in the world, and the app may become noticeably slow after those deletes.

## Repro

1. Open an editor scene with the Paint panel and Grid panel visible.
2. Focus the Paint panel.
3. Select `Grid Tool`.
4. Place a grid into the scene using the normal paint/grid placement interaction.
5. Observe that the grid exists in the scene.
6. Observe that the `grid_panel` list does not update yet.
7. Click `Add Grid`.
8. Observe that the panel list now refreshes and the previously placed grid appears.

Startup variant:

1. Open an editor scene with editor panels enabled and no authored grids yet.
2. Observe that the startup/default grid is expected to exist as hidden editor state.
3. Observe that no corresponding row appears in `grid_panel` yet.
4. Click `Add Grid`.
5. Observe that the panel list rebuild now reveals the previously missing grid row(s).

## Expected behavior

Placing a grid via the paint panel should refresh the `grid_panel` immediately.

The panel should reflect the current set of editor grids without requiring an unrelated
panel-specific action to force a rerender.

That same rule should apply to ensured startup grids: if the grid state exists, the row should
exist, even if the grid starts hidden.

## Actual behavior

The panel model appears stale until a later action causes a grid-panel rerender path.

This suggests grid creation through the paint placement path is not emitting or triggering the same
refresh/rebuild path that the explicit `Add Grid` button uses.

The startup default-grid path appears to miss that same refresh/rebuild path.

Grid deletion also appears incomplete: the row can disappear from the panel while the live grid
runtime remains registered in the world and visual world.

## Root cause notes

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

Before the 2026-06-20 fix pass, `Add Grid` had an explicit rerender path, but paint-driven grid
placement appears not to flow through the same refresh mechanism.

The startup `ensure_default_grid(...)` path also appeared not to flow through the same refresh
mechanism because the first grid-panel render used a default editor context with no
`active_editor`, so no grid rows were enumerated yet.

The delete path used raw `world.remove_component_subtree(...)`, which bypassed the normal runtime
unregister path for live renderables and transforms. That explains stale grid visuals after delete
and is a plausible cause of the slowdown after deleting all grids.

## Current fix direction

The current implementation pass now:

1. ensures the default grid before panel bootstrap
2. renders the first grid panel with the owning editor root in context
3. routes delete through shared grid cleanup instead of raw subtree removal

Smoke testing is still needed to confirm the startup row now appears immediately and that deleting
grids fully removes their live runtime.

## Investigation targets

- `EditorPaintSystem`
  - where the grid preview becomes a committed grid
  - whether any panel refresh or data event is emitted at commit time
- `GridSystem`
  - whether startup/default-grid ensure logic marks editor grid state dirty for panel rebuilds
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
