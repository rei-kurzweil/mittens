# Data Renderer System — v1 API

## Status

Spec / implementation target for Phase 1 of the data renderer system.

Builds on the proposal at [`docs/task/data-renderer-system-for-editor-ui.md`](/home/rei/_/cat-engine/docs/task/data-renderer-system-for-editor-ui.md).

## 1. Overview

The data renderer system owns the projection of structured editor UI data into live ECS component subtrees. It encapsulates:

- the mapping from data payloads to rendered subtree instances
- target-slot identity and lifecycle
- full-rerender semantics (Phase 1 -- no incremental patching yet)

The system is parametric over its payload type via a generic `RendererSpec<T>`. Two concrete instantiations are defined for v1:

| Type alias | Payload | Rendering strategy |
|---|---|---|
| `ItemRendererSpec` | `UiItem` (per row) | System iterates items, renders each independently, collects under a container |
| `DetailRendererSpec` | `UiDetailItem` | System renders once, attaches result directly |

Both MMS-driven and Rust-driven renderers are supported for each.

## 2. Payload types

### 2.1 UiItem (list/item rows)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiItemKind {
    Component,
    Info,
    EditorRoot,
    Spacer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiItem {
    pub key: String,
    pub kind: UiItemKind,
    pub label: String,
    pub selected: bool,
    pub target_ref: Option<ComponentId>,
}
```

**Fields:**

| Field | Purpose |
|---|---|
| `key` | Stable identity for the item. Required from v1 even with full-rerender -- enables future keyed patching and debug naming. |
| `kind` | Tag for the renderer to distinguish row types. Closed enum matching current `WorldPanelRowKind` and `InspectorPanelRowKind`. |
| `label` | Display text for the row. |
| `selected` | Whether this item is currently selected/highlighted. |
| `target_ref` | Direct ECS reference to the scene component this row represents. `None` for non-interactive rows (Info, Spacer). |

### 2.2 UiDetailItem (detail/inspector views)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiDetailItem {
    pub name: String,
    pub id: String,
    pub guid: String,
}
```

Matches the current `InspectorPanelDetailModel` and the positional args of the `inspector_details` MMS export.

Will be generalized (to `view_kind` + `fields: Vec<UiField>`) in a later phase when more detail view types exist.

## 3. RendererSpec

```rust
/// How to render one unit of data into a live component subtree.
pub enum RendererSpec<T> {
    /// Materialize an MMS component expression and spawn into the ECS.
    Mms {
        asset_path: &'static str,
        export_name: &'static str,
        to_args: fn(&T) -> Vec<Value>,
    },
    /// Build and spawn a component subtree directly from Rust.
    Rust {
        render_fn: fn(&mut World, &mut dyn SignalEmitter, &T) -> Result<ComponentId, String>,
    },
}
```

### Type aliases

```rust
pub type ItemRendererSpec = RendererSpec<UiItem>;
pub type DetailRendererSpec = RendererSpec<UiDetailItem>;
```

### Variant semantics

**Mms variant:**
- The system calls `MeowMeowRunner::materialize_mms_module_component_from_file(path, name, args, world, emit)` where `args = to_args(payload)`.
- The resulting `MaterializedCE` is spawned via `spawn_tree`.
- The `to_args` function bridges the structured payload to the positional `Vec<Value>` arguments the MMS export expects.

**Rust variant:**
- The system calls `render_fn(world, emit, payload)`.
- The function is responsible for building the subtree (using `world.add_component_boxed_named`, `world.add_child`, etc.) and returning the root `ComponentId`.
- This is the same pattern as the current `spawn_panel_ui_row_tree`.

## 4. DataRendererSystem

### Slot lifecycle tracking

The system maintains a `HashMap<ComponentId, ComponentId>` mapping slot → most recently rendered subtree root. This enables the full-rerender lifecycle: on each call, the previous subtree for that slot is removed before the new one is attached.

If a slot's `ComponentId` becomes stale (component removed from world), the system detects this and clears the stale entry on the next operation.

### Public API

```rust
pub struct DataRendererSystem {
    rendered_subtrees: HashMap<ComponentId, ComponentId>,
}

impl DataRendererSystem {
    pub fn new() -> Self;

    /// Render a list of items into a target slot.
    ///
    /// Lifecycle:
    ///   1. Remove any previously rendered subtree for this slot.
    ///   2. Create a container component to hold all items.
    ///   3. For each item, call the spec's renderer (MMS or Rust).
    ///   4. Attach each rendered subtree as a child of the container.
    ///   5. Attach the container to the slot.
    ///   6. Mark nearest layout dirty.
    ///
    /// Returns the container `ComponentId` so callers can attach additional
    /// panel-specific state (e.g. `SelectionComponent`).
    ///
    /// Error policy: returns Err on materialization/spawn failure.
    /// The caller decides how to surface the error.
    pub fn render_list(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        slot: ComponentId,
        spec: &ItemRendererSpec,
        items: &[UiItem],
    ) -> Result<ComponentId, String>;

    /// Render a detail view into a target slot.
    ///
    /// Lifecycle:
    ///   1. Remove any previously rendered subtree for this slot.
    ///   2. Call the spec's renderer once (MMS or Rust).
    ///   3. Attach the resulting subtree to the slot.
    ///   4. Mark nearest layout dirty.
    ///
    /// Returns the root `ComponentId` of the rendered subtree.
    pub fn render_detail(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        slot: ComponentId,
        spec: &DetailRendererSpec,
        detail: &UiDetailItem,
    ) -> Result<ComponentId, String>;

    /// Remove any rendered content for this slot. No-op if nothing is tracked.
    pub fn clear_slot(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        slot: ComponentId,
    );
}
```

### What the system owns

- The **container** for list items (a `TransformComponent` + `StyleComponent` with `display: block`, `width: 100%`, `overflow: visible`).
- The **remove → spawn → attach** lifecycle for the slot.
- The **rerender policy** (full-rerender in v1).
- The **slot → subtree tracking** in `rendered_subtrees`.

### What the system does not own

- Editor workspace reducer logic.
- Cross-panel coordination rules.
- Semantic selection decisions.
- Scene mutation logic.
- Local interaction state (hover, scroll, text caret, etc.).

## 5. Rendering lifecycle (detail)

### 5.1 render_list

```
1. Validate slot exists in world (else clear stale entry, return Err)
2. Remove previous rendered subtree for this slot:
   - If rendered_subtrees contains an entry for slot:
     - Push RemoveSubtree intent for that root
     - Remove from rendered_subtrees
3. Spawn container:
   - TransformComponent (named "data_renderer_list_container_{slot}")
   - StyleComponent (block, width 100%, overflow visible)
4. For each item (in order):
   - Match spec:
     - Mms: materialize_mms_module_component_from_file → spawn_tree → child_id
     - Rust: render_fn(world, emit, item) → child_id
   - world.add_child(container, child_id)
5. Attach container to slot:
   - Push Attach intent (parent: slot, child: container)
   - Insert slot → container into rendered_subtrees
6. mark_nearest_layout_dirty(world, slot)
```

### 5.2 render_detail

```
1. Validate slot exists in world (else clear stale entry, return Err)
2. Remove previous rendered subtree for this slot
3. Match spec:
   - Mms: materialize_mms_module_component_from_file → spawn_tree → root
   - Rust: render_fn(world, emit, detail) → root
4. Push Attach intent (parent: slot, child: root)
5. Insert slot → root into rendered_subtrees
6. mark_nearest_layout_dirty(world, slot)
```

### 5.3 Error handling

- If `materialize_mms_module_component_from_file` fails, return `Err` with the MMS error message.
- If `spawn_tree` fails, return `Err`.
- If the Rust `render_fn` returns `Err`, propagate it.
- On error, the slot is left empty (previous content was already removed).
- The caller is responsible for logging or surfacing the error.

## 6. First integration targets

### 6.1 Inspector sidebar rows (Rust-backed)

```rust
static INSPECTOR_ROW_SPEC: ItemRendererSpec = RendererSpec::Rust {
    render_fn: |world, emit, item: &UiItem| {
        // Call into a refactored spawn_inspector_panel_row_tree
        // that takes UiItem instead of InspectorPanelRow
    },
};
```

Called from the reducer/effect layer when inspector panel state changes:
```rust
data_renderer.render_list(world, emit, sidebar_slot, &INSPECTOR_ROW_SPEC, &rows)?;
```

### 6.2 Inspector details (MMS-backed)

```rust
static INSPECTOR_DETAIL_SPEC: DetailRendererSpec = RendererSpec::Mms {
    asset_path: "assets/components/inspector_details.mms",
    export_name: "inspector_details",
    to_args: |detail: &UiDetailItem| {
        vec![
            Value::String(detail.name.clone()),
            Value::String(detail.id.clone()),
            Value::String(detail.guid.clone()),
        ]
    },
};
```

Called from the reducer/effect layer:
```rust
data_renderer.render_detail(world, emit, detail_slot, &INSPECTOR_DETAIL_SPEC, &detail)?;
```

## 7. Open questions (tracked for later phases)

- Should `RendererSpec<T>` gain an `Args` associated type for per-instance configuration beyond the payload? (Phase 5+)
- How should the system handle slot ComponentId reuse after panel layout rebuild? (Phase 4)
- Should `clear_slot` also take a spec to validate that the caller is clearing the right kind of content? (Phase 4)
- What is the right key for slot tracking when panels can be rearranged? A panel-scoped slot name rather than a ComponentId? (Phase 4)
- Should the container for list rendering be configurable (e.g., gap between items)? (Phase 2)

## 8. Future: UiItemKind extensibility

When new row kinds are needed (e.g., `Asset`, `PaintTool`, `SectionHeader`), the enum is extended:

```rust
pub enum UiItemKind {
    Component,
    Info,
    EditorRoot,
    Spacer,
    // future:
    // Asset,
    // PaintTool,
    // SectionHeader,
}
```

Each renderer spec either handles the new kind or returns a fallback (e.g., an Info row with "unhandled kind: Asset").
