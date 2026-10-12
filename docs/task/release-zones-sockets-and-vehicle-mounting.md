# Task: interaction zones, sockets, and vehicle mounting

## Status and outcome

Planned. Part of [grabbing, poses, and release zones](epic/grabbing-poses-and-release-zones.md).
The broom asset is present in [E2](../../examples/e2.mms). The
[E2 broom first slice](e2-broom-mounting-first-slice.md) defines the focused
pickup/release-to-mount/dismount test before full vehicle physics.
The [broom flight follow-up](broom-flight-followup.md) defines the next motion
and input tasks and a separate MMS vehicle example.

Design updated 2026-09-07. The filename is retained for existing links.
This is also the vehicle-entry/mounting design record; vehicle driving and
flight behavior still require implementation policy below.

Foundation decision, 2026-09-08: `Zone` is an explicit detection-only component.
It reuses `CollisionShape` and shared collision-query geometry, but it is not a
`Collision.kinematic()` body and never opts into `CollisionResponse`. See
[interaction zones on the collision-query foundation](interaction-zone-collision-query-foundation.md).

## Terminology and activation

The general mechanism is **event-driven user attachment negotiation**. A
pointer gesture, held-object release, or configured proximity event requests
an attachment; eligibility and interaction rules determine whether and how it
commits. This is broader than grabbing or mounting, but does not replace their
author-facing semantics.

Keep the existing `Grabbable` name and pickup behavior. `Mountable` is the main
proposed new component: when its mount interaction is initiated by a grab
gesture, the attachment direction is the opposite of grabbing an object.
The user attaches to the target instead of the target attaching to the user.
Here, "user" means the explicitly resolved rider/movement root or configured
camera rig, not the pointer transform itself.

Keep these independently authorable:

- **Grabbable**: a tree that can be picked up and held by a pointer.
- **Mountable**: the proposed component declaring that a target accepts a
  rider, with a mount anchor and mount/dismount policy. It need not be
  grabbable; a broom can support both capabilities, while a car need only
  support entry. An entry affordance can refer to the vehicle's mount target.
- **Eligibility zone**: a volume determining which candidate may act, with
  accepted capabilities, exclusions, occupancy, and optional preview feedback.
  The volume does not by itself specify an activation or a destination.
- **Activation policy**: pointer grab/press initiation, release of an active
  held tree, or explicitly enabled proximity activation. Use the pointer's
  configured gesture source rather than requiring a particular mouse button,
  XR grip, or trigger. Cancellation is not an intentional release activation.
- **Attachment anchors**: the moving tree's authored contact/rider transform
  and the destination socket/grip/seat transform, including orientation. Align
  the former to the latter; the eligibility volume is not the snap transform.
- **Action**: hold an object, socket a prop, or mount a rider, with explicit
  follower and target roles. A common vocabulary does not require one component
  or system to own all these actions.

Start with one attachment configuration on each `Grabbable` and `Mountable`,
which may share an internal `AttachmentRule` struct. Both components may belong
to the same parent tree. Multiple alternative rules per component remain a
possible extension, not a requirement of the initial API. Keep implicit default
pickup configuration for existing `Grabbable {}` authors.

`Zone` is now the agreed component packaging for reusable eligibility geometry.
`Socket` and `InteractionAction` remain possible API shapes rather than required
components. Ordinary named transforms can supply grip, contact, and riding
anchors; separate anchor component types are not required by this design.
Attachment negotiation names shared machinery, not a replacement public
component for `Grabbable`. The committed attachment relationship still needs a
concrete runtime representation in the attachment-system implementation slice.

## Negotiation and component coexistence

An attachment request identifies the initiating user/pointer, candidate target,
activation event, and any current held relationship. Resolve eligible actions,
their source/destination anchors, and which tree will follow which. Check
occupancy, capabilities, ownership, and transform cycles before committing one
coordinated transition. Negotiation means this validation and arbitration;
it does not imply a user confirmation dialog or a network protocol.

Neither `Grabbable` nor `Mountable` unconditionally overrides the other when
both are present. Author activation/context rules that distinguish their uses:

- A broom has both: grabbing its handle picks it up; releasing it in the
  eligible leg zone mounts the rider onto it.
- A car exposes a mount interaction through its door handle or steering wheel:
  the grab gesture requests rider attachment to the seat, without first holding
  the handle or vehicle.
- A bike may opt into proximity mounting using the same mount negotiation,
  without requiring a pointer grab or a preceding release.

Ambiguous eligible actions require deterministic arbitration as specified
below; one event must not both pick up a vehicle and mount its rider.

## Activation examples

| Interaction | Activation | Result |
| --- | --- | --- |
| Pick up a broom | Pointer grab initiation | Broom follows pointer/hand |
| Put a prop in a mouth socket | Release in eligible zone | Prop follows mouth anchor |
| Mount a held broom or bike near the legs | Release in eligible zone (default) | Rider follows vehicle riding anchor |
| Mount a nearby broom or bike automatically | Explicit proximity policy | Rider follows vehicle riding anchor |
| Enter a car through its door handle or steering wheel | Pointer grab/press initiation on entry affordance | Rider follows designated seat anchor |

A release zone is one specialization of an eligibility zone. Merely entering
a release-configured zone never commits. Proximity-configured mounting needs
an explicit candidate/rider relationship, entry/dwell and exit hysteresis,
and rearm rules so remaining nearby after dismount cannot immediately remount.
Specify whether proximity activation accepts a held vehicle; if it does, it
must use the same coordinated hand-release handoff as release mounting.
Car entry consumes the activation for mounting without first picking up the
car or handle; arbitrate competing grab and entry actions to select one.

## Shared zone contract

Separate the eligibility volume from the destination transform. A zone owns
its geometry and coordinate frame/placement. Attachment configuration on
`Grabbable` or `Mountable` owns activation, accepted candidates/capabilities,
zone conditions, and source/destination anchor references. A zone does not
choose the attachment action. Socket API packaging remains open.

Rules should explicitly reference zones when the relationship is known. They
may alternatively enumerate zones beneath the initiating pointer's resolved
interaction owner (avatar/rider movement tree or configured camera-rig root)
and filter by semantic role. Do not search the whole world forest or infer an
arbitrary humanoid. Resolve and retain that owner when the grab begins so
release uses the same participant context; topology and zone changes invalidate
the cached candidate set before synchronous revalidation.

Define which point or bounds of the candidate tree or rider is tested, overlap versus
containment, exclusions, and any entry/exit hysteresis. Reevaluate eligibility
at activation; preview state is not sufficient authority. Choose at most one
winner using an explicit deterministic priority/distance/tie-break policy.
A zone may reject incompatible or occupied attachments.

Author zone geometry through `Zone.cube(...)`, `Zone.sphere(...)`, or
`Zone.capsule_y(...)`. The zone embeds the same shape value and uses the same
query math as collision detection, but does not generate a collidable,
collision-shape child, or renderable. Zone queries are detection-only: they do
not use collision response or assign a mechanical static/pose-driven mode.
For the first broom slice, transform the authored probe point into each zone's
local space and classify it synchronously. Inclusion accepts its boundary;
exclusion wins on its boundary. Collision enter/exit or cached preview state may
drive feedback, but release must query current transforms again before commit.

Release handling must choose ordinary drop or a zone action as one coordinated
handoff. Existing grab release restores the original parent while preserving
world pose; it must not race a zone's new attachment. Invalid or removed zones
fall back to a normal release. Specify explicit detach/regrab and cleanup when
an attached tree, anchor, or owner is removed.

## Socket action: prop to avatar

A mouth zone accepts a suitable prop, such as a lollipop or marijuana cigarette.
On release, the prop's authored grip/contact transform aligns to a mouth socket
with an authored position and orientation. The prop thereafter follows that
socket. Snapping the prop's arbitrary root origin is not sufficient.

The attachment may use tree reparenting or an effective transform-parent basis;
select the mechanism after reviewing existing transform contracts. The required
behavior is explicit and persistent following, regardless of that choice.

## Mount action: avatar to broom

While held, the broom follows the grabbing pointer's nearest ancestor transform.
An avatar-relative zone near the legs accepts it, with the torso excluded.
Releasing there mounts; merely moving through the zone does not.

The action reverses which tree follows which:

1. Validate the broom, rider, riding anchor, eligibility, and physics handoff.
2. Remove the broom's hand attachment and establish its independent world/physics
   basis, preserving its world pose before the authored mount alignment.
3. Attach the avatar's designated movement/root transform to the broom's riding
   anchor and apply the authored riding alignment/pose.
4. Give broom physics authority over vehicle motion; coordinate avatar locomotion
   and collision ownership so two drivers do not compete.

Never attach the avatar beneath a broom that is still beneath the avatar's hand.
Check both structural and effective transform-basis cycles. Commit the operation
atomically from the consumer's perspective; failure must leave a valid ordinary
release, not a partially mounted tree or a lingering hand attachment.

Define dismount/regrab behavior, restoration of avatar movement/collision
ownership, and cleanup if the broom disappears before enabling the feature.
Vehicle controls, flight forces, velocity inheritance, and collision details
need a concrete policy during implementation; this ticket does not presume
that an existing physics path already provides them.

## Nested movement authority and canonical controls

Mounting transfers the built-in movement mapping one layer outward; it does not
disable the input device or XR tracking. `InputXR` must continue applying the
tracked HMD pose. `InputXRGamepad.disable()` suppresses only its automatic
pedestrian locomotion while its canonical axis/button events remain available
to the mounted vehicle controller. `enable()` restores pedestrian locomotion.
Desktop `Input.disable()` and `enable()` apply the same authority gate to its
built-in keyboard movement mapping.

Model mounted contexts as a stack of movement authorities. The innermost rider
relinquishes movement to the mech, the mech can relinquish movement to a larger
carrier, and so on. Canonical controls are offered to the top active layer; an
inner layer may remain attached and posed but must not also integrate the same
movement. Dismount pops exactly one layer and restores the immediately inner
authority. Removal and failed mount commits must unwind the affected layers and
must not leave an input mapping disabled.

The live enable/disable methods are the first gating mechanism, not the complete
stack manager. Attachment state should eventually record which input mapping it
suspended and its prior state, so nested mounts restore state transactionally
instead of blindly enabling every inner controller.

## Acceptance criteria

- Releasing a compatible prop in the mouth zone aligns its authored contact to
  the socket and follows subsequent avatar/head motion; regrabbing detaches it.
- Entering either zone while holding does not commit. Releasing outside or in
  an incompatible zone performs ordinary release.
- The leg zone mounts the broom on release; torso-only proximity does not.
- Mounted avatar motion follows the physics-driven broom without a transform
  cycle, double driving, or a transient return to the old grab parent.
- Overlapping zones select one deterministic action; removed/occupied targets
  and failed handoffs leave valid attachment and grab state.
- A target with both `Grabbable` and `Mountable` picks up on its authored grab
  interaction and mounts on its authored mount interaction. One activation
  cannot commit both directions; existing ordinary pickup semantics remain.
- Dismount and owner removal restore a usable avatar and consistent physics state.
- An explicitly proximity-configured vehicle can mount without a grab/release
  gesture; its rearm policy prevents repeated mounting while still in range.
- Activating a car handle or steering-wheel entry affordance mounts to its
  authored seat without grabbing the vehicle. Failure leaves the rider's prior
  valid state intact; only a release-triggered failure falls back to a drop.
- Source/contact and destination anchors determine final position and rotation
  independently of zone size and origin, for props and riders alike.
- A camera-only pointer may hold props, but rider/body zones require an explicit
  rider or mountable camera-rig configuration; do not invent humanoid legs.

## Related work

- [Rider + Mountable attachment-system first slice](rider-mountable-attachment-system-first-slice.md)
- [Interaction zones on the collision-query foundation](interaction-zone-collision-query-foundation.md)
- [Spatial, collision, and physics naming](spatial-collision-and-physics-naming.md)
- [Effective transform-parent basis resolution](../draft/effective-transform-parent-basis-resolution.md)
- [Transform pipeline](../spec/transform-operators.md)
- [Grab pose transitions](grab-animation-and-pose-transitions.md)
