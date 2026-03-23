# IK System

Date: 2026-03-23

Supersedes: `docs/spec/ik-transform-pipelines.md`

IK is a post-FK pass. It reads the world matrices established by `TransformSystem` and
writes corrected local rotations back via `UpdateTransform`. It shares no primitives with
`TransformPipelineSystem` — the two systems are orthogonal.

---

## Component

One component covers all cases:

```rust
pub struct IKChainComponent {
    /// Which solver to run on this chain.
    pub solver: IKSolver,

    /// The TC whose world position/rotation is the IK target this frame.
    /// For AimConstraint: target rotation is read from this component.
    /// For TwoBoneIK / FABRIK: target position (and optionally rotation) is read here.
    pub target_id: ComponentId,

    /// The TC at the end of the bone chain (the end-effector bone).
    /// IKSystem walks TCs from this component's parent down to end_effector_id
    /// to collect the joint chain.
    pub end_effector_id: ComponentId,

    /// Blend weight: 0.0 = no IK applied, 1.0 = full solve.
    pub weight: f32,
}

pub enum IKSolver {
    /// Single-bone rotation match.
    /// Orients the root joint so its world rotation equals the target's world rotation.
    /// Used for neck / head bone alignment from InputXR.
    AimConstraint {
        /// Rotation offset applied after copying target world rotation.
        /// Use [0, PI, 0] for the OpenXR → VRM handedness flip.
        offset_yaw: f32,
    },

    /// Closed-form 2-bone IK.
    /// Requires exactly 2 joints between this component and end_effector_id.
    /// Used for arms (UpperArm → LowerArm → Hand).
    TwoBoneIK {
        /// World-space direction hint for the middle joint (elbow / knee).
        /// Breaks the singularity when the chain is collinear.
        pole_direction: [f32; 3],
        /// If true, also copies target world rotation to the end-effector bone.
        copy_end_rotation: bool,
    },

    /// Iterative forward-and-backward reaching IK.
    /// Works for any chain length ≥ 1.
    /// Used for spine when TranslationFollow adds XZ lag between hips and head.
    Fabrik {
        max_iterations: u32,
        tolerance: f32,
    },
}
```

`IKChainComponent` is placed on the **root joint** of the chain (e.g. `J_Bip_L_UpperArm`
for arm IK, `splice_head` for neck/head aim). No marker components are needed on the
intermediate joints or end-effector — the chain is implicitly the TC path from root to
`end_effector_id`.

---

## The three avatar IK cases

### Neck / head — AimConstraint

| | |
|---|---|
| Root joint | `splice_head` (plain TC inserted above `J_Bip_C_Neck` by AVC) |
| End effector | same as root (1 bone, chain length = 1) |
| Target | `driven_t` — the TC child of `InputXRComponent`, set to HMD stage pose each tick |
| Offset | `offset_yaw: PI` — OpenXR −Z forward → VRM +Z forward handedness correction |

The solve is: `splice_head world_rot = target_world_rot × rot_y(offset_yaw)`, then
decomposed to local: `splice_local_rot = inv(parent_world_rot) × splice_head_world_rot`.

This replaces the manual computation currently hardcoded in `AvatarControlSystem::tick_one`.

### Arms — TwoBoneIK

| | |
|---|---|
| Root joint | `J_Bip_L_UpperArm` / `J_Bip_R_UpperArm` |
| End effector | `J_Bip_L_Hand` / `J_Bip_R_Hand` (2 joints between root and end) |
| Target | `controller_driven_t` — TC output of the ControllerXR smoothing pipeline |
| Pole direction | world-space elbow hint (e.g. `[−1, 0, −1]` for left, `[1, 0, −1]` for right) |

The closed-form solve: use law of cosines to find upper-arm and forearm angles given
bone lengths (from rest-pose world matrices at init) and the distance from shoulder to
target. Place the elbow in the plane spanned by `(target − root)` and `pole_direction`.

`copy_end_rotation: true` — the hand bone also copies the controller's world rotation
(with any grip-to-VRM offset applied through the existing ControllerXR pipeline).

### Spine — FABRIK

| | |
|---|---|
| Root joint | `J_Bip_C_Spine` (or `J_Bip_C_Hips`) |
| End effector | `J_Bip_C_Neck` base (or the bone just below the neck splice) |
| Target | derived position: head world XZ − model_root world XZ, applied as an offset from root |
| When active | only when `TranslationFollow` body pipeline op is in use |

FABRIK is deferred until `TranslationFollow` exists. Without it, body XZ = head XZ and
the spine chain has no positional gap to close.

---

## Tick order

```
1224  transform_pipeline.tick      — FK: body yaw-follow, hand smoothing pipelines
1227  transform.tick               — propagate FK world matrices (all bones at rest pose)
1230  skinned_mesh.tick            — skin palette v0 (one frame behind; current behaviour)
...
1258  openxr.tick + flush          — driven_t and controller_driven_t set to this frame's poses
...
1288  avatar_control.tick + flush  — topology init only (try_init_splices); no longer solves head rot
 NEW  ik.tick + flush              — AimConstraint (head), TwoBoneIK (arms), FABRIK (spine)
 OPT  skinned_mesh.tick (second)   — zero-lag skin palette; only worthwhile if lag is perceptible
```

IK runs after OpenXR so both `driven_t` and `controller_driven_t` hold this frame's poses.
The one-frame skin lag (skinned_mesh at 1230 precedes all of this) is consistent with the
rest of the avatar and is acceptable at 90 Hz.

---

## IKSystem behaviour each tick

```
for each IKChainComponent c in world:
    if c.weight == 0.0: skip

    target_world = world_model(c.target_id)          // read this frame's target TC
    chain = collect_tc_chain(c.parent, c.end_effector_id)  // TCs root..end in order

    match c.solver:
        AimConstraint { offset_yaw }:
            solved_world_rot = quat_mul(quat_from(target_world), rot_y(offset_yaw))
            emit UpdateTransform(chain[0], local_rot = inv(parent_world_rot) × solved_world_rot)

        TwoBoneIK { pole_direction, copy_end_rotation }:
            // bone lengths fixed at first-tick init from rest-pose world matrices
            solve_two_bone(chain[0..=1], target_pos, pole_direction)
            → emit UpdateTransform for chain[0] and chain[1]
            if copy_end_rotation:
                → emit UpdateTransform for end_effector (rotation from target)

        Fabrik { max_iterations, tolerance }:
            solve_fabrik(chain, target_pos, max_iterations, tolerance)
            → emit UpdateTransform for each joint in chain

    blend with weight < 1.0: lerp local_rot from FK value toward solved value
```

Bone lengths for TwoBoneIK and FABRIK are measured once from rest-pose world matrices
on the first tick that the chain is initialized (same frame as AVC `try_init_splices`).

---

## Relation to AvatarControlSystem

After IK is in place, AVC's responsibilities narrow:

| Responsibility | Before IK | After IK |
|---|---|---|
| Topology init (splice bones, body pipeline, hand pipelines) | AVC | AVC (unchanged) |
| Head bone rotation each tick | AVC `tick_one` | IKSystem (AimConstraint) |
| Arm bone angles each tick | not implemented | IKSystem (TwoBoneIK) |
| Spine bending | not implemented | IKSystem (FABRIK, future) |

AVC's `try_init_splices` creates and attaches the `IKChainComponent` for each chain it
discovers (head, left arm, right arm). The IKSystem then owns the per-tick solve.

---

## Open questions

1. **Pole vector in body space vs world space**: the elbow hint should probably be expressed
   in body-local space and transformed to world each tick, so it stays anatomically correct
   when the body rotates.

2. **Handedness correction for arm end rotation**: does the controller grip pose already
   align with the VRM hand bone rest frame, or does it need the same `rot_y(PI)` offset as
   the head? Needs empirical verification against the model.

3. **Shoulder / clavicle**: anatomically, the clavicle rotates as the arm raises. A 3-bone
   chain (clavicle → upper arm → forearm) would look more natural for high arm poses. Not
   required for a first implementation.

4. **Weight blending transition**: when a controller is lost/regained, blending `weight`
   from 1 → 0 over a few frames prevents the arm from snapping back to rest pose.

5. **Second skinned_mesh.tick**: profile cost on the target platform before committing to
   it. A dirty-only incremental pass (only rebind joints touched by IK) would make it cheap.
