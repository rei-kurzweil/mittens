# Avatar Pose System

Date: 2026-03-21

Explores a system for coupling avatar body yaw to head yaw when head rotation exceeds a
threshold — the "body follows the head" behaviour seen in most VR avatar rigs.

Related prior docs:
- [docs/spec/vr-input.md](vr-input.md) — InputXR + body translation pipeline
- [docs/spec/skinned-mesh-system.md](skinned-mesh-system.md) — armature / bone topology
- [docs/analysis/vr-input-controllerxr-armature-splice.md](../../analysis/vr-input-controllerxr-armature-splice.md)

---

## 1. Current state

`vr-input.rs` sets up the following topology for the avatar body:

```
EditorComponent                               ← scene root; contains the entire avatar subtree
  └── InputXRComponent (avatar_input_xr)
        └── TransformComponent (avatar_driven_t)    ← OpenXRSystem writes full HMD pose here
              └── TransformPipelineComponent
                    └── TransformForkTRS
                          ├── MapTranslation         ← passes through XZ/Y translation
                          ├── MapRotation
                          │     └── TransformDrop    ← DROPS HMD rotation
                          └── MapScale
                    └── TransformPipelineOutput
                          └── TransformComponent (model_root)
                                └── GLTFComponent (avatar mesh)
```

`model_root` has a fixed `rotation_euler(0, π, 0)` so the VRM model faces -Z (matching
OpenXR's forward direction). Since body rotation is dropped by the pipeline, the avatar
**never turns**, only translates.

The head bone is handled separately: a second `InputXRComponent` is spliced into the neck
subtree and drives the head bone rotation directly via a `SampleAncestor` translation
pipeline.

---

## 2. The desired behaviour

When the user turns their head past a **yaw threshold** relative to the body's current facing
direction, the body should rotate to follow until the relative yaw is back within range.

This mirrors natural human movement:
- Small head turns (e.g. ±30°): body stays still, only head rotates.
- Large turns (e.g. ±60°): body rotates so the head stays within a comfortable arc.
- The rotation should be smooth, not a hard snap (configurable rate in °/sec or a slerp
  factor).

Body pitch and roll are not affected — this is yaw-only.

---

## 3. What needs to be read and written

| Input | Where it lives |
|---|---|
| Current HMD yaw | `avatar_driven_t` (TransformComponent, set by OpenXRSystem each frame) |
| Current body yaw | `model_root` local rotation (or tracked state on AvatarPoseComponent) |

| Output | Where it goes |
|---|---|
| New body yaw | `model_root` local rotation, via `UpdateTransform` intent |

Both reads and the write are on `TransformComponent` instances already in the world.

---

## 4. AvatarPoseComponent — shape options

### Option A — marker on model_root, system does all the work

```rust
pub struct AvatarPoseComponent {
    /// Yaw delta (radians) that triggers body rotation.
    pub yaw_follow_threshold: f32,

    /// How fast the body rotates to follow (radians per second).
    /// Set to f32::MAX for instant snap.
    pub yaw_follow_rate: f32,

    /// ComponentId of the HMD-driven TransformComponent to read yaw from.
    /// Set at runtime; not serialized.
    pub hmd_driven_transform: Option<ComponentId>,

    /// Accumulated body yaw (radians). Maintained by AvatarPoseSystem, not stored
    /// in model_root.rotation directly so the system has a stable float to differentiate.
    pub(crate) body_yaw: f32,
}
```

Placed on `model_root` (or a separate anchor sibling). `AvatarPoseSystem` ticks every frame,
reads `hmd_driven_transform`'s world yaw, computes delta, and emits `UpdateTransform` on
`model_root` when rotation changes.

**Pros:** simple component, all logic in one system.
**Cons:** `hmd_driven_transform` is a runtime ComponentId wired at scene construction; not
expressible in MMS without Phase 6 live IDs.

---

### Option B — component on a dedicated "pose rig" node between pipeline output and model_root

```
TransformPipelineOutput
  └── AvatarPoseRigComponent   ← new node between pipeline output and model_root
        └── model_root (TransformComponent)
              └── GLTFComponent
```

`AvatarPoseRigComponent` owns the body yaw state. Because it sits between the pipeline
output (which has world position) and `model_root` (which has the Y-offset and base
orientation), it only needs to supply a rotation around Y and can leave translation/scale
to the surrounding nodes.

**Pros:** topology makes the responsibility boundary clear; no external ID reference needed
(parent is always the pipeline output, child is always model_root).
**Cons:** adds a node to the hierarchy; the component needs to walk up to find the driven
transform (or receive it via intent).

---

### Option C — parameters only, logic inside existing TransformPipelineSystem

Rather than a new system, the pipeline system gains a new operator:
`TransformYawFollowComponent` (like `TransformMapRotationComponent` but stateful). It
interpolates a tracked body yaw toward the HMD yaw and feeds only the resulting Y rotation
into the merged TRS.

```
TransformForkTRS
  ├── MapTranslation
  ├── MapRotation
  │     ├── TransformDrop        (drop pitch and roll)
  │     └── TransformYawFollow   (keep & smooth-follow yaw)
  └── MapScale
```

**Pros:** fits cleanly into the existing pipeline vocabulary; no new system needed; composable.
**Cons:** the operator needs state (current body yaw, last HMD yaw), which complicates the
stateless pipeline design; the threshold/rate parameters have no natural home.

---

## 5. Recommended starting shape: Option A with Option B topology

Use **Option A's data** (fields on a component) placed at the **Option B node position**
(between pipeline output and model_root). The component is called `AvatarBodyYawComponent`
to be specific about its scope:

```rust
pub struct AvatarBodyYawComponent {
    /// Relative yaw (radians) beyond which the body starts rotating to follow the head.
    /// Typical value: π/4 (45°) – π/3 (60°).
    pub threshold: f32,

    /// Rotation rate (radians/sec). Use f32::MAX for instant.
    pub rate: f32,

    /// Runtime: ComponentId of the HMD-driven TransformComponent.
    /// Wired at scene construction. Not serialized.
    pub hmd_driven_transform: Option<ComponentId>,

    /// Runtime: current body yaw (radians, world space).
    /// Maintained by AvatarBodyYawSystem. Initialized from model_root's base rotation.
    pub(crate) body_yaw: f32,

    component: Option<ComponentId>,
}
```

Topology:

```
EditorComponent                              ← scene root
  └── InputXRComponent (avatar_input_xr)
        └── ...
              └── TransformPipelineOutput (av_output)
                    └── AvatarBodyYawComponent    ← sits above model_root
                          └── TransformComponent (model_root, Y-offset + base π rotation baked in)
                                └── GLTFComponent
```

`AvatarBodyYawSystem::tick()`:
1. For each `AvatarBodyYawComponent` in the world:
   a. Read HMD world yaw from `hmd_driven_transform.transform.matrix_world` (decompose Y rotation).
   b. Compute `delta = signed_yaw_diff(hmd_yaw, body_yaw)`.
   c. If `|delta| > threshold`:
      - `target_body_yaw = hmd_yaw - sign(delta) * threshold`
      - `body_yaw = lerp_angle(body_yaw, target_body_yaw, rate * dt)`
   d. If body_yaw changed: emit `UpdateTransform` on `model_root` with new rotation.

---

## 6. Yaw extraction

Both input and output are world-space yaw around Y. The HMD's `matrix_world` is a 4×4
column-major matrix. Yaw can be extracted from the rotation part as:

```rust
fn extract_yaw(m: TransformMatrix) -> f32 {
    // For a Y-up right-hand system, the forward vector is the -Z column (column 2, negated).
    // atan2 of forward.x / forward.z gives yaw.
    let fwd_x = -m[2][0];
    let fwd_z = -m[2][2];
    fwd_z.atan2(fwd_x)  // or atan2(fwd_x, -fwd_z) depending on convention
}
```

`signed_yaw_diff(a, b)` wraps the difference into `[-π, π]` to avoid the ±π discontinuity.

---

## 7. The model_root base rotation interaction

`model_root` currently stores `rotation_euler(0, π, 0)` as the VRM→OpenXR flip. If
`AvatarBodyYawSystem` emits `UpdateTransform` with a new rotation, it must compose the new
body yaw ON TOP of the π base rotation, not replace it:

```
final_rotation = Quat::from_y_rotation(body_yaw) * Quat::from_y_rotation(π)
```

The component should store the **world-space body yaw** (not relative to the base flip), and
the system composes with the baked-in base when writing back.

Alternatively, `model_root` base rotation is removed and the π flip is absorbed into
`body_yaw`'s initial value (`body_yaw_initial = π`). Cleaner but requires care when the
avatar asset changes.

---

## 8. Integration with head rotation splice

The head rotation splice (`InputXRComponent` on `J_Bip_C_Neck`) drives the head bone
independently. When the body rotates to follow, the neck bone moves with it (it's a
descendant of `model_root`). The head splice uses `TransformSampleAncestor(skip=1)` to
sample the neck world position — this recalculates each frame, so it remains correct after
body yaw changes. No special handling required.

---

## 9. Open questions

| Question | Notes |
|---|---|
| Snap vs smooth when threshold first crossed | Snap feels jarring. Smooth lerp toward `hmd_yaw ± threshold` is more natural. |
| Rate curve | Constant rate (°/sec) vs proportional to delta? Large deltas may need faster rate. |
| Pitch/roll isolation | OpenXR HMD pitch can be large (looking up/down). Must strip pitch before extracting yaw — only use Y-axis rotation. |
| Full 360° spin | Fast spins may cause body to lag badly. May need a max-lag clamp (e.g. body never > 90° behind head). |
| Idle settling | If the user holds still, should the body drift to match head over time even within threshold? |
| `hmd_driven_transform` wiring | Currently wired at scene construction. MMS Phase 6 live IDs would allow scripted wiring. |
| System tick position | Should run after OpenXRSystem (which writes `avatar_driven_t`) but before TransformSystem (which propagates matrices). This fits naturally into the existing tick order. |
| AvatarBodyYawComponent name | May want a broader `AvatarPoseComponent` if pitch/IK follow later. Keeping it narrow for now. |
