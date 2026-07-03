# Style Background — Scope and Authoring Convention

## What `background_color` does (and only does)

`background_color` in `StyleComponent` is a **LayoutSystem-managed** feature.
When set, LayoutSystem spawns a `__bg` child TC under the layout item TC, sizes it to
cover the full padding box, and colors it. Nothing else. It does not inspect, recolor, or
resize any other children of the item TC.

## Authoring convention

Two patterns; pick one per item TC, never mix:

### Pattern A — layout-managed background

```
item TC  (LayoutSystem child)
  Style { height=auto, background_color=[r,g,b,a] }
  Color { rgba=[text_color] }
    Text { "..." }
  [__bg TC]  ← spawned and managed by LayoutSystem
```

Use this for rows, list items, any repeating element where the background is uniform
and tied to the layout box. The author never touches `__bg`.

### Pattern B — authored background

```
item TC  (LayoutSystem child)
  Style { height=2gu }          ← no background_color
  bar_t (TC, explicit scale)
    Color { rgba=[bg_color] }
      R { QUAD_2D }
  label_t (TC, text scale)
    Color { rgba=[text_color] }
      Text { "..." }
```

Use this when the background has unusual geometry (extends beyond the item box, has a
specific z, needs its own color animation, etc.). LayoutSystem does not touch `bar_t`.
The author sizes and positions it manually.

The title bar (`header_slot` + `bar_t`) is Pattern B.

## Why not auto-detect and reconcile?

`bar_t` (background) and `label_t` (text) are structurally identical from LayoutSystem's
perspective — both are direct TC children with a `Color → Renderable` chain. There is no
reliable heuristic to tell them apart without an explicit marker. Adding that marker
(e.g. `background_tc: Option<ComponentId>` in Style) is extra API surface for a case
that authoring discipline already solves for free.

If a `background_color` item TC also happens to have an authored `Color → Renderable`
child, both render (double quad). That is an authoring error, not a system defect.
