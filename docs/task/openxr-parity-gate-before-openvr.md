# Task: OpenXR parity gate before OpenVR implementation

Date: 2026-06-26

Status: completed on 2026-06-28.

Outcome:

- the refactored OpenXR path is now considered back at parity for the backend-abstraction work
- OpenVR runtime bring-up is no longer blocked on this gate
- this note remains as a historical checklist and completion record

This task exists to prevent the backend-abstraction refactor from drifting straight into
OpenVR implementation before the existing OpenXR path is re-verified.

The rule was:

- do not treat OpenVR runtime bring-up as the next primary milestone
- first verify that the refactored OpenXR path still works at least as well as it did before

This was a gate task, not a separate XR direction.

Related context:

- [docs/task/shared-xr-backend-abstraction-and-openvr-followup.md](./shared-xr-backend-abstraction-and-openvr-followup.md)
- [docs/task/openxr-controller-actions-and-default-stick-locomotion.md](./openxr-controller-actions-and-default-stick-locomotion.md)
- [docs/task/openxr-runtime-session-comparison-with-wayvr.md](./openxr-runtime-session-comparison-with-wayvr.md)
- [docs/analysis/openxr-runtime-investigation-matrix.md](../analysis/openxr-runtime-investigation-matrix.md)
- [src/engine/ecs/system/vr_system.rs](../../src/engine/ecs/system/vr_system.rs)
- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)

---

## 1. Why this gate exists

The backend refactor has already changed the structure of XR ownership:

- `VrSystem` now owns backend selection and fallback policy
- `OpenXRSystem` sits behind a `VrBackend` trait
- shared XR input/gamepad state now lives above the OpenXR-specific file boundary

That is the right direction, but structural cleanup is not the same thing as behavioral parity.

The risk is:

- OpenVR work starts
- the abstraction gets more complicated
- and only later do we discover that OpenXR regressed in a subtle way during the packaging step

That is the wrong order.

---

## 2. Definition of parity

"OpenXR parity" here does **not** mean:

- solving every known controller-input/runtime issue
- making Focus 3 controller actions work everywhere
- matching an ideal future XR architecture

It means:

- the refactored code should preserve the same working OpenXR behavior that existed before the
  backend abstraction work

At minimum that means preserving:

1. OpenXR runtime initialization through `VrSystem`
2. OpenXR session creation through `VrSystem`
3. session event pumping / state transitions
4. HMD pose publication
5. hand tracking / hand-root fallback publication
6. controller pose publication for `VRHand`
7. current XR render path
8. current `InputVRGamepad` publication shape
9. example-scene topology assumptions that depend on the OpenXR path

---

## 3. Verification checklist

### A. Non-runtime/code-path checks

- [x] Engine compiles with `VrSystem` as the XR entry point
- [x] `OpenXRSystem` still implements the full backend contract used by `VrSystem`
- [x] shared XR input/gamepad state remains readable from the coordinator
- [x] authored example `.mms` files under `examples/` use the current VR surface
- [ ] authored data/world `.mms` files under `assets/data/` use the current VR surface where intended
- [ ] docs/spec/task notes stop teaching obsolete `OpenXR` / `InputXR` / `ControllerXR` names when referring to the public authored surface

### B. Runtime bring-up checks

- [ ] `VR.on()` with default backend preference still initializes OpenXR when OpenXR is the preferred backend
- [ ] `VR.openxr()` still forces OpenXR selection
- [ ] `VrSystem::active_backend_kind()` reports `OpenXR` when OpenXR is active
- [ ] `VrSystem::last_backend_error()` remains empty on successful OpenXR startup

### C. Session and pose checks

- [ ] OpenXR reaches the expected usable session states (`READY`, `VISIBLE`, `FOCUSED`) when runtime conditions allow
- [ ] HMD pose still drives the `InputVR` direct child transform as before
- [ ] `CameraXR` still renders from the active XR rig selected through the same authored topology assumptions
- [ ] `VRHand` grip/aim pose driving still updates child transforms as before
- [ ] hand tracking still initializes when available
- [ ] hand-root fallback still works when full hand tracking is unavailable

### D. Render checks

- [ ] `render_xr()` still publishes eye views into `VisualWorld`
- [ ] stereo rendering still submits through the OpenXR path
- [ ] mirror / desktop companion rendering still behaves as before for the example scenes that use it
- [ ] renderer stats that depend on XR frame timing still update as before

### E. Input/gamepad checks

- [ ] `VrSystem::xr_input_state()` still resets/updates correctly across runtime availability
- [ ] `VrSystem::xr_gamepad_state()` still preserves the current publication shape
- [ ] `InputVRGamepadSystem` still receives the same shared XR gamepad snapshot shape
- [ ] authored `InputVRGamepad` event flows still work wherever they worked before the refactor

### F. Example checks

- [ ] `examples/vr-input.mms` still expresses the intended XR topology after migration
- [ ] `examples/bisket-vr-demo.mms` still expresses the intended XR topology after migration
- [ ] `examples/vtuber-mirror-example.mms` still expresses the intended XR topology after migration
- [ ] `examples/vtuber-editor-example.mms` still expresses the intended XR topology after migration
- [ ] `examples/input-xr-gamepad.mms` still expresses the intended XR topology after migration

---

## 4. Exit criteria

OpenXR is considered "back at parity for the abstraction refactor" when:

- the authored surface migration needed for OpenXR examples is complete enough to avoid teaching obsolete names
- the coordinator/backend split is verified not to have broken the working OpenXR paths
- the remaining known OpenXR issues are the same old runtime/input issues, not new regressions introduced by the backend packaging

This gate is now considered satisfied. After that, the engine can treat:

- real OpenVR runtime/session/input/render implementation

as the next primary milestone.

---

## 5. Next steps after completion

The next useful steps are:

1. implement minimal real OpenVR runtime/session bring-up
2. publish OpenVR controller buttons, triggers, and analog sticks through the shared XR input/gamepad state
3. verify authored `InputVRGamepad` flows against real OpenVR controller input
4. decide whether the first OpenVR milestone also includes full stereo render submission
