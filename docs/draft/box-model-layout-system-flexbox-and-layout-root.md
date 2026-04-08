---

# Box Model, Layout System & Flexbox ( ˘ω˘ )

## Motivation

Panels need:
1. Row heights that adapt when text wraps (currently hardcoded, causing overlap)
2. Title bars that can be dragged to move the panel (gizmo-style)
3. Clean side-by-side initial positioning without manual arithmetic
4. A foundation that scales toward browser rendering

The fix is a CSS-derived layout system built on two components that always
go together:

- **`HtmlElementComponent`** — element type (div, span, body, …); implies
  default display and layout role; the structural/semantic half
- **`StyleComponent`** — CSS property bundle; the visual/layout half
- **`LayoutComponent`** — marks the root of an independent layout context
  (the "viewport" of that subtree)

The layout unit is the **glyph unit** (1.0 = one monospace character cell).
World-space scaling stays in `TransformComponent`. Same split as browsers
(CSS px vs device px).

---

## 1. Box model

```
┌─────────────────────────────── margin ──────────────────────────────────┐
│  ┌──────────────────────────── border ────────────────────────────────┐  │
│  │  ┌───────────────────────── padding ─────────────────────────────┐ │  │
│  │  │                      content box                              │ │  │
│  │  └───────────────────────────────────────────────────────────────┘ │  │
│  └────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

All dimensions in glyph units. Border is zero-width in phase 1 (visual only).

---

## 2. `HtmlElementComponent` — element type

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElementType {
    // Block-level (default display: block)
    Div, P, H1, H2, H3, H4, H5, H6,
    Article, Section, Header, Footer, Main, Nav, Aside,

    // Inline (default display: inline)
    Span, A, Strong, Em, Code,

    // Special
    /// Block + also acts as a LayoutComponent root content node.
    Body,
    /// Replaced element; intrinsic size from asset.
    Img,

    // Table (phase 2)
    Table, Thead, Tbody, Tr, Th, Td,

    // Form (phase 3)
    Input, Button, Textarea, Select,

    // Generic — no implied display; StyleComponent must set display explicitly.
    #[default]
    Element,
}

pub struct HtmlElementComponent {
    pub element_type: ElementType,
}
```

Element type determines the **UA-stylesheet default** for any style
property not set in `StyleComponent`. `Div` → block, `Span` → inline,
`Body` → block, `Table` → table, etc.

---

## 3. `StyleComponent` — CSS property bundle

CSS properties live here rather than as individual ECS components. This
keeps authoring compact and mirrors how browsers treat computed style as a
single record per element, not dozens of separate objects.

```rust
#[derive(Debug, Clone)]
pub struct StyleComponent {
    // ── Display ──────────────────────────────────────────────────────
    /// None = inherit from HtmlElementComponent default.
    pub display: Option<Display>,

    // ── Sizing ───────────────────────────────────────────────────────
    pub width:      SizeDimension,   // default: Auto
    pub height:     SizeDimension,   // default: Auto
    pub min_width:  Option<f32>,     // glyph units
    pub max_width:  Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,

    // ── Box model ────────────────────────────────────────────────────
    pub margin:  EdgeInsets,   // default: all 0
    pub padding: EdgeInsets,   // default: all 0
    // border: EdgeInsets (phase 2 — visual only)

    // ── Flex container ───────────────────────────────────────────────
    pub flex_direction:   FlexDirection,   // default: Row
    pub justify_content:  JustifyContent,  // default: FlexStart
    pub align_items:      AlignItems,      // default: Stretch
    pub flex_wrap:        FlexWrap,        // default: NoWrap
    pub row_gap:          f32,             // default: 0
    pub column_gap:       f32,             // default: 0

    // ── Flex item ────────────────────────────────────────────────────
    pub flex_grow:   f32,            // default: 0.0
    pub flex_shrink: f32,            // default: 1.0
    pub flex_basis:  SizeDimension,  // default: Auto

    // ── Position ─────────────────────────────────────────────────────
    pub position: Position,          // default: Static
    pub top:    Option<SizeDimension>,
    pub right:  Option<SizeDimension>,
    pub bottom: Option<SizeDimension>,
    pub left:   Option<SizeDimension>,

    // ── Text / typography ────────────────────────────────────────────
    pub line_height: f32,   // default: 1.0 (glyph units)
    // font_size, text_align: phase 2

    // ── Overflow ─────────────────────────────────────────────────────
    pub overflow: Overflow, // default: Visible

    // ── Stacking ─────────────────────────────────────────────────────
    pub z_index: Option<i32>,
}
```

### Supporting enums

```rust
pub enum Display { Block, Inline, InlineBlock, Flex, None }

pub enum Position { Static, Relative, Absolute, Fixed }

pub struct EdgeInsets { pub top: f32, pub right: f32,
                        pub bottom: f32, pub left: f32 }

pub enum SizeDimension {
    Auto,
    GlyphUnits(f32),
    Percent(f32),
}

pub enum FlexDirection   { Row, Column, RowReverse, ColumnReverse }
pub enum JustifyContent  { FlexStart, FlexEnd, Center, SpaceBetween, SpaceAround, SpaceEvenly }
pub enum AlignItems      { Stretch, FlexStart, FlexEnd, Center, Baseline }
pub enum FlexWrap        { NoWrap, Wrap, WrapReverse }
pub enum Overflow        { Visible, Hidden, Scroll, Auto }
```

### Individual property components?

Separate `DisplayComponent`, `WidthComponent`, `FlexDirectionComponent`,
etc. are **not** the primary API — they don't exist in the layout system.
`StyleComponent` is the single record the layout system reads, mirroring
how browsers use computed style.

Where individual property components make sense is for **reactive mutation**
(e.g., a `TransitionComponent` animating `width` over time, or an intent
that updates just `style.display`). These are implemented as targeted
`UpdateStyle` intents rather than separate component types:

```rust
// Intent variant — updates one or more fields on an existing StyleComponent.
IntentValue::UpdateStyle {
    component_ids: vec![element_id],
    patch: StylePatch,   // enum or struct with Option fields
}
```

`TextBackgroundComponent.padding_*` fields are subsumed by
`StyleComponent.padding` once this lands. The old fields become defaults
for nodes without a `StyleComponent`.

---

## 4. `LayoutComponent` — layout viewport

`LayoutComponent` is the **viewport analog** — the initial containing
block of a self-contained layout subtree. It does not itself participate
in flow; it is the space-definer. The first `HtmlElementComponent` child
flows inside it.

```rust
pub struct LayoutComponent {
    /// Available inline width for children, in glyph units.
    pub available_width: f32,
    /// Optional block-axis constraint (clip/scroll later).
    pub available_height: Option<f32>,
    pub(crate) dirty: bool,
}
```

Multiple `LayoutComponent` nodes coexist — one per panel, one per HUD
region, one per browser tab.

A panel's `available_width` comes from either:
- An explicit `StyleComponent.width` on the panel element
- The workspace flex layout assigning it (for in-flow panels)
- Nothing (panel is floating/absolute: uses its own `StyleComponent.width`)

---

## 5. Style resolution order

For any given CSS property on a layout node:

1. `StyleComponent` value (if not the type default)
2. `HtmlElementComponent.element_type` implied default
3. Layout system built-in fallback (block → fill width, etc.)

This mirrors CSS specificity: inline style > UA stylesheet.

---

## 6. Layout algorithm — two-pass

Width constraints flow **down**; computed heights flow **up**.

```
LayoutComponent { available_width: W }
  │  ← W flows down
  ▼
HtmlElementComponent::Body  StyleComponent { display: Block }
  used_width = W
  │  ← (W - padding) flows down to children
  ▼
HtmlElementComponent::Div   StyleComponent { display: Block, height: Auto }
  TextComponent "long label…"
    wrap_at = floor(available_width)           // width used here
    line_count = TextSystem::measure(…)       // measure step
    used_height = line_count * line_height     // height flows UP
  ▲
  │  used_height propagates up through ancestors
```

### Phase 1 — Measure (bottom-up)

```
measure(node, available_width) -> (used_width, used_height)
```

- **Block**: `used_width = available_width` (or explicit); recurse into
  children with `used_width - padding_h`; stack heights + margin collapse.
- **Flex column**: assign flex-basis per child, distribute free space by
  flex-grow/shrink, sum block-axis sizes.
- **Text leaf**: compute `wrap_at`, call `TextSystem::measure`, return
  `(max_col, line_count * line_height)`.
- **Absolute/Fixed nodes**: measured independently (their own available
  width from the nearest `LayoutComponent`), do not contribute to parent
  height.

### Phase 2 — Layout (top-down)

Emit `UpdateTransform` for each node with the resolved content-box origin
relative to the parent content-box origin, in glyph units.

---

## 7. Panel architecture

### Per-panel topology

```
SelectableComponent::off()
  OverlayComponent
    LayoutComponent { available_width: PANEL_WIDTH }
      HtmlElementComponent::Body
        StyleComponent { display: Flex, flex_direction: Column }

        HtmlElementComponent::Header    ← title bar
          StyleComponent { display: Block, height: GlyphUnits(1.5) }
          TextComponent "World Panel"
          RaycastableComponent::drag_only()  ← handles panel drag
          RaycastableShapeComponent::Quad2D

        HtmlElementComponent::Div       ← content area
          StyleComponent { display: Block, flex_grow: 1.0 }
          ScrollingComponent
          drag_plane (RaycastableComponent::drag_only())
          [row items — HtmlElementComponent::Div, display: Block]

    TransformComponent  ← panel anchor; moved by title-bar drag
```

### Title bar drag — gizmo-style plane projection

The title bar behaves exactly like a gizmo translate handle. The key is
that it uses `StartPlaneProjection` (already the default for desktop
pointers) so dragging continues even when the cursor leaves the title bar.

On `DragStart` (title bar):
1. Capture the panel anchor's current world position.
2. The standard `StartPlaneProjection` captures a plane at the hit point,
   normal facing the camera.

On `DragMove` (title bar):
1. `delta_world` gives movement on the projection plane.
2. Handler emits `UpdateTransform` on the panel anchor:
   `translation = captured_pos + accumulated_delta`.
3. Set `StyleComponent.position = Position::Absolute` on the panel's
   `HtmlElementComponent::Body` node (removes it from workspace flow).

Once `position: Absolute`, the `LayoutSystem` skips this panel in any
workspace-level flex pass — it is fully free-floating. The panel's own
internal `LayoutComponent` still runs normally (row heights still adapt,
text still wraps).

The panel stays at `Position::Absolute` for the rest of its life (or until
re-docked, which is phase 3).

---

## 8. Workspace layout

The workspace positions in-flow panels side-by-side:

```
WorkspaceComponent
  OverlayComponent
    LayoutComponent { available_width: VIEWPORT_WIDTH }
      HtmlElementComponent::Body
        StyleComponent { display: Flex, flex_direction: Row, gap: 0.12 }

        [world panel Body — flex item, StyleComponent { width: GlyphUnits(PANEL_W) }]
        [inspector panel Body — flex item]
        [assets panel — flex item]
```

When a panel is grabbed via title bar:
- Its `StyleComponent.position` becomes `Absolute`
- It is skipped in the flex pass
- Remaining in-flow panels do not reflow (panels are fixed-width, not `flex-grow`),
  so the visual gap left behind is intentional (it's overlay space, not a document)

Until the workspace `LayoutComponent` is implemented, current arithmetic
placement (`estimate_panel_width` + `PANEL_GAP`) is correct and does not
need migration.

---

## 9. `TextComponent` integration

Once a `TextComponent` is inside a `LayoutComponent` subtree:

1. `LayoutSystem` assigns `available_width` (glyph units)
2. `wrap_at_cols = floor(available_width)`
3. `TextSystem::measure(text, wrap_at_cols, …)` → `(max_col, line_count)`
4. Write `wrap_at` to `TextComponent` if changed (marks unbuilt)
5. Return `used_height = line_count * style.line_height`

`TextSystem::measure` must be a stateless pure function:

```rust
pub fn measure(
    text: &str,
    wrap_at_cols: usize,
    word_wrap: bool,
    word_wrap_tokens: &[String],
) -> (usize, usize)  // (max_col, line_count)
```

---

## 10. Known issue — TextSystem drops `PointerEvents` on glyph spawn

`TextSystem::spawn_glyph_quad` copies only `enable: bool` from the
TextComponent's child `RaycastableComponent`, always spawning
`RaycastableComponent::new(enable)` (`PointerEvents::All`) per glyph.

World panel rows attach `RaycastableComponent::click_only()`, but after
text build each glyph ends up `All` — meaning glyphs still capture drag
events and the drag plane behind them may be shadowed.

Fix: read full `RaycastableComponent` (including `pointer_events`) in
`register_text` and pass it through to `spawn_glyph_quad`.

---

## 11. New components summary

| Component              | Phase | Notes                                              |
|------------------------|-------|----------------------------------------------------|
| `HtmlElementComponent` | 1     | element_type; implies UA-default display           |
| `StyleComponent`       | 1     | all CSS layout properties in one struct            |
| `LayoutComponent`      | 1     | layout viewport; available_width + dirty flag      |
| `UpdateStyle` intent   | 1     | patch individual fields on an existing StyleComponent |

Legacy helper components (`DisplayComponent`, `WidthComponent`, etc.)
from earlier drafts are **not** introduced — `StyleComponent` subsumes them.

---

## 12. Scroll containers — `overflow: scroll`

`overflow: scroll` (or `auto`) on a `StyleComponent` makes a node a **scroll
container**. This replaces `ScrollingComponent` for all UI scrolling needs.

### Layout behaviour

**Measure pass**: children are measured with an unconstrained block axis
(Y-axis for a vertical scroll container). The container's used height is
`min(content_height, available_height)`. The excess
(`content_height - available_height`) is the scrollable range.

**Position pass**: when positioning children, the layout system subtracts
`layout_state.scroll_offset` from each child's block-axis origin:

```
child_y = padding_top + flow_offset_y - scroll_offset
```

All children are always present as ECS nodes — no virtual windowing.
Visibility is handled by a two-layer approach:

- **CPU culling (layout position pass)**: children whose computed position
  falls entirely outside `[-item_h, container_h + item_h]` are skipped —
  no `UpdateTransform` emitted, renderable disabled. This avoids GPU draw
  calls for items far off-screen.
- **GPU scissor (renderer)**: the scroll container registers a scissor rect
  with the renderer covering its content box. Items near the edge that
  straddle the boundary are rendered but clipped, giving a crisp cutoff.

The render zone is therefore `visible items + 1 item buffer on each side`.
Items beyond the buffer are CPU-culled; items in the buffer are scissored.

### Scroll state

`scroll_offset` lives in the layout system's per-node computed state, NOT in
`LayoutComponent` directly (LayoutComponent is a viewport root marker, not a
per-node record). The layout system maintains a side table keyed by node ID.

```rust
// Internal layout state (not an ECS component)
struct NodeLayoutState {
    used_size: [f32; 2],
    content_size: [f32; 2],
    scroll_offset: f32,   // only set on overflow:scroll nodes
}
```

### Gesture integration

The layout system registers a `DragMove` handler on every `overflow:scroll`
node (detected during layout). On drag:

```rust
state.scroll_offset -= delta_y;   // direct world-unit accumulation
state.scroll_offset = state.scroll_offset.clamp(
    0.0,
    (state.content_size[1] - state.used_size[1]).max(0.0),
);
// Mark position pass dirty — measure is skipped (sizes unchanged)
layout_root.dirty_position_only = true;
```

No `fract()`, no sawtooth, no window boundary logic. `scroll_offset` is a
direct world-space offset applied to child positions.

### Migration from ScrollingComponent

`ScrollingComponent` stays for the editor panels until LayoutSystem is wired.
Once `overflow:scroll` is working:

- Remove `ScrollingComponent`, `wsc_id`, `isc_id` from panel setup
- Remove `DragMove` scroll handlers from `InspectorSystem`
- Remove `rows_anchor_base_pos`, virtual-window rebuild logic
- Remove `ScrollChanged` event (or keep for external triggers)
- All rows become persistent children; layout positions and clips them

See `docs/draft/rendering-pipeline.md` for the full pipeline context.

---

## 13. Open questions

- **Glyph unit vs world unit**: `LayoutComponent.available_width` is in
  glyph units. A future `scale` field could map to world units for
  mixed geometry + text layouts.

- **Proportional fonts**: `TextSystem::measure` stays the same API; needs
  per-glyph advance widths when non-monospace fonts are added.

- **Inline formatting context**: multi-span text, inline elements — deferred
  until panels need rich text.

- **`wrap_at` coexistence**: nodes inside a `LayoutComponent` have `wrap_at`
  owned by the layout system. Manual `wrap_at` stays for nodes outside.

- **Re-docking panels**: dragged panels are permanently `Absolute` for now.
  Phase 3: drag-to-snap back into workspace flex flow.

- **`StyleComponent` vs transition / animation**: `TransitionComponent` needs
  to animate individual style fields. The `UpdateStyle` intent patch
  mechanism is the hook; the transition system emits it each frame.
