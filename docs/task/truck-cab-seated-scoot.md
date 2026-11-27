# Truck cab: seated, with a little room to scoot

Notes, 2026-11-25. Not started. This refines Phase 6 (seated cab) of
`collision-contexts-and-vehicle-carry.md`.

## Intent

Inside the pickup cab you don't walk around; you sit. The lower body is
frozen in a sitting pose on the bench, but the rider can still shift a bit
with their head and body:

- **Slide sideways along the bench**, between the driver's and passenger's
  places, and a little forward and back.
- **Vertical clamp:** you can lean down and stand up partway, but you can't
  sink below the seat or put your head through the cab roof.
- Leaving is still the seat's dismount (grip or eject), not walking out.

So this is a constrained seat, not a carry deck. Walking stays for the truck
bed and the mech cockpit (`mech-carry-support-and-cockpit-floor.md`).

## Today

The cab seats (`pickup_driving` driver, `pickup_passenger_seat()` in
`pickup_ride_seats.mms`) are snapped seats: mounting puts the rider's eye on
`mount_anchor` (driver at x -0.33, passenger at x 0.33, both at y 1.78,
z 0.30 in chassis space), and `hold_rider_walking` stops walking. There is no
seated posture: the avatar's body follows the head as if standing.

## Proposal

1. **Seat region** on the Mountable, in seat space: a box the rider's
   head (and the movement root under it) may move within, for example
   `Mountable.seated(hip, feet).room([-0.45, 0.45], [-0.25, 0.10], [-0.15, 0.20])`
   (x, y, z offsets from the eye anchor). HMD motion inside it moves the
   view; motion outside it is clamped, which moves the world past the
   head instead of letting the head through the bench or roof. On desktop,
   the move stick could slide within the same box.
2. **Frozen lower body.** The hips stay on the seat surface under the head's
   x (sliding with you), the feet stay on the floor pan, and the knees come
   from procedural seated legs. Upper-body IK runs as usual from the hips.
   This is the body-solver `Seated` mode from Phase 6, with the hip following
   the clamped head sideways only.
3. **One bench, two places.** Driver and passenger could share one seat
   region across the bench. Driving controls then belong to whoever is
   nearer the wheel (x < 0), or stay with the seat you entered. Decide when
   implementing; the simplest first slice keeps two seats and clamps each
   to its own half.
4. **Re-centre.** Setting the eye anchor on entry (plus a button) stays as
   planned, so a player sitting on the real floor or standing in the real
   room both fit.

## Constraints

- Steering wheel, dashboard and doors aren't collidable today. The clamp box
  is the only collision; it doesn't need a collision context.
- NPC Mitten in the driver's seat uses the same seated pose.
- Multiplayer: the seated flag and the head offset within the region ride
  the existing pose packet; the region itself comes from the seat.

## Validation

- Leaning down can't put the view below the bench top; standing up can't put
  it through the roof.
- Sliding to the passenger side and back keeps the legs seated with no
  pop.
- Turning, braking and hydraulics don't shake the clamped head relative to
  the cab.
