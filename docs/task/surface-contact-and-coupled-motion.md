# Task: surface contact and coupled motion

Status: proposed, 2026-10-04. Define temporary surface relationships for stable
pushing, explicit pulling, and controlled relative motion. This is a follow-up
to static non-penetration, not an extension of legacy CollisionResponse.push.
Public component and MMS names remain undecided. No runtime changes in this task.

## Problem and goal

The old pushable cubes often allowed the player to pass through them. Their
response adds acceleration away from overlapping objects; it does not establish
a same-step constraint that keeps the player and cube on opposite sides of a
shared contact surface. Delayed overlap observations and end-pose-only tests
also cannot guarantee that a fast mover encounters a thin obstacle.

Model the surface contact itself, then apply an explicit policy to the permitted
relative motion. A user pushing a movable crate should remain outside the crate,
move it predictably while pressing into it, and stop when the crate reaches a
wall. If the crate cannot accept the full displacement, the pusher must accept
only the feasible displacement too. Tangential motion should remain available
unless the selected relationship deliberately constrains it.

A pulling relationship should let a named grip or surface anchor follow a
controlled source closely and stably while respecting solid obstacles. The
relationship is temporary and should expose enough state and controls for MMS
to manage activation, allowed motion, and release. It must not hide another
private velocity or competing transform driver inside contact response.

## Existing implementation and retirement boundary

- `GravityComponent` currently supplies a coefficient cached at registration
  by `CollisionResponseSystem`. That system stores private velocity and also
  implements push, damping, speed limits, and bounce.
- `examples/gravity-fields.rs` and `examples/collision-perimeter.rs` still use
  that path. The gravity-fields demo also consumes legacy collision events and
  directly clears a responder's cached gravity coefficient.
- The [retirement task](retire-collision-response-to-static-nonpenetration.md)
  already plans to delete those response behaviors and their APIs after the
  remaining consumers are migrated or retired. Planned retirement does not
  mean those implementations have already been removed.
- `Zone` queries now provide synchronous overlap/contact geometry for a limited
  set of shapes and frames. `Collidable.static()` and `Collidable.slide()`
  constrain a proposed mover pose against static geometry. They do not resolve
  movable-versus-movable contact or provide general horizontal sweeps.
- Desktop pile crates currently use static collidable zones: they block the
  avatar at their current pose but are not contact-driven pushable bodies.
- Velocity gravity-provider discovery exists, but gravity integration and
  per-substep contact/velocity coordination remain pending in the
  [velocity-driver task](gravity-and-acceleration-velocity-drivers.md).

Do not restore CollisionResponse.push as the basis for this work. Preserve the
useful zone geometry and explicit movement-target routing. Spring-bone gravity
and secondary motion remain out of scope; revisiting their internals belongs
to separate work when those internals are more exposed through MMS.

## Relationship to attachments

This is related to attachment through endpoints, activation, channel ownership,
and cleanup, but contact has different geometric constraints.

| Relationship | What it preserves | How it ends |
| --- | --- | --- |
| Solid contact | Non-penetration; normal separation may increase, tangential sliding is free | Participants separate |
| Push contact | Solid contact plus permitted transfer of closing normal motion to the pushed owner | Pressure/contact ends or policy is disabled |
| Explicit surface grip/pull | Selected relative channels between two surface anchors, with a configured offset | Release, invalid endpoint, or configured break condition |
| Rigid attachment | Selected transform alignment/follow channels at mount points | Explicit or forced detach |

Ordinary contact is unilateral: it blocks closing through the surface but does
not pull the other participant back when they separate. Pulling requires an
explicit relationship that can maintain separation/offset in selected channels.
Do not infer attraction, gripping, friction, or rigid following from overlap.

A surface anchor identifies a location on each participant in its local frame;
the pair defines the relative contact frame. The first implementation can use
simple shape features and contact points. Multiple points/manifolds and changing
features at edges need a deliberate later design. Anchor identity should survive
ordinary movement without repeatedly capturing a new offset and causing drift.

Share attachment endpoint validation and lifecycle machinery where useful,
without assuming tree reparenting is required. Contact should normally leave
both component trees intact. Otherwise contact could change Velocity provider
eligibility, transform bases, or mounted input authority merely by touching.
Neither pushing nor gripping automatically transfers pedestrian/vehicle input.

## Contact data and ownership

A same-step query/constraint record needs:

- Both collidable zones and their current/proposed frames.
- Explicit movement targets for both owners, including offset/generated proxies.
- Any Velocity owner associated with each movement target; infer neither from an
  arbitrary nearby component nor from the contact point alone.
- World-space contact normal with a documented participant ordering, gap or
  penetration depth, local anchors, and sweep hit fraction when available.
- Stable pair identity, active policy, selected constrained channels, and
  lifecycle state. Iterative solver visits are not new contact events.

Two proxies of one motion owner must not push that owner against itself.
Reject missing, removed, disabled, singular, or unsupported endpoints with
observable diagnostics and no arbitrary fallback target. Reciprocal policies
must produce one pair solution rather than two independent transform writes.

## Initial policy: controlled pushing without penetration

Start with a pose-driven avatar capsule, one axis-aligned movable crate, a flat
floor, and a static wall. Keep the first crate upright and constrain motion to
the floor plane; free rotation, tipping, mass, and full rigid-body simulation
are later work. An explicit movable/push permission is required. Existing
static collidables retain their current contract.

1. Gather both owners' proposed motion for the current fixed substep.
2. Query relative swept motion to find first contact, not just final overlap.
3. Preserve the non-penetration inequality along the contact normal. In normal
   motion, the pusher cannot advance farther into the crate than the crate's
   accepted motion allows.
4. Transfer the permitted closing displacement to the crate's proposal, then
   constrain that proposal against the floor, wall, and other eligible blockers.
5. Propagate the crate's accepted displacement back to the pusher constraint.
   If the wall stops the crate, it also stops inward player movement. Keep free
   tangent displacement rather than freezing all movement on contact.
6. Iterate coupled pairs in deterministic order with a bounded budget, then
   commit corrected poses together and propagate transforms before consumers.

Choose and document whether independently driven opposing motion is rejected,
clamped by priority, or solved together. Do not silently select whichever
transform writer runs last. When convergence fails, retain a validated safe
pose or clamp the contested movement; expose the failure rather than knowingly
publishing penetration. Define a separate recovery policy for an initial
penetration where no previous safe pose exists.

The first policy transfers displacement, not an inferred physical impulse.
Momentum, inertia, friction, and restitution require explicit later policies.
If a participating owner has Velocity, the coordinator must update that owner's
blocked relative normal speed consistently with accepted motion. Projecting
both participants to zero world normal speed would incorrectly stop objects
that are moving together. Gravity/contact uses the same per-substep coordinator;
contact owns no extra falling velocity.

Horizontal capsule/crate sweeps and crate/wall sweeps are prerequisites for
claiming fast-motion non-penetration. Existing downward floor crossing alone
does not meet this requirement.

## Explicit surface grip and pulling follow-up

After pushing works, add an opt-in grip relationship with named source and
target surface anchors. Specify:

- Initial alignment versus captured offset, independently from ongoing follow.
- Normal/tangent translation channels and optional angular channels in a
  defined contact frame. Unselected channels remain under their existing owner.
- Desired separation, tolerances, and follow behavior. A hard constraint should
  closely match accepted source motion without accumulated lag; any compliance
  or smoothing must be configured and have explicit units.
- Activation eligibility, release, maximum reach/error, and whether a blocked
  target limits source motion or breaks the grip. Never teleport through a wall
  to satisfy an otherwise impossible anchor match.
- Conflict rules for concurrent grips, mounting, grabbing, and pose drivers.
  Reject or arbitrate channel conflicts before activation.
- Release state: preserve accepted pose and define any Velocity handoff once,
  without a hidden impulse or continued follow after release.

Pulling is not automatically enabled on every pushable object. MMS should be
able to activate/release a relationship and inspect its endpoints, actual gap,
blocked state, and active channels. Builder/setter names and serialization
remain open; serialize authored policy consistently with existing settings,
while active contacts and captured runtime solver state remain runtime state.

## Lifecycle and scheduling

Resolve geometry, coupled motion, velocity corrections, and final propagation
within each fixed substep. Render-frame batching followed by one final contact
pass is insufficient. Desktop/XR cameras and interaction queries consume the
final corrected pose; preserve explicit AVC proxy-to-movement-root routing.
Tracked HMD poses remain device observations: a body/root constraint alone
cannot prevent physical head movement, so any visual head-obstruction policy
must be separately specified rather than claiming to constrain tracking.

Refresh eligibility after reparenting or policy edits at a step boundary.
Removal, disabling, authority changes, release, and scene teardown must clear
pair/anchor state without stale follow or later restoration of an invalid root.
Use a small separation tolerance/hysteresis with documented units to avoid
contact chatter. Do not let tolerance become visible overlap or a magnetic
binding. Optional start/stay/end observations describe committed state and
should only be added for a concrete consumer, not as another asynchronous
source of physical corrections.

## Delivery and acceptance

1. Finalize contact record, owner routing, and policy boundaries alongside the
   velocity/contact coordinator. Reuse zone query geometry and identify missing
   relative sweeps; public MMS naming is not a prerequisite for headless work.
2. Implement avatar/crate pushing on a flat floor against a static wall. Provide
   a small desktop fixture that replaces the old push demo's behavior.
3. Verify tangent sliding, separation/recontact, blocked pushing, high-speed
   crossings, and equivalent results across render rates for equal fixed steps.
4. Add multiple contacts/corners and a short crate chain with stable ordering,
   bounded convergence, and observable failure. Reject unsupported rotations
   explicitly until their geometry is implemented.
5. Implement explicit surface grip/pull, channel controls, and MMS activation,
   inspection, and release after the basic coupled constraint is stable.
6. Migrate or retire legacy push/gravity demos and collision-event consumers in
   coordination with the retirement task; do not make deletion wait on pulling.

Required checks include no player/crate penetration when the crate is free or
wall-blocked; tangent freedom; no pulling after ordinary contact separates;
no anchor drift/jitter during sustained follow; blocked pull policy; disable,
removal, reparenting, and release cleanup; correct owner routing under parent
rotation/scale where supported; two proxies sharing an owner; competing
commands; and final desktop/XR publication ordering. Distinguish headless
geometry/scheduling evidence from live headset verification.

## Related work

- [Static non-penetration migration](retire-collision-response-to-static-nonpenetration.md)
- [Gravity and acceleration velocity drivers](gravity-and-acceleration-velocity-drivers.md)
- [Attachment valence and Grabbable unification](attachment-valence-and-grabbable-unification.md)
- [Attachment movement authority and input routing](attachment-movement-authority-and-input-routing.md)
- [Interaction-zone query foundation](interaction-zone-collision-query-foundation.md)
