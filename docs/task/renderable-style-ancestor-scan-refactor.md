# Renderable Style Ancestor Scan Refactor

Date: 2026-06-14

Status: open

Related:

- `docs/spec/visibility-component.md`
- `docs/refactor/topology-queries-and-style-inheritance.md`
- `docs/task/grid-panel-select-delete-hide-and-gizmo.md`

## Goal

Refactor `RenderableSystem` so render-style ancestry for a renderable is resolved
through one shared pass instead of multiple separate ancestor walks.

This should land before adding `VisibilityComponent`, so visibility does not
become one more ad hoc ancestry query in an already fragmented render-style path.

## Why this first

We just re-verified that renderable setup currently does **not** have one unified
"effective render style" resolution step.

Instead, `RenderableSystem` performs several separate traversals for different
render-related properties:

- background
- overlay
- color
- opacity
- transparent cutout

That means a future `VisibilityComponent` would either:

- add yet another ancestor traversal, or
- have to be threaded through several separate code paths

Neither is a good foundation.

## Current behavior

In `src/engine/ecs/system/renderable_system.rs`, effective render state is
currently split across independent helpers:

- `inherited_background_for_renderable(...)`
- `inherited_overlay_for_renderable(...)`
- `inherited_color_for_renderable(...)`
- `inherited_opacity_for_renderable(...)`
- `inherited_cutout_for_renderable(...)`

These are not resolved in one scan.

Typical flow today:

1. `register_renderable_from_world(...)`
   - resolves/inherits color
   - resolves/inherits opacity
2. later `flush_pending(...)`
   - resolves/inherits cutout
   - resolves/inherits background
   - resolves/inherits overlay

So the same renderable ancestry can be walked multiple times during setup.

## Problems

- repeated ancestor traversal logic
- repeated precedence logic
- render-style semantics are spread across pending registration and flush
- adding a new inheritable render property is more work than it should be
- it is harder to reason about override rules consistently

For the grid work specifically, this makes visibility design harder than it needs
to be.

## Refactor target

Introduce one shared render-style resolution path for a renderable.

Practical rule:

- given a `RenderableComponent`, compute its effective render-style state once
- use that resolved state for initial registration into `VisualWorld`
- use the same logic for later updates

Suggested shape:

```rust
struct EffectiveRenderableStyle {
    color: [f32; 4],
    opacity: PendingOpacity,
    transparent_cutout: bool,
    background: bool,
    background_occluded_lit: bool,
    overlay: bool,
    // visibility can be added later
}
```

Suggested entrypoint:

- `resolve_effective_renderable_style(world, renderable_cid) -> EffectiveRenderableStyle`

That helper may still use small internal helpers, but the important contract is:

- ancestry is walked once
- all relevant render-style fields are checked while walking

## Scope

This task is about **render-style resolution**, not about introducing new public
component semantics yet.

In scope:

- unify render-style ancestor scanning
- unify override/default logic where practical
- make `register_renderable_from_world(...)` and `flush_pending(...)` consume the
  shared result
- prepare a clean insertion point for future `VisibilityComponent`

Out of scope:

- changing the authored meaning of `ColorComponent`
- changing the authored meaning of `OpacityComponent`
- deciding final `VisibilityComponent` topology rules
- changing grid panel behavior directly

## Design constraints

### Keep existing behavior first

The first refactor should preserve current visible behavior.

That means if current style semantics are:

- immediate child override on the renderable
- ancestor-attached fallback for some properties

the unified scan should preserve that behavior initially, even if we later choose
to simplify it.

### Separate refactor from semantic cleanup

Do not combine:

- "make it one pass"
- "change what counts as inherited"

in the same step unless the current code makes that unavoidable.

We want the refactor to reduce risk before we change visibility semantics.

## Suggested implementation shape

1. Identify the complete set of render-style fields currently resolved during
   renderable registration.
2. Introduce an internal struct to hold the effective result.
3. Add a single resolver that:
   - checks renderable-local immediate child overrides
   - walks ancestors once
   - accumulates first-match/nearest-match results for each relevant field
4. Update `register_renderable_from_world(...)` to seed pending state from that
   unified result.
5. Update `flush_pending(...)` to consume the same resolved data instead of
   re-running separate style queries.
6. Keep tests green before introducing `VisibilityComponent`.

## Why this helps `VisibilityComponent`

Once this refactor exists, visibility becomes much simpler to add:

- add one new field to effective render-style state
- add one resolution rule
- teach renderable registration/update to respect it

Instead of visibility needing to bolt onto several scattered ancestry paths, it
gets one obvious home.

That is especially useful for grids, because `grid_panel` hide/show wants a
generic render participation toggle, not more grid-specific renderer logic.

## Why this helps grids

The grid work needs a clean answer to:

- how do we keep a grid in the world graph
- while excluding its visual subtree from rendering
- without abusing opacity

If renderable effective state is already centralized, grid visibility can use the
same generic mechanism as any other renderable subtree.

That reduces the chance of:

- special-casing `grid_visual`
- duplicating removal/re-registration logic in panel handlers
- coupling `grid_panel` directly to `VisualWorld`

## Open questions

1. Should background/overlay stay as independent ancestor-only properties, or be
   folded into the same shared style-resolution struct for consistency?
2. Should emissive also move into the same effective-style pass in this refactor,
   or remain separate for now?
3. Should the first refactor preserve the current mix of immediate-child vs
   ancestor-attached style semantics exactly, or normalize some of them while the
   code is being consolidated?

## Acceptance

- render-style ancestry for a renderable is resolved through one shared path
- `register_renderable_from_world(...)` and `flush_pending(...)` no longer each
  perform separate ad hoc style ancestry lookups for the same renderable
- existing render appearance remains unchanged
- the result leaves a clear insertion point for `VisibilityComponent`
