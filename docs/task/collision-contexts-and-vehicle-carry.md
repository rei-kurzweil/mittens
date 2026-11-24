# Collision contexts and vehicle carry

Status: planned, 2026-11-24. Phase 1 (input out of `AttachmentSystem`)
implemented 2026-11-24, pending a build and headset check: the attachment no
longer touches input, `Rider.set_locomotion_enabled` / `locomotion_enabled`
exist in MMS, and `vehicles/seat_rider_walking.mms` (`hold_rider_walking`) is
used by every current Mountable.

Related: [separate mounting from movement authority](attachment-movement-authority-and-input-routing.md),
[attachment valence and Grabbable unification](attachment-valence-and-grabbable-unification.md),
[AVC crouch IK](avc-foot-penetration-crouch-ik.md), [transitionable avatar pose layers](avatar-pose-transition-layers.md).

## Outcome

A player can walk, crouch and kneel on a moving vehicle (the `car.glb` mech's
floor, wings and roof; the pickup's bed), colliding with its interior while
sharing the vehicle's basis of motion: accelerating, braking and turning never
jostle them, and stepping off leaves at the vehicle's speed. The vehicle's
exterior hull does not collide with its own riders, but the rest of the world
still does (a low bridge knocks a rider off a wing). Interior collision costs
nothing until someone boards. Cab seats remain snapped seats, later with a
seated lower body.

## Requirements

| Case | Rule |
| --- | --- |
| Pedestrian | everything solid in the world |
| Carried on the car's wing / truck bed | the vehicle's interior + the world, minus this vehicle's exterior |
| Enclosed cockpit | the vehicle's interior only |
| NPC capsule | everything minus its own hitbox (already handled by the own-movement-root rule) |
| Rocket fired from a vehicle | everything minus the firing vehicle's exterior, at least just after launch |
| Sensors | players only (washroom probe, fall catcher) |

So the design needs: category whitelists and blacklists; instance-scoped
exceptions ("my vehicle", "myself"); runtime changes on boarding and leaving;
zero cost for movers with no special rules; nothing to replicate.

## Approaches considered

- **A. Category/mask bits** (Box2D/PhysX). One AND per pair, and BVH nodes can
  carry the OR of their children's bits to prune subtrees. No instance scope:
  "this car's exterior" would need a bit per car. Right for categories only.
- **B. Group index** (negative groups never collide). Handles "myself", not
  "my car's exterior but not its interior". Too blunt.
- **C. Scoped rules on each mover** (`include/exclude {categories, scope}`).
  Expresses every case, but every rider carries the vehicle's rules and each
  candidate is checked against them.
- **D. Separate collision spaces.** A speed-up rather than a rule: moving
  geometry in its own coordinates never needs refitting, and can be skipped.
- (Per-pair MMS callbacks: too slow per contact; rejected.)

## Chosen design: outbound and incoming, vehicle-built contexts

Every collidable has two independent memberships:

- **Outbound**: the context a mover queries and reacts to (pushed by, raises
  contact events against). Switching it is a pointer swap on the mover.
- **Incoming**: the context where a collidable can be found (by rockets,
  sensors, other movers). It changes only when an object should really leave
  the world's view, which costs one BVH remove and insert.

A rider switches only its **outbound** context to the vehicle's; its hitbox
and probe stay incoming in the world, so rockets and sensors still find it.

A vehicle builds its own context as a live component tree:

```text
let deck = CollisionContext.passthrough(true) {     // parent context too, minus this vehicle
    name = "car_deck_context"
    T.position(0, 2.1, 0.4).scale(6.0, 0.2, 3.2) {
        Zone.cube([0.5,0.5,0.5]) { Collidable.static().friction(0.85) {} }
    }
}
body.attach(deck)                                     // moves with the vehicle
let carry = Mountable.carry().entry_zone(deck_entry).context(deck) {}
```

- **Membership is the subtree.** A `CollisionContext` is the home (incoming)
  context of everything beneath it. Zones added later join automatically:
  destroyable cut pieces, cargo re-parented into the bed, parts the vehicle
  attaches. The world BVH never contains them; in the world the context
  appears as one bounding box.
- **Local lookup structure, built lazily.** Each context's BVH is in its own
  coordinates, so it is never refit while the vehicle drives. The contact
  system builds it when the first rider enters and drops it after the last
  leaves. Zones are cheap ECS components and stay resident; MMS never needs a
  factory (native code cannot call MMS anyway). A vehicle that streams heavy
  visuals (as the pool hall does) keeps its collision tree resident.
- **Sweeps in context coordinates.** A rider sweeps relative to the vehicle,
  so vehicle motion can't drag the capsule through the deck. That replaces
  the world-space sweep from `previous_centers`.
- **Passthrough.** `passthrough(true)` also queries the parent context (the
  world, or an outer carrier), excluding the vehicle's own subtree. That is
  the "everything except my vehicle's exterior" rule, stated once by the
  vehicle instead of on every rider. An enclosed cockpit uses `false`.
- **Nesting.** A truck parked on a carrier deck has its context's parent set
  to the carrier's context.
- **Rays and projectiles** crossing a context's world bounds also query it in
  its coordinates (one box test in the common case).
- **The vehicle's own hull vs its riders.** Correction (2026-11-24): a mounted
  rider's root is *not* under the vehicle. `AttachmentSystem::mount` parents
  it to a helper beside its original parent that follows the seat, so the
  own-movement-root rule does not cover it. A rider's read-only hitbox is
  an obstacle in the world tree and could push the vehicle's hull spheres.
  Phase 3 must check this (it may already matter for the pickup's driver) and
  give the carry/seat connection an exclusion if so.
- **Category bits** stay available as an optional filter *within* a context
  (sensors vs solids, projectile blockers); vehicles don't need them.
- `carry.set_context(other)` swaps a vehicle's context (folding wings);
  current riders move in the same step.

## Terminology and representation

- This is **instancing** in a **two-level hierarchy** (ray-tracing terms: a
  BLAS, bottom-level acceleration structure, built once in local space; a
  TLAS of instances, each a transform plus a pointer to a BLAS). A
  `CollisionContext` is the bottom level; the vehicle in the world is the
  instance. Physics engines call the same idea shared or compound shapes;
  Jolt's object layers and broad-phase layers are the filtering half.
- At runtime a rider switches by **pointer swap**: one map entry from its
  movement root to a context id (`StaticContactSystem::set_outbound_context`).
  Nothing is copied or rebuilt.
- Storage is a `ContextSet` per context (`collidable_bvh.rs`): its surfaces
  in the context frame's local coordinates. Up to 32 surfaces are scanned as
  a flat list; larger sets build a BVH lazily on their first query after a
  change. Moving the vehicle changes no local geometry (poses compare within
  float noise), so nothing is rebuilt or refit while driving.
- Identical vehicles could later share one set (copy-on-write once
  destruction diverges them); not needed yet.

## Movement basis and collision context

Two separate questions, kept apart in this design:

- **Movement basis**: whose motion a rider inherits (Unreal calls the same
  thing a "movement base" / based movement). Carry gives a rider the deck's
  frame as its movement basis. Today boarding and staying aboard are zones;
  a later refinement can take the basis from the support surface under the
  feet, with the zone as the "still aboard while airborne" grace, once
  support queries report which context they touched.
- **Collision context**: what a rider collides with. It cannot come from
  contact (a context-only floor can't be touched before the rider points at
  it), so it switches on a zone or bounds overlap, before contact; and an
  enclosed interior must switch it so the surrounding hull is excluded.

## Carry connection

`Mountable.carry()` is a mount that:

- keeps the rider's offset (no snap to an eye point) and follows the deck's
  position and full rotation, so a rider pitches and rolls with a tilting
  deck and keeps its place on it; stepping off levels the rider, keeping
  its heading;
- leaves velocity and walking on, and converts velocity on the boundary:
  world minus vehicle becomes local on boarding, local plus vehicle becomes
  world on leaving, so a jump onto a moving deck lands softly and stepping off
  carries the vehicle's speed;
- starts when the rider's feet enter the carry zone, ends when they leave a
  slightly larger one (no flicker at the edge), and has no occupant limit;
- switches the rider's outbound context atomically with the connection.

Gravity stays world-down: on a tilted deck it has a component along the
deck, which friction resists (a steep enough tilt slides riders, as it
should). In XR a tilting deck tilts the view too; that is the intended
feel for now, with an upright-only option possible later if it is too much.

## Control: vehicle scripts, not the attachment system

The attachment system makes and removes connections and emits `MountStarted`
/ `MountEnded`. It no longer touches input. A vehicle's script decides whether
the rider can walk: `event.rider.set_locomotion_enabled(false)` on mount and
restore on dismount. A forced unwind still emits `MountEnded`, so the script
restores walking. Snapped seats still freeze velocity in the attachment system,
since that belongs to the connection's follow policy, not to control. The
full control-layer stack in the movement-authority ticket can wait for real
nesting; "whoever turned it off turns it back on" suffices for now.

## Multiplayer

- Latched per-player state `carried_by = { vehicle, kind: carry | seat, epoch }`,
  sent reliably on change and in the join snapshot, never per frame.
- Pose packets carry the epoch byte. While carried, the position is sent
  relative to the vehicle; a packet whose epoch does not match the latest
  latched state holds the last good pose instead of snapping.
- The seated flag uses the same latched state.

## Plan

**Phase 1: input out of the attachment system**
1. Remove `snapshot_input` / `suspend_input` / `restore_input` from
   `attachment_system.rs`; a Rider no longer needs an input to mount, and
   removing its input no longer unwinds a mount. Velocity freezing stays for
   snapped seats.
2. MMS: `Rider.set_locomotion_enabled(bool)` and `Rider.locomotion_enabled()`
   act on the rider's own input (`Input` translation or `InputXRGamepad`
   locomotion); no-ops when the rider or its input is gone.
3. A shared MMS helper turns walking off on `MountStarted` and restores the
   previous value on `MountEnded`; every current Mountable uses it (pickup
   driver, passenger and bed seats, display car, boxcar).
4. Tests: mounting with no input component; the attachment leaves input alone;
   the helper disables and restores walking, including after a forced unwind.

**Phase 2: collision contexts** (`static_contact_system.rs`, `CollidableBvh`).
First slice implemented 2026-11-24, pending a build: the `CollisionContext`
component (MMS `CollisionContext {}` / `.passthrough(bool)` / `.enabled(bool)`),
per-context `ContextSet`s kept out of the world tree, outbound contexts per
movement target with sweeps in context coordinates, and passthrough to the
world excluding the context frame's subtree; three contact tests. Not yet:
rays/projectiles entering contexts (step 9), and lower-body floor support
(`support_index` is still world-only, so a rider's feet won't find a deck
until phase 3 adds contexts there).
5. `CollisionContext` component; subtree membership; excluded from the world BVH.
6. Local BVH per context, lazily built and dropped.
7. Outbound context per mover; sweeps in context coordinates.
8. Passthrough to the parent excluding the vehicle's subtree.
9. Rays and projectiles entering contexts.
10. Tests: pedestrians never touch context zones; a rider collides with the
    deck and the world but not the hull; a rocket hits interior geometry; the
    hull ignores its riders.

**Phase 3: carry**. First slice implemented 2026-11-24, pending a build, as
platform-style carry rather than re-parenting (`attachment_carry.rs`): each
fixed step, after velocities integrate and before contacts, a carried root
moves with its deck frame (the point it stands on and the full rotation)
and its velocity turns with the deck. While carried, a mover's shape is
taken in the deck's coordinates, so a tilted capsule stays usable; the
passthrough world query reuses that shape and ignores the tilt. Nothing is re-parented, so velocity,
gravity, AVC, cameras and the body solver are untouched, and carried riders
are not "mounted" (no contact skip, no seated posture). `Mountable.carry()`
boards on `auto_mount_zone` and ends on leaving `entry_zone`;
`.context(deck)` sets the rider's outbound CollisionContext; boarding
subtracts and leaving adds the deck's velocity; the vehicle's own movers
ignore its riders' zones. A carry needs a CollisionContext for its deck
floor. Test: `vehicle_carry_tests.rs`. Remaining: lower-body floor support
on context decks.
11. `Mountable.carry()`: offset preserved, follow channels, no control
    hand-off, no occupant limit.
12. Zone-driven boarding and leaving with hysteresis.
13. Velocity conversion on the boundary.
14. Outbound context switch with the connection.
15. Acceptance: a player on a deck that accelerates, brakes and turns stays
    within a few cm of their spot, never snags, leaves at the deck's speed,
    and can crouch and kneel there (lower-body IK on a moving support frame).

**Only the movement-contact index splits contexts out** (2026-11-24):
`CollidableBvh::with_contexts()`. Projectile, swept-zone and spring-bone
indexes keep context zones in the world, so rockets and sensors hit decks
and interiors without any extra step (this covers step 9).

**Phase 4: vehicles**
16. Car: deck context (interior floor, wing tops, roof), bigger carry zone,
    grab-the-controls driving instead of the driver mount.
17. Pickup: the bed becomes a carry with its own context; drop the bench
    auto-mount; cab seats stay snapped seats. Done 2026-11-24, pending a
    build: `pickup_bed_deck()` (board zone from the floor to 2.2 m so jumping
    in over the walls counts; the old entry zone as the stay zone; a context
    with floor, walls, cab wall, tailgate and bench boxes on the truck root).
    Cargo in the bed (the hot dog stand) is not in the context yet, so riders
    walk through it. The deck frame (`mount_anchor`) and the context ride the
    chassis, so riders tilt and lift with the hydraulics as well as with the
    truck root on a ramp; `CollisionContext.exclude(truck)` keeps passthrough
    skipping the whole truck.
18. A mech in the target practice range.

**Phase 5: multiplayer**: latched `carried_by` + epoch on pose packets; the
seated flag on the same state.

**Phase 6: seated cab (later)**: `Mountable.seated(hip, feet)`, a body-solver
`Seated` mode with procedural seated legs, a head-height re-centre policy
(continuous, or set on entry plus a button), and an authored seated pose once
pose layers exist.
