# Single-select `SelectionComponent` deselects when clicking already-selected option

## Summary

Clicking an already-selected option in a single-select (non-multiple) `SelectionComponent` clears the
selection instead of leaving it intact. The visual result is that the clicked option appears
deselected (highlight removed, background restored).

The correct behavior for single-select is: clicking the already-selected item should be a no-op.

## Location

- `src/engine/ecs/system/selection_system.rs:661-664`

## Root Cause

In `handle_selection_click`, the single-select branch treats a re-click of the currently-selected
entry the same as "toggle it off":

```rust
} else if selection.selected_entries.len() == 1
    && selection.selected_entries[0].component == item_id
{
    (Vec::new(), None)  // ← clears selection
}
```

This logic would be correct for a multi-select toggle, but for single-select (`is_multiple() == false`)
re-clicking the same item should keep the selection unchanged:

```rust
} else if selection.selected_entries.len() == 1
    && selection.selected_entries[0].component == item_id
{
    return;  // no-op, already selected
}
```

## Impact

- In the paint panel, clicking the FreeDraw tool a second time deselects it, making the panel appear
  to have no active tool. The user must click another tool to re-enable painting.
- In any single-select list (asset panel, inspector rows, world panel), re-clicking the active row
  unexpectedly clears the selection.
- If something in the UI depends on `selection.selected_entries` being non-empty (e.g., the paint
  system's `is_paint_active`), the component can enter a state where nothing is visibly selected
  and no action can be taken without an explicit second click on a different option.

## Notes

- `resolve_selection_click` in the same file (`selection_system.rs:133-149`) already handles the
  "clicked on a selection root itself" case by returning `None` (skip). But when clicking an
  option *inside* a selection, the handler treats all click targets the same regardless of whether
  the option is already selected.
