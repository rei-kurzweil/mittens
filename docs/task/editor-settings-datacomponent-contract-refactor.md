# Task: Editor Settings `DataComponent` Contract Refactor

Date: 2026-06-13

Status: planned follow-up / cleanup task

## Intro

We have a temporary runtime fallback in the editor settings interaction path:

- if the settings payload does not expose the expected `DataComponent` fields,
  editor mode resolution falls back to the selected row root's component label

That fallback fixed the immediate regression:

- selecting `3D Cursor` now changes the editor interaction mode correctly
- scene clicks in `3D Cursor` mode no longer select scene objects or attach the
  transform gizmo through the normal select path

However, this is not the model we want long term.

The proper contract should be:

- panel semantics come from `DataComponent`
- selection handlers consume those `DataComponent` fields consistently
- runtime should not infer semantics from component names / labels except as a
  last-resort debug aid during migration

Also note the current remaining user-facing issue:

- in `3D Cursor` mode, the settings/ui mode switch now works
- but the 3D cursor visual still does not appear on scene click

That cursor-visual problem is separate from this data-contract refactor and
should be debugged independently.

## Goal

Make editor settings mode selection use a single consistent `DataComponent`
model end to end, then remove the temporary component-label fallback.

The intended end state is:

1. authored settings rows expose stable semantic fields in `DataComponent`
2. selection runtime preserves those fields on the resolved payload
3. editor-context mode resolution reads only the `DataComponent` contract
4. no editor settings logic depends on row-root labels for normal operation

## Problem

The current trace showed this mismatch:

- authored MMS defines `row_name`, `label`, `mode_value`, `row_kind`,
  `interactive`
- the runtime payload observed during selection only surfaced:
  - `row_kind`
  - `interactive`

Because `row_name` was missing at the point where
`editor_context_event_from_shared_signal()` interpreted the settings click, the
mode-selection branch could not map the chosen row to an
`EditorSettingsOption`.

That is why the editor remained in `Select` mode until the temporary fallback
started using the clicked row root's component label.

## Desired contract

For editor settings rows, the semantic payload should be fully recoverable from
`DataComponent` alone.

Minimum required fields:

- `row_kind = "EditorMode"`
- `row_name = "editor_settings_mode_select" | "editor_settings_mode_cursor_3d" | "editor_settings_mode_select_cursor"`
- `mode_value = "select" | "cursor_3d" | "select_cursor"`

Recommended behavior:

- editor-context mode resolution should prefer `mode_value`
- `row_name` can remain for panel-selection sync and authored row lookup
- `label` remains optional presentation metadata, not the semantic key

That gives us:

- a stable semantic value for runtime behavior
- a stable authored row id for UI synchronization
- no dependency on transform node names or selection-root labels

## Scope

This task is about the editor settings payload contract only.

It includes:

- authored settings row payload shape
- runtime selection payload preservation
- editor-context mode resolution cleanup
- removal of temporary label fallback once the contract is reliable

It does not include:

- the 3D cursor visual placement/rendering bug
- general panel payload standardization outside the editor settings path
- a full rewrite of all panel selection models

## Relevant files

Authored panel rows:

- [assets/components/panels.mms](/home/rei/_/cat-engine/assets/components/panels.mms)

Editor settings interpretation:

- [src/engine/ecs/system/editor/context.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/context.rs)
- [src/engine/ecs/system/editor/settings_panel.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/settings_panel.rs)
- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)

Selection / option payload plumbing:

- [src/engine/ecs/system/selection_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/selection_system.rs)
- [src/engine/ecs/system/panel_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/panel_system.rs)
- [src/engine/ecs/system/editor/panel_ui.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/panel_ui.rs)

Related task docs:

- [option-direct-data-payload-refactor.md](/home/rei/_/cat-engine/docs/task/option-direct-data-payload-refactor.md:1)
- [selection-root-target-subtree-and-direct-option-payloads.md](/home/rei/_/cat-engine/docs/task/selection-root-target-subtree-and-direct-option-payloads.md:1)

## Required refactor

### 1. Decide the semantic field to consume

Use `mode_value` as the primary semantic field for editor mode changes.

Reason:

- it maps directly to `EditorInteractionMode`
- it is less brittle than row names
- it cleanly separates semantic value from UI row identity

Suggested mapping:

- `select` -> `EditorInteractionMode::Select`
- `cursor_3d` -> `EditorInteractionMode::Cursor3d`
- `select_cursor` -> `EditorInteractionMode::SelectAndCursor`

### 2. Preserve the full payload through selection

Trace why the selected payload reaching editor-context drops fields like
`row_name` and `mode_value`.

Possible failure points:

- option payload resolution only keeps a subset of `DataComponent` entries
- some bridge path resolves the wrong `DataComponent`
- a stopgap adapter rebuilds or mirrors payloads incompletely

The fix should ensure the chosen settings payload arrives at
`editor_context_event_from_shared_signal()` with the same semantic fields that
were authored.

### 3. Update editor-context mode resolution

Once the payload contract is reliable:

- read `mode_value` first
- optionally keep `row_name` as a secondary compatibility path during migration
- remove the component-label fallback

The end result should be pure `DataComponent` interpretation.

### 4. Keep UI synchronization on a stable row id

The settings panel still needs to sync the selected row visually.

That means one of these should remain stable:

- `row_name`
- or an equivalent explicit row-id field

The sync path in the stopgap inspector adapter currently finds the row root by
known row names. That is acceptable as a view-sync concern; the semantic mode
change itself should not depend on those labels.

## Proposed implementation shape

### A. Add a payload parser helper

In `editor/context.rs`, add a small helper that reads an editor settings mode
from a `DataComponent` payload:

```rust
fn editor_settings_mode_from_payload(world: &World, payload: ComponentId) -> Option<EditorInteractionMode>
```

Preferred lookup order:

1. `mode_value`
2. temporary compatibility fallback to `row_name`
3. remove label fallback after the payload contract is fixed

### B. Add targeted tests

Add tests for:

- settings selection with payload carrying `mode_value`
- settings selection with payload carrying only `row_name` during migration
- ensure component-label fallback is removed once payload preservation is fixed

### C. Audit shared payload bridges

Audit the code paths that touch settings-row payloads between authored MMS and
editor-context:

- selection resolution
- option payload bridge
- panel bridge / paint bridge logging path
- any stopgap adapter code that clones or proxies `DataComponent`

The concrete bug is likely in one of those translation steps.

## Acceptance criteria

This task is complete when:

1. selecting `3D Cursor` or `Select + Cursor` works without consulting component
   labels
2. the runtime payload observed during settings selection includes the semantic
   fields needed to map the mode
3. editor-context resolves the mode from `DataComponent` fields only
4. the temporary component-label fallback is removed
5. regression tests cover the settings payload contract explicitly

## Out of scope follow-up

After this cleanup, continue debugging the still-open cursor issue:

- `3D Cursor` mode no longer selects objects
- but the cursor visual still does not appear on scene click

That should be tracked as a separate cursor-placement / marker-visualization
bug, not folded into this data-contract refactor.
