# Layout-owned scrolling and nested routing

Date: 2026-04-23

This note is a checklist and state recap for the next scrolling work.

It exists to separate the immediate implementation work from the larger query/splice design work.
The current blocker is simpler than the full selector/query architecture.

---

## 1. Current state recap

What exists today:

- `RouterSystem` can reroute external direct children of an owner into a named target inside that
  owner's subtree.
- `ScrollingSystem` already knows how to auto-create:
  - `__scroll_router`
  - `__scroll_track`
- `ScrollingSystem` also already knows how to:
  - register drag forwarding
  - maintain scroll offset
  - move the track transform
- `LayoutSystem` already knows how to create overflow/clipping helpers for
  `Style { overflow: Hidden | Scroll }`, including:
  - `__bg`
  - stencil clip wiring
  - drag-only raycast surface for the scroll viewport area

What does **not** exist yet:

- `LayoutSystem` does **not** yet auto-add a `ScrollingComponent` when it sees
  `Style { overflow: Scroll }`
- `LayoutSystem` does **not** yet auto-add the outer router that should feed authored content into
  the inserted scrolling wrapper
- `LayoutSystem` does **not** yet sync measured viewport/content sizes into that scrolling state
- the example topology still routes content into the visible container root, not into a layout-
  managed scrolling wrapper

So the engine already has most of the inner scrolling behavior, but it does not yet connect that
behavior to layout-owned overflow scroll containers.

---

## 2. Why `diy-panel` does not scroll currently

The current `diy-panel` failure is **not** primarily a query-language problem.

It is also not mainly “children are stealing drag events”.

The main problem is structural/runtime ownership:

1. the example's top-level layout root has a router that targets `container`
2. the rows are therefore attached under the yellow `container`
3. `container` has `Style { overflow("scroll") }`
4. but `overflow("scroll")` currently only gives us clip/drag-surface helpers
5. it does **not** currently auto-create a `ScrollingComponent`
6. therefore there is no scroll state, no `__scroll_track`, and no scrolling runtime for that
   container

So when the user drags in the yellow area:

- the drag surface exists
- clipping exists
- but there is no `ScrollingComponent` instance whose offset should change
- so nothing moves

Another way to say it:

- content is being routed into the container root
- but there is no inner scrolling wrapper yet
- therefore there is no second routing step into `Scrolling { __scroll_track }`

That is why the current work is about **layout-owned scrolling + nested routing**, not about the
full query/parser stack first.

---

## 3. The intended model

The intended model now is:

- `LayoutSystem` sees `Style { overflow: Scroll }`
- `LayoutSystem` ensures an outer `RouterComponent` on the styled item
- `LayoutSystem` ensures a `ScrollingComponent` wrapper exists
- the outer router routes authored content into the scrolling wrapper root
- `ScrollingComponent` always owns its own `__scroll_router`
- that inner router routes incoming external children into `__scroll_track`

This gives us a two-stage routing model:

1. layout boundary routing
2. scrolling-internal routing

That model is important because `ScrollingComponent` should also behave correctly when authored
outside layout.

If someone attaches content directly to `Scrolling{}` later, it should still normalize that content
into `__scroll_track` automatically.

---

## 4. Immediate checklist

### A. Make `ScrollingComponent` always auto-add its own router

Goal:

- ensure `ScrollingComponent` is self-contained
- ensure it routes incoming external direct children to `__scroll_track`
- ensure that works both:
  - at init
  - when children are attached later

What this means concretely:

- keep / harden the existing `__scroll_router` ownership in `ScrollingSystem`
- verify registration always creates the internal router and track
- verify later attachments under `Scrolling{}` get rerouted into `__scroll_track`
- make this behavior true whether `Scrolling{}` was created by layout or authored directly

### B. Make scrolling work in `diy-panel`

Goal:

- `examples/diy-panel.mms` should scroll correctly using layout-owned scrolling

What needs to become true:

- the yellow scroll container gets a layout-created scrolling wrapper
- the yellow scroll container gets a layout-created outer router
- authored/routed rows end up under the wrapper
- `ScrollingComponent` then routes them into `__scroll_track`
- dragging the yellow viewport changes the scroll offset and moves the track

Important success condition:

- `diy-panel` should scroll without manually authoring `ScrollingComponent` in MMS

### C. Add content-insertion buttons in Rust examples

Goal:

- both router and diy-panel examples should have a Rust-side button that adds new content into the
  routed content area

Why this matters:

- it verifies static authored content routing
- it verifies late runtime attachment routing
- it verifies nested routing into layout-created `Scrolling { __scroll_track }`

Expected behavior:

- pressing the button adds a new child to the routed content slot
- the outer routing step sends it into the scrolling wrapper
- the inner scrolling router sends it into `__scroll_track`
- the new content appears in the scrolled content region without manual topology fixups

Notes:

- the button should live in Rust code, not only in MMS
- `examples/router.rs` likely provides the most relevant starting point for button wiring
- `diy-panel` will need equivalent runtime content insertion so the same routing path is exercised

---

## 5. Non-goals for the first implementation pass

These should **not** block the first pass:

- general query parser/evaluator integration
- general subtree splice helper APIs
- generalized selector-based output targeting for all wrapper components
- CPU-side clipping / culling improvements

Those are still useful longer-term, but the current scrolling blocker can be solved first with the
existing router + scrolling topology conventions.

---

## 6. Implementation order

Recommended order:

1. harden/verify `ScrollingComponent` self-routing to `__scroll_track`
2. make layout auto-create the outer router + scrolling wrapper for overflow scroll items
3. confirm `diy-panel` scrolls correctly
4. add Rust-side content insertion button to `router` example
5. add Rust-side content insertion button to `diy-panel` example
6. confirm late-added children route all the way into `Scrolling { __scroll_track }`

That sequence keeps the low-level routing guarantee in place before testing layout-owned scrolling
and runtime mutation.