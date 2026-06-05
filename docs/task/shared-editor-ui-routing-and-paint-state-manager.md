# Shared editor UI routing and Paint state manager

Date: 2026-06-05

Status: planning only.

This task turns the Option B direction from
[docs/bugs/shared-editor-ui-root-duplicates-editor-scoped-paint-handlers.md](../bugs/shared-editor-ui-root-duplicates-editor-scoped-paint-handlers.md)
into an implementation plan.

It also records an explicit structural decision:

- if Paint moves to a reducer/state-manager shape, that logic should live in its own module such
  as `paint_system_state_manager.rs`
- do not start by building a generic shared reducer framework

## Goal

Keep one shared editor UI instance while preventing editor-scoped paint/UI reducers from
subscribing to that same shared UI tree multiple times.

The first implementation should:

- keep one shared world/inspector/assets/paint panel set
- reduce shared panel state exactly once
- resolve one `active_editor` target for scene-facing actions
- route paint effects only to that chosen editor root
- isolate Paint reducer/state logic in a dedicated module boundary

## Problem recap

Today the engine mixes:

- editor-scoped scene interaction
- shared runtime editor panels

That causes a shared panel event such as `SelectionChanged` to be reduced once per editor-root
Paint installation.

The bug is not just duplicate work. It is incorrect ownership:

- panel state is shared
- scene state is editor-local
- there is no explicit workspace-level unit that maps one to the other

## Decision

Use Option B.

That means:

- shared editor UI remains singleton/workspace-owned
- paint UI reduction becomes shared/workspace-owned too
- editor-root effects become explicitly targeted through `active_editor`

## Required module split

The first implementation should not keep all of this logic in `paint_system.rs`.

Recommended split:

- `paint_system.rs`
  - shared registration/wiring
  - scene-hit to editor-root resolution
  - invoking reducer/state-manager transitions
  - effect execution through current signal/intent plumbing
- `paint_system_state_manager.rs`
  - `PaintState`
  - `PaintEvent`
  - `PaintEffect`
  - reducer logic
  - activation predicates derived from shared panel state

This is a concrete module boundary, not a generic framework commitment.

## Why this should stay concrete first

The immediate problem is ownership and duplicated subscriptions, not lack of a generic
state-management API.

So the first pass should prove:

- one shared state owner
- one transition boundary
- one effect boundary

Only after that should we ask whether the pattern is repeated enough to extract a generic helper.

## Proposed data model

### Shared workspace state

The routing layer needs a workspace-scoped state holder.

Working shape:

```text
SharedEditorWorkspaceState {
  ui_root: ComponentId,
  panel_query_root: ComponentId,
  registered_editors: Vec<ComponentId>,
  active_editor: Option<ComponentId>,
  active_reason: ActiveEditorReason,
}
```

This can live inside Paint first or inside a later dedicated shared-editor coordinator.

### Paint-local state manager

Working shape:

```text
PaintState {
  focused_panel: Option<ComponentId>,
  selected_tool: Option<PaintTool>,
  selected_asset: Option<PaintSelection>,
  active_editor: Option<ComponentId>,
  stroke: PaintStrokeState,
}
```

With:

```text
PaintEvent
PaintEffect
reduce_paint_state(...)
```

The exact type names may change, but the state/effect separation should stay.

## Event sources

### Shared UI event sources

Reduce these once from the shared panel tree:

- panel focus changes
- paint-tool selection changes
- asset selection changes
- shared world-panel editor row selection

These become normalized `PaintEvent` or workspace events.

### Scene event sources

Keep scene interaction event handling editor-aware:

- `Click`
- `DragStart`
- later `DragMove` / `DragEnd` if paint strokes need them

For each scene event:

- resolve nearest `EditorComponent` ancestor from the hit renderable
- treat that editor as the event-local target
- promote it to `active_editor` when appropriate

## Active-editor contract

The first pass should choose `active_editor` by concrete precedence:

1. most recent scene interaction inside an editor subtree
2. otherwise the editor referenced by the most recent shared world-panel interaction
3. otherwise the editor that most recently changed selection
4. otherwise the first registered editor

Paint-tool selection alone should not change the active editor.

## Existing routing pieces to reuse

### `RouterComponent`

Continue using it for panel/topology attachment routing.

Examples:

- routing authored or late-attached content into a shared panel host subtree
- preserving shared panel helper topology ownership

Do not use it as the semantic "active editor chooser".

### `SignalRouteUpwardComponent`

Continue using it for scene-local intent promotion after a target editor has already been chosen.

It may still be useful when:

- a paint effect or gizmo-related intent should land on an ancestor transform/editor node

It does not replace the shared state manager.

## Implementation phases

### Phase 1. Make shared panel ownership explicit

- audit shared panel spawning so it is clearly singleton/workspace-owned
- stop treating shared panel materialization as a per-editor side effect
- document or encode where the shared panel query root lives

### Phase 2. Introduce the Paint state-manager module

- create `paint_system_state_manager.rs`
- move `PaintState` and Paint-specific reducer logic there
- define `PaintEvent`
- define `PaintEffect`

Important:

- keep this module concrete to Paint
- do not extract a generic state-manager trait yet

### Phase 3. Normalize shared UI events once

- subscribe once to the shared panel tree
- map raw `SelectionChanged` and related panel events into `PaintEvent`
- remove per-editor reduction of the same shared panel signals

### Phase 4. Add active-editor resolution

- resolve nearest editor ancestor from scene hits
- update workspace `active_editor` when scene interaction implies a target
- allow shared world-panel selection to set the target editor too

### Phase 5. Route paint effects only to the chosen editor

- gate paint activation on shared panel/tool state
- apply tool effects only for the resolved editor target
- ensure scene events no longer fan out into duplicate editor-root reducer passes

## First-pass success criteria

- one shared panel selection event causes one Paint reduction pass
- multiple editor roots can coexist without duplicate paint reducer traces
- shared paint-tool choice affects only the current active editor target
- the reducer/state logic lives outside `paint_system.rs`
- no generic framework was introduced prematurely

## Non-goals

- building a general-purpose `StateManager<S, E, Fx>` abstraction
- implementing private per-editor panel instances
- redesigning the whole signal runtime
- introducing a second event propagation direction
- editing unrelated editor systems unless required by ownership changes

## Open questions

- Should the workspace state holder live inside Paint first, or should there be a dedicated shared
  editor coordinator immediately?
- Should shared inspector targeting follow the same `active_editor` contract from day one?
- Do we want `PaintEffect` to be purely declarative data, or is a smaller "state transition only"
  first pass enough before effect extraction is complete?

## Related

- [docs/draft/shared-editor-ui-routing-layer.md](../draft/shared-editor-ui-routing-layer.md)
- [docs/analysis/paint-system-reducer-event-model.md](../analysis/paint-system-reducer-event-model.md)
- [docs/draft/event-signal-pipelines.md](../draft/event-signal-pipelines.md)
- [docs/task/paint-panel-selection-and-panel-focus.md](./paint-panel-selection-and-panel-focus.md)
