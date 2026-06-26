# Task: AvatarControl desktop/VR facing and eye-offset unification

Date: 2026-06-26

Status: active design and bug task.

This task exists because the current `AvatarControlComponent` public authoring
surface still exposes too much engine-internal convention work to examples and
games.

Current symptom:

- a desktop first-person avatar setup and a VR first-person avatar setup can
  require different authored forward/yaw conventions even when they are meant
  to represent the same humanoid and the same intended facing direction
- camera wrapper offsets can appear to need opposite signs across desktop vs VR
- after fixing the desktop camera to stop looking into or through the head, the
  avatar can still be visibly backward relative to the desktop camera and
  mirror output

That is the wrong abstraction boundary.

Game/example authors should not have to solve:

- whether "forward" is `+Z` or `-Z`
- whether `initial_yaw` must be `0` or `pi`
- whether a first-person camera wrapper should use `+offset_z` or `-offset_z`
- whether desktop and XR need different authored conventions for what should be
  the same avatar-facing result

The engine should make the default case work.

Related context:

- [docs/task/avatar-control-head-driven-redesign.md](./avatar-control-head-driven-redesign.md)
- [docs/task/avatar-control-desktop-vs-vr-divergence.md](./avatar-control-desktop-vs-vr-divergence.md)
- [docs/task/skinned-humanoid-avc-calibration.md](./skinned-humanoid-avc-calibration.md)
- [examples/vtuber-mirror-example.mms](../../examples/vtuber-mirror-example.mms)
- [src/engine/ecs/component/avatar_control.rs](../../src/engine/ecs/component/avatar_control.rs)
- [src/engine/ecs/system/avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)

---

## 1. Problem statement

Today AVC still leaks internal driver- and runtime-specific orientation details
into authored content.

The visible authoring burden is:

- `forward_plus_z()`
- `initial_yaw(...)`
- manual camera-wrapper translation tuning

Those fields exist because the engine currently needs help reconciling:

- desktop `Input`
- XR `InputVR`
- VRM-style model rest orientation
- HMD/camera eye offset semantics
- body-follow and head-target conventions

But that burden should primarily live inside the engine, not in examples.

The current mismatch is easy to reproduce:

1. add both a desktop first-person rig and an XR rig to the same scene
2. use the same avatar and similar camera-wrapper semantics
3. fix the desktop camera so it is not inside the face mesh
4. observe that the desktop avatar can still be facing backward relative to the
   expected movement/view convention

That means the engine is not yet presenting one consistent "avatar forward"
contract across desktop and VR.

Primary proof scene for this task:

- [examples/vtuber-mirror-example.mms](../../examples/vtuber-mirror-example.mms)

That scene should be the first place where desktop and XR are compared under
one shared environment while this task is investigated.

---

## 2. Desired behavior

For a normal humanoid avatar and a normal first-person setup, authored content
should be able to say, in effect:

- this is the head bone
- this is the camera bone
- this is the left hand bone
- this is the right hand bone
- this wrapper transform is the eye offset relative to the head pivot

and have the engine do the rest.

By default, the engine should ensure:

- desktop and VR produce the same intuitive avatar-facing direction
- the camera sits on the correct side of the head pivot
- movement-forward and avatar-forward agree
- mirrors do not reveal that the desktop avatar is facing backward unless the
  author explicitly asked for that

The public default should be:

- correct first-person orientation without requiring explicit forward-axis or
  yaw-fix authoring

If someone wants unusual behavior, that should be an override, not the baseline.

---

## 3. What should become engine-owned

The engine should own the logic that answers:

- how to map desktop driver forward vs XR driver forward into one avatar-facing
  convention
- how to interpret the camera wrapper translation relative to the head pivot
  consistently across desktop and XR
- how body-follow and head-target offsets should differ internally without
  forcing authored scenes to encode the difference

In practice, that likely means reducing or hiding the need for public authoring
choices around:

- `forward_plus_z`
- `initial_yaw`
- driver-specific forward-axis assumptions
- sign-flip experiments for camera-wrapper Z offsets

These may still exist internally, but they should stop being routine authoring
requirements for the common case.

---

## 4. Immediate bug evidence

Current concrete evidence from `vtuber-mirror-example`:

- adding a desktop AVC rig alongside the XR rig initially produced a first-
  person camera that appeared behind the avatar head
- flipping the desktop camera-wrapper Z offset avoided the camera/head clipping
  problem
- but then the avatar was visibly backward in the mirror for the desktop rig

That combination strongly suggests:

- fixing eye placement and fixing facing direction are still entangled in the
  current desktop AVC path
- authored camera-wrapper offsets are currently compensating for engine-facing
  convention mismatches

This should be treated as an AVC contract problem, not just per-example tuning.

---

## 5. First proof step

The first comparison step should be:

- use `vtuber-mirror-example` as the proof scene
- remove explicit authored facing-direction overrides from the desktop/VR AVC
  comparison path there
- compare default AVC behavior against default AVC behavior

Concretely, for the investigation scene this means:

- no `forward_plus_z()`
- no explicit `initial_yaw(...)`

for the comparison pass, so we can observe what the engine's default contract
actually is.

This is important because override-vs-override comparisons only tell us whether
our manual scene tuning differs, not whether the engine default is coherent.

This step should be done in the proof/calibration scene first, not as a blind
repo-wide migration.

---

## 6. Main goal

Unify desktop and VR humanoid first-person AVC behavior so that:

1. the same avatar-facing intent yields the same result across both drivers
2. camera wrapper offsets mean the same thing across both drivers
3. examples no longer need to manually discover driver-specific forward/yaw
   corrections for common humanoid setups

This is a behavior and API contract task, not just a single-scene bug fix.

---

## 7. Suggested phase plan

### Phase 1: Reproduce and instrument the mismatch

Use `vtuber-mirror-example` as the primary proof scene, then the planned
calibration example as the follow-up deeper instrumented scene.

The first pass should inspect:

- desktop camera world forward
- XR camera world forward
- avatar head bone world forward
- body/model_root world forward
- movement-forward direction
- camera-wrapper local translation and the resulting effective eye position

The point is to make the mismatch measurable, not just visible.

The first pass should explicitly compare:

- default desktop AVC behavior
- default XR AVC behavior

without authored `forward_plus_z()` / `initial_yaw(...)` compensation in the
proof scene.

### Phase 2: Define one engine-facing convention

Choose a single engine-owned contract for:

- what "avatar forward" means
- what "eye offset relative to head pivot" means
- how desktop and XR drivers are normalized into that contract

This should be defined independently from current public knobs.

### Phase 3: Push driver/runtime differences downward

Refactor AVC so desktop-vs-XR differences are handled inside the system rather
than being encoded through public scene-level yaw/forward workarounds.

Possible outcomes include:

- deriving driver-specific orientation compensation automatically
- deriving model-facing compensation from rig/rest data
- treating camera-wrapper translation as one stable semantic input regardless of
  driver type

### Phase 4: Simplify the authoring API

After the behavior is unified, reduce the need for authored:

- `forward_plus_z()`
- `initial_yaw(...)`

for normal humanoid first-person setups.

Those knobs may remain as advanced escape hatches, but they should not be
necessary for ordinary scenes.

Important constraint:

- do not remove these knobs from the component or engine API

They are still useful as explicit overrides and debugging tools.

The goal is:

- stop requiring them for common examples

not:

- delete the capability entirely

---

## 8. Non-goals

This task is not primarily about:

- solving every arm IK calibration issue for every humanoid model
- removing all AVC configurability
- forbidding intentionally unusual avatar orientation setups

It is specifically about making the default desktop/VR first-person path sane
and consistent.

---

## 9. Acceptance criteria

This task is complete when:

- the engine presents one consistent first-person facing convention across
  desktop and VR for normal humanoid AVC setups
- camera-wrapper eye offsets have one stable meaning across desktop and VR
- examples no longer need routine manual `forward_plus_z` / `initial_yaw` /
  sign-flip tuning to avoid backward avatars or backward eye placement
- `vtuber-mirror-example` has been used as the proof scene to demonstrate the
  mismatch and the eventual corrected default behavior
- the desktop rig in `vtuber-mirror-example` (or its replacement calibration
  scene) behaves correctly by default without exposing this convention-fixing
  burden to the scene author

The desired end state is:

- examples declare intent
- the engine handles convention reconciliation
