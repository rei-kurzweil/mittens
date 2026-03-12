# Inspector panels (editor UI concept)

This doc proposes an editor UI made of **two dynamic panels**:

1) a **tree panel** for navigating the component tree, and
2) an **inspector panel** for showing/editing the currently inspected component.

These panels are generated and updated by a single `InspectorSystem`.

This is a design/analysis document only.

## Current state

### How gizmo “mode” is set today

The transform gizmo reads its coord-space settings from the nearest `EditorComponent` ancestor.

Right now there is no runtime UI that changes these values. They are determined by:

- `EditorComponent` defaults (currently: translate = World, rotate = Local), and/or
- whichever code constructs the editor root (`EditorComponent::new()` plus builder setters).

The fields live in:

- [src/engine/ecs/component/editor.rs](src/engine/ecs/component/editor.rs)

And they are used in:

- [src/engine/ecs/system/gizmo_system.rs](src/engine/ecs/system/gizmo_system.rs)

1) at gizmo spawn time (handle visuals parented into local/world groups)
2) during drag application (rotation-space decisions)

### Editor selection today

Selection is currently “implicit”:

- `EditorSystem` listens to `DragStart` under an `EditorComponent` subtree.
- It reparents the editor’s `TransformGizmoComponent` under the clicked target’s nearest `TransformComponent`.

See: [docs/spec/editor+general-gizmos.md](docs/spec/editor+general-gizmos.md)

There is no dedicated `SelectionChanged` event signal yet.

## Goals

- Provide two panels under each editor:
  - a component-tree navigation panel
  - a component inspector panel
- Panels are **dynamic** (system-constructed); no prefab cloning.
- Tree panel displays an outline of the currently relevant component subtree.
- Inspector panel displays:
  - the currently inspected component header (`name: type`)
  - known fields for that component (initially: editor gizmo-space toggles)
- The panel subtrees should not steal selection from the scene.

## Key UX clarification: don’t “select the gizmo”

Selecting something triggers the gizmo. That makes “selecting the gizmo itself” a bad primary interaction pattern:

- The gizmo exists *because* something is already selected.
- Clicking the gizmo should be interpreted as “manipulate the current selection”, not “change selection to gizmo”.
- If the gizmo becomes the selected object, the inspector becomes confusing (you lose visibility into the real target).

We still might want to inspect gizmo components for debugging, but it should be gated behind a debug mode rather than the default.

## Non-goals

- A full UI widget framework.
- Fully generic property editing for all component types.
- Inspector docking/layout persistence.

## Proposed components

The system is built around two panel marker components plus one system.

### Tree panel marker

Primary name option:

- `ComponentTreePanelComponent`

Alternative (more general):

- `TreePanelComponent`

This component marks a subtree that `InspectorSystem` owns as the **tree UI**.

Suggested runtime state (stored on the component or adjacent state component):

- `editor_root: ComponentId`
- `tree_root: Option<ComponentId>` (what the tree is currently showing)
- `selected: Option<ComponentId>` (what the inspector is currently inspecting)
- `scroll_offset_rows: i32`

### Inspector panel marker

Primary name option:

- `ComponentInspectorPanelComponent`

Alternative (more general):

- `InspectorPanelComponent`

This component marks a subtree that `InspectorSystem` owns as the **details UI**.

Suggested runtime state:

- `editor_root: ComponentId`
- `inspected: Option<ComponentId>`
- `scroll_offset_rows: i32`

### Optional opt-out marker

We likely still need a marker so inspector UI subtrees are not treated as inspectable targets.

Name options:

- `InspectableComponent { enabled: bool }` (opt-out)
- `NonInspectableComponent` (simpler semantic)

Exact selection semantics can be decided alongside the picking/selection model.

## `InspectorSystem`

`InspectorSystem` is responsible for:

1) **Ensuring the panels exist** under each editor root
- Create (attach) the tree panel subtree if missing
- Create (attach) the inspector panel subtree if missing

2) **Tracking editor-local inspection state**
- Current selection root (what subtree we’re interested in)
- Current inspected component (what the inspector panel is showing)

3) **Rendering the tree panel**
- Convert a component subtree into a list of visible “rows”
- Create/update the row UI nodes (icon + text)
- Handle indentation, truncation, and scrolling

4) **Rendering the inspector panel**
- Show header (`name: type`)
- Show known fields for known component types
- Emit intents to mutate engine/editor state when toggles are clicked

## Tree panel display model

The tree panel displays a **component tree** (the actual topology).

Each row shows:

- a small **node icon**: a cube rotated so one corner faces forward
- a text label: `name: type`

Where:

- `type` = component type name (what we already use in codec / debug output)
- `name` = best-effort display name
  - preferred: a `NameComponent` or equivalent if/when we add one
  - fallback: something stable like the `ComponentId`

### Icon details

The icon is a cube (or box) renderable in overlay space.

- Rotation should be fixed so a corner faces the camera (to visually read as a “node”).
- Color can be derived from depth, selection state, or component type.

### Indentation and nesting

Indentation represents parent/child relationships in the component tree.

- Depth $d$ offsets the row’s content by `$d * indent_px` (or equivalent world units in overlay space).
- Children appear directly below the parent.

### “Child components” vs “child nodes”

In this engine, the topology already *is* a component tree, so “child components” are the children in the tree.

The tree panel therefore naturally displays “child components” by recursively walking children.

## Inspector panel display model

The inspector panel is driven by the currently inspected `ComponentId` (selected from the tree panel).

Minimum content:

- Header: `name: type`
- For `EditorComponent`: show tool settings toggles
  - `transform_gizmo_translation_space`: World / Local
  - `transform_gizmo_rotation_space`: World / Local

Editing model:

- Clicking toggles emits an intent targeted at the editor root.
- A system applies the mutation to `EditorComponent`.
- Optionally emit an “editor settings changed” event so dependent systems update visuals.

## Signals / intents

We can start without new events by having `InspectorSystem` infer selection from current editor state, but a real event is cleaner.

### Selection changed (recommended)

Introduce a `SelectionChanged` event emitted when the selection target changes.

Payload (sketch):

- `editor_root: ComponentId`
- `selected: Option<ComponentId>`

### Inspect target changed (tree → inspector)

The tree panel should update the inspector target on click.

This can be an intent (preferred) so UI doesn’t mutate state directly:

- `IntentValue::EditorSetInspectedComponent { editor_root, inspected: Option<ComponentId> }`

Or a dedicated event consumed by `InspectorSystem`.

### Editor settings intents

For gizmo coord-space toggles:

- `IntentValue::EditorSetGizmoSpaces { editor_root, translation_space, rotation_space }`

## Naming exploration

We can keep names specific until we have more panel types:

- `ComponentTreePanelComponent`
- `ComponentInspectorPanelComponent`

Or we can generalize earlier:

- `TreePanelComponent`
- `InspectorPanelComponent`

Heuristic:

- If the engine will likely have multiple tree panels (scene tree, asset browser, etc.), prefer generic names now.
- If this is the only tree UI for a while, prefer specific names to avoid premature abstraction.

## Open questions

- Do we want the tree panel to show the editor root + selection root as two top-level sections, or only the selection root?
- How do we represent “name” today (do we add a `NameComponent`)?
- How should clicks in the panel subtree be excluded from scene picking?
- Do we need multi-select in the tree panel?
