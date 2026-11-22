# Humanoid profile and in-memory normalization

Status: proposed 2026-11-22. Not started.
Related: [body solver horizontal mode](body-solver-horizontal-mode.md),
[GLTF system module split](gltf-system-module-split.md).

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

TDA Miku shows the failure mode. Her skinned meshes store their bounds
sideways, so the render-bounds capsule is wrong (she gets a hand-placed one),
the import height rule measured her depth instead of her height and never
fired (she loaded about 20 m tall), and she needs hand-set `avatar_height` and
eye height. Before 2026-11-22 she also ran behind a 0.09 transform with
head-scale, grip and camera compensations.

## Goal

Measure each humanoid once, after its bone map resolves, into a **humanoid
profile**. Optionally normalize the loaded model in memory from that profile.
Then have every consumer bind to the profile instead of measuring for itself.

## Profile

Stored on the bone map report (no new system), computed once per GLTF
instance and bone map generation, in the model root's frame:

- `up`: unit vector from the mid-soles (or ankles) to the head.
- `left`: from the right hip joint to the left, orthogonalized against `up`.
- `forward`: `left × up`, checked against the toes or face where mapped.
- `standing_height`: soles to the top of the head along `up`. Joint positions
  give soles→head joint; add the head's extent from mesh bounds where usable,
  else a fixed head-top ratio.
- `eye_height`: from eye bones, the camera anchor slot, or a ratio of the
  head joint.
- `hip_width`, `shoulder_width`, `thigh_length`, `shin_length`, `arm_length`.
- `units_plausible`: standing height within a human range (0.5–3 m).
- `source`: which inputs it came from (joints, mesh bounds, overrides), for
  diagnostics.

Sanity checks: `up` far from +Y, or a "height" smaller than the model's
horizontal extent (wider than tall, outside a T-pose's arm span), is reported
as a lying-down or sideways rig rather than silently used.

## Normalization

`GLTF.default_height_if_outside(min, max, target)` moves from import time to
after the profile:

1. The GLB imports and spawns unscaled.
2. The bone map resolves, then the profile is measured.
3. If the profile's standing height is outside `[min, max]`, scale by
   `s = target / standing_height` in place, before anything binds:
   - spawned node translations and their `BoneRestPoseComponent`s × `s`
   - skin inverse binds: re-register with translation columns × `s`
   - CPU meshes (positions, morph position deltas) × `s` if not yet
     registered; otherwise register a scaled copy under its own key and point
     this instance's renderables at it (the current `@x<scale>` keying)
   - the profile itself
4. Emit a profile-ready signal. `GLTF.import_scale()` reports `s`.

Optional later: `GLTF.normalize_up()` rotates a sideways rig upright using
`up`, with the same in-place rules.

The current import-time bake becomes this step's stopgap and is removed when
it lands. Constants authored in GLB units (Miku's arm links, sole offsets,
eye height, equipment slots) keep multiplying by `import_scale()`.

## Consumers to migrate

Each binds on profile-ready instead of bone-map-ready, and reads the profile:

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

1. This ticket.
2. Profile computed and stored on the report, unused; exposed in
   `miku_rig_report` and a debug print. Validate the numbers on Miku, a VRoid
   avatar (Lumae) and Mitten.
3. In-place normalization replacing the import-time bake; Miku at 1.7 m.
4. Profile-ready gating, then consumers one at a time in the order above.

## Constraints

- Measure once per GLTF instance and bone map generation, not per frame.
- Normalization runs once, before first bind; no per-frame transform
  compensation anywhere.
- A scaled mesh copy only when the unscaled one is already registered; never
  duplicate textures.
- Android: the profile and normalization are CPU-side and renderer neutral.

## Validation

- Unit tests: profile from synthetic upright, sideways and T-pose rigs; scale
  math on node translations, inverse binds and mesh data (the existing
  `gltf_height` tests carry over).
- Scene tests: twilight rooftop with Lumae and Mitten keep their current
  heights and capsules (profile within a few cm of today's values).
- Manual (GLB not in the repo): Miku loads at 1.7 m with a normal head,
  crouches, crawls on the roof, holds items correctly, and her capsule comes
  from the profile.

## Open questions

- Head-top estimate when mesh bounds are unusable: fixed ratio of
  soles→head-joint, or the head bone's child/end extent?
- Should non-humanoid models get a reduced profile (bounds and up only), so
  `default_height_if_outside` still works without a bone map?
- Normalization for rigs whose `up` is badly off: rotate, or only report?
