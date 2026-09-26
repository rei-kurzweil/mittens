# Tracker: grabbing, poses, and interaction zones

## Status

Planning, updated 2026-09-07. This records the design discussion; no runtime changes
are part of this work. Component and system names proposed below are provisional.

## Conclusions

Mittens operates on component trees. A grabbable object means a component
tree with a resolved movable transform, not a separate entity abstraction.
Grabbing temporarily attaches that tree to the nearest ancestor transform
of the `Pointer` that initiated the grab gesture.

The shared abstraction is event-driven user attachment negotiation. Preserve
`Grabbable` as the pickup component; `Mountable` is the main proposed addition.
A grab gesture resolved to mounting reverses the attachment direction: the
user's designated rider/root follows the target's mount anchor, instead of the
target following the user's pointer/hand. Activation remains configurable so
release and explicit proximity events can request that same mount operation.
Negotiation resolves eligibility, anchors, attachment direction, and competing
actions before committing a coordinated handoff.

A broom can be both `Grabbable` and `Mountable`: grab to hold, release near the
legs to ride. Neither capability globally overrides the other. Zones and
activation rules distinguish the interactions. `Zone` is the agreed
detection-only component packaging and reuses authored `CollisionShape`
geometry. Separate `InteractionAction` or `Socket` components remain possible
packaging, not API commitments; ordinary transforms can serve as the authored
anchors.
Start with one attachment configuration per `Grabbable`/`Mountable`, potentially
sharing an internal struct. Multiple alternatives remain a future option.
Components own the interaction conditions; zones own geometry and placement.

Pointing and grabbing can bring a distant tree into reach. The desired resting
placement puts its bounding-box boundary flush with the grabbing hand, with
configurable clearance, in XR and desktop. Untracked desktop with a humanoid
needs an explicit synthetic hand target and an animated IK reach into holding.
Camera-only desktop instead uses a camera-relative hold anchor with no arm or
hand animation. Webcam/MediaPipe integration is future work, not a prerequisite.

A `grab_animation_system` is a reasonable consumer of grab state: it selects
and transitions the appropriate pose. General pose interpolation belongs in
shared pose infrastructure, and attachment/placement belongs in grab handling.
Pose evaluation should update resolved component transforms; skinning is a
consumer of those transforms, not a requirement for interpolation.

Interaction zones share spatial eligibility and activation arbitration, but their
attachment outcomes differ. A mouth socket attaches the released prop to the
avatar. A broom mount attaches the avatar to the released broom. Entering a
release-configured zone while holding only establishes eligibility; releasing
commits the action. The general API must also express pointer grab/press entry
(car handle or steering wheel) and explicitly configured proximity mounting.
Grabbable and mountable are independent capabilities. Eligibility volumes,
activation policies, source/destination anchors, and attachment direction are
separate concepts; the zone/vehicle ticket defines the shared terminology.

Automatic XR controller behaviors should share one configurable component
surface covering locomotion and interaction. `InputXRGamepad` already provides
automatic locomotion, enabled by default, with builder configuration. Extend
`InputXRGamepad` itself with builder options for minimum grab levitation
distance; no separate automatic-behaviors component is needed.
While grabbing, the stick not assigned to locomotion should adjust effective
`min_grab_distance` continuously using its vertical axis (normally right stick;
up farther, down closer). Idle input must not change distance. The placement
ticket records mapping, live destination updates, and remaining default/lifetime
choices; this is proposed behavior, not an existing capability.
Desktop scroll-wheel input should adjust the same live hold distance, down to
zero clearance at the hand or camera-relative hold anchor.

Priority entry point: [Desktop interaction priorities](../../desktop/interaction-priorities.md).

## Tickets

- [ ] [Hand-relative, bounds-aware grab placement](../grab-hand-relative-bounds-placement.md)
- [ ] [Size a Grabbable for comfortable holding](../grabbable-held-size.md)
- [ ] [Grab poses and reusable pose transitions](../grab-animation-and-pose-transitions.md)
- [ ] [Release a held item onto a body mount point](../held-item-body-attachment.md)
- [ ] [Interaction zones, sockets, and vehicle mounting](../release-zones-sockets-and-vehicle-mounting.md)
  - [ ] [Zone collision-query foundation](../interaction-zone-collision-query-foundation.md)
  - [ ] [E2 broom attachment first slice](../e2-broom-mounting-first-slice.md)

## Existing foundations and delivery order

The current `GrabbableSystem` already reparents to the pointer's nearest
ancestor transform, levitates toward a destination derived from subtree
bounds, and restores the original parent on release while preserving world
pose. `Pointer.min_grab_distance` already configures clearance; its defaults
are 0.05 m with a controller driver and 0.75 m otherwise. These are distances
from the pointer ray origin, not a complete hand-contact contract.

Start by agreeing on grab state, hand-anchor resolution, and release ownership.
Placement and pose work can then use the same active-grab state. Reuse the
[existing pose-layer ticket](../avatar-pose-transition-layers.md) rather than
creating another interpolation runtime. The focused E2 broom slice now provides
the first attachment test, using desktop distance adjustment as needed. It
does not depend on mouth sockets or finished pose transitions; full vehicle
physics remains subsequent work.

## Integrated acceptance

1. In XR, point at and grab a rigid component tree: it approaches the hand and
   settles at the configured boundary clearance.
2. On desktop without tracking, the chosen hand smoothly assumes a holding
   pose through an IK reach when a humanoid is present. Camera-only grabbing
   uses a camera-relative hold anchor without requiring a skeleton.
3. Release a lollipop or cigarette near the mouth zone: it snaps to an authored
   mouth transform and follows the avatar; elsewhere it releases normally.
4. Hold the broom near the legs, outside the torso exclusion, then release:
   the broom stops being attached to the hand, the avatar snaps to its riding
   anchor, and broom physics becomes authoritative for vehicle motion.
5. Explicit proximity mounting and car-handle/steering-wheel activation work
   without requiring a preceding held-object release.
6. Desktop scrolling and XR stick adjustment move the held object continuously
   from levitation distance to zero anchor clearance, with input ownership
   preventing simultaneous zoom/scroll or locomotion conflicts.

## Remaining design choices

Exact public APIs, desktop hand selection and anchor placement, contact-point
selection, zone geometry and tie-breaking, and vehicle locomotion/dismount
policy remain to be specified in the linked tickets. The broom asset is present
and used by `examples/e2.mms`. Accurate deformed humanoid bounds are follow-on work;
rigid-tree placement should not wait for them.
