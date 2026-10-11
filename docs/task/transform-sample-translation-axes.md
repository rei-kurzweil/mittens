# Task: Axis-selective translation sampling operator

## Status and outcome

Planned; documentation only. Add a transform operator component that
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

## Proposed component API

Proposed grouped syntax; not implemented yet:

Prerequisite: the focused first phase of
[transform operator API cleanup](transform-operator-api-cleanup.md). Add this
operator under `TOP.sample_translation().source(...).axes(...)` before the
capsule-height behavior change. `TransformSampleTranslation` remains a working
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
- The first API is a standalone transform operator, like TransformParent
  or TransformApplyInverseLocal, with driven content nested beneath it. It
  edits translation only and preserves the incoming matrix's linear basis.
  Placement as an operation inside a TRS channel map is not part of this first
  API; the map must not silently ignore an unsupported nested operator.
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

AVC currently generates a fork, three map components, and the capsule T.
The preceding focused refactor replaces simple pass/drop map configuration
with builder values while preserving that fork's output. The height fix then
inserts this stage between fork and capsule T and recalibrates the offset.
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

Recognize the component as a stream boundary and evaluate it in
`transform_stream_system.rs::evaluate_stream_node`, returning the edited matrix
and its downstream children. This parallels the existing standalone parent
routing/inverse-local stages. Selected matrix translation entries are replaced
without decomposing or rewriting rotation/scale, backed by the component and
MMS registration/round-trip support. Keep this focused; grouping
all transform operators under one component with constructors/builders is
tracked separately in [operator API cleanup](transform-operator-api-cleanup.md).

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
need targeted dependent refresh. Do not assume the stream system's tick
re-evaluates every stream: its current tick only updates elapsed-time state.
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
   Repeated evaluation is deterministic and allocation-free after setup.
3. **AVC consumer:** wire runtime source references automatically, calibrate the
   offset, preserve upright/drop-rotation/drop-scale behavior, and verify sample
   validity gates contact. The linked capsule task owns full floor/wall,
   physics, frame-order, and headset acceptance before crouch IK begins.

## Related work

- [Transform operator API and output-contract cleanup](transform-operator-api-cleanup.md)
- [TransformParent component-reference routing](transform-parent-component-ref-routing.md)
- [Transform pipeline cleanup checklist](transform-pipeline-cleanup-checklist.md)
- [AVC crouch IK](avc-foot-penetration-crouch-ik.md)
