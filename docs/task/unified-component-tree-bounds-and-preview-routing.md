# Unified component-tree bounds API and preview routing

## Context

The assets panel currently generates live previews for exported MMS factories, but panel-module previews are hard-disabled in `AssetSystem::build_asset_item_shell()` to avoid a scrolling interaction bug.

Current behavior:

- geometry/icon previews can be measured immediately with `BoundsSystem::calculate_subtree_local_bounds()`
- styled/layout-driven previews sometimes need a layout root plus a deferred remeasure pass
- preview subtrees have raycasting disabled so they do not steal pointer input from the assets panel scroll container
- panel previews are skipped entirely because a live embedded panel subtree appears to interfere with scrolling/hit routing

This leaves two missing pieces:

1. a unified API for measuring any component subtree's bounds
2. a clear routing model for interactive previews embedded inside another UI

## Problems

### 1. Bounds measurement is fragmented

The engine currently has reusable subtree AABB aggregation in `BoundsSystem`, but the higher-level "fit this arbitrary subtree into a target box" logic is duplicated in `AssetSystem`.

The hard parts are not the raw AABB math. The hard parts are:

- some subtrees only become measurable after layout has produced renderables
- some subtrees already include a layout root, while others need one inserted
- some content has meaningful width/height only after a layout tick
- current callers are inconsistent about fit policy:
  - one path fits to `max_dimension()`
  - another path fits to `width()`

### 2. Embedded panel previews interfere with scroll/hit routing

Today preview content is made non-raycastable by walking the preview subtree and disabling every `RaycastableComponent`.

That works for passive previews, but it is not a general interaction model:

- if the preview is non-raycastable, the outer assets panel can scroll normally
- if the preview is raycastable, inner panel controls may consume hits and block the parent scroll container
- if we want previews to be clickable in the future, we need explicit retargeting or event policy, not ad hoc component disabling

This is especially visible for panel previews, because they contain nested selectable UI, scroll regions, and option items.

## Goals

- Provide one reusable API to measure a component subtree's visual bounds.
- Support both immediately-renderable geometry and layout-driven UI subtrees.
- Preserve aspect ratio when fitting content into a target preview box.
- Define a routing policy for embedded previews so parent UI scrolling remains correct.
- Re-enable panel previews once routing behavior is understood and tested.

## Non-goals

- full generic event bubbling/capture for all systems in this task
- redesigning the entire layout engine
- making embedded previews fully interactive immediately

## Proposed measurement API

Add a higher-level helper above `BoundsSystem`, for example:

```rust
pub struct FitBox {
    pub max_width: f32,
    pub max_height: f32,
    pub max_depth: Option<f32>,
    pub center_z_offset: f32,
}

pub struct MeasuredSubtreeBounds {
    pub aabb: Aabb,
    pub required_layout_root: bool,
    pub resolved_after_layout: bool,
}

pub enum MeasureSubtreeResult {
    Ready(MeasuredSubtreeBounds),
    PendingLayout {
        layout_root: ComponentId,
    },
    Unmeasurable,
}

pub fn measure_component_tree_bounds(...)
pub fn fit_aabb_uniform(aabb: &Aabb, fit_box: FitBox) -> Transform
```

The API should:

- try direct subtree renderable bounds first
- detect whether the subtree needs a temporary or permanent `LayoutComponent`
- if layout is required, allow a deferred remeasure after layout tick
- return one uniform scale value, never non-uniform scale
- use a single fit policy consistently

## Fit policy to decide

We need one explicit policy for "fit within bounds without distortion":

- fit to max dimension
- fit to width/height independently and choose the smaller uniform scale
- optionally ignore depth for flat UI previews

Recommended default:

- use width/height as the limiting axes for UI preview tiles
- compute `scale = min(max_width / width, max_height / height)`
- ignore depth for panel/icon preview fitting unless a caller explicitly opts in

This is more stable than `max_dimension()` for flat UI content, while still preserving aspect ratio.

## Panel-preview routing investigation

We need to understand exactly why embedded panel previews break scrolling.

Questions to answer:

- Are preview descendants still being hit even after `RaycastableComponent.enable = false`?
- Are panel backgrounds or layout-generated renderables hit-testing through a separate path?
- Is the scroll container losing drag initiation because the pointer lands on a preview-owned renderable?
- Are nested `Selection` / `Option` subtrees mutating focus or capture state even when raycast is disabled?

## Candidate routing models

### Model A: passive preview only

- preview subtree never participates in raycasting
- all pointer input belongs to the outer assets panel
- simplest and likely correct for the current asset browser

This should be the baseline.

### Model B: preview hit proxy

- inner preview subtree stays non-raycastable
- outer preview shell is raycastable
- any click on the preview tile is handled by the outer shell
- outer shell may emit metadata such as "open preview", "focus asset", or "expand"

This allows click behavior without letting inner controls steal input.

### Model C: retargeted embedded interaction

- inner subtree can be raycastable
- hits inside the preview are intercepted and rewritten before normal dispatch
- parent assets panel decides whether the gesture should scroll, select the asset, or forward interaction to the embedded preview

This is the most flexible, but also the most complex. It likely requires explicit pointer-event policy and gesture ownership.

## Recommended implementation order

1. Extract shared measurement/fit helpers from `AssetSystem`.
2. Standardize one uniform fit policy for preview tiles.
3. Keep embedded previews passive by default.
4. Add instrumentation to trace hit targets when dragging over preview tiles.
5. Re-enable panel previews under passive routing first.
6. Only explore clickable embedded previews after passive scrolling is stable.

## Concrete work

- [ ] Extract the preview fit math from `AssetSystem` into a reusable helper.
- [ ] Add a unified subtree measurement entry point that can report `ready`, `pending layout`, or `unmeasurable`.
- [ ] Standardize fit-to-box policy for preview tiles.
- [ ] Add tracing for pointer hits and scroll gesture ownership when dragging over asset previews.
- [ ] Reproduce the panel-preview scrolling bug with logging enabled.
- [ ] Verify whether any preview descendant still participates in hit testing after raycast disable.
- [ ] Re-enable panel previews behind the passive-preview policy.
- [ ] Add tests for geometry preview measurement and layout-driven preview measurement.
- [ ] Add tests for preview tiles not blocking assets-panel scrolling.
- [ ] Add tests for preview-shell click proxy behavior if we choose Model B.

## Relevant files

- `src/engine/ecs/system/asset_system.rs`
- `src/engine/ecs/system/bounds_system.rs`
- `src/engine/ecs/system/system_world.rs`
- `src/engine/ecs/system/scroll_system.rs`
- `src/engine/ecs/system/selection_system.rs`
- `src/engine/ecs/system/layout/*`
- `assets/components/asset_item.mms`

## Success criteria

- Any previewable component tree can be measured through one supported API.
- Fit math is shared and consistent across direct and deferred preview measurement.
- Panel previews can be rendered in the assets panel without breaking scroll behavior.
- Preview interactivity policy is explicit rather than encoded as one-off raycast disabling.
