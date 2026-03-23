# AvatarControl — Spec

## Goal

A single coordination point that rigs a humanoid avatar for first-person control from a
primary body/head driver (`InputComponent` or `InputXRComponent`) plus any number of
auxiliary pose drivers (`ControllerXRComponent` for hands, future IK nodes, etc.) —
without requiring separate input nodes for body and head, and without the
input-broadcast problem that causes body rotation when only the head should move.

---

## Design principle — route all drivers through AvatarControlComponent

The root cause of the old two-input torso-rotation bug was not "too many inputs" but
"drivers operating independently without a coordinator". The new rule is:

> **Every transform driver that moves this avatar's bones must be registered on
> `AvatarControlComponent`.**

This includes:
- The primary body/head driver (one `Input` or `InputXR`).
- Hand wrist drivers (`ControllerXR`, left and right).
- Any future auxiliary driver (finger IK, facial blend, etc.).

`AvatarControlSystem` uses this registry to splice each driver into the correct
point of the armature during init, and to apply the body-yaw-follow logic without
interference from uncoordinated sources.

Multiple drivers are fine. What is not fine is a driver that bypasses
`AvatarControlComponent` and writes directly to armature bones.

---

## Topology

```
Input  (or  InputXR)                        ← primary driver
  └── driven_t  (TransformComponent — written each tick by InputSystem / OpenXRSystem)
        └── AvatarControlComponent {
              head_bone:            "J_Bip_C_Neck",
              left_hand_bone:       "J_Bip_L_Hand",
              right_hand_bone:      "J_Bip_R_Hand",
              left_hand_controller: <ComponentId of pre-created ControllerXR>,
              right_hand_controller: <ComponentId of pre-created ControllerXR>,
            }
              └── model_root  (TransformComponent — Y offset)
                    └── GLTFComponent
                          └── [armature]
                                …
                                neck_parent
                                  ├── splice_head  ← plain TC injected by AvatarControlSystem
                                  │     └── J_Bip_C_Neck  ← displaced here
                                  │           └── J_Bip_C_Head
                                  └── [other neck_parent children]
                                left_lower_arm
                                  └── ControllerXR (left, pre-created by example)
                                        └── controller_driven_t  ← driven by OpenXRSystem
                                              └── J_Bip_L_Hand  ← displaced here
                                right_lower_arm
                                  └── ControllerXR (right)
                                        └── controller_driven_t
                                              └── J_Bip_R_Hand  ← displaced here
```

For setups without hand controllers (desktop, or hands not yet implemented):
`left_hand_controller` / `right_hand_controller` are `None`; the system inserts a plain
`TransformComponent` splice instead (bone can be driven later by IK or left static).

No `TransformPipelineComponent` is needed anywhere in this topology for head or body.
`TransformPipelineComponent` is still used for VR controller **rotation stabilisation**
(inside the pre-created ControllerXR subtree), which is a separate concern.

---

## AvatarControlComponent fields

```rust
pub struct AvatarControlComponent {
    /// Name of the bone to displace for head rotation. Default: "J_Bip_C_Neck".
    pub head_bone: String,

    /// Name of the left hand bone to splice. None = no left hand splice.
    pub left_hand_bone: Option<String>,

    /// Name of the right hand bone to splice. None = no right hand splice.
    pub right_hand_bone: Option<String>,

    /// Pre-created ControllerXRComponent (with driven_t already attached as a child)
    /// to drive the left hand bone. AvatarControlSystem attaches it under the bone's
    /// original parent and displaces the bone under its driven_t.
    /// None = insert a plain TransformComponent splice.
    pub left_hand_controller: Option<ComponentId>,

    /// Same for the right hand.
    pub right_hand_controller: Option<ComponentId>,

    /// Yaw delta (radians) that triggers body rotation. Default: π/4 (45°).
    pub body_yaw_threshold: f32,

    /// Body rotation rate (radians/sec). Default: 3.0.
    pub body_yaw_rate: f32,

    /// Use +Z as the forward axis (desktop). Default false = -Z (OpenXR).
    pub forward_plus_z: bool,

    /// Current world-space body yaw (radians). Maintained by AvatarControlSystem.
    pub(crate) body_yaw: f32,

    // Runtime splice / displaced-bone IDs (set by AvatarControlSystem on first tick):
    pub(crate) splice_head:          Option<ComponentId>,
    pub(crate) displaced_head:       Option<ComponentId>,
    pub(crate) splice_left_hand:     Option<ComponentId>,  // immediate parent of displaced bone
    pub(crate) displaced_left_hand:  Option<ComponentId>,
    pub(crate) splice_right_hand:    Option<ComponentId>,
    pub(crate) displaced_right_hand: Option<ComponentId>,
}
```

---

## AvatarControlSystem responsibilities

### Init phase (once, when `splice_head` is `None`)

1. Find `model_root` (first `TransformComponent` child of `AvatarControlComponent`).
2. Search `model_root` subtree for `head_bone` by name. Retry silently if GLTF hasn't
   spawned yet.
3. Create `splice_head` (plain `TransformComponent`), attach as sibling of the head bone,
   displace head bone under it.
4. For each hand bone (`left_hand_bone` / `right_hand_bone`) if specified:
   - If a controller is registered: find the controller's driven `TransformComponent`
     (first TC child); emit `Attach` to place the controller under the bone's original
     parent, then displace the bone under the controller's driven TC.
   - If no controller: create a plain TC splice, attach as sibling of hand bone, displace
     bone under it.
5. Store all runtime IDs on the component.

### Every tick (once `splice_head` is live)

1. Read `driven_t.matrix_world` (parent of `AvatarControlComponent`).

2. **Body rotation** — emit `UpdateTransform` on `model_root`:
   ```
   local_rotation = quat_inverse(driven_world_rot) * quat_rotation_y(body_yaw)
   ```
   This strips the driven_t rotation from model_root and applies only the desired
   body yaw. Translation and scale are preserved from model_root's current local values.

3. **Head rotation** — emit `UpdateTransform` on `splice_head`:
   ```
   correction      = forward_plus_z ? 0 : π
   head_world_rot  = driven_world_rot * quat_rotation_y(correction)
   local_rot       = quat_inverse(neck_parent_world_rot) * head_world_rot
   ```
   Translation `[0,0,0]` so splice_head sits at neck_parent's world position.
   For VR (`forward_plus_z: false`): the π bakes the VRM +Z / OpenXR -Z handedness flip
   as a constant — no `yaw_correction` scene node needed.
   For desktop (`forward_plus_z: true`): input and VRM both face +Z, no correction.

4. **Body yaw follow** — extract world yaw from `driven_t.matrix_world`, compare to
   `body_yaw`. If `|delta| > body_yaw_threshold`, advance `body_yaw` toward
   `head_yaw ± threshold` at `body_yaw_rate` rad/s; re-emit `UpdateTransform` on
   `model_root` with updated rotation.

Hand splice nodes are **not ticked** by `AvatarControlSystem` — their rotation comes
from the registered `ControllerXR` drivers (written by `OpenXRSystem`).

---

## No TransformPipeline needed (for body/head)

The `TransformPipelineComponent` fork/map/drop/merge/output chain was a declarative
graph that expressed exactly what `AvatarControlSystem` now does imperatively:

| Old pipeline node | Replaced by |
|---|---|
| `TransformForkTRS` | system reads `driven_t` once, branches in code |
| `TransformMapTranslation` | step 2: preserved from model_root local TRS |
| `TransformMapRotation` + `TransformDrop` | step 2: counteract driven rotation |
| `TransformMapScale` | step 2: preserved from model_root local TRS |
| `TransformMergeTRS` | implicit: `UpdateTransform` carries all three |
| `TransformPipelineOutput` | `UpdateTransform` intent on the target node |
| `TransformSampleAncestor` | step 3: `splice_head` at local `[0,0,0]` under `neck_parent` naturally sits at `neck_parent`'s world position |

`TransformPipelineComponent` is still used inside pre-created ControllerXR subtrees
for VR controller **rotation stabilisation** — a separate concern, addressed elsewhere.

---

## No yaw_correction scene node

Earlier designs placed a `TransformComponent` with `rotation_euler(0, π, 0)` between
the pipeline output and the displaced bone. In `AvatarControlSystem` the π correction
is a constant baked into the head rotation math (step 3 above).

---

## Desktop vs VR — same topology, different primary driver

**Desktop:**
```
InputComponent { speed: 1.5 }
  └── InputTransformModeComponent { forward_z, fps_rotation, roll_axis_y }
  └── driven_t
        └── AvatarControlComponent {
              head_bone: "J_Bip_C_Neck",
              forward_plus_z: true,
            }
              └── model_root
                    └── GLTFComponent
```

**VR (with controllers):**
```
InputXRComponent
  └── driven_t
        └── AvatarControlComponent {
              head_bone:            "J_Bip_C_Neck",
              left_hand_bone:       "J_Bip_L_Hand",
              right_hand_bone:      "J_Bip_R_Hand",
              left_hand_controller: left_ctrl,   // pre-created ControllerXR + driven_t
              right_hand_controller: right_ctrl,
              initial_yaw: π,
            }
              └── model_root
                    └── GLTFComponent
```

The only changes between desktop and VR:
- Swap `InputComponent` → `InputXRComponent` as the primary driver.
- Set `forward_plus_z: true` for desktop (omit for VR).
- Set `initial_yaw: π` for VR (model faces +Z, OpenXR identity is -Z forward).
- Register ControllerXR drivers for hands (VR only; omit for desktop).

---

## Current situation

`AvatarControlComponent` + `AvatarControlSystem` are implemented. `vtuber-desktop.rs`
and `vr-input.rs` have been migrated to the single-driver topology.

`AvatarBodyYawComponent` / `AvatarBodyYawSystem` still exist but are no longer used by
the migrated examples. They can be removed once no other examples depend on them.
