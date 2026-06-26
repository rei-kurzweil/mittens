# Task: Shared XR backend abstraction and OpenVR follow-up

Date: 2026-06-25

Status: planned implementation task.

Implementation checklist:

- [x] Introduce an engine-facing `VrSystem` coordinator above backend-specific XR runtime code
- [x] Switch engine/runtime wiring to depend on `VrSystem` instead of directly on `OpenXRSystem`
- [x] Add an `OpenVRSystem` backend slot/placeholder under the shared VR coordinator
- [x] Change the authoring/component API surface to `VR`, `VrHand`, `InputVR`, and `InputVrGamepad`
- [ ] Finish packaging current OpenXR behavior as one backend implementation under the shared VR boundary
- [ ] Define which shared XR state types should live above backend-specific code versus remain backend-local
- [ ] Decide and implement backend health/fallback policy beyond hard init/session failure
- [ ] Implement actual OpenVR runtime/session/input/render bring-up behind the same abstraction
- [ ] Verify OpenXR behavior still works at least as well as before the abstraction refactor
- [ ] Update and verify example scenes against the new VR authoring surface

This task captures the likely staged XR-engine direction:

- introduce a shared engine-owned XR backend abstraction
- package the current OpenXR-specific logic under that abstraction
- add an OpenVR backend afterwards

This is not a commitment to implement every phase in one PR.
It is the task note that should guide that sequence.

Related context:

- [docs/draft/shared-xr-backend-abstraction-openxr-openvr.md](../draft/shared-xr-backend-abstraction-openxr-openvr.md)
- [docs/task/openxr-controller-actions-and-default-stick-locomotion.md](./openxr-controller-actions-and-default-stick-locomotion.md)
- [docs/task/openxr-runtime-session-comparison-with-wayvr.md](./openxr-runtime-session-comparison-with-wayvr.md)
- [docs/task/xr-gamepad-and-hand-input-refactor.md](./xr-gamepad-and-hand-input-refactor.md)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- [src/engine/graphics/xr_renderer.rs](../../src/engine/graphics/xr_renderer.rs)

---

## 1. Why this task exists

Recent XR investigation suggests two things at once:

- OpenXR should remain a first-class path
- OpenVR may still be needed as a practical fallback on some setups

That should not lead to:

- duplicating most XR rendering logic
- duplicating most engine-facing input state logic
- duplicating authored XR behavior

Instead, Cat Engine should likely move toward:

- one engine-facing XR abstraction
- one shared XR render/input publication layer
- multiple backend adapters underneath it

The current `OpenXRSystem` already contains several layers mixed together:

- runtime/session lifecycle
- frame timing
- controller/hand polling
- pose publication
- XR render integration

So before adding OpenVR, the engine should first create a cleaner backend boundary.

---

## 2. Main goal

The implementation goal is:

- keep one shared Cat Engine XR model
- keep one shared render path above the runtime boundary
- treat OpenXR and OpenVR as backend adapters rather than separate XR subsystems

The staged order should be:

1. define the abstraction
2. package current OpenXR behavior under it
3. add OpenVR under the same abstraction

That order matters because adding OpenVR first would strongly encourage copying the current
`OpenXRSystem` shape rather than decomposing it.

---

## 3. Scope for this task

This task should cover:

- defining the backend abstraction boundary
- defining shared engine-owned XR data types
- deciding which parts of `OpenXRSystem` stay shared versus move behind the backend
- reorganizing current OpenXR code to fit the new boundary
- preparing a clean place for an OpenVR backend module
- documenting the staged implementation order

It should not cover:

- fully solving the current Focus 3 OpenXR controller-action bug first
- finishing a complete OpenVR implementation in the same step as the abstraction refactor
- broad renderer redesign unrelated to XR backend separation
- renaming every XR-facing engine concept in the same patch unless needed for the abstraction

---

## 4. Intended architecture

The intended split is:

### A. Shared engine XR layer

This layer should own the runtime-independent state the engine actually consumes:

- head pose
- per-hand pose state
- per-hand controller/gamepad state
- per-eye view/projection state
- haptics intents
- engine-facing frame lifecycle state

This is the layer used by:

- `InputXR`
- `ControllerXRComponent` / future `InputXrHand`
- `InputXRGamepad`
- AVC
- MMS event/input systems

### B. Shared XR render layer

This layer should stay responsible for:

- building eye views in engine terms
- rendering scenes into backend-provided eye targets
- reusing the same pipelines/materials/render graph where possible

This layer should not care about OpenXR vs OpenVR except at the thin image-acquire /
image-submit boundary.

### C. Runtime backend adapters

This is where:

- `OpenXRBackend`
- `OpenVRBackend`

would live.

They should own:

- runtime initialization
- session/compositor lifecycle
- runtime-specific frame timing
- pose polling
- controller input polling
- swapchain/compositor interop
- haptics submission
- frame submission

---

## 5. Phase plan

### Phase 1: Define the abstraction

The first code step should be to define shared engine-facing XR types and backend boundaries.

That likely means introducing some combination of:

- shared `XrViewState`
- shared `XrHeadState`
- shared `XrHandState`
- shared `XrGamepadState`
- shared `XrFrameTargets`
- a backend trait or equivalent adapter boundary

The exact Rust shape does not need to be finalized in this task note, but the boundary must be
clear enough that backend-specific runtime structs stop leaking upward.

### Phase 2: Package current OpenXR under the abstraction

Once the boundary exists:

- move current OpenXR-specific lifecycle/input/submission behavior behind it
- keep engine-facing XR behavior stable as much as possible
- preserve the current working OpenXR features:
  - HMD pose
  - hand tracking / hand-root fallback
  - current XR rendering path
  - current `InputXRGamepad` publication shape

The point of this phase is not feature expansion. It is isolation and packaging.

### Phase 3: Add OpenVR backend

Only after OpenXR is successfully sitting behind the shared boundary should the engine add:

- an OpenVR runtime adapter

That backend should aim to publish the same engine-facing XR state model as OpenXR:

- head pose
- per-hand pose
- controller/gamepad state
- per-eye views
- haptics

This phase should deliberately reuse as much shared XR render/input code as possible.

---

## 6. Important design constraints

### A. Avoid duplicating engine-facing state logic

We should not end up with:

- one OpenXR-only gamepad state model
- one OpenVR-only gamepad state model

The runtime adapters should translate into one shared model.

### B. Avoid duplicating XR renderer logic

We should not end up with:

- one OpenXR renderer path
- one OpenVR renderer path

that both separately recreate the same scene rendering logic.

The renderer split should be:

- shared scene rendering above
- backend-specific swapchain/compositor submission below

### C. Avoid deepening `OpenXRSystem` as the permanent center

If the engine wants multi-backend XR support, `OpenXRSystem` should no longer be the place where
all XR assumptions continue to accumulate.

It should become:

- one backend implementation under a shared XR layer

not the root of the entire future design.

---

## 7. Suggested deliverable order

The deliverables should land in this order:

1. docs/spec or draft update that makes the backend boundary explicit if needed
2. code refactor introducing shared XR abstraction types
3. code refactor moving current OpenXR behavior behind that boundary
4. verification that current OpenXR behavior still works at least as well as before
5. separate follow-up implementation for OpenVR backend

The OpenVR backend should be a follow-up deliverable, not mixed into the first abstraction patch
unless the abstraction is extremely small and already proven.

---

## 8. Completion criteria

This task is complete when:

- Cat Engine has a documented and code-level XR backend boundary
- current OpenXR behavior is packaged as one backend implementation under that boundary
- the shared engine-facing XR state/render layer is no longer implicitly OpenXR-specific
- there is a clear follow-up path for adding OpenVR without copying most XR engine logic

This task does **not** require OpenVR to be fully implemented to be considered complete.

That should be treated as the next major follow-up once the abstraction is in place.
