# Task: Narrow Down World/Inspector Panel Selection Refresh Slowness

Date: 2026-06-10

Status: investigation plan.

## Problem

Selecting objects inside an `EditorComponent` tree, especially through gizmo interaction,
still feels slower than it should even after paint input routing was structurally filtered.

Recent isolation result:

- when editor-root paint handlers are blacklisted outside Paint focus, input feels more responsive
- when editor-root panel refresh is also blacklisted, gizmo interaction becomes much faster
- world and inspector panels remain usable, but they no longer react to scene selection changes

That strongly suggests the expensive path is not just gizmo dragging itself. It is likely
scene-selection-driven world/inspector panel update work.

## Current isolation point

There is now a temporary debug switch in
[src/engine/ecs/system/editor/context.rs](../../src/engine/ecs/system/editor/context.rs)
that can blacklist the named editor-root handler `"editor_panel_refresh"`.

That handler is installed in
[src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)
on each `editor_root` and currently does:

1. `sync_world_panel_selection(...)`
2. `sync_and_refresh_inspector_panels(...)`

If blacklisting that handler makes gizmo interaction much faster, the slowdown is very likely
in one or more of:

- selection-to-world-panel model sync
- inspector workspace sync
- world panel rerender
- inspector rerender
- topology churn / detach-reattach inside those rerenders
- signal fanout caused by those updates

## Relevant codepaths

- `install_editor_refresh_handlers(...)`
  [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)
- `sync_world_panel_selection(...)`
  [src/engine/ecs/system/editor/world_panel.rs](../../src/engine/ecs/system/editor/world_panel.rs)
- `sync_and_refresh_inspector_panels(...)`
  [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)
- `rerender_world_panel_content(...)`
  [src/engine/ecs/system/editor/world_panel.rs](../../src/engine/ecs/system/editor/world_panel.rs)
- `rerender_inspector_panels(...)`
  [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)

## Goal

Systematically determine which stage of selection-driven panel updates is responsible for the
perceived pause during gizmo placement / selection changes.

The first pass should answer:

1. Is the time mostly in model-building/sync or in rerendering?
2. Is the cost in world panel refresh, inspector refresh, or both?
3. Is the cost CPU work inside one call, or a chain of secondary signals / rerenders?
4. Does the cost happen on every `SelectionChanged`, only on gizmo-attached transforms, or only
   on certain authored subtree shapes such as painted icons?

## Suggested instrumentation phases

### Phase 1: time the top-level editor-root refresh handler

At the named `"editor_panel_refresh"` handler:

- log start/end duration for the whole handler
- include:
  - `editor_root`
  - selected component
  - whether the selected component changed from the previous one

This confirms how much of the stall is inside that one handler.

### Phase 2: split timings by substep

Add timing around:

1. `sync_world_panel_selection(...)`
2. `sync_and_refresh_inspector_panels(...)`

Then, inside the latter path, time:

1. inspector workspace sync
2. inspector model build
3. `rerender_inspector_panels(...)`

And for world panel:

1. world panel model rebuild / selection sync
2. `rerender_world_panel_content(...)`

### Phase 3: count topology churn

During rerenders, log:

- number of detached subtrees
- number of attached subtrees
- number of rows/items regenerated
- number of panels regenerated

This should clarify whether the pause is dominated by structural re-materialization.

### Phase 4: detect signal cascades

Add short trace counters around:

- `SelectionChanged`
- `LayoutRootSizeAvailable`
- panel-owned `Click`
- any panel-selection intents emitted during refresh

The question is whether one scene selection triggers:

- one panel refresh
- or several chained panel refreshes / layout shifts / selection writes

## Suggested A/B toggles

These are useful because the system already supports router blacklisting by handler name.

### Toggle A: blacklist `editor_panel_refresh`

This is the current coarse switch.

Expected result:

- scene selection still happens
- gizmo still works
- world/inspector panels stop following scene selection

### Toggle B: skip only world panel sync

Temporarily early-return before `sync_world_panel_selection(...)`.

### Toggle C: skip only inspector sync/refresh

Temporarily early-return before `sync_and_refresh_inspector_panels(...)`.

### Toggle D: allow sync but skip rerender

Keep state/model updates, but suppress:

- `rerender_world_panel_content(...)`
- `rerender_inspector_panels(...)`

This separates model work from visual rebuild work.

## Repro cases to compare

Use the same scene and same camera path for each:

1. select ordinary authored transform
2. place gizmo on ordinary authored transform
3. select painted icon / painted asset instance
4. drag gizmo on painted icon

For each case record:

- total handler time
- world-panel time
- inspector time
- whether layout-size events fired

## Likely outcomes

### If world panel dominates

Focus on:

- scene model caching
- avoiding full row rerender when only selection changed
- avoiding detach/reattach of unchanged rows

### If inspector dominates

Focus on:

- reducing full detail subtree rebuilds
- preserving stable rows/panels when only selection target changes
- avoiding duplicate sync + rerender passes

### If cascades dominate

Focus on:

- eliminating selection writes during refresh when state is already current
- suppressing redundant layout/selection feedback loops
- narrowing which panel owns which secondary signals

## Deliverable

A short follow-up note with:

1. measured timings by phase
2. the single worst callsite or cascade
3. the smallest change likely to remove the hitch

## Related

- [docs/task/editor_selection_and_paint_perf.md](./editor_selection_and_paint_perf.md)
- [docs/task/editor-input-routing.md](./editor-input-routing.md)
- [docs/task/inspector-panel-phase0-pin-and-multi-instance.md](./inspector-panel-phase0-pin-and-multi-instance.md)
- [docs/bugs/world-panel-does-not-follow-scene-selection-from-clicked-geometry.md](../bugs/world-panel-does-not-follow-scene-selection-from-clicked-geometry.md)
