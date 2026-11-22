# Sakura Twilight pickup and ramp

The first slice is implemented in Sakura Twilight. Headless checks cover
the complete drive from the roof to the road, throttle release and coasting,
front wheel steering, wheel roll, and pitch on an incline. All five buildings
use the existing destructible floor and roof sections.

Use the native `Velocity` step to integrate drive commands for the Sakura
Twilight pickup. The local acceleration provider supplies throttle, steering,
and braking. Neutral throttle removes thrust and leaves surface friction to
slow the truck. Steering follows actual signed forward speed. The front
wheel pivots show steering and all wheels roll from vehicle speed.

Make the pitched ramp a static collision surface. Keep the fast primitive
path for ordinary upright boxes and reuse retained triangle contacts for
inclined surfaces. Preserve the existing vehicle mover zones and shared
render meshes. Validate the ramp with the truck and a walking capsule.

An upward contact normal can align the chassis on supported slopes. When the
support changes, preserve heading and apply limited pitch and roll. Treat
gravity torque about partial support, suspension forces, bounce, and rollover
as the [terrain-contact solver follow-up](../draft/vehicle-terrain-contact.md).
That solver needs actual wheel or chassis contact points, mass and inertia,
and angular contact response.

Probe from each wheel mount along its suspension travel. Keep the hit point,
surface normal, and signed compression at each wheel. Fit a support plane
when three or more contacts agree. With fewer contacts, use the individual
surface normals and apply forces at the supported points. An unsupported
wheel contributes zero force. Integrate the torque from each force about the
center of mass. At the transition off a level deck, the rear supports can then
produce nose-down rotation while the front suspension extends toward the ramp.

If the collision broad-phase file grows further, move shape-specific surface
contact into a cohesive module beside `box_sweep.rs` and `triangle_collision.rs`.
Keep retained broad-phase ownership and both flat and inclined contact paths
allocation-free at query time. Compare pickup, avatar, and projectile contact
tests and measure static-world BVH maintenance and incline query time.

When separating Sakura Twilight's larger scene script, move the city and
street access geometry into a scene-specific asset and keep network ownership
and player wiring in the entry point. Keep the existing per-floor batches,
shared materials, and collision/damage ownership. Validate the scene load and
the same number of retained draws for each intact building before the split.
