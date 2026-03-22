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
  - a **world panel** for navigating the component tree
  - an **inspector panel** for showing/editing the currently inspected component
- Panels are **dynamic** (system-constructed); no prefab cloning.
- World panel displays all components in the editor subtree, collapsed by default (root-level nodes visible, children collapsed until expanded).
- Inspector panel displays:
  - the currently inspected component header (`name: type`)
  - known fields for that component (initially: editor gizmo-space toggles)
- Panel subtrees wrap themselves in `SelectableComponent::off()` so they are never accidentally selected.

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

### World panel marker

`WorldPanelComponent` marks a subtree that `InspectorSystem` owns as the **world/component-tree UI**.

Suggested runtime state:

- `editor_root: ComponentId`
- `expanded: HashSet<ComponentId>` (which nodes are expanded; all others collapsed)
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

### Selection model

**Everything in an editor subtree is selectable by default.** No opt-in marker is needed.

Panels wrap their entire subtree in `SelectableComponent::off()`. Any descendant of a
`SelectableComponent { enabled: false }` node is excluded from editor selection — clicking it
will not move the gizmo, will not update the inspector context, and will not trigger
`SelectionChanged`.

```
WorldPanelComponent
  └── SelectableComponent::off()     ← panels self-exclude
        └── ... panel UI rows ...
```

`SelectableComponent` fields:

```rust
pub struct SelectableComponent {
    pub enabled: bool,
}
```

`EditorSystem` checks for a `SelectableComponent { enabled: false }` ancestor before processing
a `DragStart` hit. If found, the event is ignored for selection purposes.

## `InspectorSystem`

`InspectorSystem` is responsible for:

1) **Ensuring the panels exist** under each editor root
- Create (attach) the world panel subtree if missing
- Create (attach) the inspector panel subtree if missing

2) **Tracking editor-local selection state**
- Currently selected component (drives inspector panel)

3) **Rendering the world panel**
- Walk the full editor subtree; show root-level nodes, collapsed by default
- Expand/collapse on click
- Create/update row UI nodes (icon + text)
- Handle indentation and scrolling

4) **Rendering the inspector panel**
- Show header (`name: type`) for the selected component
- Show known fields for known component types
- Emit intents to mutate engine/editor state when toggles are clicked

## World panel display model

The world panel displays **all components** in the editor subtree.

- Root-level children of the editor root are shown by default.
- All other nodes are **collapsed** until the user expands them.
- Each row shows: a small node icon + `name: type`.

`type` = the component’s type name (same string used in codec / debug output).
`name` = the component’s name from `Component::name()` / the `ComponentNode` name field — already available on every component, no `NameComponent` needed.

### Icon

Small cube renderable in overlay space, corner-facing-camera orientation.
Color can vary by depth or component type.

### Indentation

Depth `d` offsets the row by `d * indent_unit` in overlay space. Children appear below their parent.

## Inspector panel display model

Driven by the currently selected `ComponentId` (set by clicking in the world panel or the scene).

Minimum content:

- Header: `name: type`
- For `EditorComponent`: gizmo-space toggles
  - `transform_gizmo_translation_space`: World / Local
  - `transform_gizmo_rotation_space`: World / Local

Editing model: clicks emit intents to the editor root; a system applies mutations.

## Signals / intents

### Selection changed

`SelectionChanged` event emitted whenever the selected component changes.

Payload:

- `editor_root: ComponentId`
- `selected: Option<ComponentId>`

Emitted by both scene picks (DragStart → EditorSystem) and world panel row clicks.

### Inspect target intent

- `IntentValue::EditorSetInspectedComponent { editor_root, inspected: Option<ComponentId> }`

### Editor settings intents

- `IntentValue::EditorSetGizmoSpaces { editor_root, translation_space, rotation_space }`

## Component names

- `WorldPanelComponent` — the world/component-tree panel
- `InspectorPanelComponent` — the details/property panel
- `SelectableComponent` — opt-out selection marker (`enabled: bool`, default `true`)

## Open questions

- Do we need multi-select?
