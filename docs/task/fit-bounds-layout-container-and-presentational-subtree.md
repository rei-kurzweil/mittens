# FitBounds layout-container targeting and presentational subtree split

## Context

The current `FitBounds` implementation has already failed in two different ways:

- first as a wrapper node that inserted a non-`TransformComponent` parent into authored UI trees
- then as a modifier that directly rewrote a layout-owned parent transform after layout

Both designs cross an ownership boundary that the current UI/runtime stack depends on:

- `LayoutSystem` owns the box model and placement of styled `TransformComponent` items
- `FitBounds` should only fit presentational content into a target box
- renderable subtree measurement and layout box measurement are not the same thing

The paint panel icon use case exposed the issue clearly. Even though `FitBounds` was only authored in `paint_panel_item`, enabling it caused breakage in world-panel rows, asset-panel content, and text input. That is evidence that the implementation is interfering with shared UI transform/layout/clipping assumptions, not that the authored usage is shared across panels.

## Goal

Redesign `FitBounds` so it reads a container box produced by layout and fits a renderable subtree inside that box without becoming layout-owned itself.

## Non-goals

- making `FitBounds` responsible for layout measurement
- allowing `FitBounds` to mutate the transform that layout uses as its final placement result
- solving the future `layout_aware()` content-measurement mode in this task

## Desired authored model

Preferred container-targeting form:

```mms
T {
    Style { ... }
    FitBounds.to_container() {
        T {
            icon
        }
    }
}
```

Intended semantics:

- the styled `T` defines the container box
- `LayoutSystem` computes that box
- `FitBounds` reads the computed container bounds
- `FitBounds` owns the presentational subtree inside its body
- `FitBounds` measures only that renderable/icon subtree
- `FitBounds` applies a uniform centered fit inside the container box
- `FitBounds` does not change layout ownership or layout measurement rules

## Core design split

There are two separate bounds sources:

### 1. Container bounds

These come from layout, not from renderables.

We likely need layout-written local bounds on layout-managed transforms, for example:

- a `computed_bounds` field on `TransformComponent`, or
- a small layout-owned component attached to the transform, such as `LayoutBoundsComponent`

This data should represent the resolved local box for the styled node after layout:

- min/max in the transform's local space
- content/padding/box semantics should be explicit
- `FitBounds.to_container()` should target a clearly-defined box, likely the resolved padding box or content box

### 2. Content bounds

These come from renderable subtree measurement.

For V1 `FitBounds`, content measurement should stay renderable-only:

- walk the presentational subtree
- union renderable bounds in local space
- do not simulate layout
- if no renderable bounds exist, fitting is unmeasurable

## Ownership model

`FitBounds` should be a presentational wrapper under a styled container, not a layout item.

The safe ownership shape is:

- outer styled transform: layout-owned container
- `FitBounds` child: reads container bounds from the parent styled transform
- inner presentational transform: fit-owned content transform inside `FitBounds`
- renderable icon subtree under that presentational transform

What must not happen:

- `FitBounds` inserting a non-transform wrapper into authored UI content
- `FitBounds` becoming a layout item
- `FitBounds` directly mutating the transform whose placement/size is owned by layout

## Proposed runtime shape

Conceptually:

```mms
T {
    Style { ... }
    FitBounds.to_container() {
        T {
            icon
        }
    }
}
```

But the important part is the ownership contract, not the exact sugar:

- one node provides the layout box
- one `FitBounds` child owns the content to be fitted
- one presentational child transform inside `FitBounds` is the fit target
- the fitted subtree lives under that presentational child

If sugar needs to lower authored content into this shape automatically, that lowering should preserve layout ownership and avoid inserting a non-transform parent where layout expects transform children.

## Implementation direction

### Step 1: add layout-written bounds on layout-managed transforms

`LayoutSystem` should publish the resolved local box for each styled layout item in a stable place that other systems can read.

Questions to settle:

- should the stored bounds represent content box, padding box, or border box?
- should the storage live on `TransformComponent` or a separate component?
- how should non-layout transforms report "no layout bounds"?

Recommended direction:

- use a separate layout-owned data holder to keep the contract explicit
- store a local-space AABB
- make absence explicit for non-layout nodes

### Step 2: retarget `FitBounds` to container bounds

`FitBounds.to_container()` should:

- find the layout-owned container bounds on its parent styled transform
- measure renderable content bounds from the subtree owned by `FitBounds`
- compute centered uniform fit from content AABB into container AABB
- write only to the fit-owned presentational transform

### Step 3: keep `FitBounds` out of layout ownership

The fit target transform should be a dedicated presentational child, not the layout-owned host transform itself.

That means:

- layout owns host transform placement
- fit owns content transform translation/scale
- later systems see a stable separation of responsibilities

## Open design questions

### Which box should `to_container()` mean?

Candidates:

- content box
- padding box
- border box

Recommended default for icons inside styled tiles:

- padding box if icons are intended to fill the visible interior
- content box if padding should remain reserved empty space

This must be stated explicitly in the implementation contract.

### How should the presentational child be identified?

Recommended answer:

- require an explicit authored subtree inside `FitBounds`
- the first authored transform inside that body becomes the fit target, or
- `FitBounds` lowers its body into a dedicated presentational transform it owns

The key point is that `FitBounds` should own the subtree it fits, rather than guessing among its styled parent's other layout children.

## Concrete work

- [ ] Add a layout-owned local-bounds data path for styled layout items.
- [ ] Decide whether `to_container()` targets content box or padding box.
- [ ] Refactor `FitBounds` so it no longer mutates the layout-owned host transform.
- [ ] Introduce a dedicated presentational fit target transform for fitted content.
- [ ] Restrict V1 content measurement to renderable-only subtree bounds.
- [ ] Define failure behavior when container bounds or content bounds are unavailable.
- [ ] Update paint-panel icon usage to the final supported authored shape.
- [ ] Add regression coverage ensuring `FitBounds` in paint panel does not break world panel, asset panel, or text input rendering.

## Relevant files

- `src/engine/ecs/system/layout/block.rs`
- `src/engine/ecs/system/layout/measure.rs`
- `src/engine/ecs/system/fit_bounds_system.rs`
- `src/engine/ecs/system/bounds_system.rs`
- `src/engine/ecs/component/fit_bounds.rs`
- `src/meow_meow/component_registry.rs`
- `assets/components/panel_items.mms`
- `assets/components/panels.mms`

## Success criteria

- `FitBounds` can target a layout-computed container box without participating in layout ownership.
- The styled/layout-owned transform is not mutated by fit logic.
- Renderable/icon content is fitted through a dedicated presentational transform.
- Adding `FitBounds` to paint-panel icons does not regress world panel, asset panel, or text input rendering.
