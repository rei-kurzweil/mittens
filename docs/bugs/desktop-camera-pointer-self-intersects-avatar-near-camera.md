# Desktop camera pointer hits the local avatar before distant scene geometry

Status: the minimum-distance slice is implemented on 2026-09-12.
`Raycast.min_distance` filters the final narrow-phase hit distance in both BVH
and fallback queries. Choosing a non-zero value remains an authored policy: it
is not inferred from avatar or controller topology. Per-raycaster local-avatar
exclusion remains future work.

2026-10-14 follow-up: [XR color-tool self hits](xr-color-tool-targets-own-laser-and-avatar.md)
records the same ownership problem for finger pointers, plus a paint
Selectable-off sidecar inconsistency. The current direction is per-pointer
avatar deprioritization with optional exclusion, keeping deliberate self hits
as fallback and preserving interaction from other avatars.

## Observed behavior

With a desktop pointer driven through the active camera, switch the editor to
`3D Cursor` and click around the scene. Many placements land extremely close to
the camera—often within a fraction of one world unit—instead of on the visible
distant surface under the cursor.

The likely near hits are renderables belonging to the local avatar, such as its
face or hair. From the first-person view those surfaces are not meaningfully
visible interaction targets, but they can still enclose or cross the outgoing
camera ray and win hit selection.

This affects more than the 3D Cursor tool. Selection, mounting, grabbing,
painting, and any future desktop hand/arm pointing derived from camera aim all
still require an unobstructed logical camera ray. Driving an avatar hand toward
the camera target would not solve a ray that already selected the avatar itself.

## Reproduction

1. Open a desktop scene with the camera attached inside or near the local
   avatar, including `examples/mittens-corp-desktop.mms`.
2. Select the editor's `3D Cursor` interaction mode.
3. Aim at several pieces of scene geometry beyond the avatar and click.
4. Observe that some cursor placements remain near the camera instead of
   landing on the intended distant surfaces.
5. Run with `CAT_DEBUG_RAYCAST=1` and `CAT_DEBUG_CURSOR_3D=1` and compare the
   selected renderable label and ray parameter `t` with the intended farther
   candidates.

## Relevant implementation evidence

### The desktop ray begins at the camera near plane

`RayCastSystem::ray_from_cursor` unprojects the cursor at clip-space near and
far points. The emitted ray origin is the near-plane world point, not a point
outside the avatar. Its normalized direction runs toward the far point.

### All accepted hits remain available, but there is no near bound

`RayCastComponent` exposes `min_distance` (default `0.0`) and `max_distance`
(default `200.0`) in world units. Both BVH and fallback paths filter the final
narrow-phase hit distance to that interval.

Hits are ordered by interaction priority first and ray distance second.
`GestureSystem` then chooses the first hit that captures the relevant gesture.
Consequently, an ordinary-priority face or hair hit at small `t` wins over an
ordinary-priority scene surface farther down the same ray.

`Pointer.min_grab_distance` is not a solution. It controls clearance when
placing an object that has already been grabbed; it does not filter raycast
hits.

### The local avatar is eligible for editor raycasts

At editor registration, `EditorSystem::materialize_editor_raycastables` wraps
each current immediate child of the editor root with an enabled raycastable
scope unless that subtree explicitly opts in or out. In
`mittens-corp-desktop.mms`, the desktop locomotion/player branch is inside the
active editor, so imported avatar renderables inherit editor-default
pickability along with ordinary scene content.

This source map should be confirmed with the existing debug output before the
fix is considered proven; a near hit could also come from another local helper
or visualization subtree.

## Expected behavior

A pointer/raycaster must be able to reject hits closer than its configured
interaction distance and continue evaluating farther candidates on the same
ray. Rejecting a near candidate must not clamp the resulting hit to the minimum
distance and must not terminate traversal.

For a first-person desktop pointer, local avatar presentation geometry should
also be excludable from that pointer's interaction query without making the
avatar globally invisible or non-interactable to other pointers and editor
views.

## Ray interval API

Add a lower bound alongside the existing upper bound:

```text
RayCast.min_distance   default 0.0
RayCast.max_distance   default 200.0

candidate accepted when min_distance <= t <= max_distance
```

The interval belongs to `RayCastComponent`, because it constrains query
results rather than grab placement or gesture interpretation. Expose and
round-trip it with the same conventions as `max_distance`, for example:

```mms
Pointer {
    Raycast.event_driven().min_distance(0.75).max_distance(200.0) {}
}
```

`PointerSystem` already preserves an explicitly authored direct-child
`RayCast`; it creates the default one only when none exists. A convenience
Pointer builder can be considered later if this becomes a common policy, but
there should still be one authoritative interval on the actual raycaster.

Filtering must occur after narrow phase produces the final `t` and before hits
are priority/distance sorted or emitted. Apply it identically to BVH and
fallback paths so hover, click, drag, grab, mount, paint, and cursor consumers
see one consistent hit stream.

The first implementation should define the distance from the actual emitted
ray origin. For desktop cursor rays that is currently the unprojected near-plane
point. If product-facing configuration instead needs distance from the optical
camera pose, change the ray-origin contract explicitly rather than silently
adding the projection near distance in only one backend.

## Complementary self-exclusion policy

A fixed near distance is useful but not sufficient as the sole self-hit model:

- avatar sizes and camera offsets vary;
- hair, hats, wings, tools, or vehicle interiors may extend beyond one chosen
  radius;
- a large threshold can make legitimate close UI or small-object interaction
  impossible;
- XR hand rays need different clearance from camera rays.

Investigate a per-raycaster exclusion set or equivalent scoped policy that can
skip renderables belonging to the pointer's local presentation/avatar assembly
while leaving them available to other pointers. The exclusion should be based
on explicit component/subtree identity, not names or a hard-coded assumption
that every ancestor of the pointer is non-interactable; broad ancestor
exclusion could accidentally hide the entire authored scene.

For the immediate `mittens-corp-desktop` fixture, explicitly opting the local
avatar presentation subtree out of this camera pointer may be the more precise
fix. The generic minimum-distance interval still covers cameras embedded in
unrelated near geometry and gives authored tools a predictable near bound.

## Investigation and implementation slices

1. Capture debug traces for failed and successful clicks, recording ray origin,
   direction, ordered candidate labels, priorities, `t`, and final cursor hit.
2. Confirm whether the winning renderables are face/hair/avatar geometry and
   identify the raycastable scope that made them eligible.
3. Re-test 3D Cursor, selection, mount clicks, grabbing, close UI, desktop
   camera aim, and XR controller rays.

## Acceptance criteria

- Debug evidence identifies the near renderable and its governing raycastable
  policy.
- A raycaster configured with a positive minimum rejects a closer hit and
  selects an eligible farther hit on the same ray.
- The cursor is not placed at the minimum distance as a substitute for finding
  a real surface.
- `mittens-corp-desktop` camera clicks no longer select the local avatar's face,
  hair, or presentation helpers unintentionally.
- Legitimate close-range interaction remains possible for pointers configured
  with a smaller or zero minimum.
- Desktop and XR pointers can use different intervals and exclusion policies.
- BVH and fallback queries produce the same interval behavior.
- Existing `Pointer.min_grab_distance` behavior is unchanged and remains
  semantically separate.

## Related work

- [BVH and raycast data flow](../spec/bvh-and-raycast.md)
- [Pointer input, ray, and gesture pipeline](../spec/pointer-input-ray-gesture.md)
- [Editor 3D Cursor GLTF coverage and grid alignment](editor-cursor-3d-gltf-and-grid-alignment.md)
- [XR hand laser is selectable and starts past the fingertip](xr-hand-laser-is-selectable-and-origin-is-past-fingertip.md)
- [Shared 3D cursor and selection versus surface placement](../task/shared-3d-cursor-and-selection-vs-surface-placement.md)
