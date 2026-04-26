# Layout and scrolling routing

Date: 2026-04-25

This is the current handoff/task checklist for layout-owned backgrounds, clipping, and scrolling.

It is meant to be readable by a fresh agent with no prior context.

---

## 1. Preamble: systems and components involved

### Core systems

- `LayoutSystem`
  - measures and positions layout items
  - creates and updates layout-owned helper topology such as `__bg`
  - currently owns clipping helper creation for `Style { overflow: Hidden | Scroll }`
  - does **not** yet fully own layout-created `ScrollingComponent` insertion for `overflow: Scroll`
- `ScrollingSystem`
  - owns scrolling runtime behavior
  - auto-creates `__scroll_router` and `__scroll_track` for `ScrollingComponent`
  - routes direct children of `Scrolling{}` into `__scroll_track`
  - owns scroll offset, drag forwarding, and track movement
  - now prefers sibling `__bg` as its input/drag source when present
- `RouterSystem`
  - reroutes external direct children of a routed owner into a named target in that owner's subtree
  - used both for general authored routing and for scrolling's internal routing
- `ClippingSystem` / stencil clip registration path
  - treats layout-generated `__bg` renderables as the clip source when overflow requires clipping
- `GestureSystem`
  - emits drag events on the hit renderable scope
  - those events are observed through the scoped/ancestor handler chain in `RxWorld`

### Important components

- `StyleComponent`
  - `background_color` requests a layout-generated background helper
  - `overflow = Hidden | Scroll` requests clipping behavior
- `TransformComponent`
  - the main tree/topology node used for authored items and helper nodes
- `RenderableComponent`
  - visual/hit-test surface used by clip and drag behavior
- `StencilClipComponent`
  - declares the clip boundary for a subtree
- `RouterComponent`
  - declarative child rerouting into a named target in a subtree
- `ScrollingComponent`
  - scroll state + runtime-owned track and routing behavior

### Reserved helper labels

- `__bg`
  - layout-owned viewport/background helper
- `__scroll`
  - planned layout-owned scrolling wrapper root for overflow scroll items
- `__scroll_router`
  - scrolling-owned internal router
- `__scroll_track`
  - scrolling-owned moved content transform

---

## 2. Current state summary

### Recently completed

- [x] `ScrollingComponent` works on its own
- [x] `ScrollingSystem` auto-creates and owns:
  - `__scroll_router`
  - `__scroll_track`
- [x] direct children attached to `Scrolling{}` are rerouted into `__scroll_track`
  - both at init time and on later attaches
- [x] `ScrollingSystem` now prefers sibling `__bg` as the input surface before ancestor fallback
- [x] added a focused scrolling demo:
  - `examples/scrolling.mms`
  - `examples/scrolling.rs`
- [x] demo covers two cases:
  - standalone/manual scrolling using ancestor/parent renderable drag scope
  - layout-mock scrolling using sibling `__bg`

### Still pending

- [ ] `LayoutSystem` auto-creates `ScrollingComponent` for `Style { overflow: Scroll }`
- [ ] `LayoutSystem` auto-creates an outer `RouterComponent` for overflow-scroll layout items
- [ ] `LayoutSystem` routes authored children into the layout-created scrolling wrapper
- [ ] `LayoutSystem` syncs measured viewport/content sizing into scrolling state
- [ ] `examples/diy-panel.mms` actually scrolls through layout-owned scrolling
- [ ] runtime add-item buttons in `router` and `diy-panel` examples

---

## 3. Desired ownership split

### Layout owns

- detection of `Style { background_color, overflow }`
- helper topology such as `__bg`
- clip helper topology and drag-only viewport surface
- outer routing for layout-owned overflow containers that relocate authored children

### Scrolling owns

- internal router creation
- internal track creation
- rerouting incoming direct children to `__scroll_track`
- drag/input resolution
- scroll offset state
- track motion

### Router owns

- the actual sibling-to-target reroute behavior once configured

---

## 4. Input-source rule for scrolling

`ScrollingComponent` should resolve drag/input source in this order:

1. sibling `__bg` subtree renderable
2. nearest ancestor clip scope
3. nearest ancestor renderable
4. nearest ancestor transform

Why:

- layout-owned scrolling should drag from the visible viewport surface
- that viewport surface is `__bg`
- `Scrolling{}` should not need to live under `__bg`
- standalone scrolling must still work without layout helpers

### World-drag to scroll-local conversion

There is a separate problem from input-source discovery:

- `GestureSystem` currently produces drag deltas in world space
- `ScrollingComponent` currently applies that world-space Y delta directly to scroll offset
- layout-owned scrolling often lives under scaled UI transforms

That means the current drag response can feel too slow or too fast depending on inherited scale.

#### Cheapest likely fix

Keep scrolling state in its current local/layout units and convert the drag delta before applying
it.

Recommended shape:

1. keep `scroll_offset`, `viewport_height`, and `content_height` in scroll-local/layout units
2. at drag time, convert world-space drag motion into the scroll space's local Y units
3. apply that converted local Y delta to `ScrollingComponent::apply_drag(...)`

#### Why this is cheaper than world-unit-native scrolling

Making scrolling fully world-unit-native would ripple through:

- layout measurement outputs
- viewport/content height bookkeeping
- tests and examples that currently reason in layout/local units
- potential interactions with transform-pipeline scale filtering

By contrast, converting drag into scroll-local units is a smaller change because it is isolated to
the scrolling runtime.

#### Two plausible conversion strategies

##### Option A — use effective world Y scale

At drag time:

- inspect the scroll space's effective world Y scale
- divide world-space drag delta by that scale to get local/layout-space Y movement

Pros:

- cheap
- likely enough for current axis-aligned UI cases
- fits the existing scrolling model well

Cons:

- assumes scale-only correction is sufficient
- less general if rotation/shear-like cases ever matter

##### Option B — convert through matrices / local space

At drag time:

- use the relevant transform's world matrix
- convert the drag vector from world space into the scroll space's local basis
- then use the resulting local Y component

Pros:

- more geometrically correct
- naturally handles more than plain scale

Cons:

- slightly more implementation work
- requires choosing the exact scroll space whose basis should define local scroll motion

#### Current recommendation

The cheapest next step is:

- do **not** make `ScrollingComponent` world-unit-native yet
- do **not** add a persistent absolute-scale field first
- instead, convert `delta_world` into scroll-local Y at drag time

If a simple effective-scale conversion is not good enough, then move to matrix/local-basis
conversion.

#### Follow-up question to resolve in code

The implementation should make explicit which space defines “scroll-local Y”:

- the scroll wrapper root (`__scroll`)
- the scroll track parent space
- or another layout-defined content space

That choice should be documented when the drag conversion lands.

---

## 5. Hierarchies we want

The trees below distinguish:

- `[A]` authored by user / MMS / higher-level code
- `[L]` added by `LayoutSystem`
- `[S]` added by `ScrollingSystem`

### Overflow routing rule

- `overflow: Visible` / default: no relocation, no router required
- `overflow: Hidden`: authored children relocate into a layout-owned clipped-content branch
- `overflow: Scroll`: authored children relocate into scrolling, then scrolling relocates them into
  `__scroll_track`

For the hidden-only case, the exact helper label is still TBD in code. This doc uses
`__clip_content` as the conceptual name for the layout-owned clipped-content branch.

### A. Neither `background_color` nor `overflow`

```text
item_tc [A]
  Style { no background_color, overflow: Visible/default } [A]
  child_a [A]
  child_b [A]
```

Effect:

- no `__bg`
- no clipping
- no scrolling helper topology
- authored children stay directly under `item_tc`

### B. `background_color` only

```text
item_tc [A]
  Style { background_color = ... } [A]
  child_a [A]
  child_b [A]
  __bg [L]
    Transform [L]
    Color [L]
    Renderable [L]
```

Effect:

- layout adds only the visual background helper
- no clipping
- no scrolling helper topology
- authored children stay directly under `item_tc`

### C. `overflow("hidden")` only

```text
item_tc [A]
  Style { overflow: Hidden } [A]
  Router { target_name: "__clip_content" } [L]
  __bg [L]
    Transform [L]
    Color/Renderable as needed for clip surface [L]
    Raycastable(drag_only) only if layout wants a viewport hit surface [L]
  StencilClip [L]
  __clip_content [L]
    child_a [A]
    child_b [A]
```

Effect:

- layout adds `__bg`
- layout adds clipping
- layout adds a routed clipped-content branch
- authored children relocate under `__clip_content`
- content is clipped but not scrollable

### D. `overflow("scroll")` only

Target shape:

```text
item_tc [A]
  Style { overflow: Scroll } [A]
  Router { target_name: "__scroll" } [L]
  __bg [L]
    Transform [L]
    Color/Renderable [L]
    Raycastable(drag_only) [L]
  StencilClip [L]
  __scroll [L]
    ScrollingComponent [L]
    __scroll_router [S]
    __scroll_track [S]
      child_a [A]
      child_b [A]
```

Initial authored shape before routing:

```text
item_tc [A]
  Style { overflow: Scroll } [A]
  child_a [A]
  child_b [A]
```

Desired end state:

- layout adds outer router + `__bg` + clip + `__scroll`
- authored children are routed from `item_tc` into `__scroll`
- scrolling then routes them from `__scroll` into `__scroll_track`
- scrolling uses sibling `__bg` as the preferred input surface

### E. `background_color` + `overflow("hidden")`

```text
item_tc [A]
  Style { background_color = ..., overflow: Hidden } [A]
  child_a [A]
  child_b [A]
  __bg [L]
    Transform [L]
    Color [L]
    Renderable [L]
    Raycastable(drag_only) optional [L]
  StencilClip [L]
```

Effect:

- same as hidden-overflow case, but `__bg` is also visibly colored
- authored children remain under `item_tc`

### F. `background_color` + `overflow("scroll")`

This is expected to be the most common layout-owned scrolling case.

```text
item_tc [A]
  Style { background_color = ..., overflow: Scroll } [A]
  Router { target_name: "__scroll" } [L]
  __bg [L]
    Transform [L]
    Color [L]
    Renderable [L]
    Raycastable(drag_only) [L]
  StencilClip [L]
  __scroll [L]
    ScrollingComponent [L]
    __scroll_router [S]
    __scroll_track [S]
      child_a [A]
      child_b [A]
```

Effect:

- `__bg` is both visible background and preferred input/drag surface
- content ends up under `__scroll_track`
- clipping and scrolling both work from the same styled item

---

## 6. Checklist

### Scrolling on its own

- [x] auto-create `__scroll_router`
- [x] auto-create `__scroll_track`
- [x] reroute init-time children into `__scroll_track`
- [x] reroute late-attached children into `__scroll_track`
- [x] prefer sibling `__bg` for input when present
- [x] verify with standalone + layout-mock scrolling demo

### Layout-owned background/clipping

- [x] add `__bg` for `background_color`
- [x] add clip helper path for `overflow: Hidden | Scroll`
- [x] use layout-generated `__bg` as stencil clip geometry
- [x] add scroll drag surface under `__bg` renderable for overflow scroll items

### Layout-owned scrolling

- [ ] create routed clipped-content branch for `overflow: Hidden`
- [ ] route hidden-overflow authored children into that clipped-content branch
- [ ] create `__scroll` wrapper for `overflow: Scroll`
- [ ] attach `ScrollingComponent` under `__scroll`
- [ ] add outer `RouterComponent` targeting `__scroll`
- [ ] exclude helper/internal nodes from outer routing
- [ ] route authored children into `__scroll`
- [ ] let scrolling reroute them into `__scroll_track`
- [ ] compute/sync viewport height into `ScrollingComponent`
- [ ] compute/sync content height into `ScrollingComponent`
- [ ] verify `examples/diy-panel.mms` scrolls without authored `Scrolling{}`

### Runtime mutation / demo coverage

- [ ] add Rust-side add-item button to `examples/router.rs`
- [ ] add Rust-side add-item button to `examples/diy-panel.rs`
- [ ] verify newly attached items route through outer router and inner scrolling router
- [ ] verify scroll still works after runtime additions

---

## 7. Suggested implementation order

1. keep `ScrollingComponent` self-contained and stable
2. make `LayoutSystem` create `__scroll` + outer router for `overflow: Scroll`
3. make `diy-panel` scroll through layout-owned scrolling
4. add runtime insertion buttons to `router` and `diy-panel`
5. verify late attachment + scrolling together

---

## 8. Notes for a future agent

- The immediate problem is **not** blocked on the general query/parser architecture
- The near-term shape uses existing named routing conventions:
  - outer router targets `__scroll`
  - inner scrolling router targets `__scroll_track`
- `__bg` is intentionally separate from `__scroll`
  - `__bg` is the viewport/input/clip helper
  - `__scroll` is the content/runtime helper
- If `diy-panel` still does not scroll after layout owns `__scroll`, first inspect:
  - whether authored children are actually ending up under `__scroll_track`
  - whether `ScrollingComponent.viewport_height` and `content_height` are nonzero and sensible
  - whether the chosen drag scope is the `__bg` renderable as intended
