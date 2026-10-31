# Shared hand tracking system and individual finger retargeting

Date: 2026-10-09

Status: first OpenXR/finger slice implemented (2026-10-19); Vive Focus 3 visual validation
pending. The MediaPipe bridge and broader finger automapping remain separate follow-ups.

Release target: [mittens-engine 0.10.0](epic/0.10.0/README.md).
The release epic includes this task, its prerequisite basis audit, and the
optional MediaPipe bridge in dependency order.

## Where we got to

Controller tracking, hand-root tracking, and opt-in individual finger rotation are implemented.
The current flow in [OpenXRSystem](../../src/engine/ecs/system/openxr_system.rs) is:

1. Create left/right `XR_EXT_hand_tracking` trackers when supported, requesting both controller
   and unobstructed sources when `XR_EXT_hand_tracking_data_source` is available.
2. Locate the complete joint arrays in the session reference space at predicted display time.
3. Prefer runtime palm orientation, falling back to wrist orientation, with a per-side conversion
   from native OpenXR axes to the canonical hand frame. If neither orientation is usable,
   synthesize from `MIDDLE_METACARPAL -> MIDDLE_PROXIMAL` and little-to-index palm width.
4. Use wrist position, falling back to palm position. Retain the root pose, root identity, and
   reported data source in `HandRootPoseCache`. Publish all 26 joints, flags, native values,
   root identity, source, epoch, sequence, reference space, and timestamps in one immutable
   `HandTrackingFrame`, containing both hands.
5. Prefer `WristPalm` for a valid root reported as unobstructed tracking. Otherwise retain
   controller priority, falling back to the root when the requested action pose is unavailable.
6. AVC applies the retained avatar hand-basis correction and drives arm IK.
7. `HandTrackingSystem` selects a fresh articulation sample independently per hand and
   retargets mapped finger rotations after arm IK has flushed, before skinning.

The new [sample contract](../../src/engine/ecs/system/hand_tracking.rs) and
[hand system](../../src/engine/ecs/system/hand_tracking_system.rs) keep provider selection separate
from imported bone names. OpenXR remains the only live adapter; a MediaPipe enum/configuration
entry and replay tests establish its boundary without a camera or Python dependency.

The [humanoid map work](humanoid-bone-map-automapping-and-mms-presets.md) implemented conservative
semantic mapping and automatic hand bases. It now includes all 30 rotating finger slots;
the shared VRoid preset explicitly maps three bones per finger on both sides. Thumb1 maps to
metacarpal, Thumb2 to proximal, Thumb3 to distal. Non-thumb 1/2/3 map to
proximal/intermediate/distal. New slots require explicit declarations in this slice.

## Implemented first slice

Opt in under the owning GLTF alongside its humanoid map:

```mms
GLTF.new("assets/models/mitten.glb") {
    bisket_humanoid_bone_map()
    HandTracking.new()
        .source("auto")
        .timeout(0.15)
        .release_duration(0.15)
}
```

`examples/mittens-corp.mms` enables this for mitten. Source accepts `auto`, `openxr`, or
`mediapipe`; auto prefers OpenXR independently on each side. Selecting MediaPipe alone produces
no articulation until its adapter is implemented. The 150 ms timeout/release defaults are
provisional and configurable, awaiting live headset tuning. There is no additional tracking
smoothing or activation hysteresis in this slice.

Finger directions come from measured segment endpoints. Roll comes from transporting the
canonical palm width down each finger with successive minimum rotations, rather than projecting
a fixed palm normal through a closed fist. Native joint orientations and their validity/tracked
flags remain available for diagnostics/future twist refinement. This slice estimates finger twist;
it does not reproduce every native joint's axial twist. Thumb opposition follows its three
measured segments. Nearly antiparallel adjacent segments and zero-length segments release the
affected finger instead of selecting an arbitrary roll axis.

Bindings use the existing owning-GLTF semantic report and immutable rest geometry, honor
published `JointRetargetBasis` overrides, and refuse invalid/conflicting authored bases. A
VRoid distal bone has no imported tip: its rest direction continues the incoming rest segment.
Tracked tips are endpoints, never extra rotating bones. No translations or scales are written.
Actual helper-parent rotations and desired rotations of already-driven parents participate in
the local conversion. Wrist translation/rotation remain owned by the existing XR/AVC/IK path.

Tracking overrides evaluated finger animation while active. The system retains the underlying
pose, observes new animation writes, and blends toward it on missing/invalid/stale samples;
without animation this is the imported rest pose. A missing segment releases its complete finger
chain, while other valid fingers continue. Duplicate HandTracking components on one GLTF disable
that rig's tracking. Removal or map/basis-generation changes restore surviving owned bones before
rebinding. Source epochs and spaces never carry joint filters or geometry across frames: each
articulation uses one complete sample and independently recomputes its frames.

Set `CAT_DEBUG_HAND_TRACKING=1` to log source/epoch/data-source and tracked-finger-count changes.
Binding diagnostics identify unresolved chains and unusable rest bases. The existing laser still
uses its immutable `RestAttachment`; it does not automatically follow a curled fingertip.

Automated tests import the actual mitten skeleton without GPU assets and cover rest reconstruction,
five independent fingers on both sides, fists/splay/opposition, changed bone axes, helper twists,
current parent ordering, partial/NaN/degenerate/stale data, animation release, provider selection,
map rebinding, component removal, and two identically named avatar instances. OpenXR adapter tests
cover all 26 slots/flags and loss publication; existing wrist/arm IK regression tests remain in
place. Live skinning, thumb appearance, source handover, and timeout tuning still require a
Vive Focus 3 run.

Validation for this slice: 43 hand-related tests, 11 humanoid-map tests, 6 generic retarget-basis
tests, 3 runtime-schema tests, and the retained `mittens-corp` mitten scene test pass. Broader
scene checks also encounter two unrelated legacy linear-velocity scene failures
(`undefined: 'is_string'`) and an old humanoid-example count assertion expecting 29 while 31
qualifying examples exist. The latter 31-example set already exists in HEAD; this slice adds
no example files. Those unrelated failures are outside the focused passing gate.

The older [hand-armature design](../spec/hand-tracking-armature.md) is conceptual, not implemented.
Its joint numbering is incorrect, its per-joint `Space::locate` sketch does not describe the
current `locate_hand_joints` API, and its direct world-pose-to-bone proposal omits rest-basis
retargeting and differing hand proportions. Treat this task as the current implementation plan.

OpenXR enum values are palm 0, wrist 1, thumb 2–5, index 6–10, middle 11–15, ring 16–20,
and little 21–25. Use semantic enums, not the older document's numeric table.
[Khronos joint definition](https://registry.khronos.org/OpenXR/specs/1.1/man/html/XrHandJointEXT.html).

## Companion tasks and order

1. [Investigate wrist/arm/finger frames](hand-tracking-wrist-arm-and-finger-basis-audit.md).
   Establish the source and target frame contracts before expanding pose driving.
2. Implement the shared sample and OpenXR adapter described here; retain controller behavior.
3. Implement one explicit five-finger rig mapping and rotation-only articulation.
4. [Add optional MediaPipe process and transport](optional-mediapipe-tracking-bridge.md).
   A replay producer can develop this adapter independently of camera setup.
5. Add conservative finger automapping and broader rig validation after the explicit path works.

## Responsibilities and follow-ups

`HandTrackingSystem`, `HandTracking`, and the OpenXR publication path now exist. The planned
[`MediaPipeSystem`](optional-mediapipe-tracking-bridge.md#engine-owner-mediapipesystem)
owns the shared receiver/provider state for the next slice and feeds independent semantic systems.

| Owner | Responsibility |
| --- | --- |
| `OpenXRSystem` | Own session/trackers, locate joints at predicted time, publish an immutable sample |
| `MediaPipeSystem` (planned) | Own shared endpoint/stream/channel state, receive and normalize snapshots, and publish independent hand/eye provider samples |
| `HandTrackingSystem` in `hand_tracking_system.rs` | Retain sources, choose per-hand samples, expose normalized capabilities/status |
| Hand rig binding/retargeting consumer | Resolve imported bones and bases once; derive and queue finger rotations |
| AVC and IK | Consume the selected wrist target and solve arms using the corrected visual hand frame |
| Transform and skinning systems | Propagate queued transforms and update the existing skin palette |
| Input/pointer systems | Keep controller actions and pointer aim separate from skeletal articulation |

The first retargeting consumer can live within the hand system if small. Keep source selection
independent of imported skeleton names and retargeting math so it can be separated later.

Eye tracking provides the useful pattern: a generic selector and provider components, source
priority, and normalized outputs. In the current
[eye system](../../src/engine/ecs/system/xr_eye_tracking_system.rs), MediaPipe returns `None`;
[its component](../../src/engine/ecs/component/xr_eye_tracking.rs) is a future configuration
anchor. There is no working MediaPipe implementation to reuse. Reuse the separation of concerns,
but explicitly define hand freshness and ownership rather than assume the eye policy suffices.

## Sample contract

Retain one immutable snapshot per provider/frame, containing both hands when available:

- Provider identity, stream/session epoch, monotonically increasing sequence, capture/sample time,
  engine receipt time, and source time domain. OpenXR predicted time is distinct from webcam time.
- Explicit coordinate-space identity, units, handedness, mirror state, and calibration generation.
- Per-hand activity and semantic joints, with optional positions and orientations independently.
  Preserve valid/tracked flags, optional radii, and available confidence with its meaning.
- Root position kind: wrist, palm fallback, or absent. Palm is not silently relabeled as wrist.
- Capabilities: positioned wrist, hand orientation, articulation, and measured/estimated data.
- Raw provider values for diagnostics and normalized values for consumers. OpenXR quaternions
  and MediaPipe landmark-derived orientations must retain different provenance.

Use semantic joint keys in the engine. An OpenXR-shaped 26-slot representation is acceptable
only if missing and synthesized slots remain explicit. Never fill absent MediaPipe metacarpals
or palm with fictitious measurements merely to complete an array.

Publish at the existing render-to-tick boundary without moving OpenXR calls to an arbitrary
worker. Tick reads one coherent generation and queues ECS updates. Verify the actual command
queue and system ordering before implementing; do not read a partially updated left/right array.
Session stop, reference-space change, provider restart, and rig respawn invalidate cached state.

## Selection and transform ownership

Choose independently for left/right hands. Proposed default hand-data priority is OpenXR then
MediaPipe, with explicit provider selection available. Only fresh, capable samples qualify.
Timeouts, confidence thresholds, recovery hysteresis, and transition duration must be configurable
and measured during implementation, not fixed by this document.

Preserve current per-hand source selection during the first slice: unobstructed tracking owns
the root when valid; controller-derived or unclassified joints retain controller priority.
Controller actions stay active
regardless of which provider supplies skeletal articulation. Controller-driven wrist plus tracked
fingers is a supported combination: derive articulation in the provider's hand-relative frame,
then retarget it under the controller-driven avatar hand. Do not copy unrelated world joint poses.

Select articulation as a coherent hand sample initially. Do not combine individual fingers from
different providers or generations by default. A position-only webcam provider can drive finger
rotations without acquiring authority over avatar wrist translation.

Define one writer for each driven channel: wrist translation, wrist rotation, and each mapped
finger rotation. Animation, authored grip poses, and live tracking need an explicit priority/blend
policy. On loss, release tracking ownership and blend to authored/rest pose; do not freeze a stale
tracked fist forever. Reset source filters at epoch/calibration changes and avoid smoothing twice.

## Rig binding and retargeting

Use the existing owning-GLTF semantic map and retained immutable `BoneRestPose` data. Add full
finger slots or a retained explicit finger map; do not create a competing global name resolver.
Resolve bindings on import/map generation changes, removal, and respawn, with per-bone diagnostics.
Ambiguous selectors remain unresolved and cannot target another avatar instance.

For a common three-bone non-thumb chain, proximal/intermediate/distal drive the three rotating
bones. A tip is an endpoint landmark, not an automatic extra bone. Thumb metacarpal/proximal/distal
needs its own topology mapping. Missing metacarpals and helper/twist bones require explicit policy.
Verify conventions against imported VRoid anatomy rather than match suffix numbers blindly.

First slice: preserve imported bone translations, lengths, and scale; drive rotations only.
Tracked user hand proportions must not stretch the avatar. Determine segment direction from
positions and roll from a stable palm frame or valid provider orientation. Calibrate provider
joint axes against the retained target rest basis. Position-derived direction alone leaves twist
underdetermined; define a palm/previous-frame fallback with confidence and limits.

With column-vector rotation composition, let `B_j` map canonical segment axes into target bone
rest-local axes, and `S_j` map the same canonical axes into the chosen world/hand frame. Then:

```text
desired_target_world_rotation = S_j * inverse(B_j)
desired_target_local_rotation = inverse(actual_parent_world_rotation) * desired_target_world_rotation
```

Provider normalization must produce `S_j` first; raw provider quaternion is not automatically
canonical. Use current-frame desired parent rotations for driven parents and the actual evaluated
parent chain for helper bones. Hand-relative samples must first be composed with the selected
avatar hand world frame. Test multiplication direction against rest-pose reconstruction.

For straight or curled fingers, derive bend/splay with stable roll and continuity. A fully closed
hand must not make wrist orientation depend on its curled middle finger. Separate palm orientation
from per-finger segment orientation, as required by the basis audit.

`JointRetargetBasis` remains the generic rest geometry mechanism. `RestAttachment` remains an
immutable offset: the existing laser does not follow a newly curling fingertip automatically.
Specify a live bone attachment separately if that interaction is wanted; keep runtime aim,
avatar-hand aim, and fingertip position as explicit choices.

## Acceptance criteria

The checked items have deterministic coverage. Live headset/model verification remains below.

- [x] All 26 OpenXR joints reach a coherent retained sample with flags, time, and source space.
- [x] An explicit mapping animates five fingers independently on both sides of the imported mitten rig.
- [x] Flexion, extension, splay, thumb opposition, and fist gestures preserve avatar bone lengths.
- [x] Wrist frame remains stable when only fingers curl, and controller behavior remains correct.
- [x] Missing joints, partial hands, degenerate geometry, stale data, and provider loss release
      ownership predictably without NaNs, sudden flips, or stale poses.
- [x] Controller wrist with hand-relative tracked fingers works without moving the wrist twice.
- [ ] Source switches, session restarts, reference-space resets, and rig respawns invalidate filters
      and bindings correctly; two avatar instances do not cross-resolve.
- [ ] Replay tests cover arbitrary bone axes, helper parents, mirrored hands, and parent update order;
      live hardware/model checks verify visual skinning and source switching.
- [x] MediaPipe is optional at build, startup, and runtime; absence does not block OpenXR operation.

## Follow-up decisions

Tune timeouts/release durations with hardware; evaluate thumb roll and optional native joint twist,
source recovery hysteresis, broader finger automapping, and any live fingertip pointer policy.
Gesture recognition can consume normalized samples later; it is not required for initial fingers.
