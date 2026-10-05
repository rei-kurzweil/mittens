# Collision response (removed)

`CollisionResponseComponent`, `CollisionResponseSystem`, the MMS
`CollisionResponse`/`CRSP` names, and their registration/removal intents were
removed on 2026-10-04. Old scenes using those names must be migrated; they are
no longer accepted by the component registry.

Use a `Zone` with `Collidable.static()` for a blocking surface, and a `Zone`
with `Collidable.slide()` for a mover whose pose is supplied by another system.
StaticContactSystem corrects the selected movement transform and removes inward
speed from the Velocity directly owning that transform. Contact geometry does
not own a private velocity or integrate motion.

Gravity is an ancestor of Velocity, which integrates speed into its immediate
child transform. Zone shapes use local coordinates and inherit transform scale.

The old push acceleration, damping, speed clamp, and bounce policy are retired.
Movable-body pushing is separate work in
[surface contact and coupled motion](../task/surface-contact-and-coupled-motion.md).
Legacy Collision detection and its overlap events remain pending their own
consumer audit/removal; deleting response does not delete geometric queries.

See [static non-penetration migration](../task/retire-collision-response-to-static-nonpenetration.md)
and [gravity and acceleration](../task/gravity-and-acceleration-velocity-drivers.md).
