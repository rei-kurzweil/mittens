# UI button hover and avatar picker click feedback

Status: proposed. Extends the [pointer hover lifecycle task](pointer-hover-lifecycle-and-gizmo-feedback.md).

## Diagnose the avatar picker click

Each avatar button authors `Raycastable.click_only()` at priority 180. Layout
grafts the settings onto the button's `__bg` renderable with a `Quad2D` slab.
The glyph subtree is raycast-disabled and the button listens for `Click` on
its owning transform. The current rooftop test shoots synthetic rays from eye
and hand positions, checks the first hit at the label center, then injects a
`Click`. This proves the tested geometry and callback route. An actual press
and release through the gesture system is the next test.
[Avatar picker](../../assets/components/panels/avatar_picker_panel.mms),
[layout hit surface](../../src/engine/ecs/system/layout/block.rs),
[rooftop test](../../src/scripting/avatar_picker_tests.rs)

`GestureSystem` picks a click hit and a drag hit separately on press, but it
skips creating gesture state if the drag hit is absent. Click emission on
release is inside that drag state's release path. An isolated `ClickOnly`
button can therefore be visible in ray hits and still fail to click. If the
ray also hits a drag-capable object farther along the ray, it can create the
state needed for a click on the button. This is a plausible explanation for
the angle-dependent behavior and needs a controlled input test.
[Gesture press and release](../../src/engine/ecs/system/gesture_system.rs)

Reproduce with two layouts. Put just the click-only button on the ray, then
put a drag-capable surface behind it. Run desktop or XR trigger press and
release through the full pointer path in both layouts. Compare normal and
oblique views, including center, border, and glyph areas. Use
`CAT_DEBUG_RAYCAST=1` and `CAT_DEBUG_GESTURE=1` to see whether the button
missed, another surface won, there was no drag candidate, or release failed
the target, movement, or rotation checks. XR angle tolerance compares press
and release ray directions, so a steady oblique ray needs a separate test
from a moving controller.

Separate click press/release state from drag capture before advertising that a
highlighted button is clickable. A click-only button should activate on a
valid release without a drag-capable surface behind it. Preserve intentional
click and drag pass-through, pointer movement thresholds, and existing drag
and grip capture. Check whether the avatar picker also needs to block scene
drags behind its buttons as a separate routing decision.

## Hover through the existing pointer path

1. Request idle samples for enabled desktop and XR pointers when a live hover
   consumer exists. Reuse the same cursor-through-camera or controller ray,
   distance limits, BVH candidates, and narrow phase as a click at that pose.
2. Share the click candidate resolver between hover and press. Use the same
   `ClickOnly` / `All` eligibility, priority, and distance ordering. A
   click-capturing surface with no hover subscriber still participates in
   picking, so a button behind it only highlights when it would win a click.
3. Retain the resolved target per pointer. Emit `PointerEnter` and
   `PointerLeave` only when it changes. Route the event to the stable button
   scope and include pointer, raycaster, and hit renderable for diagnostics.
   Emit leave before enter when crossing between buttons. Clear state on loss
   of pointer activity, tracking, or window focus, or when a target is hidden,
   removed, or disabled. Keep captured gestures independent of hover movement.
4. Expose the edges as scoped MMS events. The panel tracks which pointers are
   over each button. The first enter applies the style and the last leave
   restores it, so either XR hand can keep the same button highlighted.

The highlight previews the current click target. A subsequent release still
has to land on the target and pass the pointer's gesture checks.

## Panel appearance

Lighten the whole warm background slightly. Add a broad inset border in the
same hue, with a small lightness difference. Keep the text and outer
dimensions steady. Have MMS update the existing fill and a retained,
raycast-disabled decorative border on the transition edges. Test that the
border and glyphs leave the background quad as the sole pick surface, and
that the updates leave layout and BVH topology alone. A visible highlight is
feedback only. Avatar loading remains in the button's `Click` callback.

## Cost and measurement

The renderable BVH is the right broad phase. It prunes the scene to ray/AABB
candidates before tests such as the avatar picker's cheap quad slab. A cast
still has CPU cost. The current path collects and sorts broad-phase candidates,
tests their shapes, sorts accepted hits by interaction priority and distance,
and publishes a `RayIntersected` event per hit. At 90 interaction updates per
second, two active XR hand pointers can add up to 180 idle queries per second
if both need a fresh sample every update.
[BVH traversal](../../src/engine/ecs/system/bvh_system.rs),
[narrow phase](../../src/engine/ecs/system/raycast_system.rs),
[pointer query and hit publication](../../src/engine/ecs/system/raycast_pointer.rs)

- Gate idle work on active pointers and observable enter/leave handlers.
  `RxWorld::ListenerGate` is an existing pattern for gating optional producer
  work. Keep ordinary action raycasts available independently. When an action
  already casts the same ray this update, use those hit results for hover.
- Resolve hover-only samples locally and emit target edges. Avoid publishing
  per-hit `RayIntersected` signals for those samples or running the linear scan
  fallback every idle tick before the BVH exists. Leave per-action fallback
  behavior on the existing path.
- Start by measuring one sample per eligible pointer per interaction update.
  If the measurements justify reusing a stationary result, invalidate it when
  the ray, camera, BVH membership or bounds, or pick eligibility changes.
  A moving panel or intervening surface must update hover under a still ray.
- Start with the existing ordering. Keeping a single best hit or reusing
  candidate buffers can reduce work if the profile shows allocation or sort
  cost. A nearest-AABB limit can discard a farther higher-priority target or
  the valid surface behind a candidate that fails the narrow phase.

Compare hover disabled and enabled on Twilight Rooftop and a dense scene.
Use one desktop pointer and both XR hands, idle and moving, with a panel and
an intervening object moving under a stationary ray. Record casts per update,
candidate and accepted-hit counts, query CPU p50/p95/p99, BVH maintenance,
and MMS callbacks. `CAT_PROFILE_SPATIAL=1` already reports aggregate ray
timings and fallback counts. Add split counters where needed to see the
incremental cost. Check on an XR target before enabling hover more broadly.

## Acceptance

- A real press and release over an isolated click-only surface clicks once.
  The same result holds with drag geometry behind it. Adding a foreground
  blocker or moving off the button before release behaves consistently with
  the established click rules.
- Aiming at either button edge or label gives the same hover and click target
  from desktop and XR at documented angles. Sweeping across buttons produces
  one leave and one enter per crossing and loads no avatar.
- Two pointers can highlight independently. Disabling or hiding one pointer
  clears only that pointer's contribution. Removal of a button or the panel
  clears outstanding hover state and leaves other gestures intact.
- No hover listeners means no idle hover casts. A click during a hover sample
  uses one shared query. Stationary aim stays correct when the panel or
  occluder moves. Hover appearance stays inside the visible hit rectangle.
