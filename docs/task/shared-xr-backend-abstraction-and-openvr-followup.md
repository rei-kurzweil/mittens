# Task: Shared XR backend abstraction follow-up

Date: 2026-06-25

Status: active OpenXR-first cleanup task.

Update: 2026-06-28

- OpenXR parity is now considered restored after the backend-abstraction refactor.
- The attempted OpenVR bring-up did not become the path that restored controller input.
- The actual controller-input breakthrough came from fixing Cat Engine's OpenXR action usage and
  binding suggestion flow.
- The runtime on this machine reported the active interaction profile as
  `/interaction_profiles/oculus/touch_controller`, even though the physical device under test was
  a Vive Focus 3.
- That means the runtime is currently presenting a Touch-style OpenXR interaction profile to the
  app, so Cat Engine must follow the runtime-reported profile rather than infer one from hardware
  branding.
- OpenVR has now been removed from the live engine/runtime path.
- The next priority is cleanup and consolidation around OpenXR-first behavior.

Implementation checklist:

- [x] Introduce an engine-facing `VrSystem` coordinator above backend-specific XR runtime code
- [x] Switch engine/runtime wiring to depend on `VrSystem` instead of directly on `OpenXRSystem`
- [x] Add an `OpenVRSystem` backend slot/placeholder under the shared VR coordinator
- [x] Change the authoring/component API surface to `VR`, `VRHand`, `InputVR`, and `InputVRGamepad`
- [x] Add a real `VrBackend` interface so `VrSystem` dispatches through one backend contract
- [x] Finish packaging current OpenXR behavior as one backend implementation under the shared VR boundary
- [x] Verify OpenXR behavior still works at least as well as before the abstraction refactor
- [ ] Define which additional shared XR state types should live above backend-specific code versus remain backend-local
- [ ] Decide and implement backend health/fallback policy beyond hard init/session failure
- [ ] Add a one-shot init-time report for the runtime-reported active OpenXR interaction profile showing which suggested bindings were accepted vs unsupported
- [x] Switch back off the vendored `openxr` crate
- [x] Decide whether `OpenVRSystem` should be removed entirely now that OpenXR input bring-up is unblocked
- [x] Remove OpenVR runtime/session/input bring-up code now that the project is OpenXR-only again
- [x] Publish real controller button / trigger / analog-stick state through the shared XR input/gamepad surface
- [x] Decide whether the first OpenVR milestone includes full stereo render submission or input-only bring-up
- [x] Make all XR examples default explicitly to `VR.openxr()`
- [ ] Decide whether engine/authored names should revert from `VR`/`VRHand`/`InputVR`/`InputVRGamepad` back to `XR`/`XRHand`/`InputXR`/`InputXRGamepad`
- [ ] If the naming reversion is accepted, update engine/component names, MMS surface names, and examples consistently

Current note:

- shared XR input/gamepad state already lives above `OpenXRSystem`
- backend dispatch now goes through a real trait boundary
- `VrSystem` now resolves to the OpenXR backend only
- the most obvious remaining user-facing gaps are:
  - finish OpenXR binding coverage for the runtime-reported profile
  - clean up temporary debug/investigation scaffolding
  - decide whether the `VR*` authoring names should stay or revert to `XR*`

Cleanup done in this pass:

- removed the `openvr` / `openvr_sys` dependencies
- removed `OpenVRSystem` and its fallback wiring from `VrSystem`
- switched `openxr` back from the vendored crate to the registry crate
- verified the current XR examples already opt into `VR.openxr()` / `VrComponent::openxr()`

---

## 0. 2026-06-28 investigation result

This section records what actually happened during the latest XR input debugging pass.

### What was observed

- OpenVR did not become usable on this machine.
- `openvr::init` repeatedly failed with `VRInitError_Init_InterfaceNotFound`.
- SteamVR-side helper processes also emitted `STEAMVR_VRENV: unbound variable`, which made the
  OpenVR logs noisy and did not produce a reliable runtime path for Cat Engine.
- Meanwhile, OpenXR continued to provide head/hand tracking and a valid session.
- After instrumenting the vendored `openxr` crate, Cat Engine could see the runtime's current
  interaction profile for both hands.
- The runtime reported:
  - `/interaction_profiles/oculus/touch_controller`
- That was true even though the physical headset/controller setup being discussed was Vive Focus 3.

### What this means

- Cat Engine was not "mistakenly hardcoded to Oculus Touch" at the runtime-detection layer.
- The OpenXR runtime itself exposed a Touch-style interaction profile.
- Therefore Cat Engine must trust the runtime-reported interaction profile and bind actions against
  what the runtime says is active.
- Hardware identity and interaction-profile identity are not guaranteed to match in a human-obvious
  way.

### What traces were added to the vendored `openxr` crate

Temporary tracing was added in the vendored `openxr` crate to expose:

- `string_to_path(...)`
- `suggest_interaction_profile_bindings(...)`
- `attach_action_sets(...)`
- `current_interaction_profile(...)`

That instrumentation made three important facts visible:

1. Cat Engine was successfully creating the expected OpenXR paths and actions.
2. The runtime was returning an active interaction profile instead of `NULL`.
3. Some batched binding suggestions failed with `ERROR_PATH_UNSUPPORTED`, which meant one bad
   alias path could poison an entire profile-suggestion batch.

### What actually fixed controller input

The important fixes were in Cat Engine's OpenXR action plumbing, not in OpenVR:

1. `suggest_interaction_profile_bindings(...)` was moved before `attach_action_sets(...)`.
   Action sets become immutable after attach, so suggestion ordering mattered.
2. Binding suggestion errors stopped being silently swallowed.
3. Binding suggestions were changed from "batch all aliases together" to best-effort per-binding
   suggestion, so one unsupported path no longer kills all other bindings for that profile.
4. Controller actions were changed to use explicit left/right subaction paths.
5. Action state polling was changed from `Path::NULL` to the correct per-hand subaction paths.
6. Aim/grip spaces were changed from one shared `Path::NULL` space to separate left/right spaces.

This was the key bug:

- Cat Engine had working action definitions and some working bindings, but it still queried button
  and axis state as if there were no per-hand subaction paths.
- That let the runtime/session exist while leaving most input effectively inert.

### What worked afterward

- Right `ButtonB` started producing real events.
- That was the first proof that the OpenXR action pipeline was now live end-to-end:
  - bindings suggested
  - action set attached
  - actions synced
  - per-hand action state queried correctly
  - engine gamepad events emitted

### What still appears incomplete

- Binding coverage is still incomplete or partially mismatched for the runtime-reported profile.
- At the time of writing:
  - `ButtonB` was confirmed working
  - `A`, `X`, `Y`, triggers, grips, and sticks were not yet all confirmed working
  - aim/grip pose actions still showed invalid in the debug output during the same session

So the current state is:

- OpenXR action plumbing bug: substantially fixed
- OpenXR profile/binding coverage: still incomplete
- OpenVR fallback path: not justified as the primary next step

### Logging cleanup done during the investigation

The debugging pass also removed or gated large amounts of log noise:

- repeated editor/inspector/scroll/layout diagnostics
- repeated OpenXR frame snapshots
- repeated XR input event spam
- repeated vendored-crate profile polling spam

Current env knobs:

- `CAT_OPENXR_TRACE=1`
  - enables vendored `openxr` trace points other than profile-poll spam
- `CAT_OPENXR_TRACE_PROFILE=1`
  - additionally enables vendored `current_interaction_profile(...)` polling logs
- `CAT_OPENXR_DEBUG=1`
  - enables Cat Engine OpenXR debug snapshots / hand / controller-source logs
- `CAT_XR_INPUT_LOG=1`
  - enables `[input_xr_gamepad_system]` axis/button event logs

---

## 0.1 Revised direction after the investigation

The previous assumption behind this task was:

- OpenXR parity is done
- OpenVR is the next practical milestone

The latest investigation changes that assessment.

The revised priority order should be:

1. keep OpenXR as the default and primary runtime path
2. finish OpenXR profile/binding coverage for real hardware/runtime combinations
3. clean up temporary compatibility/debug code
4. decide whether to delete OpenVR code instead of developing it further
5. decide whether the authoring/component API should revert from `VR*` names back to `XR*` names

That means this task is now less about "OpenVR follow-up implementation" and more about
"what to do after learning that OpenXR was the real fix."

This task captures the likely staged XR-engine direction:

- introduce a shared engine-owned XR backend abstraction
- package the current OpenXR-specific logic under that abstraction
- add an OpenVR backend afterwards

This is not a commitment to implement every phase in one PR.
It is the task note that should guide that sequence.

Related context:

- [docs/draft/shared-xr-backend-abstraction-openxr-openvr.md](../draft/shared-xr-backend-abstraction-openxr-openvr.md)
- [docs/task/openxr-controller-actions-and-default-stick-locomotion.md](./openxr-controller-actions-and-default-stick-locomotion.md)
- [docs/task/openxr-parity-gate-before-openvr.md](./openxr-parity-gate-before-openvr.md)
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

That backend boundary work is now done at the structural level, and OpenXR parity is considered
good enough to move forward.

Update as of 2026-06-26:

- `VrSystem` exists as the engine-owned coordinator
- `OpenXRSystem` and `OpenVRSystem` now both sit behind a shared `VrBackend` trait
- shared XR input/gamepad state has been extracted above `OpenXRSystem`

So the backend abstraction is now structurally real, but still incomplete:

- more frame/view/head/hand state may still need a shared backend-neutral shape
- OpenXR-specific render/session internals are still largely embedded in `OpenXRSystem`
- the authored MMS/example layer still needs migration and verification
- `OpenVRSystem` still needs a real runtime/session/input implementation

---

## 2. Main goal

The implementation goal is:

- keep one shared Cat Engine XR model
- keep one shared render path above the runtime boundary
- treat OpenXR and OpenVR as backend adapters rather than separate XR subsystems

The staged order was:

1. define the abstraction
2. package current OpenXR behavior under it
3. add OpenVR under the same abstraction

That order mattered because adding OpenVR first would have strongly encouraged copying the current
`OpenXRSystem` shape rather than decomposing it. The project is now at step 3.

---

## 3. Scope for this task

This task should cover:

- defining the backend abstraction boundary
- defining shared engine-owned XR data types
- deciding which parts of `OpenXRSystem` stay shared versus move behind the backend
- reorganizing current OpenXR code to fit the new boundary
- preparing a clean place for an OpenVR backend module
- updating authored examples to the renamed VR authoring surface once the backend boundary is stable
- defining the minimum viable OpenVR milestone for controller-input testing
- documenting the staged implementation order

It should not cover:

- finishing a complete OpenVR implementation in the same step as the abstraction refactor
- broad renderer redesign unrelated to XR backend separation
- renaming every XR-facing engine concept in the same patch unless needed for the abstraction

The earlier OpenXR controller-action investigation remains useful historical context, but it is
no longer the primary gate on starting OpenVR work.

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

- `InputVR`
- `VRHand`
- `InputVRGamepad`
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

Current state:

- done for backend dispatch and shared XR input/gamepad state
- still open for broader shared frame/view/head/hand state

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

Current state:

- largely done for backend dispatch and ownership boundaries
- still open where OpenXR-specific render/session internals remain inside `OpenXRSystem`

### Phase 3: Add OpenVR backend

Now that OpenXR is successfully sitting behind the shared boundary, the engine should add:

- an OpenVR runtime adapter

That backend should aim to publish the same engine-facing XR state model as OpenXR:

- head pose
- per-hand pose
- controller/gamepad state
- per-eye views
- haptics

This phase should deliberately reuse as much shared XR render/input code as possible.

Immediate sub-phases:

1. minimal runtime/session bring-up
2. controller pose + button/trigger/stick publication
3. authored `InputVRGamepad` verification against real controllers
4. only then decide how much additional OpenVR render/compositor work is required

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

### C. Keep the authored surface honest

The engine-side rename to:

- `VR`
- `InputVR`
- `VRHand`
- `InputVRGamepad`

is already in place, but many MMS/example files still use older XR authoring names such as:

- `OpenXR.on()`
- `InputXR.on()`
- `ControllerXR.new(...)`
- `InputXRGamepad`

That means example migration is now a primary follow-up task, not later cleanup.

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
