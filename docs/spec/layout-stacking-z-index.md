# Layout Stacking and `z_index` for Styled Nodes

Status: **Implemented**.

This document defines the layout-owned stacking model for styled layout items so UI
authors do not need to hand-author tiny local Z offsets like
`T.position(0, 0, -0.05)` just to get backgrounds and successive rows to layer
correctly.

It is intentionally a **subset** of CSS stacking behavior. The goal is to make
layout-managed panels and editor UI predictable first, without taking on the
entire browser stacking-context model.

## Problem

Today the engine has three partial layering mechanisms that do not form one
coherent model:

1. Styled nodes can carry `Style.z_index`, but layout does not yet use it as a
   general stacking primitive.
2. Generated layout backgrounds use `Style.background_z`, which is a special
   case for one helper node rather than a general rule.
3. Authors can still place local Z offsets on nested `TransformComponent`s,
   which is expressive but ad hoc and easy to abuse.

That leaves authors doing manual Z nudges to solve problems that should belong
to layout itself:

- background behind content within one styled box
- successive styled rows layering deterministically relative to each other
- text and helper content staying in front of layout-generated background quads

## Design goals

1. Give every layout-managed styled item a deterministic integer layer index.
2. Make successive styled siblings stack at a fixed stride.
3. Make the layout-generated background sit behind the item's content by
   default, without requiring authored Z offsets.
4. Keep `Style.background_z` as an optional escape hatch, but make it
   unnecessary for normal UI authoring.
5. Keep the model simple enough to explain and debug.

## Proposed stacking primitive

The layout stacking primitive is an **integer layer index**.

- Each layout-managed styled item resolves to one integer `layer_index`.
- The default stride between successive layout layers is `1`.
- `Style.z_index` is treated as an **absolute** layer index within the current
  layout context.
- Items without an explicit `Style.z_index` are assigned by layout from an
  internal per-context layer counter.

In other words:

- the layout engine keeps an integer counter for successive styled items
- the counter normally advances by `1`
- an explicit `Style.z_index` pins an item to a specific absolute layer in that
  layout context rather than offsetting relative to siblings

## Default layer spacing in Z

Layout converts the integer `layer_index` into a float local Z translation using
one layout-owned constant:

```text
resolved_z = layer_index * LAYER_DISTANCE
```

Where:

- `layer_index` is an integer
- `LAYER_DISTANCE` is a layout-owned float constant

This draft does **not** fix the numeric value of `LAYER_DISTANCE` yet. The
important part is the ratio policy below.

## Background placement

The layout-generated background for a styled item is derived from the item's
resolved layer, not from an unrelated ad hoc constant.

Default behavior:

```text
background_z = resolved_z - 0.5 * LAYER_DISTANCE
```

That means:

- item content sits at the item's resolved layer
- generated background sits half a layer behind that content
- the gap from one item's content layer to the next item's content layer is
  exactly twice the background offset

This satisfies the intended relationship:

- successive styled layers are separated by one full layer step
- each background sits halfway behind its owning content layer

## `Style.background_z`

`Style.background_z` stays as an optional override.

Semantics under this draft:

- if `Style.background_z` is not set, layout derives background Z from the
  item's resolved layer using the default half-step rule
- if `Style.background_z` is set, that value overrides the derived background Z

This keeps the escape hatch for unusual authored cases, while making the normal
case implicit and deterministic.

## Absolute `z_index`

This draft chooses **absolute** `z_index` rather than relative `z_index`.

Example:

```text
item A: no z_index   -> assigned layer 0
item B: no z_index   -> assigned layer 1
item C: z_index = 5  -> resolved layer 5
item D: no z_index   -> assigned next automatic layer per context policy
```

The exact interaction between explicit absolute layers and the automatic layer
counter needs to be defined carefully. Two plausible policies:

1. Automatic layers ignore explicit layers and just consume the next counter
   value.
2. Automatic layers advance past the highest resolved explicit layer seen so far
   in the current context.

This draft prefers **policy 2** because it keeps later automatic siblings from
accidentally collapsing "under" a manually elevated sibling.

## Layout context boundary

Layer indices are resolved within the current **layout context**.

For this draft, a layout context means the subtree governed by one
`LayoutComponent` root / layout pass.

That means:

- integer layers are meaningful within a given layout root
- nested layout roots get their own layer counter and their own absolute
  `z_index` namespace
- this draft does not attempt to define browser-style global stacking contexts
  across arbitrary nested positioned elements

This is a deliberate simplification.

## Author-visible behavior

### Normal case

Authors write:

```mms
T {
    Style {
        background_color = [0.9, 0.9, 0.9, 1.0]
    }
    T.position(0.0, 0.0, 0.015) {
        Text { "hello" }
    }
}
```

Layout assigns a layer to the styled item, places its generated background half
a layer behind it, and authors do not need to hand-author a small negative Z
offset for the background relationship.

### Explicit ordering

Authors can opt into an absolute layer when they need to force ordering among
styled siblings:

```mms
Style {
    z_index(10)
}
```

This is intended for layout-managed UI ordering, not arbitrary scene graph depth
authoring.

## Relationship to authored transform Z

This draft does **not** fully settle authored transform Z yet.

Open choices:

1. Layout ignores authored local Z for layout-managed styled items and uses only
   layout-owned layer resolution.
2. Layout preserves authored local Z and adds the resolved layer Z on top.
3. Layout forbids authored local Z on layout-managed styled items and reserves
   Z ownership fully to the layout system.

For a CSS-like model, option 3 is the cleanest. For continuity with current
engine authoring patterns, option 2 may be easier in the short term.

This draft does not lock that decision yet.

## Relationship to CSS

This model intentionally only partly matches browser semantics.

Matches CSS spirit:

- integer `z-index`
- default sibling order determined by layout / document order
- explicit author override available on styled items

Does **not** yet match full CSS:

- no full stacking-context tree
- no distinction yet between positioned / non-positioned `z-index` behavior
- no paint-order phases like browser background/border/inline content/outline
- no negative-vs-auto browser edge-case semantics

That is acceptable for this draft. The goal is a predictable engine-local model,
not immediate browser parity.

## Suggested implementation shape

Layout should compute three related values for each styled item:

1. `layer_index: i32`
2. `resolved_z: f32 = layer_index as f32 * LAYER_DISTANCE`
3. `resolved_background_z: f32`

Where:

```text
resolved_background_z =
    Style.background_z.unwrap_or(resolved_z - 0.5 * LAYER_DISTANCE)
```

Then:

- the item's layout-owned transform uses `resolved_z`
- the generated `__bg` helper uses `resolved_background_z`
- content helper nodes that conceptually belong "with" the item stay on the
  item's resolved content layer unless explicitly specified otherwise

## Open questions

1. Should the automatic layer counter advance by layout item order only, or by
   paint order after inline/block dispatch?
2. Should `z_index` exist only on styled layout items, or also affect generated
   helper nodes like scroll wrappers and clip helpers?
3. Should authored transform Z be preserved, composed, or ignored for
   layout-managed items?
4. If a child layout root sits under a parent item at layer 10, should the
   child context inherit a base resolved Z from the parent before its own local
   layering begins?
5. Once this model lands, is `background_z` still needed often enough to justify
   keeping it, or can it be deprecated after migration?

## Acceptance criteria

1. Two successive styled siblings render on distinct deterministic layers even
   when the author does not hand-author local Z offsets.
2. Each styled item's generated background renders behind its own content and in
   front of the previous sibling's background by default.
3. Setting `Style.z_index` on one styled sibling predictably moves that item to
   the requested absolute layer within the layout context.
4. Normal UI authoring no longer needs `T.position(0, 0, small_z)` just to keep
   text or content above generated layout backgrounds.
5. `Style.background_z` can be omitted in the common case without changing the
   visual result.

rawr