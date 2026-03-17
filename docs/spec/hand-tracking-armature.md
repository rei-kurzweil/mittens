# Hand tracking armatures: driving glTF skeletons from OpenXR hand joints

This document describes how to drive glTF armature/skeleton hierarchies from OpenXR hand-tracking joint data, enabling realistic hand poses for VTubers, VRM avatars, and other skinned models.

**Scope:**
- Hand-joint tracking from `XR_EXT_hand_tracking`
- Mapping OpenXR joints to glTF armature bones
- Temporal filtering and motion smoothing via the transform pipeline
- Integration with `SkinnedMeshSystem` for skinned mesh deformation

**Related docs:**
- `docs/spec/vr-input.md` — single-pose hand-root driving and controller actions
- `docs/spec/skinned-mesh-system.md` — how skinned meshes work in cat-engine
- `docs/spec/transform-pipeline.md` — temporal operators and transform composition

---

## Overview

OpenXR's `XR_EXT_hand_tracking` extension provides per-frame hand-joint data: position, orientation, and validity for up to 26 joints per hand (wrist, palm, fingers).

Currently, cat-engine reduces this to a single hand-root pose (see `docs/spec/vr-input.md`). To drive an entire armature, we need to:

1. **Extract joint data** from OpenXR at render time (predicted display time)
2. **Map OpenXR joints to glTF armature bones** (by name or explicit mapping)
3. **Apply poses to ECS transforms** via the tick-phase command queue
4. **Apply temporal filtering** (rotation smoothing, spring-style following) using the transform pipeline
5. **Let SkinnedMeshSystem handle the rest** — bone matrices propagate automatically to GPU skinning

This workflow keeps hand tracking within the existing transform infrastructure, avoiding special-case code paths.

---

## OpenXR hand joints

`XR_EXT_hand_tracking` defines hand joint enums for each hand. Common joints include:

- **0: `WRIST`** — wrist/base of hand
- **1–4: `THUMB_*`** — thumb base, proximal, distal, tip (4 joints)
- **5–8: `INDEX_*`** — index finger base, proximal, distal, tip (4 joints)
- **9–12: `MIDDLE_*`** — middle finger (4 joints)
- **13–16: `RING_*`** — ring finger (4 joints)
- **17–20: `LITTLE_*`** — pinky finger (4 joints)
- **21: `PALM`** — palm center

Each joint provides:
- Position (world space, in OpenXR reference space)
- Orientation (world space quaternion)
- Validity flags (`POSITION_VALID`, `ORIENTATION_VALID`)

---

## Architecture: pose flow to armature

### Phase 1: Sample and cache (render time)

In `OpenXRSystem::render_xr`:

```
if XR_EXT_hand_tracking available:
  for each hand (LEFT, RIGHT):
    for each joint (0..25):
      Space::locate(hand_space[joint], predicted_display_time)
      if position_valid && orientation_valid:
        hand_joint_cache[hand][joint] = pose
```

The cache stores all valid joints for each hand. Invalid joints are marked `None`.

### Phase 2: Resolve and apply (tick time)

In a new `HandTrackingArmatureSystem::tick_with_queue` (conceptual; details TBD):

```
for each authored HandTrackingArmatureComponent:
  hand = component.hand (LEFT or RIGHT)
  joint_map = component.joint_mapping  // glTF bone name → OpenXR joint index

  for each (bone_name, xr_joint_index) in joint_map:
    xr_pose = hand_joint_cache[hand][xr_joint_index]

    if xr_pose.valid:
      bone_transform = resolve_bone_component(hand_target, bone_name)

      // Convert XR pose to local space (relative to parent)
      local_pose = space_convert(xr_pose, rig_world, bone_transform.parent)

      queue_update_transform(bone_transform, local_pose)
```

This ensures:
- Only valid joint poses are applied
- Transforms are in local space (ready for the ECS hierarchy)
- Updates are queued and applied in the normal command-queue flow
- Downstream systems (transform propagation, skinning) see updated bone positions

### Phase 3: Transform propagation

`TransformSystem::transform_changed` propagates the hand-driven bone transforms through the skeleton hierarchy. Each bone's `matrix_world` is recomputed and cached.

`SkinnedMeshSystem` uses these cached world matrices to compute per-bone skinning matrices and updates the GPU palette. Mesh deformation happens automatically.

---

## Joint mapping: OpenXR to glTF

### Challenge: naming and structure mismatch

OpenXR hand joints have fixed semantic names (THUMB_PROXIMAL, INDEX_DISTAL, etc.). glTF armatures vary widely:
- Different naming conventions (e.g., `Armature.hand.R.thumb.01`, `Hand_R_Thumb_Proximal`, etc.)
- Different hierarchies (some flatten fingers, others nest them)
- Some glTF models may not include all joints (e.g., only 3 fingers)

### Strategy 1: Name matching (heuristic)

For common glTF armatures (VRM, Mixamo, etc.), match OpenXR joint names against glTF bone names using heuristics:

```rust
// Pseudo-code
fn find_bone_for_xr_joint(gltf_root: ComponentId, xr_joint: HandJoint) -> Option<ComponentId> {
    let xr_name = xr_joint.semantic_name();  // "THUMB_PROXIMAL"

    // Heuristic: look for a bone name containing "thumb" and "proximal"
    walk_transform_tree(gltf_root, |bone| {
        if bone.name.to_lowercase().contains("thumb") &&
           bone.name.to_lowercase().contains("proximal") {
            return Some(bone.id);
        }
    })
}
```

Pros:
- Works out-of-the-box for standard assets
- No explicit configuration needed

Cons:
- Fragile; breaks with unusual naming
- May match wrong bones if names are ambiguous

### Strategy 2: Explicit mapping (authored)

Attach a `HandTrackingArmatureComponent` with an explicit joint-to-bone mapping:

```rust
HandTrackingArmatureComponent {
    hand: Left,
    joint_map: HashMap::from([
        ("hand.L.thumb.01", HandJoint::ThumbMetacarpal),
        ("hand.L.thumb.02", HandJoint::ThumbProximal),
        ("hand.L.thumb.03", HandJoint::ThumbDistal),
        // ... all 26 joints
    ]),
    temporal_filter: Some(QuatTemporalFilterConfig::default()),
}
```

Pros:
- Explicit, unambiguous
- Works with any glTF naming

Cons:
- Requires manual mapping per asset
- More config upfront

### Recommended approach (hybrid)

1. Try heuristic matching first (handles 80% of cases)
2. Fall back to explicit mapping for unusual assets
3. Log warnings when matches are uncertain

---

## Temporal filtering and motion smoothing

Raw OpenXR hand-joint data can be noisy or have discontinuities. The transform pipeline provides operators for smoothing:

### Example: rotation smoothing on a finger

```
ControllerXRComponent (hand root, driven by pose precedence)
  ↓
TransformComponent (hand root transform, applied to scene)
  ↓
TransformComponent (finger base, driven by OpenXR joint)
  ↓
TransformPipelineComponent
  ↓
  TransformForkTRS
    ↓
    TransformMapRotation
      ↓
      QuatTemporalFilter (lower_frequency_hz=10.0)
    ↓
    TransformMapTranslation (passthrough)
    ↓
    TransformMapScale (passthrough)
  ↓
  TransformMergeTRS
  ↓
  TransformPipelineOutput
    ↓
    TransformComponent (smoothed finger output, used for skinning)
```

The `QuatTemporalFilter` applies a low-pass filter to rotation, smoothing jitter while preserving large motions. Translation and scale typically pass through unchanged.

See `docs/spec/transform-pipeline.md` for details on pipeline operators.

### Configuration trade-offs

- **Higher frequency (looser filter):** lower latency, more jitter
- **Lower frequency (tighter filter):** smoother motion, higher latency
- Typical range: 5–15 Hz for hand joints, depending on device and use case

---

## Integration with SkinnedMeshSystem

Once hand-driven bone transforms are updated, the rest happens automatically:

1. `TransformSystem::transform_changed` propagates bone `matrix_world` down the skeleton
2. `SkinnedMeshSystem` discovers skinned renderables under the skeleton root
3. For each skinned instance, `SkinnedMeshSystem` computes:
   ```
   SkinMat[j] = inv(mesh_world) · bone_world[j] · IBM[j]
   ```
   where:
   - `bone_world[j]` is the cached `matrix_world` of bone `j`
   - `IBM[j]` is the inverse bind matrix from the glTF skin
4. GPU palette is updated via `VisualWorld::set_skin_matrices`
5. Vertex shader applies bone transforms to all vertices

No special handling needed—skinned meshes just work with hand-driven bones.

---

## Workflow example: VTuber hand tracking

### 1. Load a VRM/glTF avatar

```rust
let avatar_root = universe.world.add_component(TransformComponent::new());
let gltf = universe.world.add_component(GLTFComponent::new("avatars/vtuber.glb"));
universe.attach(avatar_root, gltf);
universe.add(avatar_root);

// Flush glTF so skeleton exists
universe.systems.gltf.tick_with_queue(...);
```

### 2. Discover hand bones and set up tracking

```rust
let left_hand_root = find_bone_by_name(avatar_root, "hand.L")?;
let right_hand_root = find_bone_by_name(avatar_root, "hand.R")?;

// Option A: Auto-match (heuristic)
let left_tracking = HandTrackingArmatureComponent {
    hand: Left,
    joint_map: None,  // Auto-match by heuristic
};

// Or Option B: Explicit mapping (more reliable)
let left_tracking = HandTrackingArmatureComponent {
    hand: Left,
    joint_map: Some(hand_tracking_vtuber_mapping()),  // Pre-defined for this asset
};
```

### 3. (Optional) Add temporal filtering

Wrap finger bones in transform pipelines to smooth noisy input:

```rust
let thumb_base = find_bone_by_name(left_hand_root, "hand.L.thumb.01")?;
let pipeline = universe.world.add_component(TransformPipelineComponent::new());

// ... build fork/map/filter/merge/output pipeline ...

universe.attach(thumb_base, pipeline);
```

### 4. Let it run

`OpenXRSystem::render_xr` samples hand joints. `OpenXRSystem::tick_with_queue` (or future `HandTrackingArmatureSystem`) applies poses. `SkinnedMeshSystem` computes skinning. The avatar's hands move in real time.

---

## Current limitations

- Hand-armature mapping is not yet implemented in engine code (this is a design doc)
- Name-matching heuristics need to be developed for common asset types (VRM, Mixamo, etc.)
- No automatic skeleton discovery (user must explicitly specify hand-bone root)
- Temporal filtering integration is conceptual; pipeline plumbing needs to be verified
- Per-hand precedence (hand tracking vs. controller grip) not yet authored (only works at the root-pose level)

---

## Future directions

### Short term

1. Implement `HandTrackingArmatureComponent` and mapping logic
2. Wire into `OpenXRSystem::tick_with_queue` (or new system)
3. Test with common VRM/Mixamo assets
4. Develop name-matching heuristics for standard armatures

### Medium term

1. Build UI for interactive joint mapping in editor
2. Create mapping presets for popular asset sources
3. Implement spring-like "follow" operators in the transform pipeline for secondary motion
4. Test with full-body IK + hand tracking

### Long term

1. IK solvers for finger retargeting (hand tracking → bone constraints)
2. Blend between tracked hand and animation-driven hand
3. Hand gesture recognition and automation
4. Per-hand precedence, allowing some fingers to be controller-driven, others hand-tracked

---

## Related files

**Current hand-root tracking:**
- `src/engine/ecs/system/openxr_system.rs`

**Skeleton/armature:**
- `src/engine/ecs/system/gltf_system.rs` — glTF import, armature spawning
- `src/engine/ecs/system/skinned_mesh_system.rs` — bone-to-GPU transform mapping
- `src/engine/ecs/system/transform_system.rs` — world matrix propagation

**Transform pipeline (for filtering):**
- `src/engine/ecs/system/transform_pipeline_system.rs`

**Example:**
- `examples/vtuber-joints-example.rs` — loads VTuber model, inspects joints
