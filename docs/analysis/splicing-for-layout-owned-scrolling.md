# Splicing for layout-owned scrolling

Date: 2026-04-23

This note reframes layout-owned scrolling as a **general splicing problem**, not as a special-case
layout or event-handler trick.

The immediate motivation is `overflow: scroll` on layout items such as the `diy-panel` yellow
container. The layout system should be able to insert a `ScrollingComponent` wrapper automatically,
but the displaced authored children must attach to a **specific internal node** of that inserted
subtree (`__scroll_track`), not to the inserted root itself.

---

## 1. Current status of splicing in the engine

Relevant existing docs:

- [docs/refactor/splice-component-into-topology.md](../refactor/splice-component-into-topology.md)
- [docs/analysis/vr-input-controllerxr-armature-splice.md](vr-input-controllerxr-armature-splice.md)
- [docs/meow_meow/analysis/tree-splicing.md](../meow_meow/analysis/tree-splicing.md)
- [docs/refactor/scrolling-component-layout-system.md](../refactor/scrolling-component-layout-system.md)
- [docs/analysis/layout-owned-scrolling-via-router-reparent.md](layout-owned-scrolling-via-router-reparent.md)

The new router-reparent note captures a simpler near-term path for scrolling specifically. This
document remains the broader analysis for a future general splice/query-based solution.

Current status:

- the engine already recognizes the **output-node problem** for tree splices
- there is still no first-class splice helper in `World` / `Universe`
- current splices are still open-coded as explicit attach/reparent sequences
- existing splice docs mostly discuss **one displaced child subtree** being reattached under a
  nominated output node in the inserted tree

That is enough to explain armature/controller splices, but layout-owned scrolling needs one more
generalization.

---

## 2. Why scrolling is a splice problem

For a layout item with `Style { overflow: Scroll }`, the desired ownership is:

- `LayoutSystem` owns helper topology
- `ScrollingComponent` owns scroll state and initialization
- `ScrollSystem` owns drag/scroll event handling
- authored content should move under `__scroll_track`

So this:

```text
item_tc
  Style
  child_a
  child_b
  child_c
```

needs to become something more like:

```text
item_tc
  Style
  __bg
    Color
    Renderable
    Raycastable(drag_only)
    RaycastableShape(quad2d)
  __scroll
    ScrollingComponent
    __scroll_track
      child_a
      child_b
      child_c
```

The important structural fact is:

> the authored children do **not** attach to `__scroll`.
> they attach to a specific internal node inside the inserted scroll subtree: `__scroll_track`.

That is the same core shape as the armature splice docs:

- inserted subtree root
- nominated internal output node
- displaced existing subtree reattached under that output node

But scrolling generalizes it from “one old child subtree” to “a set of existing authored children
under a parent that now need to be wrapped through a nominated output node”.

---

## 3. Why layout should not own event handlers

The desired split is:

- `LayoutSystem` detects `overflow: Scroll`
- `LayoutSystem` ensures the structural wrapper exists
- `ScrollingComponent` initializes itself normally
- `ScrollSystem` registers drag forwarding and track movement

So layout should not install scroll event handlers directly.

Layout's responsibility is purely structural:

- create / maintain the scroll wrapper subtree
- splice the authored content under the wrapper's output target
- keep clipping helpers (`__bg`, stencil clip) in sync alongside it

Behavior remains component-owned:

- `ScrollingComponent::init()` triggers normal scrolling registration
- `ScrollSystem` handles drag forwarding, scroll offset, and track translation

This keeps scrolling consistent with the rest of ECS ownership rather than making layout a hidden
runtime owner of gesture behavior.

---

## 4. The missing splice capability

Existing splice language usually looks like:

```text
splice parent -> inserted_root -> child
```

or, in the richer docs:

```text
splice_tree(parent, child, inserted_root, inserted_output)
```

Scrolling needs a closely related but broader operation:

```text
wrap_children_with_subtree(
    parent = item_tc,
    inserted_root = __scroll,
    inserted_output = __scroll_track,
    displaced_children = authored_layout_children,
)
```

Key properties:

- the original parent (`item_tc`) stays the same
- the inserted subtree root (`__scroll`) becomes a new child of that parent
- a subset of the parent's existing children are displaced under `__scroll_track`
- helper children that should remain at the outer level (`Style`, `__bg`, stencil helpers, maybe
  router/helper nodes) are excluded from the displacement set

So this is not just a one-edge splice helper.

It is a **targeted child-wrap splice**:

- insert subtree under a parent
- resolve a nominated output node within that subtree
- move a selected set of siblings under that output node

---

## 5. Why the output target must use the unified query language

The engine should not invent a one-off “scroll output name” convention just for scrolling.

The output target should be selected using the engine's existing unified query language / selector
model, the same conceptual language already used for routing and planned MMS tree queries.

That means the splice/wrap operation should conceptually accept something like:

```text
output_query = "[name='__scroll_track']"
```

or equivalent query sugar in Meow Meow.

Why this is the right shape:

- it reuses one query language across routing, tree search, and splice targeting
- it avoids baking hardcoded output semantics into the helper API
- it generalizes beyond scrolling to controller pipelines, transform outputs, and future wrapper
  components
- it lets different inserted subtrees expose different output targets without new bespoke Rust-side
  helper signatures for each one

For scrolling specifically, querying by name is probably sufficient initially:

- `"[name='__scroll_track']"`

But the operation should be framed as a selector/query target, not as a special string field that
only knows about scroll track names.

---

## 6. Proposed general model

### 6.1 Structural primitive

Conceptually, the engine needs a helper in this family:

```text
splice_children_into_subtree(
    parent,
    inserted_root,
    output_query,
    child_filter,
)
```

where:

- `parent` is the existing owner whose children are being wrapped
- `inserted_root` is the newly inserted helper subtree root
- `output_query` resolves a descendant of `inserted_root`
- `child_filter` decides which existing children move under the resolved output node

For layout-owned scrolling:

- `parent` = the layout item transform with `overflow: Scroll`
- `inserted_root` = the auto-owned scroll wrapper root
- `output_query` = `[name='__scroll_track']`
- `child_filter` = authored content children, excluding style/helper/internal nodes

### 6.2 MMS / query-facing model

At the MMS level, this should eventually align with the query/splice work already discussed in:

- [docs/meow_meow/analysis/tree-splicing.md](../meow_meow/analysis/tree-splicing.md)

The main refinement from scrolling is:

- the displaced target may be **multiple existing siblings**, not only one child edge
- the output target should be a query/selector, not just an implicit port label

So a future MMS-facing concept may need both:

- a way to construct an inserted subtree
- a way to nominate its output target by selector or named port
- a way to specify which existing children are being wrapped/spliced through it

---

## 7. How this applies to layout-owned scrolling

For `Style { overflow: Scroll }`, the layout system should do the following:

1. ensure clip helper topology exists (`__bg`, stencil clip, drag surface)
2. ensure a scroll wrapper subtree exists under the item transform
3. resolve the wrapper's internal output target (`__scroll_track`)
4. splice/wrap authored content children under that output target
5. leave `ScrollingComponent` initialization and drag behavior to normal component/system flow

That means layout owns topology, not runtime behavior.

This also cleanly explains why `diy-panel` currently does not scroll:

- clip/drag-surface helpers can exist from `overflow: Scroll`
- but there is no auto-owned `ScrollingComponent` wrapper yet
- and there is no splice step rehoming content under a scroll track

---

## 8. Practical implications

### Short term

- keep `ScrollSystem` as the behavior owner
- teach `LayoutSystem` to auto-insert a scrolling subtree for `overflow: Scroll`
- make that insertion use a general output-target splice/wrap model

### Medium term

- add a general world/universe helper for subtree insertion + output-target child wrapping
- make the output target resolved through the unified query language
- reuse that same helper shape for future wrapper/pipeline/splice scenarios

### Long term

- unify routing, query, and splice-target concepts under one tree-query vocabulary
- expose that structurally in Meow Meow so scripts can describe wrapper insertion declaratively

---

## 9. Recommendation

Treat layout-owned scrolling as the first non-armature proof that the engine needs a general
**inserted-root + queried-output-target + displaced-children** splice model.

The right abstraction is not:

- “layout creates scroll handlers”
- “scrolling is a special case of overflow”
- “add a hardcoded `scroll_track` field to the layout system”

The right abstraction is:

- layout inserts an ordinary scrolling subtree
- the subtree exposes an internal output target
- existing authored children are reattached to that target
- the target is selected through the engine's unified query language

That generalization should be the basis for scrolling-in-layout and for future splice APIs.