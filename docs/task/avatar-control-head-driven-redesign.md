# Avatar control: head-driven redesign

Tracking the rework of `AvatarControlComponent` (AVC) so the head bone — not
the neck — is the node that receives the HMD/Input world rotation, with the
spine bending underneath via FABRIK and the body yaw-following hips (rather
than the whole `model_root`).

## Problem statement

The previous AVC drove `J_Bip_C_Neck` directly from `driven_t` (HMD or desktop
Input world pose) via an `IKChain { AimConstraint }`. Because the entire torso
(neck → upper-chest → chest → spine → hips) is a rigid FK chain above the neck,
rotating the neck twisted the visible torso from the neck up. Camera + body
ergonomics were also tangled: `camera_bone` reparented cameras under a bone
that wasn't actually being driven 1:1 by the input, so first-person camera and
visible head pose could diverge.

## Target design

```
driven_t (HMD / Input world pose, 1:1) ──┬─→ head bone: world rotation (and position in VR)
                                          │
                                          └─→ yaw-follow → hips/body anchor rotation (threshold-gated)

spine FABRIK chain: hips → spine → chest → upper_chest → neck → HEAD (end-effector pinned to driven_t)
```

- Head bone receives the input pose directly (already in place via `AimConstraint`).
- Spine FABRIK chain bends between hips and head, so torso follows naturally
  when the head rotates.
- Body yaw-follow sinks at hips (not `model_root`), so the entire avatar
  doesn't rotate as one rigid block — only the hips swing, with FABRIK
  redistributing through the spine.
- Camera sits where the player's eyes are: in VR, `CameraXR` reads HMD pose
  directly (parent transform irrelevant); on desktop, `Camera3D` is reparented
  under the head bone with an eye-position offset.

## Done

### Step 1 — switch head_bone default from neck to head

- `src/engine/ecs/component/avatar_control.rs`
  - `head_bone` default `"J_Bip_C_Neck"` → `"J_Bip_C_Head"`
  - Docstrings updated; topology diagram updated.
- Updated `head_bone` strings in:
  - `examples/vtuber-desktop.mms`
  - `examples/vr-input.{rs,mms}`
  - `examples/bisket-bones-and-ik.mms`

Verified: pitching desktop input no longer twists the torso from the neck up.
Only the head bone rotates; spine stays still until a body yaw-follow kicks in.

### Step 2 — camera reparenting: accept `T { C3D }` wrapper

- `src/engine/ecs/system/avatar_control_system.rs`
  - Camera-children discovery now accepts both bare cameras (`AVC { C3D {} }`)
    and T-wrapped cameras (`AVC { T.position(0, 0.08, 0.07) { C3D {} } }`).
  - In the wrapped form, the T is the node reparented under `camera_bone`,
    preserving its local transform as the eye offset relative to the head
    bone pivot.

### Step 3c — AimConstraint `target_position_offset` (eye-height applied to head only)

After `copy_position` landed, the head bone pivot was at the HMD position — but
the eye mesh sits ~8 cm above the bone pivot, so the camera (= HMD) was looking
out from the chin/jaw, with the face mesh visible above the camera. The
existing `eye_height_from_head_bone(0.08)` was dropping the *entire avatar* via
`model_root.y`, which only affected the FK rest pose of the body; once
`copy_position` started overriding the head bone, the eye-offset on `model_root`
no longer shifted the head and was effectively just lowering the body.

Insight (Rei): the eye-height knob should translate the head *relative to the
HMD driver*, not move the whole character. Implemented by giving `AimConstraint`
a target-local position offset and removing the `model_root.y` subtraction.

- `src/engine/ecs/component/ik_chain.rs`
  - `IKSolver::AimConstraint { offset_yaw, copy_position, target_position_offset }`
    — new `[f32; 3]` field, applied in the target's local frame before copying
    its world position into the joint. Ignored when `copy_position == false`.
- `src/engine/ecs/system/ik_system.rs`
  - `solve_aim` rotates the offset by the target's world rotation, then adds
    it to the target's world position. For an HMD target with `(0, -eye_h, 0)`
    this shifts the bone down along the HMD's local Y so the eye mesh (above
    the pivot in head-local) lines up with the HMD.
- `src/engine/ecs/system/avatar_control_system.rs`
  - AVC head IK now passes
    `target_position_offset = (0, -eye_height_from_head_bone.unwrap_or(0.0), 0)`.
  - Removed the `model_root.y -= eye_height` subtraction. Body stays at the
    natural calibration (head bone FK rest = HMD); the visible head/neck gap
    is what spine FABRIK will close.
- `src/meow_meow/component_registry.rs`
  - `aim_constraint(offset_yaw, copy_position?, target_position_offset?)` —
    third arg optional, defaults to `(0,0,0)`.

### Step 3e — eye offset sourced from camera-wrapper T (one source of truth)

Insight (Rei): the eye offset is semantically *where the camera sits relative to
the head bone pivot*. So it should live on the camera's wrapper T (which already
exists for the desktop forward-axis flip), and AVC should pick it up — not be a
separate scalar field. Depth (Z) also matters, so a vec3 is needed.

- `src/engine/ecs/system/avatar_control_system.rs`
  - Camera-children discovery now returns `(node_to_reparent, eye_offset_head_local)`:
    - bare `C3D`/`CXR` → eye_offset = `[0, 0, 0]`
    - `T { camera }` → eye_offset = T's local translation
  - Eye offset priority: first non-zero T-wrapper translation → fallback
    `eye_height_from_head_bone(f32)` (Y only) → `[0,0,0]`.
  - Head IK `target_position_offset` derived as
    `R(rot_y(head_ik_offset_yaw)) * -eye_offset_head_local`. For VR the X/Z
    flip; Y is preserved across modes.
- `examples/bisket-vr-demo.mms`
  - `CXR` now wrapped in `T.position(0, 0.08, 0.07) { CXR { ... } }`.
  - `eye_height_from_head_bone(...)` line removed — the T translation is the
    single source of truth.

Eye offset semantics now match between desktop (where the T translation
positions the camera directly via parent inheritance) and VR (where OpenXR
overrides the camera pose, so the T translation is consumed by AVC as a
declaration of where the eye sits, used to drop the head IK target).

### Step 3f — body-shift attempt → reverted

First attempt shifted `model_root.translation` by `head_target_offset` so the
body's FK head position would match the AimConstraint head position. Got the Z
sign wrong (didn't account for the body's `initial_yaw=π` rotation flipping
the translation in world space), so the head appeared *further* back from the
body, not closer. Reverted.

Decision (Rei): `T { camera }` translation should ONLY affect the head-vs-camera
relationship (head IK target), NOT the body. The visible head/neck gap is left
as-is — it's exactly the work item that spine FABRIK is meant to solve. The
body stays at the natural Y-only calibration from `camera_bone`.

- `src/engine/ecs/system/avatar_control_system.rs`
  - Reverted to: `model_root.translation = [0, y, 0]` (Y-only from camera_bone).
  - `head_target_offset` is computed from `eye_offset_head_local` and applied
    to the head IK only.

### Step 3d — overlay-routed bone markers in `bisket-vr-demo`

Bone markers were emissive but occluded by the avatar's head/body mesh in
first-person VR. Wrapping each marker in an `OverlayComponent` routes its
subtree into the overlay render pass (drawn after all other phases), so the
markers are visible through the mesh — useful for visualising where each bone
actually sits relative to the XR camera.

Topology per marker: `bone → OV → T(scale) → R.cube { C, EM, Raycastable }`.

### Step 3b — AimConstraint copy_position (head bone tracks HMD translation)

In VR, physically pitching your head moves the HMD forward+down (your real head
pivots around your neck, so the HMD translates). OpenXR writes that translation
into `driven_t`. But `AimConstraint` was rotation-only — the head bone stayed
FK-pinned to the static neck pivot, so the avatar's head visibly swung around
the neck while the HMD/camera moved with the player's physical head. Position
divergence between HMD and head bone, visible in third person as a head "swing"
and in first person as the overlay-cube marker drifting away from the head.

- `src/engine/ecs/component/ik_chain.rs`
  - `IKSolver::AimConstraint { offset_yaw, copy_position }` — new
    `copy_position: bool` field.
  - When true, the joint's world position is also overridden to the target's
    world position (in addition to rotation).
- `src/engine/ecs/system/ik_system.rs`
  - `solve_aim` writes local translation from `inv(parent_world) * target_pos`
    when `copy_position` is set.
  - Other call site (test): defaults to `copy_position: false` (no behavior
    change for existing TwoBoneIK / rotation-only chains).
- `src/engine/ecs/system/avatar_control_system.rs`
  - AVC's head IK now uses `copy_position: true` — head bone fully tracks
    `driven_t` pose (position + rotation).
- `src/meow_meow/component_registry.rs`
  - `aim_constraint(offset_yaw, copy_position?)` — second arg optional.

Side effect: in third person, the head visibly detaches from the neck under
sharp pitch because the neck/spine don't bend yet. That's exactly what the
spine FABRIK chain (still to do) will solve — neck/upper_chest/chest bend to
follow the head's tracked position.

### Step 3a — eye-height calibration (`eye_height_from_head_bone`)

- `src/engine/ecs/component/avatar_control.rs`
  - New field `eye_height_from_head_bone: Option<f32>` + builder
    `.with_eye_height_from_head_bone(f32)`.
  - Round-trips through `to_mms_ast`.
- `src/engine/ecs/system/avatar_control_system.rs`
  - Calibration now does `model_root.y = -(head_bone_local_y + eye_offset)`
    when set, so the avatar's eye line (not the skull base) lands at
    `driven_t`'s world Y = HMD height.
- `src/meow_meow/component_registry.rs`
  - Wires the `eye_height_from_head_bone(...)` MMS call.
- `examples/bisket-vr-demo.mms` uses `eye_height_from_head_bone(0.08)`.

Note: this still leaves a residual face-poke when pitching hard, because the
head bone *pivot* is at the skull base. The mesh swings around that pivot
while the camera stays at the HMD eye position. The full fix is per-camera
mesh culling (see Known issues).

### Step 4 — spine FABRIK chain (first cut)

The plan in `To do` below assumed head bone would stay HMD-pinned via
`copy_position` and FABRIK would close the visible neck/head gap with spine
bending.  That doesn't actually work: an end-effector whose world position is
controlled by `copy_position` is *grounded* — spine rotations don't move it,
so FABRIK measures zero gap and does nothing.

The fix flips the approach: **head bone position is FK-driven by the spine,
AimConstraint is rotation-only, and FABRIK chases the HMD target**.

Concretely:

- `head_mount` carries the rest neck→head offset (translation copied from
  `head_bone` at splice time, head_bone re-anchored with zero local
  translation).  So head_mount IS the head pivot, and FK from neck places
  head_mount at the rest head position.
- Head AimConstraint flipped to `copy_position: false, target_position_offset
  = [0,0,0]` — head_mount's *rotation* matches the HMD, but its *position*
  is whatever FK from the bent spine produces.
- New `IKSolver::Fabrik { target_position_offset }` field (symmetric with
  `AimConstraint`), so the FABRIK chain can chase `HMD + R(HMD) *
  (-eye_offset)` and put the head bone pivot (not the eye mesh above it) at
  the HMD position.
- `BoneMappingSystem::resolve_spine_chain` walks UP from head via
  `tc_ancestor_at_distance` (threshold 0.03m to skip helpers), collecting TC
  joints until a named hips bone or 8 hops.
- `collect_tc_chain` (ik_system) flipped from "walk down via first-TC-child"
  to "walk UP from end_id via parents".  Walking up is unique, so it picks
  out the spine path even when intermediate joints fork (e.g. chest →
  upper_chest + clavicles).
- `AvatarControlComponent` gets `hips_bone: Option<String>` (defaults to
  `"J_Bip_C_Hips"` when unset; FABRIK silently skipped if the bone isn't
  found).
- AVC `try_init_splices`: after head IK, resolve hips, spawn
  `IKChain { Fabrik, target_id=driven_t, target_position_offset=head_target_offset,
  end_effector_id=head_mount }` parented under the hips bone.

Files touched:
- `src/engine/ecs/component/ik_chain.rs`
- `src/engine/ecs/component/avatar_control.rs`
- `src/engine/ecs/system/ik_system.rs`
- `src/engine/ecs/system/bone_mapping_system.rs`
- `src/engine/ecs/system/avatar_control_system.rs`
- `src/meow_meow/component_registry.rs`

To verify in-headset (next): looking up/down should bend neck/chest visibly;
walking shouldn't yet because hips translate-follow is still TODO.

### Step 3 — desktop camera convention

In `examples/bisket-bones-and-ik.mms`:

```mms
AVC {
    head_bone("J_Bip_C_Head")
    camera_bone("J_Bip_C_Head")
    ...
    T.position(0.0, 0.08, 0.07).rotation(0.0, 3.14159, 0.0) {
        C3D {}
        Pointer {}
    }
}
```

- `position(0, 0.08, 0.07)`: eye offset relative to head bone pivot (Y up,
  +Z forward in head-bone local space).
- `rotation(0, π, 0)`: cameras render down -Z but avatar anatomical forward
  is +Z (VRM convention) — flip the camera 180° so its view direction
  matches the avatar's forward.
- `CameraXR` doesn't need the flip — OpenXR overrides pose anyway.

Verified: head + camera stay locked when pitching; view faces the direction
the avatar faces.

## To do

### Ergonomics
- [ ] Decide: should AVC auto-apply the 180° Y flip for `Camera3D` children
  (since it's always needed when parented to a VRM head bone), so users don't
  author it manually? Could be a `camera_flip_y(true)` opt-in/out on AVC.
- [ ] Add `eye_offset: [f32; 3]` field on AVC as a shortcut so the user
  doesn't always need to author a T wrapper for the eye offset.

### Body / spine FABRIK

> **Status (2026-05-26):** First cut landed — see `### Step 4 — spine FABRIK
> chain (first cut)` above.  The approach differs from the original plan in
> this section (head AimConstraint is now rotation-only; the head pivot is
> FK-driven by spine bending rather than warped via `copy_position`).  The
> sections below remain as design context for what's still ahead: Step E
> (hips yaw), Step F (hips translate-follow), and cleanup.

#### Current observable state (the symptom FABRIK fixes)

Running `examples/bisket-vr-demo` in VR:
- Eyes land at HMD position ✓ (head IK target_position_offset works)
- Body stands at natural pose, feet on floor ✓ (model_root.y calibrated from
  camera_bone)
- Head bone is visibly pulled DOWN by `eye_offset.y` and BACK by
  `eye_offset.z` from where the body's FK rest pose expects it
- Result: head appears "further back than it should be" relative to the
  shoulders/neck — like the avatar's head is detached and floating behind.

FABRIK closes this gap by bending the spine (hips → ... → neck) so the head
bone naturally lands at the AimConstraint-determined position WITHOUT
shifting the body. Each spine joint redistributes a fraction of the offset.

#### Implementation order

**Step A — `BoneMappingSystem::resolve_spine_chain`** (`src/engine/ecs/system/bone_mapping_system.rs`)

Existing `BoneMappingSystem` has `resolve_arm_chain` (hand → lower_arm →
upper_arm) and `tc_ancestor_at_distance` (walks TC parent chain with optional
distance filter). Mirror those for the spine.

Signature:
```rust
pub struct ResolvedSpineChain {
    pub head: ComponentId,
    pub neck: ComponentId,
    pub upper_chest: ComponentId,  // optional in some rigs
    pub chest: ComponentId,
    pub spine: ComponentId,
    pub hips: ComponentId,
    /// Ordered hips → ... → head (FABRIK convention: root first, end-effector last)
    pub chain: Vec<ComponentId>,
}

pub fn resolve_spine_chain(
    world: &World,
    model_root: ComponentId,
    head_bone_name: &str,
    hips_bone_name: Option<&str>,        // default: "J_Bip_C_Hips"
    explicit_intermediates: Option<&[Option<&str>]>, // override per-joint
) -> Option<ResolvedSpineChain>;
```

Topology fallback: walk UP from head_bone via `tc_ancestor_at_distance`
(threshold ≈ 0.03m to skip helper bones), collecting TC joints until we hit a
named hips bone or 32-step limit. Then reverse the collection so the chain
runs hips → head.

For VRM, the expected walk yields: head ← neck ← upper_chest ← chest ← spine
← hips (5–6 joints depending on rig).

**Step B — `IKSolver::Fabrik` implementation** (`src/engine/ecs/system/ik_system.rs`)

Variant already exists in `ik_chain.rs`:
```rust
Fabrik { max_iterations: u32, tolerance: f32 }
```
But `ik_system.rs::tick_chain` has no match arm — currently silently does
nothing. Add `solve_fabrik(world, emit, root_tc, target_id, chain, max_iter, tol, weight)`.

Algorithm (textbook FABRIK):
1. Measure bone lengths between consecutive chain joints in world space
   (current FK pose).
2. If target within reach: iterate up to `max_iterations`, each iteration:
   - **Forward pass**: place end-effector at target, walk back along chain
     adjusting each joint to maintain bone length to its child.
   - **Backward pass**: pin root joint at its original world position, walk
     forward adjusting each joint to maintain bone length to its parent.
   - Stop when `|end_effector_pos - target_pos| < tolerance`.
3. If target unreachable (distance > sum of bone lengths): extend the chain
   straight toward target (each joint along the line).
4. Convert per-joint world positions back to local TRS:
   - For each joint i, compute the desired direction to joint i+1 in world.
   - Compose with the parent joint's world rotation to derive local rotation.
   - Emit `UpdateTransform` per joint with new translation (in parent local)
     and rotation.

Subtleties:
- Translation along bones changes the joint pose; rotation aligns the joint's
  forward axis with the direction to the next joint.
- Need to choose what "forward" means per joint (look at next joint).
  For spine bones, the bone's local +Y typically points up the chain
  (VRM convention) — use Y as the "look toward child" axis.
- End-effector rotation: if `copy_end_rotation` style flag is desired,
  preserve the head bone's world rotation (set by the AimConstraint that
  runs separately on the head). Otherwise, FABRIK leaves end-effector rot
  to be whatever its FK chain naturally produces.

Easier alternative: have FABRIK only solve POSITIONS (per-joint translation
in world via rotation of parent), and let the head AimConstraint continue to
own head bone rotation. They compose: AimConstraint runs after FABRIK in the
tick order, so head bone rotation gets overridden.

**Step C — AVC integration** (`src/engine/ecs/system/avatar_control_system.rs`)

In `try_init_splices`, after resolving head splice but before/after building
the head IK:
1. Call `BoneMappingSystem::resolve_spine_chain(world, model_root, &head_bone_name, hips_bone.as_deref(), ...)`.
2. If resolved, spawn an `IKChainComponent` with:
   - `solver: IKSolver::Fabrik { max_iterations: 8, tolerance: 0.001 }`
   - `target_id: head_splice_id` (the splice TC AimConstraint writes — its
     world pose is HMD - eye_offset, where the spine should reach)
   - `end_effector_id: chain.last()` (head bone, or one-before-head if we
     want the AimConstraint to own the head pose)
   - `weight: 1.0`
3. Parent the chain component under `chain[0]` (hips), since IKSystem reads
   `parent_of(ik_id)` as the root joint.

Tick order matters: FABRIK must run BEFORE the head AimConstraint so the
chain reaches toward where the head WILL be (or after — needs decision).
Currently IKSystem iterates all `IKChainComponent`s in arbitrary order;
might need explicit priority or two separate ticks.

**Step D — new AVC bone-mapping fields**

Mirror `head_bone`/`left_hand_bone`/etc. Defaults for VRM:
```rust
pub hips_bone:        Option<String>,  // default None → "J_Bip_C_Hips"
pub spine_bone:       Option<String>,  // default None → topology-derive
pub chest_bone:       Option<String>,
pub upper_chest_bone: Option<String>,
pub neck_bone:        Option<String>,
```
With `.with_hips_bone(...)` etc. builders. MMS bindings in
`component_registry.rs::1890+` (alongside existing `head_bone` etc.).

Add to `to_mms_ast` for round-trip.

**Step E — move body yaw-follow sink from `model_root` to hips**

Current: `TransformForkTRS → MapRotation → QuatYawFollow → model_root` —
rotates the entire avatar around its origin.

Desired: `TransformForkTRS → MapRotation → QuatYawFollow → hips_bone_tc` —
only hips rotates. Spine FABRIK redistributes rotation through the chain so
upper body follows naturally without the rigid block-rotation feel.

This requires the body pipeline to write directly to the hips bone's local
transform (rotation channel). Since hips lives deep inside the model_root
subtree (model_root → GLTF → Armature → Root → Hips), and the body pipeline
output is currently a parent of model_root, we need to either:
- Route the yaw-follow output through a transform pipeline whose sink is the
  hips bone TC (rather than reparenting it), or
- Use an `IKChain { AimConstraint, copy_position: false }` on the hips bone,
  target = body_yaw_follow_output_t.

Second option is more consistent with the head approach.

**Step F — hips translation-follow**

After yaw is on the hips: also need translation. As the player walks (HMD
moves in XZ), the body should follow. Apply via a `TransformMapTranslation`
in the body pipeline, or an `AimConstraint { copy_position: true,
target_position_offset: (0, -avatar_height, 0) }` on hips with appropriate
smoothing.

Y stays grounded (avatar feet on floor). Foot IK is separate.

#### Cleanup after FABRIK lands

- Remove `AvatarBodyYawComponent` + `AvatarBodyYawSystem` (already unused).
- Decide whether `eye_height_from_head_bone(f32)` shortcut field on AVC is
  still useful or fully subsumed by the T-wrapper approach.

### Cleanup
- [ ] Remove `AvatarBodyYawComponent` + `AvatarBodyYawSystem` if unused — the
  yaw-follow is now done via the inline `QuatYawFollow` pipeline in AVC.

### Verification
- [x] Desktop pitching no longer twists torso (bisket-bones-and-ik)
- [x] Desktop camera locked to head pose with eye offset
- [x] VR (OpenXR) — head rotation matches HMD; body yaw-follows after threshold
  (verified via `examples/bisket-vr-demo`)
- [ ] VR — hand controllers (tracked + Grip + Aim) resolve and drive hands
- [ ] After FABRIK lands: torso bends naturally when looking up/down/around

### Known issues

**Arm IK broken (pre-existing, unrelated to this redesign).** In VR demos the
avatar's arms render as completely invisible — likely a zero scale or
zero-length transform somewhere in the `TwoBoneIK` solve path, or the arm
bone chain folding in on itself. The separate-from-AVC Aim controllers (the
cyan/red debug cubes outside the AVC subtree) track fine. So the breakage is
specifically in how AVC wires up the in-subtree Grip controllers to the
`TwoBoneIK { pole_direction, copy_end_rotation }` arm chain — not in OpenXR
or controller tracking itself. Predates this redesign session; not introduced
by any of the head/eye-offset/copy_position changes documented above. Worth
investigating after FABRIK lands, or in parallel as a separate task. Suspect
areas: `solve_two_bone` in `ik_system.rs` (~line 230), the `arm_left`/`arm_right`
construction in AVC (`avatar_control_system.rs` ~line 130–160), or the
`min_bone_length = Some(0.03)` filter in `resolve_arm_chain`.

**VR head-mesh visibility when pitching.** Same root cause as the (now-fixed)
desktop camera divergence: the head bone *pivot* sits at the skull base, while
the HMD pose sits at eye height. AVC currently calibrates `model_root.y` so
the head bone pivot lands at HMD Y — meaning the model's eyes/face mesh ends
up ~5-8cm *above* the HMD camera. Pitching swings the head mesh down into the
camera frustum, so the player sees the inside of the face/hair.

In desktop this was solved by wrapping the camera in a T with the eye offset
so the camera arcs *with* the face mesh. In VR, `CameraXR` pose is
hard-overridden by OpenXR — a T-wrapper offset can't move the rendered eye
position. Two paths:

1. **Per-camera mesh culling (proper fix).** Hide the avatar head mesh from
   the XR camera; show on third-person cameras. Requires a render-layer /
   visibility-mask system that does not currently exist
   (`src/engine/graphics`). Track separately.
2. **Recalibrate `model_root.y` to put the eyes (not the skull base) at HMD
   height.** Partial — face mesh still pokes in under sharp pitch, but better
   neutral alignment. Trivial change to AVC if `eye_offset_y` is known.

For the demo, `bisket-vr-demo.mms` includes a desktop overview camera
(`CameraTarget::Window`) positioned in front of the avatar so the operator
can see the rig from outside the headset while debugging.

## Files & landmarks (cold-context reference)

| Path | What lives there |
| ---- | ---------------- |
| `src/engine/ecs/component/avatar_control.rs` | `AvatarControlComponent` — bone-name config, eye-height fallback, runtime IDs. Add `hips_bone` etc. here. |
| `src/engine/ecs/system/avatar_control_system.rs` | `try_init_splices` — splice/IK chain construction at first tick. Around line 240 = camera-children discovery (eye_offset extraction). Around line 300 = head IK creation. Spine FABRIK construction goes near here. |
| `src/engine/ecs/system/bone_mapping_system.rs` | `BoneMappingSystem` — currently has `resolve_arm_chain` + `tc_ancestor_at_distance` + `find_branching_ancestor`. Add `resolve_spine_chain` mirroring the arm one. |
| `src/engine/ecs/component/ik_chain.rs` | `IKChainComponent` + `IKSolver` enum. `Fabrik` variant exists; needs solver impl in ik_system. |
| `src/engine/ecs/system/ik_system.rs` | `tick_chain` dispatch + `solve_aim` (line ~170) + `solve_two_bone` (line ~230). Add `solve_fabrik` here; add match arm in `tick_chain`. |
| `src/meow_meow/component_registry.rs` | MMS bindings. AVC methods around line 1890; IK solver constructors around line 1290. |
| `examples/bisket-vr-demo.{rs,mms}` | Primary VR test scene. mms has the AVC config; rs has bone-marker spawning (post-GLTF-spawn pattern). |
| `examples/bisket-vr-debug.{rs,mms}` | Headless verification harness — REF + AVC avatars side-by-side, scripted poses, prints diff table + invariants. See `docs/analysis/avatar-control-verification.md`. |
| `examples/vr-input.{rs,mms}` | Earlier VR demo (pc-rei avatar). Same patterns. |
| `examples/bisket-bones-and-ik.mms` | Desktop equivalent. |
| `docs/task/mms-event-payloads-and-runtime-attach.md` | Separate task: MMS-side event handler payloads + runtime `attach()`. Independent of this work; useful for in-app debug authoring. |

## Architecture context (from `CLAUDE.md` + experience this session)

- **Tick order matters**: systems run in a fixed order in `SystemWorld::tick()`.
  IKSystem ticks after `process_signals` rounds finish for animation/transform
  — IK reads world matrices that are already up to date. Adding FABRIK does
  not change this; it just adds another solver invocation in the IK loop.
- **`UpdateTransform` intent vs world matrices**: IK solvers compute desired
  world poses, then convert to local TRS, then emit `UpdateTransform` intents
  that get drained at the next `process_signals` boundary. They don't directly
  mutate `matrix_world`.
- **`copy_position` in AimConstraint** writes BOTH local translation and
  rotation. FABRIK should similarly emit per-joint UpdateTransform intents.
- **Body pipeline = transform fork**: `TransformForkTRS` lets us tap input
  pose channels (T, R, S) independently, transform them, and merge back. It's
  how the body yaw-follow currently consumes `driven_t`'s rotation and emits a
  yaw-only quat for model_root. The same machinery can be retargeted to a hips
  bone sink.

## Verification plan for FABRIK rollout

1. **Implement `solve_fabrik` and unit-test in isolation.** Build a 4-joint
   chain with known bone lengths; assert end-effector reaches target within
   tolerance for reachable targets, lies on the line for unreachable ones.
2. **Wire spine chain in `bisket-vr-demo` only first.** Verify in-headset that:
   - Looking up: neck/chest visibly bend back, head stays at HMD
   - Looking down: spine bends forward, head still at HMD
   - Walking around: hips translate-follow, spine takes a beat to catch up
3. **Once stable, propagate to `vr-input` (pc-rei) and `bisket-bones-and-ik`
   (desktop).**
4. **Cleanup**: remove `AvatarBodyYawComponent` + system.

## Open questions for the fresh context

- Should FABRIK own head bone rotation, or leave it to the head AimConstraint?
  (Recommendation: leave to AimConstraint — it has the right input source and
  the eye-offset logic. FABRIK solves positions only.)
- What's the right FABRIK target — `driven_t` directly, or the `head_mount`
  that AimConstraint writes? (Recommendation: `head_mount` — it already has
  the eye_offset applied, so FABRIK aims at where the head bone *will be*.)
- How aggressive should the body-yaw threshold be when the spine bends? The
  visible swing from yaw-follow plus spine bend might double-count if both
  systems chase the head independently. Test and iterate.
