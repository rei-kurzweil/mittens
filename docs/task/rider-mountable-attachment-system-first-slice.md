# Task: Rider + Mountable attachment-system first slice

## Status and outcome

Implemented with final XR validation pending, 2026-09-09. The first native
mount/dismount path operates in-headset. The car entry/exit fixtures have moved
to the observed semantic front, and attachment alignment now limits the
locomotion root to translation plus yaw so tracked HMD pitch/roll is not baked
into it. Exact anchor placement and repeated-cycle behavior still need testing
in-headset.

Validate the slice in [mittens-corp](../../examples/mittens-corp.mms): while
the Bisket rider is inside the independent mech/car's front entry zone,
pressing the XR grip while pointing at the car mounts Bisket at the authored
cockpit target. Outside that zone, gripping the car does not mount. The mount
transaction suppresses Bisket's automatic pedestrian locomotion without
disabling `InputXR` tracking or raw XR controller events.

For the first usable loop, a later, distinct left XR grip press belonging to
the mounted rider dismounts it even if the pointer hits nothing. Right grip is
reserved for mounted actions such as the car's firing chord. This temporary
gesture will be replaced by an authored eject-button interaction in a later
phase; it is not the eventual general dismount policy.

This task is the narrow bridge between the implemented
[zone query foundation](interaction-zone-collision-query-foundation.md) and the
broader [zones, sockets, and vehicle mounting design](release-zones-sockets-and-vehicle-mounting.md).

## Why three authored concepts remain distinct

The public vocabulary has three useful concepts even though only two enter the
new attachment runtime in this slice:

- `Rider` describes the participant that can be mounted: its movement root,
  alignment anchor, and automatic movement mapping.
- `Mountable` describes a destination: its entry zone, destination anchor,
  activation policy, occupancy, and later its vehicle movement layer.
- `Grabbable` describes an object that can be held by a pointer.

Do not replace or reinterpret `Grabbable` while proving the first mount path.
The existing grab system already owns hand-relative placement, clearance,
release, and parent restoration. A held object is conceptually another
attachment relationship, but migrating it immediately would combine the new
mount transaction with a rewrite of established grabbing behavior.

The first implementation therefore has one interaction arbiter feeding two
consumers:

```text
grip activation + pointed hit
             |
             v
      interaction arbiter
        /             \
eligible Mountable   eligible Grabbable
        |                  |
        v                  v
 AttachmentSystem    GrabbableSystem
```

Exactly one consumer wins a grip activation. The arbiter must not emit two
independent commands and rely on system tick order to resolve the conflict.

## Proposed MMS contract

The exact builder spellings can change during registration, but the first
contract should stay close to:

```mms
// Authored within the pointer-associated Bisket/player tree.
Rider
    .anchor("[name='bisket_rider_cxr_anchor']")
    .movement_root("[name='bisket_locomotion_root']")
    .input("[name='bisket_pedestrian_locomotion']") {}

// Authored on the independent mech/car owner.
Mountable
    .entry_zone("[name='car_entry_zone']")
    .mount_anchor("[name='left_display_car_cxr_mount']")
    .dismount_anchor("[name='car_dismount']")
    .on_grip() {}
```

Every component-reference field accepts either a live MMS component object or
a durable string query, following the existing `ComponentRef` contract. A live
object becomes a GUID reference. Vehicle-side unprefixed queries resolve in the
`Mountable` owner's containing scope. Rider-side queries resolve within the
pointer-associated rider tree; they must not accidentally select another
avatar from a world-global search.

`Rider` is explicit rather than inferred from `AvatarControl`, `CXR`, humanoid
bones, or a particular input topology. This permits camera-only riders,
non-humanoid occupants, nested vehicles, and test fixtures without embedding
Bisket-specific conventions in the attachment system.

For the first slice, author `Rider` as an ancestor scope around the input/XR
subtree that contains its pointers. Pointer association then walks upward and
selects the nearest enabled `Rider`; it does not search unrelated roots. The
configured movement root may be a transform above that `Rider`, referenced
explicitly. Later nested-vehicle work can replace this nearest-scope rule with
the active movement-authority stack where necessary.

An object may carry both roles:

```mms
T {
    Rider { /* this mech can itself enter an outer carrier */ }
    Mountable { /* an inner rider can enter this mech */ }
}
```

The broom can independently carry both `Grabbable` and `Mountable`; its
release-to-mount activation policy is a follow-up after grip-to-enter is proven.

## Grip activation and eligibility

The XR grip and desktop grab gesture represent an interaction attempt, not a
precommitted grab. For the car path, mounting wins only when all of these are
true at activation time:

1. the pointed raycast hit resolves to an enabled, unoccupied `Mountable` owner;
2. the initiating pointer resolves to exactly one enabled `Rider` association;
3. the rider's configured probe is inside or on the boundary of the referenced
   entry zone;
4. the rider movement root, rider anchor, zone, and destination anchor are live;
5. the proposed relationship introduces no structural or effective-transform
   cycle;
6. the required transform bases are finite and non-singular.

For the first car slice, the rider anchor's current world origin is also the
entry probe. This corresponds to the tracked HMD/CXR position and prevents a
long controller ray from mounting the player from outside the vehicle's entry
area. Add a separate `probe(...)` field later only when a rider needs an entry
point different from its alignment anchor.

If the eligible mount check fails and the same pointed owner is enabled
`Grabbable`, dispatch the ordinary grab path. If it is only `Mountable`, do
nothing. Merely entering the zone never mounts, and merely pointing at or
gripping the car from outside the zone never mounts.

Like `Grabbable`, registering an enabled `Mountable` must ensure that its owner
has a raycast interaction marker when no authored one exists. The generated
marker is runtime-only. A ray hit on descendant car geometry resolves upward to
that mountable owner; the entry zone itself remains non-raycastable and
invisible.

Eligibility is a synchronous current-transform query. Cached zone enter/exit
state may eventually drive preview feedback but cannot authorize the commit.

Before selecting a ray hit, grip arbitration must first ask whether the
initiating pointer belongs to a rider with an active mount edge. If it does,
the press becomes a dismount request and is consumed. This lookup is based on
pointer/rider association, not on what the pointer currently intersects. Thus
the temporary escape gesture works while looking away from the car and cannot
accidentally start a grab in the same press.

Only a new press edge can dismount. The grip press that commits a mount cannot
also observe the newly created edge and immediately dismount it, and holding
the grip across the mount transition does not count as another activation.

## XR validation findings and orientation correction, 2026-09-09

Mounting and grip-anywhere dismount both committed in the initial XR test, but
the original full-matrix alignment treated live CXR/head orientation as an
authored attachment orientation. Looking above or below the horizon during a
transition therefore baked inverse pitch into the rider movement root.

The rider's tracked head pose has two different meanings that must not be
collapsed:

- its world position is the current entry probe and the point around which the
  rig should be relocated;
- its live pitch and roll are device pose and must remain relative tracking,
  not become locomotion-root orientation.

For this vehicle slice, mounting and dismounting may change the movement root's
world translation and yaw, but must never introduce pitch or roll into that
root. The vehicle mount anchor supplies the mounted layer's yaw. Dismount resets
movement-root pitch and roll to zero while preserving its current world yaw;
the dismount anchor supplies position, not a surprise heading change. The live
HMD orientation remains untouched, so a user looking up, down, or sideways
continues looking that way across the transition without permanently tilting
the world.

The implemented horizontal attachment helper avoids multiplying the mount
anchor by the inverse of the rider anchor's unrestricted live matrix. It:

1. preserves the current tracked-head world position as the relocation pivot;
2. projects the attachment heading onto the world-up plane;
3. applies only the destination yaw to the movement root;
4. leaves headset pitch/roll in the `InputXR`/CXR tracking path;
5. handles a near-vertical head-forward vector without unstable yaw extraction.

The focused unit test covers nonzero pitch and roll and asserts that the rider
anchor reaches the destination position without adding pitch/roll to the
movement root. Repeated-cycle and subjective comfort validation remain XR work.

Zone visualization also confirmed that the original fixture at local
`z = 3.5` was on the car's back. It now uses `z = -3.5`; the provisional
dismount anchor moved from `z = 4.6` to `z = -4.6`. Verify both distances and
the car model's actual forward basis in XR.

## AttachmentSystem runtime relationship

Authored components describe capabilities and rules. Mutable mounted state
belongs to `AttachmentSystem`, not to `ZoneComponent` and not to example-local
MMS handlers. Each active mount edge records at least:

```rust,ignore
struct ActiveMount {
    rider: ComponentId,
    mountable: ComponentId,
    rider_root: ComponentId,
    rider_anchor: ComponentId,
    mount_anchor: ComponentId,
    dismount_anchor: ComponentId,
    suspended_input: Option<ComponentId>,
    suspended_input_previous_state: Option<bool>,
    original_parent: Option<ComponentId>,
    dismount_world_pose: TransformTrs,
}
```

Store resolved identities at commit time. Do not repeat a world-global query
each frame and silently switch riders, anchors, or input mappings after topology
changes. Validate cached IDs before use and unwind the edge if a required
participant disappears.

The relationship must be inspectable by rider and mountable, enforce one rider
per single-seat mount, and support deterministic cleanup. Runtime occupancy is
derived from active edges; avoid a second independently mutable `occupied`
boolean on `MountableComponent`.

## Atomic mount transaction

Commit mounting as one coordinated operation:

1. Revalidate eligibility from current world transforms.
2. Snapshot the rider root's parent/world pose and the movement input's prior
   automatic-mapping state.
3. Reject transform cycles before changing either tree.
4. Attach the rider movement root under the vehicle mount basis.
5. Align the rider anchor's position and vehicle yaw with the mount anchor
   without copying tracked head pitch/roll into the movement root.
6. Disable only the rider's automatic locomotion mapping.
7. Publish the active edge and a `MountStarted` observation.

If any step fails, roll back parentage, world pose, input state, and occupancy.
The failure result must be observationally equivalent to no mount having
occurred, after which arbitration may use the permitted grab fallback.

Do not disable `InputXR`: tracked head translation and rotation must continue
while mounted. `InputXRGamepad.disable()` relinquishes its built-in pedestrian
locomotion while preserving canonical axis/button events for the vehicle
controller. Desktop `Input.disable()` gates its built-in transform mapping.

## Dismount and nested authority

Provide a repeatable first-slice dismount action. Until the eject control is
authored, a distinct left-grip press from the rider-associated pointer pops
that rider's outermost mount edge. Right grip remains available as a mounted
vehicle action modifier. No ray hit is required. Dismount must:

1. resolve and validate the authored dismount anchor before mutating state;
2. detach the rider root and align the rider anchor with that dismount anchor;
3. restore the exact previous automatic-input state;
4. clear occupancy and publish `MountEnded`;
5. leave both rider and mountable eligible for another complete cycle.

The car fixture authors `car_dismount` just outside the front
entry zone. This is preferable to a magic engine offset: it makes safe exit
placement visible in the scene, handles vehicle rotation naturally, and can be
reused when the eject button arrives. If the anchor disappears, cleanup still
must recover the rider by preserving its current world pose and restoring
input; an ordinary user-requested dismount should fail before mutation rather
than strand the rider in a partial state.

Represent nesting as a stack/chain of active edges rather than a global
"mounted" flag:

```text
Bisket -> mech -> carrier -> construction vehicle -> station
```

Each new outer edge suspends only the immediately inner movement authority.
Removing an edge restores only the state captured by that edge. Never enable
all descendant inputs during dismount. Full nested mounting is not required for
the first car test, but the stored relationship must not make it impossible.

## Engine integration boundary

Add the native behavior under `engine/ecs`:

- `RiderComponent` and `MountableComponent` own authored configuration and MMS
  serialization;
- an interaction-arbitration path consumes pointer activation plus ordered ray
  hits and chooses one action;
- `AttachmentSystem` owns eligibility, mount/dismount transactions, occupancy,
  active relationships, and lifecycle cleanup;
- `ZoneComponent` remains a detection-only geometry primitive;
- `GrabbableSystem` continues to own existing grabs in this slice.

Prefer a neutral interaction-attempt command/event from gesture recognition
over calling `AttachmentSystem` directly from `GestureSystem`. Native mount
outcome events can later be exposed to MMS without requiring scripts to perform
the transaction themselves.

For this slice, that attempt must exist even when there is no ray hit. It
contains the pointer and press-edge identity plus an optional ordered hit. The
arbiter applies this order:

1. active mount for the pointer's rider -> consume and request dismount;
2. eligible pointed `Mountable` -> consume and request mount;
3. eligible pointed `Grabbable` -> dispatch the existing grab path;
4. otherwise -> no action.

`AttachmentSystem`, rather than `ZoneComponent`, ticks because mounted edges
have lifecycle: it drains mount/dismount commands, commits relationship
changes, validates active participants, and unwinds removed or invalid edges.
It does not poll every zone every frame. Zone classification occurs only while
evaluating a mount request (and may later be used separately for previews).

Do not run mounting through the collision-response worker. Entry eligibility
uses the synchronous zone query against current authoritative transforms.

## First implementation sequence

1. [x] Add and round-trip `RiderComponent` with anchor, movement-root, input, and
   enabled configuration.
2. [x] Add and round-trip `MountableComponent` with entry-zone, mount-anchor,
   dismount-anchor, activation policy, and enabled configuration.
3. [x] Add pointer-to-rider resolution bounded to the initiating pointer's player
   tree; cover missing and ambiguous associations.
4. [x] Add pure mount eligibility and cycle-check helpers with focused tests.
5. [x] Introduce grip arbitration that chooses eligible mount before grab fallback.
6. [x] Implement atomic mount, pointer-associated grip-anywhere dismount, input
   restoration, and removal cleanup in `AttachmentSystem`.
7. [x] Author `Rider` and `Mountable` in `mittens-corp`, keeping the existing car
   zone and rider/car anchors and adding a front dismount anchor.
8. [x] Confirm the mount and grip-anywhere dismount transactions operate in XR.
9. [x] Replace unrestricted live-anchor orientation alignment with horizontal
   translation-plus-yaw alignment and cover tracked pitch/roll projection.
   Both transitions still need in-headset validation.
10. [x] Move the entry zone and provisional dismount anchor from the car's back
    to its observed semantic-front side; exact XR tuning remains.
11. [ ] Validate repeated XR mount/dismount cycles and tune the three car-local
    zone/anchor transforms. Focused attachment, serialization, example, and
    existing gesture/grab tests provide the automated baseline.

## Acceptance criteria

- Walking Bisket into the car's front zone and gripping while pointing at the
  car mounts exactly once.
- The same grip from outside the entry zone does not mount.
- Entering the zone without grip does not mount.
- A long pointer ray cannot mount from outside merely because it hits the car.
- The rider anchor aligns to the cockpit mount in position and orientation.
- `InputXR` tracking and raw XR gamepad events continue while pedestrian
  locomotion is suppressed.
- The mounted layer receives canonical control events without the inner rider
  also moving independently.
- Dismount preserves a valid world pose, exits the zone, restores the captured
  input state, and permits remounting.
- While mounted, a new left-grip press from the rider-associated XR pointer
  dismounts even with no ray hit; that press cannot also grab another object.
- Right grip remains available for mounted-layer action chords.
- Mount failure or removal of a required component leaves no partial parent,
  occupancy, or disabled-input state.
- An ineligible `Mountable + Grabbable` target follows the declared grab
  fallback; an eligible one cannot both mount and grab from one activation.
- Existing grabbable examples and tests behave unchanged.
- No mounting code depends on `CollisionResponseComponent` or asynchronous
  collision-worker timing.

## Deferred work

- migrating `Grabbable` onto the shared attachment relationship backend;
- broom release-to-mount and held-object handoff;
- multiple seats, multiple riders, reservation, and mount priority;
- continuous zone enter/exit authoring and scripted eligibility callbacks;
- vehicle velocity/force integration and physical collision ownership;
- polished entry/exit animation and pose transitions;
- the guarded red eject control (box, cylinder/button, label, translucent
  rotatable lid) and replacement of the temporary grip-anywhere gesture;
- full nested-vehicle UI, diagnostics, and recovery policies.

## Related work

- [Editor Settings generic-zone visualization first slice](editor-settings-generic-zone-visualization-first-slice.md)
- [`mittens-corp` mounted vehicle controls and laser first slice](mittens-corp-mounted-vehicle-controls-and-laser-first-slice.md)
- [Interaction zones on the collision-query foundation](interaction-zone-collision-query-foundation.md)
- [Interaction zones, sockets, and vehicle mounting](release-zones-sockets-and-vehicle-mounting.md)
- [E2 broom attachment first slice](e2-broom-mounting-first-slice.md)
- [Scriptable Velocity pose driver](scriptable-velocity-pose-driver.md)
- [Velocity, forces, and pluggable physics](velocity-forces-and-pluggable-physics.md)
