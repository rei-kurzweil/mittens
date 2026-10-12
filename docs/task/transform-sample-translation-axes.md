# Task: Axis-selective translation sampling operator

## Status and outcome

Default dependency behavior is now under review in
[Simplify default TOP translation sampling](simplify-transform-translation-sampling.md).
The external-source chains, cycle validation, and bounded refresh waves below
describe the current implementation; they are not requirements to preserve in
the simpler default. This review adds documentation only.

Implemented in the working tree; headset smoke testing is pending. The earlier
TOP API and Rust consolidation passed user smoke testing through `c38c0502`.
The new `SampleTranslation` kind in TransformOperatorComponent
replaces selected translation coordinates in its incoming world matrix with
coordinates sampled from a referenced TransformComponent, preserving the
remaining coordinates and the incoming rotation/scale basis.
The engine evaluates it during transform propagation; MMS declares its source and
axes once, without a per-frame callback.

First consumer: [AVC capsule height independent of HMD tracking](avc-capsule-before-hmd-tracking.md).
Keep the capsule's head-driven body X/Z while replacing Y from its simulated
locomotion frame, then apply its stable calibrated vertical offset.

## Terminology and math

This is component-wise selection, or a masked copy, between two vec3 values.
Swizzling can extract/reorder components, but this operator neither reduces
vector size nor reorders coordinates. X always copies source X, Y copies Y,
and Z copies Z. It is not additive movement or smoothing.

```text
input  = [body_world_x, body_world_y, body_world_z]
sample = [frame_world_x, frame_world_y, frame_world_z]
axes   = y
output = [body_world_x, frame_world_y, body_world_z]
```

Conceptually, for a boolean axis mask `m`, the rule is
`output = (1 - m) * input + m * sample`, with component-wise multiplication.
Implementation can simply assign the selected coordinates. Output is always
a vec3. Masking a world translation to zero would discard the locomotion Y;
adding the sampled translation would double-count the shared parent motion.

## Component API

Implemented grouped syntax:

Completed prerequisite: [transform operator API cleanup](transform-operator-api-cleanup.md),
including the unified component and evaluator. The operator is
`TOP.sample_translation().source(...).axes(...)`. `TransformSampleTranslation` remains a working
descriptive label in diagrams, not a second MMS API to introduce and retire.

```mms
TOP.sample_translation().source(locomotion_frame).axes("y") {}
TOP.sample_translation().source("[name='rei_mu_locomotion_root']").axes("y") {}
```

- `source(...)` is required and uses existing ComponentRef semantics: a live
  component variable becomes a GUID reference; a query string remains an
  authored query; explicit GUID references are also supported. The variable
  supplies a component, not a closure or a new parent attachment.
- Support an optional `root(...)` query scope consistent with TransformParent.
  Resolve a concrete TransformComponent; report missing, ambiguous, or invalid
  targets instead of silently sampling an unrelated ancestor.
- `axes(...)` selects a nonempty subset of x/y/z. Default is xyz, meaning full
  translation replacement. Treat the letters as a set, not a swizzle: xz and zx
  have the same effect. Reject invalid or duplicate letters and serialize a
  canonical order.
- The first API is a separately nested operator instance, like `TOP.parent(...)`
  or `TOP.inverse_local(...)`, with driven content nested beneath it. It
  edits translation only and preserves the incoming matrix's linear basis.
  Embedding it in a fork’s channel-operation list is outside this first API.
  Nest it below the fork to apply it to the recomposed matrix.
- Sample **world translation** in the first version, matching the incoming
  world transform basis. Local axes, arbitrary coordinate
  frames, scalar expressions, blend weights, and component reordering are
  follow-up work.

The source is a data dependency. It does not need to be nested beneath the
operator, and the operator does not require multi-parent component support.

## Nesting and the equivalent alternatives

There are two valid ways to build `[tracked.x, frame.y, tracked.z]`:

| Stream input | Referenced source | Replaced axes |
| --- | --- | --- |
| Head-driven body world transform | Pre-tracking locomotion frame | y |
| Pre-tracking locomotion frame | Head-driven body world transform | xz |

Choose the first for the current AVC implementation because its capsule fork
already receives the model root's world transform. Apply the new standalone
operator to the fork's recomposed output, before the capsule T's local offset.
The capsule remains within the avatar branch.

Conceptual AVC tree, with the calibrated offset expressed in world-up units:

```text
simulated locomotion frame                   <- sampled Y source
└─ InputXR
   └─ HMD tracking transform
      └─ AVC / head-driven model root
         └─ TOP.fork_trs().drop("rotation").drop("scale")
            └─ TOP.sample_translation().source(frame).axes("y")
               └─ T(calibrated capsule offset) -> Zone -> Collidable
```

This keeps the capsule structurally inside the avatar while making its final
world Y independent of the tracked branch's Y. The downstream capsule offset
must be calibrated for the newly selected Y source; copying the old model-local
center unchanged is not sufficient to prove correct standing placement.

### Exact placement and the capsule offset

Add the new component as a **downstream child of the fork operator**, then nest
the capsule T beneath it. The fork still strips rotation and scale; the new
stage edits the resulting matrix's Y; the capsule T then applies its calibrated
local offset. This updates the earlier channel-map-only proposal.

AVC generates a TOP fork with rotation/scale drops stored as values. XR inserts
the sample between fork and capsule T; desktop retains the direct capsule child.
The height fix calibrates the XR offset in world units from model-relative bounds.
HMD driver and avatar parentage stay unchanged; capsule T gains the stage as
its immediate parent.

Putting the operator below capsule T would sample after the capsule offset was
already composed, so replacing Y would discard that offset unless reapplied.
Moreover, an ordinary child operator changes the basis passed to its own
descendants, not its parent T's cached world matrix. A direct-child modifier
that edits the parent T itself would need a separate attachment/evaluation
contract. Do not imply that behavior in this first API. No callback, second
fork, or TRS decomposition is required merely to replace matrix translation.

## Runtime, resolution, and ordering

Recognize the new kind as an operator boundary and evaluate it in
`ecs/system/transform_operator.rs::evaluate_operator`, returning the edited matrix
and its downstream children. This parallels the existing standalone parent
routing/inverse-local stages. Selected matrix translation entries are replaced
without decomposing or rewriting rotation/scale, backed by the component and
MMS registration/round-trip support. Use the existing unified component and
kind-specific validation; do not introduce another standalone ECS operator type.

Resolve references outside the hot path and retain authored metadata plus a
validated live handle. Component-value/GUID sources use the existing indexed
resolution path. Query caching must invalidate when its scope or matching
topology changes; handle deletion/replacement without retaining stale values.
Do not run a whole-world selector scan on every stream evaluation.

For an unresolved source, preserve input and report an invalid sample status
rather than inventing a zero coordinate. That generic fallback is **not safe
for AVC contact**: the capsule owner must gate readiness/contact on a resolved
sample, since unmodified input Y would reintroduce the HMD feedback. Test this
both on startup and after source loss.

Sources must be propagated before consumers evaluate. Input changes already
participate in normal stream propagation; changes to a referenced source also
need targeted dependent refresh. Do not assume the operator system's tick
re-evaluates every operator: its current tick only updates elapsed-time state.
Avoid source/output dependency cycles and refresh loops. Validate selected
coordinates are finite and exercise rotated/scaled parent composition.

## Testable slices

1. **Operator and MMS surface:** verify y/xz/xyz selection, preserved input
   coordinates and full linear basis, default/invalid masks, component variables and scoped queries,
   forward references, invalid source/placement, and round-trip serialization.
   Numeric fixture: input `[10, 20, 30]`, sample `[100, 200, 300]`, y produces
   `[10, 200, 30]`; xz produces `[100, 20, 300]`.
2. **Stream dependencies:** input changes, referenced source changes, reparent,
   deletion/recreation, and invalidation refresh the correct driven subtree.
   World-space selection is consistent under nonidentity parents, shared
   parent translation is included once, and cycles are rejected or diagnosed.
   A downstream capsule T retains its calibrated offset; a stage below T must
   not unexpectedly change that parent T or unrelated sibling content.
   Repeated evaluation is deterministic. Retained reference resolution and
   masked matrix editing do not allocate or run selectors after setup; the
   existing propagation API still creates child/output vectors.
3. **AVC consumer:** wire runtime source references automatically, calibrate the
   offset, preserve upright/drop-rotation/drop-scale behavior, and verify sample
   validity gates contact. The linked capsule task owns full floor/wall,
   physics, frame-order, and headset acceptance before crouch IK begins.

## Implementation and verification

- `World::query_revision()` invalidates retained references after structural,
  name, GUID, or raw-record edits. Ordinary typed transform writes leave it
  unchanged. GUID lookup uses the existing index; query resolution requires
  exactly one concrete T. `root(...)`, `../`, and `/` scopes are supported.
- The evaluator retains each owner's input, resolved source, authored settings,
  and validation status. Missing, ambiguous, invalid, cyclic, or non-finite
  selected sources preserve input; non-finite values can recover on a pose write.
- A source-to-consumers index and changed-source set refresh affected subtrees,
  including chains across separate branches. Structural changes refresh retained
  consumers and rebuild their inherited basis through intervening operators.
  This also makes initial evaluation below a scale-dropping fork correct.
- Runtime states/index entries are removed through SystemWorld's subtree removal.
  Dependency validation and propagation are bounded to 64 dependency levels/waves;
  this is not an arbitrary-depth constraint graph.
- Automated checks cover y/xz/xyz, shear-preserving linear basis, invalid masks,
  variables and scoped/forward query references, canonical dump/reload, typed-pose
  cache retention, ambiguity/removal/recreation, non-finite recovery, own-output
  and cross-branch cycles, dependent chains, and reparented inputs.

Headset acceptance and runtime cost profiling remain in the linked AVC task.

## Related work

- [Retained operator source bindings and invalidation](transform-operator-source-bindings-and-invalidation.md):
  generalize cached sources/indexed refresh to parent and inverse-local, then
  audit contextual ancestor/camera sources and refine broad invalidation.
- [Transform operator API and output-contract cleanup](transform-operator-api-cleanup.md)
- [TransformParent component-reference routing](transform-parent-component-ref-routing.md)
- [Transform pipeline cleanup checklist](transform-pipeline-cleanup-checklist.md)
- [AVC crouch IK](avc-foot-penetration-crouch-ik.md)
