# Editor stopgap adapter next steps

Date: 2026-06-12

Status: immediate task list / dependency-ordered migration plan

## Goal

Turn the existing stopgap-adapter decomposition work into a short, concrete sequence of implementation steps.

This note is intentionally narrower than:
- [editor-stopgap-adapter-decomposition.md](/home/rei/_/cat-engine/docs/task/editor-stopgap-adapter-decomposition.md:1)
- [panel-system.md](/home/rei/_/cat-engine/docs/task/panel-system.md:1)
- [data-renderer-system-for-editor-ui.md](/home/rei/_/cat-engine/docs/task/data-renderer-system-for-editor-ui.md:1)

It is about what to migrate next, in dependency order, so the repo stops deepening:
- [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)

## Current state

Some of the generic runtime extraction has already landed:
- [`src/engine/ecs/system/panel_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:1)

That file already contains:
- `PanelKind`
- `PanelSlotKind`
- `PanelShellSpec`
- `PanelInstance`
- `PanelActionKind`
- `PanelActionPayload`
- runtime UI root / panel layout helpers
- generic panel action payload decoding helpers

So the immediate work is no longer "invent the runtime vocabulary."
The immediate work is:
- finish moving generic responsibilities onto that runtime layer
- move panel-specific behavior behind panel modules
- shrink the stopgap adapter from orchestrator-of-everything into a temporary wrapper

## Guiding rule for sequencing

Migrate the lowest-risk, most reusable generic ownership first.

That means:
1. finish generic runtime seams before panel-specific rewrites
2. move world-panel and inspector-panel behavior behind those seams before adding more panel complexity
3. only then add or expand panels like `grid_panel` on top of the new structure

## Immediate task list

### 1. Make `panel_system` the single owner of mounted-panel discovery

Why first:
- this is the cleanest generic seam already partially implemented
- it removes repeated selector walks and slot lookup from the stopgap adapter
- later panel migrations depend on stable mounted-panel identity

Work:
- move any remaining ad hoc shell-root and slot lookup into `panel_system`
- make panel mounting return resolved `PanelInstance` data immediately
- stop re-finding panel roots/slots in panel-specific code when the instance is already known
- define where mounted instances are cached or re-derived per tick

Done when:
- world/inspector/grid shell resolution goes through one path
- panel-specific code receives `PanelInstance` instead of raw selectors whenever possible

### 2. Extract a small editor workspace runtime/state module

Why second:
- the adapter still mixes runtime UI root ownership, panel layout ownership, focused panel bookkeeping, and panel ordering concerns
- panel controller migration needs a stable place for shared editor-panel runtime state

Work:
- create a small workspace/runtime representation for:
  - active editor root
  - runtime UI root
  - panel layout root/mount
  - focused panel identity
  - mounted panel identities/order
- move layout-root creation/find and panel-layout bookkeeping out of the stopgap adapter
- keep panel-specific reducer state out of this module

Done when:
- shared editor panel runtime state has an explicit home outside the stopgap adapter
- layout/runtime root creation is no longer adapter-local behavior

### 3. Put `DataRendererSystem` usage behind panel-runtime helpers

Why third:
- this is the seam between generic shell/slot ownership and panel-specific models
- the adapter currently still knows too much about attach/remove/render details
- panel controller extraction gets much easier once projection is one helper call

Work:
- add helpers for rendering list/detail content into named panel slots
- define a small projection contract per panel:
  - target slot
  - renderer spec
  - model payload
- keep phase 1 policy simple: full rerender for the targeted panel subtree

Done when:
- panel-specific code asks for "render this model into this slot"
- slot-level attach/remove/rebuild logic stops being open-coded in the stopgap adapter

### 4. Extract world panel into a real controller seam

Why before inspector:
- world panel behavior is simpler than inspector workspace behavior
- it is a good first test of the generic runtime without multi-instance detail complexity
- world-panel save/load and scene-row semantics are clearly panel-specific

Work:
- move world-panel shell spec, model rebuild, content/status projection, and action handling behind a world-panel controller module seam
- keep existing world-panel semantics unchanged
- route decoded generic panel actions into world-panel-specific handlers

Done when:
- world-panel logic is no longer interleaved with inspector logic in the adapter
- the adapter delegates world-panel refresh/action work rather than implementing it inline

### 5. Extract inspector panel into the same controller seam

Why after world panel:
- inspector has more moving parts:
  - workspace reducer state
  - pinned/unpinned instance logic
  - sidebar/detail projection
  - panel-local selection
- doing world first reduces the number of unknowns

Work:
- move inspector shell spec, sidebar/detail projection setup, and decoded action handling behind an inspector controller seam
- keep `InspectorWorkspaceState` and reducer ownership in inspector-specific code
- make the generic layer unaware of inspector pinning semantics except for carrying `instance_id`

Done when:
- inspector workspace orchestration is isolated from generic panel runtime concerns
- sidebar/detail rerender paths are invoked through panel-controller boundaries

### 6. Narrow the stopgap adapter to orchestration only

Why now:
- after the previous extractions, this file should stop owning reusable behavior
- this is the point where the remaining responsibilities become obvious

Work:
- reduce the adapter to:
  - install shared handlers
  - acquire shared workspace/runtime context
  - hand decoded actions and refresh triggers to panel controllers
  - coordinate temporary compatibility paths
- remove helper functions that became dead after runtime/controller extraction

Done when:
- the adapter is clearly a temporary wrapper
- most code in it is routing and compatibility glue, not core logic

### 7. Add or continue `grid_panel` only on top of the extracted seams

Why last in this sequence:
- adding more panel behavior before the runtime/controller split is complete deepens the wrong abstraction
- `grid_panel` is the first useful proof that the new panel path is actually reusable

Work:
- ensure `grid_panel` mounting, slot projection, and action decoding use the same generic runtime path as world/inspector
- keep grid enumeration and mutation logic inside its panel module

Done when:
- `grid_panel` does not require new stopgap-only shell/plumbing patterns

## Recommended implementation order by dependency

1. Finish `panel_system` mounted-panel ownership.
2. Extract shared editor workspace/runtime state.
3. Hide `DataRendererSystem` slot projection behind generic helpers.
4. Move world panel behind a controller seam.
5. Move inspector panel behind the same seam.
6. Shrink the stopgap adapter to thin orchestration.
7. Land further panel work only through the new seams.

## Things explicitly not first

These may still be important, but they should not go first:

- fully optimizing rerender granularity
- inspector detail-field redesign
- broad editor workspace reducer redesign
- deeper grid-panel feature work
- private/multi-editor panel routing expansions

Reason:
- those all become easier after generic runtime ownership and controller boundaries are explicit

## Suggested first code cuts

If this work starts immediately, the first concrete cuts should be:

1. move remaining panel-instance resolution helpers and repeated selector bookkeeping out of the stopgap adapter and into `panel_system`
2. introduce a small editor workspace runtime struct/module and move layout/runtime-root state there
3. add one helper path that renders a panel list/detail projection into a resolved `PanelInstance`
4. rewire only the world panel through that path first

That sequence should produce the first real shrink in the stopgap adapter without needing a full editor-panel rewrite in one move.

## Related

- [editor-stopgap-adapter-decomposition.md](/home/rei/_/cat-engine/docs/task/editor-stopgap-adapter-decomposition.md:1)
- [panel-system.md](/home/rei/_/cat-engine/docs/task/panel-system.md:1)
- [data-renderer-system-for-editor-ui.md](/home/rei/_/cat-engine/docs/task/data-renderer-system-for-editor-ui.md:1)
- [grid-panel-and-grid-inspector.md](/home/rei/_/cat-engine/docs/task/grid-panel-and-grid-inspector.md:1)
