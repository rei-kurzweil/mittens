# Grid panel and grid inspector

Date: 2026-06-11

Status: planning / inventory

## Known bugs

- `Add Grid` works without freezing only when it rerenders the grid panel in isolation. Refreshing the world panel from that path still freezes the editor and is deferred.
- Deleting a grid from `grid_panel` currently freezes the editor and remains unfixed.
- Editor panel setup currently leaks phantom panel/content roots into the authored world root (for example duplicate `world_panel_content_root` nodes). This appears related to panel/assets loading and should be treated as a separate cleanup bug.

## Goal

Add a dedicated grid management flow to the editor in two phases:

1. Phase 1:
   - add a new `grid_panel` MMS panel under `assets/components/panels.mms`
   - add a new editor-side `system/editor/` panel integration for it
   - list all grids in the active editor
   - allow add, select, hide/show, enable/disable, and delete
   - ensure grids live under transforms and can be selected with the normal editor gizmo flow
   - keep grid ownership/editor mutations inside the `grid_panel` flow rather than requiring a fully general grid lifecycle API yet
   - add a dedicated grid render path:
     - a grid shader under `assets/shaders`
     - a `GridMaterial` / `MaterialHandle` entry for it
     - plane-backed grid rendering sized from grid dimensions rather than an infinite or point-style primitive
     - slightly thick grid lines using multiple `fwidth` worth of smoothing rather than a razor-thin line
2. Phase 2:
   - clicking a grid in `grid_panel` also drives the active unpinned inspector
   - inspector details switch from a fixed hardcoded shape to a generic field-set model
   - when a grid is selected, inspector details expose:
     - `grid granularity`
     - `grid size x`
     - `grid size z`
   - valid numeric edits update the inspected grid

This note is intentionally grounded in what the repo already has.

## What already exists

### Grid runtime

- [`src/engine/ecs/component/grid.rs`](/home/rei/_/cat-engine/src/engine/ecs/component/grid.rs:1)
  already defines `GridComponent`.
- Current `GridComponent` fields are:
  - `spacing: f32`
  - `enabled: bool`
  - `selectable: bool`
- `GridComponent::to_mms_ast()` already serializes as `Grid.spacing(...).enabled(...).selectable(...)`.

- [`src/engine/ecs/system/grid_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/grid_system.rs:1)
  already exists, but today it is not a registry/listing system.
- Current behavior is editor-paint oriented:
  - resolves a live grid for snapping/render participation
  - still carries older assumptions about selected grids acting as the snap source
  - computes world matrix / inverse matrix
  - snaps hit points onto the selected grid plane

Implication:
- we do not need to invent grid snapping from scratch
- we do need to expand the grid model from “selected/live grid used for paint snapping” into:
  - stored grid records known to the editor
  - separately instantiated live grids that participate in rendering, BVH, and snapping

### Grid rendering direction

- There is already a rendering design note in
  [`docs/spec/grid-material.md`](/home/rei/_/cat-engine/docs/spec/grid-material.md:1).
- That spec already points toward dedicated grid shaders and a dedicated material handle rather than reusing the ordinary toon mesh path.

Implication:
- phase 1 should include the first real grid render path, not just panel/editor plumbing
- a grid should render on a plane mesh sized from its dimensions
- the shader should own the line pattern
- line thickness should be slightly heavier than a single razor-thin analytical line, using several `fwidth` worth of smoothing / width so the grid remains readable in-editor
- this should land as a new grid-specific material handle rather than overloading `TOON_MESH`

### Grid-driven paint integration

- [`src/engine/ecs/system/editor_paint_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_paint_system.rs:996)
  already resolves `active_grid: Option<ActiveGrid>` through `GridSystem::active_grid_for_editor(...)`.
- Paint status already reports grid state when a selected grid is active.

Implication:
- `grid_panel` selection should feed normal editor selection rather than inventing a second active-grid source
- paint snapping should only occur when the current interaction is actually hitting a grid surface
- selecting a grid should not by itself force snapping on non-grid scene geometry

### Editor selection + gizmo path

- [`src/engine/ecs/system/editor_system.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_system.rs:1)
  already owns the important behavior:
  - scene click resolves nearest `TransformComponent`
  - editor selection is stored in `EditorComponent.selected`
  - transform gizmo is attached to the selected transform
- This path expects the selectable scene target to be a transform, not an arbitrary leaf component.

Implication:
- grids should be authored/spawned as a transform subtree, with the grid component under that transform
- the `grid_panel` should select the owning transform so gizmos attach cleanly
- if paint snapping still needs the `GridComponent` id specifically, we need a clear transform<->grid resolution helper

### Panel architecture

- Full MMS panel factories live in [`assets/components/panels.mms`](/home/rei/_/cat-engine/assets/components/panels.mms:1).
- Item-level MMS factories live in [`assets/components/panel_items.mms`](/home/rei/_/cat-engine/assets/components/panel_items.mms:1).
- [`RendererSpec<T>` and `DataRendererSystem`](/home/rei/_/cat-engine/src/engine/ecs/system/data_renderer_system.rs:1)
  already provide the intended split between:
  - low-count MMS-rendered UI structure
  - repeated Rust-rendered dynamic rows/details
- The live editor panel wiring is still mostly done by the stopgap Rust adapter:
  [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1).
- Shared Rust row spawning exists in
  [`src/engine/ecs/system/editor/panel_ui.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/panel_ui.rs:1).
- World panel scene traversal / row semantics already exist in
  [`src/engine/ecs/system/editor/world_panel.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/world_panel.rs:1).

Implication:
- `grid_panel` should follow the same pattern as world/inspector:
  - MMS shell panel
  - Rust-side state + model
  - Rust-side row reconcile / click handling
- repeated grid rows should use `RendererSpec::Rust`
- non-repeated shell elements should stay in MMS

### Inspector details

- [`assets/components/inspector_details.mms`](/home/rei/_/cat-engine/assets/components/inspector_details.mms:1)
  currently renders three fixed display rows: `Name`, `ID`, `GUID`.
- [`src/engine/ecs/system/editor/inspector_panel.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/inspector_panel.rs:518)
  still models inspector detail args as a fixed three-string MMS export.
- There is already an existing design note for a richer inspector:
  [`docs/task/inspector-details-panel.md`](/home/rei/_/cat-engine/docs/task/inspector-details-panel.md:1)

Implication:
- phase 2 should not bolt grid fields onto the current fixed detail tuple
- it should switch inspector details to a field-set render model

### Text input edit path

- [`src/engine/ecs/component/text_input.rs`](/home/rei/_/cat-engine/src/engine/ecs/component/text_input.rs:1)
  supports editable and read-only text inputs.
- [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:234)
  already listens for `SignalKind::TextInputChanged`.

Implication:
- phase 2 can route grid field edits through the existing `TextInputChanged` event path
- numeric validation should happen in the adapter/system layer, not in MMS

### Icons

- [`assets/components/icons.mms`](/home/rei/_/cat-engine/assets/components/icons.mms:1)
  already contains simple procedural panel icons.

Implication:
- phase 1 can add:
  - `delete_x_icon()` as a red four-cube X
  - `grid_visibility_icon()` as the “eye / squashed outer circle + inner circle”

## Gaps

## 1. Grid data model is too small for phase 2

Current grid data only stores spacing/enabled/selectable.

Phase 1 now also wants explicit finite dimensions for rendering, and phase 2 needs those dimensions to become editable.

It also needs to distinguish between:
- persisted grid state
- visual hidden/shown state
- whether a live runtime instance currently exists in the world/BVH

Phase 1/2 needs at least:
- granularity / spacing
- size x
- size z

Likely `GridComponent` expansion:
- `spacing: f32`
- `size_x: i32` or `u32`
- `size_z: i32` or `u32`
- `enabled: bool`
- `hidden: bool`
- `selectable: bool`

Open question:
- if “16x16 squares” is the fixed phase 1 default, should `size_x` / `size_z` mean cell counts or world extents?

Recommendation:
- store them as cell counts in the component
- derive world extent as `size * spacing`
- default both to `16`

Rendering implication:
- the grid renderable should use a plane because the grid now has finite dimensions
- the plane extent should come from `size_x * spacing` and `size_z * spacing`

Runtime implication:
- `enabled = false` should mean:
  - preserve the stored grid state
  - remove the live grid from world/render/BVH participation
  - allow later re-enable to recreate the runtime subtree from saved state
- `hidden = true` should mean:
  - keep the grid state and live instance intact
  - suppress rendering until shown again

## 2. `GridSystem` is not yet a grid registry

The existing `GridSystem` is only an active-grid resolver plus snapping helper.

Phase 1 needs registry-style helpers such as:
- `editor_grids(world, editor_root) -> Vec<GridEntry>`
- `grid_owner_transform(world, grid_component) -> Option<ComponentId>`
- `grid_component_under_transform(world, transform) -> Option<ComponentId>`
- filtering for live grids under the active editor subtree
- helpers that separate stored grid entries from currently-instantiated live grid entities

Important constraint:
- this should be scoped to an editor root, not global across the entire world

Phase 1 should not require authoritative lifecycle APIs such as:
- `GridAttach`
- `GridDetach`
- `GridParentChanged`
- `GridRemove`

Reason:
- in phase 1, grids only need to be useful for editor-side grid management through `grid_panel`
- new grids can be spawned with wrapper transforms by the panel itself
- deletes/toggles/select actions can stay inside that editor-owned flow
- full support for arbitrary external attach/reparent/remove of registered grids is broader lifecycle work and should be deferred

## 3. There is no grid panel shell or grid panel state

We do not have:
- `grid_panel()` in MMS
- a grid panel item row/tile in MMS or Rust
- grid panel state reducer/model
- click handlers for add/delete/visibility/select
- the dedicated grid shader/material hookup described above

Recommended phase 1 shape:
- add `src/engine/ecs/system/editor/grid_panel.rs`
- mirror the split used by `world_panel.rs`:
  - state
  - model
  - row semantics
  - helper functions for row selection and grid actions
- use `DataRendererSystem::render_list(...)` with a Rust row renderer for the repeated grid list
- keep the panel shell, title bar, and add button in MMS
- add the grid render path in parallel:
  - shader files under `assets/shaders`
  - `MaterialHandle::GRID_MATERIAL` or equivalent
  - renderable hookup so editor-created grids use the new material on a plane mesh

## 4. Panel row spawning is currently text-only

[`spawn_panel_ui_row_tree(...)`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/panel_ui.rs:15)
only produces a single labeled row.

That is enough for world/inspector rows, but not for a grid row with:
- label
- visibility icon button
- enable/disable button
- delete icon button

Recommendation:
- do not overload the generic row helper for phase 1
- add a dedicated grid-row tree builder that emits:
  - row payload
  - main click target for selection
  - child click targets for visibility / enable / delete actions
  - explicit `DataComponent` entries describing action kind
- wire that row builder behind `RendererSpec::Rust` rather than treating the list itself as an MMS loop

## 5. Editor selection currently prefers transforms, grid snapping prefers grid component ids

This is the biggest structural seam.

Current behavior:
- editor selection/gizmo wants a transform id
- `GridSystem::active_grid_for_editor()` wants `EditorComponent.selected` to be the `GridComponent`

Those are in tension if a grid row selects the transform, which it should for gizmos.

Two implementation options:

1. Change grid activation to resolve through selection transform
   - if `EditorComponent.selected` is a transform, search its children for a `GridComponent`
   - this is the cleaner direction

2. Keep selection on the grid component and special-case gizmo attachment
   - this fights the current editor selection design

Recommendation:
- phase 1 should update `GridSystem::active_grid_for_editor()` so selected transforms can own active grids
- if the selected component is itself a grid, still support that path for compatibility

## 6. Inspector detail model is still fixed-arity

Current detail renderer contract is:
- `inspector_details(name, id, guid)`

Phase 2 needs:
- a generic list/map of fields
- render hint metadata
- field identity so edit events can be mapped back to a semantic property

Recommendation:
- move `InspectorPanelDetailModel` from:
  - `name`
  - `id`
  - `guid`
- to:
  - `title` or `header`
  - `fields: Vec<InspectorDetailField>`

Example field shape:
- `key: String`
- `label: String`
- `value: String`
- `editable: bool`
- `render_kind: InspectorFieldRenderKind`

For the first pass, `render_kind` can just be `TextInput`.

## Proposed phase 1

## Scope

- new `grid_panel` editor panel
- list all grids under active editor
- add/select/toggle/delete
- selected grid gets gizmo via transform selection
- paint snapping uses selected grid
- grid defaults:
  - spacing `1.0`
  - `size_x = 16`
  - `size_z = 16`

## Spawn shape for a new grid

Recommended world subtree:

```text
grid_transform (TransformComponent, label e.g. "grid_1")
  grid (GridComponent)
  selectable/raycast/render helpers as needed by runtime grid visualization
```

Key requirement:
- the top-level transform is a direct child somewhere under the active editor-authored scene subtree, not under runtime UI

The user requested “under transforms”.
That matches the existing editor selection/gizmo path well.

## Grid panel UI

Recommended row layout per grid:

- left: grid label
- right icon 1: visibility toggle
- right icon 2: delete

Bottom area:
- `Add Grid` button

Behavior:
- click row body: select grid
- click eye: toggle visualization only
- click X: delete grid subtree
- click add: spawn a new default grid under the active editor

Suggested MMS additions:
- `grid_panel(...)` in `assets/components/panels.mms`
- `delete_x_icon()` in `assets/components/icons.mms`
- `grid_visibility_icon()` in `assets/components/icons.mms`

Renderer split:
- MMS:
  - panel frame
  - title bar
  - bottom add button
  - static slots/wrappers
  - icon definitions
- Rust:
  - repeated grid rows
  - row payload/action metadata
  - per-row selection/toggle/delete hit targets

Reason:
- MMS is the right fit for low-count static UI pieces
- repeated panel content should use Rust factory rendering for now because MMS list materialization is still the more expensive path

## Grid panel Rust-side state

Recommended minimal model:

- `GridPanelState`
  - `active_editor: Option<ComponentId>`
  - `selected_grid_transform: Option<ComponentId>`

- `GridPanelEntry`
  - `grid_component: ComponentId`
  - `owner_transform: ComponentId`
  - `label: String`
  - `visible: bool`

This does not need its own independent selection model if it simply reflects editor selection.

## Phase 1 implementation order

1. Expand `GridSystem` helper API so it can enumerate grids under an editor and resolve transform/grid ownership.
2. Update active-grid resolution so selecting a transform that owns a grid still activates snapping.
3. Add finite grid dimensions to `GridComponent` and treat the grid as a plane-backed renderable.
4. Add a dedicated grid shader under `assets/shaders` and a `GridMaterial` material handle for it.
5. Ensure grid lines render slightly thick using multi-`fwidth` style smoothing / width.
6. Add `grid_panel` MMS shell and new icons.
7. Add Rust-side `grid_panel.rs` model/reconcile/click handling.
8. Integrate the new panel into the stopgap editor panel layout beside the existing panels.
9. Add add/toggle/delete/select actions.
10. Verify scene-click selection and panel-click selection both attach gizmos to the grid transform.

## Proposed phase 2

## Scope

- clicking a grid in `grid_panel` also retargets the active unpinned inspector panel
- `GridSystem` can grow from editor-owned registry helpers into an authoritative grid lifecycle owner
- if needed, explicit operations such as `GridAttach` / `GridDetach` / `GridParentChanged` / `GridRemove` land here rather than in phase 1
- inspector details become field-set driven
- selected grid shows editable numeric fields:
  - granularity
  - size x
  - size z

## Inspector field-set direction

Recommended detail model:

```text
InspectorPanelDetailModel
  inspected_component: Option<ComponentId>
  fields: Vec<InspectorDetailField>
```

For a grid selection, fields would be:
- `name`
- `id`
- `guid`
- `grid_spacing`
- `grid_size_x`
- `grid_size_z`

Each field should include:
- stable field key
- user-visible label
- current string value
- editable/read-only flag
- render kind

## MMS detail rendering direction

Rather than `inspector_details(name, id, guid)`, phase 2 should move toward:
- a field-set render contract, likely backed by Rust-side stable subtree spawning
- render hints that currently all map to `TextInput`

The user specifically asked for “a field set in general, not the html element, but like a map of fields, with a param for how they're rendered”.

That should become the actual inspector details contract, even if v1 only supports one render kind.

## Numeric validation behavior

When `TextInputChanged` fires for a grid field:

- parse as number
- reject invalid strings by doing nothing to the actual grid component
- optionally preserve the transient text input state until focus changes

Minimum safe behavior:
- only commit updates when parse succeeds
- do not mutate the grid on arbitrary non-numeric intermediate strings

Open design question:
- whether partially typed values like `"-"` or `"1."` should remain visible without commit

Recommendation:
- accept temporary invalid UI text locally
- only write to `GridComponent` on valid parse
- avoid immediate re-render replacing the user’s transient text mid-edit

## Risks

- The current stopgap adapter already has nontrivial panel reconciliation complexity. A grid panel should reuse those seams rather than introducing a parallel architecture.
- Inspector details are still mid-transition. Phase 2 should avoid hardcoding grid-only branches into a temporary detail shape that will be deleted immediately after.
- Grid selection must be unified around transform ownership, otherwise gizmo targeting and paint snapping will keep disagreeing.

## Recommended concrete file targets

- `assets/shaders/`
- `assets/components/panels.mms`
- `assets/components/icons.mms`
- `src/engine/ecs/component/grid.rs`
- `src/engine/graphics/primitives.rs`
- `src/engine/ecs/system/grid_system.rs`
- `src/engine/ecs/system/editor/grid_panel.rs` (new)
- `src/engine/ecs/system/editor/mod.rs`
- `src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`
- `src/engine/ecs/system/editor/inspector_panel.rs`
- `assets/components/inspector_details.mms`
- [`docs/spec/grid-material.md`](/home/rei/_/cat-engine/docs/spec/grid-material.md:1)

## Broader editor panel direction

The same renderer policy should be treated as the editor-panel default going forward:

- MMS for low-count structural UI
- Rust `RendererSpec::Rust` factories for repeated rows/items/field lists
- `DataRendererSystem` as the shared slot lifecycle owner

That direction is also the path away from the current stopgap adapter, because it lets panel-specific code shrink down to:
- build panel model
- choose renderer spec
- handle semantic actions

instead of mixing shell spawning, row spawning, slot tracking, click routing, and panel workspace state in one file.

## Summary

The repo already has the core grid primitive, editor selection/gizmo plumbing, and a stopgap multi-panel editor shell. The missing work is mostly editor state/reconciliation and a better ownership model between grid transforms, grid components, and inspector detail fields. The main architectural choice to lock in now is:

- grids are selected through their owning transform
- `GridSystem` resolves active grids from either the selected grid component or a selected transform that owns a grid
- inspector details move to a field-set model before grid-specific editable fields are added
