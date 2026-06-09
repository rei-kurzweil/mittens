# Data Event Signal for Horizontal Component Communication

## Problem

Several panels in the editor UI need to communicate across sibling subtrees.
The canonical example is **asset selection → paint panel status**:

```
runtime_ui_root
├── #paint_panel_root          ← handler scoped here
│   ├── #paint_tool_selection  ← SelectionComponent
│   └── #paint_status_wrap
│       └── paint_panel_status_value (Text)
└── #asset_panel_root
    └── #assets_selection      ← SelectionChanged scoped here
```

### Current (broken) approach

A scoped `SelectionChanged` handler is registered at `#paint_panel_root`.
When the user clicks an asset:

1. Selection system emits `SelectionChanged` scoped to `#assets_selection`.
2. `RxWorld::dispatch_event_handlers` calls `compute_scope_chain(world, assets_selection_id)`.
3. The chain walks **up** from `assets_selection_id` to `runtime_ui_root`.
4. `#paint_panel_root` is a **sibling**, not an ancestor, so it is never visited.
5. The paint-state handler is **never invoked**.

The scoped handler dispatch (`dispatch_scoped_kind`) looks up handlers by exact scope.
It does a single `HashMap::get`, not an ancestor walk; the ancestor walk is in
`dispatch_event_handlers` itself, which iterates the scope chain from `compute_scope_chain`.

**Result**: the paint panel never learns about asset selection changes at runtime.
The test suite works around this by calling `push_asset_and_panel_focus` + `bootstrap_paint_state`,
which directly reads `SelectionComponent` from the world and primes the paint state manually.

### Why global handlers don't help

Global handlers would fire for *every* `SelectionChanged` everywhere, requiring every
handler to filter by which selection root it cares about. This works but couples the
handler to a concrete component id and doesn't compose well when multiple cross-panel
relationships exist.

### Why the DataRendererSystem approach also sucked

Before the MMS conversion, the paint panel used a `DataRendererSystem` that re-rendered
the tool row whenever paint state changed. But:
- It was Rust code, not MMS — defeated the goal of MMS-authorable panels.
- Cross-panel communication was still mediated through `PaintState` and scoped handlers.
- It didn't solve the sibling-subtree dispatch problem; it just made the *output* side work.

## Design: Data Event Signals

### Core idea

A **data event signal** is a named event (a string + optional payload) emitted on a
**shared scope** that is an ancestor of both communicating subtrees.
Both sender and receiver refer to the event by name, not by the emitter's component id.

```
runtime_ui_root   ← shared scope for DataEvent signals
├── asset_panel
│   └── on_asset_select:
│       emit DataEvent("asset_selected", payload_id) on runtime_ui_root
└── paint_panel
    └── handler for DataEvent("asset_selected") on runtime_ui_root:
        read payload, update PaintState, update status text
```

### What changes are needed

#### 1. New EventSignal variant

Add to `src/engine/ecs/rx/signal.rs`:

```rust
/// A named data event for cross-subtree communication.
///
/// The `name` is a string key like "asset_selected" that both sender and receiver
/// agree on. The `scope` in the `Signal` envelope identifies the shared ancestor
/// on which the handler is registered.
///
/// Payload is a `ComponentId` reference to a `DataComponent` (or any component
/// the receiver can cast to).
DataEvent {
    name: String,
    payload: Option<ComponentId>,
}
```

The `SignalKind` enum gets a new variant `DataEvent(String)` — the name is part of
the kind so handlers can be registered for specific events.

#### 2. SignalKind carries the event name

Current `SignalKind`:

```rust
pub enum SignalKind {
    Any,
    ParentChanged,
    RayIntersected,
    // ... each event kind ...
}
```

`DataEvent` needs to be keyed by name:

```rust
pub enum SignalKind {
    Any,
    ParentChanged,
    // ... existing kinds ...
    /// Keyed by the DataEvent name string.
    DataEvent(String),
}
```

The `kind()` method on `EventSignal` returns `SignalKind::DataEvent(name)`.

#### 3. Scoped handler registration on shared ancestor

Both panels are under `runtime_ui_root`. The paint panel's setup code (or the MMS
panel template) registers a handler for `DataEvent("asset_selected")` scoped to
`runtime_ui_root`:

```rust
rx.add_handler_closure(
    SignalKind::DataEvent("asset_selected".to_string()),
    runtime_ui_root_id,
    |world, emit, signal| {
        // Extract payload from signal, update paint state, push SetText intent
    },
);
```

Since `runtime_ui_root` is an ancestor of `#assets_selection`, the existing scope-chain
dispatch works: the handler is found when the signal scope is `runtime_ui_root`.

#### 4. Emission from asset panel

When the selection system detects a new asset selection (in the `SelectionChanged`
handler that's already scoped at `#assets_selection`), it emits a `DataEvent` on
`runtime_ui_root`:

```rust
emit.push_event(
    runtime_ui_root_id,
    EventSignal::DataEvent {
        name: "asset_selected".to_string(),
        payload: Some(selected_asset_payload_id),
    },
);
```

#### 5. DataComponent as the payload contract

The `payload` in a data event signal points to a `DataComponent` that carries the
relevant information. For asset selection:

```
DataComponent on payload_id:
  "asset_key" → "some_asset.glb"
  "label"     → "My Asset"
  "asset_type" → "model"
```

The receiver reads the `DataComponent` and extracts the fields it needs.
This is the same `DataComponent` that MMS already supports. No new component types.

### Why this is better than alternatives

| Approach | Problem |
|----------|---------|
| **Scoped handler on panel root** | Can't hear signals from sibling subtrees |
| **Global handler** | Every handler fires for every event → filtering overhead, coupling |
| **Direct selection read** (`bootstrap_selection_event`) | Only works at init time, not reactive |
| **New component on shared ancestor** that polls selection state | Wrong direction; data flows up, not down |
| **Data Event Signals** | Named, decoupled, works with existing scope-chain dispatch |

### Migration plan

1. **Add `DataEvent` variant** to `EventSignal` and `SignalKind::DataEvent(String)`.
2. **Thread `runtime_ui_root_id`** through `SystemWorld` or make it discoverable via
   `find_named_root(world, RUNTIME_UI_ROOT_NAME)` at handler registration time.
3. **Register handler** in `install_shared_panel_handlers`: scoped to `runtime_ui_root`
   for `DataEvent("asset_selected")` (or a `DATA_EVENT_NAME_ASSET_SELECTED` constant).
4. **Emit from selection system** when `SelectionChanged` fires for `#assets_selection`
   and a payload is present.
5. **Remove the broken scoped handler** for `SelectionChanged` at `panel_query_root`.
6. **Remove `bootstrap_paint_state` and `sync_paint_state_from_shared_selections`**
   — the signal-driven path replaces both.
7. **Remove `label_from_selected_payload`** — the receiver reads the `DataComponent`
   directly from the event's payload.

### What about tool selection?

Tool selection happens within the paint panel subtree (`#paint_tool_selection` is a
child of `#paint_panel_root`). The existing scoped handler at `panel_query_root`
*does* work for tool selection because the scope chain from `#paint_tool_selection`
includes `#paint_panel_root`.

So tool selection stays as-is. Only asset-to-paint communication needs the new path.

### Future generalization

The same pattern can be used for any cross-panel data flow:
- World panel selection → inspector panel
- Asset panel import → file tree refresh
- Editor mode change → all panels

Each gets its own `DataEvent` name and a shared scope at `runtime_ui_root`.
