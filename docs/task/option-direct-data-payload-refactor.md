# Task: Replace `Selection.payload_selector` With Direct `Option -> Data` Payloads

## Status

Planned.

## Goal

Remove `Selection.payload_selector` and make selection payload resolution follow
a single structural rule:

- the selected `Option` owns its semantic payload
- that payload is represented by a direct child `DataComponent`
- `SelectionComponent.selected_payload` resolves to that direct `Data` child

This keeps payload semantics local to the selected option subtree and avoids
introducing a separate option-payload-specific component model.

## Proposed Contract

Target authoring shape:

```mms
T {
    Option {
        Data {
            name = "option_payload"
            row_kind = "PaintTool"
            label = "Free Draw"
        }
    }

    // visual subtree
}
```

Selection semantics:

1. selection hit resolution still follows `Renderable -> nearest Option ->
   nearest enclosing Selection`
2. once an `Option` is selected, the runtime looks for a direct child
   `DataComponent`
3. if exactly one direct child `DataComponent` exists, that becomes
   `selected_payload`
4. if zero direct `Data` children exist, `selected_payload = None`
5. if multiple direct `Data` children exist, treat that as invalid and resolve
   `selected_payload = None` with a warning in debug/dev builds

This is intentionally narrower than the current query-based model.

## Why This Refactor Exists

Current selection payload behavior is owned by `SelectionComponent`:

- `SelectionComponent.payload_selector: Option<String>`
- selection resolves the payload by querying inside the selected option subtree

That shape has two problems:

1. payload semantics live on `Selection`, even though the payload belongs to the
   selected `Option`
2. payload lookup is query-driven, which keeps authoring and runtime coupled to
   selector strings and named descendants

The desired replacement is a structural convention instead of a lookup API.

## Current Touch Points

### 1. Core selection component and runtime

- [src/engine/ecs/component/selection.rs](../../src/engine/ecs/component/selection.rs)
  - remove `payload_selector` from `SelectionComponent`
  - remove MMS serialization of `.payload_selector(...)`
- [src/engine/ecs/system/selection_system.rs](../../src/engine/ecs/system/selection_system.rs)
  - replace `resolve_selected_payload(..., payload_selector, selected_component)`
    with direct-child `DataComponent` resolution from the selected option root
  - remove query-based payload resolution behavior
  - update warning/error behavior for zero/multiple payload children
  - update selection tests that currently configure `payload_selector`

### 2. MMS component registry / parser integration

- [src/meow_meow/component_registry.rs](../../src/meow_meow/component_registry.rs)
  - remove support for named assignment `payload_selector = ...` on `Selection`
  - remove support for method call `Selection.payload_selector(...)`
  - keep `Data` assignment handling as the primary payload authoring path

### 3. Current MMS authoing using `Selection.payload_selector`

- [assets/components/assets_content.mms](../../assets/components/assets_content.mms)
  - remove `Selection.payload_selector("[name='asset_payload']")`
- [assets/components/panels.mms](../../assets/components/panels.mms)
  - remove selector-based payload configuration for:
    - editor settings selection
    - paint tool selection
    - world panel selection
    - inspector panel selection

These authored trees need to rely on direct child `Data` payloads under each
`Option` instead.

### 4. Option/item factories that already fit or nearly fit the target

- [assets/components/panel_items.mms](../../assets/components/panel_items.mms)
  - `paint_panel_item()` already uses `Option { Data { ... } }`
  - verify the `Data` node is a direct child in the emitted tree and can become
    the canonical pattern

- [assets/components/asset_item.mms](../../assets/components/asset_item.mms)
  - does not currently author a `Data` payload under `Option`
  - needs to move asset identity into a direct `Data` child under `Option`

### 5. Asset payload model

- [src/engine/ecs/component/asset_payload.rs](../../src/engine/ecs/component/asset_payload.rs)
  - currently defines a custom `AssetPayloadComponent`
  - this task should decide whether to:
    - delete it entirely and encode `asset_key` / `title` in `DataComponent`, or
    - keep it temporarily during migration and convert downstream consumers later

- [src/engine/ecs/system/asset_system.rs](../../src/engine/ecs/system/asset_system.rs)
  - currently injects `asset_payload` as a child of the asset item root after
    MMS spawning
  - must instead ensure the selected asset option owns a direct `DataComponent`
    child with the semantic asset identity needed by paint
  - may require changing `asset_item.mms` args or post-spawn topology mutation

This is the main place where the current code does **not** already match the
proposed `Option -> Data` contract.

### 6. Editor-specific payload helpers that query by name

- [src/engine/ecs/system/editor/world_panel.rs](../../src/engine/ecs/system/editor/world_panel.rs)
  - `WORLD_PANEL_PAYLOAD_NAME`
  - world panel selection bootstrap logic currently sets
    `selection.payload_selector`
  - helper resolution should stop querying named descendants and instead use the
    same direct-child payload rule as `selection_system`

- [src/engine/ecs/system/editor/inspector_panel.rs](../../src/engine/ecs/system/editor/inspector_panel.rs)
  - `INSPECTOR_PANEL_PAYLOAD_NAME`
  - `resolve_selected_inspector_panel_payload()` currently queries by name under
    the row root
  - should be replaced with direct child `DataComponent` resolution

- [src/engine/ecs/system/editor/settings_panel.rs](../../src/engine/ecs/system/editor/settings_panel.rs)
  - review editor settings payload creation and resolve path
  - ensure settings options own direct child `DataComponent`s

### 7. Stopgap editor adapter code that manually sets payload selectors

- [src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs)
  - currently assigns `selection_component.payload_selector` for world panel and
    depends on named payload descendants when building/prefilling selection
  - must be updated to use direct-child payload helpers

### 8. Downstream consumers of `selected_payload`

- [src/engine/ecs/system/editor_paint_system.rs](../../src/engine/ecs/system/editor_paint_system.rs)
  - currently accepts either `AssetPayloadComponent` or `DataComponent`
  - after migration, this should prefer or require `DataComponent` payloads for
    asset selection
  - update bootstrap and paint activation tests accordingly

- [src/engine/ecs/system/editor/context.rs](../../src/engine/ecs/system/editor/context.rs)
  - review world/inspector payload-driven editor selection sync
  - verify semantic target resolution still works when payload is always `Data`

### 9. Existing tests and assertions

- [src/engine/ecs/system/selection_system.rs](../../src/engine/ecs/system/selection_system.rs)
  - tests around `selection_payload_selector_*`
  - replace with tests for:
    - direct child `Data` payload resolves
    - zero direct child payloads return `None`
    - multiple direct child payloads return `None`
    - nested non-direct `Data` does not count

- [src/engine/ecs/system/editor_paint_system.rs](../../src/engine/ecs/system/editor_paint_system.rs)
  - asset selection bootstrap tests currently assert `AssetPayloadComponent`
  - update to assert `DataComponent` payload semantics

- [src/engine/ecs/system/editor/context.rs](../../src/engine/ecs/system/editor/context.rs)
  - update payload-based selection sync tests to use direct child `Data`

## Required Design Decisions

### 1. Should asset payloads fully migrate to `DataComponent`?

Recommended answer: yes.

Reason:

- the refactor goal is to avoid a second payload model
- assets are the main outlier preventing the contract from being uniform
- `DataComponent` already supports text/component fields needed for
  `asset_key`, `title`, `target_component`, `row_kind`, and similar metadata

Suggested `Data` keys for assets:

- `asset_key: Text`
- `title: Text`
- `row_kind: Text("Asset")`

### 2. What counts as “direct child of Option”?

Be precise here.

`Option` is currently just a component marker attached somewhere on the option
root transform, not a container node of its own. In practice this task should
define:

- payload resolution starts from the selected option root component
- inspect only the immediate children of that component
- among those children, resolve direct `DataComponent` children

Do **not** use descendant queries, sibling scans, or fallback traversal.

### 3. Should `Option` require a payload?

Recommended answer: no.

Some options may be selectable for highlighting/focus without carrying semantic
payload data. In those cases:

- selection still works
- `selected_component` is set
- `selected_payload` remains `None`

## Proposed Work Plan

1. Replace query-based payload resolution in `selection_system` with a direct
   child `DataComponent` helper.
2. Remove `payload_selector` from `SelectionComponent` and MMS registry support.
3. Convert authored MMS call sites away from `Selection.payload_selector(...)`.
4. Migrate world/inspector/settings/paint row payloads to direct child `Data`.
5. Migrate asset items away from `AssetPayloadComponent` to direct child `Data`.
6. Update editor helpers and stopgap adapter code to stop querying named
   payload descendants.
7. Rewrite tests to lock in the new structural contract.

## Acceptance Criteria

1. `SelectionComponent` no longer stores `payload_selector`.
2. MMS no longer accepts `Selection.payload_selector(...)` or
   `payload_selector = ...` on `Selection`.
3. Selecting an option resolves `selected_payload` only from a direct child
   `DataComponent` on the selected option root.
4. Asset, world, inspector, paint, and editor-settings selections all work
   without selector strings.
5. Paint asset selection uses `DataComponent` payloads rather than
   `AssetPayloadComponent`.
6. Tests cover zero, one, and multiple direct child payload cases.

## Risks

- Asset item spawning currently injects payloads after MMS creation, so asset
  topology may need more restructuring than the other panels.
- The distinction between “option root” and “child components attached via
  `Option { ... }`” must be defined carefully or payload lookup will become
  ambiguous again.
- If any current panel relies on payloads deeper than one level below the
  option root, this refactor will force that tree to be rewritten.
