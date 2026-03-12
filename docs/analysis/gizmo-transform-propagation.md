# Gizmo transform propagation (why gizmos get squished)

## Problem statement

You observed that after making the desktop-controls hint selectable (by adding an invisible raycastable “pick plane”), the transform gizmo becomes **massively squished**.

This document explains:

- how the editor decides where to attach the gizmo,
- how world matrices propagate today,
- what *scale compensation* exists (and what it does not compensate),
- why non-uniform target scales distort the gizmo,
- design options to make gizmos avoid inheriting transforms (especially scale).

This is an **analysis-only** doc: no code changes.

## What happens today

### 1) The editor picks the “target transform” by topology

`EditorSystem` installs a `DragStart` handler scoped to an `EditorComponent` subtree.

On click:

- It receives `DragStart { renderable, .. }`.
- It computes `nearest_transform_ancestor(world, renderable)`.
- It reparents the editor gizmo under that transform via `IntentValue::Attach { parents: vec![target_transform], child: gizmo }`.

Key detail: **“nearest transform ancestor” is literally the first `TransformComponent` found while walking upward from the hit renderable.**

So if you click on a renderable that sits under a helper transform (like a pick plane), the gizmo is attached under that helper transform — not under the higher-level object you may have intended.

### 2) The gizmo binds its runtime target from its new parent

`TransformGizmoSystem` treats gizmo targeting as “the transform above me”:

- On registration it walks up ancestry to find the nearest ancestor `TransformComponent`.
- It binds `TransformGizmoComponent.target_transform = Some(that_transform)`.

In practice, after the editor reparents the gizmo, the gizmo’s `target_transform` becomes “the transform you attached it under”.

### 3) World matrices propagate by multiplying ancestor transforms

`TransformSystem` maintains cached world matrices (`TransformComponent.transform.matrix_world`).

When a transform changes, it:

- recomputes `matrix_world` for the transform and all descendant transforms as:

  $$M_{world}(child) = M_{world}(parent) \cdot M_{local}(child)$$

- updates VisualWorld instance matrices for descendant renderables using the nearest transform’s cached world matrix.

There is currently **no opt-out**: descendants inherit the full parent transform matrix (including non-uniform scale).

## Existing gizmo scale compensation (and its limits)

The gizmo visuals are created as a subtree under the gizmo component.

Since the gizmo component is parented under the chosen target transform, the visuals inherit the target’s world scale.

To prevent gizmos from becoming tiny when the target is scaled (common in armature/joint hierarchies), `TransformGizmoSystem` applies a compensating **uniform local scale** to the gizmo visual root:

- It computes the target’s current world model matrix (uncached) and derives a single scalar:
  - `parent_world_scale = max_basis_scale(parent_world)`
  - where `max_basis_scale` takes the max length of the first three basis columns.
- It sets:

  $$s_{local} = \frac{s_{requested\_world}}{\max(\lVert b_x \rVert,\lVert b_y \rVert,\lVert b_z \rVert)}$$

This makes the gizmo approximately constant-size in world space.

### Why this still squishes

If the parent has **non-uniform scale**, a single scalar cannot remove the anisotropy.

Example (idealized, no rotation):

- parent scale: $(s_x, s_y, s_z) = (13, 4.5, 1)$
- `max_basis_scale = 13`
- gizmo root local scale becomes $1/13$ (times the desired world scalar)

Then the gizmo’s effective world scale becomes:

- along X: $13 \cdot (1/13) = 1$
- along Y: $4.5 \cdot (1/13) \approx 0.346$
- along Z: $1 \cdot (1/13) \approx 0.077$

So the gizmo looks extremely squashed in Y/Z relative to X.

This matches the “massively squished” observation when the picked object’s nearest transform ancestor has a non-uniform scale.

## Why the new pick-plane triggers this

In the updated desktop-controls hint helper, the invisible pick plane is implemented as a **renderable square under its own `TransformComponent`**, with a large, non-uniform scale (in glyph-ish units).

When you click that plane:

- the hit `renderable` is the plane’s `RenderableComponent`,
- the nearest transform ancestor is the plane’s transform (`pick_transform`),
- the editor attaches the gizmo under `pick_transform`,
- the gizmo visuals inherit `pick_transform`’s non-uniform scale.

The gizmo system compensates using `max_basis_scale`, which keeps *one axis* roughly stable in world size but preserves the parent’s aspect ratio distortion.

## What “gizmos should not have other transforms affecting their world matrix” means

There are two subtly different requirements that tend to get conflated:

1. **Gizmo target semantics**: “when I drag the gizmo, which `TransformComponent` am I editing?”
2. **Gizmo visuals semantics**: “where does the gizmo render, and what transforms should affect its visuals?”

Right now, both are derived from *the same* topology rule: “attach gizmo under nearest transform, and inherit everything”.

To prevent squishing, we need to change either:

- what transform the gizmo is attached under (avoid non-uniform parents), or
- what it inherits from that transform (inherit translation/rotation but not scale), or
- stop using ancestry as the mechanism for positioning the gizmo visuals at all.

## Design options

### Option A (tactical): don’t select helper transforms

Make selection resolve to a more meaningful transform than “nearest”.

Common patterns in this codebase:

- “viz transform” problems (see `docs/analysis/gizmo-target-topology.md`) are the same class of issue.

Ways to do it:

- **Skip marked transforms** during `nearest_transform_ancestor`.
  - Requires an explicit marker like `EditorPickProxyComponent` / `NonEditableTransformComponent`.
- **PickTargetComponent on the renderable**:
  - attach `PickTargetComponent { target_transform: ComponentId }` on the pick plane renderable.
  - editor uses it if present.

Pros:

- Fixes squish by ensuring gizmo attaches under the “real” object transform.

Cons:

- Still inherits scale; you can still get squish if the real object is non-uniformly scaled.

### Option B (smallest gizmo-only change): cancel full parent scale (not just max)

Instead of treating parent scale as a single scalar, compute per-axis scale and apply the inverse on the gizmo visual root.

Conceptually:

- if parent TRS is well-behaved (no shear), basis column lengths give $(s_x, s_y, s_z)$.
- set gizmo root local scale to $(1/s_x, 1/s_y, 1/s_z)$ times a desired world scalar.

Pros:

- Keeps gizmo shape consistent under non-uniform scales.

Cons:

- Needs careful handling for:
  - rotation + non-uniform scale (still OK if using basis lengths, but orientation matters),
  - negative scales (mirrors),
  - shear (basis lengths are not a clean TRS decomposition).
- Still “inherits” the full parent matrix; it just compensates for it.

### Option C (declarative opt-out): transform inheritance flags

Introduce a component or fields on `TransformComponent` to control what descendants inherit.

Two common shapes:

- `TransformInheritComponent { inherit_translation: bool, inherit_rotation: bool, inherit_scale: bool }`
- or finer-grained: `inherit_scale: None | Uniform | Full`

Then `TransformSystem` would propagate with an adjusted parent matrix.

Example intent:

- gizmo visual roots inherit translation + rotation but **not scale**.

Pros:

- General-purpose; helps other UI/overlay subtrees too.

Cons:

- Requires matrix decomposition or a different multiplication strategy.
- Needs clear rules when multiple ancestors disagree.

#### A concrete proposal: `TransformFilterComponent`

Rather than baking inheritance flags into `TransformComponent` itself, we can model this as a
separate component that *filters what a subtree inherits* from its nearest transform ancestor.

This matches how other “behavioral modifiers” are modeled in the engine (small components that
systems interpret), and it makes the feature opt-in and local.

##### Intended usage (gizmo example)

Conceptual topology:

```text
target TransformComponent
  editor_transform_gizmo (TransformGizmoComponent)
    gizmo_root (TransformComponent)                 <- created by gizmo system
      gizmo_filter (TransformFilterComponent)       <- new
        gizmo_overlay (OverlayComponent)
          ... gizmo internals ...
```

And the intended behavior:

- The gizmo subtree follows the target’s **world translation** and **world rotation**.
- The gizmo subtree does **not** inherit the target’s **world scale**.
- The gizmo still has an explicit world-size knob (`TransformGizmoComponent.scale`).

This is the core requirement behind:

```rust
TransformGizmo {
  // spawns:
  TransformFilterComponent {
    with_inherited_translation,
    with_inherited_rotation,
    // not inherited scale
    Transform { /* gizmo internals */ }
  }
}
```

##### API shape

One pragmatic shape:

```rust
pub struct TransformFilterComponent {
    pub inherit_translation: bool,
    pub inherit_rotation: bool,
    pub inherit_scale: bool,
}
```

Two convenience constructors:

- `TransformFilterComponent::inherit_tr()` (inherit translation + rotation, not scale)
- `TransformFilterComponent::inherit_trs()` (current default behavior; mostly for explicitness)

##### Semantics in math terms

Let the *unfiltered* inherited parent world matrix be $P$ (the current `current_world` in the DFS).

We want to produce an *effective parent matrix* $P'$ used for descendants under a filter node.

The simplest definition is:

1. Decompose $P$ into $(T, R, S)$.
2. Recompose $P'$ using only the requested channels.

For example, “inherit translation + rotation only”:

$$P' = T(P) \cdot R(P)$$

Then descendants use:

$$M_{world}(child) = P' \cdot M_{local}(child)$$

This ensures descendants don’t inherit non-uniform or negative scale.

##### Integration point in `TransformSystem`

Today, `TransformSystem::transform_changed` DFS uses a `current_world` (nearest transform ancestor
world matrix) and multiplies it by a child’s local matrix when it encounters a `TransformComponent`.

To support filters, the DFS needs to treat a `TransformFilterComponent` as a “world-matrix modifier”
for the subtree that follows it.

Mechanically:

- When visiting a node, compute `next_world = current_world` by default.
- If the node has (or is) a `TransformFilterComponent`, compute `next_world = filter(current_world)`.
- If you encounter a `TransformComponent`, compute its cached `matrix_world` as `next_world * local`.

This keeps the propagation model intact: we still pass down a single `TransformMatrix`, but it can
be modified by filter nodes.

##### Where does the filter live in the tree?

There are two viable conventions:

1) **Filter-as-node**: a standalone component node in the topology (like `OverlayComponent`).
   - Pros: consistent with how the ECS tree is used (marker/modifier nodes).
   - Cons: it’s not a transform; it just affects how transforms below it inherit.

2) **Filter-on-transform**: attach `TransformFilterComponent` *to the same node id* as a transform.
   - This engine currently models “one component per node id” rather than “entity with multiple
     components”, so this is not directly representable without changing the ECS data model.

Given the existing architecture, **Filter-as-node** is the compatible choice.

##### Hard parts / edge cases

1) **Decomposition quality**

`TransformMatrix` is an arbitrary $4\times4$ matrix. If content introduces shear (or numeric drift
creates non-orthonormal bases), then “extract rotation” is ambiguous.

We can set a clear contract:

- Filters are defined for matrices that are *approximately TRS*.
- Rotation extraction is done by normalizing the basis vectors (Gram-Schmidt or simple normalize)
  and dropping scale.
- If basis vectors are degenerate (near zero), fall back to identity rotation.

2) **Negative scale (mirrors)**

Dropping scale also drops handedness flips. For gizmos that’s usually desirable (you don’t want the
gizmo mirrored), but it should be called out explicitly.

3) **Multiple filters in a chain**

Define the rule: filters apply in traversal order; each filter receives the already-filtered
`current_world` and produces a new one.

In practice for editor UI, it’s best to avoid deep filter stacking.

4) **Which transform does the filter reference?**

The filter is defined to only operate on the matrix being propagated from further up the tree
(`current_world` in the DFS). It does not “look up” an arbitrary transform elsewhere.

##### How this interacts with `TransformGizmoComponent.scale`

If gizmo visuals no longer inherit scale, then the existing compensation code in
`TransformGizmoSystem` (divide by `max_basis_scale(parent_world)`) becomes unnecessary (or at least
much less important) for gizmos.

That suggests an ordering preference:

- First, make gizmo visuals not inherit scale (via filter).
- Then, treat `TransformGizmoComponent.scale` as a direct local/world size knob without additional
  correction.

This would also remove the “squish” failure mode entirely for gizmos.

### Option D (architectural): decouple gizmo visuals from the target hierarchy

Keep gizmo *target* as a transform id, but render gizmo visuals in an editor-owned “gizmo space” that does not inherit target transforms.

Mechanically:

- gizmo lives under the editor root (or a global overlay root)
- each frame (or on transform changes), set gizmo visual root to:
  - match target world translation
  - match target world rotation
  - apply fixed world scale
  - ignore target world scale

This implies adding a “world transform override” concept somewhere, for example:

- `WorldTransformComponent { matrix_world: Mat4 }` that bypasses parent multiplication, or
- a special-case path in `TransformSystem` for `OverlayComponent` subtrees, or
- having `TransformGizmoSystem` call into `VisualWorld` directly with an explicit model matrix for gizmo renderables.

Pros:

- Directly matches the requirement “gizmo world matrix should not be affected”.

Cons:

- Bigger conceptual shift: some subtrees are no longer pure transform-hierarchy driven.
- Requires consistent updates when the target moves.

### Option E (local fix in content): make pick planes non-distorting

If the intent of the pick plane is purely “clickable area”, it doesn’t have to be a transform with a huge non-uniform scale.

Alternatives:

- attach the pick renderable directly under a uniform-scale transform (e.g. the hint root transform),
- bake plane size into the mesh (CPU mesh vertices) so the local transform can stay close to identity,
- ensure the nearest transform ancestor you click is the intended root (by topology layout).

Pros:

- Easy and predictable.

Cons:

- Doesn’t solve the general editor/gizmo problem for arbitrary non-uniformly scaled objects.

## Recommendation (if we want a declarative engine-level rule)

If the goal is a robust editor that behaves well under arbitrary content transforms, the cleanest long-term shape is:

- **Decouple gizmo visuals from the target hierarchy** (Option D), *or*
- **Add declarative inheritance flags** (Option C) with an initial focus on `inherit_scale`.

If we want the smallest step that directly addresses the current squish, the shortest path is:

- keep existing parenting, but change gizmo visual compensation to cancel **full non-uniform scale** (Option B).

## Related docs

- `docs/spec/gestures-and-gizmos.md` (overall interaction pipeline)
- `docs/analysis/gizmo-target-topology.md` (another case where “nearest transform ancestor” is not the semantic target)
