# Humanoid profile and in-memory normalization

Status: weighted rest-height fix implemented. Humanoid profiling and
profile-driven normalization are proposed.
Related: [body solver horizontal mode](body-solver-horizontal-mode.md),
[GLTF system module split](gltf-system-module-split.md), and
[GLTF profile fields and configuration](gltf-humanoid-profiling-and-normalization.md).

## Problem

Models are expected in meters with +Y up, but several systems each measure a
humanoid on their own, from different sources, and assume +Y:

| What | Where | Measures from | Produces |
|---|---|---|---|
| Bone mapping | `humanoid_bone_map_system.rs` | Bone names, arm topology | Slot → bone report, readiness flags |
| Spine rig | `ik/upper_body/rig.rs` (`SpineRig::bind`) | Mapped hips→head rest poses | Standing head height, thigh length, hip width, pitch (left) axis |
| Leg rig | `ik/lower_body/rig.rs` | Leg bone rest poses, sole offsets | Leg lengths, sole calibration |
| Avatar capsule | `avatar_control_system/capsule.rs`, `collision_shape_inference.rs` | Aggregate render bounds; head bone or `avatar_height` fallback | Capsule height and centre |
| Torso spheres | `avatar_control_system/detail_collision.rs` | Hip width | Sphere radius |
| Import height rule | `gltf_height.rs` (`default_height_if_outside`) | Mesh bounds through the bind pose, before spawn | Baked unit scale |

TDA Miku shows the failure mode. The old height estimator placed her entire mesh
using the first skin joint, which influences zero vertices in this export and
rotates its bounds onto Z. The local rig-report test logged a Y extent of
7.04 units and applied a scale of 0.2413. The weighted rest mesh is upright
and 20.39 units tall, so the applied scale gives a rest height of about 4.92 m.
A correct 1.70 m height calibration gives a scale of about 0.08336. The scene
also uses avatar-specific capsule and eye-height settings. Before 2026-11-22
Miku ran behind a 0.09 transform with head-scale, grip and camera compensations.
[Miku measurements](gltf-humanoid-profiling-and-normalization.md#tda-miku-and-vroid-comparison)

## Goal

Measure each opted-in humanoid once after its bone map resolves. Normalize the
loaded model in memory by default when profiling is enabled. Authors can keep
the measurements and disable normalization. Consumers use the profile when
available and follow their existing binding path for unprofiled imports.

## Profile

Store the profile with the bone map report for the opted-in GLTF instance.
Compute source rest measurements in the model root's frame.

- `up`: unit vector from the mid-soles (or ankles) to the head.
- `left`: from the right hip joint to the left, orthogonalized against `up`.
- `forward`: `left × up`, checked against the toes or face where mapped.
- `standing_height`: soles to the top of the head along `up`, measured using
  the selected body's weighted rest geometry.
- `eye_height`: from eye bones or a camera anchor marked as an estimate.
- `hip_width`, `shoulder_width`, `thigh_length`, `shin_length`, `arm_length`.
- Unit convention, scale estimate, and confidence from rig and asset evidence.
- `source`: which inputs it came from (joints, mesh bounds, overrides), for
  diagnostics.

Sanity checks: `up` far from +Y, or a "height" smaller than the model's
horizontal extent (wider than tall, outside a T-pose's arm span), is reported
as a lying-down or sideways rig rather than silently used.

## Normalization

The import stages uncorrected rest data and resolves the bone map before
building the profile. A `HumanoidProfile` child opts the `GLTF` into this path.
`HumanoidProfile.normalize(false)` leaves the loaded model as authored and
exposes its source report. With normalization enabled, apply supported unit,
up, and facing corrections. A requested height policy replaces the default
unit policy.

For `normalize_height_outside(min, max, target)`, use
`s = target / standing_height` when the source height interpreted in meters is
outside `[min, max]`. Apply the resulting correction before publishing the
import so all consumers bind to consistent data.

- For scale, multiply spawned node translations, `BoneRestPoseComponent`
  translations, inverse-bind translation columns, CPU vertex positions, and
  morph position deltas by `s`.
- Key corrected mesh data by the effective import transform and share textures.
- Retain the source measurements and expose effective measurements and the
  applied correction. `GLTF.import_scale()` reports the applied factor.

Remove `GLTF.default_height_if_outside` when profile-based scale normalization
ships. Migrate existing callers to the profile component in the same change.
Constants authored in GLB units (Miku's arm links, sole offsets, eye height,
equipment slots) keep multiplying by `import_scale()` for scale conversion.

## Consumers to migrate

Read the profile once the opted-in import publishes its final rest state.
Keep the existing bone-map-ready path for unprofiled avatars.

1. Avatar capsule: height and centre from `standing_height` along `up`
   (removes Miku's hand-placed capsule).
2. AVC default `avatar_height` and eye height (removes Miku's overrides).
3. `SpineRig`: `head_rest_height`, `thigh_length`, `pitch_axis` from the
   profile; keep its own chain validation.
4. Leg rig: lengths, and sole offsets where the profile can derive them.
5. Torso spheres: radius from `hip_width`.
6. `fallback_avatar_height` and `collision_shape_inference` use for avatars
   become profile reads.

## Slices

1. Complete. Weighted rest height uses decoded skin and morph data, with a
   synthetic regression for an unused first joint. The local Miku rig report
   measures 20.393234 units and applies the 0.0834 scale for the 1.70 m target.
   The Mitten and Miku equipment scene tests pass.
2. Add opt-in profiling and `model.profile()` readback. Exercise source
   measurements with `normalize(false)` on Miku, Lumae, and Mitten.
3. Apply profile-driven scale correction by default. Migrate the rooftop and
   other callers, then remove `GLTF.default_height_if_outside`.
4. Add default up and forward correction for profiled imports, with independent
   opt-outs. Validate already-corrected roots and genuinely sideways rigs.
5. Migrate consumers in the order above.

## Constraints

- Measure once per opted-in GLTF import.
- Normalization runs once before first bind.
- A scaled mesh copy only when the unscaled one is already registered; never
  duplicate textures.
- Android: the profile and normalization are CPU-side and renderer neutral.

## Validation

- Unit tests: profile from synthetic upright, sideways and T-pose rigs; scale
  math on node translations, inverse binds and mesh data (the existing
  `gltf_height` tests carry over).
- Load the same URI without a profile, with a reporting-only profile, and with
  default normalization. Verify each instance's reported and applied scale.
- Scene tests: twilight rooftop with Lumae and Mitten keep their current
  heights and capsules (profile within a few cm of today's values).
- Manual (GLB not in the repo): Miku loads at 1.7 m with a normal head,
  crouches, crawls on the roof, holds items correctly, and her capsule comes
  from the profile.

## Open question

When weighted mesh bounds are unavailable, decide whether to leave head-top
height unresolved or expose an estimate from the head bone's child or end.
