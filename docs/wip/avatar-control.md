# AvatarControl — WIP Design Notes

Date: 2026-03-24

Scratch space for thinking through AVC evolution: current system inventory,
spine IK implications, the head-drift problem, and how IKSystem and
TransformPipelineSystem do or don't share structure.

---

## 1. Current system inventory

### What AVC uses at init

`try_init_splices` creates the following at first tick (when GLTF has spawned):

| Created node | Type | Purpose |
|---|---|---|
| `body_pipeline` | `TransformPipelineComponent` subtree | Shapes `driven_t` → `model_root` (yaw only) |
| `splice_head` | `TransformComponent` | Injected above neck bone; receives head rotation |
| `head_ik` | `IKChainComponent { AimConstraint }` | Sets `splice_head` world rot to match HMD |
| `arm_ik_left` | `IKChainComponent { TwoBoneIK }` | Reaches left arm toward controller |
| `arm_ik_right` | `IKChainComponent { TwoBoneIK }` | Reaches right arm toward controller |

In **simple splice mode** (no arm IK resolved), also creates:

| Created node | Type | Purpose |
|---|---|---|
| `hand_pipeline` (optional) | `TransformPipelineComponent` subtree | `QuatTemporalFilter` on controller rotation |
| `smoothed_t` | `TransformComponent` | Output of hand pipeline; hand bone displaced here |

### What runs per tick (after init)

| System | What it does for AVC |
|---|---|
| `OpenXRSystem` | Writes `driven_t` world pose (HMD) and controller `driven_t` world poses |
| `TransformSystem` | Propagates world matrices; calls into `TransformPipelineSystem` for pipeline nodes |
| `TransformPipelineSystem` | Evaluates body pipeline: `driven_t` → `QuatYawFollow` → `model_root.world` |
| `AvatarControlSystem` | No-op after init (`splice_head.is_some()` short-circuits) |
| `IKSystem` | AimConstraint: head world rot; TwoBoneIK: arm rotations |

`TransformPipelineSystem` does not write directly — `TransformSystem` calls
`evaluate_pipeline_node` during world-matrix propagation and applies the result to
`model_root`'s world matrix inline. No `UpdateTransform` intent is emitted.

`IKSystem` does emit `UpdateTransform` intents for each joint it drives, which are
processed in the same tick's signal drain.

---

## 2. The head-drift problem

### What happens now

`head_bone = "J_Bip_C_Neck"` means `splice_head` is injected **above the neck bone**.
`AimConstraint` sets `splice_head.world_rot = driven_t.world_rot × rot_y(π)`.

`J_Bip_C_Neck` (the neck bone) is displaced under `splice_head`. Its local offset from
`splice_head` is the neck bone vector — roughly `[0, neck_length, 0]` at rest.

When `splice_head` rotates (e.g. pitching down), the neck bone rotates with it. The
HEAD bone (`J_Bip_C_Head`) sits at the tip of the neck bone, so it **arcs forward and
down** relative to the HMD position. From the user's perspective: looking down causes the
vtuber head to swing forward in front of them.

```
HMD position (driven_t) — stays fixed
      ↑
neck_base (splice_head.world_pos) — stays fixed
      |
      | neck bone vector — ROTATES with splice_head
      ↓
J_Bip_C_Head — ARCS when pitching
```

### Why the camera doesn't fix it

`CameraXR` is re-parented under `J_Bip_C_Head`. In VR, what the player SEES is
driven by raw OpenXR eye poses — not `CameraXR`'s world transform. So the camera
re-parenting is cosmetic for third-party views / desktop mirrors. The visual head
mesh is still wrong regardless.

### Solution: splice above J_Bip_C_Head, not J_Bip_C_Neck

Change `head_bone` to `"J_Bip_C_Head"` (the actual head bone, one level up from neck).

```
J_Bip_C_Neck  (stays at rest pose — no IK on it)
  └── splice_head  (TC injected here)
        ├── IKChain { AimConstraint }
        └── J_Bip_C_Head  (displaced under splice_head, local_pos = [0,0,0])
```

`splice_head.world_pos = J_Bip_C_Neck.world_pos + rot(J_Bip_C_Neck.world_rot, head_local_offset)`

Since `J_Bip_C_Neck` has no IK (stays at rest), its world_rot is constant → `splice_head`
stays at a **fixed world position** relative to `model_root`. Only its rotation changes.
`J_Bip_C_Head` has `local_pos = [0,0,0]` relative to `splice_head` → it inherits world
position directly from `splice_head` → **head stays fixed, only rotates**. No arcing. ✓

Tradeoff: the neck bone (`J_Bip_C_Neck`) no longer bends visually when looking down.
It stays at rest pose. This is acceptable for now and is how most simple VR avatar
rigs work.

### Future: neck IK to close the gap

Once spine IK exists, the neck could be driven by a short FABRIK chain or a dedicated
"LookAt position" constraint:

```
goal: J_Bip_C_Head.world_pos = driven_t.world_pos
      J_Bip_C_Head.world_rot = driven_t.world_rot × rot_y(π)
```

Step 1: Rotate `J_Bip_C_Neck` to **aim toward** `driven_t.world_pos` from the neck base.
This is a direction-to-rotation solve: `desired_dir = normalize(driven_t.pos - neck_base.pos)`,
rotate neck to point that direction. If neck length ≈ |driven_t.pos - neck_base.pos|,
`J_Bip_C_Head` lands at `driven_t.pos`.

Step 2: Rotate `J_Bip_C_Head` (via splice above it) to match HMD world rotation, cancelling
whatever the neck added.

This is a 1-bone position + rotation constraint — not quite TwoBoneIK or AimConstraint alone.
A new solver variant (`PositionAndAim`?) or a two-pass approach (neck points toward target,
then head corrects rotation). Deferred until spine IK design is clearer.

---

## 3. How AVC changes with spine IK

### Current body data flow

```
driven_t  →  [body pipeline: yaw only]  →  model_root (translation + yaw)
driven_t  →  [AimConstraint IK]         →  splice_head (full rotation)
controller →  [TwoBoneIK]               →  upper/lower arm rotation
```

Body XZ = HMD XZ exactly. No lag, no lean.

### With spine IK

The missing piece is a **translation delta** between the body's position and the head's
position. When the user leans forward, `driven_t.pos` moves forward, but `model_root`
should lag behind a little, creating a lean angle.

Proposed data flow (not yet designed):

```
driven_t  →  [TranslationFollow: XZ lag]  →  model_root (lagged position + yaw)
driven_t  →  [AimConstraint IK]           →  splice_head (full rotation)
hips_world_pos + driven_t.pos             →  [FABRIK spine chain]  →  spine bone rotations
controller →  [TwoBoneIK]                →  upper/lower arm rotation
```

`TranslationFollow` would be a new `TransformPipelineVec3Op` (like `TemporalFilter` but
for position XZ only). The delta between `model_root.world_pos` and `driven_t.world_pos`
then becomes the "lean" that the spine FABRIK chain needs to solve.

New IK components needed:
- `IKChain { Fabrik }` under hips, end_effector = neck base, target = driven_t position
  (or an offset below driven_t)

### New AVC runtime topology (with spine IK)

```
AVC
  ├── [sys] body_pipeline (TransformPipeline)
  │         TranslationFollow XZ + QuatYawFollow → model_root
  │
  ├── [sys] spine IKChain { Fabrik }
  │         under hips TC, end_effector = neck_base, target = driven_t
  │
  ├── model_root
  │     └── GLTF → armature
  │           ├── hips  ←  spine IK root
  │           │     └── spine chain ... neck_base
  │           │                          └── splice_head
  │           │                                ├── IKChain { AimConstraint }
  │           │                                └── J_Bip_C_Head
  │           ├── J_Bip_L_UpperArm  ←  IKChain { TwoBoneIK }
  │           └── J_Bip_R_UpperArm  ←  IKChain { TwoBoneIK }
  │
  ├── ControllerXR Left  (driven by OpenXRSystem)
  └── ControllerXR Right
```

### Role of TransformPipeline in AVC init

Currently: body pipeline IS a `TransformPipelineComponent` subtree, created by
`try_init_splices`. `TransformSystem` drives it transparently — AVC doesn't need to
know when it runs.

For spine IK: `IKChain` components are also created at init. There's a question of
whether future AVC configuration could be expressed **more declaratively** — describing
which bones get which constraints, and letting AVC init be a pure "construct the
constraint graph" phase. As it stands:

- TransformPipeline handles **shaped streaming** (filtered, stateful signal shaping)
- IKChain handles **constraint solving** (solve for joint angles given a target)

These are complementary, not redundant. AVC init builds the graph; the two systems run
it. No change needed to that division.

---

## 4. IKSystem vs TransformPipelineSystem — shared structure?

### How each system works

**TransformPipelineSystem:**
- Called by `TransformSystem` during world-matrix propagation
- Input: world matrix of nearest TC ancestor of the pipeline node
- Evaluates a linear chain of operators (ForkTRS → ops → MergeTRS) producing an
  output world matrix
- Writes result by overriding the world matrix propagated to child TCs (inline, no intents)
- Stateful operators (`TemporalFilter`, `YawFollow`) key state by component ID

**IKSystem:**
- Called directly in `SystemWorld::tick`, after `AvatarControlSystem`
- Reads world matrices of target TCs and joint TCs (already propagated by `TransformSystem`)
- Solves for joint local rotations; emits `UpdateTransform` intents for each joint
- Stateless solvers currently; no temporal state in IK

### What's duplicated

Both systems define their own private math helpers:

| Math operation | TransformPipelineSystem | IKSystem |
|---|---|---|
| Quat normalize | `quat_normalize` | `normalise_quat` |
| Quat multiply | implicit in basis ops | `quat_mul` |
| Quat from matrix | `quat_from_basis_columns` (columns) | `mat_to_quat` (trace method) |
| Rotation Y | `quat_rotation_y` | `quat_rotation_y` |
| Vec3 lerp | `vec3_lerp` | `vec3_lerp` |

Both read `transform.matrix_world` directly. Neither calls the other. The duplication
is real but small — these are 5-10 line functions and the two systems use them in
different ways (pipeline: mostly quat blending; IK: mostly cross products, arc solves).

**Could these live in `crate::utils::math`?**

There's already a `crate::utils::math` module (`src/utils/math.rs`) used by
`OpenXRSystem` (`math::quat_mul`, `math::quat_conjugate`). The shared helpers could
move there. No architectural change required — just de-duplication. Low priority.

### What can't be shared

The fundamental contract is different:

| Aspect | TransformPipeline | IKSystem |
|---|---|---|
| **Output mechanism** | Overrides world matrix propagation inline | Emits `UpdateTransform` intents |
| **Timing** | Runs inside `TransformSystem` (world-matrix pass) | Runs after `AvatarControlSystem`, reads already-propagated matrices |
| **Multi-joint writes** | One output per pipeline | Writes N joints per chain |
| **State** | Temporal filter state keyed by component ID | Stateless (each tick re-solves) |
| **Graph shape** | Linear: ForkTRS → ops → MergeTRS → output | Arbitrary: root → chain → end_effector |

A pipeline operator that "does IK" (e.g. a `ForkTRS` stage that runs TwoBoneIK and
writes multiple TCs mid-propagation) would break the single-output contract of pipeline
evaluation and create ordering hazards — the modified joints would need to be
re-propagated. These are genuinely separate passes.

### One place they could converge: temporal IK state

Currently IK is **fully stateless** — it re-solves from scratch each tick. Some future
IK features will need per-joint state:

- Pole direction body-local rotation (needs `model_root` world rot sampled last tick)
- IK weight blending / transitions (needs previous weight)
- Jiggle / spring on end effectors (needs velocity)

When that happens, `IKSystem` will need a state store similar to
`TransformPipelineSystem`'s `stage_states: HashMap<ComponentId, StageState>`. Same
pattern, different data. Still not shared code, but same design.

### Could AimConstraint become a pipeline op?

`AimConstraint { offset_yaw }` maps one rotation to another with a constant offset.
It could in principle be expressed as a `TransformPipelineQuatOp`:

```
QuatOp::CopyWorldRotFromTarget { target_id, offset_yaw }
```

...evaluated as a pipeline stage on `splice_head`. The pipeline input would be `driven_t`
world matrix (via `TransformPipelineVec3Op::SampleAncestor`).

But: it still needs to **write to a TC** (`splice_head`) that isn't the inline child of the
pipeline. That requires `PipelineRoute` (a TC-targeting output — not yet built). And the
cross-TC read (`target_id`) breaks the pipeline's assumption that its input is always the
parent world matrix.

Verdict: AimConstraint is simpler as an IK solver. The pipeline op form would add
complexity for no gain.

### Summary

TransformPipeline and IKSystem are **complementary, not convergent**:
- Pipeline: shaping a pose stream (filter, yaw-extract, temporal smoothing) → one output TC
- IK: constraint solving across multiple joints → N `UpdateTransform` intents

The clearest shared opportunity is moving math helpers into `utils::math`. Otherwise
they stay separate. The body pipeline stays a pipeline; head/arm/spine stay IK.
