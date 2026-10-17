# MMS specification example: head/neck pose driver

Status: proposed, 2026-10-17. The [MMS example](examples/avc-head-neck-pose-driver.mms)
is an authored specification for future implementation, not a runnable scene.
It uses the [typed math proposal](declarative-vector-operator-components.md) to
spell out the two slices of the [head/neck task](../task/xr-head-neck-bend-and-body-offset.md):
rotation-derived placement, followed by bounded planar follow and geometric
closure. The ordinary user-facing behavior remains automatically wired by AVC;
this file exposes a future graph representation for a simple fixture. The
selected first implementation is native `HeadNeckBodyConstraint`, replacing
the eligible avatar's current body-follow constraint without depending on VMN.
These examples remain specifications for VMN and reference math, not required
runtime machinery for that native implementation.

## Read the example

`head_neck_pose_driver(source_kind, bounded_follow)` selects one of four graph
configurations at construction. Its function and `if` statements do not run
during each pose update. Nodes capture constants or live typed ports; the engine
would compile them into the retained native plan. There are no per-frame MMS
callbacks, arithmetic expressions, array updates or string selector lookups.

The example intentionally expands the scalar/vector/quaternion calculations.
It does not hide the unresolved real-rig solve behind a proposed avatar-specific
IK component. The bounded branch implements an exact single-segment fixture
under the support conditions below, not every mapped humanoid.

## Two smaller examples

The two separate MMS files below each contain one composite node, with no nested
composite scopes, construction-time driver branches or temporal state. They use
`VMN` / `VectorMathNode` as a candidate replacement name for `MG` / `MathGraph`;
the larger example retains the earlier spelling. Both still denote the same
proposed composite of scalar/vector/quaternion operations. Individual numeric
nodes remain connected through named operands. None of these APIs is implemented.

Leaf declarations omit empty `{}` bodies. When a declaration has two or more
chained calls, start the chain on a new line after the component type and put
each call on its own indented line. Braces remain around actual properties or
children, such as the composite's contents and the named T placeholders. This
style also applies to the larger example and argument-nested math nodes.

| Example | Reused computation | Output/transform arrangement |
| --- | --- | --- |
| [One channel, one child T](examples/vector-math-node-neck-rotation.mms) | The current `preferred_neck` operations: relative head rotation, pitch extraction, weighting/clamping, axis-angle and body-frame composition | One rotation export supplies the basis for exactly one direct child T; its authored local TRS follows |
| [Two output bundles](examples/vector-math-node-neck-and-body.mms) | The same preferred rotation, `preferred_arc` and the immediate-placement subtraction `body = head - c - preferred_arc` | Named neck-rotation and body-translation exports, selected by two explicit transform branches; no nested composite node |

The files extract the existing operations/operand wiring rather than replacing
them with a visual approximation or a pitch-to-position multiplier. Their scopes
are flattened and their input/output boundaries changed, so the complete files
are not verbatim copies of an enclosing block. Input types and coordinate spaces
are explicit. A rotation-only transform inherits translation/scale; its final
calibrated attachment to an imported bone remains AVC integration work.
Similarly, the two-output example's translation-only body export preserves the
incoming rotation/scale: integration must supply its existing body-yaw basis.
These separate identity T placeholders illustrate channel routing, not a complete
connected skeleton. The larger example also publishes neck position/body yaw.

The one-channel example is a reusable **preferred-rotation calculation**, not
an independently complete head/neck constraint. In the immediate-placement
slice, that preferred rotation can be applied directly. In the bounded-follow
slice, positional closure may correct it before publication.

The two-output example is the **stateless immediate-placement fixture**. Its
rotation/arc primitives are reusable in the final driver, but its body-position
formula is not the final bounded-follow body output. The latter also needs the
retained anchor, feasibility/closure correction and atomic state acceptance
shown in the larger example. Neither small example implements those policies,
full real-rig calibration or crouch IK.

Keep the synthetic calibrated geometry clearly labeled and identify which
inputs would be supplied from the retained humanoid rest path. The aim is small
composable pieces of the actual proposed computation, with simplified fixtures
and a deliberately narrower behavior contract.

## Required input bindings

`ValueSource.input(key).type(type)` is proposed syntax for a native graph input
binding. Keys below name supplied slots, not component queries or MMS variables.
`ValueSource.constant(...)` infers Scalar for a number and Vec3 for a three-number
literal. All pose sources must be valid and in the same current world frame.

| Binding | Type / meaning |
| --- | --- |
| `head_world_orientation` | Quaternion; calibrated anatomical head orientation, including requested desktop look or measured XR orientation |
| `head_world_position` | Vec3; XR head-bone pivot after the existing eye/head conversion; bound only in XR configurations |
| `body_world_yaw` | Quaternion; upright body orientation supplied by the existing single yaw-follow owner |
| `body_anchor_world` | Vec3; calibrated neutral body anchor transported by intentional locomotion; desktop positional authority and initial follow anchor |
| `locomotion` | Non-numeric frame binding used by PlanarFollow to transport retained anchor state; it is not a new generic matrix-valued numeric node |

AVC prepares these bindings once and refreshes them on relevant rig/driver/frame
changes. The graph must not read its own corrected body/head matrices as next
frame's input authority. Driver changes reinitialize the appropriate anchor;
tracking loss suspends XR solving, and reacquisition establishes a feasible
current pose. Mounted/seated ownership remains the consumer task's separate
policy. Invalid sources do not successfully publish a pose or commit state.

## Nested graph and transform interface

`MG` is the proposed shortform of `MathGraph`. A named nested graph is a value
producer/owner, not an implicitly applied transform stage merely because its
declaration appears inside another graph. Its exported values are applied where
forwarded or explicitly selected.
At the math level it is a composite node/scope; its internal operations form a
graph. The outer evaluation owner compiles/schedules the connected operations
together, rather than ticking each nested MG separately.

The example proposes the following concrete export spelling:

| Syntax | Meaning |
| --- | --- |
| `graph.output` | The graph's single export, a typed value or bundle; reject if there are zero or several exports |
| `graph.output.rotation` | A live Quaternion port from that bundle; translation/scale similarly expose Vec3 ports when declared |
| `graph.outputs.body` | Export explicitly named `body` with `.name("body")` |
| `VMNOutput.from(bundle)` | Re-export the same channel ports and coordinate-space metadata; no new evaluation or scene-matrix readback |
| `TOP.channels(bundle) { T {} }` | Apply the selected bundle at this hierarchy boundary, then compose the T's local TRS |

`VMNOutput` is recipient-independent. Generic math exports use
`.name("pitch").value(pitch)` and expose a typed port at
`producer.outputs.pitch.value`; they need no transform channel. The channel
builders used in this pose example are a convenience for exports destined for
transforms, and their ports remain reusable by other VMNs. Another VMN binds a
port with `.input("local_name", producer.outputs.pitch.value)`; its internal
`ValueSource.input("local_name")` reads that node-local binding. See the
[complete scalar-to-node wiring example](declarative-vector-operator-components.md#named-outputs-and-input-bindings).

For supported TRS outputs, Quaternion uniquely identifies rotation, while Vec3
requires translation/scale selection. This does not remove explicit export or
destination selection. The pose source uses `.rotation(...)`; a generic
`.value(q)` export can instead infer rotation at a transform consumer.

Graph ownership and data connections are separate. Nested graphs can capture
already-declared ancestor ports; the compiler validates/lower dependencies across
these scopes without copying shared producers or separately ticking each nested
graph. One outer evaluation snapshots inputs, computes the reachable operations,
and publishes related outputs. Reject same-update cycles across graph boundaries.

For a graph with one transform-compatible export, a direct child T consumes it automatically. That is
the same bundle interface as explicit selection, with the output adapter inserted
internally. A named graph producer with no content children just exports data.
With several transform-compatible exports, unqualified transform content is ambiguous and rejected.
An inline content MG which explicitly forwards a selected outer bundle supplies
its own output basis, so it is also an explicit consumer at the outer boundary.
The example uses `MG { VMNOutput.from(body_pose) T {} }` for the
body branch and `TOP.channels(...)` for neck/head to show both forms. Either
starts from the outer graph's incoming basis, without applying an implicit
outer bundle first. A nested numeric producer with no content is not a consumer.
Identity T placeholders in this example make effective pose destinations clear;
they do not show the final imported skeleton attachment/calibration code.

Forwarding preserves bundle space/channel metadata; it must not silently relabel
world values as local. Missing channel access is an error. Output descriptors
and native slot references are not sampled MMS object fields. The existing
runtime does not implement this live-port member access, first-builder default
construction, graph ownership, acceptance wiring or transform integration yet.

## Fixture geometry and limits

All lengths are metres, angles radians, quaternions xyzw. The fixture has +Y up,
-Z forward, identity rest rotations, an upright yaw-only body, root-to-neck rest
offset `[0.02, 1.30, 0.03]`, and neck-to-head rest vector `[0, 0.12, 0]`.
Uniform scale is already included; there is no remaining shear or nonuniform
scale. Positive anatomical pitch is rotation about -X. Pitch sharing is 0.5,
clamped to +/-25 degrees. These values are illustrative, not measured rig data
or selected shipping defaults.

The preferred graph computes `q_body * bend(clamp(0.5 * relative_head_pitch))`.
Desktop head position is `body_anchor + c + rotate(q_preferred, v)`. XR uses the
measured head position instead. Both retain the full requested head orientation.

Immediate placement subtracts the same geometry from the desired head position:
`body = head - c - rotate(q_preferred, v)`. This makes desktop look-only body XYZ
constant and makes XR translation follow immediately without stretching.

The bounded branch retains an X/Z anchor. PlanarFollow first applies a 0.06 m
comfort radius to `head - neutral_root_to_head`. The actual supported neck cone
has symmetric 25-degree swing in every planar direction. Its hard planar reach
is `0.12 * sin(25 degrees) = 0.05071419140888393 m`, smaller than that comfort
radius. Geometric closure therefore makes the body follow earlier when necessary:

```text
requested_horizontal = (head - c - candidate_anchor).xz
horizontal = requested_horizontal * min(1, hard_reach / guarded_length)
height = sqrt(length_squared - dot(horizontal, horizontal))
required_arc = horizontal + [0, height, 0]
body = head - c - required_arc
neck_rotation = align(preferred_arc, required_arc) * preferred_rotation
```

The positive square root selects the supported upper hemisphere. A finite
positive segment length, cone angle below 90 degrees and validated quaternions
are prerequisites. Clamping the square-root radicand protects roundoff; it does
not authorize arbitrary invalid geometry. The direction alignment is a shortest
arc; these fixtures avoid its zero/opposite-vector singularities. Quaternion
composition uses `rotate(a * b, v) = rotate(a, rotate(b, v))`.

Position has priority over the preferred neck share while holding an anchor.
For example, rotating a head at an unchanged pivot cannot force the preferred
neck bend while also holding body X/Z and preserving segment length. The bounded
fixture solves the actual separation; the rotation-only fixture demonstrates
the different policy of moving the body to retain that preference.

Residual head rotation is unrestricted in this fixture, matching the initial
desktop policy of leaving requested rotation on the head. The full task's
asymmetric anatomical limits, head-versus-neck limits, nonidentity rest frames,
multi-transform virtual FK path and more general feasible-anchor projection
remain implementation work. This example does not claim the cone solves them.
Body Y follows connected geometry; it is not a fixed-height foot-plant solver.
The [crouch task](../task/avc-foot-penetration-crouch-ik.md) remains separate.

## Temporal acceptance and propagation

`follow.candidate` reads a proposed anchor from previous accepted state.
`follow.accept_from(body_position)` is proposed construction-time wiring of a
delayed next-state edge. It does not update state during construction, call MMS
each frame, or create a same-update numeric cycle. Convert accepted final body
X/Z into the retained locomotion frame and commit once per intended update,
only after the related pose outputs validate and apply successfully. Repeated
reads/propagation must not advance state again. Rejection preserves prior state.

The example emits body, neck-base and head-pivot world bundles. Verify
`neck_base + rotate(neck_rotation, rest_segment) == head_position`. Propagate
the accepted body/neck pose before arms, later crouch targets, secondary motion
and skinning read it. The real AVC integration must preserve rest neck translation
and scale, insert effective output boundaries correctly and replace the previous
body-follow writer. It cannot append a competing correction writer or apply
model-root calibration twice. XR tracking origins, rendered eye poses, physics
capsule Y and locomotion authority are never outputs of this graph.

## Acceptance cases for implementation

Start with identity body yaw, neutral body anchor `[0, 0, 0]` and neutral head
position `[0.02, 1.42, 0.03]`. The numbers below are geometric expected results,
not outputs captured from an implemented MathGraph.

| Case | Expected result |
| --- | --- |
| Neutral, either source and slice | Body at the anchor; neck/head match calibrated rest landmarks |
| Immediate XR, fixed head pivot, look down 50 degrees | Preferred neck bend 25 degrees; body offset about `[0, 0.011243066, 0.050714191]`; tracked head unchanged |
| Desktop look sweep, either slice | Body XYZ stays at the intentional locomotion anchor; head follows the connected arc, including height; full head orientation retained |
| Bounded XR, head displaced +0.02 m on X, neutral orientation | Body X/Z holds; neck tilts to close; body rises about 0.001678404 m |
| Bounded XR, head displaced +0.08 m on X | Comfort stage proposes body X = 0.02 m; hard reach requires final body X about 0.029285809 m and Y about 0.011243066 m; commit that final anchor |
| Hold/return from a lean | Stable held pose; return does not automatically recenter the retained anchor |
| Rotate without translating an already leaning XR head | Use the total neutral-relative range policy; do not promise rotations can never cause follow near the boundary |
| Move/turn the locomotion frame or rebind the driver | Transport/reinitialize anchor as specified; simulated locomotion is not delayed by the lean allowance |
| Shared/nested exports, repeated propagation | Same typed results; one evaluation/state commit per intended update; no double-applied local offset |

Also sweep diagonal displacements, combined head rotations and body yaw; preserve
neck length and endpoint closure. State/transform verification belongs in the
[production math suite slices](../task/declarative-math-performance-suite.md#production-adapter-and-remaining-slices).
Parser acceptance alone does not establish bindings, numerical results, runtime
availability or headset comfort.
