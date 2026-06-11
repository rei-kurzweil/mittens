# Task: Editor Settings Panel

Date: 2026-06-10

Status: design / implementation task.

## Purpose

Add a dedicated editor settings panel that exposes a small set of editor-scoped runtime settings:

1. transform gizmo translation space: `World` / `Local`
2. transform gizmo rotation space: `World` / `Local`
3. glTF armature visualization: `On` / `Off`

This should be a real editor panel, rendered entirely from
`assets/components/editor_settings_panel.mms` via a `RendererSpec`, rather than another Rust-only
stopgap subtree builder.

## Why this exists

We now have editor-level gizmo space settings on `EditorComponent`, but they are not exposed in the
runtime editor UI.

Separately, armature/bone visualization is useful while inspecting rigs, but it also creates visual
and interaction noise. We need a quick toggle in the editor so a user can turn it off without
removing the underlying glTF content.

## Scope

This task is about the editor panel and the state plumbing required to make the three settings
toggle live at runtime.

It is not a full editor preferences system, and it is not a general persistence/settings-profile
design.

## Existing state and ownership

### Gizmo coord spaces

These are editor-scoped settings, currently stored on `EditorComponent`:

- `transform_gizmo_translation_space: TransformGizmoCoordSpace`
- `transform_gizmo_rotation_space: TransformGizmoCoordSpace`

Those fields should remain the source of truth for the two gizmo rows.

### Armature visualization

For v1, we do not need a perfect long-term armature helper-tree architecture here. A pragmatic first
implementation is acceptable:

- toggling the setting may remove and re-attach / rebuild editor-owned glTF armature visualization
  subtrees
- this should affect visualization helpers only, not the authored glTF transform/joint tree itself

The panel should treat this as an editor-scoped toggle, even if the first implementation is a bit
heavy.

## Panel form

Create:

- `assets/components/editor_settings_panel.mms`

The panel should render three rows.

Each row has:

- a left column for the label
- a right column for the current value / toggle button

The requested layout shape is:

- each row is horizontally arranged
- the left column uses a `T { Style { display("inline-block") } }` wrapper
- the right column sits beside it in the same row

### Rows

1. `Translation Space`
2. `Rotation Space`
3. `Show Armature`

### Right-column control behavior

Each right-column control is a button-like authored subtree that:

- changes label text based on current value
- changes color based on current value
- emits a panel interaction that flips or cycles the setting

Expected labels:

- translation space: `world` / `local`
- rotation space: `world` / `local`
- show armature: `on` / `off`

Expected visual behavior:

- active state is visually distinct from inactive state
- do not rely on text alone; color should change too

## Rendering path

This panel should be driven through the data renderer path rather than custom Rust-only panel tree
construction.

Use a `RendererSpec::Mms` path targeting:

- asset path: `assets/components/editor_settings_panel.mms`
- export name: panel export to be defined in that file

The exact payload type can be small and purpose-built for v1. It does not need to fit the existing
world-panel or inspector row models.

Reasonable options:

1. render the whole panel from one detail-style payload
2. render the three rows from an item renderer and keep the surrounding shell separate

For v1, prefer the simpler route with the least adapter glue.

## Suggested runtime model

Introduce a compact editor-settings view model for rendering:

```rust
pub struct EditorSettingsPanelModel {
    pub translation_space: TransformGizmoCoordSpace,
    pub rotation_space: TransformGizmoCoordSpace,
    pub show_armature: bool,
}
```

This model is derived from editor runtime state and then passed to the MMS renderer.

## Interaction contract

Each row should produce an editor-scoped action.

Suggested action set:

```rust
pub enum EditorSettingsAction {
    CycleTranslationSpace,
    CycleRotationSpace,
    ToggleShowArmature,
}
```

Expected semantics:

- `CycleTranslationSpace`: `World <-> Local`
- `CycleRotationSpace`: `World <-> Local`
- `ToggleShowArmature`: `true <-> false`

The panel should not mutate gizmo or glTF state directly from MMS. MMS should only surface the user
intent; Rust-side editor systems remain responsible for applying effects.

## Effect semantics

### 1. Translation space

When toggled:

- update the owning `EditorComponent.transform_gizmo_translation_space`
- ensure the transform gizmo visuals are refreshed if needed so the arrows reflect the new space
- subsequent drags should use the new space immediately

### 2. Rotation space

When toggled:

- update the owning `EditorComponent.transform_gizmo_rotation_space`
- ensure the transform gizmo visuals are refreshed if needed so the rings reflect the new space
- subsequent drags should use the new space immediately

### 3. Show armature

When toggled off:

- remove or detach editor-owned armature visualization helpers for glTF content in that editor
  scope

When toggled on:

- rebuild or re-attach those helpers

V1 allowance:

- it is acceptable if this is implemented by re-running the armature visualization attach path for
  relevant glTF/editor roots
- avoid mutating or recreating the authored glTF subtree itself

## Topology / ownership constraints

The panel should be editor-scoped, not global.

That means:

- each `EditorComponent` subtree can have its own settings state
- toggling settings in one editor should not implicitly mutate a different editor subtree

Likewise, armature visibility should operate on editor-owned helper content for that editor, not as
a global process-wide flag.

## Relation to gizmo math

The coord-space rows are settings gates for gizmo behavior, not separate gizmo implementations.

Current expected mapping:

- translation space row controls which translation basis the gizmo uses
- rotation space row controls which rotation basis the gizmo uses

The underlying transform storage is still local TRS on `TransformComponent`. So when a gizmo uses a
world-space intent, Rust-side gizmo logic must convert that world-space delta or axis into the
target's parent-local frame before writing local transform values.

That conversion logic belongs in the gizmo/editor implementation, not in the panel.

## Suggested implementation stages

### Stage 1 — MMS asset

- create `assets/components/editor_settings_panel.mms`
- author the panel shell and the three rows
- author button states for:
  - `world`
  - `local`
  - `on`
  - `off`

### Stage 2 — panel spawn / rendering

- add an editor settings panel spawn path in the editor UI setup
- render it through `RendererSpec::Mms`
- define a small payload/model-to-MMS-args adapter

### Stage 3 — interaction wiring

- route panel clicks to `EditorSettingsAction`
- update the owning `EditorComponent` fields or editor runtime state

### Stage 4 — gizmo refresh

- ensure toggling translation/rotation space updates existing gizmo visuals and subsequent drag
  semantics without requiring a full editor rebuild

### Stage 5 — armature visibility v1

- connect `show_armature` to armature helper-tree removal / re-attachment
- keep the implementation scoped to editor-owned visualization content

## Acceptance criteria

- an editor settings panel appears in the editor UI
- it renders from `assets/components/editor_settings_panel.mms`
- it has exactly three visible settings rows:
  - `Translation Space`
  - `Rotation Space`
  - `Show Armature`
- each row has a left label column and a right value/button column
- translation space button toggles between `world` and `local`
- rotation space button toggles between `world` and `local`
- show armature button toggles between `on` and `off`
- button color changes with state
- changing translation space affects existing gizmo behavior in that editor
- changing rotation space affects existing gizmo behavior in that editor
- toggling armature visibility removes/restores editor-owned glTF bone visualization for that
  editor scope
- the authored glTF content itself is not destroyed or recreated as part of the armature toggle

## Non-goals for v1

- persistence of settings across app restarts
- a generalized preferences schema
- a polished final armature-helper architecture
- a broad multi-panel editor settings framework
- refactoring every editor panel to the same model in this task

## Related

- [docs/spec/editor-gizmo-coord-spaces.md](../spec/editor-gizmo-coord-spaces.md)
- [docs/spec/data-renderer-system.md](../spec/data-renderer-system.md)
- [docs/task/armature-visualization-toggle.md](./armature-visualization-toggle.md)
- [docs/task/serialize-component-and-armature-viz-save-plan.md](./serialize-component-and-armature-viz-save-plan.md)
