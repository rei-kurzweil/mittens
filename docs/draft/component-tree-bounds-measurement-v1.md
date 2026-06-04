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

## Fit-to-box helpers

The measurement methods should not also decide preview scaling policy.
That logic should sit beside them as a separate helper.

Suggested helper:

```rust
pub struct FitBox {
    pub max_width: f32,
    pub max_height: f32,
    pub max_depth: Option<f32>,
    pub z_offset: f32,
}

pub struct UniformFitTransform {
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

pub fn fit_aabb_uniform(aabb: &Aabb, fit_box: FitBox) -> UniformFitTransform
```

Rules:

- always use uniform scale
- never distort aspect ratio
- center the subtree using `aabb.center()`
- default UI-preview policy should fit against width/height and choose the
  smaller scale
- depth should be opt-in for flat UI content

This keeps `BoundsSystem` responsible for measurement, not presentation.

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
