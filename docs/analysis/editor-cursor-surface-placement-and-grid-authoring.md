# Editor Cursor Surface Placement and Grid Authoring

Date: 2026-06-12

Status: analysis

## Summary

The current behavior is mixing two different concepts under the label of a "3D cursor":

- copy the selected object's world transform
- derive a placement frame from the user's ray hit on a surface

Those are not equivalent.

They can agree for some assets, but they diverge badly for common editor cases:

- clicking a wall-mounted or tilted object and copying its authored transform may produce a grid that is perpendicular to the clicked surface
- clicking a box face may imply "place on this face", while copying the box transform implies "use the box's object axes"
- spawning a grid at the selected transform origin can put it inside volume geometry, making it effectively invisible even if its orientation is technically correct

So the core issue is not "grid rotation is wrong" in the narrow sense. The issue is that the editor currently has only a transform-copy cursor model, while grid authoring and paint-style placement want a surface-derived placement model.

There is also a higher-level editor routing issue:

- workspace-level tools/modes and panel-level tools are not yet described as one unified system

Conceptually, the editor should have:

- a top-level workspace context tool/mode
- optional panel-local tool overrides while a panel is focused

That is already aligned with the engine's existing `ObserverRouter` pattern.

## What the current systems do

### Workspace context versus panel-local tools

At the editor level, we should distinguish:

- workspace context tool/mode
- panel-scoped tool/mode

Examples of workspace context tools:

- `Select`
- `3D Cursor`

Examples of panel-scoped tools:

- the current paint panel tool selection
- future grid-authoring tools
- future terrain / nav / annotation / snapping tools

The conceptual rule should be:

- the workspace owns the default tool/mode for scene interaction
- an individual focused panel may override that mode for interactions relevant to that panel
- when the panel is no longer focused, routing falls back to the workspace mode

This is not just a UX statement. It is a routing statement.

The engine already has a mechanism for this kind of routing:

- `ObserverRouter`

So the correct editor architecture is not "each tool independently listens to all scene events forever".
Instead it is:

- workspace-level observers are active by default
- focused panel/tool observers can override or blacklist those handlers while active
- routing is controlled explicitly through observer-router state

That matches how the paint system is already partially treated today.

### Editor context / cursor

The editor context currently stores:

- `cursor_translation`
- `cursor_rotation`

That pose is derived from the selected component's world transform.

This is useful for:

- "move gizmo to selected thing"
- "spawn something at the selected thing"
- workflows where the intended cursor really is "copy this object's pose"

But it is not enough for:

- "place onto the clicked surface"
- "align to the hit normal"
- "spawn a grid on top of terrain / floor / wall geometry"

It is also missing one conceptual field:

- which workspace tool/mode is currently active

Because cursor behavior should depend on whether the workspace is in:

- `Select`
- `3D Cursor`
- or some future mode

### Paint placement

`paint_placement.rs` already has a more surface-aware model.

It can derive:

- exact hit point
- surface normal
- an orientation aligning the placed asset's local `+Z` to that normal
- a small outward offset so the placed thing does not sit exactly inside the surface

That is much closer to the right mental model for grid placement and for a surface-driven 3D cursor.

### Grid placement

Recent grid spawn behavior is now closer to "copy current cursor/selection transform".

That makes sense if the cursor is explicitly in transform-copy mode.

It does not make sense if the user expectation is:

- "I clicked this surface"
- "put a grid on that surface"

## Why the current behavior feels wrong

### 1. Object orientation is not the same as surface orientation

An object can visually present a face that looks like "ground" or "wall" while its authored transform carries a different basis than the clicked face implies.

Examples:

- a rectangular prism may be rotated so its local axes do not match the face the user clicked
- a helper icon mesh may have an authored yaw offset to look correct in-world
- a decorative prop may be tilted, but the user still wants to derive placement from the exact face they clicked, not from the prop root's whole transform

This is why "select object, then add grid" can produce a technically consistent but semantically wrong result.

### 2. Volume origins are bad spawn points for surface tools

If a grid spawns at an object's transform origin, that origin may be:

- inside a cube
- centered inside a wall thickness
- inside terrain or mesh bulk

Even if orientation is acceptable, the grid can be occluded or z-fighting inside geometry.

For grid authoring, the user usually wants:

- the grid on top of a surface
- visible immediately
- offset outward by a small amount

That is a different rule than "spawn at selected transform origin".

### 3. Grid authoring is surface partitioning, not object cloning

The likely real workflow is:

- point at a ground plane, floor mesh, wall, tabletop, or large model surface
- establish a local snapping plane there
- maybe create several grids across different surfaces

That is conceptually much closer to painting or stamping onto a surface than to cloning an object's transform.

## Recommended conceptual split

The editor should support at least two cursor placement modes.

But those placement modes should sit inside a broader tool-routing model.

## Recommended tool-routing model

### Workspace-level mode

The editor workspace should expose a top-level mode/tool such as:

- `Select`
- `3D Cursor`

This is the default interpretation of scene clicks/drags when no panel-local tool is overriding it.

### Panel-level override

A focused panel may temporarily override workspace behavior.

Today the clearest example is:

- paint panel focus activates paint placement behavior

Future examples could be:

- grid panel enters "paint grids onto surfaces"
- terrain panel enters sculpt mode
- annotation panel enters marker placement mode

### Routing mechanism

This should be expressed through `ObserverRouter` rather than ad hoc "if focused do X" logic spread across systems.

Conceptually:

- workspace tool installs the default scene interaction handlers
- panel tools install their own handlers
- `ObserverRouter` blacklists or enables handler groups based on current focus + current tool

That gives the correct precedence model:

- focused panel tool overrides workspace mode
- otherwise workspace mode handles the interaction

### Why this matters for the 3D cursor

The 3D cursor should not just be "some state the editor has".
It should be associated with a workspace tool/mode:

- when workspace mode is `Select`, clicking scene geometry selects
- when workspace mode is `3D Cursor`, clicking scene geometry updates the cursor from hit data

And if a focused panel overrides this:

- paint-focused interactions place paint assets instead
- future grid-paint interactions place grids instead

So cursor semantics must be described together with event routing semantics.

### Mode A: Transform-copy cursor

Definition:

- cursor pose is copied from the selected component's world transform

Good for:

- spawning a helper at the selected object's pivot
- aligning things to a selected object intentionally
- current gizmo-centric workflows

Not good for:

- placing onto a clicked face
- creating surface grids

### Mode B: Surface-hit cursor

Definition:

- cursor pose is derived from the latest relevant ray intersection event
- translation comes from the exact collision point, optionally offset outward
- orientation comes from a derived surface frame, not from the selected object's full transform

Good for:

- grid painting / stamping
- placing props on floors, walls, slopes
- setting a 3D cursor that means "here on this surface"

This should become the preferred basis for grid authoring.

## What data a surface-driven cursor should store

The editor cursor should likely evolve from just:

- translation
- rotation

to a richer placement record such as:

```rust
struct EditorCursorPose {
    translation_world: [f32; 3],
    rotation_world: [f32; 4],
    source: EditorCursorSource,
    hit_point_world: Option<[f32; 3]>,
    surface_normal_world: Option<[f32; 3]>,
    surface_tangent_world: Option<[f32; 3]>,
    surface_bitangent_world: Option<[f32; 3]>,
    target_renderable: Option<ComponentId>,
}
```

Where `source` might distinguish:

- `SelectionTransform`
- `SurfaceHit`
- `Manual`

The important part is not the exact struct shape. The important part is preserving whether the pose came from object-copy or surface-hit logic.

## Surface frame derivation

For surface placement, the editor needs more than a normal.

Normal alone determines "which way is out", but not the in-plane yaw.

So the cursor/grid placement frame should be thought of as:

- normal
- tangent
- bitangent

There are several reasonable derivation policies.

### Policy 1: Hit normal + projected world up

This is roughly what `paint_placement.rs` already does conceptually.

Approach:

- use the hit normal as local `+Z`
- project world up onto the tangent plane to get a stable in-plane `+Y`
- derive `+X` from cross products

Pros:

- stable
- simple
- good default for floors, walls, slopes

Cons:

- in-plane yaw is only a heuristic
- if the surface is nearly parallel to world up, fallback rules matter

### Policy 2: Use hit triangle tangents or mesh-local frame

Approach:

- if the raycast system eventually exposes triangle/UV/tangent-space data, derive tangent and bitangent from the actual hit primitive

Pros:

- more faithful to authored surface orientation

Cons:

- much more engine work
- raycast data likely does not expose this yet
- can be unstable or unintuitive on arbitrary meshes

### Policy 3: Use normal only, allow user yaw adjustment afterward

Approach:

- create a default tangent basis from normal plus fallback axis
- let the user rotate the cursor/grid after placement

Pros:

- practical
- likely enough for first implementation

Cons:

- still needs a stable initial basis

## Recommended near-term direction

Use the same family of surface-frame logic as paint placement.

That means:

1. derive hit point from ray intersection
2. derive surface normal from the hit surface
3. derive tangent/bitangent from normal plus projected reference up
4. compute an outward-offset placement point
5. store that as a surface-sourced cursor pose

This is sufficient for:

- surface-driven 3D cursor
- grid painting / grid stamping
- future editor settings that switch cursor behavior

## Outward offset is not optional for grids

For grids specifically, spawning exactly at the hit point is usually wrong.

Reasons:

- the hit point may be on or slightly inside the surface due to numerical precision
- the grid visual can z-fight with the surface
- if the selected object's origin is inside a volume, the grid becomes invisible

So grid placement should have an explicit "surface clearance" rule:

- place at `hit_point + normal * epsilon`

Potentially with a dedicated grid-specific offset larger than general prop paint placement.

Example:

- props might use a tiny visual clearance like `0.01`
- grids might want a slightly more obvious clearance so they remain readable

## Recommended editor behavior

### Short term

- keep transform-copy cursor as existing behavior
- do not treat it as the only cursor model
- add analysis-driven groundwork for a surface-hit cursor mode

### Medium term

Add an editor setting for cursor placement mode:

- `CopySelectionTransform`
- `SurfaceHit`

Potentially later:

- `SurfaceHitThenLock`
- `Manual`

Separately, add a workspace tool/mode concept:

- `Select`
- `3D Cursor`

These are not the same axis.

Suggested distinction:

- workspace tool/mode answers: "what does a scene click mean right now?"
- cursor placement mode answers: "if the cursor is being set, how is its pose derived?"

That lets `3D Cursor` mode later support multiple cursor derivation policies without collapsing the concepts.

### For grid creation specifically

Preferred behavior:

- if the cursor mode is surface-hit and a valid surface hit exists, spawn the grid from that surface-derived cursor pose
- otherwise fall back to transform-copy cursor pose

An even better grid-specific tool later may be:

- "paint grids" directly from hits, without going through selection-copy at all

## Architectural implication

The editor should stop treating selection state as the only authoritative source of placement pose.

Instead, placement should be sourced from one of two pipelines:

- selection-derived transform pipeline
- ray-hit-derived surface pipeline

That suggests a reusable utility boundary, something like:

```rust
fn resolve_surface_placement_frame(
    world: &World,
    target_renderable: ComponentId,
    hit_point_world: [f32; 3],
) -> Result<SurfacePlacementFrame, PlacementError>
```

Where `SurfacePlacementFrame` contains:

- point
- normal
- tangent
- bitangent
- recommended outward-offset point
- rotation quaternion

This should be shared by:

- paint placement
- future 3D cursor surface mode
- grid stamping / grid authoring

And the decision to invoke it should be owned by the active routed tool:

- workspace `3D Cursor`
- panel paint tool
- future panel grid-paint tool

## Why this should be generalized from paint

`paint_placement.rs` is already the closest thing to the correct abstraction.

Today it is framed as asset placement logic, but most of its real value is more general:

- converting hit data into a stable placement frame
- distinguishing hit-surface semantics from object-transform semantics
- preventing spawned content from intersecting the support surface

So the right move is not "copy grid logic into paint" or "copy paint logic into grids" ad hoc.

The right move is:

- extract a general surface placement/frame utility
- let paint, cursor placement, and grid tools all depend on it

## Open questions

### Should the cursor always move from click hits, or only in a dedicated mode?

Likely only in a dedicated mode or explicit tool state.

Reason:

- selection-click and cursor-placement-click are different intents

### What should happen on curved surfaces?

The normal-based frame still works, but:

- tangent selection becomes more heuristic
- large grids on curved surfaces may not make sense

This suggests:

- surface-hit cursor can work on curves
- grid authoring may want to warn or constrain to approximately planar support

### Should grid creation use exact hit point or snapped point?

Eventually likely both:

- cursor pose from exact hit
- grid origin optionally snapped in-plane afterward

### Should "grid on surface" require the clicked renderable to stay selected?

Probably not.

The support surface and the selected object should not be conflated.

Selection is an editor focus concept.
Support surface is a placement concept.

## Recommended next implementation steps

1. Add this conceptual split to editor design language:
   - transform-copy cursor versus surface-hit cursor

2. Add workspace tool routing to the design language:
   - top-level workspace mode/tool
   - panel-local override
   - `ObserverRouter` as the mechanism

3. Treat `Select` and `3D Cursor` as workspace tools, not just ad hoc behaviors.

4. Extract a reusable surface-frame helper from `paint_placement.rs`:
   - hit point
   - normal
   - tangent / bitangent
   - outward offset
   - rotation

5. Extend editor cursor state to preserve source and optional surface metadata.

6. Add workspace mode and cursor placement mode to future editor settings / workspace UI.

7. Make grid creation prefer surface-derived cursor pose when available.

8. Keep transform-copy fallback for workflows that intentionally want object-pivot alignment.

## Conclusion

The current grid result is exposing a missing editor concept, not just a broken transform.

We need to distinguish:

- "place at this object's pose"
- "place on this surface"

Grids belong primarily to the second category.

The paint placement code is already the best starting point for the shared abstraction, because it reasons from:

- ray hit point
- surface normal
- stable orientation
- outward surface offset

That should become the generalized placement-frame system used by future cursor modes and by grid authoring.
