# Phase 0: inspector panel multi-instance via pinning

Date: 2026-06-06

Status: planning only.

This is a `docs/task` note only. No `src/` or `assets/` changes are proposed here yet.

## Goal

Lay the architectural groundwork for the inspector details panel by ensuring:

1. **Pin-to-lock** works end-to-end — clicking the pin button on an inspector panel
   freezes it on its current selection and spawns a new unpinned panel next to it.
2. **Each inspector panel instance** has independent state (its own `inspected` component,
   its own sidebar selection, its own detail view).
3. **World panel selection** routes to the active (unpinned) inspector panel, not a pinned one.
4. **Layout root** accommodates multiple inspector panels side by side.

This is phase 0 because it's a prerequisite for the sidebar+detail architecture
(planned in `docs/task/inspector-details-panel.md`). The visual structure doesn't change
yet — this phase is about making the state plumbing and panel spawning robust.

## Current state

The reducer `reduce_inspector_workspace_state` in `editor_inspector_system.rs` already
handles all three events (`SelectionChanged`, `PanelFocused`, `PanelPinToggled`) and
`InspectorWorkspaceState` already stores `panels: Vec<InspectorPanelState>` with
per-panel `inspected`, `pinned`, `subtree_selection`, and `scroll_offset`.

Multi-instance behavior is **tested** (see `setup_panels_for_editor_pinned_inspector_spawns_second_instance_for_new_selection`
at line 788), and the pin-button click path is wired through
`handle_inspector_panel_workspace_click` → `PanelPinToggled` event → reducer.

But there are gaps in the visual/spawning layer:

### Gap 1: world panel selection routing

When a user clicks a row in the **world panel** (not the inspector), the selection change
updates `EditorContextState.selected_component` but does **not** emit a
`InspectorWorkspaceEvent::SelectionChanged` event for the inspector workspace.

Currently the inspector panel only updates its selection when:
- The user clicks an inspector panel instance (focus change)
- The user clicks the pin button
- `setup_panels_for_editor` is called explicitly

There is no handler that watches `EditorContextState.selected_component` and translates
it into an `InspectorWorkspaceEvent::SelectionChanged` for the active inspector panel.

**Fix (Option A — chosen):** Install a second handler on `SignalKind::SelectionChanged`
scoped to the same `panel_query_root` that the editor context handler uses. The inspector
adapter already imports `resolve_semantic_target_from_payload` — it can extract the
selected component directly from the signal without going through `EditorContextState` at all.

Flow when `SelectionChanged` fires:

```
SelectionChanged signal
  ├→ EditorContext handler: update EditorContextState, sync editor selection
  └→ Inspector handler: determine selected component, feed to inspector reducer
       └→ reduce_inspector_workspace_state(SelectionChanged { editor_root, selected_target })
            └→ finds active_panel (unpinned), updates that panel's `inspected` field
            └→ if active panel is pinned, spawns a new unpinned panel
       └→ re-render only the affected inspector panel instance's content subtree
            (not all panels, not the full layout)
```

The inspector handler runs the reducer on `InspectorWorkspaceState` directly (same as
`handle_selection_changed_for_inspector_workspace` does today when called from click
handlers). The reducer already handles the pinned/unpinned distinction correctly.

After the reducer updates the workspace state, only the specific inspector panel instance
whose `inspected` changed gets its **detail view content** re-rendered (sidebar rows
and detail fields). Other panels' instance roots stay attached and untouched.

### Gap 2: pinned-panel detachment from world selection

When the active panel is pinned and a new world-panel selection arrives, the reducer
already spawns a new panel (`should_spawn` branch at line 122-142 of `editor_inspector_system.rs`).
This works correctly in tests — but the spawning path in the stopgap adapter
(`rerender_inspector_panels`) may not handle rapid add/remove well since it detaches and
reattaches every instance root every tick (lines 2416-2428).

**Fix:** `rerender_inspector_panels` should preserve the attach order of existing panels
and only detach/reattach when the panel order actually changed. Or, keep the current
approach but verify it handles 2+ inspector instances without visual flicker.

### Gap 3: layout root width for multiple inspector panels

The layout root's `available_width` budget is computed in `spawn_panel_layout()` using
fixed constants. When a second inspector panel spawns, the budget is already stale
(the 10× fudge multiplier on the initial guess covers extra panels for now, but it's
fragile).

Once `LayoutRoot { available_width: auto }` (from `docs/task/layout-root-auto-dimensions-and-computed-size.md`)
lands, this is solved naturally — panels advance the inline cursor and the root sizes
to fit. Until then, the panel strip width formula in `spawn_panel_layout` needs to
account for variable inspector panel count:
```
let inspector_count = inspector_models.len().max(1) as f64;
```

This is already computed as `inspector_models.len()` in the formula at line 1289, so
it should already scale — but verify that `spawn_panel_layout` is re-invoked when
inspector panel count changes (it currently only runs once at setup).

**Fix:** Either re-run `spawn_panel_layout` when the panel count changes (expensive),
or adopt `available_width: auto` so the layout naturally stretches.

### Gap 4: inspector panel content update for non-focused panels

When a pinned inspector panel's component tree changes (e.g., a child is added/removed),
the pinned panel's sidebar rows should reflect that. Currently `refresh_inspector_panels_from_workspace`
rerenders all panels, so this should work — but the trigger for refresh may not fire
for pinned panels since `handle_selection_changed_for_inspector_workspace` only runs
when the selection changes.

Currently, `sync_and_refresh_inspector_panels` is called in the main tick loop
(see `editor_inspector_system_stopgap_mms_adapter.rs:906`), so all panels do refresh.
But confirm this covers pinned panel tree mutations.

## What's already done (no new work)

- `InspectorPanelState`, `InspectorWorkspaceState`, `InspectorPanelId` types ✅
- `InspectorWorkspaceEvent` enum with `SelectionChanged`, `PanelFocused`, `PanelPinToggled` ✅
- `reduce_inspector_workspace_state` reducer ✅
  - Correctly spawns new panel when active panel is pinned ✅
  - Correctly retargets active panel when unpinned ✅
- Pin button spawn and render ✅
- `rerender_inspector_panels` handles arbitrary count of panel instances ✅
- `handle_inspector_panel_workspace_click` routes clicks to correct panel ✅
- Tests for pin-then-select behavior ✅

## What needs work

| # | Gap | Location | Effort |
|---|---|---|---|
| 1 | World panel selection doesn't route to active inspector | Adapter + handler | Medium |
| 2 | Detach/reattach in `rerender_inspector_panels` may flicker | Adapter | Small |
| 3 | Layout root width doesn't adapt to panel count dynamically | Adapter or `available_width: auto` | Medium |
| 4 | Pinned panel refresh on tree mutation (likely works) | Adapter (verify) | Small |

## Acceptance criteria

- Clicking a world panel row updates the active (unpinned) inspector panel's content.
- Clicking the pin button freezes the panel; a new unpinned panel spawns.
- The new unpinned panel shows the next world panel selection.
- Both pinned and unpinned panels are visible in the layout.
- No detach/reattach flicker when panels are re-rendered in place.
- The layout root accommodates the correct number of inspector panels.

## Dependencies

- `docs/task/layout-root-auto-dimensions-and-computed-size.md` — for clean layout adaptation
  to variable panel count (alternative: keep the fudge multiplier during phase 0).
- `docs/task/inspector-details-panel.md` — builds on phase 0 to add the sidebar+detail
  visual structure.

## Relevant files

- `src/engine/ecs/system/editor_inspector_system.rs` — state types, reducer
- `src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs` — adapter,
  panel spawning, click handling
- `src/engine/ecs/system/editor_context_system.rs` — `EditorContextState` with
  `selected_component`
- `src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2377-2442` —
  `rerender_inspector_panels`
- `src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:2618-2655` —
  `handle_inspector_panel_workspace_click`
- `assets/components/panels.mms` — `inspector_panel()` factory
