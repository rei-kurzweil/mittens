# Style.pointer_events (CSS-like) for layout boxes

## Motivation

Today the canonical way to make a `Style { … }` layout box clickable is to add
`Raycastable.enabled()` as an explicit author component on the same `T`.
`sync_bg_author_raycastable` (`src/engine/ecs/system/layout/block.rs`) then
grafts a Raycastable + `Quad2D` onto the layout-generated `__bg` renderable.

That works for the common case, but it conflates two concepts CSS keeps
separate:

- **Interactivity** — should this surface receive pointer events?
- **Existence of a Raycastable component** — engine-level opt-in.

CSS solves this with `pointer-events: auto | none | …`. Adding the same
property to `Style` would let authors say "this overlay is decorative" or
"this scrollbar track passes drag through" without removing the
`Raycastable` component (which other systems may also be reading from).

## Proposed surface

```mms
Style {
    display("inline-block")
    background_color = [...]
    pointer_events("auto")   // default — graft Raycastable when present
    pointer_events("none")   // suppress: skip graft even if Raycastable.enabled
    pointer_events("drag")   // graft as DragOnly
    pointer_events("click")  // graft as ClickOnly
}
```

Maps to `PointerEvents` (`src/engine/ecs/component/raycastable.rs:15-26`):
`All`, `DragOnly`, `ClickOnly`, `PassThrough`.

## Behavior

`sync_bg_author_raycastable` already reads the author's `RaycastableComponent`
verbatim. The change is to read `Style.pointer_events` and override the
grafted component's `pointer_events` (or skip grafting entirely when
`none` / `PassThrough`).

## Out of scope (for v1)

- CSS-style `inherit` cascade. Per-box only.
- Auto-inferring "clickable" from "has a Click handler attached" (would
  require querying the handler registry from inside the layout system —
  worth considering separately).

## Related

- Resolved bug `docs/bugs/inline-block-raycastable-no-clicks.md` — the
  graft infrastructure this property would build on.
- `src/engine/ecs/component/raycastable.rs:15-71` — `PointerEvents` already
  enumerates the variants this property exposes.
