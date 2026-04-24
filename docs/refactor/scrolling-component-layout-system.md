# ScrollingComponent + LayoutSystem
## Refactor plan: scrolling is separate from clipping

---

## 1. Goal

Scrolling and clipping are related but **not the same concern**.

See also [docs/analysis/splicing-for-layout-owned-scrolling.md](../analysis/splicing-for-layout-owned-scrolling.md) for the newer framing of layout-owned scrolling as a general splice/output-target problem.

- `StyleComponent::overflow = Hidden | Scroll` expresses **viewport clipping**.
- Scrolling expresses **motion of content inside that clipped viewport**.

The mistake in the current panel code is that scrolling is still manually owned by
`InspectorSystem`, while clipping is becoming layout-owned via `StyleComponent` and
`LayoutSystem`.

The target architecture is:
- `StyleComponent::overflow` tells `LayoutSystem` whether the item is a clip viewport.
- `LayoutSystem` automatically maintains the helper topology needed for that viewport.
- `LayoutSystem` adds both `ScrollingComponent` and an outer `RouterComponent` when `overflow: Scroll` requires scroll wrapping.
- `ScrollingComponent` remains a standalone scrolling primitive, with behavior owned by a dedicated `ScrollSystem`.
- `ScrollingComponent` always owns an internal router targeting `__scroll_track`, even when authored outside layout.
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
2. **scroll layer** — `ScrollingComponent` + `ScrollSystem`, owning offset / range / track topology and gesture behavior independently of clipping

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
  wrapped-by-layout, driven-by-`ScrollSystem` model described here

That means the migration order matters:
1. remove `ScrollingComponent` from the places where it is currently used in the old way
2. then change `ScrollingComponent`'s implementation / runtime ownership model

The key architecture point is now split cleanly:
- `LayoutSystem` owns attachment / wrapping topology and the outer content-selection router
- `ScrollSystem` owns scrolling behavior
- `ScrollingComponent` owns its internal content normalization into `__scroll_track`
- `InspectorSystem` should own neither

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
  RouterComponent { target_name: "__scroll", ... }
  __bg / clip root            ← auto-managed clip geometry / clip owner
    ColorComponent
    RenderableComponent
    StencilClipComponent
  __scroll                    ← auto-managed `ScrollingComponent` holder / scope root
    ScrollingComponent
    __scroll_router           ← auto-managed by `ScrollingComponent`
    __scroll_track            ← auto-managed transform wrapping scrollable children
      child_0_tc
      child_1_tc
      ...
```

Important properties:
- the clip root (`__bg`) is the outer viewport boundary
- `__scroll` holds `ScrollingComponent`
- the outer router on `item_tc` chooses which authored siblings become scroll content
- `ScrollingComponent` then routes its incoming external children into `__scroll_track`
- `ScrollSystem` handles scroll state / gesture registration / limits for that component
- `__scroll_track` is what actually moves
- authored child transforms end up under the scroll track, not directly under `item_tc`

So the nesting is:
- `StencilClip` / clip root outside
- styled item router at the scroll boundary
- scrolling helper inside that boundary
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

Once manual panel ownership is gone, add layout-owned scroll wrapping:

- when `overflow: Scroll` is detected, `LayoutSystem` ensures a scroll helper exists
- `LayoutSystem` also ensures an outer router exists targeting that helper
- `ScrollingComponent` ensures its internal router + `__scroll_track` exist
- authored children are routed into the scroll helper, then into the internal track
- `ScrollSystem` applies scroll state by moving the scroll track, not by rebuilding row windows

This should be implemented analogously to `sync_bg_quad(...)` / `sync_stencil_clip(...)`:
- detect required helper topology
- create it if absent
- update it if present
- remove it when `overflow` changes away from `Scroll`

What `LayoutSystem` should **not** do:
- own scroll state itself
- apply drag logic itself
- know about `__scroll_track` as a public attachment target
- become the runtime system responsible for scrolling behavior

That runtime responsibility belongs in `ScrollSystem`, so `ScrollingComponent` can also
work on its own outside layout-specific clipping / paging logic.

And that standalone use matters: when `ScrollingComponent` is authored outside layout, it should
still route its direct content children into `__scroll_track` automatically.

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

After layout-owned scroll wrapping + `ScrollSystem` behavior work, migrate panel content slots to the style API.

Recommended order:
1. **world panel first** — easiest to validate visually
2. **inspector panel second**

For both panels:
- set `content_style.overflow = Overflow::Scroll`
- keep `background_color` behavior as needed so clip geometry exists
- rely on layout-owned clip + scroll-helper wrapping topology
- remove virtual-window rebuild-on-scroll behavior
- keep selection-change rebuilds only for actual data changes, not drag motion

### Phase 4 — unify / retire old virtual-window component model

After both panels are migrated:
- switch `ScrollingComponent` over fully to the new standalone + `ScrollSystem` implementation
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
- `LayoutSystem` owns helper topology for clip + scroll wrapping
- `ScrollSystem` owns scrolling behavior
- scrolling is just moving a scroll track inside a clipped viewport
- panel systems provide content only; they do not own scroll mechanics

That is a much cleaner boundary.

Related draft:
- `docs/draft/layout-owned-stencil-clip-source.md` describes how a layout-generated
  `StencilClip` may resolve its clip shape from the computed adjacent `__bg`
  renderable while keeping authored content on a separate branch.

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

This relationship tracking likely belongs with the future `ScrollSystem` / render-side
registration path, not inside `LayoutSystem` itself.

---

## 9. Rollout checklist

- [ ] Create refactor note / target topology for layout wrapping + `ScrollSystem` behavior
- [ ] Remove manual `ScrollingComponent` creation from `InspectorSystem`
- [ ] Remove manual `DragMove` / `ScrollChanged` handler wiring from `InspectorSystem`
- [ ] Verify panels still build/render without manual scrolling ownership
- [ ] Remove / isolate all old-model `ScrollingComponent` call sites before changing its implementation
- [ ] Reintroduce a clean standalone `ScrollSystem`
- [ ] Make `LayoutSystem` create clip-root + scroll-helper topology for `overflow: Scroll`
- [ ] Ensure the nesting is clip root → scroll helper → scroll track → authored children
- [ ] Reparent / maintain authored children under the scroll track
- [ ] Make `ScrollSystem` apply scroll offset by moving the scroll track, not rebuilding visible windows
- [ ] Keep all children live in v1; do not add CPU hide/show yet
- [ ] Add incremental relationship/cache maintenance on helper add/remove and renderable attach/detach
- [ ] Migrate world panel content slot to `overflow: Scroll`
- [ ] Validate world panel scrolling / clipping behavior end-to-end
- [ ] Migrate inspector panel content slot to `overflow: Scroll`
- [ ] Remove old virtual-window assumptions from panel rebuild logic
- [ ] Reimplement `ScrollingComponent` for the standalone + `ScrollSystem` model once old uses are gone
- [ ] Add coarse CPU-side clip-bound rejection in v2 once bounding-volume support exists

---

## 10. Suggested implementation order

If we want the safest execution order:

1. **Document the ownership change**
2. **Delete manual inspector scroll wiring / old `ScrollingComponent` call sites**
3. **Reintroduce a clean `ScrollSystem`**
4. **Make layout-owned scroll helper wrapping work**
5. **Reimplement `ScrollingComponent` for the new `ScrollSystem` ownership model**
6. **Migrate world panel to `overflow: Scroll`**
7. **Test world panel thoroughly**
8. **Migrate inspector panel**
9. **Clean up old virtual-window assumptions**

This keeps the easiest test target (`world panel`) as the first real migration site.
