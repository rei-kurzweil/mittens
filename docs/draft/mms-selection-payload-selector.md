# Draft: MMS `Selection.payload_selector(...)`

## Status

Draft only.

This note describes a possible MMS authoring surface for selection payload
resolution.

It is a companion to:

- [`docs/draft/selection-changed-generic-payload.md`](./selection-changed-generic-payload.md)
- [`docs/task/selection-payload-query-for-editor-assets-and-paint.md`](../task/selection-payload-query-for-editor-assets-and-paint.md)

---

## 1. Purpose

The goal is to let a `Selection` scope define how to resolve a semantic payload
from the selected `Option`, without forcing downstream systems to walk option
subtrees manually.

The authored API should stay:

- generic
- scoped to the selected option root
- declarative rather than callback-driven for the first version

---

## 2. Proposed surface

Conceptually:

```mms
Selection.payload_selector("[name='option_value']")
```

Used inside a component tree:

```mms
T {
    Selection.payload_selector("[name='option_value']")

    option_a()
    option_b()
    option_c()
}
```

Meaning:

- when an option in this selection scope becomes selected
- evaluate the query string relative to that selected option root
- if a component matches, expose it as the selection payload

---

## 3. Resolution model

Given:

- selection scope `S`
- selected option root `O`
- payload selector `Q`

Resolution is:

```text
payload = find_component(root = O, selector = Q)
```

Important constraint:

- the query is scoped to the selected option root

This prevents selection payload selectors from acting like global lookups.

---

## 4. Why scope-relative queries are useful

They let one authored `Selection` work across many options that share one local
shape.

Example:

```text
Option(asset item)
├── preview_slot
├── label
└── option_value
```

Then every option can share:

```mms
Selection.payload_selector("[name='option_value']")
```

without any consumer needing to know the internals of `asset_item`.

---

## 5. Recommended selector style

Prefer stable semantic markers over presentational structure.

Good:

```mms
Selection.payload_selector("[name='option_value']")
Selection.payload_selector("[name='asset_payload']")
Selection.payload_selector("#option_value")
```

Fragile:

```mms
Selection.payload_selector("Transform > Transform > Transform")
Selection.payload_selector("[name='preview_slot'] > Transform")
```

The selector should describe meaning, not layout accident.

---

## 6. Why not a callback first?

Something like:

```mms
Selection.payload_at(fn(option) {
    return option.children[1].children[2]
})
```

is expressive, but weaker as a default authoring model because it is:

- harder to inspect
- more fragile if based on child indices
- less declarative
- more difficult to serialize and validate

Callbacks may still become useful later, but the query-string form is a much
better first surface.

---

## 7. Failure behavior

The authoring contract should define what happens when the selector does not
resolve cleanly.

Recommended behavior:

- zero matches: `selected_payload = None`
- one match: use it
- multiple matches: log a warning and resolve to `None`

That is stricter than "first match wins" and makes payload ambiguity visible.

---

## 8. Asset-panel example

Possible authoring shape:

```mms
export fn assets_content(items, item_background_color) {
    return T {
        name = "assets_content_area"
        id = "assets_content_area"
        Selection.payload_selector("[name='asset_payload']")
        Style {
            display("block")
        }
    }
}
```

And each asset item would contain a stable marker:

```text
Option(asset_item)
├── preview_slot
├── label
└── asset_payload
```

Then the asset selection event can expose the payload directly to the paint
system.

---

## 9. Tool-panel example

Paint tools may not need a rich payload. They may still benefit from the same
surface if tool identity should come from a marker instead of label text.

Example:

```mms
Selection.payload_selector("[name='tool_value']")
```

But if tool labels are already stable enough and there is no nested payload
node, this may be optional for tool selection.

---

## 10. Future extensions

If the selection-scope-level selector proves too coarse, possible later
extensions are:

- `Option.payload_selector(...)`
- explicit `OptionValue` helper component
- callback-based payload extraction for exceptional cases

Those should come later only if the shared selection-level query is too weak.

---

## 11. Recommendation

Start with:

- `Selection.payload_selector("...")`
- query evaluated relative to the selected option root
- stable semantic markers under options

That gives the engine a generic selection payload mechanism without introducing
panel-specific event variants or arbitrary callback logic.
