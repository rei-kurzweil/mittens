# glTF humanoid profiling and in-memory normalization

Status: proposed, 2026-11-22.

## Goal

An authored `HumanoidProfile` child opts a glTF import into profiling at load
time. Expose the rig family and bone layout, coordinate frame, and scale on the
owning `GLTFComponent`. A profiled import normalizes in memory by default, and
authors can opt out of normalization while retaining the report. Keep the
source `.gltf` or `.glb` and its referenced files read-only.

The [humanoid bone map](humanoid-bone-map-automapping-and-mms-presets.md)
already resolves anatomical slots and reports their provenance. The
[pose-basis review](../review/imported-humanoid-pose-basis-detection-and-conversion.md)
describes anatomical directions for individual joints. The existing
[`default_height_if_outside(min, max, target)`](../../src/engine/ecs/system/gltf_height.rs)
uses an imported scene's Y extent to choose a uniform baked scale. These are
the starting points for a profile that connects geometry, anatomy, and import
configuration.

## TDA Miku and VRoid comparison

Read-only inspection of the optional local TDA Miku GLB used by the
[rooftop player](../../assets/components/characters/rooftop_player.mms) gives
three different results depending on which geometry is measured.

| Measurement in the selected glTF scene | TDA Miku |
| --- | --- |
| Midpoint of the mapped foot pivots to the head pivot | About 2.9° from `+Y` |
| Rest mesh after applying each vertex's skin weights, joint transforms, and inverse binds | About 20.39 source units tall along `+Y` |
| Current first-joint transformed primitive boxes | About 7.04 units along `+Y` and 20.39 along `Z` |

The weighted rest pose is upright. The first joint's placement matrix rotates
the bounds by a quarter turn about X, while the weighted mesh stays upright.
For the rooftop's 1.70 m target, the weighted Y measurement implies a scale of
about `0.0834` (roughly 12× smaller). The current height estimator's 7.04-unit
measurement would select about `0.2413`, yielding a weighted mesh about 4.92 m
tall before any other scene adjustments. Reproduce the comparison from the
selected scene using its composed node matrices and primitive `POSITION`,
`JOINTS_n`, `WEIGHTS_n`, and inverse-bind accessors.

The Miku skin lists 293 joints. Its mapped center chain is `hips → spine →
chest → neck → head`, with three torso joints through the chest. The Bisket
GLB lists 129 skin joints and its mapped VRoid chain is `hips → spine → chest →
upper_chest → neck → head`, with four torso joints through the upper chest.
Miku has twist joints between the upper arm and forearm and between the forearm
and hand, and its leg map uses D-bones. Its right shoulder sits under the tongue
in the head branch. The rooftop's `prepare_miku_arms` patches the shoulder and
arm chains at runtime. Several joint names contain `U+FFFD` replacement
characters. The profile should show the actual joint paths and flag the damaged
names used by explicit maps.
See the [Miku bone map](../../assets/components/humanoid_bone_maps/tda_miku.mms),
[VRoid bone map](../../assets/components/humanoid_bone_maps/vroid.mms), and
[Miku arm adaptation](../../assets/components/characters/tda_miku_rig.mms).

## Bone schema

Analyze the selected scene in source model space using the imported rest pose
before startup pose overlays, scene placement, animation, or AVC. Keep these
measurements and their evidence on the owning glTF instance.

- **Family.** Report VRoid/VRM-style, MMD, Mixamo, Blender Rigify, or unknown,
  with matching evidence and confidence. Reuse the name patterns and tokenizer
  from `HumanoidBoneMap`. The current tokenizer drops non-ASCII characters, so
  also inspect original names for Japanese labels, D-bone suffixes, and
  replacement characters. Corroborate names with topology, particularly on
  damaged MMD exports.
- **Torso shape.** List and count the resolved torso joints from hips through
  the highest spine slot, and list neck and head separately. Report whether
  `upper_chest` exists and where the spine and leg branches share a parent.
  Distinguish the MMD `腰` / `下半身` split so the pelvis and lower-body branch
  are identified correctly.
- **Arm and leg chains.** Reuse resolved slots and their provenance. List and
  count mapped shoulder/arm, hand/finger, and leg joints. Report intervening
  twist and helper joints, D-bone leg chains, and unexpected parent paths such
  as Miku's right shoulder. Include missing and ambiguous slots.
- **Inventory and name encoding.** Record joint counts per skin and across
  unique joints, original source names, node indices, parents, and joint-local
  rest axes. Exclude generated anchors and identify independent armatures.
  Record affected names containing `U+FFFD` and warn when explicit mappings
  rely on them. Retain exact node identity for each resolved slot.

## Coordinate frame

- **Anatomical axes.** Estimate up from soles to head, corroborated by the
  hips-to-head chain. Use the line from the right upper-leg origin to the left
  for left, corroborated by the shoulders when available. Resolve forward using
  toes or face evidence and check it against the other axes. Record the
  landmarks used, angle to engine `+Y`, and whether up is upright, sideways,
  inverted, oblique, or unresolved.
  Mark unavailable or conflicting axes individually. Distinguish small
  rest-pose lean from a scene-axis conversion and preserve joint-local roll.
- **Mesh storage and bounds.** Record whether vertex data, mesh nodes, and the
  skeleton use different frames. Compare raw or first-joint bounds with the
  weighted mesh bounds in composed scene space. Report which bounds represent
  the avatar at rest for capsule fitting and which are approximations. Use
  valid deformed bounds or a conservative envelope for animated culling.
- **Existing axis conversion.** Report authored root transforms that account
  for an axis change, with the affected nodes and observed result. Compose
  those transforms once when measuring the anatomy. Derive any further
  correction from the composed frame.

glTF 2.0 declares meters, `+Y` up, and `+Z` forward. Store the format convention
alongside the observed frame and Mittens' normalization target of `+Y` up and
`-Z` forward. Reuse existing joint retarget bases for local anatomy when available.
[glTF 2.0 coordinate system and units](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#coordinate-system-and-units)

## Scale and proportions

- **Height.** Measure standing height from the weighted rest mesh projected
  along the detected up axis. Record the meshes used and the lower and upper
  landmarks. Distinguish the body measurement from whole-mesh bounds when
  accessories can be identified. Measure eye height from the sole reference to
  the eye midpoint. Mark any camera-anchor proxy as an estimate and leave the
  result unresolved when no usable eye reference exists.
- **Units.** Report likely meters, MMD units (about `0.08 m` per unit),
  centimeters (`0.01 m` per unit, as in a 100× FBX-derived export), or unknown,
  with confidence and evidence. Use scale metadata when available, and compare
  the measured height and rig evidence when inferring units. A target-height
  ratio is a calibration. Geometry alone cannot establish the intended
  physical height of a stylized or unusually sized model.
- **Proportions.** Report upper- and lower-limb segment lengths and hip and
  shoulder widths from mapped joints, with missing measurements identified.
  Retain measurements in file units alongside any converted values.

## GLTF configuration and load order

The proposed MMS configuration is a single `HumanoidProfile` child of `GLTF`,
alongside `HumanoidBoneMap` overrides. Expose equivalent configuration in Rust.

| Configuration | Load behavior |
| --- | --- |
| No `HumanoidProfile` child | Regular glTF import. No humanoid profile or profile-driven correction. |
| `HumanoidProfile` | Profile the source rest pose, then apply eligible corrections in memory. |
| `HumanoidProfile.normalize(false)` | Profile the source rest pose and expose the report with an identity correction. |

For a Miku import that needs a specific 1.70 m target, select the height policy
on the profile component. The other defaults still apply.

```mms
GLTF.new(uri) {
    miku_humanoid_bone_map()
    HumanoidProfile
        .normalize_height_outside(1.0, 3.0, 1.7)
}
```

- **Opt-in profiling.** The child owns the report and correction request.
  `HumanoidBoneMap` can still resolve slots for its existing consumers without
  requesting a profile. An opted-in import with no humanoid candidate can
  report `not_humanoid`. All normalization operations depend on profile data.
- **Scale correction.** `normalize_units("meters")` converts using a supported
  unit determination with sufficient confidence. This is the default scale
  policy with normalization enabled.
  `normalize_height_outside(min, max, target)` calibrates measured standing
  height to the target when the current extent interpreted in meters falls
  outside the inclusive range. Require positive finite heights and `min <= max`.
  An explicit height policy replaces the default unit policy. Reject a
  configuration that explicitly selects both. Retire
  `GLTF.default_height_if_outside` when the replacement ships. Migrate its
  authored callers, serialization, scripting bindings, docs, and tests with
  the replacement.
- **Axis correction.** `normalize_up(true)` aligns detected up to engine `+Y`.
  `normalize_forward(true)` aligns detected facing to engine `-Z` around the
  applicable up axis, after any requested up correction. Both default to
  enabled with normalization and can be disabled independently. If
  forward-only alignment cannot reach `-Z` while preserving up, report it as
  unresolved.
  Keep the model origin as the pivot for a proper rotation and positive
  uniform scale. Report ambiguous, mirrored, or degenerate inputs for any
  orientation operation that cannot be determined.
- **Reporting.** `silent` retains diagnostics in the profile. The default
  `warn` also logs implausible findings or unresolved requests once per load
  and continues the load. `fail`
  surfaces a load error for an unresolved requested correction or a humanoid
  that would remain implausible after the planned corrections. Evaluate this
  before publishing bindings. Record the outcome of each correction and retain
  diagnostics on the glTF component when load is rejected.

`model.profile()` returns a read-only MMS record after analysis and no profile
for an import that did not opt in. Proposed fields
include `status`, `family`, `bone_schema`, `axes`, `bounds`, `unit_convention`,
`unit_confidence`, `meters_per_file_unit`, `scale_applied`, `height`,
`eye_height`, `proportions`, and `diagnostics`. `height` and `eye_height` are
the measured file-unit values. Include `height_meters` and `eye_height_meters`
when the effective metric values are known. Keep unresolved measurements
absent. Serialize the authored `HumanoidProfile` settings with the scene and
recompute the report on each import.

Reuse the existing slot resolver and apply explicit map overrides to the
imported nodes. Stage the rest data and mapping before publishing
`GltfInitialized` or queuing startup poses. The current resolver normally also
reacts to `GltfInitialized`, so add a pre-publication handoff for profile and
normalization inputs. Consumers that start at that event should see one
consistent imported pose, skin, and scale. Keep the source-space profile
available after normalization and discard instance references on removal or
reload.

Measure skinned meshes with the weighted rest pose when the joints disagree
about placement. The existing first-joint bounds shortcut is suitable only
when the influencing joint matrices support it. Use the corrected measurement
in the existing height rule until the profile-based policy replaces it.
Perform the analysis at import time using CPU data, with no per-frame scan or
GPU readback. Share immutable source data where possible and key baked
variants by the effective import correction so normal and normalized loads of
the same URI can coexist.

Apply the result consistently across imported mesh geometry, skinning, morphs,
rest transforms, and any imported animation channels. Reuse the scale bake and
report its effective factor through `GLTF.import_scale()` so asset-unit
attachment offsets can convert once. Validate that bind pose, normals, and
attachments agree with the corrected frame before handing the rig to AVC,
collision, or animation. Scope this work to import and profiling. Changes to
the upper-body solver's chain requirements belong to
[body solver follow-ups](body-solver-horizontal-mode.md).

See the [implementation slices and consumer migration](humanoid-profile-and-normalization.md#slices).

## Acceptance

- Inspect the local TDA Miku asset and the checked-in Bisket asset. Confirm
  the reported rig families, rest directions, torso chains, helper paths,
  D-bones, damaged names, and measured extents. Extend the existing
  [optional Miku report](../../src/scripting/miku_rig_report.rs) for this
  comparison when implementing the feature.
- Use a checked-in synthetic skinned fixture for a sideways rest rig, a
  rotated mesh with an upright weighted rig, a composed axis-conversion root,
  and a first joint that disagrees with the weighted geometry. Verify up-axis
  detection, scale, facing, and bounds in each applicable case.
- Verify a 1.70 m target using rest mesh measurements after spawn. Check bind
  pose, a morph, and a joint attachment before and after normalization. Use
  the same URI for a regular import, a profile-only import, and an import
  normalized by default in one scene. Confirm the source asset bytes are
  identical before and after.
- Cover a meter-scale rig, a 100× export, an incomplete or ambiguous rig, and
  conflicting scale options. Verify `model.profile()` measurements, each
  reporting policy, and that `fail` prevents initialization. Verify that scene
  serialization round-trips the settings and reload rebuilds the profile.
