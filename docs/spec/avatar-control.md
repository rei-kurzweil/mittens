# AvatarControl — Spec

## Goal

A single coordination point that rigs a humanoid avatar for first-person control from a
primary body/head driver (`InputComponent` or `InputXRComponent`) plus any number of
auxiliary pose drivers (`ControllerXRComponent` for hands, future hand-tracking or webcam
drivers, etc.) — without requiring separate input nodes for body and head, and without the
input-broadcast problem that causes body rotation when only the head should move.

---

## Design principle — route all drivers through AvatarControlComponent

The root cause of the old two-input torso-rotation bug was not "too many inputs" but
"drivers operating independently without a coordinator". The rule is:

> **Every transform driver that moves this avatar's bones must be a child of (or otherwise
> routed through) `AvatarControlComponent`.**

This includes:
- The primary body/head driver (one `Input` or `InputXR`).
- Hand wrist drivers (`ControllerXR`, left and right; or future hand-tracking nodes).
- Any future auxiliary driver (finger IK, facial capture, etc.).

`AvatarControlSystem` uses this registration to:
1. Splice each driver into the correct point of the armature during init.
2. Create transform pipelines that shape each driver's pose stream before it reaches
   the armature — so no raw world-matrix propagation from `driven_t` ever reaches
   `model_root` or `splice_head` unshaped.

---

## Topology

### Authored (what the user writes)

```
Input  (or  InputXR)                        ← primary driver
  └── driven_t  (TransformComponent — written each tick by InputSystem / OpenXRSystem)
        └── AvatarControlComponent {
              head_bone:                "J_Bip_C_Neck",
              left_hand_bone:           "J_Bip_L_Hand",    // optional
              right_hand_bone:          "J_Bip_R_Hand",    // optional
              body_yaw_threshold:       π/4,
              body_yaw_rate:            3.0,
              forward_plus_z:           false,             // true for desktop
              hand_rotation_smoothing:  Some(220.0),       // None = no smoothing
            }
              ├── model_root  (TransformComponent — Y offset)
              │     └── GLTFComponent
              ├── ControllerXR (Left,  Grip)  ← declared here; re-parented on init
              │     └── controller_driven_t
              └── ControllerXR (Right, Grip)
                    └── controller_driven_t
```

### Runtime (after AvatarControlSystem init)

```
Input  (or  InputXR)
  └── driven_t
        └── AvatarControlComponent
              │
              ├── [sys] body_pipeline  (TransformPipeline)
              │     reads driven_t world matrix
              │     MapTranslation: Pass
              │     MapRotation:   ExtractYaw → YawFollow { threshold, rate }
              │     MapScale:      Pass
              │     output → model_root
              │
              ├── [sys] head_pipeline  (TransformPipeline)
              │     reads driven_t world matrix
              │     MapTranslation: Drop
              │     MapRotation:   Pass + YawOffset(π for VR, 0 for desktop)
              │     MapScale:      Drop
              │     route → splice_head  (by ComponentId, set during init)
              │
              ├── model_root  (TransformComponent)
              │     └── GLTFComponent
              │           └── [armature]
              │                 neck_parent
              │                   └── [sys] splice_head  ← plain TC injected by system
              │                         └── J_Bip_C_Neck  ← displaced here
              │                 left_lower_arm
              │                   └── ControllerXR (Left, Grip)  ← re-parented by system
              │                         └── controller_driven_t
              │                               └── [sys] hand_pipeline (if smoothing set)
              │                                     └── [sys] smoothed_t
              │                                           └── J_Bip_L_Hand  ← displaced
              │                 right_lower_arm
              │                   └── ControllerXR (Right, Grip)
              │                         └── controller_driven_t
              │                               └── [sys] hand_pipeline
              │                                     └── [sys] smoothed_t
              │                                           └── J_Bip_R_Hand
              │
              ├── ControllerXR (Left,  Grip)  ← re-parented away; no longer here
              └── ControllerXR (Right, Grip)  ← re-parented away
```

`[sys]` nodes are created programmatically by `AvatarControlSystem` during init.

For setups without hand controllers (desktop, or hands not yet implemented), a plain
`TransformComponent` splice is inserted instead. If `hand_rotation_smoothing` is `None`,
the bone is displaced directly under `controller_driven_t` with no pipeline wrapper.

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

    /// Yaw delta (radians) that triggers body rotation. Default: π/4 (45°).
    pub body_yaw_threshold: f32,

    /// Body rotation rate (radians/sec). Default: 3.0.
    pub body_yaw_rate: f32,

    /// Use +Z as the forward axis (desktop). Default false = -Z (OpenXR).
    /// Controls whether the body pipeline uses +Z or -Z for yaw extraction,
    /// and whether a π handedness correction is applied in the head pipeline.
    pub forward_plus_z: bool,

    /// Rotation smoothing factor for hand pose drivers (ControllerXR, hand-tracking, etc.)
    /// applied to the rotation channel of each discovered hand driver's pipeline.
    /// Equivalent to QuatTemporalFilter smoothing_factor. None = no smoothing pipeline.
    /// Default: None.
    ///
    /// The same factor is applied to all hand drivers. Per-hand overrides may be added
    /// in a future revision.
    pub hand_rotation_smoothing: Option<f32>,

    // Runtime IDs set by AvatarControlSystem on first tick:
    pub(crate) splice_head:          Option<ComponentId>,
    pub(crate) displaced_head:       Option<ComponentId>,
    pub(crate) splice_left_hand:     Option<ComponentId>,
    pub(crate) displaced_left_hand:  Option<ComponentId>,
    pub(crate) splice_right_hand:    Option<ComponentId>,
    pub(crate) displaced_right_hand: Option<ComponentId>,

    component: Option<ComponentId>,
}
```

Note: `body_yaw` (the running yaw accumulator) is not a field on `AvatarControlComponent`
in the pipeline design. It is temporal state owned by `TransformPipelineSystem`, keyed by
the `YawFollow` stage path — the same pattern as `QuatTemporalFilter` state.

---

## AvatarControlSystem responsibilities

### Init phase (runs lazily each tick until `splice_head` is set)

#### Step 1 — topology setup

1. Find `model_root` (first `TransformComponent` child of `AvatarControlComponent`).
2. Search `model_root` subtree for `head_bone` by name. **Retry silently if GLTF hasn't
   spawned yet** — this is the only reason init can span multiple ticks.
3. Create `splice_head` (plain `TransformComponent`), attach as sibling of `head_bone`,
   displace `head_bone` under it.
4. For each hand bone (`left_hand_bone` / `right_hand_bone`) if specified:
   - Discover controller by topology: find `ControllerXRComponent` direct children of
     `AvatarControlComponent` matching `ControllerHand::Left` / `Right`.
   - If a controller exists: re-parent it under the hand bone's original parent; displace
     the hand bone under the controller's first TC child (`controller_driven_t`).
   - If no controller: insert a plain TC splice.
5. Store all splice / displaced-bone IDs on the component.

#### Step 2 — pipeline creation

After topology is live, create the system-managed pipeline nodes and attach them.

**Body pipeline** (attached as a child of `AvatarControlComponent`, output root = `model_root`):

```
TransformPipeline
  TransformForkTRS
    MapTranslation: Pass
    MapRotation:   ExtractYaw
                   YawFollow { threshold: body_yaw_threshold, rate: body_yaw_rate }
    MapScale:      Pass
  MergeTRS
  PipelineOutput { roots: [model_root_id] }
```

`TransformPipelineSystem` reads `driven_t`'s world matrix as the pipeline input (since
`AvatarControlComponent` is transparent — not a `TransformComponent` — world matrix
propagation sees `driven_t` as the effective parent).

Because the body pipeline strips pitch and roll from the rotation before it reaches
`model_root`, `model_root`'s local translation `(0, -1.6, 0)` is only ever rotated by
a pure-Y body_yaw quaternion — which leaves the Y component unchanged. The circle-on-pitch
bug (feet arcing when looking up/down) cannot occur with this design.

**Head pipeline** (attached as a sibling of the body pipeline under `AvatarControlComponent`,
routes to `splice_head`):

```
TransformPipeline
  TransformForkTRS
    MapTranslation: Drop
    MapRotation:   YawOffset { radians: π }   // VR: π to bake VRM/OpenXR handedness flip
                                               // Desktop: YawOffset { radians: 0 } (identity)
    MapScale:      Drop
  MergeTRS
  PipelineRoute { target: splice_head_id }
```

`splice_head`'s local translation is `[0,0,0]` under `neck_parent`, so it naturally sits
at `neck_parent`'s world position. Only the rotation is meaningful.

**Hand pipeline** (attached under `controller_driven_t`, output root = `smoothed_t` which
the hand bone is displaced under; only created when `hand_rotation_smoothing` is `Some`):

```
TransformPipeline
  TransformForkTRS
    MapTranslation: Pass
    MapRotation:   TemporalFilter { smoothing_factor: hand_rotation_smoothing }
    MapScale:      Pass
  MergeTRS
  PipelineOutput (inline)
    smoothed_t  (plain TC; hand bone displaced here)
```

If `hand_rotation_smoothing` is `None`, the hand bone is displaced directly under
`controller_driven_t` with no pipeline wrapper — same as the unsmoothed case.

### Tick (after init)

No per-tick transform math. `AvatarControlSystem::tick` only re-attempts init until
`splice_head` is live. Once init is complete, `TransformPipelineSystem` evaluates the
body pipeline, head pipeline, and any hand pipelines automatically every frame.

---

## New pipeline operators required

The body, head, and future hand-tracking pipelines depend on operators not yet in the engine.
This section lists what needs to be added to `TransformPipelineSystem`.

### `QuatExtractYaw` (new `TransformPipelineQuatOp` variant)

Strips pitch and roll, keeping only the Y-axis rotation component.

```rust
TransformPipelineQuatOp::ExtractYaw
```

### `QuatYawFollow { threshold: f32, rate: f32 }` (new temporal quat op)

Stateful operator that advances a running `body_yaw` toward the input yaw when the delta
exceeds `threshold`, at `rate` rad/s. State lives in `TransformPipelineSystem` alongside
`QuatTemporalFilter` state, keyed by stage path.

```rust
TransformPipelineQuatOp::YawFollow { threshold: f32, rate: f32 }
```

### `QuatYawOffset { radians: f32 }` (new `TransformPipelineQuatOp` variant)

Applies a constant Y-axis rotation to the output quaternion. Used in the head pipeline to
bake the VRM/OpenXR handedness flip (`π`) or leave desktop rotation unchanged (`0.0`).

```rust
TransformPipelineQuatOp::YawOffset { radians: f32 }
```

### `TransformPipelineRoute { target: ComponentId }` (new pipeline terminal component)

An alternative to `TransformPipelineOutputComponent` that writes the pipeline's shaped world
matrix to a specific `TransformComponent` identified by `ComponentId`, rather than routing
to the next inline TC child in the tree. Needed so the head pipeline (attached under AVC)
can target `splice_head` which lives deep inside the GLTF armature.

The `ComponentId` is set programmatically by `AvatarControlSystem` after creating
`splice_head` — it is not authored by the user.

---

## Desktop vs VR — same topology, different configuration

**Desktop:**
```
InputComponent { speed: 1.5 }
  └── InputTransformModeComponent { forward_z, fps_rotation, roll_axis_y }
  └── driven_t
        └── AvatarControlComponent {
              head_bone:     "J_Bip_C_Neck",
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
              head_bone:               "J_Bip_C_Neck",
              left_hand_bone:          "J_Bip_L_Hand",
              right_hand_bone:         "J_Bip_R_Hand",
              hand_rotation_smoothing: Some(220.0),
              initial_yaw:             π,
            }
              ├── model_root
              │     └── GLTFComponent
              ├── ControllerXR (Left,  Grip) { T {} }
              └── ControllerXR (Right, Grip) { T {} }
```

The only authoring differences between desktop and VR:
- Swap `InputComponent` → `InputXRComponent` as the primary driver.
- Set `forward_plus_z: true` for desktop (omit for VR).
- Set `initial_yaw: π` for VR (model faces +Z, OpenXR identity is -Z forward).
- Declare `ControllerXR` children for hands (VR only; omit for desktop).
- Set `hand_rotation_smoothing` if rotation smoothing is desired (typically VR only).

The head pipeline's `YawOffset` radians (`π` vs `0`) is derived from `forward_plus_z` by
the system — the user does not set it separately.

---

## Future pose drivers

The same topology handles future auxiliary drivers without changes to authoring:

- **Webcam / face tracking**: a future `FaceCaptureComponent` placed as a child of
  `AvatarControlComponent` would be discovered by topology during init. The system
  would create a pipeline routing its rotation stream to `splice_head` (or a dedicated
  face bone splice) instead of the primary head pipeline.

- **Hand tracking (non-controller)**: a `HandTrackingComponent` child of AVC would be
  treated like `ControllerXRComponent` — discovered by hand, re-parented under the
  hand bone's original parent, with an optional smoothing pipeline wrapping its driven TC.

- **IK / procedural**: placeholder splice TCs are already inserted for configured hand bones
  even when no driver is present. IK systems can target these TCs directly.

The general rule: any pose driver that is a **direct child of `AvatarControlComponent`**
at init time is registered and spliced. Unrecognised children are ignored.

---

## Current implementation status

The current implementation (as of this writing) uses manual `UpdateTransform` emissions
per tick rather than system-created pipelines, because the required pipeline operators
(`ExtractYaw`, `YawFollow`, `YawOffset`, `PipelineRoute`) do not yet exist.

A `model_root_rest_local` cache compensates for the pitch-causes-circle bug that the
pipeline design eliminates structurally.

Pending:
- Implement `ExtractYaw`, `YawFollow`, `YawOffset` as new `TransformPipelineQuatOp` variants
  in `TransformPipelineSystem`.
- Implement `TransformPipelineRouteComponent`.
- Update `AvatarControlSystem::try_init_splices` to create pipeline trees instead of relying
  on per-tick `tick_one` math.
- Add `hand_rotation_smoothing: Option<f32>` field to `AvatarControlComponent` and wire
  it into hand pipeline creation (currently the rotation filter is handled externally in
  `spawn_controller_cube` in `vr-input.rs`).
- Remove `body_yaw`, `model_root_rest_local` fields once pipeline state owns yaw tracking.
- Remove the `AvatarBodyYawComponent` / `AvatarBodyYawSystem` stubs (no longer used).
