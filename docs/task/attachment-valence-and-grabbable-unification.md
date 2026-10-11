# Task: attachment valence and Grabbable unification

Status: planned, 2026-09-12. Follows the `Rider` + `Mountable` first slice.

Focused prerequisite: [AttachmentSystem topology/lifecycle cleanup](attachment-system-world-owned-topology-cleanup.md).
Verify existing managed car attachments before multi-parent support; full generic
role/valence and grab consolidation remain in this task.

## Attachment valence model

Use signed attachment valence relative to a user-controlled frame, rather than
making humanoids the root concept:

| Valence | Role | Meaning | Default movement authority |
| --- | --- | --- | --- |
| -1 | carried/equipped item | a held prop, wearable, hat, glasses, tool, or gear rigidly following a compatible mount point | none |
| 0 | user-controlled frame | an avatar body, creature rig, camera rig, or another transform basis a user directly controls | owns pedestrian control |
| +1 | mount | a vehicle, broom, mech, platform, or carrier that the controlled frame can enter/follow | may offer the next control layer |

Attachment valence describes an individual edge, not an object's fixed global
type, scene depth, or number of sockets.  A vehicle at `+1` can itself become
the controlled participant for a carrier at `+2`; an equipped item can have
its own rigid children.  A level-`-1` holdable or wearable may attach to any
compatible mount point, including one owned by a level-0 controlled frame, a
level-`+1` mount, or another item when that policy permits it.  A humanoid is
one possible level-0 frame, not a requirement: any user-controlled transform
tree can expose named or authored mount points.

Valence belongs to the selected role/endpoint for an edge, not to the entire
owner object.  One owner may expose several mount points with different roles
at the same time.  For example, a car can expose a `+1` occupant mount, a neutral
dashboard mount point for equipment, and a rider-side mount point used when
that car enters an outer carrier.  Attachment eligibility must evaluate the
chosen points and their capabilities; it must not assign one permanent integer
to the car.

Use **mount point** for the specific authored attachment endpoint whose
position and orientation participate in alignment.  A mount point is a type of
socket in the broader attachment terminology.  Prefer the more specific term
in this work because it distinguishes the alignment endpoint from its optional
eligibility `Zone`.  Attachment valence belongs to the mount point/endpoint
role; the associated zone describes where an attempted attachment is allowed
and does not independently acquire that valence.

## Geometric alignment and follow policy

Valence is not a geometric transform.  A mount point is an oriented coordinate
frame, and its authored rotation carries the desired facing and angular offset.
If an endpoint must face the other way, author that rotation into its mount
point rather than adding a parallel `Same`/`Opposed` policy that can contradict
the frame.

That edge policy needs to distinguish at least:

```text
initial translation: none | selected axes | full
initial rotation:    none | yaw | full
ongoing translation: none | selected axes | full
ongoing rotation:    none | yaw | full
offset behavior:     snap | preserve
```

The two selected mount points supply coordinate frames.  A higher-level policy
such as occupancy, holding, or wearing selects and validates the geometric
relationship between those frames.  The resolved active attachment edge owns
the selected channels, follow behavior, and any captured offset.  The mount
points themselves own the authored frames.  Neither valence nor the edge adds
an implicit 180-degree facing correction.

For the current desktop car fixture, the destination mount point is oriented
toward the desired seated view, while the Rider-side camera slot contains the
avatar-to-camera basis correction.  Horizontal alignment must solve from both
frames rather than assuming the source frame has identity rotation.  Ongoing
follow policy is a separate choice: an upright seat may follow vehicle
translation plus yaw, while an aircraft or broom may intentionally carry the
Rider through full pitch and roll.  In both cases, local player/head look
remains a descendant pose capability rather than being baked into the
mount-point alignment.

## Existing vocabulary

Keep the present terms narrow:

- `Rider` is the level-0 occupancy participant.  It is an attachment-system
  role and supplies the movement root/rider-side mount point needed by the
  movement-authority handoff.  It does not mean every attachable object.
- `Mountable` is a level-+1 attachment-system destination.  Its general job is
  accepting a rider relationship; a vehicle is only one implementation.  A
  mount need not itself be humanoid-shaped or user-controlled.
- `Grabbable` remains an interaction affordance: it says a pointer can acquire
  an object.  It is not yet the retained attachment relation.

`Wearable` and a future `Holdable` belong at level -1.  They should share a
common attachment-item foundation, but remain separate semantic policies:

- `Holdable` supplies grab pose, release/throw behavior, pointer eligibility,
  and a temporary attachment to a compatible hand, handle, or other mount point.
- `Wearable` supplies equip/unequip policy, target-mount-point eligibility, and a
  persistent rigid attachment.

An item may offer both roles: glasses can be grabbed, then equipped to a head
mount point; a tool can be held, then placed at a belt mount point; a mounted
vehicle can offer compatible mount points for equipment, dashboard items,
handles, or cargo.
Do not use `Rider` for any of those cases, and do not make them transfer
pedestrian movement authority merely because a transform was parented.

## Current implementation and problem

Source review, 2026-10-10: the runtime still has occupancy-specific `ActiveMount`
records and separate `ActiveGrab` bookkeeping. Endpoint valence and the generic
retained-edge slices below are not implemented. AttachmentSystem still suspends
and restores input/velocity; the movement-authority extraction remains planned.
Coordinate topology ownership and external detach/reparent/removal behavior with
[multi-parent components](multi-parent-components-and-material-sharing.md#coordinate-with-the-existing-attachmentsystem-refactor).
World owns actual parent/child adjacency; retained relationship records add
policy and transaction state and must use that authoritative topology rather
than duplicate it. Valence does not apply automatically to every ECS edge.

`GrabbableSystem` already creates a temporary parent relationship while held:
it reparents the grabbed target beneath the pointer's nearest transform,
preserves world pose, smooths the held local destination, then restores the
original parent on release.  Its active-edge bookkeeping and lifecycle are
private to `GrabbableSystem`, while `AttachmentSystem` separately owns mounted
Rider-to-Mountable edges.

That duplicates the core concerns of attachment: identity of child/target,
original parent, authoritative lifetime, removal cleanup, cycle protection,
and transform preservation.  The interaction policies are legitimately
different, but their retained attachment edge should not be implemented twice.

## Goal

Create one low-level, directionally generic attachment-edge foundation used by
grab/hold, equip/wear, and ride/mount.  It must describe parent/child or
follow-mount-point relations without assuming a humanoid, a vehicle, a pointer, or
movement authority.

Targets must be expressed as generic authored mount points.  `Mountable` owners
may expose such points just as a user-controlled frame can; attaching an item
to one does not turn that item into a rider or activate the mount's movement
controls.

Higher-level systems remain responsible for their own policies:

```text
Gesture / interaction system
  chooses a valid Grabbable or Mountable activation
        |
        +--> Holdable / Grabbable policy -> generic attachment edge (level -1)
        +--> Wearable/equip policy       -> generic attachment edge (level -1)
        +--> Rider / Mountable policy    -> generic attachment edge (0 -> +1)
                                                   |
                                                   +--> MovementAuthoritySystem,
                                                        only when that role pair
                                                        requests a handoff
```

The generic edge must not imply that every child is literally ECS-parented.
Rigid parentage is appropriate for the first wearable pass; later constraints,
physics, skin binding, and retargeted bone attachment may use a different
runtime mechanism while preserving the same relationship semantics.

## Implementation slices

### Slice 1: make AttachmentSystem valence-aware without changing behavior

- Define a validated attachment-valence/endpoint representation.  Prefer an
  enum or constrained type over arbitrary scene-authored integers.
- Map the current `Rider` endpoint to neutral/`0` and the current `Mountable`
  occupant endpoint to positive/`+1`.
- Record both selected endpoints, their valences, and the semantic relationship
  kind on every active mount edge.
- Make mount eligibility reject an incompatible direction or role pairing in
  addition to its existing zone, occupancy, reference, transform, and cycle
  checks.
- Preserve the present Rider-to-Mountable result exactly and add focused tests
  proving `0 -> +1` succeeds while a reversed or incompatible pairing fails.

This slice establishes vocabulary and evidence in the existing system.  It
must not yet rename `Rider`/`Mountable`, migrate grabbing, add wearables, or
change input-routing behavior.

### Slice 2: generic retained attachment edges

- Make World the single owner of structural attachments. Retained mount/grab
  relationships refer to the exact World-owned attachment(s), using validated
  IDs or parent/child pairs under the chosen topology contract. Keep semantic
  endpoints, alignment, occupancy, and restoration state in the relationship
  layer; do not duplicate mutable edge existence there. Coordinate external
  detach/reparent/removal with cleanup so explicit detachment is not reversed
  by original-parent restoration. This boundary is an early dependency of the
  multi-parent task and can be implemented before the complete valence model.
- Inventory the exact invariants shared by `GrabbableSystem` and
  `AttachmentSystem`: atomicity, cycle checks, original-parent restoration,
  world-pose preservation, single-owner policy, removal cleanup, and lifecycle
  events.
- Introduce a generic active attachment-edge representation and public
  query/cleanup boundary.  Do not expose one system's private maps to another.
- Rebuild Rider-to-Mountable commits on that edge while preserving its current
  activation and alignment behavior.

### Slice 3: separate movement authority

- Move input suspension/restoration out of attachment as specified by
  [the movement-authority task](attachment-movement-authority-and-input-routing.md).
- Let only an occupancy edge with the appropriate capability request a
  movement-authority layer.  Numeric valence alone must never cause an input
  handoff.

### Slice 4: migrate held grabs

- Rebuild current temporary grabs on the generic edge while retaining their
  existing ray clearance, smoothing, release, and interaction-priority
  semantics.
- Define `Holdable` as the policy above the retained grab edge.  Decide whether
  `Grabbable` becomes that role, remains the activation marker, or composes
  with it; preserve existing MMS scenes during migration.

### Slice 5: rigid wearables and general mount points

- Add rigid `Wearable`/equip-to-mount-point behavior with arbitrary authored
  mount points first; defer skinning and deformation binding.
- Prove a held/worn item can target a mount point owned by either a controlled
  frame or a `Mountable` without activating movement authority.

### Slice 6: composition and nesting

- Add nesting, ownership, save/load, removal, and conflict rules across held,
  worn, and mounted relationships.
- Prove that one object may expose multiple endpoint roles and participate in
  different-valence edges without acquiring one fixed global valence.

## Acceptance criteria

- A non-humanoid user-controlled frame or a mount can hold or wear an item
  through an authored compatible mount point without special-case humanoid code.
- A prop can be grabbed, released, equipped, unequipped, and grabbed again
  without orphaned parentage, transform jumps, or duplicate ownership.
- A `Grabbable` item and a `Mountable` vehicle still compete deterministically
  for a single interaction activation.
- Held/worn edges do not alter pedestrian movement authority.
- Rider-to-Mountable edges alone can request a movement-authority layer.
- Removal of either endpoint unwinds the edge safely and emits a coherent
  lifecycle result.
- Existing grab clearance, placement smoothing, and current mount alignment
  retain their behavior after migration.

## Related work

- [Separate mounting from movement authority and input routing](attachment-movement-authority-and-input-routing.md)
- [Rider + Mountable attachment-system first slice](rider-mountable-attachment-system-first-slice.md)
- [Interaction zones, sockets, and vehicle mounting](release-zones-sockets-and-vehicle-mounting.md)
- [Grab hand-relative bounds placement](grab-hand-relative-bounds-placement.md)
- [Desktop mounted facing is reversed](../bugs/mittens-corp-desktop-mounted-facing-is-reversed.md)
