# Task: AttachmentSystem cleanup over World-owned attachments

Date: 2026-10-10
Status: core cleanup implemented and automated checks passed, 2026-10-10;
in-headset vehicle validation pending. Multiple parents are not implemented.
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

## Implementation before cleanup

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
Rider selects `mitten_locomotion_root`, `mitten_rider_cxr_anchor`, and
`mitten_pedestrian_locomotion`; `left_display_car` supplies a Mountable with
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

1. [x] Specify attachment identity, mutation notifications, transaction ownership,
   end reasons, and final topology rules; inventory direct World mutation callers.
2. [x] Establish the World mutation/reference boundary with single-parent storage
   and focused edge replacement/removal tests.
3. [x] Migrate AttachmentSystem commits, validation, unwind, and sibling queries;
   preserve role/slot resolution and occupancy rules.
4. [x] Verify external mutations, removal, rollback, event counts, and resource
   cleanup against World records; check ordinary grabs and tree behavior.
5. [ ] Validate the mittens-corp car and record performance/lifecycle evidence.
   Headless authored XR-scene and release benchmark checks passed; manual XR
   mount/drive/stop/fire/dismount/remount validation remains pending.
6. [ ] After this gate, proceed to multi-parent components: add detach-one and
   detach-all APIs and shared material declarations using this lifecycle boundary.

Exit: managed car attachments and World topology agree after every tested
transition, explicit detach/reparent requests survive cleanup, and existing
vehicle behavior works without stale state or measured regression.

## Implemented contract and evidence

World now supplies `AttachmentRef` and `attachment_of`/`attachment_is_live`.
A reference contains parent/child identities and an incoming-attachment revision.
Every successful add/replace or detach updates that revision at the storage
boundary; deletion invalidates the component identity. Reattachment between the
same endpoints does not revive an old reference. Rejected mutations preserve
the existing reference. Structural fields are ECS-internal; scripting/save paths
read children through World. References are runtime-only, not serialized identities.

There is no new World notification queue or per-mutation scan. Invalidation is
immediate when a consumer queries the reference. Managed cleanup remains at
AttachmentSystem tick, and targeted reconciliation also runs before interaction
attempts for the affected rider/slot/root. MountStarted/MountEnded still describe
managed relationship transitions; ParentChanged behavior of existing intent and
direct mutation paths is otherwise unchanged.

ActiveMount uses the World reference instead of separately retaining structural
root/parent endpoints. It retains semantic endpoints, original-parent restoration,
and resource/input/velocity state. `mounted_relationship(world, rider)` returns a
validated snapshot; movement-root queries use a reverse index and validate World
state, so stale records never authorize mounted behavior. One movement root
cannot simultaneously belong to two managed occupancy mounts.

Normal dismount restores original parentage. Endpoint loss restores a surviving
root to its surviving original parent (or a root if that parent disappeared),
using the dismount pose when resolvable and otherwise the current world pose.
External detach/reparent/replacement relinquishes ownership: cleanup restores
captured input/velocity and ends occupancy without changing the caller's topology.
If an external caller attaches a surviving subtree beneath a generated helper,
cleanup preserves that helper and subtree rather than deleting caller-owned
attachments. This deliberate ownership transfer is covered by the same-endpoints
replacement test. Invalid dismount/input endpoints also trigger forced cleanup.

Automated evidence:

- Attachment tests cover external detach/reparent, same-endpoint replacement,
  endpoint/helper/root deletion, failed-commit rollback without edge replacement,
  input/velocity restoration, and exactly one end event per committed mount.
- 100 mount/dismount cycles keep component count at baseline plus two helpers
  while mounted and exactly baseline after dismount; occupancy indexes are cleared.
- `mittens_corp_managed_car_attachments_survive_topology_cleanup` loads the authored
  `examples/mittens-corp.mms`, processes real initialization/transform intents,
  resolves its Rider and car mount frames, and performs actual mount transactions.
  It verifies car movement carries the root, normal dismount/remount, external
  detach, input restoration, and helper counts. It injects a pointer beneath the
  authored rider frame and positions that frame in the entry zone; it does not
  simulate hardware grip/raycast acquisition or fabricate mount events.
- Graph, grab, gesture, transform, serialization/round-trip, static-contact,
  zone-observation, and mittens-corp test groups pass. Desktop input suspension
  and example control routing remain covered by existing tests. The desktop
  scene's GLTF-dependent camera-slot import is not a full live desktop mount proof.

Release comparison against pre-change commit `4796e0e9`, using the same ignored
`attachment_cleanup_benchmark` harness in a temporary baseline checkout:

| Workload / representation | Before | After |
| --- | --- | --- |
| 200,000 tree detach/attach pairs, 10,000 children in 100 groups | 24.670 ms | 25.585 ms |
| 200,000 ticks with one valid active mount | 3.631 ms | 4.414 ms |
| ComponentNode size on this target | 144 bytes | 152 bytes |
| ActiveMount size on this target | 88 bytes | 104 bytes |

Times are medians of seven local release runs, not portable timing guarantees.
Tree mutation increased about 3.7%; active validation added about 3.9 ns per tick
(about 22% relative to this very small baseline). The root reverse index adds
one entry per managed mount; it allocates on registration, not unchanged frames.
Reference reads and validation allocate nothing; existing invalid-edge tick
collection remains. No allocation counter or full-scene frame benchmark was run.
For this fixture, use a 10% tree-mutation budget and a 0.1 microsecond additional
cost per active-mount tick as review gates; both local comparisons are within
those budgets. Recheck full-scene behavior during hardware validation.

Hardware investigation update (2026-10-11): the reported pedestrian HMD Y
constraint also occurs before mounting on pre-cleanup main `4796e0e9`. Mounting
frees physical head height; dismounting restores the constraint. Track it
separately in [HMD floor-contact feedback](../bugs/mittens-corp-hmd-height-floor-contact-feedback.md).
This rules out the cleanup as its origin; the complete hardware validation gate
remains pending.

Reproduce the current benchmark with:

```sh
cargo test --release --lib attachment_cleanup_benchmark -- --ignored --nocapture --test-threads=1
```
