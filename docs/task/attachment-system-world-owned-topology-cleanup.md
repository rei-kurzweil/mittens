# Task: AttachmentSystem cleanup over World-owned attachments

Date: 2026-10-10
Status: scoped refactor plan; documentation only.
Release: [0.10.0](epic/0.10.0/README.md).
Next: [multiple parents for shared components](multi-parent-components-and-material-sharing.md).

## Outcome and sequencing

Make managed attachments use authoritative World topology through one mutation
and lifecycle boundary. Preserve the existing Rider/Mountable vehicle behavior
before adding multiple parents. Finish and verify this cleanup first; then
implement the multi-parent storage/API and material-sharing task.

World owns structural attachment existence and endpoints. AttachmentSystem owns
managed relationship policy and transaction state, and refers to the structural
attachments it creates. Do not give it a second independently mutable topology.
The current system does not contain a second adjacency list: the cleanup fixes
the boundary between its cached mount records and World mutations.

## Current implementation

[AttachmentSystem](../../src/engine/ecs/system/attachment_system.rs) retains
`ActiveMount` records by Rider, plus a reverse Mountable occupancy index.
Records include semantic endpoints, alignment frames, generated TransformParent,
original parent, and suspended input/velocity state. Mounting creates the helper,
reparents the movement root, aligns its pose, and emits MountStarted. Dismount
restores parentage and captured settings, removes the helper, and emits MountEnded.
Tick checks endpoint validity and the rider root's singular parent, then unwinds
invalid mounts. That unwind can restore parentage after an external detach.

[World](../../src/engine/ecs/mod.rs) still stores one parent per component.
Low-level topology mutations and intent-driven notifications have different
side effects. [GrabbableSystem](../../src/engine/ecs/system/grabbable_system.rs)
has separate active-grab/restoration bookkeeping. Generic retained relationships,
endpoint valence, and movement-authority extraction are still planned.

## Scope

- Define a World-owned attachment reference using the current parent/child pair
  and validated component identities, or a stable attachment handle if justified.
  Validate replacement as well as deletion: an old managed relationship must not
  claim a newly created attachment between the same endpoints. Do not require
  multi-parent storage to establish this boundary.
- Separate structural attachment references from managed relationship identity.
  Rider, Mountable, and selected mount points/slots describe the semantic role;
  the actual structural edge currently connects the movement root to a generated
  TransformParent. The mount target is followed through a reference.
- Route managed commits and external topology changes through a documented
  mutation/notification boundary. Cover public World APIs and runtime intents;
  low-level storage helpers must not silently bypass relationship invalidation.
- Retain occupancy, alignment/follow policy, original-parent restoration, and
  temporary input/velocity snapshots in the managed layer. Keep existing authored
  Rider/Mountable components and mount points/slots. No new generic slot or valence
  authoring API is needed for this cleanup.
- Define transaction ownership and end reasons: normal dismount, explicit
  external detach, external reparent, endpoint destruction, and failed commit.
  Normal dismount restores the surviving original parent. Explicit detach leaves
  the root detached; explicit reparent preserves the caller's chosen parent.
  Cleanup releases semantic state/resources without undoing either request.
  Endpoint-loss handling must define a surviving-root fallback separately.
- End once: remove occupancy/reverse indexes, retire generated helpers, restore
  captured input/velocity settings, and publish one coherent lifecycle result.
  Failed commits must leave no active relationship or partially changed topology.
- Provide a managed-relationship query/invalidation boundary for sibling systems;
  do not expose private maps or ask consumers to infer mounting from tree shape.
  Target affected relationships rather than scanning every component per mutation.

Keep mount policy separate from generic edge storage: an ordinary parent/child
attachment must not imply a seat, valence, input suspension, or occupancy.

## Deliberate boundaries

This task preserves single-parent spatial behavior. It does not implement
multiple parents, shared geometry transforms, the complete valence model,
Holdable/Wearable roles, a control-layer stack, or new car controls. Preserve
current input/velocity behavior behind the improved lifecycle boundary; extraction
to movement authority remains [a separate task](attachment-movement-authority-and-input-routing.md).

Inventory GrabbableSystem against the same mutation contract and verify grabbing
has not regressed. Full consolidation of grab/hold and mount relationship records
remains [the valence/unification task](attachment-valence-and-grabbable-unification.md).

## Verification fixture: mittens-corp vehicle

Use [examples/mittens-corp.mms](../../examples/mittens-corp.mms) and
[display_car.mms](../../assets/components/vehicles/display_car.mms). The player
Rider selects `rei_mu_locomotion_root`, `rei_mu_rider_cxr_anchor`, and
`rei_mu_pedestrian_locomotion`; `left_display_car` supplies a Mountable with
entry, mount, and dismount frames. Preserve those managed role/slot selections.
The existing example is the end-to-end fixture, not proof obtained solely from
private AttachmentSystem maps.

Automated checks inspect World topology and managed records together:

| Action | Required evidence |
| --- | --- |
| Mount in the eligible entry zone | One committed relationship/occupied slot; selected endpoints match; root attached to the generated helper; one MountStarted. |
| Attempt a second occupant or invalid mount | Rejected without changing topology, occupancy, or input state. |
| Move/rotate the car | Rider follows with existing alignment behavior; independent local look still works. |
| Normal dismount and remount | Original parent restored, helper removed, exact captured settings restored; slot reusable; one MountEnded per committed mount. |
| Detach the root externally | Root remains detached; managed state/helper cleared; no restoration that reverses detach. |
| Reparent the root externally | Requested parent survives cleanup; old mount no longer owns the root. |
| Remove Rider, Mountable, root, target, or helper | Defined safe unwind/fallback, no stale occupancy or double cleanup, surviving components remain valid. |
| Failed commit / scene teardown / repeated cycles | No orphan helpers, leaked relationships, or stuck input/velocity settings. |

Run focused existing attachment, gesture/grab, serialization, and transform
checks plus meaningful tests for external mutation and cleanup. In the example,
manually verify mount, drive, stop, fire, dismount, and remount in XR. Preserve
mounted-only car/laser controls and pedestrian suppression/restoration; document
hardware validation still pending rather than treating headless tests as XR proof.

Record a tree-only baseline and repeated mount/dismount resource counts. No
whole-world scan or new steady-frame allocation should be introduced by the
notification/reference boundary. Existing tick validation can remain during
migration; do not broaden it to all component edges every frame.

## Implementation slices

1. [ ] Specify attachment identity, mutation notifications, transaction ownership,
   end reasons, and final topology rules; inventory direct World mutation callers.
2. [ ] Establish the World mutation/reference boundary with single-parent storage
   and focused edge replacement/removal tests.
3. [ ] Migrate AttachmentSystem commits, validation, unwind, and sibling queries;
   preserve role/slot resolution and occupancy rules.
4. [ ] Verify external mutations, removal, rollback, event counts, and resource
   cleanup against World records; check ordinary grabs and tree behavior.
5. [ ] Validate the mittens-corp car and record performance/lifecycle evidence.
6. [ ] After this gate, proceed to multi-parent components: add detach-one and
   detach-all APIs and shared material declarations using this lifecycle boundary.

Exit: managed car attachments and World topology agree after every tested
transition, explicit detach/reparent requests survive cleanup, and existing
vehicle behavior works without stale state or measured regression.
