# Component-tree bounds measurement V1

This draft proposes a small, explicit API in `BoundsSystem` for measuring
component subtrees.

The key design choice is to split the problem into two methods:

- a narrow method for subtrees that are already measurable from renderables
- a layout-aware method for styled/UI trees that may need a temporary or
  persistent layout root before they produce useful bounds

V1 should land the first method cleanly and define the second method's
contract without forcing the engine to fully solve layout simulation in the
same change.

## Why split the API

We currently have one low-level primitive:

- `BoundsSystem::calculate_subtree_local_bounds(world, render_assets, root)`

That function is useful, but it only works once a subtree already contains
`RenderableComponent`s with usable mesh bounds. That covers icons, raw
geometry, and many previewable non-UI assets.

It does not by itself solve styled trees whose visible bounds only emerge
after layout has run.

Trying to hide both cases behind one magical method would blur two different
 operations:

1. measure what already renders
2. make a subtree measurable by resolving layout first

Those are related, but they are not the same operation and should not share
the same failure semantics.

## Goals

- Provide one obvious V1 entry point for renderable subtree bounds.
- Keep the V1 API reusable for asset previews, icon buttons, and editor UI.
- Preserve a path for later layout-aware measurement without changing the V1
  mental model.
- Avoid implicit mutation or hidden layout bootstrapping in the narrow API.

## Non-goals

- solving all styled/UI measurement in the first implementation
- adding hidden layout ticks inside `BoundsSystem`
- making the basic method spawn, attach, or mutate world state

## Existing behavior

`BoundsSystem::calculate_subtree_local_bounds(...)` walks a subtree,
accumulates transformed AABBs from `RenderableComponent`s, and returns a
root-relative aggregate `Aabb`.

This already gives us:

- `width()`
- `height()`
- `depth()`
- `center()`
- `max_dimension()`

That is enough to build reusable fit-to-box helpers for non-styled trees.

## Proposed API

### Method 1: renderable-only measurement

This is the actual V1 method.

```rust
pub enum RenderableBoundsMeasure {
    Measured(Aabb),
    Unmeasurable,
}

pub fn measure_renderable_subtree_bounds(
    world: &World,
    render_assets: &RenderAssets,
    root: ComponentId,
) -> RenderableBoundsMeasure
```

Semantics:

- walks the existing subtree only
- does not mutate the world
- does not create a layout root
- does not tick layout
- succeeds only if the subtree already exposes measurable renderable bounds

`Unmeasurable` means:

- the subtree has no measurable renderable bounds yet
- or the subtree's visual bounds depend on unresolved layout/text state
- or the subtree contains only structures this method intentionally ignores

The important property is that `Unmeasurable` is not an error. It is a valid
answer that tells the caller to choose another path.

### Method 2: layout-aware measurement

This is the explicitly broader method. It may arrive after V1, but the shape
should be named now so downstream callers do not overfit to the narrow API.

```rust
pub enum LayoutAwareBoundsMeasure {
    Measured(Aabb),
    PendingLayout {
        layout_root: ComponentId,
    },
    Unmeasurable,
}

pub fn measure_layout_aware_subtree_bounds(
    world: &mut World,
    render_assets: &RenderAssets,
    root: ComponentId,
    mode: LayoutMeasureMode,
) -> LayoutAwareBoundsMeasure
```

Possible `LayoutMeasureMode` values:

- `RequireExistingLayout`
- `InsertTemporaryLayoutRoot`
- `InsertPersistentLayoutRoot`

Semantics:

- may inspect style/layout structure
- may decide that the subtree needs a layout root before bounds can exist
- may return `PendingLayout` instead of forcing synchronous completion
- may mutate the world if the chosen mode allows layout-root insertion

This method is intentionally not part of the narrow V1 implementation target.
It is a separate capability with different lifecycle and ownership concerns.

## Why two methods instead of one enum-heavy method

A single "do everything" API would encourage misuse:

- call sites that only need geometry bounds would pay conceptual cost for
  layout state they do not care about
- callers might accidentally trigger world mutation during what looks like a
  pure measurement query
- failure modes would become ambiguous

Two named methods keep call sites honest:

- if you want pure measurement, use the pure method
- if you want layout-assisted measurement, opt in explicitly

## Declarative API

The low-level measurement split should stay in `BoundsSystem`, but MMS
authoring should expose one declarative wrapper: `FitBounds`.

Preferred shape:

```mms
FitBounds.to([-0.5, -0.5, -0.5, 0.5, 0.5, 0.5]) {
    pencil_icon()
}
```

Preferred extensible direction:

```mms
FitBounds.renderable_only().to([-0.5, -0.5, -0.5, 0.5, 0.5, 0.5]) {
    pencil_icon()
}

FitBounds.layout_aware().to([-0.5, -0.5, -0.5, 0.5, 0.5, 0.5]) {
    styled_button("Save")
}
```

`to([min_x, min_y, min_z, max_x, max_y, max_z])` defines the target local
`Aabb` that the child subtree should fit inside.

Rules:

- always use uniform scale
- never distort aspect ratio
- translate from measured bounds center to target box center
- fit the measured subtree inside the target box rather than scaling the box
  itself

This wrapper is intentionally declarative and modeful. It is not a special
case of `T.scale(...)`, and it is not just a one-off transform helper. The
author is declaring "fit this subtree into these bounds," while the engine
chooses the measurement path based on the selected mode.

## Low-level mapping

`FitBounds` is the MMS-facing facade over the two low-level measurement
paths.

- `FitBounds.renderable_only()` maps to
  `measure_renderable_subtree_bounds(...)`
- `FitBounds.layout_aware()` maps to
  `measure_layout_aware_subtree_bounds(...)`

The important separation is:

- low-level Rust keeps two honest methods with different contracts
- MMS exposes one user-facing concept with explicit modes

V1 implementation priority should be `FitBounds.renderable_only()`. That
path covers icon/geometry trees and stays aligned with the pure
renderable-only API.

## Why `FitBounds`

`FitBounds` names the user intent: fit child content into a target box.

`ScaleBounds` was considered and rejected because it sounds like scaling the
box itself, not fitting content into it. The mechanism may involve scaling,
but the declarative concern is fitting.

Keeping one wrapper name also avoids proliferating MMS surface area. Authors
learn one concept and then opt into broader behavior with modes only when
needed.

## Caller guidance

### Asset previews

For icon/geometry previews:

1. call `measure_renderable_subtree_bounds(...)`
2. if measured, call `fit_aabb_uniform(...)`
3. if unmeasurable, fall back to a layout-aware preview path or placeholder

For styled panel/button previews:

1. do not use the narrow method as if it were broken
2. treat `Unmeasurable` as expected
3. switch to the layout-aware path if that preview class needs support

### Paint-panel icons

Paint-panel icon sizing is a good example of the narrow path:

- the icons are geometry
- they already produce renderable bounds
- no layout simulation is needed

That is exactly the kind of caller V1 should support first.

In MMS terms, that same case should be representable as:

```mms
FitBounds.renderable_only().to([-0.5, -0.5, -0.1, 0.5, 0.5, 0.1]) {
    pencil_icon()
}
```

## Failure semantics

The API should distinguish these outcomes:

- "measured successfully"
- "not measurable through this method"

It should not collapse them into guessed transforms or arbitrary fallback
scales. Fallback display policy belongs in the caller.

That means:

- no hard-coded `0.5` scale from the measurement method
- no auto-created placeholder transforms
- no hidden side effects in the pure method

For `FitBounds.renderable_only()`, this means the declarative layer should
also preserve the narrow method's honesty:

- if bounds are available, compute the uniform fit transform
- if bounds are unavailable, produce no measured fit
- do not invent a fallback scale
- do not imply hidden layout simulation or world mutation

Any placeholder rendering, deferred loading state, or fallback presentation
belongs to the caller or presentation layer.

For `FitBounds.layout_aware()`, broader behavior is acceptable because the
mode makes that tradeoff explicit. It may require a layout root and may only
complete after a layout tick. That mode is intentionally broader than the V1
implementation scope.

## Examples

### Renderable icon subtree

```mms
FitBounds.to([-0.5, -0.5, -0.1, 0.5, 0.5, 0.1]) {
    pencil_icon()
}
```

This is the primary V1 case. The subtree already has renderable geometry, so
the engine can measure existing bounds and apply a centered uniform fit.

### Future layout-aware styled subtree

```mms
FitBounds.layout_aware().to([-1.0, -0.4, -0.1, 1.0, 0.4, 0.1]) {
    styled_button("Save")
}
```

This is intentionally not hidden behind the default path. A styled button or
panel may need layout participation before meaningful bounds exist, so the
author opts into the broader measurement contract explicitly.

## Why one wrapper with modes

- declarative authoring stays simple
- low-level engine semantics stay explicit
- avoids proliferating wrapper names
- preserves honest measurement contracts

This keeps the API surface aligned across layers:

- engine code sees the real measurement split
- MMS authors see one fitting concept
- layout-aware behavior is opt-in instead of ambient

## Interaction with existing code

Likely mapping:

- keep `calculate_subtree_local_bounds(...)` as the low-level implementation
  detail
- add `measure_renderable_subtree_bounds(...)` as the stable higher-level API
- extract preview fit math out of `AssetSystem` into a shared helper

`AssetSystem` then becomes a normal caller instead of the owner of the
policy.

## Open questions

### Should the pure method return `Option<Aabb>` instead?

It could, but an explicit enum reads better once there are two measurement
paths in the codebase. `Unmeasurable` is more communicative than `None`.

### Should the layout-aware method live in `BoundsSystem`?

Probably yes if its responsibility stays "produce visual bounds for a
subtree." If it grows into "manage preview measurement lifecycle," that is
too broad and should move into a higher-level preview/measurement service.

### Should text-only trees be measurable without layout?

Probably not in V1. Text measurement is a layout concern in this engine.
Pretending otherwise would create inconsistent sizing rules between layout and
preview code.

## Recommended implementation order

1. Add `measure_renderable_subtree_bounds(...)` as a pure wrapper over the
   existing subtree AABB walk.
2. Add `fit_aabb_uniform(...)` and move shared preview-fit math to it.
3. Update geometry/icon callers to use the new pair.
4. Leave styled/layout-driven measurement on the existing special-case path
   until the layout-aware API is ready.
5. Add the second method only when its mutation and lifecycle semantics are
   nailed down.

## Success criteria

- The engine has one obvious pure API for measuring already-renderable
  subtrees.
- Geometry/icon callers no longer duplicate fit math.
- The code clearly distinguishes "not measurable through the pure path" from
  "bounds system failed."
- A later layout-aware API can be added without changing the meaning of the
  narrow V1 method.
