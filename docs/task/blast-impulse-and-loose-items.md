# Blast impulse, knocked vehicles and loose items

Notes, 2026-11-25. Not started; a design to agree before code.

## Wanted

- A rocket's blast, out to its outer ring (6 m), pushes and spins things away
  from it: vehicles, debris, and weapons (on racks, in mounts, or lying
  around).
- Weapons become movable: released or knocked loose, they fly with gravity
  until they touch something, then go static where they land. No bouncing.
  Landing on something that moves (a vehicle: vehicles always have velocity)
  rides with it instead.

## What happens today

- `apply_rocket_impact` (`assets/components/projectiles/rocket.mms`) calls
  `Damage.at(point, 1.5, amount)` and plays the explosion. Each scene's
  `on_explosion` adds shield and health rings (`INNER_BLAST` 2 m and
  `OUTER_BLAST` 6 m in `target_practice.mms`). Nothing applies motion.
- `DamageComponent` is `{ position, radius, amount }`. `DestroyableSystem`
  cuts with it and `IntegritySystem` counts it.
- Debris from a cut always gets the same velocity, `[0.3, 1.2, 0.2]`
  (`spawn_debris`), and falls without collision until it expires. Hence
  "debris isn't affected by the blast".
- Vehicles have a `Velocity` (and `Gravity`) and slide movers, so they could
  take an impulse, but nothing gives them one.
- A released weapon keeps its world pose and hangs in the air, unless the
  release lands in a `receive_drops` zone (the truck bed, `attach_drops`),
  which re-parents it. `shared_grabbable` streams its pose only while held.

## Overlapping systems to be careful with

| System | Touches | Risk |
|---|---|---|
| Damage, Destroyable, Integrity | the damage event | a second, separate blast event would let cut, debris and push drift apart |
| Debris (`destroyable_system.rs`) | its own ballistic motion | no collision, no relation to `Velocity` |
| Velocity, Gravity, static-contact movers | vehicles, players | an impulse in world space vs deck-relative velocity for carried riders |
| Carry (`attachment_carry.rs`) | riders on decks | pushing the deck moves riders; pushing a rider must convert to deck space |
| Grab, mounts, `attach_drops`, restock | where a weapon lives | knocking an item out of a mount must update the shared mount record |
| Networking | impacts are replicated (`shared_impact`); vehicles and items have owners | a push applied on every peer to a body only one peer simulates diverges |
| Collision contexts | truck bed and mech deck floors exist only inside their context | a falling item in the world space falls through the bed floor |

## Proposal

### 1. One blast, carried on the damage event

Give the damage request an optional impulse (on the damage intent, once
`damage-impacts-without-ecs-churn.md` lands): `Damage.at(x, y, z, radius, amount)
.blast(outer_radius, speed, spin)`. The cut still uses `radius`; the push
reaches `outer_radius`, falling off to zero there. Every consumer reads the
same event, so cut, debris and push agree, and pistols (no `.blast`) push
nothing. `apply_rocket_impact` adds `.blast(6.0, 9.0, 8.0)`, so every scene
gets it without touching `on_explosion`.

### 2. Debris takes the blast

`spawn_debris` sets each fragment's velocity from the event: away from the
blast point (from the point to the fragment's centroid, with an upward
bias), scaled by falloff, plus a random spin (debris has no angular state
today; add it). Without `.blast`, keep today's small drift. Optionally,
landing on the first surface along its path (the same sweep as section 4)
instead of falling through the ground until it expires.

### 3. A native `Knockback` response on moving bodies

A `Knockback` component (mass or response scale, spin scale) next to a
`Velocity` opts a body in. A blast adds `delta_v = falloff * speed / mass`
along the blast direction, and an angular kick around the axis
`cross(up, direction)` (vehicles: yaw only, or a clamped tilt). Runs in
the fixed step, after integration and before contacts, so the movers resolve
any shove into walls.

- Vehicles: add `Knockback` in `ship_vehicle_motion` (the mech) and on the
  pickup's motion. Their drive controllers then steer back. The truck's
  patrol should tolerate being shoved off its line.
- Players (optional, later): only for uncarried riders, or converted into
  deck space while carried. Probably smaller, and never while seated.

### 4. Loose items: `Loose` (fly, land, rest)

A native `Loose` component on an item's root (weapons, ammo): states
`resting | flying`.

- **Flying**: velocity and spin integrate with gravity. Each step, a ray
  sweeps from the item's previous to its new position (plus its radius)
  against solid zones, including collision contexts (the truck bed and mech
  deck), ignoring the item itself, players' movers and sensors. On a hit:
  place it there, clear its velocity and keep its rotation: **resting**. No
  bounce.
- **Landing on something moving**: if the hit zone has an ancestor with a
  `Velocity` (a vehicle), re-parent the item to that zone's nearest transform,
  keeping its world pose, so it rides along. Landing inside a
  `receive_drops` zone goes through the existing drop path instead.
- **Starting to fly**: on release (not in a drop zone), with the hand's
  throw velocity; or when a blast reaches a resting item and its push is
  above a small threshold.
- **Losing support** (later): a resting item does a cheap downward check
  every half second or so; if what it rested on was destroyed or drove
  away, it flies again.
- Events: `LooseLaunched` and `LooseLanded { support, moving }` for scripts.

`shared_grabbable` listens: on launch the owner marks the world mount and
streams the pose while it flies (as it does while held); on landing it sends
the final pose and mount (a vehicle frame registered as a mount, or the
world).

### 5. Who applies the push (multiplayer)

The rocket impact is already replicated, so every peer sees the blast. Each
body is pushed only by its authority: a vehicle by the peer simulating it
(the host for the patrolled truck, the driver for the mech), an item by its
owner (last holder, or the host if never held). Others see the result
through the existing pose replication. Debris is local and cosmetic on each
peer.

## Order

1. `Damage...blast(...)` plus debris velocity and spin (sections 1 and 2).
   Small, local, visible straight away. Do it together with
   `damage-impacts-without-ecs-churn.md`: `Damage.at` still creates a
   temporary `DamageComponent` per hit (the move to a damage intent drained by
   the destruction system is only proposed there). Carry the blast fields on
   that intent's payload rather than adding them to the component.
2. `Knockback` on vehicles (section 3).
3. `Loose` items, single-player first (section 4), then the shared-grab
   integration (section 5).
4. Players, and resting items losing support.

## Tests

- A blast at a known point gives debris velocities pointing away from it and
  shrinking with distance; a pistol hit (no blast) keeps today's drift.
- A truck 3 m from a blast gains speed away from it; one 7 m away doesn't.
- A dropped weapon falls onto a crate and rests on its top face; one dropped
  over the truck bed lands on the bed floor and then moves with the truck;
  a resting weapon 3 m from a blast flies and lands again.
- A held weapon is never knocked out of a hand.
