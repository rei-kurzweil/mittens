# Draft: Shared XR backend abstraction for OpenXR and OpenVR

Date: 2026-06-25

Status: draft only. This is an architectural direction note, not a committed implementation task.

This note captures a likely direction after the recent XR investigation:

- Cat Engine should probably support both OpenXR and OpenVR
- but it should not grow two largely separate XR engines
- most engine-facing XR logic should be shared
- only runtime-specific session/input/submission details should differ

The goal is:

- one engine-owned XR abstraction
- one shared renderer and input surface above it
- multiple runtime backends below it

---

## 1. Why this draft exists

Recent investigation suggests:

- Cat Engine's current OpenXR path can create a session and receive head/hand data
- but controller interaction profiles/actions are still not activating in the desired path
- local WayVR testing shows a working controller path on this machine, but currently through OpenVR fallback rather than a usable OpenXR overlay path

So the engine should be prepared for the possibility that:

- OpenXR remains the preferred path
- OpenVR is still needed as a practical fallback on some setups

That should not automatically imply:

- duplicating renderer logic
- duplicating engine-level input systems
- duplicating authored XR behavior

Instead, it argues for a shared backend abstraction.

---

## 2. The intended split

The clean split is:

### Shared engine-facing XR layer

This layer should define the runtime-independent data the engine actually wants:

- head pose
- per-hand pose state
- per-hand controller/gamepad state
- per-eye view/projection data
- haptic requests
- frame lifecycle hooks at an engine-friendly level

This is the layer consumed by:

- `InputXR`
- `ControllerXRComponent` / future `InputXrHand`
- `InputXRGamepad`
- AVC
- MMS event/input systems
- shared XR rendering code

### Runtime backend adapters

Each backend should translate its own runtime API into the shared engine XR layer:

- `OpenXRBackend`
- `OpenVRBackend`

Those backends should own:

- runtime instance/session/compositor startup
- action or controller input polling
- runtime-specific pose acquisition
- runtime-specific swapchain/compositor image handling
- haptics submission
- frame submission

### Shared XR render layer

Above the backend adapters, Cat Engine should keep one shared XR render flow for:

- building eye views
- rendering the scene for XR eyes
- sharing render graph / pipeline setup as much as possible
- applying engine-owned world/view conventions consistently

This render layer should not care whether the backend is OpenXR or OpenVR except at the thin
image-acquire / image-submit boundary.

---

## 3. What should stay shared

The following should ideally remain backend-independent:

- XR camera/view derivation at the engine level
- ECS-driven head/controller/hand transform publication
- `InputXRGamepad`-style state shape
- higher-level authored behavior such as locomotion and pointer logic
- most shader/pipeline/material/render-graph logic
- most mirror-view and scene-render logic
- higher-level haptics intent generation

If these are duplicated between OpenXR and OpenVR implementations, the architecture is probably
too low-level from the engine's point of view.

---

## 4. What should remain backend-specific

The following are expected to differ materially:

- runtime initialization
- instance/system/session/compositor creation
- frame timing APIs
- per-eye image acquisition/release
- compositor submission structs and calls
- controller input polling APIs
- controller role/device discovery
- haptic output APIs
- runtime-specific extension/feature negotiation

These differences are real and should not be papered over inside one giant monolithic XR system.

They should be isolated behind the backend adapter boundary.

---

## 5. The practical renderer boundary

It is tempting to say:

- "the XR renderer should be the same"

That is mostly true, but only above the compositor boundary.

### Shared render concerns

- compute view/projection for each eye
- render scene into backend-provided XR render targets
- reuse the same pipeline creation and draw logic
- preserve the same engine camera/world conventions

### Backend-specific render concerns

- how eye images are acquired
- what image handles/formats/usages are required
- how depth/color layers are submitted
- how frame begin/end is synchronized with the runtime

So the correct statement is:

- the scene-rendering logic should be mostly shared
- the swapchain/compositor plumbing should remain backend-specific

---

## 6. Suggested abstraction shape

The exact trait/API should be designed later, but conceptually the backend boundary wants something
like:

- `begin_frame()`
- `views()` or `predicted_views()`
- `head_pose()`
- `hand_state(left/right)`
- `gamepad_state(left/right)`
- `acquire_eye_images()`
- `submit_frame(...)`
- `apply_haptics(...)`

This should expose engine-owned data types, not raw runtime structs wherever possible.

For example:

- `XrViewState`
- `XrHeadState`
- `XrHandState`
- `XrGamepadState`
- `XrFrameTargets`

The goal is:

- runtime APIs below the line
- engine APIs above the line

---

## 7. Migration implication for Cat Engine

Cat Engine's current `OpenXRSystem` likely mixes several layers together:

- runtime/session ownership
- controller input/runtime polling
- engine-facing state publication
- XR render integration

If multi-backend XR becomes a goal, this probably needs to be decomposed into:

- shared engine XR state/publication layer
- backend runtime adapter(s)
- shared XR render layer

This does **not** mean rewriting everything immediately.

It means future XR work should avoid deepening the assumption that:

- "OpenXRSystem is the one true place where all XR concerns live forever"

---

## 8. Why this is worth doing

If successful, this approach gives:

- OpenXR and OpenVR support without duplicating most engine logic
- cleaner debugging boundaries
- fewer authored-behavior differences across runtimes
- a clearer path for future runtime-specific fallbacks

It also makes investigation easier:

- runtime bugs live in backend adapters
- engine behavior bugs live in shared XR/input/render layers

That separation is valuable even before both backends are fully implemented.

---

## 9. Non-commitment

This note does **not** commit Cat Engine to:

- implementing OpenVR immediately
- preserving the current `OpenXRSystem` API
- choosing a final trait/object boundary today
- refactoring the renderer right now

It only records the likely architectural direction:

- shared XR engine layer
- backend-specific runtime adapters
- minimal duplication above the compositor/input boundary
