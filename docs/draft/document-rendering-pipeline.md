---

# Cat Engine *Document* Rendering Pipeline ( ˘ω˘ )

A browser-style breakdown of how a frame goes from component tree → pixels,
annotated with what exists today vs. what the layout system will own.

---

## Overview

```
                     ┌─────────────────────────────────────┐
                     │         Component Tree (ECS)         │
                     │  HtmlElementComponent                │
                     │  StyleComponent                      │
                     │  TextComponent / RenderableComponent │
                     │  TransformComponent                  │
                     └──────────────┬────────────────────── ┘
                                    │
                          ┌─────────▼──────────┐
                          │  Style Resolution   │
                          │                    │
                          │  element_type       │
                          │  → UA default       │
                          │  + StyleComponent   │
                          │  → computed style   │
                          └─────────┬──────────┘
                                    │
                          ┌─────────▼──────────┐
                          │   Layout System     │
                          │                    │
                          │  Phase 1: Measure  │ ← bottom-up
                          │  (text, box model, │
                          │   flex, scroll)    │
                          │                    │
                          │  Phase 2: Position │ ← top-down
                          │  → UpdateTransform │
                          └─────────┬──────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
          ┌─────────▼──────┐ ┌──────▼──────┐ ┌────▼────────────┐
          │   TextSystem   │ │  Renderable │ │  Light / Camera │
          │                │ │   System    │ │    System       │
          │  glyph quads   │ │             │ │                 │
          │  text rebuild  │ │  mesh inst. │ │  shadow maps    │
          └────────┬───────┘ └──────┬──────┘ └────┬────────────┘
                   │                │              │
                   └────────────────┼──────────────┘
                                    │
                          ┌─────────▼──────────┐
                          │    Visual World     │
                          │                    │
                          │  sorted batches    │
                          │  skin palettes     │
                          │  GPU upload        │
                          └─────────┬──────────┘
                                    │
                          ┌─────────▼──────────┐
                          │  Vulkano Renderer  │
                          │                    │
                          │  render phases     │
                          │  (6 ordered passes)│
                          └────────────────────┘
```

---

## Stage 1 — Component Tree

The source of truth. Every visible thing is a node:

| Component | Role |
|---|---|
| `HtmlElementComponent` | Structural/semantic type (div, span, body, …) |
| `StyleComponent` | CSS layout properties (display, flex, overflow, …) |
| `LayoutComponent` | Viewport root — defines available_width for its subtree |
| `TextComponent` | Text content; leaf in the layout tree |
| `RenderableComponent` | GPU geometry (mesh + material) |
| `TransformComponent` | World-space TRS; output of layout + user animation |

**Not yet**: `LayoutComponent` subtrees are currently only used for the editor
panels via manual arithmetic. StyleComponent and HtmlElementComponent are
implemented but not wired to LayoutSystem yet.

---

## Stage 2 — Style Resolution

For each node in a `LayoutComponent` subtree, the layout system resolves the
**computed style** used during layout:

```
computed.display = style.display
                   ?? element_type.default_display()
                   ?? Display::Block          // fallback

computed.overflow = style.overflow            // default: Visible
computed.position = style.position            // default: Static
... (all fields)
```

This mirrors the browser cascade: inline style > UA stylesheet > built-in default.

**Specificity order:**
1. `StyleComponent` field (if explicitly set — not the Rust `Default`)
2. `HtmlElementComponent.element_type` UA default (Div→Block, Span→Inline, …)
3. LayoutSystem fallback

No external CSS files, no cascade, no inheritance (phase 1). Inheritance of
`color`, `font-size`, etc. is a phase 3 concern.

---

## Stage 3 — Layout System (two-pass)

### Phase 1: Measure — bottom-up

```
LayoutComponent { available_width: W }
  │  W flows down
  ▼
HtmlElementComponent (Body / Div / …)
  ├─ Block: used_width = W or explicit; stack children vertically
  ├─ Flex:  distribute along main axis per flex-grow/shrink/basis
  ├─ Text leaf: TextSystem::measure(text, wrap_at) → (cols, lines)
  │             used_height = lines * line_height
  └─ Scroll container (overflow: scroll/auto):
       measure all children unconstrained → content_size
       used_height = min(content_size, available_height)
       record overflow_y = content_size.y - available_height
```

**Absolute/Fixed nodes** are measured independently (own containing block),
do not contribute to parent height.

### Phase 2: Position — top-down

Emits `UpdateTransform` for each node with its content-box origin, in glyph
units, relative to the parent content-box:

```
for each child of node:
    child_origin.x = padding_left + margin_left + flex_offset_x
    child_origin.y = padding_top  + margin_top  + flow_offset_y
    if node.overflow == Scroll:
        child_origin.y -= node.scroll_offset       // apply scroll
    emit UpdateTransform(child, child_origin)
    recurse
```

Scroll containers apply their `scroll_offset` during positioning, so children
are shifted without any virtual-windowing or rebuild. Visibility is enforced by
a two-layer strategy:

- **CPU cull** (position pass): children fully outside `[-item_h, container_h + item_h]`
  are skipped — no `UpdateTransform`, renderable disabled. No GPU work for
  items far off-screen.
- **GPU scissor** (renderer): the container registers a scissor rect for its
  content box. Items straddling the edge render into it and get clipped crisp.

The **render zone** is: visible items + 1-item buffer on each side.
Beyond the buffer → CPU-culled. Inside the buffer → scissored.

### Dirty propagation

```
LayoutComponent.dirty = true
    → full measure + position pass for that subtree

StylePatch applied (UpdateStyle intent)
    → mark nearest LayoutComponent ancestor dirty

TextComponent.wrap_at changed
    → mark LayoutComponent dirty (size may change)

Scroll gesture (DragMove on overflow:scroll node)
    → update layout_state.scroll_offset
    → position pass only (measure skipped — sizes unchanged)
```

---

## Stage 4 — Paint systems

After layout emits `UpdateTransform` intents, the paint systems run:

### TextSystem

- Detects `TextComponent` nodes with `is_built() = false` (wrap_at changed)
- Calls `TextSystem::measure()` (pure fn) to get line count
- Spawns/rebuilds glyph quad children
- Registers glyphs as renderables

### RenderableSystem

- Processes `RegisterRenderable` intents from component init
- Uploads mesh + material to `VisualWorld` batches
- Updates instance transforms when `UpdateTransform` fires

### Other systems

LightSystem, CameraSystem, TextureSysem — run after transforms are stable.

---

## Stage 5 — Visual World (sort + upload)

`VisualWorld` collects all registered renderables and organises them into
**render phase buckets**:

| Phase | Contents |
|---|---|
| Background | Sky, fullscreen quads (no depth write) |
| Background occluded+lit | World behind depth clear |
| Opaque instanced | Solid geometry (depth write) |
| Cutout | Alpha-tested (depth write) |
| Transparent single-layer | Overlay quads, UI panels |
| Transparent multi-layer | Back-to-front sorted glass / particles |

Phase assignment is driven by `OpacityComponent` + material flags, not
manual sorting.

---

## Stage 6 — Vulkano Renderer

Records one Vulkan render pass per phase, in order. No frame-to-frame
retained state — everything is rebuilt from `VisualWorld` each tick.

---

## Scroll containers vs. ScrollingComponent

### Current (ScrollingComponent — phase 0)

```
ScrollingComponent
  └── rows_anchor                 ← only page_size live children
        ├── row_0 (Transform+Text)
        ├── row_1
        └── ...

DragMove handler:
  apply_drag(dy) → scroll_offset (items)
  sub_y = fract(scroll_offset) * item_height
  rows_anchor.y = base + sub_y            // visual sub-row offset
  if window_crossed: rebuild rows (synchronous)  // content swap
```

Virtual windowing required: rows are destroyed/rebuilt as they scroll in/out.
Snap at window boundaries was a known bug; fixed by `base + sub_y` formula and
synchronous rebuild.

### Target (overflow:scroll — phase 2)

```
StyleComponent { overflow: Scroll, height: GlyphUnits(panel_h) }
  HtmlElementComponent::Div
    ├── row_0 (persistent child, positioned by layout)
    ├── row_1
    └── ...  (ALL items, always children)

DragMove handler (registered by LayoutSystem on overflow:scroll nodes):
  layout_state.scroll_offset += dy / item_h
  layout_state.scroll_offset.clamp(0, content_h - panel_h)
  mark position-pass dirty  // no measure needed
```

No virtual windowing. No rebuild. All rows are persistent children; layout
positions them, CPU-culls rows far off-screen, and GPU scissor gives crisp
edges. Performance optimization (virtual scroll for very long lists) is a
follow-up, opt-in.

**Migration path:**
`ScrollingComponent` stays for the editor panels until `LayoutSystem` is
fully wired. Once layout handles `overflow:scroll`, the panel's
`ScrollingComponent` + rebuild logic is removed.

---

## Layout nodes — internal model

During the layout pass, `LayoutSystem` builds an internal tree of **layout
nodes** from the component tree. This mirrors the browser's layout object tree:

```
Component tree (ECS)            Layout tree (internal, per-pass)
─────────────────────           ────────────────────────────────
LayoutComponent                 LayoutRoot { available_width }
  HtmlElementComponent::Body      BlockBox { computed_style, children }
    HtmlElementComponent::Div       FlexBox { direction, children }
      HtmlElementComponent::P         BlockBox { … }
        TextComponent                   TextBox { text, wrap_at, … }
```

**Styled layout node** — has both `HtmlElementComponent` and `StyleComponent`.
**Anonymous layout box** — generated by LayoutSystem when inline content
appears in a block context (phase 3, not needed for panels).

The layout tree is ephemeral — rebuilt each time the `LayoutComponent` is
dirty. It is not stored in ECS; it exists only during the layout pass.

---

## What's implemented today

| Stage | Status |
|---|---|
| Component tree | ✅ HtmlElementComponent, StyleComponent, LayoutComponent (data only) |
| Style resolution | 🔲 Not wired to LayoutSystem yet |
| Layout: measure | 🔲 Not implemented (TextSystem::measure stub needed) |
| Layout: position | 🔲 Not implemented |
| Layout: scroll container (position + CPU cull) | 🔲 Not implemented |
| Renderer: scissor rect per scroll container | 🔲 Not implemented |
| Dirty propagation | 🔲 Not implemented |
| TextSystem::measure (pure fn) | 🔲 Needed by layout measure pass |
| ScrollingComponent (interim) | ✅ Implemented; used by editor panels |
| Paint systems | ✅ TextSystem, RenderableSystem, LightSystem |
| Visual World + Renderer | ✅ All 6 render phases working |
