# Inspector panel multi-instance and v2 draft

Date: 2026-06-06

Status: draft only.

This note proposes two related phases for the editor inspector UI:

1. make `inspector_panel` spawnable as multiple live panel instances
2. evolve `inspector_panel` into a split view where the current hierarchy subset becomes
   `inspector_panel_sidebar` and a larger main area hosts property editors

The intended runtime direction is that inspector state should be managed by an event-driven
reducer/state-manager pattern, similar in shape to `editor_workspace_context` and
`editor_paint_system`, rather than growing more ad hoc state inside the current `inspector_system`.

This is intentionally a docs-only planning note. It does not require `src/` changes yet.

## Why this needs a phased spec

The current inspector concept mixes two different jobs:

- showing a small hierarchy slice around the current target
- editing properties of the currently inspected component(s)

Those jobs should be separated.

The first phase is mainly about panel lifecycle and editor layout behavior:

- more than one inspector can exist
- pinning turns an inspector into a stable reference
- selecting something else in `world_panel` should open a new inspector when the current one is pinned

The second phase is about content architecture:

- the hierarchy slice becomes a dedicated sidebar component
- the main inspector area becomes a property editor surface
- property editing needs a clearer reducer/state model than the current panel experiments

## Related docs

- [docs/spec/inspector-panel.md](../spec/inspector-panel.md)
- [docs/draft/editor-panels-reimplementation.md](./editor-panels-reimplementation.md)
- [docs/draft/paint-system-reducer.md](./paint-system-reducer.md)
- [docs/draft/shared-editor-ui-routing-layer.md](./shared-editor-ui-routing-layer.md)
- [docs/draft/grab-handle.md](./grab-handle.md)
- [docs/task/shared-editor-ui-routing-and-paint-state-manager.md](../task/shared-editor-ui-routing-and-paint-state-manager.md)

## Terms

- `world_panel`: the editor panel used to browse world/editor topology
- `inspector_panel`: one inspector window instance
- `inspector_panel_sidebar`: the left-side hierarchy slice inside `inspector_panel` v2
- pinned inspector: an inspector instance whose inspect target should not be replaced by ordinary
  world-panel selection changes
- active inspector: the inspector instance that should receive selection-driven updates when no
  pinning rule blocks replacement

## Goals

- allow many inspector panels to exist in the editor panels layout
- define the rule for when selection reuses the current inspector vs opens a new one
- split inspector content into sidebar navigation and main property editing
- define reducer-owned state for one or more inspector panels
- keep the editor-facing command surface narrow and explicit

## Non-goals

- implementing generic docking or arbitrary tab systems in this phase
- specifying every component editor in the engine
- changing `src/` in this document
- building a generic reducer/store framework before inspector proves the pattern
- expanding the property widget set in phase 1 or phase 2 beyond the minimum needed controls

## Current problem

The current inspector direction is effectively single-instance and overloaded:

- one inspector is expected to follow current selection
- pinning semantics are not defined
- the hierarchy subset and property controls are not separate UI surfaces
- there is no explicit panel-instance state model for multiple inspectors
- there is no reducer/effect split for inspector UI state similar to the Paint direction

That makes it hard to answer basic questions cleanly:

- what happens when the user pins one target and selects another?
- where does subtree-local navigation state live?
- which state is panel-local vs workspace-shared vs scene-derived?
- how should property forms preserve draft values while selection changes elsewhere?

## Phase 1: multi-instance `inspector_panel`

### Summary

`inspector_panel` should become a spawnable editor panel type.

There may be zero, one, or many live inspector instances in the editor panels layout.

Each instance inspects one target at a time and can be either:

- unpinned, meaning it is eligible to follow the current selection
- pinned, meaning its target is stable until explicitly changed through inspector-local actions

### Core interaction rule

When the user selects a different target from `world_panel`:

- if there is an unpinned active inspector, retarget that inspector
- if the active inspector is pinned, spawn a new unpinned inspector panel for the new target
- if multiple unpinned inspectors exist, reuse the active inspector rather than guessing among all
  panels

This keeps pinning meaningful without forcing every new selection to destroy existing context.

### Recommended first-pass selection policy

Use an explicit workspace rule:

1. `world_panel` selection chooses an inspect target
2. the workspace resolves the active inspector instance
3. if that inspector is unpinned, it receives the new target
4. if that inspector is pinned, the workspace spawns a new unpinned inspector instance beside or
   after it in the editor panels layout
5. the newly spawned inspector becomes the active inspector

### Why the spawn-on-pinned rule is preferable

Without spawning, pinning is weak because the next ordinary selection would need to either:

- silently unpin and replace the pinned target, or
- do nothing and make selection feel broken

Spawning preserves both user intents:

- "keep this inspector around"
- "inspect the newly selected thing too"

### Panel instance state for phase 1

Each live `inspector_panel` instance needs panel-local state:

```text
InspectorPanelState {
  panel_id: InspectorPanelId,
  editor_root: ComponentId,
  inspected: Option<ComponentId>,
  pinned: bool,
  title_mode: InspectorTitleMode,
  subtree_selection: InspectorSubtreeSelection,
  scroll_offset: InspectorScrollState,
}
```

Suggested details:

- `panel_id` is stable for the life of the panel instance
- `editor_root` ties the panel to one editor workspace target space
- `inspected` is the current primary inspected component
- `pinned` controls replacement behavior
- `subtree_selection` is the highlighted/expanded hierarchy subset inside the inspector content
- `scroll_offset` remains panel-local and should not cause full panel rerenders

### Workspace-owned state for phase 1

The workspace also needs shared inspector coordination state:

```text
InspectorWorkspaceState {
  panels: Vec<InspectorPanelState>,
  active_panel: Option<InspectorPanelId>,
  pending_spawn_target: Option<ComponentId>,
}
```

The important split is:

- workspace state decides which panel to reuse or spawn
- panel-local state decides what that instance is showing and how its internal UI behaves

### Spawn and layout contract

Phase 1 should define layout behavior narrowly:

- spawning an inspector means creating a new panel instance in the editor panels layout
- the new panel should appear adjacent to the source inspector when spawned because of pinning
- exact docking persistence can remain out of scope

The layout system only needs one reliable semantic input:

- `SpawnInspectorPanel { editor_root, inspected, placement_hint }`

`placement_hint` can stay coarse in v1, such as:

- after active inspector
- end of inspector row/strip

## Phase 2: `inspector_panel` v2 split view

### Summary

The current hierarchy subset shown by the inspector should become a dedicated component:

- `inspector_panel_sidebar`

The new `inspector_panel` becomes a two-column panel:

- left: `inspector_panel_sidebar`
- right: property editor content

The right side should be about 1.5x the width of the sidebar.

### Proposed layout shape

```text
inspector_panel
  ├── title bar
  └── body
      ├── inspector_panel_sidebar     (1.0x width)
      └── inspector_panel_content     (1.5x width)
```

This is intentionally simple:

- the sidebar owns local hierarchy navigation
- the content area owns editing widgets and editor sections

### Sidebar responsibility

`inspector_panel_sidebar` should own the hierarchy subset that the current inspector shows today.

Its responsibilities:

- show the local tree slice around the inspected target
- support subtree expansion/collapse
- support choosing a different nearby target within the same inspector panel
- expose stable row identity for reducer/event mapping

It should not own property form state.

### Main content responsibility

The main area should render editor sections for the current inspected target.

Initial sections likely include:

- header: name and component type
- transform-like fields where applicable
- component-specific fields for known component types
- editor-only controls such as pin/unpin or future debug toggles

This content area should be treated as the primary growth surface for inspector v2.

## Property editing controls

Phase 2 requires a clearer statement about widget support.

### Already-available primitives

The existing UI surface already gives a partial foundation:

- text input
- radio-button style selection via `Selection` / `Option`

Those are enough for:

- enum-like choices
- raw scalar entry
- basic booleans if represented as selection groups

### Missing or desirable controls

The spec should explicitly note likely near-term needs:

- color picker control
- logarithmic float input slider

The logarithmic float slider is especially useful for values with wide practical ranges, such as:

- scale-like values
- falloff/intensity values
- radii, exposure-like controls, or effect strengths

This doc does not define those controls fully, but it should reserve space for them in the
inspector editing model.

### Recommended control policy

For early inspector v2:

- allow each property editor to choose between direct text input and structured controls
- prefer radio/select controls for closed enumerations
- keep color and logarithmic slider work as explicit follow-up items rather than hidden ad hoc UI

### Vec input widgets belong later

`vec2` and `vec3` editor widgets should also be treated as later widget work, not phase 1 or phase
2 scope.

They should not be framed as "reuse the gizmo in miniature".

Reason:

- gizmo interaction is too heavy and is solving a different problem
- inspector field editing needs different affordances than scene manipulation
- vector editors will likely want reference grids, axis-local affordances, and compact field-local
  drag interactions

The future `GrabHandle` direction is more relevant here than the transform gizmo:

- [docs/draft/grab-handle.md](./grab-handle.md)

That is a better fit for lightweight field-level vector interaction once the underlying handle
primitive exists.

## Inspector reducer pattern

Inspector should follow the same broad direction as the Paint reducer work:

- normalize raw runtime signals into inspector-local events
- reduce state in one owner
- apply world/UI mutations in a side-effect phase

This should stay concrete to inspector first.

Do not begin by building a generic shared reducer framework.

## System boundary direction

The current `inspector_system` appears to be carrying too much historical panel/runtime baggage.

This doc's preferred direction is:

- treat the existing `inspector_system` as too bloated to keep extending in place
- move toward an `editor_inspector_system` or similarly named focused system boundary
- give inspector its own explicit state-manager/reducer module in the same spirit as
  `editor_paint_system` and its state-manager split

The exact type/module names can change, but the architectural point should stay:

- one inspector-focused owner for event normalization, reduction, and effect execution
- not more hidden state and dead behavior accumulated inside the current broad `inspector_system`

## State ownership model

Inspector needs three kinds of state.

### 1. Scene-derived state

This comes from the editor/world and should not be treated as form state:

- selected component in `world_panel`
- component existence/liveness
- current component property values from the world

This state can invalidate panel-local targets if components disappear.

### 2. Workspace/shared inspector coordination state

This decides which panel is active and whether a new inspector should spawn:

- panel registry
- active inspector
- spawn policy
- mapping between `world_panel` selection changes and inspector targeting

### 3. Panel-local UI state

This is reducer-owned UI state for each panel instance:

- pinned/unpinned
- subtree selection within the sidebar
- sidebar expansion state
- active property section
- draft form values
- validation errors
- local scroll state

This is the state that should survive while the panel instance remains alive, even if another
panel becomes active.

## Proposed events

Working shape:

```text
InspectorEvent
```

With categories like:

```text
InspectorEvent::WorldSelectionChanged {
  editor_root,
  selected,
}

InspectorEvent::InspectorPanelFocused {
  panel_id,
}

InspectorEvent::InspectorPinToggled {
  panel_id,
}

InspectorEvent::InspectorSidebarTargetChosen {
  panel_id,
  target,
}

InspectorEvent::InspectorPropertyEdited {
  panel_id,
  property_key,
  edit,
}

InspectorEvent::InspectorPropertyCommitted {
  panel_id,
  property_key,
}

InspectorEvent::InspectedComponentRemoved {
  panel_id,
}
```

The exact names can change, but the split matters:

- selection/navigation events
- panel lifecycle/focus events
- form editing draft events
- commit/apply events

## Proposed reducer split

A concrete first-pass shape:

```text
InspectorWorkspaceState
InspectorPanelState
InspectorEvent
InspectorEffect
reduce_inspector_workspace(...)
```

Recommended boundary:

- reducer mutates only logical workspace/panel state
- effect phase handles panel spawning, panel rerender requests, and world mutation intents

### Example effect types

```text
InspectorEffect::SpawnPanel { editor_root, inspected, placement_hint }
InspectorEffect::RetargetPanel { panel_id, inspected }
InspectorEffect::RequestPanelRerender { panel_id }
InspectorEffect::CommitPropertyEdit { editor_root, target, property_key, value }
InspectorEffect::DropPanel { panel_id }
```

This matches the Paint direction:

- state transitions are explicit
- side effects are centralized
- panel content does not query arbitrary global state whenever it wants answers

## Reducer behavior for the two requested state classes

The user-facing concern here is how reducer ownership handles both:

- subtree selected state
- form/component-property editing UI state

### Subtree selected state

This should be panel-local.

Reason:

- different inspector panels may intentionally be looking at different parts of the hierarchy
- a pinned inspector should keep its own sidebar focus/expansion state
- selecting inside one inspector sidebar should not overwrite another inspector's local navigation

Suggested state:

```text
InspectorSubtreeSelection {
  focused_row: Option<ComponentId>,
  expanded: Vec<ComponentId>,
}
```

### Form / property editing UI state

This should also be panel-local, with clear distinction between:

- draft UI state
- committed world state

Suggested shape:

```text
InspectorFormState {
  sections: Vec<InspectorSectionState>,
}

InspectorSectionState {
  section_key: String,
  fields: Vec<InspectorFieldState>,
}

InspectorFieldState {
  property_key: String,
  draft_value: InspectorDraftValue,
  dirty: bool,
  validation_error: Option<String>,
}
```

Important rule:

- editing a field should first update reducer-owned draft state
- committing applies an explicit world/editor intent
- the reducer then reconciles the draft with scene-derived truth after the mutation path completes

That avoids coupling text-entry latency directly to scene mutation plumbing.

## Shared vs panel-local inspector behavior

The reducer should treat these behaviors differently.

### Shared/workspace behaviors

- respond to `world_panel` selection changes
- decide whether pinning requires spawning a new inspector
- decide which inspector becomes active
- remove dead panels or retarget panels whose targets disappear

### Panel-local behaviors

- expand/collapse sidebar nodes
- choose nearby target inside sidebar
- manage draft property field values
- manage widget-specific interaction state

This matters because a shared reducer alone would flatten too much panel-local detail, while a
purely panel-local design would not be able to decide spawn/reuse policy.

## Interaction examples

### Example 1: unpinned reuse

1. user selects `A` in `world_panel`
2. active inspector is unpinned
3. reducer retargets that inspector to `A`
4. sidebar and property content rerender for `A`

### Example 2: pinned spawn

1. inspector for `A` is pinned
2. user selects `B` in `world_panel`
3. reducer sees active inspector is pinned
4. reducer emits `SpawnPanel { inspected: B }`
5. new unpinned inspector opens
6. pinned inspector for `A` remains unchanged

### Example 3: sidebar-local navigation

1. user opens children of `B` inside one inspector sidebar
2. reducer updates only that panel's `expanded` and `focused_row`
3. other inspector panels keep their own sidebar state

### Example 4: property draft and commit

1. user edits a float field in inspector panel `P`
2. reducer updates `draft_value` and `dirty`
3. user commits the field
4. reducer emits `CommitPropertyEdit`
5. world/editor mutation path applies the change
6. reducer reconciles draft against resulting scene state

## MMS-first rendering implications

This spec fits the current MMS-first panel direction.

Recommended render split for v2:

- `inspector_panel(...)` owns shell, title bar, pin affordance, and two-column layout
- `inspector_panel_sidebar(...)` owns left-column hierarchy content
- `inspector_panel_content(...)` owns right-column property sections

The same shell/content split discussed in
[docs/draft/editor-panels-reimplementation.md](./editor-panels-reimplementation.md)
should apply here too:

- stable shell
- rerenderable content subtrees
- deterministic row/field identity for post-render binding or later MMS-authored intents

## Implementation phases

### Phase 1

- define `InspectorWorkspaceState` and `InspectorPanelState`
- define spawn/reuse policy for pinned vs unpinned inspectors
- add panel-instance identity and active-panel rules to the draft editor panel model
- keep inspector content close to current behavior

### Phase 2

- split current hierarchy subset into `inspector_panel_sidebar`
- add 1.0x / 1.5x sidebar-to-content layout
- introduce reducer-owned form state and commit flow
- add first property editor sections

### Phase 3

- add richer controls such as color picker and logarithmic float slider
- add dedicated `vec2` and `vec3` inspector input widgets
- evaluate `GrabHandle`-based vector editing affordances for lightweight per-field drag interaction
  rather than reusing the scene gizmo
- standardize property editor widget contracts across known component editors

## Open questions

- Should `world_panel` selection always target the active inspector, or should the world panel
  itself remember a preferred inspector target?
- When a pinned inspector is focused, should sidebar-local navigation within that inspector retarget
  the same panel even if it is pinned? Current preference: yes, because pinning should block
  external replacement, not local intentional navigation.
- Should property edits apply live on every change for some widgets, or should commit remain the
  default? Current preference: commit-by-default, with opt-in live preview for a few safe controls.
- Should panel-local draft state survive target switches for unpinned inspectors, or be cleared on
  retarget? Current preference: clear on retarget unless the editor later introduces explicit
  history/undo for inspector drafts.

## Recommendation

Treat inspector as a concrete second user of the reducer/state-manager pattern after Paint, but
keep the implementation local and explicit.

The important architectural decision is not "use reducers everywhere". It is:

- shared workspace logic decides inspector spawn/reuse
- each inspector panel instance owns its own subtree and form UI state
- property edits flow through explicit commit effects rather than implicit panel queries
