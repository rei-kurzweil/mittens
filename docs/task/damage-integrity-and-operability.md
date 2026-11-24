# Damage integrity, removed volume, and operability

Status: proposed, 2026-11-24. Not started. Depends on
[damage impacts without ECS churn](damage-impacts-without-ecs-churn.md): damage
must arrive as a queued engine signal, not a temporary component per hit,
before anything else consumes it.

Related: [collision contexts and vehicle carry](collision-contexts-and-vehicle-carry.md),
`assets/components/fire.mms` / `smoke.mms`, `destroyable_system.rs`.

## Outcome

Any object can keep track of how damaged it is, whether or not CSG can cut
it. Scripts hear about it at thresholds (no per-frame polling) and decide what
it means: a truck's engine block smokes at half integrity, burns at a quarter,
and blows up and despawns at zero; a wheel that has lost enough of its volume
stops driving; a mostly destroyed prop despawns with an explosion.

## What exists today

- `Damage.at(point, radius, amount)` is consumed only by `Destroyable`
  owners: per-surface damage cells, and cuts once a cell passes the threshold.
  Objects without `Destroyable` (the pickup's open lower cabin, any part
  excluded from cutting) ignore damage entirely.
- `Solid::volume()` (`csg_mesh.rs`, manifold) exists but is used only in
  tests. Cuts replace chunk solids; nothing sums what is left.
- No damage or cut events reach MMS.
- Fire and smoke: `fire.mms` (a burning material plus a bounded smoke pool,
  updated by the caller) and the rocket exhaust materials.

## Delivery: one damage queue, no components

Today each `Damage.at(...)` spawns and removes a `DamageComponent`, which
changes the world's query revision and makes the next collision-index sync
revisit every collidable, once per hit. The prerequisite ticket replaces that
with a damage intent into a reusable queue drained once per frame. Integrity
reads that same queue (or the destruction system forwards per-target results
from it); it never adds its own damage path or per-hit components.

## Who owns the state

The damage itself lives where it lands: a Destroyable surface's sparse damage
map (its per-cell damage) is already the record for cuttable parts, and a
per-part hit counter is the same thing for parts that can't be cut. What that
damage *means* (smoking, a dead wheel, a wrecked engine) is owned by the
vehicle: a vehicle-level damage state that reads its parts' maps and keeps
operability, rather than a separate operability component per part.

## First version: damage only (decided 2026-11-24)

Track integrity by damage received, not by removed volume. A part's
integrity falls by the damage that lands on it (Destroyable surfaces already
sum per-cell damage; uncuttable parts count hits from the same queue), and
crossing a named threshold emits one event. Removed volume stays a later
option for parts that need "how much is physically left".

## Two measures, one component

1. **Hit integrity** for anything: an `Integrity` component (working name)
   with `max` and `current`, fed by `Damage` spheres that overlap its zones
   or bounds. Works on uncuttable parts (an engine block) and on cuttable
   ones alike.
2. **Removed volume** for cuttable parts: when a `Destroyable` partitions a
   surface it records the starting volume (sum of chunk solids); each committed
   cut subtracts the changed chunks' volume difference, so only changed chunks
   are measured. The fraction remaining feeds the same `Integrity` when the
   part asks for it (`Integrity.from_volume()`).

Either way the state lives in one place, on the part, and MMS sees:

- `integrity.fraction()` / `remaining()` for reads;
- `IntegrityChanged { fraction }` events, throttled to threshold crossings;
- named thresholds: `Integrity.max(100).threshold("smoking", 0.5)
  .threshold("burning", 0.25).threshold("wrecked", 0.0)`, emitting
  `IntegrityThreshold { name }` once per downward crossing (and upward on
  repair, if repair exists).

## Operability

Whether a part still works is a decision on top of integrity, so it belongs
with whatever uses the part, not in the destruction system:

- **Wheels:** the vehicle script listens for a wheel's "inoperable" threshold
  (say, under 50% volume left) and tells the drive. The native drive needs a
  per-wheel `operable` flag (`Acceleration.set_wheel_operable(wheel, bool)`)
  so a lost wheel stops contributing traction and steering, and the chassis
  sags or drags on that corner.
- **Engine:** "wrecked" stops the drive entirely (`set_control_input` ignored,
  or throttle clamped to zero), ejects seated riders (`Rider.set_enabled`-like
  release on the vehicle side), and plays the explosion and despawn.
- A general `Operable` flag component is worth adding only if several
  systems need to query it; start with vehicle-script state.

## Smoke and fire on damaged parts

- "smoking": attach a smoke source (`smoke.mms`) under the engine block,
  world-rooted pool like `fire.mms` recommends, so the trail streams behind
  a moving truck.
- "burning": swap to `fire.mms` (burning material + smoke), or a flame
  material like the rocket exhaust.
- Both need a time input. `fire.mms` takes `update(dt)` from the caller;
  run it only while burning (one FrameTick binding added at "smoking",
  removed at "wrecked"), or move it to animation-driven material time once
  that exists.
- More hits while burning bring "wrecked" sooner; that is just integrity
  continuing to fall.

## Plan

0. Prerequisite: damage impacts as a queued signal (no components).
1. `Integrity` component: max/current, thresholds, MMS reads and events;
   fed from the damage queue for anything overlapping its zones (not only
   Destroyable surfaces).
2. Destroyable volume accounting: starting volume at partition, per-cut
   volume deltas, `Integrity.from_volume()` binding.
3. Pickup: a vehicle damage state that owns operability, reading its parts:
   engine block integrity (smoking/burning/wrecked) and wheels' volume-based
   inoperable thresholds.
4. Drive: per-wheel operable flag; wrecked stops the drive and ejects riders.
5. Smoke and fire attachments on thresholds; explosion and despawn on
   wrecked (reuse `player_death`'s burst for the explosion).
6. Tests: a part with no Destroyable loses integrity under Damage; volume
   fraction falls after cuts; thresholds fire once; a wrecked truck stops.
