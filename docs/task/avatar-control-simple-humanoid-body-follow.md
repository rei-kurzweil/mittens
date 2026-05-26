# Task: Avatar control simple humanoid body follow

Replace the current spine-FABRIK body-follow experiment in AVC with a simpler,
more stable heuristic body module. The head/camera lock that was just landed is
the foundation; the body should now follow the pose driver in a limited,
predictable way without using spine IK.

Target implementation surface:

- `src/engine/ecs/system/ik/simple-humanoid.rs`
- `src/engine/ecs/system/avatar_control_system.rs`

## Problem statement

The head/camera relationship is now stable because the visible head is mounted
under a dedicated driven node rather than being jointly owned by the spine
solver. That solved the hardest VR issue.

The remaining body behavior is still wrong for the current product goal:

- the torso pitches forward because the spine is still being asked to solve
  toward a target derived from the HMD/camera offset,
- the neck can stretch / translate instead of behaving like a rotational joint,
- the current behavior is overcomplicated for the near-term need,
- crouch / kneel posture should eventually come from authored animation, not
  from pushing a procedural spine IK chain harder.

The desired near-term behavior is intentionally simpler:

- the head stays locked to the pose driver plus authored camera offset,
- the body keeps its own planar anchor under the tracked head rather than
  matching the head position 1:1 at all times,
- planar head/body divergence is allowed temporarily,
- the rest of the body follows the pose driver only in X/Z translation,
- that follow is threshold / deadzone based: small planar head motion does not
  move the body, larger planar separation does,
- the body never inherits pitch/roll from the pose driver,
- only yaw-follow is allowed on the body,
- the neck should not translate or stretch; it should only rotate.

## Target design

```text
driven_t (HMD / InputXR pose)
├── fixed_head_target / visible head mount
│   └── J_Bip_C_Head
└── simple humanoid body heuristic
    └── model_root / body anchor
    └── body follows planar divergence with deadzone + yaw only
            └── avatar skeleton up to neck

neck:
- rotation allowed
- no translation solve
- no stretch

future:
- crouch/kneel state derived from calibrated headset height delta
- delegated to avatar_animation_system blending authored poses
```

## Non-goals for this task

- no spine FABRIK for normal body follow,
- no procedural crouch solved through spine IK,
- no attempt to make the torso exactly match the HMD pitch,
- no separate X-rule and Z-rule unless later testing proves that split is
  necessary,
- no new transform-stream operator in the first implementation; start as AVC /
  simple-humanoid policy code first,
- no backward-compat support for old AVC behavior.

## Phase 1 — simple humanoid body heuristic

Introduce a dedicated helper module for AVC body behavior:

- add `src/engine/ecs/system/ik/simple-humanoid.rs`,
- this module owns a body anchor / model_root planar follow state derived from
  the pose driver,
- the tracked head remains authoritative while the body anchor is allowed to lag
  behind on the ground plane,
- compute planar (`XZ`) delta between tracked head/driver and the current body
  anchor,
- if the planar delta is below a deadzone threshold, keep the body anchor where
  it is,
- if the planar delta exceeds the threshold, move the body anchor toward the
  target according to the heuristic,
- this is one planar follow rule, not separate semantic systems for X and Z,
- body orientation remains yaw-only,
- body must not inherit pose-driver pitch or roll,
- body follow should be stable in VR even when looking sharply up/down,
- no spine IK in this phase,
- do not introduce a new transform-stream operator unless reuse pressure shows
  up after the AVC-specific version is proven.

Expected AVC integration:

- keep the fixed head/camera mount path exactly as-is,
- remove or bypass the current spine FABRIK body-follow path for this mode,
- route model_root/body anchor updates through the simple-humanoid heuristic,
- preserve existing yaw-follow semantics where useful, but make the heuristic
  the owner of body translation behavior.

Acceptance criteria:

- looking up/down does not tilt the torso forward/back,
- small planar head motion inside the deadzone does not constantly drag the
  body,
- larger planar separation causes body recenter/follow on the ground plane,
- walking / leaning causes body planar follow only,
- head/camera lock remains stable,
- body never jitters because of HMD pitch/roll.

## Phase 2 — neck constraints and rigid upper chain behavior

Once body follow is heuristic-driven, fix the neck joint behavior explicitly.

Requirements:

- neck may rotate, but must not translate,
- neck may not stretch,
- upper torso → neck relation should remain rigid in translation,
- if any procedural solve remains in this area, it must preserve authored bone
  lengths exactly.

This phase may use one of two approaches:

- remove neck translation writes entirely and keep only rotational updates, or
- keep a constrained solve but clamp the neck to pure rotational behavior.

Acceptance criteria:

- neck length is visually stable while looking around,
- no visible telescoping / stretching,
- no camera-relative drift introduced by the neck fix.

## Phase 3 — avatar animation for crouch / kneel

Replace procedural body-drop behavior with authored animation blending.

Plan:

- calibrate standing headset height at init (or when XR becomes active),
- measure headset vertical delta from that standing baseline,
- once the delta passes a configurable threshold, derive a crouch amount,
- delegate that crouch amount to a future `avatar_animation_system`,
- blend authored crouch / kneel / sit poses based on that amount.

The simple-humanoid heuristic remains responsible for:

- stable body X/Z follow,
- yaw follow,
- maintaining the head/body separation of concerns.

The avatar animation system becomes responsible for:

- body compression / crouch pose,
- kneel transitions,
- future posture-specific polish.

Acceptance criteria:

- lowering the headset below the standing threshold does not procedurally crush
  the torso,
- crouch is animation-driven and blendable,
- returning to standing restores the idle pose cleanly.

## Documentation follow-up

After the implementation phases above land, audit and refresh stale AVC docs.

Likely affected docs:

- `docs/task/avatar-control-head-driven-redesign.md`
- any AVC comments / topology diagrams in `src/engine/ecs/component/avatar_control.rs`
- any examples or comments that still describe spine FABRIK as the current body
  follow path.

Update them to reflect:

- fixed head mount under the pose driver,
- simple-humanoid body-follow heuristic,
- no spine IK for normal body follow,
- avatar-animation ownership of crouch/kneel behavior.