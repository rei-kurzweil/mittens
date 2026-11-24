# Damage integrity, removed volume, and operability

Status: proposed, 2026-11-24. Not started.

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

1. `Integrity` component: max/current, thresholds, MMS reads and events;
   fed by `Damage` overlapping its zones (not only Destroyable surfaces).
2. Destroyable volume accounting: starting volume at partition, per-cut
   volume deltas, `Integrity.from_volume()` binding.
3. Pickup: engine block integrity with smoking/burning/wrecked; wheels with
   volume-based inoperable thresholds.
4. Drive: per-wheel operable flag; wrecked stops the drive and ejects riders.
5. Smoke and fire attachments on thresholds; explosion and despawn on
   wrecked (reuse `player_death`'s burst for the explosion).
6. Tests: a part with no Destroyable loses integrity under Damage; volume
   fraction falls after cuts; thresholds fire once; a wrecked truck stops.
