# Layout-owned scrolling via router reparent

Date: 2026-04-23

This note describes a simpler near-term path for automatic scrolling in layout:

- `LayoutSystem` auto-inserts a `ScrollingComponent`
- `LayoutSystem` also auto-inserts a `RouterComponent` on the styled layout item
- that outer router selects authored siblings and routes them into the inserted scrolling node
- `ScrollingSystem` always owns `__scroll_router` and `__scroll_track`
- the scrolling node's owned router then redirects those direct children into `__scroll_track`

This avoids needing the full general query/splice system immediately, while still matching the
desired ownership split:

- layout owns structure
- scrolling owns scroll behavior
- layout does not install event handlers

Related docs:

- [docs/refactor/scrolling-component-layout-system.md](../refactor/scrolling-component-layout-system.md)
- [docs/analysis/splicing-for-layout-owned-scrolling.md](splicing-for-layout-owned-scrolling.md)
- [docs/analysis/query-usage.md](query-usage.md)

---

## 1. Why this is simpler

The current generalized splice framing is still good long-term, but it may be more machinery than
we need just to get layout-owned scrolling working.

The important current engine facts are:

- `RouterSystem` already supports routing external direct children into a named target anywhere in
  the router owner's subtree
- `ScrollingSystem` already auto-creates:
  - `__scroll_track`
  - `__scroll_router`
- that router already targets `__scroll_track` by name
- that router already reroutes newly attached external direct children of the scrolling owner

So layout does not need to wait on `__scroll_track` directly, and it does not need to know the
track as a public attachment point.

Instead, layout can:

1. ensure the outer styled item has a router
2. point that router at the scrolling wrapper root
3. let `Scrolling{}` itself route its external children into `__scroll_track`

That means we may not need a new general “output query” splice helper for v1 scrolling.

---

## 2. Current behavior we can reuse

From the existing runtime:

- [src/engine/ecs/system/scroll_system.rs](../../src/engine/ecs/system/scroll_system.rs)
  - `ensure_owned_router_and_track(...)`
  - creates `__scroll_track`
  - creates `__scroll_router` with `target_name = "__scroll_track"`
- [src/engine/ecs/system/router_system.rs](../../src/engine/ecs/system/router_system.rs)
  - reroutes external direct children of the router owner into the target subtree node
- [src/engine/ecs/component/router.rs](../../src/engine/ecs/component/router.rs)
  - router targeting is already a first-class component concern

This matters because the missing topology step for layout-owned scrolling may be expressible as:

1. create scrolling wrapper
2. create an outer router on the styled item
3. initialize scrolling normally
4. let the outer router send authored content into the scrolling wrapper
5. let the scrolling wrapper's owned router forward them to `__scroll_track`

---

## 3. Desired topology

Starting from a layout item like:

```text
item_tc
  Style { overflow: Scroll }
  child_a
  child_b
  child_c
```

layout would move toward:

```text
item_tc
  Style
  Router(target = "__scroll")  ← layout-owned content selection policy
  __bg
    Color
    Renderable
    Raycastable(drag_only)
    RaycastableShape(quad2d)
  __scroll
    ScrollingComponent
    __scroll_router       ← owned by ScrollingSystem
    __scroll_track        ← owned by ScrollingSystem
      child_a
      child_b
      child_c
```

Layout does not attach authored children directly to `__scroll_track`.

It instead creates a two-stage route:

```text
item_tc
  Style
  Router(target = "__scroll")
  __bg
  child_a
  child_b
  child_c
  __scroll
    __scroll_router
    __scroll_track
```

At runtime:

- the outer router moves `child_a..c` into `__scroll`
- `__scroll_router` then moves them into `__scroll_track`

---

## 4. How it would play out

### Step 1 — detect `overflow: Scroll`

`LayoutSystem` sees `Style { overflow: Scroll }` on a layout item root (`item_tc`).

It already knows how to do the clip side:

- ensure `__bg`
- ensure stencil clip
- ensure drag surface on the background renderable

### Step 2 — ensure a scrolling wrapper exists

`LayoutSystem` ensures a direct child of `item_tc`, conceptually:

```text
__scroll
  ScrollingComponent
```

It also ensures `item_tc` has a router component configured to target `__scroll`.

The scrolling component is initialized normally.

That means `ScrollingSystem` does its existing setup:

- ensure `__scroll_track`
- ensure `__scroll_router`
- register drag forwarding based on the nearest drag scope

This inner router exists regardless of whether `ScrollingComponent` was added by layout or authored
directly elsewhere.

### Step 3 — choose which siblings to move

`LayoutSystem` then finds the current direct children of `item_tc` and selects the ones that
represent authored content.

Those should be selected by the outer router and routed into `__scroll`.

Importantly, layout should **not** move:

- `StyleComponent`
- `__bg`
- layout-owned stencil clip helpers
- the scrolling wrapper itself
- any other internal helper labels beginning with `__`

The main payload to move is the authored content subtree that currently sits directly under the
scrolling item root.

### Step 4 — let the two routers finish the wrap

Once the outer router routes authored children into `__scroll`, the scrolling wrapper's owned
router should reroute them into `__scroll_track`.

That gives the desired final shape without layout needing to know about the track node directly.

### Step 5 — normal scrolling behavior proceeds

Now the existing runtime can work as intended:

- `ScrollingComponent` owns scroll state
- `ScrollSystem` updates `__scroll_track`
- drag events are forwarded by the already-installed scrolling handlers

No layout-owned event registration is needed.

---

## 5. Why this avoids the bigger query/splice dependency

This path avoids needing all of the following just to land v1 automatic layout scrolling:

- CSS selector parser
- MMQ parser
- shared query evaluator integration into world/universe query APIs
- general subtree splice helper with output-target query resolution

Why it works without those:

- the layout-facing attachment point is the scrolling wrapper root
- the internal scrolling output target convention is already fixed: `__scroll_track`
- `ScrollingSystem` already knows how to create and target that node
- `RouterComponent` already provides a targeted rehome mechanism by name

So the layout system can rely on an existing special-purpose two-stage routing convention rather
than waiting for the fully generalized query-targeted splice architecture.

---

## 6. Caveats / what still needs thought

### 6.1 Content-height and viewport-height still need ownership

This router-based reparent path only solves the topology problem.

Automatic scrolling still needs correct values for:

- viewport height
- content height

Those likely need to be derived from layout measurement and synced into the auto-owned
`ScrollingComponent`.

So the remaining work is not just “insert wrapper and done”.

### 6.2 Layout re-entry / child discovery rules

Once content is wrapped under `__scroll` and then under `__scroll_track`, layout must still be
clear about which nodes are considered layout items and which are helpers.

That means helper exclusion rules need to stay consistent:

- nodes with `__` labels remain non-authored/internal
- `Style` still belongs at the outer scroll item root
- `__scroll_track` contents remain the actual layout-owned authored children

### 6.3 Outer-router selection rules still matter

The outer router now becomes the place where layout expresses content-selection policy.

That means its ignore list / routing criteria must clearly exclude:

- `StyleComponent`
- the outer router itself
- `__bg`
- `__scroll`
- other internal helper nodes beginning with `__`

This is cleaner than targeting `__scroll_track` from outside, but it still requires a stable rule
for what counts as authored content.

### 6.4 This is still a scrolling-specific convention

This is simpler, but it is not the same as a general splice API.

It works because scrolling already has:

- a known wrapper component
- a known internal target name
- a known router-based rehome strategy

Future wrapper/pipeline cases may still need the more general query/splice mechanism.

---

## 7. Recommended near-term direction

For automatic layout scrolling, the simplest practical approach is:

1. `LayoutSystem` auto-creates a scrolling wrapper for `overflow: Scroll`
2. `LayoutSystem` also auto-creates a router on the styled item targeting that wrapper
3. `ScrollingSystem` continues to auto-own `__scroll_track` and `__scroll_router`
4. the outer router routes authored content into the scrolling wrapper
5. the inner scrolling router rehomes those children into the track
6. layout synchronizes viewport/content sizes into the scrolling component

This gets automatic scrolling working without first blocking on a general query parser/evaluator or
general output-target splice helper, while still keeping `Scrolling{}` self-contained when used
outside layout.

---

## 8. Long-term relationship to the general splice design

This router-based reparent approach is a good **v1 implementation strategy**.

It does not invalidate the broader query/splice work.

Instead:

- v1 scrolling can use the existing scrolling/router convention
- later, the engine can generalize that pattern into a proper subtree splice/wrap operation
- at that point, scrolling can become one client of the general mechanism instead of a special
  convention

So the simple path is:

- good enough for scrolling now
- still compatible with the more general future architecture

That makes it a strong candidate for the next implementation step.