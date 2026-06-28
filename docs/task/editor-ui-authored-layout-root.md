# Editor UI Authored Layout Root

Date: 2026-06-27

Status: planned task.

See:

- [docs/spec/editor-ui.md](/home/rei/_/cat-engine/docs/spec/editor-ui.md:1)
- [docs/task/editor-slot-projection-and-mount-points.md](/home/rei/_/cat-engine/docs/task/editor-slot-projection-and-mount-points.md:1)
- [docs/draft/shared-editor-ui-routing-layer.md](/home/rei/_/cat-engine/docs/draft/shared-editor-ui-routing-layer.md:1)

## Goal

Move the shared editor panel `LayoutRoot` mount from Rust-authored topology into an MMS-authored `EditorUI` export, while keeping current panel state ownership and panel refresh logic in Rust.

The first implementation should only expose the shared editor UI subtree to the MMS / scene graph API.

It should not yet introduce configurable default panel sets or panel subsets.

It should also define the fallback bootstrap path used when editor scopes exist but no authored `EditorUI` is present.

## Why this task exists

Today the editor runtime already treats panel shells as authored MMS trees, but the shared editor layout mount is still created by Rust in
[src/engine/ecs/system/panel_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:245)
and
[src/engine/ecs/system/panel_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:557).

That prevents authored scene topology from owning editor UI placement.

The first refactor should fix only that ownership boundary.

## Desired outcome

After this task:

- the shared editor UI mount is authored in MMS
- the shared editor `LayoutRoot` appears as normal scene-graph topology
- `EditorUI` can be wrapped or positioned by authored transform ancestry
- editor initialization auto-inserts a default transform-wrapped `EditorUI` if `Editor {}` exists and no `EditorUI` exists
- Rust still finds the same layout/panel selectors and continues to drive dynamic panel content

## Non-goals

- changing which default panels exist
- exposing panel inclusion/exclusion to MMS
- introducing named panel presets
- moving panel reducer/workspace state into MMS
- redesigning panel shell internals unrelated to the mount contract

## Required output

### 1. Add an authored `EditorUI` export

Create an MMS export, likely in
[assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:1)
or a dedicated editor-ui module, that owns:

- `#editor_panel_layout_mount`
- `Overlay`
- `#editor_panel_layout_root`
- `#editor_panel_layout_selection`

It should accept the sizing/configuration values currently passed through `PanelLayoutMountSpec`, or an equivalent narrowed arg set.

### 1.5 Add fallback bootstrap behavior

When editor systems initialize:

- if one or more `Editor {}` components exist
- and no `EditorUI` exists in the world
- insert a default `EditorUI` automatically

That fallback insertion should create a transform wrapper around the `EditorUI` instance so the runtime-owned path still presents normal scene topology.

If an authored `EditorUI` already exists, do not insert a duplicate fallback UI.

### 2. Preserve the selector contract

The new authored subtree must preserve the selectors that current runtime code depends on.

At minimum:

- `#editor_panel_layout_mount`
- `#editor_panel_layout_root`
- `#editor_panel_layout_selection`
- existing panel root selectors used by `resolve_panel_instance(...)`

If any selector changes, all runtime resolution code and tests must be updated together.

### 3. Replace Rust-authored mount assembly

Refactor the shared editor UI spawn path so Rust no longer manually emits:

- outer `T.position(...)`
- `Overlay`
- `LayoutRoot`

for the shared editor workspace path.

Rust should instead materialize/spawn the authored `EditorUI` export and then resolve the same runtime nodes from the live tree.

For the fallback path, Rust may still create the outer transform wrapper, but the inner shared UI subtree should come from the same `EditorUI` export/contract.

### 4. Keep panel runtime behavior stable

Do not change the current ownership of:

- default panel creation
- panel models
- content rerender/projection
- shared panel handlers
- workspace reducer/state logic

If additional compatibility glue is needed, keep it local to the spawn/materialization path.

## Suggested implementation steps

### Phase 1. Extract the authored mount topology

- mirror the current mount structure in MMS
- keep the same names/selectors
- thread current width/height/unit-scale values into the authored export

### Phase 2. Switch shared editor spawn to `EditorUI`

- replace `build_panel_layout_mount_ce(...)` / `spawn_panel_layout_mount(...)` usage in the shared editor path
- spawn the authored `EditorUI` subtree instead
- add presence detection for existing `EditorUI`
- auto-spawn the fallback transform-wrapped `EditorUI` only when editor scopes exist and no authored `EditorUI` is present
- keep panel shell children and runtime resolution behavior unchanged

### Phase 3. Reconcile selection ownership

Today Rust may still call `ensure_panel_layout_selection(...)`.

For the authored-root path, decide one of:

- author the selection node directly in `EditorUI` and remove the post-spawn creation path
- temporarily keep the Rust fallback, but make it a compatibility guard rather than the normal path

The preferred end state is for `#editor_panel_layout_selection` to be authored and stable.

### Phase 4. Verify existing panel resolution paths

Confirm that these continue to work against the authored subtree:

- static panel caching in `EditorWorkspaceRuntime`
- `resolve_panel_instance(...)`
- shared panel handler installation
- panel layout selection and panel focus behavior
- world/paint/assets/grid/pose/settings selector lookups
- no duplicate shared editor UI when an authored `EditorUI` already exists
- fallback shared editor UI appears when `Editor {}` exists and `EditorUI` does not

## Code areas likely involved

- [src/engine/ecs/system/panel_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs:1)
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)
- [src/engine/ecs/system/editor/workspace.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/workspace.rs:1)
- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms:1)

## Validation

The implementation should preserve current behavior for:

- shared panel spawning exactly once
- panel layout selection resolving under the shared editor UI root
- world panel, assets panel, paint panel, grid panel, pose panel, and settings panel root discovery
- panel content projection into existing slots
- automatic fallback `EditorUI` spawn only when required by the bootstrap rule

Relevant test areas likely include:

- editor panel setup tests in
  [src/engine/ecs/system/editor_inspector_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system.rs:1)
- selection behavior tests in
  [src/engine/ecs/system/selection_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/selection_system.rs:1)

## Open questions

- Should the authored export be named `EditorUI`, `editor_ui`, or something panel-system-specific?
- Should the current sizing values remain Rust-provided args, or should some become authored constants?
- Should the panel-layout selection root be fully authored in the first pass, or kept behind a temporary Rust fallback?
- What exact query should runtime use to detect whether an `EditorUI` already exists in the live world?
- What default transform placement should the fallback wrapper use before authored placement is provided?

## Definition of done

- there is a written `EditorUI` v1 contract
- the shared editor layout root is no longer Rust-authored mount topology
- authored placement of the shared editor UI subtree is possible
- fallback bootstrap creates one transform-wrapped `EditorUI` when needed and does not duplicate authored `EditorUI`
- runtime panel resolution still works through stable selectors
- panel preset/subset state is still explicitly out of scope
