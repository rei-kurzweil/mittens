# Declarative vector operator components

Status: proposed names and execution model, 2026-10-17. Documentation and
diagram only; none of the new components below is implemented. `Vec` and `Quat`
are proposed MMS operator-family shortforms, and `MG` abbreviates `MathGraph`.
Naming candidate: `VectorMathNode` / `VMN` may replace `MathGraph` / `MG` as the
public composite-node name. The two small examples use `VMN` tentatively; the
larger example retains `MG`. Both spellings describe the same proposed semantics,
including scalar and quaternion math. This is not an implemented alias or a
final naming decision.

## Direction

Treat the numeric part as a small typed value graph that MMS can describe and
the engine can evaluate directly. Scalar, Vec3 and quaternion operations return
values; they do not all consume or produce a transform matrix. Existing transform
operators remain useful at the boundaries. The AVC constraint is one consumer
of this vocabulary, rather than the definition of the vocabulary.

This updates the earlier [declarative transform math](declarative-transform-math.md)
sketch: the generic math/patch box now has explicit proposed component boundaries.
[The interactive diagram](../how_to/avc-head-neck-constraint-diagram.md) shows the
catalog, its use in AVC, and future authoring context.

This does not require a second textual parser. Ordinary MMS component
declarations and a native Rust builder can describe the same typed nodes and
connections. The fixed numeric vocabulary is the small domain language; the
native evaluator does not interpret MMS source during a pose update.

The [head/neck MMS specification example](avc-head-neck-pose-driver-example.md)
spells out nested graphs, desktop/XR source preparation, rotation-derived body
placement and bounded planar follow with explicit single-segment closure. Its
[`.mms` source](examples/avc-head-neck-pose-driver.mms) is a target for future
implementation, not a runnable engine scene or a full real-rig solver.
Start with the smaller flat examples to inspect output application:
[one rotation channel and one child T](examples/vector-math-node-neck-rotation.mms),
then [two outputs sharing the neck calculation](examples/vector-math-node-neck-and-body.mms).
Neither introduces nested composite nodes or temporal state.

## Proposed components

Names identify proposed operations/components, not current registered APIs.
Where a family has several operations, each declaration chooses a fixed typed
operation and arity; it does not contain an MMS callback or arbitrary expression.

| Proposed name | Inputs -> output | Role in the first constraint |
| --- | --- | --- |
| `MathGraph` / `MG` | Node descriptions / bindings -> typed exports and retained native plan | Composite math node/scope; the outer evaluation owner schedules the connected computation |
| `ValueSource` | Declared constant/rest value, input channel or bound upstream port -> Scalar / Vec3 / Quaternion | Expose rig calibration, head pose and current body yaw |
| `QuaternionRelative` | Reference quaternion, desired quaternion -> Quaternion | Express head orientation relative to the body/anatomical frame |
| `QuaternionPitch` | Quaternion, calibrated frame -> Scalar radians | Extract the head's signed flexion/extension |
| `ScalarArithmetic` | Fixed scalar operation / operands -> Scalar | Weight angles and compose length/reach formulas; add, subtract, multiply, divide, minimum/maximum, square root |
| `ScalarClamp` | Value, minimum, maximum -> Scalar | Bound the neck's contribution and supported geometric quantities |
| `QuaternionAxisAngle` | Axis Vec3, radians -> Quaternion | Construct the bounded neck bend |
| `QuaternionCompose` | Ordered quaternions -> Quaternion | Combine body/rest/bend frames and world/local conversions |
| `VectorRotate` | Quaternion, Vec3 -> Vec3 | Turn the measured neck segment into a rotated displacement |
| `VectorArithmetic` | Fixed vector operation / operands -> Vec3 | Add/subtract offsets and multiply by a scalar |
| `VectorProject` | Vec3, axis mask / frame -> Vec3 | Retain X/Z and zero Y for planar measurement |
| `VectorMeasure` | Fixed length/dot operation / vectors -> Scalar | Measure departure and support geometric reach checks |
| `QuaternionFromDirections` | Rest direction, desired direction -> Quaternion | Align the neck with positional separation, with explicit singularity handling |
| `PlanarFollow` | Target, previous anchor, frame, range -> candidate Vec3; accepted next state | Retain the body anchor until the common calibrated allowance is exceeded |
| `VMNOutput` | Typed producer ports -> named public value or bundle | Export values to other math nodes or consumers; optional channel metadata supports transform adapters |

`TOP.yaw_follow` remains an existing transform operator. A `ValueSource` can
expose its current output; this proposal does not replace it with a second yaw
implementation. The outer `MathGraph` is an evaluation owner, `ValueSource` an input
boundary, and `VMNOutput` an export declaration. Transform application belongs
to an automatic child adapter or explicit `TOP.channels` consumer. The other components
are numeric operations or explicit temporal behavior.

`MG` groups a graph of individual math operations and exposes its outputs. When
nested, it acts as a composite node within the enclosing computation, rather
than introducing another independently scheduled graph. One output does not
mean one operation: a multi-node calculation may expose just one channel.
Component ownership, numeric dependencies and the native evaluation boundary
are distinct; nesting alone must not add another tick or duplicate shared state.

## Vec and Quat shortforms

Use `Vec` and `Quat` as the public MMS families for vector and quaternion
operations. They select the same typed operations listed above, rather than
introducing a second implementation or a generic dynamically typed value node.
The long names remain useful for describing individual operations in this draft;
the registry's final long family names are not settled here.

| Proposed MMS spelling | Operation described above |
| --- | --- |
| `Vec.add()`, `Vec.subtract()`, `Vec.multiply()` | `VectorArithmetic`; multiply a vector by a scalar |
| `Vec.rotate()` | `VectorRotate` |
| `Vec.project()` | `VectorProject` |
| `Vec.length()`, `Vec.dot()` | `VectorMeasure` |
| `Quat.relative()` | `QuaternionRelative` |
| `Quat.pitch()` | `QuaternionPitch` |
| `Quat.axis_angle()` | `QuaternionAxisAngle` |
| `Quat.compose()` | `QuaternionCompose` |
| `Quat.from_directions()` | `QuaternionFromDirections` |

For example, `Vec.rotate()` and `Quat.compose()` select operations without
`.new()`. Operand builders configure the selected nodes. Operation selectors choose
the fixed operation; subsequent named builders wire its operands. Families name
the operation domain, not necessarily the result: `Vec.dot()` and `Quat.pitch()`
produce scalars. `Vec` initially operates on Vec3; it does not imply arbitrary
vector dimensions. Quaternion remains a distinct typed port, not a Vec4 alias.

## Optional bodies and declaration formatting

An empty component body is optional for all proposed math components, including
sources, scalar/vector/quaternion operations, output declarations and composite
nodes. Omit `{}` for leaves; use a body when declaring properties or children.
An omitted body is semantically the same as an empty one: it still constructs
a component/node, not a copied value or a per-frame function call. Nested operand
declarations can omit empty bodies too. A composite containing its operations or
a child T still needs braces around that content.

For these examples, when a declaration has two or more chained calls, put a
newline immediately after the component type, then put each call on its own
indented line. Include the operation selector in that chain; do not leave
`Vec.rotate()` on the declaration line when operand builders follow it. A single
call may stay inline:

```mms
let rest_segment = ValueSource.constant([0.0, 0.12, 0.0])
let arc = Vec
    .rotate()
    .rotation(neck_orientation)
    .vector(rest_segment)
```

The current MMS parser already supports omitted bodies on chained component
expressions and whitespace before chained calls. Registering the proposed node
types and interpreting their builders/ports remain separate implementation work.
A bare registered type with no calls and no body must also support implicit
default construction in the proposed API; existing parsing of an unqualified
bare identifier alone does not establish that behavior.

## Operand wiring through builder arguments

Direction update, 2026-10-17: use named operand arguments on constructors/builders.
The component forest supplies graph ownership and inspection; child order does
not mean left/right, vector/quaternion, or minimum/maximum. Inline component
expressions may nest inside those arguments, and named nodes/ports allow reuse.

Illustrative proposed syntax, not registered engine APIs:

```mms
MathGraph {
    let arc = Vec
        .rotate()
        .rotation(neck_world_rotation)
        .vector(neck_to_head_rest)

    let body_position = Vec
        .subtract()
        .left(
            Vec
                .subtract()
                .left(head_world_position)
                .right(root_to_neck_world)
        )
        .right(arc)

    VMNOutput
        .translation(body_position)
        .space("world")

    T { /* body content; its local TRS composes after the graph output */ }
}
```

The input names above denote already-declared typed sources/ports in the graph's
scope. Exact method names remain proposals. Prefer semantic names such as
`rotation`/`vector`, `left`/`right`, and `value`/`minimum`/`maximum`.
Noncommutative operands must never be inferred from sibling order.

Default construction is implicit: the first builder can be
`VMNOutput.translation(p)`, without a `.new()` call. The first
operand builder must work just like subsequent builders; it does not select a
different constructor. Named operation selectors such as
`Vec.subtract()` remain useful because they choose the operation.
Registration/materialization must distinguish those selectors from operand
builders and apply the default constructor when the chain starts with a builder.

A repeated reference to one named producer connects consumers to the same
output slot; two inline declarations create distinct authored nodes even with
equal settings. An operand with multiple outputs needs explicit port selection.
An ordinary MMS getter supplies a copied value, not an automatically live port.
Constants can use literal values, with fixed-arity/type validation at graph
construction. Do not reinterpret arbitrary MMS arithmetic as a graph.

MathGraph owns the nodes once. Operand connections are data edges, separate
from ECS parent attachments; sharing a value does not require attaching its
component to multiple parents. Lower argument-nested declarations into owned
nodes and retained port connections at construction, and serialize that wiring
without duplicating named shared producers. Reject invalid scope, port types,
missing operands and same-update cycles before execution.

Current MMS parsing/materialization already preserves nested component
expressions as builder argument values; the suite's operand fixture verifies it.
The current materializer classifies the first chained call as a constructor;
supporting an operand builder first with implicit default construction is still
an implementation requirement, not functionality established by that fixture.
Engine integration still needs to normalize those inline descriptions and live
component handles into graph-owned node/port references. The current generic
`arg_component_ref` helper accepts live handles or selectors, not arbitrary
inline component descriptions, so it is not sufficient by itself.

## Named outputs and input bindings

Use `VMNOutput` for public exports regardless of recipient. The name replaces
the earlier transform-specific output name throughout this proposal. Exporting
a value does not apply a matrix or restrict its consumers to transforms.

`VMNOutput.name("pitch").value(pitch)` exports one typed Scalar, Vec3 or
Quaternion port. Access that port as `producer.outputs.pitch.value`. A channel
bundle can instead use `.rotation(q)`, `.translation(p)` and/or `.scale(s)`;
those fields remain typed ports reusable by numeric consumers. Do not mix
`.value(...)` with channel-bundle builders on one declaration. `.name(...)`
identifies the producer's public export; names are unique within that node.

The enclosing node connects an exported port to an enclosed node's local input
with `VMN.input(local_name, producer_port)`. Inside the enclosed node,
`ValueSource.input(local_name).type(type)` reads that binding. The local name is
an arbitrary part of the recipient's interface, not a global lookup, a reserved
engine property or an inferred match with the producer's output name. Types
must match; `.type(...)` validates rather than converts values. Native AVC
assembly supplies bindings through the same typed input interface.

For example, this complete wiring fragment passes a scalar between two nodes
before turning it into a rotation for a child transform:

```mms
VMN {
    let producer = VMN {
        let pitch = ValueSource.constant(0.2)
        VMNOutput
            .name("pitch")
            .value(pitch)
    }

    let consumer = VMN
        .input("angle", producer.outputs.pitch.value)
    {
        let angle = ValueSource
            .input("angle")
            .type("scalar")
        let orientation = Quat
            .axis_angle()
            .axis([-1.0, 0.0, 0.0])
            .radians(angle)
        VMNOutput
            .rotation(orientation)
            .space("world")
    }

    VMNOutput.from(consumer.output)
    T { name = "rotation_recipient" }
}
```

The connection is `producer.outputs.pitch.value -> consumer input "angle" ->
angle node`. It is explicit even though the output and input names differ.
Bind names and validate types once, then retain slot/port connections; reads
reuse current values without per-update string lookup or MMS calls. Missing
bindings, duplicate export names, invalid ports and type mismatches fail graph
validation. Already-declared ancestor ports can also be passed directly to
inline operations when a reusable named-input boundary is unnecessary.

`node.output` selects exactly one export, whether a value or a bundle;
`node.outputs.name` selects a named export. `VMNOutput.from(export)` forwards
its ports and metadata, without copying sampled values or applying a transform.
These declarations, bindings and live member accesses remain proposed APIs.

## Transform output bundles and child transforms

The graph computes typed values. A transform output additionally specifies what
those values mean: **translation: Vec3**, **rotation: Quaternion**, and
**scale: Vec3**. A Vec3 alone cannot tell a child transform whether it supplies
translation or scale. Declare that channel explicitly inside the graph through
`VMNOutput`. Within the supported TRS channels, an explicitly
exported Quaternion can infer rotation; Vec3 must specify translation or scale.
Explicit `.rotation(...)` remains valid and clear. This inference only selects
a channel for an export: it does not make every Quaternion node an output,
choose the last node, select a transform destination, or choose coordinate space.
The pose examples keep explicit channel builders. A generic `.value(q)`
export can infer rotation when adapted to a transform. For `.value(v)` with
Vec3, optional `.channel("translation")` or `.channel("scale")` metadata selects
the transform role; ordinary numeric consumers do not require that metadata.
Automatic transform application never guesses a Vec3's role. Transform space
must also be specified when applying a value to a transform. Scalars have no direct TRS
channel mapping. Each output bundle has at most one value per channel and an explicit
coordinate space. One bundle can carry one, two, or all three channels for the
same destination basis.

The graph export interface is proposed as `graph.output` for exactly one export
and `graph.outputs.name` for an export declared with
`VMNOutput.name("name")`. `graph.output.translation`, `.rotation`
and `.scale` expose the declared live typed ports; accessing an undeclared
channel is an error. `VMNOutput.from(bundle)` forwards those ports
and their space metadata, without sampling a scene matrix or relabeling space.

A named nested `MG` is a producer/owner. Its declaration does not also apply its
output to the surrounding hierarchy: forwarding or explicit consumption selects
where application occurs. Capture already-declared ancestor ports as inputs and
validate dependencies across nested scopes. Nested ownership must not duplicate
producers, add a per-graph numeric tick or permit same-update cycles. Lower the
reachable computations together, with one input snapshot and acceptance boundary
for the owning pose update. A nested graph with ordinary content children may
also supply their basis under the same selection rules as the outer graph.

When a graph declares exactly one transform-compatible output, that export is its
implicit transform output. A `T` can be a direct child of `MathGraph`:

```mms
MathGraph {
    // position and orientation denote declared typed producer nodes/ports.
    VMNOutput
        .translation(position)
        .rotation(orientation)
        .space("world")

    T { /* inherits the graph's translation and rotation; local TRS follows */ }
}
```

Here `.translation(...)` is a builder on implicit default construction, as
required above. Numeric node descriptions and output declarations are graph
configuration/data, not transform-output content. Ordinary content children,
including T, TOP, and transparent wrappers, receive the selected basis following
existing transform-operator traversal rules. Several child branches may share
that one output basis; "one output" counts bundles, not child transforms or
numeric ports. With no transform bundle, the graph leaves the incoming basis
unchanged. Other pure numeric exports do not affect bundle selection.

Output declarations also retain typed channel ports: for example, a named
`pose_output` exposes `pose_output.translation` as a live Vec3 graph connection.
Other numeric nodes may consume it, or share its original producer, without
reading back an applied scene matrix. These reads depend on the computed value
before transform application. They remain subject to the same acyclic dependency
validation; exporting a value does not make feedback legal or add a temporal
delay. Channel-port member syntax is proposed, not an existing MMS getter API.

For several destination bundles, bind explicit output consumers. Proposed syntax:

```mms
MathGraph {
    let body_pose = VMNOutput
        .translation(body_position)
        .rotation(body_orientation)
        .space("world")
    let neck_pose = VMNOutput
        .rotation(neck_orientation)
        .space("world")

    TOP.channels(body_pose) { T { /* body branch */ } }
    TOP.channels(neck_pose) { T { /* neck branch */ } }
}
```

`TOP.channels(bundle)` is a proposed explicit adapter, not a current TOP API.
It consumes the selected bundle and supplies its basis to its own descendants;
it does not mutate a transform identified by a target query. Its first scope is
bundles owned by the enclosing graph, with retained bindings rather than
per-update selector lookup. It does not evaluate the graph again per consumer.
More than one transform-compatible export disables implicit output: unqualified transform content
must be rejected with a selection diagnostic, not silently receive the first
bundle. Node/output declarations are exempt from that content check. A named
bundle can also be selected explicitly when it is the only one.
At the graph's content boundary, explicit selection replaces implicit inheritance
for that branch and applies to the original incoming basis; it must not apply
the same local bundle twice. Carry that selection through transparent wrappers.
An inline content `MG` that explicitly forwards one selected outer bundle is
also an explicit consumer: its child T can receive the automatic adapter for
that single bundle. It starts from the outer graph's incoming basis under the
same rule. This is distinct from a named, content-free nested producer.

Initial application semantics are channel replacement in the declared `world`
or `local` space. World-space replacement patches the incoming effective world
basis. Local-space replacement patches a local TRS relative to that basis,
starting at zero translation, identity rotation and unit scale. Unspecified
channels retain the incoming basis in world mode and the identity local channel
in local mode. A downstream T then composes its authored local TRS exactly once;
the output is not a setter for that T's authored properties. The coordinate
space belongs to the whole bundle; mixing spaces requires explicit conversion
or separate stages. Additive displacement and other composition modes remain
separate operations, not inferred from nesting or value types.

A world translation-only replacement preserves the entire incoming linear
basis, including shear. World rotation/scale replacement requires a documented,
validated decomposition/recomposition policy; unsupported shear or singular
bases must report invalid output rather than silently lose information. Local
TRS composes by matrix multiplication. Define these policies before implementing
rotation/scale output. Invalid/non-finite bundles preserve the incoming basis and
report readiness/validation status; consumers must not treat that fallback as a
successfully accepted pose. Related output bundles and temporal next-state commit
need one graph-level acceptance boundary so body/neck outputs cannot partially
advance a stateful driver.

This makes a custom pose driver a typed computation plus explicit transform
channel outputs. The same numeric vocabulary can have non-transform consumers;
a math result is not inherently a pose or a complete transform matrix.

## MMS construction and native execution

The intended lifecycle is:

```text
MMS declarations or native AVC assembly
    -> authored typed node descriptions and durable input references
    -> bind inputs, validate types/dependencies, topologically order pure nodes
    -> compile a flat typed operation list + reusable value/state slots

ordinary pose update
    -> snapshot valid bound input channels
    -> execute fixed native operations using retained slots
    -> validate/accept pose outputs
    -> propagate transforms and commit next temporal state
```

MMS evaluation is appropriate for construction and live authoring edits. It is
not part of the ordinary numeric update. That update should avoid parsing,
general MMS AST evaluation, lexical environments, closure calls, dynamic host
method dispatch, repeated selector resolution and per-node ECS discovery.

The preferred runtime representation is a **flat typed operation list**: a small
native instruction enum with input/output slot indices, effectively register
bytecode. Keep the authored graph for wiring, diagnostics and serialization;
topologically sort and lower it when its structure changes. Ordinary evaluation
walks the resulting array in order, without recursive graph traversal, node
lookups, or a separately ticked ECS system for each authored component.

An illustrative internal plan, not public MMS syntax, is:

```text
snapshot bound inputs -> q_body, q_head, rig constants, measured p_head
q_relative = QuaternionRelative(q_body, q_head)
pitch      = QuaternionPitch(q_relative, anatomical_frame)
share      = ScalarMultiply(pitch, neck_share)
bend       = ScalarClamp(share, extension_limit, flexion_limit)
q_bend     = QuaternionAxisAngle(neck_axis, bend)
q_neck     = QuaternionCompose(q_body, q_rest, q_bend)
arc        = VectorRotate(q_neck, neck_to_head_rest)
... derive desktop target / retain measured XR target; measure planar departure
candidate  = PlanarFollowPropose(target, previous_anchor, range)
... positional closure and joint-limit validation
outputs    = VMNOutput(accepted_pose)  // export values; adapters apply the pose
... publish / propagate accepted channels successfully
next_anchor = PlanarFollowAccept(accepted_body_xz)
```

Each symbolic value lowers to an allocated typed slot. Sources/constants, output
bindings and explicit previous/next state are separate tables. Share a compiled
immutable plan where configuration permits, with independent value/state buffers
per avatar. Reuse these buffers; do not allocate nodes, rebuild the order or
resolve scene selectors for a normal numeric update. Store native scalar/Vec3/
quaternion payloads directly, rather than generic MMS `Value` objects. Keep a
mapping from instructions to authored components for useful errors and inspection.
The phase boundaries above remain explicit even if the first implementation uses
separate operation slices.

This needs no general bytecode VM, instruction parser, dynamic expression
language or JIT. Start with fixed native operations and typed slot validation;
packing, fusion, constant folding and incremental evaluation can follow measured
need. Native graph traversal could also avoid MMS AST/closure overhead, but its
relative speed is unmeasured. The flat plan is preferred for predictable ordering
and fewer runtime lookups, rather than a claim that graphs are inherently slow.
Benchmark equivalent native graph, flat-plan and MMS math before claiming a
speedup; include binding/rebuild costs separately from steady-state updates.

The [performance suite](../task/declarative-math-performance-suite.md) now sets
up equivalent scalar/vector/quaternion workloads, retained MMS float/array/helper
baselines, direct Rust, and benchmark-only graph/flat-plan prototypes. It checks
numerical results and warmed buffer reuse and reports construction, marshalling,
steady timing and allocation counts separately. The real MathGraph backend is
explicitly unavailable until implemented; prototype results are not its results.

Resolve source identities once and retain generation-checked bindings. Relevant
topology/configuration/binding changes trigger targeted rebind or plan rebuild.
Ordinary source value changes reuse the plan. Graph-local shared values are
explicit port dependencies; ordinary MMS getters still return copied snapshots.
Full MMS type-system changes are not a prerequisite for checking this small
fixed set of port types.

The first scope is constants, declared pose/rest inputs and explicit upstream
values. Arbitrary scene references still require the dependency and lifetime
contract in [retained sources](../task/transform-operator-source-bindings-and-invalidation.md).
Do not enlarge the existing ancestor-only translation sampler as a side effect.

## State and evaluation order

Most listed nodes are pure. Existing yaw-follow state has one existing owner.
`PlanarFollow` has explicit previous and next anchor slots:

1. Read the previous accepted anchor and current inputs.
2. Propose an anchor using the common calibrated range.
3. Run geometric closure and produce accepted output channels.
4. Publish/propagate the accepted pose and commit its anchor for the next update.

The acceptance input is a next-state dependency, not a same-update cycle that
requires iterative graph evaluation. Repeated propagation within one intended
update must not repeatedly advance temporal state. Initial binding, source/frame
changes, tracking reacquisition, rig replacement and removal have explicit
initialization/rebase/cleanup rules. Serialize configuration and durable references,
not sampled values, native plan caches or previous pose history.

Frame conventions, quaternion multiplication order, normalization, finite values
and failure behavior belong to the component contracts. Unsupported geometry or
invalid input must be diagnosed. The bridge into transforms respects effective
parent/operator boundaries and downstream local TRS; pure math nodes do not
silently mutate referenced parent/sibling transforms.

## AVC use and remaining design work

The selected initial avatar implementation is native `HeadNeckBodyConstraint`,
replacing the current body-follow constraint for supported rigs. It does not
require VMN or construct this graph. The graph is a future declarative
representation/reference consumer for the same behavior, with reusable typed
native inputs/results and numerical fixtures. VMN delivery remains independent.

In that future representation, AVC would bind rig measurements and construct
the graph automatically for supported humanoids. Both input drivers use the same calibrated follower. Desktop intent
derives a connected head arc; XR keeps measured head position authoritative.
The [constraint task](../task/xr-head-neck-bend-and-body-offset.md) owns those
consumer policies and headset/desktop acceptance.

`QuaternionFromDirections` and the scalar/vector primitives make positional
closure explicit, but the exact supported joint-limit projection is still work
to specify and verify. A list of generic nodes does not establish that every rig
or every point inside a radial threshold is reachable.

Named operand builders are the proposed wiring direction. Exact public syntax,
operation names, general port selection, graph scope, rotation/scale matrix
policies and the atomic state-acceptance hook remain open. Single-bundle child
inheritance and explicit selection for multiple bundles are the output direction
specified above, not implemented APIs. The proposal
should be assessed as a small numeric DSL with a native execution path, rather
than as an extension that evaluates arbitrary MMS expressions per frame.
