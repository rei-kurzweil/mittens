# Inspector details panel: sidebar + detail view with editable fields

Date: 2026-06-06

Status: planning only.

This is a `docs/task` note only. No `src/` or `assets/` changes are proposed here yet.

## Goal

1. Create a new MMS component `inspector_details()` in `assets/components/inspector_details.mms`
   with a 2-column form layout (label | TextInput).
2. Restructure the inspector panel so the current row list becomes a **sidebar** on the left,
   and the new `inspector_details` takes up most of the width on the right.
3. Wire sidebar selection to show the selected component's fields (name, id, guid — only name
   editable to start).
4. Double the inspector panel width to accommodate the new layout.

## Current state

The inspector panel (`assets/components/panels.mms:inspector_panel`) is a single `content_slot`
containing `inspector_panel_content(items, item_background_color)` — a flat list of rows that
mirror the ECS subtree of the selected component. There is no detail view, no editable fields,
no form layout.

The detail view is currently disabled in the live Rust update path while the sidebar layout is
being stabilized. The current blocker is that the inspector sidebar renders at an incorrect text
scale and the pin button wraps outside the title bar.

Width constants:
- `INSPECTOR_PANEL_WIDTH_GU = 22.0` in `panels.mms`
- `INSPECTOR_PANEL_WIDTH_GU = 22.0` in `editor_inspector_system_stopgap_mms_adapter.rs:58`

The panel is spawned per-inspected-component via `spawn_inspector_panel_instance_tree()` which
calls `build_panel_component_expr()` with the MMS `inspector_panel` factory, passing rows as
an empty `Value::Array` (rows are populated imperatively via
`spawn_inspector_panel_content_tree` → `spawn_inspector_panel_row_tree`).

The detail subtree is still modeled in Rust, but its live render call is currently disabled while
the sidebar/text sizing regression is addressed.

## Proposed layout

```
┌─────────────────────────────────────────────────┐
│ title_bar                          [pin_button] │
├────────────────────┬────────────────────────────┤
│ sidebar            │ detail                     │
│ (current row list) │ ┌─────────────────────────┐│
│                    │ │ Label       │ TextInput  ││
│   ◉ ComponentA     │ │ Name        │ "foo"      ││
│     ├ child1       │ │ ID          │ "42"      ││
│     └ child2       │ │ GUID        │ "abc…"    ││
│                    │ └─────────────────────────┘│
│   ○ ComponentB     │                            │
│     └ sub          │  (content area, scrolls)   │
│                    │                            │
└────────────────────┴────────────────────────────┘
```

- **Sidebar** (left, ~1/3 width): the existing `inspector_panel_content` row list.
  Selecting a row sets which component's details appear in the detail view.
- **Detail** (right, ~2/3 width): a scrollable form with 2-column rows.
  Each row is `Text(label)` | `TextInput(value)`.

## `inspector_details()` MMS component

New file: `assets/components/inspector_details.mms`

```mms
// inspector_details.mms — component detail form (=^･ω･^=)
//
// Renders a 2-column form for inspecting/editing a component's fields.
// Called with:
//   inspector_details(name, id, guid)
// where only name is editable (TextInput), id/guid are display-only Text.

export fn inspector_details(name, id, guid) {
    // returns a styled container with the form
}
```

Fields to start:
| Field  | Widget    | Editable |
|--------|-----------|----------|
| name   | TextInput | yes      |
| id     | Text      | no       |
| guid   | Text      | no       |

Each row is a styled `inline-block` pair in a `block` row container:
```
Text("Name")       TextInput("current_value")
Text("ID")         Text("42")
Text("GUID")       Text("abc-def-123")
```

The label column right-aligns or left-aligns consistently. The input/value column
takes the remaining width.

## Changes to `inspector_panel()` in `panels.mms`

The `inspector_panel` function gets a new internal structure:

```mms
export fn inspector_panel(title, items, title_color, panel_background_color, item_background_color) {
    // ... title bar (unchanged) ...

    // Split content into two columns
    T {
        name = "content_area"
        Style { display("block") ... }

        // Sidebar (left)
        T {
            name = "sidebar"
            Style { display("inline-block") width(35%) ... }
            inspector_panel_content(items, item_background_color)
        }

        // Detail (right)
        T {
            name = "detail_area"
            Style { display("inline-block") width(65%) ... }
            inspector_details(name, id, guid)
        }
    }
}
```

The sidebar retains the existing selection behavior (click a row to select).
When the detail view is re-enabled, the detail area should re-render with the selected
component's fields. For now, it stays disabled so the sidebar can be stabilized on its own.

## Width changes

The inspector panel needs to be wider — roughly **2× the current width** to fit both
sidebar and detail view:

- `INSPECTOR_PANEL_WIDTH_GU` in `panels.mms`: `22.0` → `44.0` (or `46.0` to account for gaps)
- `INSPECTOR_PANEL_CONTENT_HEIGHT_GU` in `panels.mms`: may need adjustment for taller content
- `INSPECTOR_PANEL_WIDTH_GU` in `editor_inspector_system_stopgap_mms_adapter.rs:58`: update to match
- Width constants in `spawn_panel_layout()`: update `INSPECTOR_PANEL_WIDTH_GU` there too

## Data flow (stopgap adapter side)

Currently the Rust adapter owns the row list and re-renders it imperatively
(`spawn_inspector_panel_content_tree`). For the new architecture:

1. The adapter still builds the sidebar row list from the ECS tree (unchanged).
2. When a sidebar row is selected, the adapter determines which component is selected.
3. The adapter extracts `name`, `id`, `guid` from the selected component.
4. The detail area is re-rendered with `inspector_details(name, id, guid)` — either by:
   - Spawning a new MMS `inspector_details` subtree via `build_panel_component_expr`/`spawn_tree`
   - Updating Text/TextInput values via `SetText` intents (for the non-editable fields)

Option A (simpler for v1): the adapter spawns `inspector_details` via MMS with the current
values as CE arguments, destroying and re-spawning on selection change (same pattern as
`rerender_single_inspector_panel_content`).

Option B (more efficient): a stable MMS subtree with TextInput/Text nodes that the adapter
updates in place via `SetText` intents when selection changes.

Start with Option A. Move to Option B if respawn flickers.

## Future fields

The intention is that `inspector_details()` will eventually accept a richer schema —
possibly a list of field descriptors — but for now the fixed three-field schema
(name, id, guid) is sufficient.

Later additions:
- Transform fields (position, rotation, scale) with numeric TextInput
- Component-specific fields from authored MMS metadata
- Color swatches, dropdowns, toggles

## Acceptance criteria

- `assets/components/inspector_details.mms` exists with `inspector_details(name, id, guid)`
  rendering a 2-column form.
- `inspector_panel` in `panels.mms` has a sidebar + detail layout.
- Selecting a row in the sidebar shows that component's details.
- Name is editable via TextInput. ID and GUID are display-only Text.
- The inspector panel is wide enough (~2×) to show both columns without horizontal overflow.
- Existing inspector panel behavior (row list from ECS tree, pin button, title bar) is preserved
  in the sidebar.
- The Y-axis shift from `LayoutRootSizeAvailable` still works correctly.

## Relevant files

- `assets/components/panels.mms` — `inspector_panel()` factory
- `assets/components/panel_items.mms` — `inspector_panel_content()`, `inspector_panel_row()`
- `assets/components/inspector_details.mms` — new file
- `src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs` — Rust-side adapter
- `src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:58` — `INSPECTOR_PANEL_WIDTH_GU`

## Current regression note

The live repro now looks like this:

- pinning works as described
- the pin button does not fit inside the title bar and wraps down a line
- the sidebar text is oversized even with the detail view disabled
- the sidebar rows overlap vertically and become unreadable

That makes the sidebar text-size/layout path the immediate target before reintroducing the detail
pane.

## Related

- [`docs/task/editor-workspace-width-from-post-layout-bounds.md`](./editor-workspace-width-from-post-layout-bounds.md)
- [`docs/task/layout-root-computed-size-and-shift-event.md`](./layout-root-computed-size-and-shift-event.md)
