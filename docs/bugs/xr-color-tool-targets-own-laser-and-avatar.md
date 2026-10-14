# XR color tool targets its own laser and avatar

Date: 2026-10-14
Status: open; user headset reproduction and source audit, no implementation.
Example: `examples/graphics-settings.mms`.
Related: [desktop avatar self-intersection](desktop-camera-pointer-self-intersects-avatar-near-camera.md),
[XR laser selection/origin](xr-hand-laser-is-selectable-and-origin-is-past-fingertip.md),
and [graphics panel interaction](graphics-settings-panel-controls-selectable.md).

## Reproduction and expected behavior

Choose a color with the editor color tool and aim at scene geometry in the XR
graphics-settings example. Pulling the trigger first colors the emitting finger
laser; after more presses, the avatar body/skin takes the color. UI clicks
still work. The exact sequence of changing ray hits has not been captured.

The casting avatar's renderables should be lower-priority candidates for its
own pointer, with explicit exclusion available where appropriate, such as the
emitting hand or beam. Do not globally disable avatar interaction: another
avatar's pointer should still be able to target it, and deliberate self-editing
should remain possible. Deprioritization keeps self hits as fallback candidates
when no preferred eligible target exists.

## Checked implementation

- [EditorSystem](../../src/engine/ecs/system/editor_system.rs) materializes
  raycastable scopes for editor scene branches. The example's player branch
  is inside `ED.active()`, making avatar geometry and nested presentation
  eligible through inherited raycastability.
- [PointerSystem](../../src/engine/ecs/system/pointer_system.rs) puts
  `Selectable.off()` sidecars on the runtime laser and rest-attachment mount.
  This blocks semantic scene selection; it does not remove their inherited
  raycastability. Selection and ray-query eligibility are separate contracts.
- [RayCastSystem](../../src/engine/ecs/system/raycast_system.rs) obtains all
  BVH candidates, performs narrow phase, and orders accepted hits by authored
  interaction priority, then distance. Its fallback follows the same rule.
  Neither query receives a casting-avatar owner or self-hit policy.
- [GestureSystem](../../src/engine/ecs/system/gesture_system.rs) collects
  `RayIntersected` events and sorts them again using priority/distance from
  Raycastable markers. It chooses click and drag targets separately. Changing
  only the raycast list order would lose a new ownership-based preference here.
- [EditorPaintSystem](../../src/engine/ecs/system/editor_paint_system.rs)
  maps a Click to SceneClick after an eligibility check. That check's private
  `has_selectable_off_ancestor` only checks the ancestor node itself, unlike
  [editor_scene_hit](../../src/engine/ecs/system/editor_scene_hit.rs), which
  also checks immediate child markers at each owner. The laser's off sidecar
  therefore does not give paint the same exclusion as scene selection.
- Color application directly updates/adds a Color child on the hit renderable.
  It does not independently choose a farther target. The SceneClick mapping
  also drops the original raycaster identity, so a later paint-only ownership
  decision would need extra context.

These are confirmed code paths, not a headset trace proving every observed
press. In particular, capture the actual laser/body renderable IDs before
attributing the whole sequence to one geometry intersection.

## Why UI clicks can still work

UI controls already use higher interaction priorities: this graphics panel's
controls use 120 and its shell uses 100. Ordinary avatar/world geometry defaults
to 0. UI can win even with nearer self hits in the list; aiming away from those
controls leaves ordinary-priority candidates competing by distance. Successful
UI clicks therefore do not establish that the pointer is free of self hits.
Independent click/drag target resolution is another distinction to retain in
diagnostics, especially for the [mirror Slider bug](graphics-settings-mirror-resolution-slider-unresponsive.md).

## Practical implementation slices

1. [ ] Unify paint's selection-exclusion check with the existing shared
   owner/sidecar-aware helper. Prove that `Selectable.off()` on the laser's
   transform prevents coloring it, without disabling its pointer or UI events.
   Merely rejecting the chosen paint target is insufficient to make a farther
   target win; address target acquisition in the next slice.
2. [ ] Resolve an optional casting-avatar identity in PointerSystem and retain
   it per pointer/raycaster. Existing pointer-to-raycaster mappings, AVC
   model/head/hand target IDs, and rest-attachment registration provide useful
   integration points. PointerTopologyContext currently only has driver flags.
3. [ ] Associate generated avatar renderables and runtime hand/laser helpers
   with that identity. Use GLTF generation and AVC/presentation lifecycle
   registration, not a full-world scan per candidate. A nearest-GLTF or
   nearest-AVC ancestor walk alone is insufficient: AVC reparents the head
   under a sibling head mount, and finger pointers move under corrected hand
   targets. Sharing a locomotion/editor ancestor is not evidence of ownership.
4. [ ] Add query-relative policies with separately configurable body and
   presentation behavior: ordinary, prefer other targets, and exclude.
   Names/API are undecided. Prefer-other retains self hits as fallback;
   exclude omits them for this pointer only. Preserve explicit UI interaction
   precedence and define how ownership preference composes with authored
   priorities; do not implement this by changing global Raycastable priorities.
5. [ ] Use one ownership-aware ordering contract in raycast publication and
   gesture acquisition, or carry resolved ranking metadata through the event.
   Apply it consistently to click, trigger drag, grip acquisition, hover, and
   other consumers that take the first hit. Keep pointer identities separate
   when both hands/avatars cast in the same frame.

Keep the shared BVH populated for other pointers. A deprioritized hit still
needs intersection testing and is not automatically a performance optimization.
An explicitly excluded candidate can be skipped before narrow phase in both
BVH and fallback paths, without truncating traversal to the nearest broad hit.
Do not use a larger `min_distance` as an ownership substitute: it also rejects
unrelated nearby targets and cannot identify which avatar emitted the ray.

Maintain ownership caches on GLTF readiness/replacement, AVC head/hand reparent,
pointer retarget, subtree removal, and scene reload. Unowned pointers preserve
ordinary behavior. Freeze runtime ownership and public policy spelling before
implementation; no new graphics-settings or editor-panel system is required.

Ownership filtering operates at renderable granularity. A runtime beam is a
separate renderable, but an imported hand can be part of the same skinned
primitive as the torso. A hand bone does not imply a separate hand renderable;
excluding that primitive would exclude its other body regions too. First
inventory the Rei(mu) primitives. Any finer hand-only exclusion needs geometry
or intersection metadata beyond this initial owner-preference slice.

## Diagnosis and acceptance

- Use existing `CAT_DEBUG_RAYCAST=1`, `CAT_DEBUG_GESTURE=1`, and
  `CAT_DEBUG_EDITOR_SCENE_HIT=1` traces to compare hit IDs, distance, priority,
  click/drag targets, and selection exclusion. Add bounded paint-target/owner
  diagnostics only in a later implementation slice; current traces do not
  expose an ownership relation that does not yet exist.
- With the local beam/body nearer than a world box, prefer-other selects the
  box. With only self geometry eligible, fallback self-editing works.
- Exclude-emitting-hand/beam continues to farther candidates. A second avatar
  or external editor pointer can still interact with the first avatar.
- UI clicks keep their intended priority; generated beam helpers do not get
  painted or selected. Verify off sidecars and direct ancestor markers alike.
- Cover both XR hands, desktop head pointers, moved/replaced avatar rigs,
  simultaneous pointers, BVH/fallback parity, and pointer capture through release.
