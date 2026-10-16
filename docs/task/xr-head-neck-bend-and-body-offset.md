# Task: AVC head/neck bend, body offset, and bounded follow

## Status and outcome

Proposed, 2026-10-15; feasibility review and implementation task only. No
runtime changes have been made for this task.

Scope update, 2026-10-16: this is intended as automatic behavior of `AVC {}` for
supported humanoids under both `Input {}` and `InputXR {}`. The original filename
reflects the XR repro, not an XR-only activation rule. The desktop contract below
supersedes earlier suggestions to leave desktop on its existing rigid-head path.

Design direction update: [declarative transform math](../draft/declarative-transform-math.md)
breaks this behavior into rotation selection, quaternion/vector math, translation
patching, and shared derived values. Implement the constraint through composed
transform operators, automatically wired by AVC. Small shared math helpers may
back the operator primitives; a separate dedicated avatar IK solver is not the
selected implementation. Existing TOP composition already expresses part of the
geometry; retained math/follow capabilities still need to be added.

When looking down at their torso or legs in XR, the user should see a connected
neck bending with the head, with the body placed behind the head as required by
that bend. The tracked view and visible head remain authoritative. Body position
is derived from the articulated neck geometry, rather than an arbitrary
pitch-to-backward-translation multiplier.

This is feasible as a small bounded kinematic constraint. It does not require
motion classification, pose history, or a full-body IK solver for the initial
rotation-derived placement. The 2026-10-16 follow-up below adds thresholded X/Z
following with a retained body anchor and a position-driven neck constraint.
The exact bend share and limits need headset evaluation on Bisket and Rei(mu).

## Default AVC behavior and shared follow policy

No extra component, authored TOP graph, or opt-in flag should be required for
the normal mapped-humanoid case. AVC binds calibration and constructs/owns the
necessary math, follow state, and pose outputs automatically. Expose tuning and
an explicit opt-out, but the intended shipped default is enabled whenever its
rig prerequisites validate. Missing neck/unsupported rigs retain diagnosed
fallback behavior without breaking head control. Desktop needs no XR component
or XR runtime to activate the supported head/neck behavior.

Use the same operator-based constraint and calibrated body-follow threshold for
both drivers. Driver-specific source preparation supplies the desired head pose;
it does not select a different follow algorithm or a desktop bypass.

Share the geometry while making the source's authority explicit:

| Driver | Authoritative inputs | Derived output / body-follow trigger |
| --- | --- | --- |
| `InputXR` | Valid measured head position and orientation, composed with locomotion | Apply the common calibrated range and geometric closure; excess displacement moves the body |
| `Input` | Desired look orientation and intentional locomotion translation; head position on the connected rotation arc | Apply the same range and closure; the generated rotation-only offset fits inside the calibrated allowance |

For desktop with no locomotion, hold body-root translation fixed through the
supported look range. The neck receives its bounded rotation share, the head
receives the remaining orientation, and head/eye positions follow the resulting
FK geometry. The head can therefore move forward/sideways relative to the body
and need not remain vertically above it in world space. This positional movement
is a result of rotating the connected joints. It may be presented to the common
follower: calibration must ensure this rotation-only displacement cannot cross
its boundary from the neutral anchor. Do not accumulate that generated output
as new input next frame. Body yaw-follow may still turn the body; it must not
introduce root translation as a side effect of looking.

For the simple segment fixture, compare the two uses of the same geometry:

```text
XR:      p_body = measured_head_p - c - rotate(q_neck, v)
         or choose a feasible retained body anchor and solve q_neck for closure

Desktop: p_body = calibrated_locomotion_body_anchor
         p_head = p_body + c + rotate(q_neck, v)
         p_eye  = p_head + rotate(q_head, calibrated_head_to_eye)
```

Use the real rest path, basis conversions, and supported scale in both modes.
`p_body` remains fixed for desktop look-only input in XYZ, not just X/Z; the
head can change height on its arc. Desktop movement commands translate the body
anchor directly, carrying the connected pose without consuming a lean deadzone.
Keep existing scene calibration/locomotion-origin conventions instead of treating
every desktop input translation as an HMD/eye position or redefining its origin.

The source adapter and the common threshold solve different parts of the task.
The current unconditional head-under-driver mount makes the desktop head pivot
follow a fixed-eye-style target. Merely enlarging its threshold does not create
the desired connected desktop head arc; source preparation must supply it.
Derive desktop head position from the body/neck geometry before publishing the
head pose; either adapt the existing mount or use a compatible FK arrangement.
Do not let head maintenance overwrite the derived result or feed camera/head
output back into the input's locomotion state. The mapped Camera3D follows that
derived head/eye frame. XR rendered eye poses keep their existing authoritative
tracking path and receive no synthetic neck-arc offset.

`Input` itself does not promise anatomical limits: FPS pitch currently clamps
to +/-1.55 radians, while relative/flight rotation can continue through complete
turns. Do not change generic Input behavior to solve this AVC feature. Bound the
neck contribution and leave remaining requested orientation on the head for the
initial desktop fixture; stricter anatomical head-angle clamping would be an
explicit separate policy. Ordinary look, including reaching the neck bound,
must not make the body translate to satisfy the XR positional constraint.

Changing between desktop and XR rebinds source authority and initializes the
appropriate anchor state from the current calibrated pose. Never reuse a
desktop-derived head position as if it were a measured XR sample.

## Calibrate one follow threshold from the rotational motion envelope

Direction update, 2026-10-16: the allowed relative displacement generated by
the rotational constraint provides a principled minimum for the common follow
range. The preceding desktop/XR distinction concerns pose inputs, not separate
body-follow policies. This replaces the earlier implication that desktop must
skip positional follow to keep look-only motion from translating the body.

Measure a consistent anatomical point and subtract its neutral offset. For a
head-pivot target in the locomotion frame:

```text
b             = retained body anchor
d0            = neutral body-root-to-head displacement under current body yaw
relative_xz   = (head_position - b - d0).xz
rotation_only(q) = relative_xz generated with fixed b and no positional input

R_rotation = max(length(rotation_only(q))) over all supported rotation states
R_follow   >= R_rotation
```

`R_follow` is the same calibrated value for this avatar/configuration regardless
of driver type. The maximum is over the complete supported rotation composition,
including simultaneous axes, rest frames, eye/head conversion if it affects the
chosen target point, and configured uniform scale. It is not the absolute
distance from feet/root to head, nor just one nod from a neutral sample.

For the simple vertical neck segment with pitch share capped at 25 degrees,
12 cm length gives about 5.1 cm maximum planar rotational displacement. Keep
these as explanatory measurements, not a universal threshold. Derive bounds
from the permitted operations/ranges where possible; use pose sweeps including
combined rotations to validate them on the actual rig. Recompute calibration
when rig, scale, geometry, or rotation limits change.

Pure desktop look generated by that same calibrated constraint remains inside
the allowance. XR head movement can also remain inside it; additional measured
translation can cross it and cause follow. No source-type branch is needed in
the follower. Intentional simulated locomotion transports the anchor frame and
continues to move the avatar immediately in either mode.

Containment of generated rotational motion and anatomical reach are separate
checks. The allowed range must contain the generated envelope, while its accepted
poses must still close the neck/head geometry under the selected limits. If
there is no reach beyond the maximum preferred rotation, there is no guaranteed
extra lean margin in that direction. Reserve bend/reach for any physical margin;
use numerical comparison tolerance at the boundary rather than permitting an
unreachable gap. A conservative circle can include unreachable off-axis points,
so keep the geometric validation described below even with a radial threshold.

Choose one threshold measurement and keep its calibration consistent. The
formula above measures total departure from neutral geometry. If instead a
stage subtracts the expected rotation-induced displacement first, its output is
a residual and its threshold means additional lean. Do not subtract that motion
and also charge it against the same total-distance budget. The first shared
threshold fixture uses the total neutral-relative measure above.

This guarantees rotation-only containment from the neutral anchor. If XR has
already leaned near the boundary, further rotation can cross a total-distance
threshold without additional physical translation. That is a consequence of
using one total range, not a desktop/XR difference. Test this explicitly. A
stronger rule that rotations never initiate follow at any existing lean would
require rotation-compensated residual measurement or an orientation-dependent
allowed region; it is not implied by increasing one scalar radius.

## Existing documentation and actual implementation

- [IK system](../spec/ik-system.md) describes AimConstraint, TwoBoneIK, and
  FABRIK. Parts of its AVC wiring and frame-order descriptions are historical.
- [Simple humanoid body follow](avatar-control-simple-humanoid-body-follow.md)
  establishes the head-driven body baseline and rotation-only neck policy.
- [HMD-center versus head-pivot bug](../bugs/avatar-body-follow-targets-hmd-center-instead-of-head-pivot-xz.md)
  records an earlier anchor mismatch. Its raw-HMD-target description is not
  the current code: body follow already targets the displaced head pivot.
- [Transform operators](../spec/transform-operators.md) describes current TOP
  composition and retained temporal state.

Checked implementation surfaces:

- [AVC initialization and head maintenance](../../src/engine/ecs/system/avatar_control_system.rs):
  creates `driven_t -> head_mount -> displaced_head`, restores head rest
  rotation, and keeps the displaced head's local translation zero. AVC does
  not install a head AimConstraint. The head is outside the body's FK branch.
- [HeadPoseBodyXzFollowSystem](../../src/engine/ecs/system/ik/head_pose_body_xz_follow.rs):
  sets model-root X/Z to displaced-head world X/Z and Y to head Y plus a
  cached height offset. It preserves body yaw and pins neck local translation
  to rest. That pin does not calculate neck rotation or enforce endpoint
  closure between the separated branches.
- [IK solvers](../../src/engine/ecs/system/ik_system.rs) and
  [configuration](../../src/engine/ecs/component/ik_chain.rs): existing
  AimConstraint matches one joint's pose; TwoBoneIK solves a positional
  three-joint chain. Neither expresses this distributed orientation and
  floating body-root constraint. FABRIK follows a connected positional chain
  and contains an older small-neck-bend clamp; it is not a ready-made
  anatomical neck constraint for the displaced-head topology.
- [Frame scheduling](../../src/engine/ecs/system/system_world.rs): AVC writes
  flush before body follow. Body follow and IK currently tick before their
  shared flush; final secondary motion and skin refresh follow. A neck/body
  constraint that changes shoulder placement needs an explicit propagation
  boundary before arm IK reads those shoulders.

This explains a plausible missing behavior, not a headset-confirmed diagnosis.
An incorrect camera/eye-to-head calibration can produce a similar symptom.
Inspect both before tuning a neck bend to hide an offset error.

## Geometry and ownership

Keep head position and orientation derived from the same current XR pose and
calibrated eye-to-head convention as today. Do not add an offset to the rendered
XR view, tracking origin, or locomotion root to create this effect.

Distinguish three landmarks: HMD/eye origin, head-bone pivot at the skull base,
and neck base on the upper torso. Rotating the eye-to-head offset finds the
head pivot; it does not also solve the neck-base location once the neck bends.

For the first supported fixture, leave the upper torso upright and choose a
bounded fraction of head pitch as neck flexion. Compute pitch in a calibrated
anatomical frame relative to the current yaw-only body frame, accounting for
the rig's rest rotations. Avoid raw imported Euler axes. Retain the head's
full world orientation; the remaining head-versus-neck rotation is whatever
is needed to preserve that orientation. Begin with pitch sharing; separate
roll/yaw sharing and chest bending can follow.

Evaluate a virtual copy of the original root-to-head FK path using immutable
rest transforms, current body yaw, and the chosen neck rotation. Include
intermediate helper transforms. The displaced head remains the independent
target, so this virtual path must be retained before splicing changes topology.

In world units:

```text
P_head = desired head-pivot world position from tracking and calibration
d      = virtual root-to-head displacement with selected neck bend/body yaw
P_root = P_head - d

P_virtual_head = P_root + d = P_head
```

For a simple neck with fixed root-to-neck displacement `c`, neck-world
orientation `Q_neck`, and rest-local neck-to-head vector `v`:

```text
d = c + rotate(Q_neck, v)
```

Use the complete FK path for real rigs and include supported scale. Keep neck
local translation, segment lengths, and scale at their baseline values.
Apply rotation relative to the calibrated rest frame, never relative to last
frame's corrected rotation. The head remains on its existing tracking mount;
the constraint makes the virtual neck endpoint meet that separate head pivot.

As a geometric example, a vertical 12 cm segment bent forward by 25 degrees
advances its endpoint about 5.1 cm and lowers it about 1.1 cm. With head pivot
held fixed, the body moves back about 5.1 cm and up about 1.1 cm. These are
illustrative dimensions and angles, not selected human or avatar defaults.

The small Y correction matters: an X/Z-only offset with the old fixed height
cannot in general preserve both neck length and endpoint closure. Select the
full derived model-root translation for the first constraint fixture. Feed
the resulting body pose into subsequent leg/crouch evaluation; keep simulated
grounding Y independent of head tracking. Standing feet may need leg correction
or may lift slightly, so test this alongside
[crouch IK](avc-foot-penetration-crouch-ik.md) before enabling it by default.
Foot planting or zero sole error is not guaranteed by a neck solve alone.

Do not stack a second backward-offset rule on top of existing body follow.
For an eligible avatar this constraint replaces that model-root translation
write, deriving the complete root position from the shared head target and
virtual chain. Ineligible avatars retain the current behavior.

## Follow-up: bounded X/Z lean before body follow

Direction update, 2026-10-16: rotation-derived placement is the first fixture,
not the complete desired body behavior. Allow some head/body displacement in
X/Z, then move the body once that separation exceeds its permitted range.
Keep the head tracked and close the neck geometry throughout.

The earlier [body-follow attempt](avatar-control-simple-humanoid-body-follow.md)
reported rotation-induced HMD motion dragging the body and a stretching neck.
The [older alignment analysis](../analysis/cameraxr-avatar-alignment.md#13-the-ik-problem-and-why-it-surfaces-with-translationfollow)
also explicitly identified an unsolved head/neck gap when body position lags.
Sharing head rotation with the neck could address part of this failure, but
the available notes do not establish it as the sole cause. Keep anchor
calibration, positional closure, and update order in the investigation.

Importantly, `P_root = P_head - d(chosen_neck_rotation)` still follows head
translation 1:1 when orientation and the chosen neck rotation are unchanged.
It creates an anatomical offset, not a region in which the body stays put.
Allowing that region requires neck rotation to respond to positional error as
well as the preferred share of head rotation. An orientation-only neck driver
cannot close an arbitrary positional gap.

Separate the pieces:

1. **Shared measurement:** the current desired head pivot and neutral virtual
   root-to-head displacement calculate the target `a = head_position - d0`.
   The neutral-relative displacement includes the bounded rotational arc used
   to calibrate the common range. The chosen neck rotation remains a preference
   for geometric closure, not an independently applied second translation.
2. **Retained planar anchor:** keep the previous accepted body X/Z `b` in an
   explicit locomotion frame. Small movement of `a` can leave `b` unchanged.
3. **Comfort bound:** move `b` when the neutral-relative target is too far away.
   Begin with a radial X/Z bound; independently clamping X and Z permits a
   larger diagonal displacement. Directional/body-relative bounds can follow.
4. **Geometric closure:** solve neck orientation from the actual head-to-body
   separation, using the rotation-share result as a preference. Enforce neck
   limits and the residual head-versus-neck orientation limits; preserve the
   tracked head's full pose. An unsupported target makes the body follow sooner.

An illustrative planar follower, before anatomical validation, is:

```text
a = (head_position - neutral_root_to_head_displacement).xz
b = previous_accepted_body_anchor.xz
e = a - b

b_candidate = b                              if length(e) <= radius
b_candidate = a - radius * e / length(e)     otherwise
```

This moves only the amount needed to remain at the permitted boundary. It is
not a speed-limited chase or a perpetual easing toward the head. At zero radius
it becomes immediate neutral-anchor follow (disabling the rotational allowance).
After a lean and return, the body may remain at its newly accepted anchor;
automatic recentering would be another
explicit policy, not an accidental consequence of smoothing.

The comfort radius is not a guarantee of reachability. Validate the candidate
against the current chain, bend limits, and available head/neck orientation
range. If it cannot close, move the body to a feasible position and solve the
neck against that final position. A short neck permits only a small lean; a
12 cm segment with a 25-degree bend has about 5.1 cm horizontal reach from
vertical, and a preferred bend already consumes some of that range. Do not
select a generic 10 cm deadzone without measuring the rig and directional
limits. Larger torso lean requires an explicit chest/spine extension or earlier
body translation; it cannot be supplied by neck stretching.

Body Y remains derived from the final connected chain, not independently
frozen by the planar follower. Even a reachable horizontal offset can need a
vertical correction. If a standing/foot constraint later fixes Y, re-evaluate
feasibility jointly; the planar bound alone no longer proves closure.

The hard reach/bend bound takes priority over a cosmetic follow-rate cap. Fast
physical motion or a teleport must not leave the tracked head detached while
the body slowly catches up. Init/rebind/tracking reacquisition establish a
current feasible anchor; removal clears state. Locomotion-frame motion carries
the retained anchor with it, so simulated walking/turning is not delayed by this
lean allowance. Define teleport/recenter frame changes explicitly. One HMD does
not identify whether room-scale translation was leaning or stepping; this is a
bounded pose policy, not inferred foot planting.

Implement after the rotation-only geometry fixture. Add captures/tests for
translation with fixed head orientation, small sway, threshold crossing in both
directions and diagonally, holding a lean, returning, and looking down while
leaning. Record neutral-relative target, retained anchor, final neck rotation, body Y,
endpoint residual, and the reason for early follow. Repeat under different frame
rates, fast motion, frame/teleport changes, tracking loss, and avatar replacement.

## State and transform-operator feasibility

| Concern | Required state / existing fit |
| --- | --- |
| Initial rotation-derived placement | Current tracked head pose, current body yaw, rig calibration and limits; deterministic without pose history |
| Thresholded X/Z body follow | Previous accepted planar anchor and explicit locomotion frame; deterministic per update with retained state |
| Neck pose during allowed lean | Positional closure and joint limits in addition to preferred rotation sharing; use the final accepted anchor |
| Rig binding | Per-instance IDs, original FK path, immutable rest transforms, anatomical basis, supported scale, and map generation |
| Existing body yaw | Already stateful in `TOP.yaw_follow`; consume its current result |
| Optional smoothing | Retained per-avatar filter values, reset on rebind/tracking transitions; not needed for the first fixture |
| Walking versus looking down | No motion classifier required; one HMD pose does not uniquely identify actual torso/neck posture |

Existing TOPs can express a fixed local landmark offset followed by yaw-only
output. They already support temporal state, so statefulness itself does not
rule out using operators. However, current fork channels are independent;
there is no operation that distributes head orientation into a neck joint and
derives a second body output from the resulting FK geometry. An ordinary child
operator also cannot update its parent or sibling neck branch.

Use the [declarative math decomposition](../draft/declarative-transform-math.md)
to implement the constraint through transform operators. The body calculation
can be a single-output stage; shared upstream values supply both it and neck
rotation. Current TOPs need the retained math and stateful-follow extension to
express the full behavior. AVC binds calibration/eligibility and owns the
generated composition; backend math helpers should implement reusable primitives
without introducing an alternative standalone avatar solver.
Current translation sampling is restricted to structural ancestor sources and
cannot simply sample a solved sibling branch. No general transform constraint
graph is a prerequisite for the first slice.

The thresholded follow-up also needs an explicit stateful planar-follow stage
and geometric feasibility/closure. Those are separate from pure quaternion/vector
math. Statefulness is compatible with TOPs, as yaw follow demonstrates, but
state advances must be bounded to one intended update. The final pose must be
validated before it is published to neck/body consumers; two independent
expression branches with incompatible targets do not enforce a constraint.

## Implementation slices

1. **Inspect calibration and reproduce.** On
   [Bisket VR](../../examples/bisket-vr-demo.mms) and
   [Rei(mu)](../../examples/rei(mu).mms), visualize/log HMD origin, calibrated
   head pivot, neck base, virtual neck endpoint, body root, relative head pitch,
   chosen neck pitch, and endpoint residual. Verify eye/head offset and forward
   axes before comparing constraint enabled/disabled.
2. **Bind and calculate.** Use the owning GLTF's retained HumanoidBoneMap report
   for Head and Neck plus the original path to model root. Derive rest geometry
   from BoneRestPose data. Validate finite nonzero neck length and supported
   hierarchy/scale; initially support uniform scale. Missing/ambiguous maps,
   unsupported rigs, or missing calibration skip with a retained reason.
   Preserve the original pre-splice path rather than walking toward the head
   in the already-reparented runtime tree.
3. **Deterministic fixture.** Implement pitch sharing with configurable weight
   and separate flexion/extension bounds; exact defaults/API remain TBD.
   Saturate neck flexion while the head retains the rest of its tracked
   rotation. Test the virtual endpoint and derived body translation together.
4. **Integrate ownership and scheduling.** Enable only for the avatar's active,
   usable pose driver by default. XR waits for valid tracking; desktop uses the
   look/locomotion authority contract above and does not need XR readiness.
   Invalid/unbound rigs keep their diagnosed fallback.
   Make one module own neck rotation and body translation; compose the neck
   rest-position pin with it. For the first fixture the neck has one pose owner;
   future animation supplies an explicit base pose instead of racing writes.
   Flush and propagate neck/body results before arm IK and future leg target
   generation, then propagate solves before secondary motion and skinning.
   Do not depend on IK chain HashSet order.
5. **Lifecycle and headset checkpoint.** Provide enable/disable and bend tuning,
   restore the prior base pose on disable, invalidate/rebuild on rig/map/basis
   replacement, and clean up per-instance state on removal. Invalid tracking
   suspends solving; reacquisition evaluates the current pose without replaying
   accumulated correction. Suspend for mounted/seated modes until their body
   ownership is defined. Default activation is part of the implementation, not
   an author opt-in; validate both desktop and headset fixtures before shipping.

The model-root update must use the current transform propagation contract
through operator boundaries. Existing body-follow comments describe an older
cached-world workaround; verify current behavior rather than copying that
workaround into a second writer.

## Acceptance and meaningful verification

- Holding an input produces a stable accepted neck/body pose with no accumulation,
  drift, oscillation, or feedback from previous corrected FK. The initial pure
  geometry fixture is stateless; bounded follow intentionally retains its anchor.
- Initial calibrated neutral pose reproduces rest geometry, including nonzero authored
  horizontal offsets. Virtual neck endpoint and tracked head pivot agree within
  a stated numeric tolerance; neck lengths/translations/scales stay fixed.
- Pitch down through a supported sweep (for example 0, 30, 60, 80 degrees):
  the neck shares only its bounded amount, head/view remain tracked, and body
  offset follows the computed arc. Looking at torso and legs feels connected
  in the headset. Compare with actual captured motion, not a requirement that
  every physical nod hold the HMD pivot fixed.
- Upward pitch, yaw, roll, and combined rotations remain finite and continuous.
  Near vertical gaze, retain a stable body/anatomical frame rather than
  extracting yaw from a vanishing horizontal forward vector.
- Translating the whole tracked pose at fixed orientation translates the solved
  body by the same amount in the immediate-follow fixture. In bounded-follow
  mode, translation inside the allowed lean range leaves the retained body
  anchor in place; beyond it the body follows to maintain a feasible pose.
  Rotated parents, both avatar forward conventions,
  nonidentity rest rotations, uniform scale, and renamed bones remain correct.
- Confirm current shoulders are used by arm IK and future crouch targets use
  the corrected body pose in the same frame. Inspect sole motion from the Y
  correction, and verify head pitch cannot drive simulated capsule Y or move
  the rendered XR origin.
- Missing neck, unsupported scale, tracking loss/reacquisition, disable/re-enable,
  avatar replacement, and desktop mode behave according to the lifecycle above.
- Plain `AVC {}` binds supported rigs automatically under either driver. A
  desktop-only scene exercises the feature without initializing XR. No authored
  operators or additional constraint component are needed for default behavior.
- Run the same calibrated source-pose sequence through the shared follow stage
  independent of driver type: identical poses, initial anchor, and configuration
  produce identical accepted anchors. Sweep the complete generated rotational
  envelope without moving the neutral anchor, then add positional excess and
  verify follow begins at the same threshold in both cases. Exercise XR rotation
  after leaning near the boundary and record total-range behavior explicitly.
- With desktop translation input disabled, sweep look pitch/yaw/roll and combined
  rotations, hold at neck limits, return, and repeat: body-root XYZ remains fixed
  within a stated numeric tolerance while head position follows the connected
  arc. Include a nonzero-X/Z head-to-body offset and check the mapped camera
  follows the derived eye frame. Existing body-yaw follow may change yaw.
- Desktop WASD/other intended locomotion still moves the complete avatar;
  look-only motion never changes retained locomotion translation. Test source
  switching without inherited XR/desktop anchor state or feedback drift.

## Implementation readiness

Ready for a focused prototype with the source-authority contract above, shared
rig geometry, and existing binding/propagation infrastructure. No further
product decision is required to start that prototype. First prove default
desktop rotation with a fixed body anchor, then rotation-derived XR placement,
then bounded XR positional follow and geometric closure.

Use internal typed operator composition with explicit state acceptance; the final public
declarative expression syntax is not settled and need not be invented merely
to run the first fixture. Keep the pure math reusable by the proposed operator
facility, and keep the stateful follow/acceptance boundary explicit. Exact bend
shares, anatomical limits, and feasible lean margins are measured/tuned during
implementation on supported rigs, with values recorded in the fixtures. They
are not yet validated shipping defaults. The generic declarative math facility
remains a separate API design deliverable if included in the same implementation.

Pure geometry tests and a small integration fixture should establish closure,
frame ordering, and ownership before the headset checkpoint. A plausible
skeleton result alone does not prove skin deformation or first-person comfort.

## Related work and boundaries

- [VR embodiment epic](epic/vr-avatar-embodiment.md): integration tracker.
- [Avatar camera anchoring](../spec/avatar-camera.md): camera calibration and
  tracking-view ownership.
- [Avatar pose layers](avatar-pose-transition-layers.md): later animation
  composition; not required for the initial single-owner neck fixture.
- [Joint basis retargeting](../spec/joint-basis-retargeting-system.md): anatomical
  frame conventions; inspect available runtime basis support during binding.

This task extends the simpler body-follow baseline with bounded neck flexion;
it does not revive spine FABRIK, infer whole-body posture from one headset,
change eye tracking, or implement walking/foot planting.
