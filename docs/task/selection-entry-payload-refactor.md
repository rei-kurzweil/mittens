= SelectionEntry — Replace `item: Option<String>` with `payload: Option<ComponentId>`

## Motivation

`SelectionEntry::item` is a cached copy of the option's display text that
duplicates what's already in the tree as a `TextComponent`. It's only used
as a display convenience and is stale-prone. Meanwhile the *real* identity
of the selected option flows through a separate path:

1. `SelectionComponent.payload_selector` is a CSS query
2. `resolve_selected_payload()` runs it under the option node after click
3. The matched `DataComponent`'s `ComponentId` is stored on
   `SelectionComponent.selected_payload`

This means per-entry payloads don't exist — `selected_payload` is a single
field, so multi-select loses all payload information. `SelectionEntry` should
carry the payload `ComponentId` directly.

## Principle

Selection / Option components wrap existing UI element trees. The option node
already contains whatever authored content (text, icons, etc.). The
`OptionComponent` marker just makes it selectable. The `SelectionComponent`
manages which ones are highlighted.

The label should be resolved from the tree (the existing `TextComponent`),
not duplicated on the `SelectionEntry`.

## Change

```rust
// Before
pub struct SelectionEntry {
    pub index: Option<usize>,
    pub item: Option<String>,          // duplicated label
    pub component: ComponentId,        // the option node
}

// After
pub struct SelectionEntry {
    pub index: Option<usize>,
    pub component: ComponentId,        // the option node
    pub payload: Option<ComponentId>,  // the DataComponent under it
}
```

### Cascade

1. **`SelectionComponent`** — remove `selected_item`, `selected_payload`.
   Add `selected_payload: Option<ComponentId>` that's just a convenience
   for `selected_entries.last()?.payload`.

2. **`handle_selection_click()`** (`selection_system.rs:634`) — stop
   calling `find_selected_item_text()`. Instead resolve payload after
   building the entry (same as `apply_selection_set` does today).

3. **`apply_selection_set()`** (`selection_system.rs:512`) — already
   resolves payload via `payload_selector`. Just store it on the entry
   instead of on the component-level `selected_payload` field.

4. **`find_selected_item_text()`** (`selection_system.rs:171`) — can be
   removed. Callers that need the label can look it up from the tree.

5. **All consumers** that read `selected_item` or `selected_payload` from
   `SelectionComponent` — update to read from `selected_entries`.

### Cleanup

- `SelectionEvent` and `EventSignal` variants that carry `item: Option<String>`
  should drop that field.
- `IntentValue::SelectionSet` carries `items: Vec<...>` — needs audit.

## Related

- `docs/bugs/paint-panel-free-draw-special-case.md` — the paint panel's
  tool identity is currently resolved via label matching instead of payload.
  This refactor makes payload the canonical path.
