# Inline-block items don't receive Click hits when authored as `T + Style` only

## Status

Open bug. Confirmed in padding-demo while building an MMS-side control bar.

## Symptom

A button authored as a layout item — i.e. a `T` whose child is a `Style`
component with `display: inline_block`, `background_color`, etc., plus a
`Raycastable.enabled()` sibling — renders correctly but **never fires
`Click` events**. The same `Raycastable.enabled()` placed inside an
`R.square() { ... }` child receives clicks normally.

Concrete reproduction (abridged from
`assets/components/button.mms`):

```mms
// DOES NOT receive Click:
T {
    Raycastable.enabled()
    Style {
        display("inline_block")
        padding_xy(2.0, 1.0)
        background_color = [0.30, 0.45, 0.90, 1.0]
    }
    T.position(0,0,0.05) { Text { "label" } }
}

// Works fine outside any LayoutRoot:
T {
    R.square() {
        C.rgba(0.30, 0.45, 0.90, 1.0)
        Raycastable.enabled()
    }
    T.position(0,0,0.05) { Text { "label" } }
}
```

## Likely cause

The layout system (`src/engine/ecs/system/layout/block.rs:475-489`)
spawns a `__bg` child under the styled `T` to render the
`background_color` quad — that quad is `RenderableComponent::square()`
positioned by the layout pass. The `Raycastable` authored on the
*outer* `T` doesn't live under that generated renderable, so when the
raycast system walks renderables looking for a paired
`RaycastableComponent`, the styled box has no raycastable surface.

The non-layout `R.square() { Raycastable.enabled() }` form puts the
raycastable **as a child of a renderable**, which is where the
raycast pipeline looks (cf. the scroll-drag pattern in
`sync_scroll_drag_surface` at `src/engine/ecs/system/layout/block.rs:419-471`
— that code explicitly grafts a Raycastable + RaycastableShape onto
the layout-generated background quad).

## Workaround in current code

`assets/components/button.mms` uses the explicit
`R.square() { … Raycastable.enabled() }` form. That means the button
can't participate as a layout-managed inline-block item — it has no
`Style` and no measurable layout size. The padding-demo control bar is
positioned manually as a result (see
`examples/padding-demo.mms`, "Control bar" section).

## Suggested fix paths

1. **Implicit raycastable on styled boxes.** When a `T` has a
   `RaycastableComponent` sibling and the layout spawns a `__bg` quad,
   auto-graft a `Raycastable` (+ `RaycastableShape::Quad2D`) onto the
   generated renderable — same shape as `sync_scroll_drag_surface`, but
   driven by the author-declared `Raycastable.enabled()` rather than
   scroll routing.

2. **Explicit `clickable` style flag.** Add `Style.clickable(true)`
   that triggers the same graft. Lets the layout system stay in
   control of when a raycastable surface is materialised.

3. **Support `Style` on `R.square()`.** Let an `R.square()` be a
   layout item with measurable `width`/`height`/`padding`/`margin`,
   keeping the Raycastable colocated with the renderable. This sounds
   simpler but it crosses the inline-block / renderable boundary in a
   way that probably wants more design.

Option (1) is the smallest change and matches the engine's existing
pattern for scroll-drag surfaces. (2) is a small extension on top.

## Related code

- `src/engine/ecs/system/layout/block.rs:475-489` — `spawn_bg_quad`
- `src/engine/ecs/system/layout/block.rs:419-471` — `sync_scroll_drag_surface` (the existing graft pattern)
- `assets/components/button.mms` — current workaround
- `examples/padding-demo.mms` — manually-positioned control bar
