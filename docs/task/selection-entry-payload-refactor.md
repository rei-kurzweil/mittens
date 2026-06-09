= SelectionEntry — Drop `item`, resolve labels from tree

## Motivation

`SelectionEntry::item` is a cached copy of the option's display text that
duplicates what's already in the tree as a `TextComponent`. It's used by
only one external consumer (`editor_paint_system.rs` derives the tool via
`paint_tool_from_item(selected_item)`), and it's stale-prone.

The *real* identity of a selection flows through `selected_payload` — a
`DataComponent` under the option node resolved by `payload_selector`.

## Principle

Selection / Option components wrap existing UI element trees. The option node
already contains whatever authored content (text, icons, etc.). The
`OptionComponent` marker just makes it selectable. The `SelectionComponent`
manages which ones are highlighted.

Labels come from the tree's `TextComponent`, not from a cached copy.

## Change

```rust
// Before
pub struct SelectionEntry {
    pub index: Option<usize>,
    pub item: Option<String>,      // duplicated label — REMOVE
    pub component: ComponentId,    // the option node — KEEP
}
```
`SelectionEntry` becomes just `{ index: Option<usize>, component: ComponentId }`.

**Do NOT add `payload` to `SelectionEntry`.** External consumers never iterate
`selected_entries` — they read `selected_payload` or `selected_component`
directly from `SelectionComponent`. The entries vec is internal plumbing for:
- Multi-select toggle membership checks
- Highlight diff (old vs new entries to restore previous style)

`selected_payload: Option<ComponentId>` stays on `SelectionComponent` as the
resolved `DataComponent` of the primary (last-selected) entry. For multi-select
this is the same "last selected" semantics as today.

### Cascade

1. **`SelectionComponent`** (`selection.rs:16-26`)
   - Remove `selected_item: Option<String>` field
   - Remove from `new()`, `clear()`, `select_entry()`, `toggle_entry()`,
     `sync_primary_from_entries()`
   - Keep `selected_payload`, `selected_entries` as-is

2. **`handle_selection_click()`** (`selection_system.rs:634`)
   - Stop calling `find_selected_item_text()`
   - Build `SelectionEntry { index: selected_index, component: item_id }`
     (no `item` field)
   - `apply_selection_set()` already resolves `selected_payload` after this

3. **`find_selected_item_text()`** (`selection_system.rs:171`) — remove.
   Any caller that needs a label reads it from the tree `TextComponent`.

4. **`apply_selection_set()`** (`selection_system.rs:512`)
   - Already resolves `selected_payload` via `payload_selector` — no change
   - Stop writing `selection.selected_item`
   - Keep reading `selection.selected_component` for highlight diff

5. **`editor_paint_system.rs`**
   - `bootstrap_paint_state()`: read `selected_payload` instead of `selected_item`
   - `sync_paint_state_from_shared_selections()`: same
   - `paint_tool_from_item()`: becomes `paint_tool_from_payload(selected_payload)`
     which reads the `DataComponent`'s `"tool"` key

6. **All other consumers** — no change needed. They already read
   `selected_payload` or `selected_component`, never `selected_item`.

### Event / Intent cleanup

- `EventSignal::SelectionChanged` — drop `selected_item` field
  (`rx/signal.rs:143-149`)
- `SelectionAdded` / `SelectionRemoved` — drop `entry.item` references
- `IntentValue::SelectionSet` — no change (entries already carry `component`
  for highlight diff)

## Related

- `docs/bugs/paint-panel-free-draw-special-case.md` — after this refactor,
  the paint panel reads `selected_payload` to derive the tool instead of
  matching on `selected_item` labels. No more `PaintTool::FreeDraw` special
  case in `is_paint_active()`.
