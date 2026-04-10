# Retiring TextBackgroundComponent → Style.background_color
## (o´ω`o) making padding do what padding is supposed to do

---

## 1. Problem Statement

`TextBackgroundComponent` is a rendering primitive that lives in `TextSystem`. It spawns
a background quad sized from **text metrics** (measured cols × rows) plus its own padding
concept. This is entirely separate from `StyleComponent.padding`, which LayoutSystem reads
for box model layout.

Two different padding systems pulling in different directions:

```
TextSystem world:
  TextComponent
    TextBackgroundComponent { padding_top=0.35, padding_bottom=0.125 }
      ColorComponent          // background color
    (spawned) Transform { scale=(cols+pad_h, rows+pad_v) }  // sized from TEXT, not layout box
      ColorComponent → Renderable { square } → Opacity

Layout world:
  Transform { name="row_t" }
    Style { height=auto, padding.bottom=0 }   // layout padding — TextSystem never reads this
    Text { ... }
      TextBackground { ... }                  // rendering padding — LayoutSystem never reads this
```

Neither system knows about the other's padding. The consequence is that the visual
background overflows the layout box (or leaves gaps), and the layout cursor doesn't account
for visual background size at all.

### Why this is wrong

In CSS, `padding` on a box element is a SINGLE concept that:
1. Increases the layout box size (`box = content + padding`)
2. Is painted as part of the element's background
3. Is the same value in both the layout pass and the render pass

`TextBackgroundComponent` violates this by owning a parallel padding system that only
affects rendering. `StyleComponent.padding` owns layout-only padding that nothing renders.

---

## 2. Target Design

### 2.1 StyleComponent gains background_color

```
Style {
  height: auto,
  padding: [0.1, 0.3, 0.1, 0.3],        // top right bottom left — gu — SINGLE source
  margin-left: 1.5gu,
  background_color: [0.92, 0.92, 0.92, 0.80],   // Vec4 rgba; None = no background
  background_z: -0.1,                    // z offset behind content in glyph units
}
```

`StyleComponent.padding` is the single padding value. It affects:
- `measure_item` box height/width (already does this ✓)
- The background quad size (new — LayoutSystem drives this)

`background_color` is `Option<[f32; 4]>` (rgba). When `Some`, LayoutSystem creates and
manages a background quad TC as a child of the layout item TC. When `None`, no quad exists.

MMS value: `background_color=[r, g, b, a]` — plain float array, no component reference needed.

### 2.2 background quad lifecycle owned by LayoutSystem

After placing each TC in the block layout pass, LayoutSystem checks `Style.background_color`:

```
if background_color is Some(rgba):
  look up or create a child TC named "__bg"
  set its transform: pos=(content_x - padding_left, content_y + padding_top, background_z)
                     scale=(box_width_gu * unit_scale, box_height_gu * unit_scale, 1.0)
  set its Color to rgba (alpha < 1.0 → also set Opacity)
  set its Renderable to square
if background_color is None:
  if a "__bg" child exists: emit RemoveSubtree for it
```

The background quad covers the **padding box** (content + padding on all sides), which
is exactly `box_width × box_height` — the same quantities LayoutSystem already computes
in `measure_item`. No separate measurement needed.

### 2.3 background_color vs ColorComponent — the distinction

```
Color { rgba=[r,g,b,a] }         // explicit: goes on a Renderable you authored
Style { background_color=[...] } // implicit: LayoutSystem creates the Renderable for you
```

This mirrors CSS: `color` is for text/foreground, `background-color` creates an implicit
background layer. The implicit Renderable from `background_color` gets its own internal
`ColorComponent` managed by LayoutSystem — never collides with user-authored `Color {}` nodes.

Having both is fine. `Color {}` on an explicit mesh, `background_color` in `Style {}` on
a layout container. They serve different scopes.

### 2.4 Clickability

Currently `RaycastableComponent` is a child of `TextComponent` (the text node itself).
With the new design, the **background quad** is the natural click surface — it covers the
full padding box and is always behind the text. Two options:

**Option A** (simplest): add `Style { pointer_events: click | drag | none }`.
When `pointer_events != none`, LayoutSystem attaches a `RaycastableComponent` to the
background quad. This replaces the explicit `RaycastableComponent::click_only()` child
of TextComponent.

**Option B**: keep explicit `RaycastableComponent` children as today, but author them on
the row TC (or its Style-driven background) instead of directly on TextComponent.

Option A is cleaner long-term; Option B is zero-cost interim. Plan for A but ship B first.

### 2.5 Where to store the background quad TC id

LayoutSystem needs to find/reuse the background quad TC across layout passes (so it can
update its transform rather than spawning a new one each tick).

Stored in `LayoutComponent.bg_quads: HashMap<ComponentId, ComponentId>`:
- key: the layout item TC id
- value: the background quad TC id

On layout pass: if `tc_id` is in `bg_quads`, update the existing quad's transform via
`UpdateTransform`. If not (or if the quad was removed), spawn a new one and record it.
If `background_color` becomes `None`, remove from map and emit `RemoveSubtree`.

This keeps runtime state in `LayoutComponent` (already the layout root's state holder)
and out of `StyleComponent` (which is pure config/data).

---

## 3. Migration Topology

### Before (current)

```
Transform { name="wp_row_0", scale=(0.08, 0.08, 0.08) }     // row TC
  Style { height=auto, margin-left=1.5gu }
  Color { rgba=[0,0,0,1] }                                   // text color
    Text { "Transform { name=catgirl }" }
      Emissive {}
      Raycastable { click_only }                             // click target on text node
      TextBackground { padding_top=0, padding_bottom=0 }    // ← RETIRING THIS
        Color { rgba=[0.92, 0.92, 0.92, 0.80] }
```

### After (target)

```
Transform { name="wp_row_0", scale=(0.08, 0.08, 0.08) }     // row TC
  Style {                                                    // single source of truth
    height=auto,
    margin-left=1.5gu,
    background_color=[0.92, 0.92, 0.92, 0.80],
    background_z=-0.1,
    pointer_events=click                                     // Option A; or keep explicit RC
  }
  Color { rgba=[0,0,0,1] }
    Text { "Transform { name=catgirl }" }
      Emissive {}
  // "__bg" quad spawned and managed by LayoutSystem — not authored
```

The `Color {}` on the text node stays — it's the text foreground color, not the background.
`background_color` in `Style {}` is the background layer. These don't conflict.

---

## 4. What Needs to Change in StyleComponent

```rust
pub struct StyleComponent {
    // ... existing fields ...
    pub background_color: Option<[f32; 4]>,  // None = no background quad
    pub background_z: f32,                   // default: -0.1 gu (behind glyphs)
    // future: pointer_events: PointerEvents
}
```

MMS decode additions:
- `background_color`: decode as `[f64; 4]` array → `[f32; 4]`
- `background_z`: decode as f64 → f32

---

## 5. What Needs to Change in LayoutComponent

```rust
pub struct LayoutComponent {
    // ... existing fields ...
    /// Maps layout item TC id → its managed background quad TC id.
    /// LayoutSystem maintains this as background_color is set/unset.
    pub(crate) bg_quads: HashMap<ComponentId, ComponentId>,
}
```

---

## 6. What Needs to Change in block.rs

After the existing UpdateTransform emit for each item, add a background quad pass:

```rust
// (pseudocode)
if let Some(rgba) = style.background_color {
    let bg_pos = [
        (item.margin_left_gu) * unit_scale,           // left edge of padding box
        -(cursor_before_margin + item.margin_top_gu) * unit_scale,  // top edge
        style.background_z * unit_scale,
    ];
    let bg_scale = [
        item.box_width_gu * unit_scale,
        item.box_height_gu * unit_scale,
        1.0,
    ];
    // create or update bg quad TC (looked up via layout_component.bg_quads)
    // emit UpdateTransform for it
    // emit color/opacity update if changed
}
```

The background quad is a sibling-child of the layout item TC's own children — it is
created as a child of the layout item TC by LayoutSystem, not by the author. It uses the
already-computed `box_width_gu` and `box_height_gu` from `measure_item`, so no extra
measurement pass is needed.

---

## 7. TextSystem Changes

Remove the `TextBackgroundComponent` detection block from `TextSystem::register_text`
(the `if let Some((bg_id, bg)) = background { ... }` block). TextSystem no longer owns
background quads.

---

## 8. Removal Checklist

- [ ] Add `background_color: Option<[f32; 4]>`, `background_z: f32` to `StyleComponent`
- [ ] Add `bg_quads: HashMap<ComponentId, ComponentId>` to `LayoutComponent`
- [ ] Implement background quad create/update/remove in `block::layout`
- [ ] Remove `TextBackgroundComponent` from `inspector_system.rs` (both `rebuild_*` fns and `panel_row_bg`)
- [ ] Remove `TextBackgroundComponent` background-spawn block from `text_system.rs`
- [ ] Delete `src/engine/ecs/component/text_background.rs`
- [ ] Remove from `src/engine/ecs/component/mod.rs` (pub use + re-export)
- [ ] Remove import from `inspector_system.rs`
- [ ] Replace `panel_row_bg` helper with `Style { background_color }` in row StyleComponent
- [ ] Move `PANEL_V_PADDING` from `TextBackgroundComponent.padding_bottom` to `Style { padding.bottom }` on the last row (so it enters the layout cursor and is painted by the background quad)
- [ ] (Optional) add `pointer_events` to StyleComponent and wire `RaycastableComponent` to background quad

## 9. Interim State (acceptable)

Steps 1–2 (add fields to Style/Layout) and step 8 (remove TextBackgroundComponent from
inspector callsites) can happen first, giving a brief period with no row backgrounds.
Steps 3–7 (implement the quad spawn in block.rs, remove from TextSystem) complete the migration.

The engine should compile and run without TextBackgroundComponent being used — its absence
just means no visual background on rows temporarily, which is fine.
