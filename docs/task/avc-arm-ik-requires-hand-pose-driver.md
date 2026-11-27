# Require a real hand pose driver before AVC creates arm IK

Status: Implemented; manual XR regression check pending

Date: 2026-08-09

Runtime investigation updated: 2026-08-12

## Problem

`AvatarControlSystem` currently creates left- and right-arm `TwoBoneIK`
chains whenever the humanoid map resolves the corresponding upper arm, lower
arm, and hand bones. It does this even when the AVC has no hand pose drivers.

This breaks desktop avatars such as `examples/vtuber-desktop.mms`:

```text
Input
  -> driven T
       -> AVC
            -> model_root
                 -> GLTF
```

The desktop topology has a primary `Input` pose driver for the body/head, but
no `XRHand` children and no other source of hand target poses. PC-Rei and
Bisket still have complete humanoid arm mappings, so bone availability alone
activates both arm solvers.

The resulting arms and hands continually reach and orient toward synthetic
targets at the world origin.

## Runtime confirmation

Running `cargo run --release --example vtuber-desktop` with the desktop topology
that has no hand tracking produced both AVC arm-chain creation messages:

```text
[AVC] left arm IK: ... target=(id=ComponentId(28593v1))
[AVC] right arm IK: ... target=(id=ComponentId(28596v1))
```

The component IDs vary per run. The important result is that AVC created two
arm chains and assigned two runtime targets even though
`examples/vtuber-desktop.mms` contains no `XRHand` children or other hand pose
drivers. This confirms the chain-construction half of the failure in the real
desktop example, not only by static inspection.

After repairing the `ik_debug()` overlay lifecycle described below, the same
desktop scene provided direct visual confirmation of the target positions:

- the cyan pole, pink plane-normal, green elbow, and white elbow-point markers
  remained near the arms at their expected diagnostic scale;
- both yellow target segments were abnormally long and terminated at the world
  origin;
- the yellow segments therefore showed the exact vectors consumed by
  `TwoBoneIK`: each upper-arm root was solving toward its generated origin
  target.

This closes the remaining runtime question. A custom headless probe is not
needed to establish the cause.

### `ik_debug()` overlay lifecycle defect (fixed 2026-08-12)

Enabling `AVC { ik_debug() }` did not make the target/pole/elbow overlays
visible. This is a separate diagnostic defect:

- `IKSystem::spawn_debug_cube` creates the overlay, transform, renderable,
  color, and emissive components directly with `World::add_component`.
- `World::add_component` is explicitly storage-only and does not invoke
  `Component::init`.
- `spawn_debug_cube` attaches the new nodes but never calls
  `World::init_component_tree` on the generated root.
- Consequently, the debug `RenderableComponent` never emits its registration
  intent and never enters the renderer's visual state. The IK system still
  updates the invisible debug transforms each tick.

Other runtime visualization systems, such as GLTF bounds visualization, build
their generated tree and then call `world.init_component_tree(root, emit)`.
Arm-IK debug visualization now follows the same lifecycle: its generated
overlay roots are passed to `world.init_component_tree(root, emit)`, and a
focused regression test verifies that all five debug renderables initialize and
emit registration intents. This repair remains logically separate from the
desktop arm-activation fix.

## Confirmed cause

AVC discovers direct `ControllerXRComponent` / `XRHand` children independently
for the left and right sides in
`src/engine/ecs/system/avatar_control_system.rs`.

`resolve_hand_splice` then has two behaviors:

- With a controller, use its direct `TransformComponent` child as the tracked
  target.
- Without a controller, create a new identity `TransformComponent`.

In the no-controller case, the new transform is not attached beneath the AVC,
the primary input transform, or the armature. Its world position remains
`[0, 0, 0]`. An optional hand-basis correction is parented beneath it, but that
correction changes rotation only and therefore remains at the same position.

AVC subsequently constructs a full-weight `TwoBoneIK` chain using that
transform as `target_id`. `find_xr_pose_driver` returns `None` for the synthetic
target. In `IKSystem`, `xr_pose_driver == None` means that no pose-validity gate
is applied, so the solver runs every tick against the origin.

This yields two bad channels:

- `TwoBoneIK` rotates the upper and lower arm toward the world origin.
- `copy_end_rotation: true` copies the synthetic target rotation to the hand.

The primary desktop `Input` transform cannot update these targets because the
targets are not part of its transform hierarchy.

## Why bone availability is not an activation signal

AVC requests an implicit automatic `HumanoidBoneMap` when an authored map is
not present. Ordinary humanoid models are expected to resolve upper-arm,
lower-arm, and hand slots. PC-Rei's maintained preset shares the VRoid mapping,
which explicitly defines both complete arm chains.

Consequently, a mapped hand or complete mapped arm means only that the model
*can* support arm IK. It does not mean that the scene supplied live arm-control
intent.

Arm IK needs both structural capability and a target source:

```text
complete mapped arm
AND usable per-side hand pose driver
AND not explicitly disabled
```

### Classification lifetime and refresh policy

The eligibility classification runs once per side during AVC initialization.
No separate persistent classification flag is necessary: an eligible result is
materialized into the cached hand target IDs and generated `IKChainComponent`;
an ineligible side leaves those fields `None` and creates no chain.

This matches the supported runtime lifecycle today:

- `GLTFSystem` spawns each `GLTFComponent` once;
- there is no supported in-place GLTF URI/mesh reload operation;
- replacing the authored GLTF/AVC subtree creates a new AVC and naturally
  classifies it again.

Although `HumanoidBoneMapReport` has a generation number, AVC's current
generation refresh only reroutes the camera anchor. It does not tear down and
rebuild displaced bones, correction targets, or arm chains. Likewise, adding or
removing an `XRHand` after AVC initialization does not currently reconfigure
the arms.

Do not add a manual "reclassify" switch without first implementing an idempotent
AVC teardown/rebind operation. Re-running only classification/creation could
leave duplicate chains or stale joint IDs. A future dynamic rebind should be
triggered by map generation or driver-topology generation and must explicitly
remove the old per-side runtime nodes before rebuilding them.

## Desired behavior

AVC arm IK should be driver-based opt-in on each side.

| Mapped arm | Hand pose driver | Result |
|---|---|---|
| missing/incomplete | any | No chain for that side |
| complete | absent | No chain; preserve FK, animation, or rest pose |
| complete | present but missing its required target `T` | No chain; diagnose malformed driver topology |
| complete | present, target available, XR pose invalid | Chain may exist but solver remains gated |
| complete | present and pose valid | Solve that side normally |
| complete | present but explicitly disabled | No chain |

One-sided tracking must remain valid. A left-hand driver should create only the
left-arm chain; it must not imply a right-arm target.

For the current component set, a usable automatic hand driver means a direct
`XRHand` child of AVC with a direct tracked `TransformComponent` child:

```text
AVC
  -> XRHand(..., Left, ...)
       -> T                         # written by OpenXRSystem
```

The design should leave a seam for non-XR targets from future webcam, keyboard,
pose-library, or procedural drivers without pretending an identity transform is
a live target.

## Proposed fix

### 1. Separate arm capability from arm activation

Implemented 2026-08-12 via the dedicated, non-mutating per-side
`classify_arm_ik_eligibility` seam. It returns one of:

- `Eligible(ArmIkBinding)`,
- `IncompleteArmMap`,
- `NoHandDriver`,
- `MalformedHandDriver`.

`ArmIkBinding` retains the resolved controller, direct tracked target, upper
arm, lower arm, and hand IDs used by initialization.

In `AvatarControlSystem::try_init_splices`, resolve the humanoid bones as model
capabilities, but do not call the hand-target construction path unless that side
has a supported pose driver.

Conceptually:

```rust
let left = left_ctrl.and_then(|controller| {
    resolve_hand_target(
        world,
        mapped_left_hand,
        controller,
        left_aim_correction,
    )
});
```

The actual implementation may use a per-side helper instead of duplicating
this expression. The important invariant is that `controller: None` must not
manufacture a target.

The existing complete-arm checks remain necessary before constructing the IK
chain:

- mapped upper arm,
- mapped lower arm,
- mapped hand,
- usable hand target.

If any item is absent, skip only that side.

### 2. Remove the no-controller identity-target fallback

Implemented 2026-08-12. The old `resolve_hand_splice` fallback was removed and
replaced by `resolve_hand_target`, which accepts only an already eligible
binding and can no longer invent an unattached target.

`resolve_hand_splice` is now an arm-IK target resolver despite retaining an old
"splice" name and fallback. Change its contract so an absent controller or
other target provider returns `None`.

The helper should never create an unattached identity transform as an implicit
IK target. If static IK targets are needed later, they must be explicitly
authored and attached through a target/driver API.

Consider renaming the helper to reflect its current responsibility, for example:

- `resolve_controller_hand_target`, for the minimal XR-specific fix, or
- `resolve_hand_pose_target`, once a generic pose-driver abstraction exists.

The tuple's `bone_original_parent` value also appears to be a remnant of the old
simple-splice implementation. Audit and remove tuple fields that are no longer
consumed rather than preserving misleading topology concepts.

### 3. Calibrate hand basis only for active sides

Implemented 2026-08-12. Hand-basis preparation and correction derivation now
run only after that side classifies as eligible.

AVC currently requests/derives left and right hand aim corrections before it
knows whether either side has a driver. That can delay all AVC initialization on
hand landmark/basis readiness even for a head-only desktop avatar.

Move `ensure_map_hand_basis` and `derive_hand_aim_correction` into the active
per-side path. A desktop AVC with no hand drivers should not:

- create hand correction transforms,
- cache hand target IDs,
- wait for finger landmarks or retained hand bases,
- initialize pointer/laser attachment for absent hands.

Head, body, camera, collision, and neck initialization must remain independent
of unused hand tracking.

### 4. Preserve and strengthen pose-validity gating

Implemented for the current XR-specific path. Eligible bindings originate from
an enabled `ControllerXRComponent` with its required direct tracked transform;
the generated chain retains the existing XR pose-validity owner/gate.

For an AVC-generated XR arm chain, `xr_pose_driver` should always resolve to the
owning `ControllerXRComponent`. Keep the existing `pose_valid` check in
`IKSystem` so initial or temporarily unavailable XR poses do not overwrite the
arm pose.

Add a defensive invariant at chain construction: an XR-controller target that
cannot resolve its owning pose driver should not produce a chain. This avoids
turning a topology error into an ungated solver.

Do not globally change `IKChainComponent.xr_pose_driver == None` to mean
"disabled." Authored non-XR IK chains legitimately have no XR driver and must
continue to solve. The activation correction belongs in AVC's chain-construction
policy, not in the generic IK solver.

### 5. Add an explicit override without making it the activation source

Driver presence should be the default opt-in signal. An explicit AVC setting is
still useful when a scene contains hand drivers for pointing, UI, or attachment
but does not want them to deform the avatar.

A future-facing configuration could be:

```rust
pub enum AvatarArmIkMode {
    Auto,      // default: create each side only when it has a usable driver
    Disabled,  // never create AVC-owned arm chains
}
```

with an MMS surface such as:

```mms
AVC {
    arm_ik_disabled()
}
```

If per-side control is already needed, prefer `left_arm_ik_disabled()` and
`right_arm_ik_disabled()` or a small per-side configuration rather than adding
ambiguous behavior later.

An `arm_ik_enabled()` boolean is not sufficient as the core fix: enabling IK
without an explicit target must never recreate the origin-target bug.

Explicit target references, if introduced later, should be a separate mode or
API, such as `ExplicitTarget(ComponentRef)`, and should participate in target
validity/liveness semantics appropriate to that driver type.

## Implementation seams

The minimal activation fix touches the seams below. A dynamic teardown and
rebind operation remains future work.

### `src/engine/ecs/system/avatar_control_system.rs`

- `try_init_splices`
  - Make arm setup conditional on a per-side usable driver.
  - Avoid unconditional hand-basis preparation.
  - Create and cache target IDs only for active sides.
  - Create `TwoBoneIK` only when both mapped joints and a target exist.
- `resolve_hand_target`
  - Accept only an eligible `ArmIkBinding`.
  - Create only the optional basis-correction child; never synthesize a target.
- `find_xr_pose_driver`
  - Keep as the runtime validity-owner lookup, or make the controller ID
    explicit when constructing an XR-owned chain.
- `update_hand_pose_corrections`
  - Continue to tolerate inactive sides; verify it does not emit work for them.
- Runtime target-ID fields on `AvatarControlComponent`
  - Leave inactive sides as `None`.
- XR hand laser retry loop
  - Invoke only for controllers that actually resolved usable targets.

### `src/engine/ecs/component/avatar_control.rs`

- Correct the component documentation: the current text describes a plain
  transform fallback and old bone-displacement topology that do not match the
  current TwoBoneIK implementation.
- If the explicit override is included, add its configuration field, builder,
  default, and `to_mms_ast` round-trip behavior.

### `src/scripting/component_registry.rs`

- Register the optional MMS disable/per-side builder methods.
- Keep serialization and parsing behavior symmetric.

### `src/engine/ecs/system/ik_system.rs`

- No solver-policy change should be necessary for the minimal fix.
- Retain XR pose-validity gating for chains that declare `xr_pose_driver`.
- Add tests here only if generic gating coverage needs extension; AVC activation
  tests belong with `AvatarControlSystem`.

### Examples and documentation

- `examples/vtuber-desktop.mms` should remain head/body-only without needing an
  explicit opt-out.
- XR examples with direct `XRHand { T }` children should keep automatic arms.
- Update `docs/spec/avatar-control.md`, which currently says desktop has no
  controllers/head-only AVC while also describing a static/simple-splice
  fallback that no longer reflects runtime behavior.
- Update comments in `AvatarControlComponent` and examples that still describe
  controller/bone reparenting from the pre-TwoBoneIK design.

### Completed diagnostic follow-up: `src/engine/ecs/system/ik_system.rs`

- `spawn_debug_cube` initializes/registers its generated component tree.
- `two_bone_debug_visuals_initialize_generated_renderables` verifies the five
  generated renderables (target, pole, plane normal, elbow line, elbow point).
- The diagnostic lifecycle fix does not alter arm-activation policy; the two
  bugs retain independent acceptance criteria.

## Regression tests

Add focused tests that inspect both generated topology and emitted transforms.

### Required

1. **Complete humanoid arms, no controllers**
   - Initialize AVC with mapped left/right upper arm, lower arm, and hand.
   - Assert that no AVC-owned `TwoBoneIK` chain is created.
   - Assert that no synthetic hand target transform is created or cached.
   - Tick IK and assert that arm bone rotations are not updated by AVC-owned IK.

2. **Left controller only**
   - Provide a valid direct tracked `T` for the left `XRHand` only.
   - Assert exactly one left-arm chain and no right-arm chain.
   - Assert the left target is the controller target/correction path, not an
     identity transform at the origin.

3. **Both controllers**
   - Assert two independent chains with the expected target and joint IDs.

4. **Controller missing its direct tracked transform**
   - Preserve the existing regression that refuses to create an origin target.
   - Extend it to assert that no IK chain is produced for that side.

5. **XR pose not yet valid**
   - Assert that the chain may be initialized but emits no bone updates until
     `pose_valid` becomes true.
   - Existing `IKSystem::xr_driven_chain_skips_until_pose_is_valid` coverage can
     remain as the generic solver-level test.

6. **Desktop `Input` topology**
   - Exercise the topology used by `vtuber-desktop.mms`.
   - Assert that head/body AVC initialization completes without waiting for hand
     basis calibration and without creating arm targets/chains.

### Optional override coverage

If an explicit disable API is included:

- both controllers plus `arm_ik_disabled()` creates no arm chains,
- the setting round-trips through MMS serialization,
- per-side disable, if implemented, suppresses only the selected side.

## Manual verification

### Desktop

Run `vtuber-desktop` with PC-Rei:

- arms remain in their authored rest/animation pose,
- hands no longer point or reach toward world origin,
- moving the avatar away from the origin does not alter arm pose,
- head, body yaw, camera, collision capsule, and secondary motion are unchanged.

Repeat with `bisket-desktop-demo` or another Bisket desktop scene where AVC is
present.

### XR

Run a Bisket/PC-Rei XR example with both `XRHand` children:

- neither arm moves until its pose becomes valid,
- each arm follows only its corresponding tracked hand,
- arm pole directions and hand-basis correction remain unchanged,
- temporary loss of a pose does not make the arm reach toward the origin.

Also test one controller disabled or unavailable to confirm per-side behavior.

## Historical note

The no-controller identity transform originated in the old simple-splice path,
where an identity node was inserted into the hand's existing hierarchy and was
therefore harmless at rest. When TwoBoneIK was reintroduced, the same fallback
became a world-space target without being attached into that hierarchy.

Commit `66df8d1` later fixed the related case where an `XRHand` exists but lacks
its required direct tracked transform. It intentionally refuses to create an
origin target and added
`missing_direct_tracked_transform_does_not_create_an_origin_target`. The absent
controller branch still creates one and needs parallel regression coverage.

## Acceptance criteria

- Desktop/head-only AVC creates no arm IK targets or chains by default.
- A complete humanoid arm map alone never activates arm IK.
- Each arm activates only from a usable driver for that side.
- No AVC-generated IK target can silently default to the world origin.
- XR arm solving remains gated until the corresponding pose is valid.
- Missing or inactive hand drivers do not delay unrelated AVC initialization.
- Existing XR examples retain automatic arm IK without additional opt-in syntax.
- An explicit disable override, if implemented, suppresses arm deformation even
  when drivers exist.
- Authored non-AVC and non-XR `IKChain` behavior is unchanged.
- The arm activation fix does not depend on `ik_debug()` becoming visible.

## Out of scope

- Implementing webcam hand tracking.
- Designing keyboard bindings for direct hand movement.
- Reworking the TwoBoneIK geometry, pole-vector math, or wrist-basis correction.
- Changing head/body/camera grounding behavior.
- Making every generic `IKChain` require an XR pose-validity owner.
- Introducing a full generic pose-driver interface in the minimal fix; this task
  only requires that the current XR-specific path stop inventing targets.
