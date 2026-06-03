# Selection topology refactor: explicit nested `Selection` / `Option`

This note defines the intended selection model for the editor panel layout.
It is still a refactor note, but it now chooses a concrete target instead of presenting multiple equivalent shapes.

The current runtime is heuristic, tree-shape-specific, and mostly oriented around the asset panel.
The target runtime should instead use explicit `Selection` barriers and nested `Option` resolution.

## Target model

Use a two-level model:

- a top-level `Selection` wraps the entire editor-created panel `LayoutRoot`
- each panel shell contributes one outer `Option` on its outer transform so the whole panel can be selected
- panels may contain nested `Selection` scopes for their own internal option groups
- nested scopes are barriers: outer scopes must not steal hits from inner scopes

Core semantics:

- `Selection`
  - defines a selection scope
  - defines a selection barrier
  - owns the selected option state for that scope
  - can be nested
- `Option`
  - defines a selectable unit within the nearest enclosing `Selection`
  - may be nested arbitrarily within that scope
  - should sit above the authored subtree whose visuals and renderables represent the option

The intended resolution rule is:

1. raycast hits a renderable
2. walk upward to the nearest `Option`
3. that `Option` belongs to the nearest enclosing `Selection`
4. stop at the first `Selection` barrier

That barrier rule is the contract. If a click lands inside an inner scope, that inner scope resolves the hit. An outer panel or layout scope must not claim it.

## Current system

Today the selection system is not topology-neutral. It is built around a specific authored tree shape plus runtime guesses.

Relevant code:

- [src/engine/ecs/system/selection_system.rs](../../src/engine/ecs/system/selection_system.rs)
- [assets/components/panels.mms](../../assets/components/panels.mms)
- [assets/components/assets_content.mms](../../assets/components/assets_content.mms)
- [assets/components/asset_item.mms](../../assets/components/asset_item.mms)
- [assets/components/panel_items.mms](../../assets/components/panel_items.mms)

The current behavior is roughly:

- find a nearby `SelectionComponent`
- infer the selected item by inspecting ancestors, siblings, and descendants
- special-case scroll wrappers and layout helper topology
- infer item identity and presentation from descendant `text` and `color`

That works for the current asset-panel-oriented tree, but it keeps selection behavior coupled to:

- sibling/ancestor guessing for `Selection`
- magic `scrolling` and `__scroll_track` item inference
- text/color-descendant assumptions for option identity and highlighting
- asset-panel-specific selection behavior embedded in `selection_system`

## Target topology

The first implementation target is the current editor panel layout, not a whole-engine rewrite.
The authored tree should communicate selection intent directly.

### Top-level editor layout scope

The editor panel layout should have this shape:

```text
Selection(LayoutRoot)
└── LayoutRoot
    ├── Option(world panel shell)
    │   └── world_panel_root
    ├── Option(inspector panel shell)
    │   └── inspector_panel_root
    ├── Option(asset panel shell)
    │   └── assets_root
    └── Option(paint panel shell)
        └── paint_panel_root
```

Important constraints:

- `Selection(LayoutRoot)` is the top-level panel-selection scope
- direct or nested panel shell transforms under that layout belong to that scope
- each panel shell has an outer `Option`
- world, inspector, assets, and paint are all panel options in that same scope

This is what lets the editor select an entire panel by clicking its outer styled surface.

### Nested asset selection

The asset panel should keep its inner scope for asset items:

```text
Option(asset panel shell)
└── assets_root
    └── content_slot
        └── Selection(assets content scope)
            └── assets_content_area
                ├── Option(asset item A)
                │   └── asset_item
                │       ├── preview_slot
                │       └── label/renderables
                └── Option(asset item B)
                    └── asset_item
```

The important property is that a click on a nested asset item resolves inside the inner asset `Selection` and does not fall back to the outer panel-shell `Option`.

### Nested paint tool selection

The paint panel should move to the same explicit nested-scope model:

```text
Option(paint panel shell)
└── paint_panel_root
    └── content_slot
        ├── Selection(paint tool scope)
        │   └── tool_options_wrap
        │       ├── Option(tool: Free Draw)
        │       │   └── paint_panel_item
        │       ├── Option(tool: Line)
        │       │   └── paint_panel_item
        │       └── ...
        └── settings_or_other_controls
```

This differs from the current `paint_panel` shape, where `Selection {}` sits directly on `content_slot` and all content in that area implicitly belongs to the selection set.

## Paint panel restructuring

The paint panel needs a small authored-topology change so the selection model stays clean as the panel grows.

Current shape in [assets/components/panels.mms](../../assets/components/panels.mms):

- the outer paint panel root is a styled panel shell
- `content_slot` currently carries `Selection {}`
- tool/icon entries are attached directly under that content area

Target shape:

- the outer paint panel root remains the panel-level `Option`
- inside `content_slot`, introduce a dedicated wrapper transform that contains only the tool/icon options
- attach an inner `Selection` to that wrapper
- make each tool/icon entry an `Option`
- reserve sibling space in the content area for future controls or settings that are not part of the tool option set

Why this matters:

- tool selection stays a coherent inner scope
- later sliders, toggles, or settings can live beside the icon grid without accidentally becoming tool options
- the panel-level `Option` and the tool-level options remain separate concepts

## Layout-system compatibility

Selection resolution must work with layout-owned helper topology, not just authored transforms.

Styled nodes with `Style { background_color }` may produce layout-owned `__bg` renderables.
Those renderables may be the actual raycast hit surface.

The runtime model should therefore be:

- raycast may hit a layout-owned helper renderable
- selection resolution treats that hit surface as an implementation detail
- the system still recovers the authored `Option`
- nested options must continue to work across layout-owned wrappers, scroll wrappers, and authored inner transforms

This must not depend on asset-panel-specific names or assumptions.

In practice, the resolution path should still be described as:

`Renderable -> nearest Option -> nearest enclosing Selection`

even when the concrete hit began on:

- a layout-owned `__bg` quad
- a layout-owned scrolling wrapper
- a scrolling-owned `__scroll_track` subtree
- an authored inner transform below the option root

## Presentation and highlight model

The selection note should stop treating highlight behavior as asset-item-specific.
The target model should use a general styled-node / renderable adapter.

Recommended abstraction:

- input: resolved `Option`
- output: presentational target for selection visuals

Responsibilities of that helper/adapter:

- find an authored styled surface when present
- understand that layout may materialize a `__bg` renderable behind the scenes
- prefer updating the authored style or its background-backed surface when one exists
- fall back to adding a selection overlay or helper surface for plain renderable/object options that do not have a styled background

Non-goal:

- do not expose layout-owned `__bg` naming as the authored public selection API
- do not make `selection_system` publicly model selection as direct `__bg` mutation

This keeps selection visuals compatible with both:

- styled nodes that render through layout-owned background helpers
- plain object options such as `T { R }` with no authored style node

## What this removes from the runtime

Moving to explicit nested scopes and option nodes should remove the need for:

- sibling/ancestor guessing to discover the active `Selection`
- magic `scrolling` and `__scroll_track` inference to identify selectable items
- descendant `text` assumptions for item identity
- descendant `color` assumptions for highlight presentation
- asset-panel-specific selection behavior embedded in `selection_system`

The authored tree should declare:

- where selection scopes start
- which subtree is an option
- which nested scope owns a hit

The runtime should only resolve that declaration.

## Migration guidance

A practical implementation sequence is:

1. introduce explicit `Option` nodes or components as the authored selectable marker
2. wrap the editor-created panel `LayoutRoot` in a top-level `Selection`
3. mark each panel shell as an outer `Option`
4. keep or add inner `Selection` scopes for panel-local option groups such as assets and paint tools
5. teach selection resolution to follow `Renderable -> Option -> Selection` and stop at the first `Selection` barrier
6. add a highlight adapter that resolves styled-surface presentation separately from selection ownership
7. remove the old tree-shape-specific guesses as the authored topology becomes explicit

## Verification scenarios

The first implementation should verify these cases:

- clicking a panel outer surface selects that panel in the layout-root `Selection`
- clicking inside a nested asset option selects the asset option, not the outer panel option
- clicking a paint tool/icon selects it within the paint panel's inner `Selection`
- adding non-option settings beside the paint icon wrapper does not make those settings part of the paint tool option set
- a styled `Option` with `background_color()` remains selectable when the actual raycast hit lands on the layout-owned background quad
- a plain object option like `T { R }` without authored style still gets a visible selection treatment through the adapter fallback
- scrolling/layout-owned wrappers do not break `Renderable -> Option -> Selection` resolution

## Defaults and deferred details

This note locks in these defaults:

- use nested scopes as the intended model
- use the permissive nested `Option` model only; do not present the strict direct-child model as an equal recommendation
- treat `Option` as a structural/selectability marker in this note; metadata fields can be deferred
- prefer a style-backed highlight adapter over direct `__bg` mutation as the authored model
- treat the editor panel layout plus nested asset and paint selections as the first implementation target

Still deferred:

- whether `Option` later grows metadata such as label, semantic id, or ordering hints
- whether selection indexing is authored order, visible order, or explicitly stored order
