# Bone Mapping System

Date: 2026-03-23

`BoneMappingSystem` is a **stateless utility** — a collection of free functions with no
instance state — that resolves semantic bone landmarks (`upper_arm`, `lower_arm`, `hand`,
`neck`, `shoulder`, etc.) to `ComponentId`s in a live skeleton tree.  Called once during
`AvatarControlSystem::try_init_splices`; returns resolved IDs that AVC uses to wire IK
chains and splices.

Its raison d'être: callers should not need to know skeleton naming conventions or topology
rules to find the bones they need.  They provide what they know (explicit names, or just
`hand_bone`) and `BoneMappingSystem` fills in the gaps.

---

## 1. Resolution tiers

For each bone landmark, resolution is attempted in order:

1. **Explicit name** — user-set name on AVC (e.g. `left_upper_arm_bone: Some("J_Bip_L_UpperArm")`).
   Look up by name selector under `model_root`.
2. **Topology derivation** — walk the TC hierarchy relative to a known anchor (e.g. nearest
   sufficiently-distant TC ancestor of `hand` = lower arm; next such ancestor = upper arm).
3. **Heuristic scan** — structural analysis of the skeleton tree (e.g. first 3-way TC split
   above hips = shoulder girdle).  Slower, opt-in, used when neither of the above is available.

Tier 1 always wins over tier 2, tier 2 over tier 3.  A missing result at all tiers means
that landmark is `None` — the caller handles the absence gracefully (skip that IK chain,
leave the bone at rest pose).

---

## 2. Arm chain resolution

### 2.1 AVC fields (new)

```rust
// Explicit arm bone names — all optional.
// If None and the corresponding hand bone is set, topology derivation fills them in.
pub left_upper_arm_bone:  Option<String>,
pub left_lower_arm_bone:  Option<String>,
pub right_upper_arm_bone: Option<String>,
pub right_lower_arm_bone: Option<String>,
```

### 2.2 Derivation rule (tier 2)

Given only `left_hand_bone = "J_Bip_L_Hand"`:

```
hand      = find_by_name(model_root, "J_Bip_L_Hand")
lower_arm = explicit_lower_arm
            OR nearest_tc_ancestor_at_distance(hand, min_bone_length)
upper_arm = explicit_upper_arm
            OR nearest_tc_ancestor_at_distance(lower_arm, min_bone_length)
```

`nearest_tc_ancestor_at_distance(start, min_dist)` walks up the TC parent chain from
`start`, skipping any ancestor whose world position is closer than `min_dist` to the
previous anchor.  This filters out wrist-twist, elbow-helper, and other short helper bones
that sit between the real limb joints.

`min_dist` defaults to `None` (first TC parent, no distance filtering), but can be set to
e.g. `Some(0.03)` (3 cm) to skip helper bones.

For a standard VRM skeleton with `min_dist = None` this reliably gives:

```
J_Bip_L_UpperArm → J_Bip_L_LowerArm → J_Bip_L_Hand
```

With helper bones present (e.g. `J_Bip_L_LowerArm_end` 0.5 cm above the hand):

```
J_Bip_L_UpperArm → J_Bip_L_LowerArm → [J_Bip_L_LowerArm_end skipped] → J_Bip_L_Hand
```

### 2.3 Output

```rust
pub struct ResolvedArmChain {
    pub upper_arm: ComponentId,  // root joint for TwoBoneIK
    pub lower_arm: ComponentId,  // mid joint
    pub hand:      ComponentId,  // end effector (already positioned by ControllerXR)
}
```

---

## 3. Head / neck chain (already resolved by AVC)

AVC currently finds `head_bone` by explicit name and creates `splice_head` one level above
it.  This is already tier-1 resolution.  No change needed here, but `BoneMappingSystem`
could absorb this lookup in a future refactor for consistency.

---

## 4. Future: shoulder / clavicle detection (tier 3)

The shoulder girdle is the first node in the spine chain (going upward from hips) that
branches into three or more TC children:

```
hips
  └── spine
        └── chest
              └── upper_chest
                    ├── neck           ← child 1
                    ├── left_clavicle  ← child 2
                    └── right_clavicle ← child 3
```

The algorithm:

1. Start at model_root (or hips — the armature root TC).
2. Walk upward along the single-child TC chain.
3. The first TC with ≥ 3 TC children = shoulder girdle node.
4. Among those children, classify by rest-pose X position:
   - X ≈ 0 → neck / spine continuation
   - X < 0 → left clavicle (VRM: +X = model right, −X = model left)
   - X > 0 → right clavicle
5. Follow each clavicle child one TC further to reach the upper arm.

`min_bone_length` applies here too — skip child bones closer than the threshold when
stepping from clavicle to upper arm.

This heuristic works for VRM, Mixamo, ReadyPlayerMe.  It will fail for skeletons with
non-standard torso branching or extra helper bones inserted at the shoulder split.  Explicit
names (tier 1) remain the escape hatch.

---

## 5. Future: hip / spine detection (tier 3)

Finding hips:
- Walk all TCs under model_root that have no TC parent within the model_root subtree.
- If exactly one: that is hips.
- If multiple: pick the one with the most TC descendants (most likely the skeletal root).

---

## 6. Full humanoid map (longer-term)

| Landmark | VRM example name |
|---|---|
| hips | J_Bip_C_Hips |
| spine | J_Bip_C_Spine |
| chest | J_Bip_C_Chest |
| upper_chest | J_Bip_C_UpperChest |
| neck | J_Bip_C_Neck |
| head | J_Bip_C_Head |
| left_shoulder | J_Bip_L_Shoulder |
| left_upper_arm | J_Bip_L_UpperArm |
| left_lower_arm | J_Bip_L_LowerArm |
| left_hand | J_Bip_L_Hand |
| right_shoulder | J_Bip_R_Shoulder |
| right_upper_arm | J_Bip_R_UpperArm |
| right_lower_arm | J_Bip_R_LowerArm |
| right_hand | J_Bip_R_Hand |
| left_upper_leg | J_Bip_L_UpperLeg |
| left_lower_leg | J_Bip_L_LowerLeg |
| left_foot | J_Bip_L_Foot |
| right_upper_leg | J_Bip_R_UpperLeg |
| right_lower_leg | J_Bip_R_LowerLeg |
| right_foot | J_Bip_R_Foot |

A `vrm_names()` preset (tier 1) could resolve all of these from a single call, falling back
to topology detection for any that don't match.

---

## 7. API

All functions are free functions (or associated functions on a unit struct — no instance).
`BoneMappingSystem` holds no state.

```rust
pub struct BoneMappingSystem;

impl BoneMappingSystem {
    /// Resolve a 2-bone arm chain from (optional) explicit names + topology fallback.
    ///
    /// `min_bone_length`: if `Some(d)`, topology derivation skips TC ancestors closer
    /// than `d` metres to the previous anchor, filtering out short helper bones.
    /// Pass `None` to always use the immediate TC parent.
    ///
    /// Returns `None` if `hand_name` is not found under `model_root`.
    pub fn resolve_arm_chain(
        world: &World,
        model_root: ComponentId,
        hand_name: &str,
        lower_arm_name: Option<&str>,
        upper_arm_name: Option<&str>,
        min_bone_length: Option<f32>,
    ) -> Option<ResolvedArmChain>;

    /// Walk upward from `start`, returning the nearest TC ancestor whose world position
    /// is at least `min_dist` away from the previous anchor.
    /// If `min_dist` is None, returns the immediate TC parent (first step only).
    pub fn tc_ancestor_at_distance(
        world: &World,
        start: ComponentId,
        min_dist: Option<f32>,
    ) -> Option<ComponentId>;

    /// Find the first TC ancestor of `start` that has >= `min_tc_children` TC children.
    /// Used for shoulder girdle and hip detection.
    pub fn find_branching_ancestor(
        world: &World,
        start: ComponentId,
        min_tc_children: usize,
    ) -> Option<ComponentId>;
}

pub struct ResolvedArmChain {
    pub upper_arm: ComponentId,
    pub lower_arm: ComponentId,
    pub hand:      ComponentId,
}
```

---

## 8. Integration with AvatarControlSystem

```rust
// In try_init_splices, for left arm:
if let Some(arm) = BoneMappingSystem::resolve_arm_chain(
    world,
    model_root,
    &left_hand_bone_name,
    left_lower_arm_bone.as_deref(),
    left_upper_arm_bone.as_deref(),
    Some(0.03),  // skip helper bones closer than 3 cm
) {
    let ik_id = world.add_component(IKChainComponent::new(
        IKSolver::TwoBoneIK {
            pole_direction: [-1.0, -0.3, -0.5],  // left elbow: out + slightly back
            copy_end_rotation: true,
        },
        left_controller_driven_t,
        arm.hand,
    ));
    let _ = world.set_parent(ik_id, Some(arm.upper_arm));
}
```

---

## 9. Resolved open questions

1. **Pole direction space**: resolved — `IKChainComponent.avc_id` caches the ancestor AVC;
   the solver transforms the pole from body-local to world via model root rotation when
   the chain is under an AVC.  Non-AVC chains keep world-space `pole_direction`.

2. **VRM naming preset**: a `vrm_names()` tier-1 resolver would eliminate the need for
   users to set any bone name fields on AVC for standard VRM models.

3. **Armature root**: the topology heuristics assume `model_root` (TC directly under AVC)
   is the GLTF armature root.  Worth documenting as an explicit invariant.
