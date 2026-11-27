# Task: separate mounting from movement authority and input routing

Status: planned, 2026-09-12. Follows the first `Rider` + `Mountable` mount
slice and its `mittens-corp` vehicle demonstration.

2026-11-24: narrowed by [collision contexts and vehicle carry](collision-contexts-and-vehicle-carry.md).
Slice 1 below (input out of `AttachmentSystem`) is being done there as its
phase 1, with a simpler authority rule: the vehicle's MMS script turns the
rider's walking off and back on (`Rider.set_locomotion_enabled`), and whoever
turned it off turns it back on. The full control-layer stack waits for real
nesting (a vehicle inside a carrier). The first consumers are now the sakura
pickup (driver, passenger and bed seats) and the `car.glb` mech, not only
`mittens-corp`.

## Problem

`AttachmentSystem` currently performs two unrelated jobs:

1. It validates and commits the Rider-to-Mountable relationship, keeps the
   transform relationship alive, and unwinds it safely.
2. It snapshots the Rider's pedestrian input setting, disables that automatic
   movement mapping on mount, and restores it on dismount.

The second job is movement authority and input routing, not attachment.  In
the current car example, raw controller events remain available and MMS keeps
its own `vehicle_state.mounted` flag from `MountStarted`/`MountEnded` to decide
whether it should drive the car.  That is a useful first slice, but it is not
a general control-context model.

Do not add desktop arrow-key vehicle controls as a workaround.  Vehicle
controls need a deliberate control-layer contract, not another global handler
whose behavior happens to be gated by an MMS boolean.

## Goal

Make attachment responsible only for relationship eligibility, atomic
mount/dismount, transform alignment/following, occupancy, and lifecycle
observations.  Move automatic-movement suppression and restoration into a
separate generic movement-authority/input-routing system.

The system must be generic: a car, broom, mech, or outer carrier may supply a
movement layer.  It must not be a car-specific rule embedded in
`AttachmentSystem`.

It is also not a `VehicleSystem`.  Vehicle motion is only the first consumer
of a generic movement-authority layer; a mech, moving platform, carrier, or
future non-humanoid pilot can use the same handoff.

```text
interaction activation
        |
        v
AttachmentSystem
  validate + commit/unwind Rider <-> Mountable
  publish MountStarted / MountEnded
        |
        v
MovementAuthority / InputRoutingSystem
  maintain the active control-layer stack
  route input to its top layer
  suspend and restore lower automatic movement mappings
        |
        v
pedestrian / vehicle / carrier controller
  consumes its selected actions and owns its motion
```

## Intended contract

### AttachmentSystem

It must:

- resolve the rider, mountable, mount points, entry eligibility, and cycles;
- atomically establish or remove the transform-follow relationship;
- retain the authoritative active attachment edge for validation and cleanup;
- emit `MountStarted` only after commit and `MountEnded` after every normal or
  forced unwind.

It must not inspect, mutate, snapshot, or restore `Input`,
`InputXRGamepad`, or another control component.

### Movement authority and routing

Introduce an engine-owned, stack-based authority model.  A mounted controller
may push a layer above the rider's pedestrian layer; dismount removes exactly
that layer and restores the captured lower layer state.  Nested mounts must
work transactionally: rider -> mech -> carrier is three distinct layers, and
unwinding one must not blindly enable every lower mapping.

The layer must distinguish:

- raw device observation (keyboard/controller events remain available to the
  input backend);
- automatic transform locomotion, such as `I {}` WASD or
  `InputXRGamepad.locomotion()`;
- the active consumer of movement actions.

The routing owner must receive authoritative mount state directly from the
attachment relationship, rather than infer it from entry-zone membership,
disabled input, tree shape, or a scene-local boolean.  `MountStarted` and
`MountEnded` remain useful MMS observations, but event timing must not be the
only internal authority mechanism.

Vehicle motion itself stays out of this system.  A vehicle controller consumes
the routed action values and drives its own transform/velocity; a later physics
controller can make the same choice without changing mounting.

## Role vocabulary and attachment direction

Keep `Rider` and `Mountable` as narrow semantic roles for an occupancy mount:

- `Rider` means an occupant that contributes a movement root, a rider-side
  mount point, and a pedestrian movement layer that can yield authority.
- `Mountable` means a destination that can accept that occupant at an authored
  seat/entry/exit relationship and may offer the next movement layer.

They must not become generic names for “a thing which may be parented” and “a
thing which may receive a child.”  A mug held by a hand, a hat placed on a
head, a backpack worn on a torso, and a passenger entering a vehicle all use
an attachment relation, but they do not have the same eligibility, placement,
input, occupancy, or cleanup semantics.

The attachment foundation should remain directionally generic and independent
of humanoids.  Later roles can build on it without pretending they are riders:

| Relation | Initiator/child role | Target/parent role | Movement authority |
| --- | --- | --- | --- |
| Vehicle seat | `Rider` | `Mountable` | transfers to the mounted controller |
| Held prop | future `Holdable`/`Grabbable` attachment | future hand mount point | none by default |
| Clothing/gear | future `Wearable` | future body/bone mount point | none by default |
| Carrier/platform | a rider or other movement participant | movement-capable mount | may transfer |

Humanoids are therefore providers of optional authored mount points (hand,
head, torso, bone, or another named point), not a privileged global model.
The first wearable pass may use rigid transform attachment only; skinning and
deformation binding are separate later concerns.

An authority handoff must be requested by the role pair/capability, not by the
mere fact that an attachment edge exists.  Holding a prop or wearing clothing
must not suppress locomotion or capture vehicle controls.

## Suggested implementation slices

1. Remove `snapshot_input`, `suspend_input`, and `restore_input` from
   `AttachmentSystem`, while preserving current mount/dismount safety tests.
2. Define a durable active-attachment query/notification boundary suitable for
   a sibling system; do not make a vehicle system reach into private attachment
   maps.
3. Add a generic movement-authority stack with exact prior-state restoration.
4. Migrate pedestrian `Input` and `InputXRGamepad.locomotion` to registered
   automatic movement layers.
5. Define the generic attachment-role/capability boundary for later held,
   worn, and mount-point attachments; preserve `Rider`/`Mountable` as the first
   occupancy-specific pair rather than overloading them.
6. Add an authored or engine-owned vehicle-controller layer that becomes active
   only for its matching `Mountable` edge.
7. Migrate `mittens-corp` from manual mounted-state gating to that layer, while
   retaining its existing XR stick/laser bindings as the first controller
   consumer.

## Acceptance criteria

- Mounting and dismounting succeed with no input components in the world.
- Attachment code has no dependency on desktop or XR input component types.
- A held or worn attachment can commit and unwind without receiving a movement
  authority layer, and a non-humanoid can be an attachment endpoint.
- Mounting a vehicle stops only the relevant pedestrian automatic movement;
  tracking and raw device observation remain live.
- A mounted vehicle alone receives its routed movement actions; an unmounted
  vehicle never receives them.
- Nested mounted contexts restore the immediately underlying movement layer,
  including when a vehicle/rider is removed unexpectedly.
- Focus loss, input-device loss, removal, failed commit, and scene teardown
  leave no stuck layer or stale held action.
- Existing `MountStarted` / `MountEnded` MMS behavior remains observable and
  occurs after the corresponding relationship transition.

## Related work

- [Desktop occupancy mount pose and input-authority first slice](desktop-occupancy-mount-pose-and-input-authority-first-slice.md)
- [Attachment valence and Grabbable unification](attachment-valence-and-grabbable-unification.md)
- [Rider + Mountable attachment-system first slice](rider-mountable-attachment-system-first-slice.md)
- [Interaction zones, sockets, and vehicle mounting](release-zones-sockets-and-vehicle-mounting.md)
- [Mittens-corp mounted vehicle controls and laser first slice](mittens-corp-mounted-vehicle-controls-and-laser-first-slice.md)
- [MMS keyboard and regular gamepad events](mms-keyboard-and-gamepad-events.md)
