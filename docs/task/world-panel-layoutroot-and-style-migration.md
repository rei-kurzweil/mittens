# World panel should be fully LayoutRoot/Style-driven

Date: 2026-05-14

This task captures the remaining layout migration work for the editor's World panel.

The target model is:

- panel UI is authored as layout items, not mostly hand-positioned transforms
- title bar children are styled transforms using `StyleComponent`
- title bar items flow with `display("inline-block")`
- panel width is determined by the panel layout contract (`available_width`) rather than by
  whichever title bar child happens to be visible
- the content list is a vertical block stack whose indentation is expressed through style
  margins rather than ad hoc transform x offsets

This task is about the Rust-built panel first. MMS-module panel factories are a later step.

Related broader task:
- [mms-asset-component-panels.md](mms-asset-component-panels.md)

---

## 1. Current state

The World panel already uses some of the right primitives, but not consistently.

### What is already layout/style-based

In `src/engine/ecs/system/inspector_system.rs`:

- `spawn_panel_title_bar(...)` creates a panel root with an inline-block `StyleComponent`
  carrying explicit width/height
- that root owns a `LayoutComponent` used as the panel-local layout root
- the title bar `header_slot` is a layout child with styled height/background
- the content area is a second layout child with styled height/background/overflow
- row entries in `rebuild_world_panel(...)` already use `StyleComponent` for height,
  background color, and indentation via `margin.left`

So the panel is not raw-transform UI anymore.

### What is still not layout-owned

The title bar internals are still mostly absolute-positioned transforms:

- title label placement is a hand-positioned `TransformComponent`
- Save / Load buttons are spawned by `spawn_titlebar_button(...)` as absolute transforms
  pinned to right-edge world coordinates
- button width and position are computed manually in world units instead of being children in a
  horizontal title-bar layout flow
- the status label above the panel is also a hand-positioned transform

This means the panel shell is layout-based, but several title bar children are still using the
old “place by transform math” model.

---

## 2. Symptom to fix

The immediate regression is that the World panel appears approximately as narrow as one of the
new Save / Load buttons, when it should be several times wider.

Desired behavior:

- the panel should keep a substantial fixed width, roughly on the order of five title-bar
  buttons rather than one
- the title bar children should lay out *within* that width instead of visually determining it
- panel-local `available_width` should be the source of truth for content width

Even if the root cause turns out to be a sizing bug elsewhere in LayoutSystem, the current title
bar topology makes the width contract harder to reason about because the buttons are positioned
outside the layout flow.

---

## 3. Desired structure

### 3.1 Panel shell

The panel should remain a style-driven inline-block item under the editor layout root.

High-level shape:

```text
panel_t
  Style(display=inline-block, width=..., height=...)
  LayoutRoot / LayoutComponent(available_width=...)
    title_bar_t
      Style(display=block, ...)
      title_row_t
        Style(display=block or implicit block, ...)
        title_label_t
          Style(display=inline-block, ...)
        spacer/fill item
        save_btn_t
          Style(display=inline-block, ...)
        load_btn_t
          Style(display=inline-block, ...)
    content_t
      Style(display=block, overflow=scroll, ...)
      rows_track
        rows_layout_root
          row_0_t
          row_1_t
          ...
```

### 3.2 Title bar items

Title bar children should be styled transforms:

```text
T { Style { display("inline-block") ... } ... }
```

For the Rust-built version, that means the equivalent ECS structure should be:

- transform root for each child item
- `StyleComponent` on that transform root
- child text/renderable content inside it

The key point is: **title bar children participate in layout**, not only in transform math.

### 3.3 Content list

The component list is naturally block layout:

- vertical list of rows
- each row is a block item
- indentation is expressed by `margin.left`

That part is already close to the desired model. The migration should preserve the current
margin-based indentation rather than introducing manual x-offset transforms.

---

## 4. Specific migration goals

### Goal A — stop absolute-positioning title bar controls

Replace `spawn_titlebar_button(...)`'s right-edge transform math with layout-owned title bar
items.

Current anti-pattern:

- `btn_center_x = right_edge_x - btn_w * 0.5`
- direct `TransformComponent::with_position(...)`

Target pattern:

- Save and Load buttons are children of a title-row layout container
- button roots carry `StyleComponent { display = inline-block, ... }`
- title row width is determined by the panel layout width

### Goal B — make panel width flow from panel layout config

The panel width should be owned by the panel shell:

- outer panel style width
- panel-local layout root available width
- content slot width expectations

These should agree by construction.

The current implementation already computes `panel_width_world` and converts it to glyph units,
but the title bar children do not make that contract obvious. The migration should make
`available_width` the readable source of truth.

### Goal C — keep row list style-driven

Do not regress the list body into transform-positioned rows.

The existing row shape in `rebuild_world_panel(...)` is largely correct:

- row transform root
- style child with `margin.left` for depth indentation
- text child + raycastable child

The migration should only tighten this if needed for consistency.

---

## 5. Suggested implementation order

### Stage 1 — title bar row container

Introduce a dedicated title-row child under `header_slot` whose job is to own title-bar flow.

That container should:

- span the full header width
- host label + save + load as layout children
- use block parent + inline-block children, or flex row if that proves easier

### Stage 2 — convert title label to styled transform item

Move the title label off of manual `TransformComponent::with_position(...)` placement and into a
styled transform child inside the title row.

### Stage 3 — convert Save / Load buttons to styled transform items

Replace `spawn_titlebar_button(...)`'s world-space placement math with title-row children that:

- carry `StyleComponent`
- use `display = inline-block`
- preserve the current visual appearance and click targets

### Stage 4 — verify content slot width and scroll body sizing

Once the title bar is layout-owned, re-check:

- panel width
- content slot width
- rows layout width
- scroll viewport behavior

---

## 6. Acceptance criteria

- World panel title bar items are styled transform children, not absolute-positioned controls
- Save and Load buttons use `display = inline-block`
- title label also participates in title-bar layout rather than being manually positioned
- panel width is visibly larger than a single button and remains stable when buttons are present
- content rows still lay out as a vertical list with indentation via style margins
- panel shell continues to work from Rust before any MMS factory migration happens

---

## 7. Non-goals

- moving the World panel entirely into MMS in this task
- redesigning the inspector panel at the same time unless code sharing falls out naturally
- changing save/load behavior in this task

---

## 8. Relevant code

- `src/engine/ecs/system/inspector_system.rs`
  - `spawn_panel_title_bar(...)`
  - `spawn_titlebar_button(...)`
  - `spawn_world_panel(...)`
  - `rebuild_world_panel(...)`
- `src/engine/ecs/component/world_panel.rs`
- `src/engine/ecs/component/style.rs`
- `src/engine/ecs/component/layout.rs`
