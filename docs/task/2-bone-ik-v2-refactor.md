# Fix arm IK "invisible arms" by making all three joints explicit on TwoBoneIK

## Context

We re-introduced 2-bone arm IK in `AvatarControlSystem` last session. In VR
on the bisket avatar, the arm meshes vanished. Cause: the IK solver's mid
joint isn't resolved from the explicit bone chain — it's picked
heuristically as `first TC child of root_tc` at
`src/engine/ecs/system/ik_system.rs:124-136`. Direct inspection of
`bisket.8.0.glb` shows `J_Bip_L_UpperArm`'s children in glTF order:

```
1. J_Sec_L_TopsUpperArmInside_01     ← cloth/tops deformer (PICKED!)
2. J_Sec_L_TopsUpperArmOutside_01    ← cloth/tops deformer
3. J_Bip_L_LowerArm                  ← the bone we actually want
4. J_Bip_L_UpperArm_collider_0.001
5. J_Bip_L_UpperArm_collider_1.001
6. J_Bip_L_UpperArm_collider_2.001
```

`mid_tc` lands on `J_Sec_L_TopsUpperArmInside_01` (a cloth bone parented
near the shoulder), bone-length math degenerates across a tiny
clothing-deformer offset, the solver writes huge rotations to UpperArm,
the arm mesh collapses into the torso and clips invisibly.

The arm's *parent chain* is actually clean:
`J_Bip_L_Hand → J_Bip_L_LowerArm → J_Bip_L_UpperArm → J_Bip_L_Shoulder`.
So `parent_of(hand)` and `parent_of(parent_of(hand))` resolve to the
correct LowerArm + UpperArm. The bug is exclusively the
`children_of(root_tc) | .first(TC)` heuristic in the IK solver.

The deeper API issue: `IKSolver::TwoBoneIK` only carries
`pole_direction` + `copy_end_rotation`. Root joint is inferred from
`parent_of(IKChainComponent)` and mid joint is inferred from "first TC
child of root" — both implicit-topology assumptions that break on rigs
with intermediate bones (twist/roll/helper) or sibling helper nodes
(colliders, etc.). Pole-vector defaults are not at fault.

User intent: make all three arm joints **explicit** on the TwoBoneIK
solver. AVC resolves them at chain-creation time (honoring the existing
`*_upper_arm_bone` / `*_lower_arm_bone` / `*_hand_bone` AVC builder
methods), with a `parent_of` walk-up fallback for clean rigs and verbose
init-time logging so authors can verify resolution.

## Fix shape

### 1. `IKSolver::TwoBoneIK` carries all three joint IDs

In `src/engine/ecs/component/ik_chain.rs`:

```rust
IKSolver::TwoBoneIK {
    root_joint_id: ComponentId,   // upper arm
    mid_joint_id:  ComponentId,   // lower arm
    pole_direction: [f32; 3],
    copy_end_rotation: bool,
}
```

End effector (hand) stays on `IKChainComponent::end_effector_id`. So
three joints + target, all by explicit ID. `parent_of(IKChainComponent)`
is no longer consulted for TwoBoneIK — the chain can be parented anywhere
(or live as a child of AVC for cleanup-on-removal hygiene).

`AimConstraint` and `Fabrik` are **unchanged** — they keep the
`parent_of(IKChainComponent) = root_tc` convention, which works because
their chain definition is unambiguous (1-bone for AimConstraint;
walk-up via `collect_tc_chain` for FABRIK).

Acceptable API asymmetry: TwoBoneIK is the only solver where the middle
joint isn't uniquely determined by walking parent links, so it's the
only one that needs explicit joint IDs. Can refactor all three to be
explicit later if we want consistency.

### 2. IK solver consumes explicit IDs, no topology guessing

In `src/engine/ecs/system/ik_system.rs`:

- `tick_chain`'s match arm for `TwoBoneIK` no longer calls
  `parent_of(id)` for root, and no longer does the "first TC child of
  root" search for mid. It just reads `root_joint_id`, `mid_joint_id`,
  `end_effector_id` straight from the solver/chain and calls
  `solve_two_bone` with `chain = [root, mid, end]`.
- Skip the chain (silent) if any of the three IDs is null or doesn't
  resolve to a `TransformComponent`.
- `solve_two_bone` itself is unchanged — it already takes `chain: &[ComponentId]`.

### 3. AVC resolves all three joints at chain-creation

In `src/engine/ecs/system/avatar_control_system.rs`, replace the current
upper-arm-only resolution block with per-side resolution that produces
`(upper_id, lower_id, hand_id, target_id)`:

For each side (left, right):
- `hand_id`: already resolved via `resolve_hand_splice` (existing).
- `upper_id`: if `*_upper_arm_bone` set → `world.find_component(model_root, "#name")`;
  else fallback to `parent_of(parent_of(hand_id))`.
- `lower_id`: if `*_lower_arm_bone` set → same name lookup;
  else fallback to `parent_of(hand_id)`.
- `target_id`: controller's `driven_t` (already resolved).

If `upper_id` or `lower_id` is `None` after both attempts, log
`[AVC] arm IK disabled for <side>: couldn't resolve <upper|lower> arm bone` and skip.

If an explicit name was provided but `find_component` returned `None`
(typo / wrong rig), log `[AVC] explicit *_upper_arm_bone "<name>" not
found under model_root — left/right arm IK disabled` and skip the chain
entirely (don't fall back to the heuristic — fail loudly so the author
fixes the name).

Construct the chain:
```rust
let chain = IKChainComponent::new(
    IKSolver::TwoBoneIK {
        root_joint_id: upper_id,
        mid_joint_id:  lower_id,
        pole_direction: pole_dir,
        copy_end_rotation: true,
    },
    target_id,
    hand_id,  // end_effector_id
);
let chain_id = world.add_component(chain);
emit_attach(emit, avc_id, chain_id);  // parent for cleanup, not used by solver
```

### 4. Verbose init-time logging (always on, not gated)

At chain-creation time, log one line per side:

```
[AVC] left arm IK: root=J_Bip_L_UpperArm (id=ck#NN), mid=J_Bip_L_LowerArm (id=ck#NN), hand=J_Bip_L_Hand (id=ck#NN), target=ck#NN
[AVC] right arm IK: root=J_Bip_R_UpperArm (id=ck#NN), mid=J_Bip_R_LowerArm (id=ck#NN), hand=J_Bip_R_Hand (id=ck#NN), target=ck#NN
```

Bone names from `world.get_component_node(id).map(|n| n.name.clone())`
(or the project's standard name accessor — verify against existing
`println!`s in `avatar_control_system.rs`).

After resolution, if a resolved `upper_id` or `mid_id` bone's name
contains `Twist`, `Roll`, `Helper`, `_collider`, or `J_Sec_` (VRoid's
"secondary" cloth/hair bone prefix), also log:

```
[AVC] WARNING: left arm IK resolved mid=J_Bip_L_UpperArm_collider_0.001 — this looks like a helper/collider node.
[AVC]   Set explicit left_upper_arm_bone("...") and left_lower_arm_bone("...") in your AvatarControl block.
```

This catches the bisket-style collider regression early without requiring
the author to bisect.

### 5. MMS round-trip stays minimal

The new `root_joint_id` / `mid_joint_id` fields default to
`ComponentId::null()` in the MMS handler at
`src/meow_meow/component_registry.rs:1296-1299`. AVC fills them at
runtime; MMS-authored TwoBoneIK chains would need a separate
name-resolution pass (out of scope — no MMS source currently creates
`TwoBoneIK` chains; only AVC does).

`IKChainComponent::to_mms_ast` at `src/engine/ecs/component/ik_chain.rs:160-166`
already encodes only `pole_direction` + `copy_end_rotation` for the
TwoBoneIK case — leave it that way; runtime-resolved IDs don't round-trip.

## Bisket-specific behavior after the fix

The demo (`examples/bisket-vr-demo.mms`) doesn't currently set
`*_upper_arm_bone` / `*_lower_arm_bone`. Bisket's **parent chain** is
clean (no twist bones along the up-walk from Hand), so the `parent_of`
fallbacks resolve correctly per the inspection in the Context section:

- `parent_of(J_Bip_L_Hand) = J_Bip_L_LowerArm` ✓
- `parent_of(J_Bip_L_LowerArm) = J_Bip_L_UpperArm` ✓
- (R side: identical)

So bisket stays zero-config and works *after the fix*. The cloth +
collider sibling children of UpperArm/LowerArm (which were what broke
the old "first TC child" heuristic) no longer matter, because IK never
iterates `children_of(upper_arm)` anymore — it just uses the explicit
`mid_joint_id` AVC fills in.

Richer rigs with twist bones in the *parent chain* (e.g.
`Hand → LowerArm → UpperArmTwist → UpperArm`) would still need explicit
`*_upper_arm_bone(...)` / `*_lower_arm_bone(...)` names in the `.mms`;
the verbose `[AVC] ... arm IK: root=... mid=... hand=...` log line tells
the author what got resolved, so they can spot when the walk landed on a
twist/helper bone and add the explicit names.

Richer rigs (twist bones, extra wrists) will need explicit names in
their `.mms`; the verbose logging tells the author what got resolved so
they can spot when the heuristic picked a helper bone.

## Critical files

- `src/engine/ecs/component/ik_chain.rs` — add `root_joint_id` +
  `mid_joint_id` fields to `IKSolver::TwoBoneIK`.
- `src/engine/ecs/system/ik_system.rs` — `tick_chain`'s TwoBoneIK branch
  reads explicit IDs; no parent_of/first-child topology guessing for
  this solver.
- `src/engine/ecs/system/avatar_control_system.rs` — per-side joint
  resolution (explicit name → walk-up fallback), verbose logging,
  warning on suspicious-looking resolved bone names, chain creation with
  explicit IDs.
- `src/meow_meow/component_registry.rs` — `TwoBoneIK` constructor in
  the `IKChain` handler at `:1296-1299` defaults the new IDs to
  `ComponentId::null()`.

## Verification

1. `cargo build` clean.
2. `cargo run --release --example bisket-vr-demo`:
   - Stdout should contain two `[AVC] (left|right) arm IK: root=...`
     lines with the expected J_Bip bone names and no `WARNING`.
   - Arms visible in VR, following controllers; UpperArm and LowerArm
     bend at the elbow naturally; Hand orientation matches controller
     (via `copy_end_rotation`).
   - Body XZ + head + neck still correct (regression check on prior fixes).
3. Optional manual edge-case probe: temporarily add
   `left_upper_arm_bone("J_Bip_L_UpperArm_collider_0")` to the demo to
   force the resolver onto a collider node, confirm the WARNING line
   fires, then remove.
4. `cargo run --release --example vtuber-desktop` — untouched (no
   controllers → no arm chains spawned).

## Out of scope

- Refactoring AimConstraint and FABRIK to also use explicit joint IDs
  (current parent-of-chain convention works for them).
- MMS-authored TwoBoneIK chains (would need name-based resolution).
- The clickable debug-cube tool — superseded by the always-on `[AVC]
  ... arm IK:` log line for this bug. Worth revisiting as a general
  tool in a separate session if other bone-inspection needs arise.
