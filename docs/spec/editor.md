# Editor (selection, picking, gizmos)

This document proposes an **EditorComponent** and **EditorSystem** for cat-engine.

The intent is to support:

- Selecting/deselecting renderables via mouse picking.
- Automatically attaching a gizmo to the selected target.
- Restricting selection to a subset of the component graph ("editable scopes").
- An eventual inspector workflow (initially: print details via REPL when selection changes).

It also introduces a missing conceptual layer: **picking**.

- **Raycasting** is geometry (rays, hits, distances) and is often used for many things (mouse hit-test, AI sensors, scripted queries, physics helpers).
- **Picking** is *interaction intent* (select, hover, context menu, drag start, etc.) and should be driven by specific raycasters / purposes.

See also:

- `docs/bvh-and-raycast.md` (current raycast/BVH pipeline)
- `docs/gestures-and-gizmos.md` (Input → Raycast → Gesture → Gizmo)

## Current state (what exists today)

- `RayCastSystem` emits `EventSignal::RayIntersected { raycaster, renderable, t, origin, dir }` into `RxWorld`.
- `GestureSystem` reads queued `RayIntersected` + `InputState` and emits `DragStart/DragMove/DragEnd`.
- `GizmoSystem` reads drag signals and applies TRS operations when the drag renderable is inside a gizmo handle subtree.
- `GizmoComponent` is a component you attach under a `TransformComponent` you want to manipulate; on init it spawns the gizmo visual subtree.

What’s missing for an editor:

- A concept of **selection**.
- A clear way to treat **click on empty space** as a meaningful event (deselect).
- A **routing layer** so selection logic only reacts to “user picking” raycasts, not arbitrary raycasts.

## Verified issues (high-level map)

These are the main pain points you listed, mapped to what the codebase does today.

1. **No fine-grained line-of-sight picking**
  - Verified: current picking is primarily **ray vs world-space AABB**.
  - `RayCastSystem` narrow-phase is intentionally stubbed and currently accepts the AABB hit distance.
  - Result: rings/cones/stems can be “hit” even when the ray passes through the visual hole.

2. **Axis-aligned bounds are not enough for rotated thin/flat shapes**
  - Verified: the BVH is built over **world-space AABBs** using the `bvh` crate.
  - AABBs are axis-aligned, so rotated parts can get inflated bounds, increasing false positives.
  - Note: broadphase BVHs are typically AABB-based; OBB/shape-specific tests normally live in narrow-phase.

3. **Narrow-phase rejection must be able to “fall through” to other candidates**
  - Verified limitation: `BvhSystem::raycast_renderables` returns only the single best AABB hit.
  - If narrow-phase rejects that candidate, the current query cannot automatically continue to the next candidate behind it.
  - Fix direction: query multiple candidates (sorted by $t$) or allow “continue traversal” after rejection.

4. **Input routing / control scheme conflicts**
  - Verified: `InputState::mouse_dragging()` becomes true when *any* mouse button is held while the cursor moves.
  - `InputSystem` uses mouse dragging to rotate rigs (yaw/pitch), while gestures use left click/drag for interaction.
  - Result: dragging gizmos can also rotate the camera unless we gate camera-look (e.g. right mouse) or add capture.

5. **Gizmos need a local/world mode**
  - Verified: `GizmoSystem` currently applies operations using world unit axes (X/Y/Z), with no mode switch.

6. **World-mode gizmo + parenting requires compensation or detaching**
  - Verified: gizmo visuals are parented under the target transform subtree, so they inherit rotation/scale.
  - For a true world-space gizmo, we need either:
    - detaching gizmo visuals to a neutral world node, or
    - applying an inverse-parent transform at the gizmo root so it visually stays axis-aligned in world.

See also: `docs/bvh-and-raycast.md`, `docs/analysis/raycast-circles-and-cones.md`, `docs/gestures-and-gizmos.md`.

## Proposed components

### `EditorComponent`

A marker/config component representing an **editable scope**.

Conceptually:

- Any component subtree parented under an `EditorComponent` is eligible for editor selection.
- You can have multiple editor scopes (e.g. one scene root, one UI overlay root, one imported asset sandbox).

Potential fields (not required for a first implementation):

- `selection_policy`: `Single` (default) | `Multi`
- `allow_gizmos`: bool (default true)
- `allow_select_gizmos`: bool (default false) — usually you don’t want gizmo parts to become the “selected object”.

### Optional: `EditorOwnedComponent`

If we want deselect to remove *only* editor-managed gizmos (and not gizmos the user attaches via scripts later), we need a way to mark ownership.

One approach:

- When the editor spawns a `GizmoComponent`, also attach an `EditorOwnedComponent` as a child of that gizmo node.
- On deselect, remove only gizmos that have `EditorOwnedComponent`.

Alternative: track the gizmo component ids in `EditorSystem` state.

### Optional: `PickingRaycasterComponent`

A tag/config on raycasters to declare intent.

Why: `RayCastSystem` can be used for more than “user cursor selection”, and editor selection must not react to unrelated rays.

Fields might include:

- `purpose`: `SelectPrimary` | `SelectAdd` | `Hover` | `ScriptQuery` | ...

## Proposed system: `EditorSystem`

### Responsibilities

- Maintain editor selection state.
- On a selection change, spawn/destroy gizmos.
- Produce selection-change outputs (initially: prints; later: signals / inspector UI).

### Where it fits in the tick order

A reasonable order is:

1. `RayCastSystem` (produces `RayIntersected`)
2. `PickingSystem` (optional layer; produces pick hit/miss)
3. `EditorSystem` (consumes pick hit/miss; spawns/removes gizmos)
4. `GestureSystem` / `GizmoSystem` (dragging gizmo parts)

Practical note (based on the issues above): if we add narrow-phase picking for gizmo rings/cones, we should also update the raycast query to support multi-candidate selection so rejecting a gizmo part still allows hitting the selected object behind it.

If we *don’t* add `PickingSystem` initially, `EditorSystem` can read `RxWorld::signals()` the same way `GestureSystem` does.

### Selection state

Start simple:

- `selected: Option<ComponentId>` — the selected **renderable** id.
- `editor_gizmos: Vec<ComponentId>` — gizmo component ids spawned by the editor.

Evolve later:

- Multi-select: `HashSet<ComponentId>` with a “primary selection”.
- Store richer info (selected transform, selection timestamp, etc.).

## Parenting subtrees under `EditorComponent`

The world graph is already a parent/child component graph. The editor scope is just another component node.

Example topology:

- `Transform (scene_root)`
  - `EditorComponent`
    - `Transform (object A)`
      - `Renderable (mesh A)`
    - `Transform (object B)`
      - `Renderable (mesh B)`

When a ray hits a renderable, the editor can decide whether it belongs to an editor scope by walking up ancestry:

- Start at `hit_renderable`.
- Walk `parent_of` repeatedly.
- If an `EditorComponent` is encountered, the renderable is within that editor scope.

Note: outside engine internals, prefer the `Universe` query wrappers (`universe.parent_of`, `universe.children_of`, `universe.get_component_by_id_as`) rather than accessing `universe.world` directly.

## Selecting: attach a gizmo to the hit renderable

### Important nuance: gizmos attach to transforms

Today, `GizmoComponent` is designed to be attached under a `TransformComponent`.

So when we say “attach gizmo to the renderable that was hit”, the practical implementation is:

- Selection is tracked as the **hit renderable** id (what the user clicked).
- The gizmo is attached to the **nearest ancestor transform** of that renderable.

This keeps compatibility with existing gizmo behavior and is aligned with the notion that you manipulate transforms, not renderables.

### How to find the target transform

Given `hit_renderable: ComponentId`:

- Walk up `parent_of(hit_renderable)` until you find a `TransformComponent`.
- That transform is the gizmo target.

If no transform is found, you can either:

- Treat it as non-selectable, or
- Attach the gizmo anyway and let the gizmo system fail gracefully.

### Spawning the gizmo

On selection:

1. Remove existing editor-owned gizmo(s) (default single-selection behavior).
2. `let gizmo_cid = world.add_component(GizmoComponent::new())`
3. `universe.attach(target_transform, gizmo_cid)`
4. `universe.add(gizmo_cid)` (or rely on normal init path)

Cleanup should remove the gizmo visual subtree via `GizmoComponent::cleanup`.

## Deselecting and “raycast hit nothing”

Today, `RayCastSystem` emits `RayIntersected` only on hit.

For deselection we need a “miss” condition.

### Minimal approach (works today)

In `EditorSystem::tick`:

- If left mouse was pressed this frame:
  - Look for the best `RayIntersected` signal for the editor’s selection raycaster(s).
  - If none exist, treat it as a **click miss** ⇒ deselect.

This works without changing `RayCastSystem`.

### More explicit approach (future)

Add a new signal emitted by raycast:

- `EventSignal::RayMissed { raycaster, origin, dir }`

Then picking/selection can consume hit/miss uniformly.

## Don’t select gizmo parts

When a gizmo exists, its handle renderables are also raycastable (by design).

In most editors, clicking a gizmo handle should:

- Keep the current selection.
- Start a drag gesture that manipulates the selected transform.

So selection logic should ignore hits that are “inside a gizmo subtree”.

Practical rule:

- If the hit renderable has a `GizmoComponent` in its ancestor chain, ignore it for selection changes.

This matches the existing gizmo topology: gizmo visuals live under the gizmo component.

## Allowing multiple gizmos (but default single)

Default behavior:

- Single selection ⇒ a single editor-owned `GizmoComponent` at a time.

Support multiple in theory:

- `EditorComponent.selection_policy = Multi`
- Use modifier keys (e.g. Shift-click adds/removes from selection).
- Track `editor_gizmos` per selected item.

To allow scripting multiple gizmos later without fighting the editor:

- Prefer removing only editor-owned gizmos on deselect.
- Leave user-attached gizmos alone (requires an ownership tag or editor tracking).

## Why we need a “picking” abstraction

`RayCastSystem` is currently a general facility:

- It casts based on input (click/drag) in `EventDriven` mode.
- It can also cast on demand via `Action::raycast(...)` (used for scripted/automatic casts).
- It supports ray sources beyond cursor picking (e.g. parent-forward rays).

Selection should not necessarily react to all raycasts.

### Option A (minimal): dedicate a selection raycaster

Create a dedicated `RayCastComponent` for selection (cursor driven) and ensure other systems use other raycasters.

Then `EditorSystem` simply filters `RayIntersected` by `raycaster == editor_selection_raycaster_id`.

Pros:

- No new component types required.
- No new signals required.

Cons:

- Semantics are implicit ("this raycaster means selection").

### Option B (clean): `PickingSystem` on top of raycasting

Add a `PickingSystem` that consumes:

- `InputState` (mouse press, modifiers)
- `RayIntersected` facts

And produces semantic events such as:

- `PickHit { purpose, raycaster, renderable, hit_point, t }`
- `PickMiss { purpose, raycaster }`

Then `EditorSystem` consumes `PickHit/PickMiss`.

Pros:

- Separates geometry facts from interaction intent.
- Supports non-cursor picking (XR pointers, scripted selection) without leaking into editor selection.

Cons:

- Requires new signal kinds and a new system.

## Inspector workflow (initial)

Short-term, `EditorSystem` can just print selection changes:

- On select: print renderable id and its nearest transform id.
- On deselect: print “selection cleared”.

Then the user can use the REPL to explore:

- `tree` to see the component tree.
- `cat <path>` (or `cat <guid>`) to print details.

Longer-term, the editor can maintain an “inspector view model” and/or emit signals like:

- `SelectionChanged { selected: Vec<ComponentId> }`

so UI or scripts can respond.

See also:

- [docs/spec/inspector-panel.md](docs/spec/inspector-panel.md)
