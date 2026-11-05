# Vehicle terrain contact and rollover

Status: proposed native physics work; not implemented by the ship example.

The station pickups currently integrate translation and yaw through `Velocity`.
They now have four spherical `Collidable.slide()` contact zones approximating
the lower chassis and upper cab, plus static body proxies for other actors.
Contacts ignore the truck's own descendants and accumulate corrections from
zones sharing its movement root. This provides basic wall/terrain separation
and sliding, without wheel support, gravity-driven settling or rollover.
Mounting and driving commands remain separate responsibilities.

## Limits of the existing slide response

`Collidable.slide()` constrains an externally supplied pose and clips inward
linear velocity. It does not compute contact torque, angular inertia, suspension
or rollover. Its moving boxes require axis alignment; upright capsules can yaw
but cannot pitch or roll. Uniform spheres accept arbitrary rotation, but a
single sphere is a poor approximation of the pickup's long body.

Adding a mover to the current truck also needs self-collision filtering: its
own static body panels must not push its chassis out of itself. A rotated box
alone would not solve this, and attaching gravity alone would not make the
wheels support the truck correctly.

## Proposed implementation

Use one native dynamic chassis per truck, with mass, centre of mass, angular
inertia and linear/angular velocity. It must be the sole motion integrator;
do not also integrate that chassis through the current `Velocity` path.
Keep the retained combined visual mesh separate from simplified collision.

Represent the chassis with a small compound of convex shapes, with collision
filtering by body identity. All truck parts belong to that body; body-to-body
contacts can then include the other pickups. Terrain uses the generated hill
triangle mesh, shared by rendering and collision, rather than a separate height
approximation. Walls, trunks and furniture remain static world geometry.

Sample four wheel contacts with downward suspension sweeps. Spring/damper forces
support the body; limited tyre friction supplies traction and steering at the
contact points. Apply forces at their actual offsets from the centre of mass,
producing torque. Chassis contact impulses and friction handle sliding on the
side or roof after a rollover. Downward rays alone can miss ledges and steps;
sweeps and chassis collision remain necessary.

Allow pitch and roll, keep angular momentum while airborne, and avoid an
automatic upright constraint. Side slopes, fast turns and an off-centre landing
can then tip the truck naturally. Tune the centre of mass and suspension for a
clear risk of rollover without making ordinary driving unstable.

Keep attachment outside the solver. Player and NPC adapters feed the same
vehicle command consumer; it applies driving forces rather than overwriting
velocity or resetting angular motion every frame. The proposed
[linear/angular acceleration requests](vehicle-command-inputs.md) remain the
driver-facing contract. The current `pickup_driving` reset/translate/rotate
prototype must be replaced for a dynamic truck so steering cannot erase gravity,
impact response or rollover momentum.

Use a bounded fixed timestep, cache collision shapes/terrain, and sleep parked
vehicles. Do not add per-truck MMS update callbacks or rebuild combined geometry
as the truck moves. Hardware/controller input should not be needed to test it.

## Verification before enabling it in the station

- Drive obliquely into a wall: inward movement stops while tangent motion slides.
- Cross the rendered hills: wheels support the chassis without sinking or hovering.
- Traverse a side slope and land on one wheel: sufficient torque rolls the truck.
- Remain on the roof after tipping; steering does not snap the truck upright.
- Compare fixed-step results under different display update rates.
- Check parked sleep/wake, self-collision exclusion, other trucks and unloading.
- Confirm mounted players follow the seat while pedestrian motion stays suspended.

The orchard hill change is independent: exposed crests are now about 2.4 m
above the deck, versus about 0.4 m before, and their base diameters are about
8 m. The four retained implicit chunks still clip their underside into the deck.
These larger hills do not themselves add vehicle collision or rollover physics.
