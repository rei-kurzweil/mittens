# Zone observer candidate filtering and broad phase

Status: tracker / proposed follow-up. No filtering or spatial-index API has been
implemented by this document.

## Current behavior

`Zone.enable_events()` opts a sensor into observation. Detection still polls
once per frame after poses settle; the notifications are enter/exit events, but
intersection calculation is not itself driven by movement events.

`ZoneObservationSystem` scans world components to collect enabled sensors and
enabled slide collidables with resolved movement targets, excluding mounted
roots. With no enabled sensors it stops after sensor discovery. Otherwise it
checks every sensor against every eligible mover: O(sensor count × mover count).
Bare passive zones and static collidables are not observation candidates.

`overlap_zones` performs a world-AABB rejection before narrow-phase overlap.
Downward capsule/box crossing checks additionally catch fast falls. These are
per-pair checks, not an indexed broad phase, and frames/bounds can be recomputed
for the same zone across many pairs. The rendering BVH is not a Zone index.

Static contact is a separate consumer: slide movers versus static collidables,
with AABB rejection, a downward floor sweep, and up to six correction passes.
Its candidate/test/iteration counters do not currently profile the observer.
Shared acceleration must preserve each consumer's candidate and response rules.

## Intended improvement

Only explicitly relevant groups should be checked. A player fall pit needs
player movers, a mount entry region needs eligible riders at interaction time,
and an interaction sensor may need only a named subtree or particular roles.
Those are different consumers; enabling events should not imply all-zone pairing.

Zone roles already exist as metadata, but event observation currently has no
role, group, mask, or candidate-root filter. Choose the public filtering contract
before optimizing it. Potential shapes include explicit candidate roots,
role-based inclusion, or category/mask pairs. These are alternatives to evaluate,
not committed MMS spellings. Avoid a selector query on every pair or frame.

## Work items

1. Measure sensor/mover counts, candidate pairs, AABB rejections, exact tests,
   sweeps, and detection time in the desktop/Corp demos and a dense fixture.
2. Define explicit candidate filtering and its relationship to Zone roles,
   slide mode, mounting, disabled components, and collision response masks.
3. Maintain an enabled-consumer registry and cache resolved frame references and
   world bounds, invalidating on structural/configuration/reference changes.
4. Use a spatial broad phase (grid, tree, or dedicated Zone BVH) to produce
   candidates. Include swept bounds for fast-moving capsules and separate
   stationary sensor/static-surface data from moving zones.
5. Re-evaluate pairs affected by actual pose/shape/filter changes; retain valid
   overlaps for unchanged pairs. A changing observer can affect stationary
   movers too. Do not simply skip movers whose transforms were not marked dirty.
6. Preserve stable event order, pair deduplication, and existing disable/removal
   retirement rules. Filtering changes must not silently retain stale pairs.
7. Benchmark improvement against the baseline and require identical event
   sequences for the supported overlap/crossing contract.

## Correctness cases

Cover overlapping broad-phase bounds without actual overlap, sensor motion,
fast downward crossings, tangent contact, nested/transformed frames, `Zone.at`
references, spawn/attach/reparent/remove, enabled-event toggles, mounting,
filter/role edits, and respawn plus `Velocity.reset()` history invalidation.
Sensors remain non-solid unless separately opted into physical contact.

General continuous sweeps, tilted/sheared contact frames and dynamic-body
response are separate follow-ups. Do not claim them as part of an index change.

## Related work

- [Zone events and pits](zone-enter-events-and-teleport-pits.md)
- [Query foundation](interaction-zone-collision-query-foundation.md)
- [Gravity/contact drivers](gravity-and-acceleration-velocity-drivers.md)
- [Surface friction](surface-friction-for-static-collidables.md)
