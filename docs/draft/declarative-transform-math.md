# Declarative transform math: rotation-derived translation

Status: design exploration, 2026-10-15. Documentation only; operation names
below are semantic notation, not an implemented MMS API.

Direction update, 2026-10-17: [declarative vector operator components](declarative-vector-operator-components.md)
names the proposed typed value nodes and native evaluation model. The numeric
graph is not limited to transform operators. MMS describes it at construction;
the engine evaluates a retained typed plan. Existing transform operators and an
explicit channel-output bridge connect it to scene transforms. This supersedes
the generic math/patch box as the proposed component decomposition below.
The value-component proposal also specifies `Vec`/`Quat` MMS operation families
and [transform output bundles](declarative-vector-operator-components.md#transform-output-bundles-and-child-transforms):
one bundle supplies the basis for graph child transforms, while multiple bundles
require explicit selection. Translation/rotation/scale are declared output roles,
separate from Vec3/Quaternion value types. These remain proposed APIs.
The [MMS pose-driver example](avc-head-neck-pose-driver-example.md) writes out
both placement slices and their nested graph/output wiring as an implementation
target, with explicit fixture limits and expected results.

[Interactive proposal diagram](../how_to/avc-head-neck-constraint-diagram.md)
shows the typed component catalog and its use in the AVC algorithm, with clickable
contracts and a separate future-example section.

## Motivation and scope

The [AVC head/neck task](../task/xr-head-neck-bend-and-body-offset.md) is to be
implemented through composed typed value components, existing transform
operators and explicit transform outputs, using avatar-specific calibration.
Calling the whole behavior an IK constraint describes its goal; it does not
require a dedicated IK solver. AVC automatically constructs the graph for
supported rigs.

The useful chain is:

```text
head rotation -> selected neck rotation -> rotated neck vector
              -> body position -> translation in an output transform
```

A quaternion supplies orientation, not a displacement magnitude. The vector
must also supply a length and rest direction. A general quaternion describes
three-dimensional orientation; extracting a scalar pitch is a separate choice.

Start with a small typed vocabulary for scalar, Vec3, quaternion, and transform
values. Keep avatar calibration and anatomical policy explicit inputs. The first
extension need not include an arbitrary script callback, a general constraint
solver, or an automatic whole-scene reactive dependency graph.

## 1. Rotation and a vector produce a displacement

The core operation is:

```text
rotate(q: Quat, v: Vec3) -> Vec3
```

For a unit quaternion this rotates `v` without changing its length. Quaternion
components are not translation coordinates. The result depends on the chosen
vector; rotating a vertical neck segment is different from rotating the
eye-to-head offset.

This operation is already implicit in ordinary transform composition:

```text
parent world pose = (p, q), unit scale
child local translation = v
child world position = p + rotate(q, v)
```

Consequently, some rotation-to-translation effects need no new operator at all.
For an aligned single-segment fixture, a local negative neck-length translation
below the head pose locates the neck base. Stripping pitch/roll *after* that
translation keeps the derived position while making downstream content upright.
Stripping rotation *before* the offset loses the arc.

Conceptual hierarchy using existing composition, not a complete avatar rig:

```text
head-pivot pose (p_head, q_head)
  T(local translation = -v)
    TOP.extract_yaw()
      T(local translation = -c)
        body content
```

Here the entire head rotation is used for the segment; `c` is the aligned
body-root-to-neck displacement. This demonstrates position derivation only.
It neither gives the neck a partial bend nor drives the existing mapped neck
bone. Real rig rest frames and existing stateful body yaw need explicit handling.

## 2. Derive either an absolute position or an offset

Use world positions/displacements below, except for the explicitly rest-local
segment vector `v`:

```text
p_head = tracked/calibrated head-pivot world position
c      = body-root-to-neck-base displacement under current upright body pose
v      = neck-to-head segment in its rest-local frame, with uniform scale applied
q_neck = desired neck world orientation, including rest-frame conversion

neck_to_head_world = rotate(q_neck, v)
p_body = p_head - c - neck_to_head_world
```

That is an **absolute replacement** of output translation. If instead an
upstream stage already calculated a correct neutral body placement, derive a
delta from the same reference:

```text
q_reference = neutral neck world orientation under the same current body yaw
p_base      = p_head - c - rotate(q_reference, v)
delta       = rotate(q_reference, v) - rotate(q_neck, v)
p_body      = p_base + delta
```

Both forms give the same answer. The reference uses the current body frame,
not the previous frame. Never repeatedly add `delta` to last frame's corrected
body position. Existing AVC placement must first be checked against this neutral
geometry; its head-X/Z-plus-height baseline is not guaranteed to include every
rig's horizontal rest offset.

For a vertical 0.12 m segment bent 25 degrees toward world forward `-Z`, with
neutral neck world rotation identity:

```text
v                       = [0, 0.12, 0]
rotate(q_neck, v)        ≈ [0, 0.10876, -0.05071]
delta                   ≈ [0, 0.01124,  0.05071]
```

The body moves backward and slightly upward relative to the neutral placement
at the same head pivot. Rotation to Vec3 explains both components. Selecting
only X/Z is possible, but discards the vertical part and generally leaves a
neck endpoint error. The XR task retains full closure and tracks leg/grounding
implications separately.

## 3. Choose the rotation independently of the position formula

There are several distinct inputs a generic math stage might accept:

| Choice | Meaning / limit |
| --- | --- |
| Use head quaternion directly | Simple landmark offset; gives the neck the full head orientation |
| `slerp(identity, relative_rotation, weight)` | Fraction of the shortest relative rotation; shares yaw/roll as well as pitch |
| Extract signed pitch, multiply a weight, clamp, construct axis-angle quaternion | Explicit pitch-only neck flexion/extension policy |

These choices are not interchangeable. For the initial XR fixture, select the
third: compute head pitch in the calibrated body/anatomical frame, apply a
weight, clamp to separate flexion/extension bounds, and construct the bend about
the calibrated axis. Compose it with neck rest orientation and current body yaw
to obtain `q_neck` in world space. Radians and quaternion `xyzw` order must be
explicit. Do not multiply quaternion components by a weight to scale an angle.

Relative orientation and composition are general primitives:

```text
relative = inverse(q_body) * q_head_canonical
q_neck   = q_body * q_neck_rest * q_bend_in_neck_rest_frame
```

`q_head_canonical` includes source-to-anatomical conversion. Define multiplication
as function composition: `rotate(a * b, v) = rotate(a, rotate(b, v))`.
Pitch extraction must have a documented axis/sign convention and supported
range. Near vertical gaze, use the already-established body frame rather than
inventing body yaw from a vanishing horizontal head-forward vector.

The avatar layer selects the bend share, limits, rest/basis conversions, and
rig eligibility. The math layer rotates vectors, combines values, and patches
transforms without knowing what an avatar is.

## 4. Patch an output transform explicitly

Distinguish three operations instead of one ambiguous translation setter:

| Operation | Semantics |
| --- | --- |
| Replace world translation | `out.position = calculated_position` |
| Add world displacement | `out.position = input.position + calculated_delta` |
| Compose a local offset | `out_matrix = input_matrix * local_offset_matrix` |

The third includes the input linear basis and scale; the second does not rotate
or scale a displacement that is already in world space. Masked replacement such
as X/Z must name world axes, preserve unselected coordinates, and expose residual
constraint error at the avatar level if exact closure is no longer possible.

A translation-only patch preserves the complete incoming linear basis, including
shear; it need not decompose and rebuild TRS. Changing orientation is a separate
output operation with explicit scale/shear support. Snapshot all input channels
before computing expressions so translation can read original head rotation even
when output rotation is replaced by body yaw.

Like current TOPs, patches produce the basis supplied to downstream children.
They do not reach backward to edit a parent T or silently mutate a referenced
sibling. Place a stage before the node it drives, and include subsequent local
TRS exactly once. Existing model-root calibration cannot be applied again after
an expression has already calculated the final root position. Wrapping an
imported neck also requires accounting for its authored local TRS; simply adding
an operator child beneath the neck does not rotate that neck bone.

## 5. Reuse one result for the neck and body branches

Body-position math can be a single-input/output operator. The complete avatar
behavior also needs the mapped neck to receive the **same** selected rotation.
Representing it as two declarations is possible if both consume the same inputs
and calibration. Shared named derived values make that relationship clearer.

Semantic dataflow, not proposed runnable MMS syntax:

```text
inputs: head_pose, current_body_yaw, rig_calibration, bend_settings

relative_head = relative_rotation(current_body_yaw, head_pose.rotation)
neck_bend    = bounded_pitch_share(relative_head, bend_settings, rig_calibration)
neck_world   = compose_neck_rotation(current_body_yaw, neck_bend, rig_calibration)
body_world_p = head_pose.position - root_to_neck_world
                                 - rotate(neck_world, neck_to_head_local)

body output: translation = body_world_p, rotation = current_body_yaw
neck output: rotation = neck_world, rest translations/scales retained
head output: existing tracked pose
```

The composite names above abbreviate the scalar/quaternion/vector operations;
they need not become avatar-specific TOP constructors. The real rig's virtual
FK path supplies `root_to_neck_world` and the segment geometry.

Prefer evaluating reusable values in an upstream scope available to descendant
branches. These are value dependencies with explicit outputs, not circular reads
between a solved body's FK neck and the head target. A shared filtered bend or
body-yaw value must have one temporal owner; two independently instantiated
filters do not become one state just because their settings match.

## 6. Thresholded X/Z following adds retained state and positional closure

Direction update, 2026-10-16: the body should also allow a bounded amount of
head movement before following in X/Z. The [XR task](../task/xr-head-neck-bend-and-body-offset.md#follow-up-bounded-xz-lean-before-body-follow)
defines the candidate follower and geometric limits.

This is a distinct composition from the initial stateless example:

```text
head pose - neutral root-to-head displacement -> neutral-relative follow target
target + previous accepted anchor + calibrated range -> planar follow candidate
candidate + tracked head + calibrated joint limits -> feasible body/neck pose
accepted planar anchor -> retained state for the next update
```

With a prescribed neck rotation, body translation has a unique answer for exact
closure. Merely inserting a deadzone after that answer breaks the closure. Neck
rotation must also depend on head/body positional separation; orientation sharing
becomes a preference subject to positional reach and joint limits. If the gap
cannot be absorbed, the body moves sooner. A larger deadzone requires sufficient
articulated reach, potentially chest/spine participation in a later extension.

The generic reusable temporal primitive is a bounded planar follower: retain an
anchor inside a radius of a moving target, moving only when required to stay in
range. Keep radius, axes/frame, optional recentering, and optional rate control
explicit. Radial X/Z distance needs vector projection/length rather than separate
scalar clamps. The avatar consumer supplies its geometric feasibility policy;
a generic follower does not know neck anatomy or guarantee a connected skeleton.

This adds a genuine state boundary beyond scalar/Vec3/Quat expressions. Give
each live follower its own anchor, initialization/reset rules, and frame
identity. Use the final feasible anchor, not an unvalidated intermediate, as next
update's state. Advance once per intended update even if transform propagation
re-evaluates consumers, and keep repeated reads of the same update stable.
If implemented as separate stages, define how the accepted result commits that
state; otherwise one composite pose owner should perform the update atomically.

An upstream simulated locomotion frame carries the anchor with it. Physical
head motion relative to that frame uses the lean allowance; teleports/recenters
must transport or reset the state explicitly. This small retained state does
not require a history of HMD samples or a walking-versus-leaning classifier.

## 7. Choose which quantity to derive from the driver contract

AVC should construct the supported behavior by default under both `Input` and
`InputXR`; authors should not need to assemble this graph themselves. Shared
math does not mean treating desktop look input as measured HMD position.

The same relation can be evaluated in either direction:

```text
root_to_head = root_to_neck + rotate(neck_world, neck_to_head_local)

XR fixed head target:     body_position = head_position - root_to_head
Desktop fixed body root: head_position = body_position + root_to_head
```

The bounded XR follow variant chooses a feasible body anchor and solves the
remaining neck geometry before accepting state. Desktop look-only input keeps
body translation fixed and derives the head's position on its arc. That derived
displacement can enter the shared follower within its calibrated rotational
allowance; do not accumulate the output as additional input on later updates.
Intentional desktop locomotion moves the anchor directly. Desktop camera position
follows the derived eye frame; the XR view continues to follow measured tracking.

This is a choice of known inputs and desired outputs, not a request for an
automatic bidirectional equation solver. Build an explicit acyclic evaluation
for each source contract from the same primitives. The public expression syntax
can be refined separately from the default AVC prototype.

## 8. Calibrate the shared range from generated rotational displacement

The source contracts above do not require different body-follow algorithms or
thresholds. Select a neutral-relative head/body displacement as the follower
input and determine the largest planar offset the rotational constraint can
produce with its body anchor fixed. Calibrate one per-rig range containing that
entire generated envelope and use it for either driver.

```text
d0         = neutral root-to-head displacement in the current body frame
e_total    = (desired_head_position - retained_body_anchor - d0).xz
R_rotation = max(length(e_total)) over supported rotation-only outputs
R_follow   >= R_rotation
```

Desktop-generated head motion stays within this envelope from the neutral
anchor. XR adds measured position changes that can exceed it. The follower is
source-agnostic: identical pose/state/configuration produce identical results.
Source adapters normalize their pose inputs, not their follow policies.

Add a calibration/validation step to the operator composition: measure the
generated range in the same frame, units, target point, and output stage used by
the follow test. Rotation limits, eye offsets, scale, or rest-path changes
invalidate it. An analytic bound is preferable where available; combined-pose
sweeps validate the actual fixture. A few sampled angles alone do not prove the
maximum of an arbitrary expression graph.

The geometric pose stage must still validate closure; enclosing a motion
envelope in a circle does not make every point inside the circle anatomically
reachable. Reserve reach for a physical margin, and avoid counting generated
rotation twice. If the pipeline instead subtracts the expected rotational
offset, calibrate the remaining threshold as residual lean; do not reuse the
total-range interpretation without adjustment. After XR has already leaned to
the edge of a total range, rotation can initiate follow. Preventing that in all
states needs residual measurement or an orientation-dependent region, not a
different threshold for desktop.

## 9. Declarative relationships differ from ordinary MMS reads

[Transform accessors](../../crates/meow-meow-script/docs/draft/transform-component-accessors.md)
copy values when evaluated. An expression such as `head.world.rotation()` does
not establish a retained relationship. Adding `Math.rotate(q, v)` as an ordinary
function alone would not make an authored body follow live head motion.

The current host's `Math` dispatch has scalar trigonometry/clamp and dot/cross
helpers; TOP channel lists have a fixed operation vocabulary. Neither currently
provides this retained cross-channel expression surface.

A declarative expression should be an explicit typed value/program descriptor:
construction builds a bounded operation list or acyclic value graph; runtime
evaluation reads live bound inputs during transform propagation. Constants remain
constants, referenced inputs remain references, and copied snapshots remain
snapshots. Exact constructor names and `let` syntax are still open; do not silently
reinterpret ordinary MMS arithmetic or getters as live expressions.

Minimum vocabulary to assess before adding more:

- Scalar/Vec3/Quat constants and explicit input-channel reads.
- Vec3 addition/subtraction, scalar multiplication, and quaternion-vector rotation.
- Quaternion composition/inverse and axis-angle construction; signed pitch
  extraction, scalar weighting, and clamping for the bounded neck example.
- Explicit output translation replacement/addition and output rotation selection.
- Reusable scoped derived values when both neck/body outputs are authored together.

Normalize valid nonzero quaternions at a defined boundary; reject non-finite or
zero-length inputs rather than producing NaNs. Invalid expressions preserve input
and expose invalid status; avatar readiness must honor that status. Validate
operation types at construction without requiring a new general MMS type system.
Keep runtime evaluation free of script callbacks and repeated selector resolution.

Initially restrict live inputs to the incoming basis, available ancestor inputs,
and explicit upstream derived values. If arbitrary external source references
are added, they require binding lifetime, invalidation, source-before-consumer
ordering, and cycle handling described in
[retained operator sources](../task/transform-operator-source-bindings-and-invalidation.md).
Do not expand the existing ancestor-only translation sampler implicitly.

## Delivery and checks

1. Prove the fixed-vector/full-rotation example with existing hierarchy
   composition and record which desired controls it cannot express.
2. Specify the small typed expression/value vocabulary and single-output patch
   contract. Add numeric fixtures for `q -> rotate(q, v) -> translation`, absolute
   versus delta equivalence, input/output rotation separation, world/local offset
   distinction, parent transforms, supported scale, and repeated evaluation.
3. Specify scoped shared values and output placement for the neck/body fixture.
   Advance temporal stages once per intended time step even if propagation visits
   them repeatedly. Preserve independent state for separate avatars.
4. Bind rig measurements/bases through AVC, validate connected neck geometry and
   propagation before arm/leg solves, and run default-AVC desktop and XR fixtures.
   Desktop look must leave body translation fixed while moving the head along
   the neck arc; desktop movement must move the complete avatar.
5. Add the thresholded X/Z fixture as a separate increment: a retained planar
   follower with one calibrated rotational allowance for both sources, plus
   position-responsive neck closure. Test the finite reach bound,
   diagonal motion, state commit/reset/frame changes, and propagation cadence.
   The initial rotation-only fixture remains independently testable.

Serialization must retain expression operations, constants, and durable source
references, not sampled values or runtime caches. Rebinding/configuration changes,
deletion, invalid inputs/recovery, output-local offset order, and shared-value
identity are part of verification. Exact authoring syntax, expression storage,
and whether reusable math is a new TOP kind or a common value facility remain
design decisions for the next iteration.
