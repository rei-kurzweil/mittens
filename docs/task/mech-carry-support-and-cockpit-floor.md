# Mech carry: leave when off the deck, and walk the cockpit floor

Notes, 2026-11-25. Not started.

## Problem 1: thrown around after ejecting from a spinning mech

Ejecting (eject button or grip) stands the rider on the cockpit floor
(`car_dismount`, car space `[0, 4.2, 1.2]`), where the carry deck picks them
up. If the mech is turning and the rider steps or drops off, they keep being
carried, whirling with it, until they leave a region much larger than the
mech. Far from the pivot that turn is fast (deck velocity there is w x r), and
leaving adds that velocity back, so they are flung.

Cause: staying aboard is one box, `car_stay_zone` in `display_car.mms`:
`Zone.cube([5.8, 3.6, 4.4])` at `[0, 2.8, -0.3]`. Cube sizes are half
extents, so it is 11.6 x 7.2 x 8.8 m, reaching y -0.8 to 6.4. The mech itself
is about 8.8 m across the wings (x +-4.4), its body only x +-1.58, and its
lowest deck surface is the cockpit floor at 2.6. So:

- standing **on the ground** beside the mech still counts as aboard (the box
  goes below the ground; the board zone starts at 0.35 so it would never pick
  you up there, but the stay zone never lets go);
- the box reaches about 1.4 m past the wing tips and 2 to 3 m past the nose
  and tail, and its corners are about 7.5 m from the pivot, roughly twice the
  radius of the body.

`attachment_carry.rs` (`deck_zone_holds`) tests one zone per Mountable:
`auto_mount_zone` to board, `entry_zone` to stay. There is no narrow phase
against the deck itself.

### Fix: stay aboard while the deck supports you

The deck already has a coarse narrow phase: the boxes in its
CollisionContext (`deck_box` in `display_car.mms`: cockpit floor, cabin side
and back walls, roof, each wing top separately, rear deck). Those follow the
GLB's split (the wings are their own primitives, `vehicle_body_wing_left` and
`_right`, used for damage), so the body is a narrow prism and the wings are
two slabs.

1. **Support test.** While carried, the rider stays aboard while a short ray
   down from their feet (or the capsule's bottom) hits a zone in the deck's
   context within some distance (about 0.6 m, so stairs, crouching and
   bumps hold). Query only the context's local tree, in deck coordinates; it
   is already built per context, so this is a handful of boxes per rider per
   step.
2. **Grace time.** Losing support starts a short timer (about 0.3 to 0.5 s)
   so jumping on the deck, or a step over a gap between the floor and a wing,
   doesn't end the carry. Landing on deck support again cancels it. Ending
   after the grace adds the deck velocity where the rider is, as now.
3. **Coarse cap.** Keep the stay zone as an outer bound (leave it anyway
   once outside), but shrink it to the mech's real extent and lift its floor
   above the ground: x +-4.6, y 1.5 to 6.0, z -2.0 to 3.0, roughly.
4. Boarding stays zone-driven (`auto_mount_zone`), so jumping onto the deck
   from a crate still works; optionally also require deck support within
   the same ray distance, so brushing the board zone in mid-air doesn't
   board.

API: something like `Mountable.carry().leave_without_support(0.6, 0.4)`
(distance, grace seconds), or make it the default when the carry has a
`.context()`. The pickup bed should then use it too, since its stay zone
reaches 0.4 m past the walls and tailgate.

Smaller first step, if the support test needs to wait: allow several stay
zones per carry (union: the body prism plus each wing slab), with floors
above the ground. That fixes standing on the ground, but not hanging out in
the air between the wings.

### Tests

In `vehicle_carry_tests.rs`:
- A rider ejected onto the floor of a turning mech who walks off a wing tip
  ends the carry within the grace time, never more than about 4.6 m from the
  pivot.
- A rider standing on the ground next to the mech is never carried.
- A rider jumping on the floor (in the air for about 0.4 s) stays carried.

## Problem 2: walk around the cockpit

Goal: a walkable square floor around the seat and the steering wheel. Walls
would be nice; the floor is what matters.

A floor already exists in code (`display_car.mms`): a deck box at
`[0, 2.5, 0.55]` sized `[3.1, 0.2, 4.3]`, so its top is at 2.6, spanning
x +-1.55 and z -1.6 to 2.7. There are cabin side walls at x +-1.62, a back
wall at z 2.75, and the roof above. The context is a passthrough context
under `body`, so the hull's sliding spheres (which are inside the cockpit
volume) are excluded while carried. This is **untested in the headset**.
Ejecting should already stand you on it.

To check when building:
- Eject while parked. Do you stand at cockpit-floor height (eye about 4.2
  above the ground), or fall through, get pushed out, or end up on the roof?
  "Possible hull push when leaving the mech seat" is an earlier open note.
- The seat, console and steering wheel are not solid, so you walk through
  them. Add boxes for the seat and console if that feels wrong.
- Does the floor box line up with the visible floor in car.glb? Its numbers
  were measured from the GLB bounds (body y 2.34 to 5.05), not from the
  cockpit mesh, so it may need nudging.
- Gripping near the seat re-mounts it (the seat's entry zone is
  `car_entry_zone`, at the front, below). Make sure walking in the cockpit
  doesn't fight that.

## Related

- `collision-contexts-and-vehicle-carry.md` (Phase 3 carry, Phase 4 car deck)
- `display-car-floor-contact-and-gravity.md`
- `truck-cab-seated-scoot.md`
