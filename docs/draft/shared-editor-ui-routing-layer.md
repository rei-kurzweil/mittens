# Shared editor UI routing layer

Date: 2026-06-05

Status: draft only.

This note assumes the direction in
[docs/bugs/shared-editor-ui-root-duplicates-editor-scoped-paint-handlers.md](../bugs/shared-editor-ui-root-duplicates-editor-scoped-paint-handlers.md)
should be **Option B**:

- keep one shared editor runtime UI
- keep one instance of each shared editor panel
- move UI-driven editor workflows onto a shared routing/reducer layer
- route scene-facing effects into one chosen editor root at a time

The intent is to avoid duplicate reducers now, while still leaving room for future multi-panel or
multi-workspace behavior.

## Decision

For the current architecture, the ownership split should be:

- shared editor UI owns panel selection/focus/tool state
- editor roots own scene selection, gizmos, and scene topology
- a shared routing layer maps shared UI state onto one active editor target

That means Paint should no longer be installed as "one UI reducer per editor root".

Instead, Paint should be split into:

1. a shared UI-facing reducer/coordinator
2. a scene-facing executor that targets the currently active editor

## Why this matches the repo better

This is already the shape the engine uses elsewhere:

- `RouterComponent` handles **attachment routing**:
  external children attach to an owner, then get rerouted into an owned internal target
- `SignalRouteUpwardComponent` handles **parent/ancestor intent routing**:
  a local intent can be promoted to the nearest matching ancestor
- `docs/draft/event-signal-pipelines.md` already frames the missing piece as **event projection**,
  not a second bubbling direction

So the shared-editor problem should use the same vocabulary:

- shared panel topology stays under one shared UI root
- shared panel events are normalized once
- those normalized events update shared editor-workspace state
- scene intents/effects are then routed to one editor root

No new generic routing primitive is required for the draft design.

## Core model

Introduce a workspace-level coordinator concept.

Working name:

- `SharedEditorWorkspace`

It can live inside `InspectorSystem`, `PaintSystem`, or a dedicated coordinator later. The draft
only cares about the model.

### Coordinator state

The shared coordinator should own state like:

```text
SharedEditorWorkspace {
  ui_root: ComponentId,
  panel_query_root: ComponentId,
  panel_focus_selection: ComponentId,
  paint_tool_selection: Option<ComponentId>,
  asset_selection: Option<ComponentId>,
  registered_editors: Vec<ComponentId>,
  active_editor: Option<ComponentId>,
  active_reason: ActiveEditorReason,
}
```

`active_editor` is the key missing concept in Option B.

### What `active_editor` means

`active_editor` is the editor root that should receive scene-facing editor actions from shared UI.

Examples:

- paint placement targets this editor
- shared inspector reads selection from this editor
- future asset insertion/drop actions target this editor

This is not global engine focus. It is only the target editor for the shared editor workspace.

## Active-editor routing rule

The coordinator should choose `active_editor` by explicit precedence.

Recommended first pass:

1. most recent scene interaction inside an editor subtree
2. otherwise the editor referenced by the most recent shared world-panel interaction
3. otherwise the editor that most recently changed selection
4. otherwise the first registered editor in the workspace

This keeps routing tied to concrete user activity instead of vague "current editor" state.

### Scene interaction that should update `active_editor`

These should promote an editor to active:

- click on an object inside that editor subtree
- drag start on an object inside that editor subtree
- selecting a row in the shared world panel for that editor

These should not:

- clicking shared paint-tool buttons alone
- clicking arbitrary panel chrome that does not imply an editor target

Paint-tool selection chooses a tool, not an editor.

## Paint split under Option B

`PaintSystem` should be conceptually split even if implementation stays in one file at first.

However, the preferred implementation direction is to give the reducer/state logic its own module
from the first pass instead of embedding it back into `paint_system.rs`.

Recommended shape:

- `paint_system.rs`
  - signal subscription/wiring
  - scene-hit to editor-root resolution
  - shared workspace registration
  - effect execution against the chosen editor target
- `paint_system_state_manager.rs`
  - `PaintState`
  - `PaintEvent`
  - `PaintEffect`
  - reducer / transition logic
  - activation and targeting decisions derived from shared UI state

That keeps the first implementation concrete and local to Paint, while making it easy to extract
later if the same reducer/store pattern proves useful elsewhere.

### Shared half

The shared half listens once to the shared panel/UI event stream and reduces:

- focused panel
- selected paint tool
- selected asset
- maybe shared status text

This is the same reducer direction already described in
[docs/analysis/paint-system-reducer-event-model.md](../analysis/paint-system-reducer-event-model.md),
except the target scope is now clearly workspace/shared-UI scoped.

The important design constraint is:

- use a dedicated Paint state-manager module first
- do not introduce a fully generic reducer/store framework before at least a few real users need
  it

### Scene half

The scene half listens to scene interaction events and, before doing anything:

1. resolves which editor subtree the event belongs to
2. updates `active_editor` if appropriate
3. checks whether shared paint state is active
4. applies the tool effect only for that resolved editor

Important:

- scene events should not fan out to every editor root
- shared UI selection should not be reduced once per editor root

## Routing-layer responsibilities

The shared routing layer should do four jobs.

### 1. Register editors into one shared workspace

When editor roots are discovered:

- register them with the shared workspace
- do not spawn another shared paint/assets/inspector/world panel pair for each one

This aligns with the "one of each panel" expectation.

Private panels can remain a later extension, but they should be opt-in and modeled explicitly.

### 2. Normalize shared UI events once

The routing layer should consume the shared panel tree exactly once and derive workspace-local
events such as:

```text
SharedEditorEvent::PanelFocusChanged { focused_panel }
SharedEditorEvent::PaintToolChanged { tool }
SharedEditorEvent::AssetSelectionChanged { asset }
SharedEditorEvent::WorldPanelEditorChosen { editor_root }
```

This is event projection in the sense of
[docs/draft/event-signal-pipelines.md](./event-signal-pipelines.md).

The important rule is:

- the shared UI tree produces one normalized event stream
- editor-specific systems do not each subscribe directly to the same raw panel tree

### 3. Resolve scene events to an editor root

For scene-originating events like `Click` or `DragStart`:

- walk ancestry from the hit renderable
- resolve the nearest `EditorComponent` ancestor
- treat that as the event's editor target

This part stays editor-local and matches the current editor-selection model.

### 4. Emit scene-facing effects to the chosen editor

After shared state says "Paint is active" and scene routing says "this hit belongs to editor E",
emit/evaluate the effect only for `E`.

That effect can still be expressed through the current signal/intent layer.

## How existing routing pieces fit

The point of this draft is to reuse the patterns already present.

### `RouterComponent`

Use it for shared-panel topology ownership, not for choosing the active editor.

It already solves:

- one shared panel host owns internal helper topology
- authored or late-attached children can be routed into named internal targets

So if the shared editor workspace materializes a panel host subtree, `RouterComponent` remains the
right tool for panel content routing inside that subtree.

It is not the right primitive for "which editor receives Paint".

### `SignalRouteUpwardComponent`

Use it when a scene-local intent should be promoted to a meaningful ancestor, not for shared UI
reduction.

Examples where it still fits:

- local gizmo or viz proxy intents that should land on a transform ancestor
- future scene-local editor intents that should promote to the editor root

It is useful after an editor target has already been chosen.

It does not solve:

- shared panel state normalization
- active-editor selection
- one-to-many duplicate UI reducer registration

### Event projection

The missing shared-editor piece is event projection:

- subscribe once to shared `SelectionChanged` / `Click` / panel events
- map them into workspace-local editor semantics
- update shared state or emit targeted follow-up work

That is why this problem is closer to `docs/draft/event-signal-pipelines.md` than to
`SignalRouteUpward`.

## State-manager direction

This design does imply a small reactive state owner.

For now, that should stay concrete rather than generic.

Recommended first-pass shape:

```rust
struct PaintState { ... }

enum PaintEvent { ... }

enum PaintEffect { ... }

fn reduce_paint_state(
    state: &mut PaintState,
    event: PaintEvent,
) -> Vec<PaintEffect>
```

or equivalent.

The key point is not the exact API surface. The key point is that Paint gets:

- one canonical shared state owner
- one transition boundary
- one effect/output boundary

### Why not make it generic yet

The current pressure is architectural ownership, not lack of a reusable framework.

So the first extraction point should be the module boundary, not a generic trait hierarchy.

Good:

- `paint_system_state_manager.rs` exists as a self-contained unit
- later systems can copy the pattern if it proves sound

Not recommended yet:

- introducing a generic `StateManager<S, E, Fx>` or reducer trait before multiple systems need it

If the pattern repeats later across Paint, Assets, Inspector, or shared panel focus, that module
boundary gives us a clean place to extract a reusable abstraction.

## Proposed runtime shape

The first implementation does not need a new global engine primitive.

A practical runtime shape would be:

1. shared workspace registry
2. one shared UI reducer for panel-derived state
3. one scene router that resolves hits to an editor root
4. one executor that applies the active tool/action to that editor only

Conceptually:

```text
shared panel UI
  -> normalize once
  -> shared workspace state
  -> gate/editor-tool activation

scene click/drag
  -> resolve nearest editor ancestor
  -> maybe promote that editor to active
  -> apply shared tool state to that editor only
```

## Relationship to shared panels

This draft deliberately assumes one shared instance of:

- world panel
- inspector panel
- assets panel
- paint panel

That matches the current need better than the earlier
[docs/draft/editor-shared-panels.md](./editor-shared-panels.md) boolean model.

If private per-editor panels become a real feature later, the model can extend to:

- one or more `SharedEditorWorkspace`s
- editor roots assigned to one workspace each
- private panels represented as a workspace of size one

But the first pass should not design for multiple panel instances by default.

## Suggested implementation order

1. Introduce the draft workspace/coordinator model in docs and align Paint/Inspector work to it.
2. Make shared panel spawning explicitly singleton/workspace-owned.
3. Move paint UI reduction to a shared scope instead of per-editor installation.
4. Add active-editor resolution from scene hits and shared world-panel interaction.
5. Route paint scene effects only to the resolved editor target.

## Non-goals

- implementing private per-editor panels right now
- adding a second event propagation direction
- making `RouterComponent` responsible for semantic editor targeting
- making `SignalRouteUpwardComponent` solve shared UI subscription ownership
- editing `src/` as part of this draft

## Open questions

- Should the shared world panel explicitly expose "editor chosen" events, or is row selection
  enough to derive that?
- Should shared inspector state follow `active_editor` only, or can explicit panel interaction pin
  a different editor temporarily?
- Do Assets and Paint both route through the same `active_editor`, or do we need per-tool target
  policies later?
