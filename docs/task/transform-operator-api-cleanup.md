# Task: Transform operator API and output-contract cleanup

## Status and outcome

Existing MMS operator migration implemented; awaiting the user’s smoke test. Make transform
operators a coherent component/API family with obvious input/output composition
and compact MMS authoring. Use **transform operator** as the public term; pipeline and stream remain historical
or implementation names until deliberately migrated.

MMS namespace: `TOP` (transform operator), long form `TransformOperator`.
The parser uses dotted calls, such as `TOP.fork_trs()`; `TOP::` describes the
namespace informally. Constructors select the operator kind; builders configure it.
Existing operator spellings, examples, and dumps migrate together. The grouped API
lowers to existing Rust kinds/plans during this behavior-preserving checkpoint.
The proposed `sample_translation` operation is not implemented yet.

**User checkpoint:** finish migrating every existing MMS operator, then stop for
manual testing before adding axis sampling, changing AVC capsule height, or
implementing crouch IK. Generated capsule/body and gizmo forks use value settings;
camera markers retain their documented live-settings T exception. See the
[implemented API](../spec/transform-pipeline.md) for the constructor inventory.

**Next after smoke testing:** remove the old TransformMapTranslation,
TransformMapRotation, TransformMapScale, TransformDrop, and TransformMergeTRS
components and migrate their remaining Rust fixtures to builder/value settings.
Retire obsolete standalone filter/yaw/ancestor configuration helpers where their
only role is configuring a fork. Delete the legacy map parser branches, dump
folding, and `is_operator_configuration_child` filtering once there are no such
helper trees left. Every ordinary fork child should then be output by construction.
This is the next cleanup slice before proceeding to axis sampling / locomotion;
it should not be deferred behind those behavioral changes.

This is a new cleanup beyond the earlier
[authored pipeline cleanup](transform-pipeline-cleanup-checklist.md), which
removed TransformPipeline and TransformPipelineOutput. It does not restore an
output wrapper or introduce a second graph language.

## Findings before this migration

`transform_stream_system.rs::downstream_children` already sends the fork's
recomposed matrix to every direct child except TransformMapTranslation,
TransformMapRotation, TransformMapScale, and TransformMergeTRS. Output roots
are not restricted to TransformComponent. Another operator is already a valid
continuation. `TransformSystem::propagate_subtree` evaluates operators during
hierarchy traversal and continues through their returned output children.

The authoring problem is that configuration and output occupy the same child
list and are distinguished by component-type exceptions:

```text
TransformForkTRS
├─ TransformMapRotation               <- configuration
│  └─ TransformDrop                  <- configuration
├─ TransformMapScale                  <- configuration
│  └─ TransformDrop                  <- configuration
└─ TransformSampleTranslation         <- output continuation (proposed)
   └─ capsule T                      <- output continuation
```

The new sample operator does not invent operator-to-operator composition. It
exposes how non-obvious the current configuration/output classification is.
TransformCameraSpecific has another exception: its markers contain T settings
that are explicitly not downstream content. A T therefore does not universally
mean output in the current API either.

Existing channel settings are also fragmented across separate ECS components.
Pass is usually implicit rather than an authored Pass component; Drop selects
zero translation, identity rotation, or unit scale according to its containing
map. TransformMergeTRS remains a nominal child the parser excludes and warns
about if it contains content, despite recomposition already being implicit.

## Desired composition rule

For an ordinary transform operator:

1. Its parent provides an incoming world transform basis.
2. The operator evaluates its configured operation, optionally reading an
   explicitly referenced transform and retained runtime state.
3. Its resulting basis is inherited by all ordinary children.

An output child may be another operator, a T, a renderable, or a transparent
component containing downstream content. There is no distinguished first T,
implicit singular output, or reliance on a child's name. Multiple children
receive the same result and can apply independent further operations.

```text
upstream T
└─ operator A
   ├─ operator B
   │  └─ downstream T -> rendered content
   └─ another downstream subtree
```

Operators act on the basis supplied to their children. They do not rewrite
their parent T. A following T composes its authored local TRS afterward.
Referenced source transforms are data dependencies, not additional parents.
Validate dependency refresh and cycle handling without requiring multi-parent
components or a general constraint graph.

Configuration belongs in constructor arguments, builder values, or explicit
references stored on the operator. For the cleaned-up ordinary operators,
children are output content, not configuration. If specialized behavior still
needs configuration subtrees, give it an explicit documented role and validation;
do not infer it by finding the first T or an undocumented type exclusion.

## Proposed grouped component API

Mixed implemented/proposed illustration; `sample_translation` is still future MMS:

```mms
TOP.fork_trs().drop("rotation").drop("scale") {
    TOP.sample_translation().source(locomotion_frame).axes("y") {
        T.position(0.0, calibrated_capsule_center_y, 0.0) {
            // Runtime capsule Zone / Collidable supplied by AVC.
        }
    }
}
```

The fork keeps unspecified channels. `.drop(channel)` records a channel policy
on that operator instance; it does not create a child component. An optional
`.pass(channel)` restores the pass policy. A repeated simple pass/drop setting
for the same channel has an explicit last-setting-wins rule. Reject unknown
channel names. Rotation drops to identity, scale to unit scale, translation
to zero, as today.

The sample constructor selects a separate operator kind. `.source(...)` stores
a ComponentRef and `.axes(...)` selects coordinates to replace. Both component
variables and query/GUID strings follow the existing engine reference API.
The source is not evaluated by an MMS callback each frame.

A likely Rust representation is one `TransformOperatorComponent` with a typed
kind enum and validated per-kind configuration. `TOP` would be its MMS alias,
not an untyped bag of arbitrary fields. Constructor/method validation must
remain specific to the selected kind. Switching kind on a live instance is
outside the first migration slice; do not accidentally retain incompatible
temporal state or references through such a change.

Separate compact authoring from runtime implementation: grouped constructors
can initially lower to existing components/plans while behavior is proven.
If the existing representation remains temporarily, document that distinction
and account for generated configuration helpers. The final authored child rule
must still be clear; simply aliasing old types as TOP would not remove mixed
configuration/output children.

## Inventory and migration decisions

| Existing surface | Proposed cleanup direction | Semantics to preserve |
| --- | --- | --- |
| TransformForkTRS + maps + Drop | `TOP.fork_trs()` with pass/drop builders | TRS decomposition/recomposition, absent channels pass, existing dropped defaults |
| TransformMergeTRS | Retire authored marker after compatibility audit | Fork already recomposes; do not introduce an extra output boundary |
| TransformParent | Grouped parent-basis constructor, exact name TBD | Explicit world-basis source/scoped reference; does not reparent topology |
| TransformApplyInverseLocal | Grouped inverse-local constructor | Exact multiplication order and source's authored local matrix; missing/singular-source policy |
| Proposed TransformSampleTranslation | `TOP.sample_translation().source(...).axes(...)` | World-axis masked replacement, preserve other coordinates and matrix basis |
| TransformSampleAncestor | Explicit contextual constructor or migration to source references | Ancestor counting, coordinate space, and input meaning; do not rename it as ordinary source sampling |
| Vector3TemporalFilter / QuatTemporalFilter | Typed smoothing configuration or dedicated operator kinds | Time units, dt, state ownership, ordering, unchanged-channel behavior |
| QuatExtractYaw / QuatYawFollow | Explicit yaw kinds/configuration | Body-relative conventions, yaw limits/rates and initial calibration; preserve AVC head/body behavior |
| TransformCameraSpecific + settings T children | Separately decide explicit camera configuration/reference shape | Selected mono/stereo basis, idempotent refresh, matching camera/BVH transforms |

Keep ordered channel-operation lists representable as typed configuration if
needed. Simple pass/drop flags do not replace ordered smoothing, yaw, or
ancestor operations. Specify ordering and ownership before removing their map
components; avoid replacing one opaque child convention with opaque builder
order. Independent full-matrix operators can compose through nesting where
that preserves the original math. Do not assume every TRS operation is safely
interchangeable with a world-matrix edit: the translation sample should preserve
the full linear basis, while a TRS fork can decompose/recompose it.

Camera-specific settings need their own migration decision before claiming
all configuration children are gone. They are currently live transforms
mutated by the gizmo system, not just static constructor parameters. Explicit
references to settings transforms are one option; embedded typed configuration
is another. Preserve runtime updates, cache behavior, and serialization first.

## Terminology and compatibility scope

Use transform operators in active docs, new APIs, and user-facing diagnostics.
Keep current names when describing actual source files/types or existing MMS
syntax. The implementation can be renamed to TransformOperatorSystem and
TransformOperatorPlan when that migration lands; the exact Rust naming is
secondary to the authored contract.

Audit MMS constructor names, public Rust exports, serialization/dumps, query
component names, GUID/reference behavior, generated AVC/gizmo helpers, and
existing saved examples before choosing aliases or an atomic migration. Do not
silently change selector meanings or temporal-state identity. Preserve explicit
historical status notes rather than rewriting old architecture as implemented.

This task supersedes the transform-naming direction in
[legacy transform terminology/command queue cleanup](legacy-transform-pipeline-and-command-queue-cleanup.md).
The separate command-queue/signal-staging work remains separate.

## Testable slices

Delivery decision: migrate **all existing MMS operators** first, including
stateful and camera-specific spellings, then stop for the user's manual test.
This checkpoint deliberately retains the existing capsule-height behavior.
After the API passes that checkpoint, remove the obsolete configuration helper
components and the compatibility exclusions described above. Then introduce the
Y sample and its composition fixture, then apply AVC calibration/contact routing in the dedicated height-fix
task. Crouch IK follows afterward. Backend type consolidation and camera-settings
representation changes can be reviewed separately; they are not implied by an
MMS namespace migration.

1. **Contract and inventory:** document current configuration exceptions and
   normal downstream children; prove nested operators and multiple output
   branches with the existing evaluator. Settle TOP/long-form naming and the
   per-kind schema, including syntax accepted by the MMS parser/runtime.
2. **Simple fork configuration:** replace pass/drop map helpers with typed
   builder configuration. A camera-constraint example drops rotation; UI
   scrolling/gizmo fixtures drop scale. Numeric output, local-offset order,
   render placement, and picking must match before/after. Round-trip dumps
   preserve policy and child topology.
3. **Composition and source operators:** introduce grouped constructors for
   whole-basis routing, inverse-local, and axis-selective sampling. The capsule
   fixture chains fork → Y sample → offset T; component variables, scoped
   queries, source refresh/loss, cycles, and multi-child outputs are tested.
4. **Stateful and specialized cases:** migrate AVC yaw/smoothing/ancestor use
   and explicitly resolve camera-specific settings. Repeated refresh does not
   compound offsets/scales; state is isolated per live operator, cleaned on
   removal, and correctly invalidated when its configured kind/inputs change.
5. **Representation and naming cleanup:** retire superseded helper nodes/types
   and migrate active docs/examples after compatibility decisions. Validate
   initialization, updates, removal, serialization, rendering/picking, and
   desktop/XR smoke cases. Measure component count, plan/propagation cost, and
   allocations; no per-frame MMS evaluation or whole-world dependency scan.

Implement the [translation-sample task](transform-sample-translation-axes.md)
under the grouped API after the existing-API manual-test checkpoint. Avoid introducing a
temporary standalone MMS spelling only to replace it immediately. The capsule
fixture in slice 3 proves operator composition and offset order; the subsequent
[height-fix task](avc-capsule-before-hmd-tracking.md) owns AVC calibration,
contact routing, and headset acceptance. Keep those reviewable changes separate.

## API migration checkpoint validation

Automated checks on this slice:

- 84 transform checks passed with the three baseline failures below excluded.
- Four focused TOP tests passed: constructor/kind validation, independent output
  branches and offset order, source refresh and GUID-preserving dump/reload, and
  loading the migrated camera-constraint/UI examples.
- Runtime schema checks, 20 gizmo tests, 27 AVC tests, nine mittens-corp tests
  (including managed car mounting/dismounting), and humanoid AVC example parsing
  passed. All Rust examples compile with `cargo check --examples`.
- Legacy-map and value-configured smoothing/yaw/drop results match over 90
  frames; separate live operators retain separate state. Translation and scale
  smoothing now use separate channel cache keys.
- Dumps’ GUID property is registered in RuntimeSpec and supported by the direct
  registry, so referenced transforms survive the normal saved-scene load path.

Broader test selections have known failures also reproduced from HEAD in a
separate source checkout: legacy animation/update-transform evaluation (two),
legacy quaternion builder evaluation (one), animation/keyframe/music-note
round-trips (three), and the secondary-motion example’s spring-chain count
expectation (one: 17 expected, 15 present). These remain outside this API slice.
They are recorded rather than counted as passing checks.

Manual acceptance is pending. Use the rebuilt release engine to check:

1. `examples/mittens-corp.mms`: avatar body/head/controller motion, normal
   locomotion, car entry/exit, and prop selection/gizmos. Capsule calibration and
   contact routing are unchanged; the existing HMD Y issue is still expected.
2. `examples/camera-constraint.mms`: translation follows input while mouse-look
   rotation is dropped.
3. `examples/ui-layout.mms`: clipped scrolling content keeps its intended scale.
   On desktop and XR, gizmos should retain their current camera-dependent size
   and have matching rendered and picked positions.

Stop at this checkpoint. After feedback, remove obsolete helper components and
compatibility filtering before implementing the new translation-axis sample or
changing locomotion behavior.

## Related work

- [AVC capsule height independent of tracking](avc-capsule-before-hmd-tracking.md)
- [TransformParent routing](transform-parent-component-ref-routing.md)
- [Camera-specific transform configuration](../spec/transform-camera-specific.md)
- [Transform operator mechanics under the existing implementation](../spec/transform-pipeline.md)
