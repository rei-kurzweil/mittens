# ScrollingComponent + LayoutSystem
## Refactor plan: scrolling is separate from clipping

---

## 1. Goal

Scrolling and clipping are related but **not the same concern**.

- `StyleComponent::overflow = Hidden | Scroll` expresses **viewport clipping**.
- Scrolling expresses **motion of content inside that clipped viewport**.

The mistake in the current panel code is that scrolling is still manually owned by
`InspectorSystem`, while clipping is becoming layout-owned via `StyleComponent` and
`LayoutSystem`.

The target architecture is:
- `StyleComponent::overflow` tells `LayoutSystem` whether the item is a clip viewport.
- `LayoutSystem` automatically maintains the helper topology needed for that viewport.
- A layout-owned `ScrollingComponent` owns scroll state for `overflow: Scroll`.
- `InspectorSystem` stops manually creating / driving scroll state.

---

## 2. Design rule

### `overflow` is clipping policy, not scroll state

`overflow: Hidden` and `overflow: Scroll` should both create the same clip viewport.
The difference is:

- `Hidden` = clipped, but no scrolling behavior
- `Scroll` = clipped, plus a scroll state / scroll track / gesture wiring

So clipping and scrolling should be modeled as two cooperating layers:

1. **clip layer** — already represented by `StencilClipComponent` on the auto-managed
   `__bg` clip geometry
2. **scroll layer** — `ScrollingComponent`, owning offset / range / track topology

This mirrors CSS:
- `overflow` determines whether content is clipped
- scrolling is the movement of the inner content box inside that clipped area

---

## 3. Current state

### What LayoutSystem already does

`layout/block.rs` already auto-manages the clip side:
- `sync_bg_quad(...)` creates the `__bg` helper quad
- `sync_stencil_clip(...)` attaches / removes `StencilClipComponent`
- `overflow: Hidden | Scroll` can therefore create a clip region without author code

### What InspectorSystem still does manually

`inspector_system.rs` still manually creates and drives `ScrollingComponent` for both
panels:
- `spawn_world_panel(...)` creates `world_panel_scroll`
- `spawn_inspector_panel(...)` creates `inspector_panel_scroll`
- `setup_panels_for_editor(...)` wires `DragMove` + `ScrollChanged` handlers directly
- panel rebuild logic still depends on virtual-window scrolling (`window_start`,
  `PAGE_SIZE`, row rebuild-on-scroll)

That is the coupling we want to remove first.

### Naming note

The naming decision is now:
- there should only be `ScrollingComponent`
- its implementation will change from the current virtual-window panel model to the
  layout-owned scroll helper model described here

That means the migration order matters:
1. remove `ScrollingComponent` from the places where it is currently used in the old way
2. then change `ScrollingComponent`'s implementation / ownership model

The key architecture point remains:
**LayoutSystem owns attachment/topology, not InspectorSystem.**

---

## 4. Target topology

When a `TransformComponent` has a child `StyleComponent { overflow: Scroll }`,
`LayoutSystem` should automatically ensure both:

1. clip helper topology
2. scroll helper topology

Conceptually:

```text
item_tc
  StyleComponent { overflow: Scroll, ... }
  __bg / clip root            ← auto-managed clip geometry / clip owner
    ColorComponent
    RenderableComponent
    StencilClipComponent
    __scroll                  ← auto-managed `ScrollingComponent` / state holder
      __scroll_track          ← auto-managed transform wrapping scrollable children
        child_0_tc
        child_1_tc
        ...
```

Important properties:
- the clip root (`__bg`) is the outer viewport boundary
- scrolling lives **inside** that clip boundary
- `__scroll` / `ScrollingComponent` handles scroll state / gesture registration / limits
- `__scroll_track` is what actually moves
- authored child transforms live under the scroll track, not directly under `item_tc`

So the nesting is:
- `StencilClip` / clip root outside
- scrolling helper inside that
- authored content inside the scroll track

This is analogous to the way `LayoutSystem` already auto-manages `__bg` today.

---

## 5. Migration strategy

### Phase 1 — remove manual panel scrolling ownership

First, stop `InspectorSystem` from being the owner of scrolling behavior.

This must happen **before** `ScrollingComponent` is reimplemented. Otherwise the same
component name would be serving two incompatible models at once.

Work:
- remove manual `ScrollingComponent` creation from panel spawn helpers
- remove manual `DragMove` / `ScrollChanged` wiring from `setup_panels_for_editor`
- remove panel-specific scroll bookkeeping (`wsc_id`, `isc_id`, `window_start` plumbing)
- verify the panels still build/render correctly without manual scroll behavior

Expected temporary state:
- panels may stop scrolling for a short time
- clipping and layout should still function
- this is acceptable while ownership is moved into `LayoutSystem`

### Phase 2 — make LayoutSystem own scroll helper topology

Once manual panel ownership is gone, add layout-owned scroll management:

- when `overflow: Scroll` is detected, `LayoutSystem` ensures a scroll helper exists
- `LayoutSystem` also ensures a scroll-track transform exists
- authored children are wrapped / reparented under that scroll track
- scroll state is applied by moving the scroll track, not by rebuilding row windows

This should be implemented analogously to `sync_bg_quad(...)` / `sync_stencil_clip(...)`:
- detect required helper topology
- create it if absent
- update it if present
- remove it when `overflow` changes away from `Scroll`

### Phase 2.5 — v1 scope boundary

Version 1 should **not** attempt CPU-side hide/show or windowing of offscreen items.

In v1:
- all items remain present under the scroll track
- GPU stencil clipping is the only visibility enforcement
- scroll just moves the inner track

This keeps the first migration focused on correctness and ownership.

Version 2 can later add CPU-side clipping / culling once the engine has reliable
bounding-box (or broader bounding-volume) calculation for clip shapes and their children.

### Phase 3 — migrate panels to `overflow: Scroll`

After layout-owned scroll behavior works, migrate panel content slots to the style API.

Recommended order:
1. **world panel first** — easiest to validate visually
2. **inspector panel second**

For both panels:
- set `content_style.overflow = Overflow::Scroll`
- keep `background_color` behavior as needed so clip geometry exists
- rely on layout-owned clip + scroll helper topology
- remove virtual-window rebuild-on-scroll behavior
- keep selection-change rebuilds only for actual data changes, not drag motion

### Phase 4 — unify / retire old virtual-window component model

After both panels are migrated:
- switch `ScrollingComponent` over fully to the layout-owned implementation
- remove the old virtual-window assumptions from its API / call sites
- remove obsolete row-window assumptions (`PAGE_SIZE`, `window_start`,
  `sub_row_y_offset`, fixed per-row height scroll math) where no longer needed

---

## 6. Why this is better

This refactor fixes the ownership split:

### Before
- `StyleComponent::overflow` partly owned clipping
- `InspectorSystem` manually owned scroll interaction
- panel content was virtual-windowed and rebuilt during drag
- scroll math depended on fixed row heights

### After
- `StyleComponent::overflow` declares viewport behavior
- `LayoutSystem` owns helper topology for clip + scroll
- scrolling is just moving a scroll track inside a clipped viewport
- panel systems provide content only; they do not own scroll mechanics

That is a much cleaner boundary.

---

## 7. CPU-side clipping note

Even with stencil clipping on the GPU, `VisualWorld` / renderer should eventually do a
**coarse CPU-side reject** for items that are completely outside the clip volume.

Important nuance:
- the clip shape is not guaranteed to be rectangular forever
- so this should be framed as **bounding-volume rejection against the clip shape's
  coarse bounds**, not "rectangle clipping"

Suggested note for implementation:
- compute a conservative world-space bounding box / bounding sphere for the clip shape
- if an item's conservative bounds do not intersect that clip bound at all, skip
  emitting it into the phase-local DFS render stream for that clip subtree
- if bounds overlap, keep it and let stencil do the exact per-pixel clipping

This is an optimization only. Correctness still comes from stencil.

This is explicitly **v2 work**, not part of the initial scrolling migration.

Prerequisite:
- reliable bounding-volume calculation for both clip shapes and candidate children

Until that exists, the engine should not try to hide/show children under scroll
containers based on CPU-side tests.

---

## 8. Relationship tracking note

We should avoid repeatedly scanning subtrees to rediscover what is nested under
`Scrolling{}` or `StencilClip{}`.

The target model is incremental relationship maintenance:

- when a scroll helper is added or removed, update the relevant maps
- when a stencil clip is added or removed, update the relevant maps
- when a renderable is attached under one of those helpers, update the relevant maps
- when a renderable is detached or removed, update the relevant maps

So instead of "find descendants every frame", maintain ownership/cached relationships
on topology changes.

Conceptually this can be represented as maps like:
- clip root → scroll helper / scroll track
- scroll helper → clip root
- renderable / instance → nearest enclosing clip root
- renderable / instance → nearest enclosing scroll helper

The exact storage location can be decided during implementation, but the rule is:
**derive once on attach/remove, not by scanning on every layout or draw pass.**

---

## 9. Rollout checklist

- [ ] Create layout-owned refactor note / target topology
- [ ] Remove manual `ScrollingComponent` creation from `InspectorSystem`
- [ ] Remove manual `DragMove` / `ScrollChanged` handler wiring from `InspectorSystem`
- [ ] Verify panels still build/render without manual scrolling ownership
- [ ] Remove / isolate all old-model `ScrollingComponent` call sites before changing its implementation
- [ ] Make `LayoutSystem` create clip-root + scroll-helper topology for `overflow: Scroll`
- [ ] Ensure the nesting is clip root → scroll helper → scroll track → authored children
- [ ] Reparent / maintain authored children under the scroll track
- [ ] Apply scroll offset by moving the scroll track, not rebuilding visible windows
- [ ] Keep all children live in v1; do not add CPU hide/show yet
- [ ] Add incremental relationship/cache maintenance on helper add/remove and renderable attach/detach
- [ ] Migrate world panel content slot to `overflow: Scroll`
- [ ] Validate world panel scrolling / clipping behavior end-to-end
- [ ] Migrate inspector panel content slot to `overflow: Scroll`
- [ ] Remove old virtual-window assumptions from panel rebuild logic
- [ ] Reimplement `ScrollingComponent` for the layout-owned model once old uses are gone
- [ ] Add coarse CPU-side clip-bound rejection in v2 once bounding-volume support exists

---

## 10. Suggested implementation order

If we want the safest execution order:

1. **Document the ownership change**
2. **Delete manual inspector scroll wiring / old `ScrollingComponent` call sites**
3. **Make layout-owned scroll helper topology work**
4. **Reimplement `ScrollingComponent` for the new ownership model**
5. **Migrate world panel to `overflow: Scroll`**
6. **Test world panel thoroughly**
7. **Migrate inspector panel**
8. **Clean up old virtual-window assumptions**

This keeps the easiest test target (`world panel`) as the first real migration site.
