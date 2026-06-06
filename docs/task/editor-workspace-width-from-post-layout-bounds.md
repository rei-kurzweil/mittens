# Editor workspace width should come from post-layout bounds

Date: 2026-06-06

Status: planning only.

This is a docs/task note only. No `src/` changes are proposed here yet.

## Goal

Stop guessing the shared editor workspace `LayoutRoot.available_width` up front.

Instead:

- give the workspace layout root a deliberately large temporary width budget
- let the layout system place all panel children
- measure how far right the laid out workspace content actually extends
- feed that measured extent back into the workspace `LayoutRoot.available_width`
- let `editor_context_system` own that final width update once the shared panel tree has finished
  loading/materializing

## Problem

The current width computation for the shared editor workspace is fragile because it tries to infer
the required width before layout has resolved the actual panel subtree.

That is the wrong level of abstraction for this UI:

- panel widths can come from authored `Style` / `LayoutRoot` content
- shared panels may attach late or rebuild after startup
- internal layout helpers such as backgrounds and scroll/clip helpers only exist after layout runs
- title bars, content regions, and panel chrome can change the effective rightmost extent in ways
  that are awkward to predict from setup-time arithmetic

The result is a finicky width contract:

- sometimes the workspace root is too wide and produces oversized clip/debug geometry
- sometimes it is too narrow or requires ad hoc constants
- ownership is unclear because the code estimating width is not the system that actually knows the
  final laid out geometry

## Proposed direction

Treat workspace width as a post-layout measurement problem, not a pre-layout estimate problem.

### Phase 1: layout with an intentionally wide root

When the shared editor workspace `LayoutRoot` is created, assign it a wide temporary
`available_width` that is large enough to avoid constraining ordinary editor panel placement.

This width is not the final contract. It is only the initial measurement budget.

### Phase 2: read the laid out right edge

After layout has run and the shared panel subtree has materialized, inspect the layout-managed
workspace subtree and compute the furthest-right occupied extent.

The measurement should come from layout/runtime state that already exists after layout:

- `LayoutBoundsComponent` for layout-owned item boxes
- resolved transform world matrices for those items

Concretely, the system should compute something equivalent to:

- for each relevant laid out workspace child/item:
  - take its padding/content box
  - transform that box into world space
  - read the maximum X extent
- choose the maximum right edge across the workspace subtree

The important design point is that the answer comes from the actual laid out geometry, not from
duplicated panel-width arithmetic in editor setup code.

### Phase 3: convert that extent back into workspace width

Once the furthest-right world-space extent is known, convert it back into the workspace root's
expected width space and set `LayoutRoot.available_width` to that measured amount.

That update should become the authoritative workspace width for subsequent layout passes.

### Phase 4: make `editor_context_system` own the update

`editor_context_system` is the right owner for this because it already tracks shared editor
workspace state and panel-query-root registration.

The intended responsibility split is:

- layout system: compute actual layout geometry
- transform system: provide valid world transforms
- editor workspace/context layer: decide when the shared panel tree is sufficiently loaded to
  promote measured geometry into the workspace width contract

## Timing requirement

This measurement must happen after the relevant shared editor panels have loaded, attached, and
completed at least one layout pass.

That means this should not run merely when the workspace root is spawned.

The workspace-width update needs an explicit "ready enough to measure" moment, such as:

- after shared panel materialization is complete
- after late-attached shared panel content has routed into the host subtree
- after the first layout pass that created any needed helper topology

If panel rebuilds later change the rightmost extent materially, the same measurement path should be
able to rerun and refresh the workspace width.

## Why this is better

- removes duplicated panel-width heuristics from editor setup code
- makes the workspace width follow actual panel layout, including authored MMS changes
- aligns the width contract with the system that already knows the final geometry
- gives clipping/debug geometry a narrower and more truthful workspace envelope
- scales better as more shared panels or panel variants are added

## Constraints and cautions

- Do not read only authored child widths; use resolved layout boxes after layout.
- Be explicit about which subtree counts toward workspace width so helper nodes do not inflate the
  measurement accidentally.
- Be careful about unit conversion:
  - layout widths are stored in glyph units
  - transforms and world extents are in world units
  - the fed-back `available_width` must end up in the root's expected unit space
- Avoid a feedback loop where tiny measurement drift keeps marking the workspace layout root dirty
  every frame.
- Prefer a one-way stabilization rule:
  - measure
  - update if materially different
  - relayout
  - then remain stable until topology/layout content changes again

## Suggested implementation shape

1. identify the shared workspace layout root and the subtree whose children represent real panel
   occupancy
2. start it with a large temporary `available_width`
3. after shared panel load/materialization and a completed layout pass, collect the relevant laid
   out item bounds
4. compute the maximum right-edge world X
5. convert that into workspace width units
6. update the workspace `LayoutComponent`
7. only repeat when the shared panel topology or measured width meaningfully changes

## Acceptance criteria

- the shared editor workspace no longer relies on setup-time width arithmetic as the primary source
  of truth
- workspace width is derived from actual laid out panel geometry
- the measured width update runs only after the panel subtree is ready to measure
- the width settles instead of oscillating every frame
- adding/removing/resizing shared panels changes the measured workspace width without new ad hoc
  constants

## Open questions

- What exact subtree should count toward workspace width:
  - immediate shared panel shells only
  - all styled descendants
  - all descendants except reserved helper nodes such as `__bg`, `__scroll`, router helpers, and
    clip helpers?
- Should the measurement use `padding_local` or `content_local` from `LayoutBoundsComponent` as
  the canonical visible width?
- What is the best readiness signal for "everything loaded":
  - panel registration count
  - a bootstrap/materialization event
  - one-shot delayed measurement after first clean layout
  - explicit dirty/version tracking on the shared panel host?
- Should width ever shrink automatically after panels are removed, or only grow until the next full
  workspace rebuild?

## Relevant files

- [src/engine/ecs/system/editor_context_system.rs](../../src/engine/ecs/system/editor_context_system.rs)
- [src/engine/ecs/system/layout/mod.rs](../../src/engine/ecs/system/layout/mod.rs)
- [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs)
- [src/engine/ecs/component/layout.rs](../../src/engine/ecs/component/layout.rs)
- [src/engine/ecs/component/layout_bounds.rs](../../src/engine/ecs/component/layout_bounds.rs)
- [src/engine/ecs/system/transform_system.rs](../../src/engine/ecs/system/transform_system.rs)

## Related

- [docs/bugs/layoutroot-available-width-does-not-constrain-explicit-panel-widths.md](../bugs/layoutroot-available-width-does-not-constrain-explicit-panel-widths.md)
- [docs/bugs/panel-stencil-geometry.md](../bugs/panel-stencil-geometry.md)
- [docs/task/shared-editor-ui-routing-and-paint-state-manager.md](./shared-editor-ui-routing-and-paint-state-manager.md)
- [docs/task/editor_context_issues.md](./editor_context_issues.md)
