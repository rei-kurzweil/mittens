# Paint panel selection, panel focus, and paint-system activation

Date: 2026-06-02

Status: planning only. Do not start implementation from this doc until the panel-selection
contract below is accepted.

## Goal

Make the Paint panel behave like a focused tool palette:

- the Paint panel should fit the height of its visible tool items instead of using the
  same tall scroll body as the Assets panel
- Paint panel tools should be wrapped by a `Selection` scope so clicking a tool with a
  `Pointer` moves a yellow selected-tool highlight
- all editor panels should also be wrapped by a separate `Selection` scope so exactly one
  panel is focused at a time
- the paint system should only consume editor click/pointer events when the focused panel is
  the Paint panel
- when active, the paint system should use the currently selected Paint panel tool for clicks
  on objects inside an `Editor {}` subtree

This overlaps with `docs/task/assets-slection-and-paint-panels.md`, but is narrower: this task is
about selection scopes, focus routing, and first paint-system activation gating.

## Current implementation facts

### Paint panel

The Paint panel is authored in MMS:

- `assets/components/paint_panel.mms`
- `assets/components/paint_panel_item.mms`

Current sizing:

- `PAINT_PANEL_CONTENT_HEIGHT_GU = 57.0`
- total height is `3.0 + 0.5 + 57.0`
- content uses `overflow("scroll")`

Current item shape:

- each tool item is a `T` named `paint_panel_item`
- each item has `Raycastable.enabled()`
- item dimensions are `width(7.0)` and `height(7.5)`
- five items are authored: Free Draw, Line, Spray Can, Fill, Erase

So the panel currently looks like a full-height peer of the Assets panel even though it only has
one short row of tools.

### Selection components

`SelectionComponent` exists in `src/engine/ecs/component/selection.rs`.

Current state:

- stores `selected_index`
- stores `selected_item`
- stores `selected_component`

`SelectionSystem` exists in `src/engine/ecs/system/selection_system.rs`.

Current behavior:

- listens globally for `EventSignal::Click`
- finds the nearest `SelectionComponent` ancestor from the clicked renderable
- treats immediate children of the selection's visual/content root as selectable items
- updates the `SelectionComponent`
- changes selected visuals through `set_asset_item_selected_color`

Important limitation:

- the selection-state model is already generic enough for panel tools and panel focus
- the visual update path is not generic yet; it assumes an asset-item-style color descendant
  and uses blue selected color

This means the requested behavior should not create a second selection model. It should
generalize the existing visual/highlight path.

### Click and pointer path

`EventSignal::Click` already exists and is emitted by `GestureSystem` after a small-displacement
drag ends.

That is the correct event for tool and panel selection. Do not use `DragStart` for this feature,
because that would fight with scroll/drag behavior and would select before click classification.

### Editor panels

The current editor panel layout is assembled in:

- `src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs`

The adapter currently spawns these panel shells:

- `editor_world_panel_shell`
- `editor_inspector_panel_shell`
- `assets_panel_shell`
- `editor_paint_panel_shell`

Those shells are immediate children of one shared `LayoutRoot` under an `Overlay`.

That shared `LayoutRoot` is the natural place to insert a panel-focus `Selection` wrapper whose
immediate children are the panel shells.

### Scene/editor selection

`EditorSystem` already ignores panel UI through `Selectable.off()` subtrees and selects scene
objects from `DragStart`.

The new paint-system path must not silently replace this behavior. Paint behavior should be an
additional gated consumer:

- if Paint panel is not focused, editor click/drag selection keeps normal behavior
- if Paint panel is focused, paint system may consume appropriate editor object clicks
- transform gizmo behavior should remain ignored for paint hits unless explicitly supported later

## Proposed topology

### Paint panel tool selection

Wrap the Paint panel item list in `Selection`.

Intended MMS shape:

```mms
T {
    name = "content_slot"
    Style { ... }

    Selection {
        name = "paint_tool_selection"

        T {
            name = "paint_tools"
            paint_panel_item("Free Draw", ...)
            paint_panel_item("Line", ...)
            paint_panel_item("Spray Can", ...)
            paint_panel_item("Fill", ...)
            paint_panel_item("Erase", ...)
        }
    }
}
```

The exact extra visual child is negotiable because `SelectionSystem::selection_visual_child`
currently treats a single child under `Selection` as the content root. The important rule is:
the selectable things must be immediate children of the effective content root.

### Panel-focus selection

Wrap all editor panel shells in a separate `Selection`.

Intended Rust/materialized topology inside `spawn_panel_layout`:

```text
LayoutRoot
  Selection #editor_panel_focus_selection
    T #editor_panel_focus_items
      editor_world_panel_shell
      editor_inspector_panel_shell
      assets_panel_shell
      editor_paint_panel_shell
```

The immediate children of the effective content root are the selectable panel shells. Clicks can
hit any descendant inside a panel and bubble upward through selection resolution to select the
panel shell.

This is the same contract requested for tool items: Selection selects immediate children, while
events may originate from descendants.

## Highlight behavior

The visible selection indicator should be a yellow square/background behind the selected item.

Recommended first pass:

1. Add an authored highlight/background child to selectable items, or reuse the layout-generated
   `#__bg` when present.
2. Generalize `SelectionSystem` visual updates so it can set selected and unselected colors on
   the selected item root instead of assuming an asset-item `Color` descendant.
3. Use yellow for the selected Paint tool and panel focus.

Recommended colors:

- selected: `[1.00, 0.88, 0.20, 0.96]`
- unselected paint tool background: the factory-provided `item_background_color`

Open detail:

- `SelectionComponent` currently stores only selected state, not style configuration. We can
  either add selection visual config fields later, or use a convention such as a child named
  `selection_highlight` / generated `#__bg`.

For this feature, a convention is enough. Avoid building a broad theming API until multiple
selection styles need it.

## Paint panel sizing

Change the Paint panel constants so height fits the current item row.

Recommended first pass:

- keep title bar at `3.0`
- keep title/content gap at `0.5`
- set content height to about `8.5` to cover item height `7.5` plus vertical margins/padding
- remove `overflow("scroll")` unless the panel later grows enough tools to require scrolling

Expected total height:

```text
3.0 title + 0.5 gap + 8.5 content = 12.0 GU
```

The stopgap panel adapter also has width/height constants for panel shells. Any MMS height change
must be mirrored there until shell sizing is derived from the MMS panel root.

## Paint system activation contract

Add a `PaintSystem` only after panel focus selection and tool selection are observable.

Activation predicate:

```text
active =
  editor_panel_focus_selection.selected_component == editor_paint_panel_shell
  && paint_tool_selection.selected_item is Some
```

When inactive:

- ignore editor object clicks
- do not block normal editor selection behavior
- do not mutate scene topology

When active:

- listen for `Click` events that hit renderables inside an `Editor {}` subtree
- ignore clicks inside editor panel UI
- read the current selected paint tool
- dispatch the tool-specific action

Initial tool behavior can be deliberately minimal:

- Free Draw: no-op placeholder or emit status/log
- Line: no-op placeholder
- Spray Can: no-op placeholder
- Fill: no-op placeholder
- Erase: no-op placeholder

The first implementation should prove the routing and gating before adding object placement,
surface-normal alignment, or brush strokes.

## Implementation checklist

- [ ] Shrink `assets/components/paint_panel.mms` content height to fit one row of paint items.
- [ ] Remove or defer Paint panel scrolling unless needed after more tools exist.
- [ ] Wrap Paint panel tool items in a `Selection` scope.
- [ ] Make paint tool items expose a stable label/id for selected-tool lookup.
- [ ] Generalize `SelectionSystem` visual updates beyond asset items.
- [ ] Add yellow selected highlight behavior for Paint panel tools.
- [ ] Add yellow selected highlight behavior for focused editor panel shell.
- [ ] Wrap editor panel shells in a parent `Selection` scope in the stopgap panel adapter.
- [ ] Ensure clicks from descendants select the immediate child panel shell/tool item.
- [ ] Decide default focused panel after spawn; recommended default is World panel.
- [ ] Decide default selected paint tool after spawn; recommended default is Free Draw.
- [ ] Add a `PaintSystem` gated by focused Paint panel + selected paint tool.
- [ ] Ensure paint-system clicks only target objects inside an `Editor {}` subtree.
- [ ] Ensure panel UI clicks do not trigger paint actions.
- [ ] Add tests for selection resolution through descendants.
- [ ] Add tests for nested Selection scopes: tool selection must not change panel focus except
  through the panel-focus wrapper.
- [ ] Add tests for paint-system inactive/active gating.

## Risks and constraints

- Nested `Selection` scopes must select the nearest scope first. A click on a Paint tool should
  update the Paint tool selection and also likely focus the Paint panel through event bubbling.
  If the current global handler only updates the nearest selection, panel focus may need an
  explicit second pass or a "continue to outer selection" policy.
- The existing `SelectionSystem` does not emit a domain-specific `SelectionChanged` event for UI
  selections. PaintSystem can poll `SelectionComponent` initially, but a UI selection event would
  be cleaner later.
- Panel shell sizing is duplicated between MMS constants and Rust stopgap constants. Keep the
  first change narrow and verify the shell height after layout.
- `EditorSystem` currently selects scene objects on `DragStart`. If PaintSystem acts on `Click`,
  a paint click may also briefly select the object first. That is acceptable for a first routing
  pass, but later paint mode may need an editor-selection suppression policy while active.
- Assets panel selection currently uses the same `SelectionSystem`; generalizing visuals must not
  regress existing asset selection tests.

## Verification

- Paint panel appears short, roughly matching one row of tools.
- Clicking each paint tool moves a yellow highlight to that tool.
- Clicking inside World, Inspector, Assets, and Paint panels moves the panel-focus highlight to
  that panel shell.
- Clicking descendants inside a panel focuses the containing panel, not an arbitrary leaf.
- Clicking Paint panel tools both focuses Paint panel and changes selected paint tool.
- With any non-Paint panel focused, clicking scene/editor objects does not invoke PaintSystem.
- With Paint panel focused and a tool selected, clicking scene/editor objects reaches PaintSystem
  with the correct selected tool.

## Related

- `docs/task/assets-slection-and-paint-panels.md`
- `assets/components/paint_panel.mms`
- `assets/components/paint_panel_item.mms`
- `src/engine/ecs/component/selection.rs`
- `src/engine/ecs/system/selection_system.rs`
- `src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs`
- `src/engine/ecs/system/editor_system.rs`
- `docs/spec/click-and-panel-scroll.md`
- `docs/spec/pointer-input-ray-gesture.md`
