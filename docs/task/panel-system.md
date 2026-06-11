# Panel system

Date: 2026-06-11

Status: proposed

## Goal

Introduce a dedicated `panel_system` that owns generic editor-panel runtime infrastructure.

This is the first concrete refactor step toward replacing:
- [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)

This task addresses the larger decomposition described in:
- [`docs/task/editor-stopgap-adapter-decomposition.md`](/home/rei/_/cat-engine/docs/task/editor-stopgap-adapter-decomposition.md:1)

## Why this exists

The current stopgap adapter is already functioning like an implicit panel system, but it does not have explicit boundaries.

That file currently mixes:
- runtime UI root/layout setup
- panel shell spawning
- slot lookup and content reconcile
- click/action decoding
- focus bookkeeping
- panel-specific behavior for world and inspector

Before adding more editor panels like `grid_panel`, we should extract the generic panel runtime into a dedicated system.

## Scope

This task is about the generic runtime layer only.

It should not absorb panel-specific behavior like:
- world scene traversal
- inspector workspace reduction
- grid enumeration rules

Those remain in dedicated panel modules.

## Responsibilities of `panel_system`

The new system should own:

1. Panel shell mounting
   - materialize MMS panel shells
   - attach them under the shared editor runtime UI layout
   - keep stable references to mounted panel roots

2. Mounted panel registry
   - record which panel instances exist
   - track root ids, slot ids, owning editor root, and panel kind

3. Slot lifecycle integration
   - coordinate with `DataRendererSystem`
   - render dynamic list/detail content into named slots
   - centralize slot lookup rather than repeating selector walks

4. Generic panel action decoding
   - decode panel row/button/icon clicks from shared payload metadata
   - route semantic actions to the appropriate panel controller

5. Panel focus bookkeeping
   - identify which panel was clicked
   - update focused panel state in a generic way

6. Shared editor panel layout/runtime root
   - own creation/find of `editor_runtime_ui_root`
   - own creation/find of the panel layout root/mount

## Non-goals

`panel_system` should not own:

- world panel save/load semantics
- inspector panel pin/reducer semantics
- grid panel add/delete/toggle/select semantics
- panel-specific item/detail model construction
- scene selection semantics outside generic panel event dispatch

Those belong in per-panel controller modules.

## Proposed core types

This task should introduce explicit generic representations for the panel runtime.

### `PanelKind`

Closed enum for known mounted panel kinds.

Initial variants:
- `World`
- `Inspector`
- `Paint`
- `Assets`
- `Grid`

### `PanelSlotKind`

Closed enum for slot identity inside panel shells.

Initial variants:
- `List`
- `Detail`
- `Status`
- `Sidebar`
- `Toolbar`
- `Footer`

Exact variants can be tightened once the first extraction is underway.

### `PanelShellSpec`

Describes how to mount a low-count MMS panel shell.

Expected fields:
- `panel_kind`
- `asset_path`
- `export_name`
- `args`
- named selectors for root and slots

This becomes the generic mount contract between panel modules and the runtime.

### `PanelInstance`

Represents one mounted panel in the runtime.

Expected fields:
- `panel_kind`
- `editor_root`
- `root`
- `slots`
- optional `instance_id`

`slots` should map `PanelSlotKind` to resolved `ComponentId`.

### `PanelActionKind`

Closed enum for generic UI actions.

Initial variants should cover:
- `Select`
- `Toggle`
- `Delete`
- `Add`
- `Focus`
- `Pin`
- `ActivateField`
- `EditField`

Not every panel uses every action.

### `PanelActionPayload`

Shared payload metadata attached to clickable panel subtrees.

Expected fields:
- `panel_kind`
- `action_kind`
- `item_key`
- `target_component`
- `instance_id`
- `field_key`

This should replace the current growing pile of ad hoc row/button payload shapes.

## Proposed module layout

Possible initial files:

- `src/engine/ecs/system/panel_system.rs`
- `src/engine/ecs/system/editor/workspace.rs`

Optional supporting files if the implementation wants to split early:

- `src/engine/ecs/system/editor/panel_runtime.rs`
- `src/engine/ecs/system/editor/panel_actions.rs`

The exact file split is less important than the ownership split.

## Interaction with `RendererSpec`

This task should explicitly preserve the current intended rendering policy:

- MMS for static or low-count shell structure
- Rust `RendererSpec::Rust` for repeated dynamic rows/items/field lists

`panel_system` is not a replacement for `RendererSpec`.
It is the generic runtime around it.

## Expected panel-controller seam

`panel_system` needs a narrow way to call panel-specific logic.

Whether this is a Rust trait, a table of closures, or plain module functions is secondary.

The runtime needs a seam for:
- shell spec
- list/detail render specs
- model refresh
- semantic action handling

At minimum, each panel module should be able to provide:

- how to mount its shell
- how to build its dynamic list/detail model
- how to handle decoded panel actions

## Suggested implementation order

1. Introduce `PanelKind`, `PanelSlotKind`, `PanelShellSpec`, and `PanelInstance`.
2. Extract runtime UI root + shared panel layout creation out of the stopgap adapter.
3. Extract generic shell spawn/find and slot resolution.
4. Move `DataRendererSystem` usage behind panel runtime helpers.
5. Introduce shared panel action payload decoding.
6. Rewire world and inspector panels through the new system without changing their semantics yet.
7. Add `grid_panel` against the new runtime rather than against the old adapter directly.

## Acceptance criteria

- A dedicated `panel_system` module exists.
- Shared runtime UI root / panel layout creation no longer lives only inside the stopgap adapter.
- Mounted panel identity and slot lookup are represented by explicit types rather than repeated selector walks.
- Generic panel action payload decoding exists in one place.
- World and inspector panels can begin migrating onto the new runtime without semantic regression.
- The path for adding `grid_panel` is clearer and does not require deepening the stopgap adapter.

## Related

- [`docs/task/editor-stopgap-adapter-decomposition.md`](/home/rei/_/cat-engine/docs/task/editor-stopgap-adapter-decomposition.md:1)
- [`docs/task/grid-panel-and-grid-inspector.md`](/home/rei/_/cat-engine/docs/task/grid-panel-and-grid-inspector.md:1)
- [`docs/spec/data-renderer-system.md`](/home/rei/_/cat-engine/docs/spec/data-renderer-system.md:1)
