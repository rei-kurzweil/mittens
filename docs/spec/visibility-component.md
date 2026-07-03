# Visibility Component

## Goal

Define a general `VisibilityComponent` that controls whether renderable descendants
participate in rendering without removing them from the ECS/world topology.

This is intended to solve cases like:

- hide/show rows in `grid_panel`
- temporary runtime hiding of helper visuals
- future systems that need to exclude a subtree from rendering while preserving:
  - transforms
  - ownership/parenting
  - selection/topology relationships
  - serialization identity

This spec is specifically about **render visibility**, not deletion and not opacity.

## Motivation

Today we already have:

- `OpacityComponent`: controls translucency
- `GridComponent.enabled`: currently used as a grid-specific semantic enabled flag

Neither is the right general render-visibility contract:

- `OpacityComponent` means "how transparent is this if rendered"
- `enabled` on a domain component means "is this feature active"

Treating `opacity == 0` as "hidden" would overload transparency into a visibility
toggle, which becomes awkward for fades, animation, or intentionally transparent
objects that should still exist in renderer bookkeeping.

So we want an explicit visibility concept instead.

## Proposed component

Add:

- `VisibilityComponent`

Suggested shape:

```rust
pub struct VisibilityComponent {
    pub visible: bool,
}
```

Suggested constructors:

- `VisibilityComponent::visible()`
- `VisibilityComponent::hidden()`
- `VisibilityComponent::new()` defaulting to `visible: true`

## Semantics

`VisibilityComponent` affects whether descendant renderables are submitted to the
render path.

It does **not** by itself mean:

- deleted
- detached
- unselectable
- unserialized
- unsnappable

Those behaviors may choose to follow visibility in specific systems, but they are
not implied by the component itself.

## Inheritance and override rule

Visibility should follow the same general topology pattern we already use for other
inheritable render properties.

For a given `RenderableComponent`, effective visibility is resolved by:

1. Check for an immediate `VisibilityComponent` child on the renderable itself.
   - If present, that is the effective value.
2. Otherwise walk ancestors upward from the renderable's parent.
3. At each ancestor:
   - if the ancestor node itself is a `VisibilityComponent`, use it
   - else if the ancestor has an immediate `VisibilityComponent` child, use that
4. Nearest match wins.
5. If no match exists, default to visible.

This gives the intended override rule:

- child/local visibility overrides ancestor/group visibility

That means:

- a hidden parent subtree can still contain a locally re-shown branch if we allow
  a nearer `visible = true` override
- a visible parent subtree can hide a specific leaf renderable with a local
  `visible = false`

## Authoring/topology examples

### Group hide

```text
T "grid_root"
|- GridComponent
|- VisibilityComponent { visible: false }
|- T "grid_visual"
   |- T "grid_visual_shape"
      |- RenderableComponent
```

Result:

- the grid remains in the world
- the renderable under `grid_visual_shape` is not rendered

### Local override on a leaf

```text
T "group"
|- VisibilityComponent { visible: false }
|- T "child_a"
|  |- RenderableComponent
|- T "child_b"
   |- RenderableComponent
   |- VisibilityComponent { visible: true }
```

Result:

- `child_a` is hidden
- `child_b` is rendered

## Renderer contract

`VisibilityComponent` should affect whether a renderable is present in the visual
render world at all.

Practical rule:

- if effective visibility is `false`, the renderable should not appear in
  `VisualWorld` draw lists
- if effective visibility changes from `true -> false`, the renderable instance
  should be removed or otherwise fully excluded from render submission
- if effective visibility changes from `false -> true`, the renderable should be
  registered/re-registered normally

Important:

- this should **not** be modeled as "keep the instance but set opacity to zero"
- zero opacity may still be useful for rendering semantics and should remain
  distinct from visibility

## Suggested system ownership

Primary ownership should live in `RenderableSystem`, because that is already where
we resolve inheritable render properties like:

- color
- opacity
- transparent cutout

Suggested additions:

- `RenderableSystem::inherited_visibility_for_renderable(...)`
- registration/update paths that consult effective visibility before creating or
  retaining `VisualWorld` instances

This keeps visibility logic centralized rather than teaching domain systems
(`grid_system`, text, gizmos, previews, etc.) how to manually add/remove renderer
instances.

## Topology query pattern

This spec is compatible with the proposal in:

- `docs/task/refactor/topology-queries-and-style-inheritance.md`

Visibility should use the same family of helper queries as other inheritable render
properties.

In particular, it wants:

- immediate-child override
- nearest-ancestor fallback
- subtree propagation semantics during registration/update

## Relationship to selection and interaction

Visibility is render-only by default.

That means a hidden object may still be:

- selected by panel actions
- the owner of a gizmo target
- present in inspector/world listings

Whether hidden objects should be raycastable/selectable in scene space is a
separate policy decision and should not be hard-coded into `VisibilityComponent`.

If we later want "visible and interactive" as a combined authoring pattern, that
should likely be a higher-level helper or convenience API, not the base visibility
primitive.

## Relationship to grids

This component directly helps `grid_system` and `grid_panel`.

### Current problem

For grids, we want hide/show to:

- keep the grid in the component graph
- keep ownership mapping stable
- avoid deleting and respawning the grid subtree
- exclude the grid visual from rendering

Today `GridComponent.enabled` is already meaningful for grid behavior such as snap
selection. That is useful, but it is not a general renderer visibility API.

### Recommended grid usage

For the grid subtree:

- keep `GridComponent.enabled` as the grid-domain semantic flag
- add/use `VisibilityComponent` on the visual branch or owning transform

This gives a clean split:

- `GridComponent.enabled`: should this grid participate in grid behavior?
  - snapping
  - active-grid resolution
  - grid-specific editor logic
- `VisibilityComponent.visible`: should this grid's renderables be drawn?

### `grid_panel` behavior

`grid_panel` hide/show can then be implemented without special-casing renderer
behavior for grids:

1. Resolve row -> owning transform -> grid subtree
2. Toggle visibility by mutating `VisibilityComponent`
3. Rerender the panel row immediately
4. Let `RenderableSystem` update render participation

If desired, `grid_panel` can also choose to keep `GridComponent.enabled` and
`VisibilityComponent.visible` in sync for the first implementation.

That would mean:

- hide = not drawn
- hide also disables snapping

But the important point is that the renderer exclusion comes from the generic
visibility system, not a grid-specific hack and not opacity abuse.

## Non-goals

This spec does not define:

- a general-purpose `EnabledComponent` for all systems
- scene-raycast filtering rules for hidden objects
- serialization/editor UI defaults for hidden nodes
- animation of visibility over time

## Suggested first implementation steps

1. Add `VisibilityComponent`.
2. Teach `RenderableSystem` to resolve effective visibility with child override and
   ancestor fallback.
3. Ensure hidden renderables are excluded from `VisualWorld`, not merely rendered
   with `opacity = 0`.
4. Use the new component in grid visual subtrees.
5. Decide separately whether `grid_panel` hide/show should also flip
   `GridComponent.enabled`.

## Expected benefits

- One generic render hide/show primitive for the engine
- Cleaner separation between transparency and visibility
- No grid-specific renderer branching
- Better reuse for previews, helpers, widgets, and future editor tooling
- Stable world topology even when visuals are hidden
