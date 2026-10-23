# Task: multi-parent components, starting with shared material declarations

Date: 2026-10-10
Status: first leaf-Shading sharing slice implemented and user-confirmed in VR, 2026-10-22; broader graph migration pending.
Parent use case: [custom materials](epic/custom-materials.md).
Related foundation: [material instances and families](material-instances-and-family-contracts.md).
Prerequisite: [AttachmentSystem topology/lifecycle cleanup](attachment-system-world-owned-topology-cleanup.md),
with existing automated single-parent checks accepted as the baseline. Manual
mittens-corp car validation is tracked separately and does not block this slice.
Release tracking: [0.10.0](epic/0.10.0/README.md), with material-sharing slice first.

2026-10-22: the existing single-parent automated checks are accepted as the
baseline; manual car validation no longer blocks material work. The first
[VR checkpoint](shared-materials-xr-checkpoint.md) implements shared leaf Anime/
Toon declarations, detach-one/all, safe shared deletion and a controller-driven
scene. The user confirmed independent rightmost-sphere shading, detaching the
shared source from the other objects and reattaching both. Keep implementation
on hold at the user's request; this follow-up updates documentation only.
The broader contracts below remain the target; generic material instances and
graph-preserving save/clone are not implemented by that checkpoint.

## Goal

Allow one instantiated component to remain attached beneath multiple parents,
with one identity and mutable state. First prove a shared shading/material
declaration on separate renderables. Repeating the same live MMS handle should
share that component, not silently reparent it, clone it, or create another
renderer material instance.

This is a component-graph change, not merely a new material reference setter.
Define the semantics and migrate affected assumptions carefully before claiming
arbitrary spatial subtrees can be shared. The initial material use case needs
shared declarations, not multiple copies of geometry or a new transform model.

## Original single-parent audit (2026-10-10)

The table records the starting point before the leaf-Shading slice. Current
implemented behavior is described in the [accepted VR checkpoint](shared-materials-xr-checkpoint.md):
additional incoming edges for leaf Shading, exact edge references, detach-one/
all, shared-aware deletion, consumer refresh and identity-deduplicated queries.
Structural components retain their original single-parent behavior.

| Boundary | Current source/behavior |
| --- | --- |
| Record | [ComponentNode](../../src/engine/ecs/component/mod.rs) stores one `parent: Option<ComponentId>` and a children Vec. |
| Mutation | [World](../../src/engine/ecs/mod.rs) `add_child` detaches the existing parent before attachment; `set_parent`, `detach_from_parent`, root/ancestor helpers are singular. |
| Runtime wiring | [IntentExecutor](../../src/engine/ecs/signals/intent_executor.rs) emits `ParentChanged { old_parent, new_parent }`, initializes attached trees, and routes topology-dependent work. Low-level World mutations do not perform those side effects. |
| MMS | [configured registry](../../src/scripting/configured_registry.rs) attaches an existing `CeChild::Attach` handle through `World::add_child`. Current repeated attachment is reparenting. |
| Deletion | `remove_component_subtree` visits all descendants and removes them regardless of outside consumers; that is unsafe for a shared descendant. |
| Transforms | [TransformSystem](../../src/engine/ecs/system/transform_system.rs) uses ancestor walks and cached world matrices. A shared transform under different parents has no unique world matrix without further semantics. |
| Shading | [RenderableSystem](../../src/engine/ecs/system/renderable_system.rs) resolves wrappers/direct children but registration uses singular owner ancestry. Sharing needs consumer fan-out and invalidation. |
| Queries/references | [mittens-query adapter](../../crates/mittens-query/src/evaluator.rs) is tree-oriented; [ComponentRef](../../src/engine/ecs/component/component_ref.rs) has singular upward traversal. |
| Signals | [RxWorld](../../src/engine/ecs/signals/rx_world.rs) builds one ancestor scope chain. Multi-path routing/delivery needs an explicit policy. |

At this audit, 88 source files contain `parent_of(`. That is a migration inventory,
not evidence that every caller should enumerate all paths. Classify each caller
by ownership, structural ancestry, shared consumers, or contextual path needs.
The older [topology/style helper proposal](refactor/topology-queries-and-style-inheritance.md)
is useful motivation; its single-ancestor helpers are not a multi-parent spec.

## First observable MMS examples

Working first-slice syntax, now confirmed by the user in the VR fixture:

```mms
let shared = Shading.anime().shade_strength(0.35) {}
T.position(-1.0, 0.0, 0.0) { R.cube() { shared } }
T.position( 1.0, 0.0, 0.0) { R.sphere() { shared } }
```

This uses the existing Anime program to test graph sharing before custom
shader loading exists. Both renderables remain in their original transform
trees and reference the same shading component ID. Mutating its scalar updates
both; neither attachment steals it from the other.

Material follow-up, with constructor spelling still proposed:

```mms
let shared = Material.custom("assets/shaders/animated-led-strip.frag")
    .input_f32("time_seconds", 0.0)
R.cube() { shared }
R.sphere() { shared }

let independent = Material.custom("assets/shaders/animated-led-strip.frag")
    .input_f32("time_seconds", 0.0)
R.cube() { independent }
```

The first two consumers share one Material component and one logical renderer
material instance. The third uses another instance even with identical values.
Moving/rotating any consumer does not move the others. No value-based deduplication.

Other useful fixtures:

- The [four-strip example](custom-materials-led-strip-example-preparation.md):
  attach one shared Material declaration beneath three LED-only CombineMesh
  scopes, and a separately instantiated declaration beneath the fourth. Four
  Toon backings remain independent. Verify generated-output identity after baking.
- Two unrelated containers or GLTF roots using one shading/material declaration
  as a direct child scope. Generated primitives share the source; one local
  override remains independent.
- A shared color or texture-setting declaration is a later candidate once its
  owner resolution/update semantics are audited. Do not advertise every existing
  style component as shareable just because storage admits multiple parents.

## Graph semantics to settle before editing representation

Use one component identity plus explicit incoming/outgoing attachment edges;
reject cycles and duplicate identical edges. Settle whether edges are keyed by
parent/child pair or stable edge ID. Parent enumeration must be allocation-free
for ordinary reads, and the usual zero/one-parent case must remain cheap.

Define these operations independently:

- Add an attachment edge without removing existing edges.
- Remove one specified parent/child edge.
- Explicitly move/reparent a structural node, preserving existing movement use
  cases such as routers, scrolling, editors, and avatar topology changes.
- Detach all incoming edges without destroying the component.
- Explicitly destroy a component, independently of detachment.

### Detach one attachment versus detach all attachments

Use **attachment** for a parent/child relationship. A component instance keeps
its identity and state regardless of how many attachments it has. Both MMS and
the engine APIs need these two distinct operations; no separate component clone
or material-reference mechanism is needed merely to distinguish them.

Proposed API shape (names to confirm against existing callers):

| Intent | MMS | Engine | Effect |
| --- | --- | --- | --- |
| Detach one attachment | `parent.detach(child)` | `detach_child(parent_id, child_id)` | Remove this parent/child edge; preserve all other attachments. |
| Detach all attachments of an instance | `child.detach_all()` | `detach_all_parents(child_id)` | Remove every incoming edge of this component; preserve the instance and its outgoing child attachments. |

Because duplicate parent/child edges are rejected, the parent and child IDs
identify one attachment unambiguously. A stable attachment handle is optional
if another use case needs it. A bare `child.detach()` cannot mean "this
attachment" without an explicit parent or attachment context; do not guess.

For a material shared by a cube and sphere, `cube.detach(shared)` leaves the
sphere using the same live material instance. `shared.detach_all()` removes
both bindings, and each consumer resolves its fallback material. The detached
material can still be edited and reattached. Neither operation destroys it or
detaches its own children. Explicit destruction remains a separate lifecycle
operation, and subtree deletion must follow the shared-descendant policy below.

Verify both APIs with two parents: detach one, mutate the still-shared source,
reattach, detach all, then reattach again. Check component/material identity,
surviving bindings, fallback resolution, edge-specific invalidation, and bounded
cost: detach-one uses indexed adjacency; detach-all visits the instance's incoming
edges rather than scanning the world.

Do not silently redefine old `.detach()`/`set_parent` or runtime Attach intents
without migrating their callers. A staged shareability/edge policy can preserve
structural reparenting while enabling shared Material/Shading attachments first;
the graph storage/API must make that distinction explicit. A singular `parent_of`
must never return an arbitrary first parent for ambiguous shared nodes. Define
its valid single-parent domain and migrate shared-aware callers to plural or
contextual APIs. A designated structural parent, if used, must have documented
semantics rather than serve as a hidden traversal tie-breaker.

First supported sharing is declaration/configuration nodes whose state does
not depend on an incoming transform or inherited values. A shared declaration
can apply to multiple consumers without sharing their geometry. Material
wrappers can still scope their own subtree; attaching a wrapper with spatial
children under multiple transforms is a later explicit occurrence/transform
decision. Define where such attempts are rejected during the initial slice.
This is a delivery boundary, not a claim that general multi-parent components
are solved by supporting materials.

## Lifetime, traversal, and context

### Coordinate with the existing AttachmentSystem refactor

Updated after cleanup, 2026-10-10: [AttachmentSystem](../../src/engine/ecs/system/attachment_system.rs)
retains occupancy-specific `ActiveMount` records indexed by Rider, Mountable,
and movement root. Its structural endpoints come from a World-owned
`AttachmentRef`, which detects deletion and replacement. Mounting generates a
`TransformParent`, reparents the rider movement root beneath it, records the
original parent, and suspends input/velocity. Normal dismount restores parentage
and settings; external detach/reparent cleanup preserves the caller's topology.
Tick and targeted interaction reconciliation validate endpoints and references.
The [cleanup task](attachment-system-world-owned-topology-cleanup.md) records
passing automated/car-scene tests and performance costs; hardware validation is
still pending. [GrabbableSystem](../../src/engine/ecs/system/grabbable_system.rs)
retains its own active-grab/original-parent state. Shared leaf-Shading edges now
use the World attachment boundary; generic endpoint valence and broader shared
relationship support remain pending. See
[attachment valence/unification](attachment-valence-and-grabbable-unification.md)
and [movement authority](attachment-movement-authority-and-input-routing.md).

World must remain authoritative for actual component attachment adjacency.
AttachmentSystem and the proposed generic retained relationships should use
that same topology API, with exact parent/child identities (or attachment IDs),
not maintain a competing copy of the graph. They additionally retain semantic
policy, alignment/follow targets, occupancy, original-parent restoration, and
transaction state. A mount-point/follow relationship is not necessarily a direct
ECS edge: the current mount follows a referenced target through TransformParent.
Material-sharing edges do not imply mount valence, occupancy, or movement authority.

Before migrating mutation callers, define the common mutation/notification
boundary and lifecycle owner for system-managed attachments:

- Record precisely which structural attachment a mount/grab transaction owns;
  keep its spatial root single-parent until spatial sharing has a contract.
- Detach-one, detach-all, explicit reparenting, and endpoint destruction must
  notify/invalidate affected managed relationships through the same boundary.
  Do not require other systems to inspect AttachmentSystem's private maps.
- Preserve the cleanup contract: an external detach/reparent cancels a mount
  without restoring its original parent or undoing the caller's request.
  Extend it to detach-all and settle equivalent Grabbable cancellation behavior.
- Release semantic state, generated helpers, and input/velocity leases exactly
  once. Removing a shared material edge must not cancel an unrelated mount.
- Test a mounted/grabbed root detached externally, detached from all parents,
  reparented, or destroyed, plus endpoint removal and shared materials inside
  those subtrees. Verify final topology as well as semantic relationship state.

Coordinate the two tasks at this boundary; the material-sharing slice need not
wait for every valence, wearable, or movement-authority feature.

First refactor at this boundary: make AttachmentSystem refer to World-owned
attachments instead of treating its cached relationship as independent evidence
that parentage is still valid. World owns edge existence and endpoints; a managed
mount record associates its policy/state with the exact structural attachment(s)
created by its transaction. Use a stable attachment ID if that is the chosen
graph representation, or an exact parent/child pair with suitable lifetime
validation. Relationship endpoints (Rider/Mountable and alignment frames) remain
necessary metadata: they are not identical to the structural edge's endpoints.
The current implementation already stores adjacency in World; this refactor
formalizes the reference and coordinated lifecycle rather than removing a
second adjacency list that does not exist.

First complete the [focused AttachmentSystem cleanup](attachment-system-world-owned-topology-cleanup.md)
using single-parent storage. Then extend that boundary with multiple parents.
Prove World mutation and managed-relationship cleanup stay coherent; full
Grabbable consolidation remains in the valence/unification follow-up. Do not
move occupancy/alignment/restoration fields onto every ordinary graph edge or
make generic World topology depend on Rider/Mountable types.

- Initialize/register one component once; adding another edge registers a new
  consumer relationship, not another copy of its state/system resource.
- Removing a parent/subtree removes its edges but preserves a shared descendant
  still attached elsewhere. Specify what happens at zero incoming edges:
  detached live MMS components already exist, so zero parents alone is not a
  general destruction signal. Distinguish subtree ownership from attachment.
- Explicit source destruction removes every incoming edge, invalidates consumer
  material bindings, and retires renderer resources safely. Define conflicts
  between destruction, source removal fallback, and surviving children.
- Whole-world component work traverses unique identities once. Consumer/path
  work visits the needed edges/occurrences. A diamond graph must not repeatedly
  register one source or exponentially revisit shared descendants.
- Root queries, subtree queries, child/descendant selectors, ordering, parent
  selectors, and relative references need defined identity-vs-occurrence results.
  Do not follow an unrelated parent's scope when resolving one consumer.
- Resolve shared declaration state independently of parent-specific inheritance.
  If a component needs context, supply the selected attachment path or reject
  unsupported sharing; do not choose whichever parent was inserted first.
- Define scoped event delivery per component versus per attachment, including
  duplicate handlers reached through a diamond. Topology events identify the
  changed edge and invalidate only affected consumers, not all renderables.
- Save one component declaration and all attachment edges, then restore the
  same sharing. MMS emission must not duplicate the declaration under each
  parent. Audit current save/clone paths; specify external-edge handling when
  cloning a subtree and whether an explicit clone creates new material instances.

## Performance and verification gates

Record a tree-only baseline before changing storage. Benchmarks cover ordinary
trees, high-fan-out shared declarations, diamonds, deep scopes, and repeated
attach/detach/update/removal. Count unique components, edges, traversal visits,
resolver calls, allocation/high-water bytes, renderer instances, parameter
uploads, and initialization/event delivery. Measure initialization/mutation and
steady tick/render separately with the same release-build workloads.

Maintain indexed child and parent adjacency; mutation should not scan the whole
world for consumers. Hot unchanged frames should not allocate parent lists,
walk every shared path, or rebuild reverse links. Dirty propagation should scale
with affected edges/consumers. Cycle checking can cost reachable-graph work on
mutation; it must not run per draw. Choose inline/side storage and edge capacity
from measured zero/one-parent and fan-out cases, not assume a Vec on every
component is free. State measured regression budgets before merging.

Concrete implementation sequence:

First-slice result: shared leaf Shading, explicit detach-one/all and primitive
consumer lifetime handling are implemented, with automated checks and the
user-confirmed VR workflow. The following checklist retains the broader task
scope; its unimplemented portions must not be inferred from that narrow proof.

1. [ ] Build on the accepted AttachmentSystem cleanup/automated baseline;
   complete the singular-parent caller inventory and graph/operation/lifecycle
   contracts, initial shareable kinds, query/event policies, and tree baseline.
2. [ ] Add edge/parent storage and low-level tests for duplicate edges, cycles,
   detach-one vs detach-all, shared deletion, stale handles, and tree compatibility.
   Extend the prerequisite's World-owned attachment references and notifications;
   verify managed detach/reparent/removal behavior under the new representation.
3. [ ] Wire runtime intents, initialization/invalidation and MMS repeated-handle
   attachment. Use shared Shading on separate primitives as the first real proof.
4. [ ] Inspect renderer records after MMS initializes and edits sources: shared
   component/source IDs, distinct consumer transforms, stable material IDs where
   implemented, and separate IDs for equal-valued independent sources.
5. [ ] Verify edge removal leaves the other consumer live; local overrides,
   reparenting, source destruction, GLTF projection and CombineMesh baking behave
   correctly. Add save/load and clone-sharing fixtures under the chosen policy.
6. [ ] Run the tree/fan-out/diamond benchmarks and visible Anime/LED examples;
   record performance and lifecycle evidence before enabling broader component kinds.

Exit: the same material/shading component can be attached in two unrelated
places, survives either edge's removal, updates every eligible consumer exactly
as specified, preserves renderer instance identity, and has measured bounded
cost without regressing ordinary tree behavior. Spatial DAG rendering remains
a separate gate until occurrence-dependent transforms/draws have a contract.
