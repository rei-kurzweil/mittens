# Pointer activations and XR trigger state

This document describes the runtime mechanism for feeding controller trigger / select actions and
other non-mouse activation sources into `GestureSystem`.

It extends `docs/draft/pointer.md`, which covers authored topology and source resolution.
That draft establishes the intended pairing between each `PointerComponent` and its trigger source.
This document specifies what those pairings look like at runtime.

## Problem statement

`GestureSystem::tick_with_rx` currently reads `InputState.mouse_pressed / down / released`
(winit `MouseButton`) directly to decide when to open, continue, and close a drag gesture.
There is no path for a VR controller trigger or a gaze dwell timer to drive those same decisions.

The fix should not:

- couple `GestureSystem` to XR types or OpenXR concepts
- require `system_world` to know the mapping from controllers to pointers
- make `GestureSystem` branch on "is this XR or mouse" at every gesture decision point

## Two parallel input-state objects

### `InputState` (existing, winit-derived)

Lives in `engine::user_input`. Contains edge-triggered sets (`mouse_pressed`, `mouse_released`)
and level-triggered sets (`mouse_down`) for `MouseButton`.

This is not going away. Desktop gesture triggering continues to read from it.

### `XrInputState` (new, XR-derived)

A parallel struct, owned by `OpenXRSystem`, that captures per-hand trigger state in the same
pressed / down / released idiom:

```rust
/// Per-frame trigger activation state for XR controllers.
///
/// Mirrors the edge-triggered / level-triggered structure of `InputState`
/// for mouse buttons, but for XR select actions.
///
/// Populated by `OpenXRSystem` each frame by polling the OpenXR select action
/// for each active controller hand.
#[derive(Default, Debug, Clone)]
pub struct XrInputState {
    /// Trigger newly pressed this frame (rising edge).
    pub trigger_pressed: [bool; 2],  // index 0 = left, 1 = right
    /// Trigger held (level).
    pub trigger_down: [bool; 2],
    /// Trigger newly released this frame (falling edge).
    pub trigger_released: [bool; 2],
}
```

Index convention follows `ControllerHand`: 0 = left, 1 = right.

`OpenXRSystem` needs a new `select` boolean action added to `ControllerInput` alongside the
existing `aim_pose` / `grip_pose`. It polls that action each frame inside `tick_with_queue` and
writes the result into `XrInputState`, managing the edge transitions the same way `UserInput`
does for `mouse_pressed`.

### What "select" means in OpenXR

`select` here should mean an **OpenXR boolean input action**, not an OpenXR event name.

In practice:

- the engine creates a boolean action on the existing XR action set
- it suggests bindings for that action to the runtime's interaction profiles
- each frame it calls `xrSyncActions` and reads the action via `xrGetActionStateBoolean`
- `XrInputState` is then derived from the action's current boolean state and edge transitions

For the Khronos simple controller profile, the semantic component path is the hand's select click:

- `/user/hand/left/input/select/click`
- `/user/hand/right/input/select/click`

This matches how `OpenXRSystem` already uses the action system today for `aim_pose` and
`grip_pose`, and already suggests bindings for `/interaction_profiles/khr/simple_controller`
plus several concrete controller profiles.

`XrInputState` is exposed as a read accessor on `OpenXRSystem`:

```rust
impl OpenXRSystem {
    pub fn xr_input_state(&self) -> &XrInputState { &self.xr_input_state }
}
```

It is reset to default each frame when XR is not running.

## `PointerActivations`: the bridge

`PointerSystem` owns a `PointerActivations` struct that it builds each frame. It is the single
place that maps from raw input states (both `InputState` and `XrInputState`) to the set of
`PointerComponent` ids that are pressed, held, or released.

```rust
/// Which PointerComponent ids have an active trigger state this frame.
///
/// Built by `PointerSystem::build_activations` from `InputState` (mouse)
/// and `XrInputState` (XR controllers).
///
/// `GestureSystem` consumes this to drive drag/click lifecycle per pointer,
/// without knowing anything about the underlying input source.
#[derive(Default, Debug, Clone)]
pub struct PointerActivations {
    pub pressed:  Vec<ComponentId>,
    pub down:     Vec<ComponentId>,
    pub released: Vec<ComponentId>,
}
```

`PointerSystem` builds this in a new `build_activations` method called before `gesture.tick_with_rx`:

```rust
impl PointerSystem {
    pub fn build_activations(
        &self,
        world: &World,
        input: &InputState,
        xr: &XrInputState,
    ) -> PointerActivations {
        let mut act = PointerActivations::default();

        for (&pointer_cid, _) in &self.pointer_to_raycast {
            let topology = /* cached or recomputed PointerTopologyContext for pointer_cid */;

            if topology.has_controller_driver {
                // Map controller hand → XrInputState index.
                if let Some(hand_idx) = controller_hand_index(world, pointer_cid) {
                    if xr.trigger_pressed[hand_idx]  { act.pressed.push(pointer_cid); }
                    if xr.trigger_down[hand_idx]     { act.down.push(pointer_cid); }
                    if xr.trigger_released[hand_idx] { act.released.push(pointer_cid); }
                }
            } else {
                // Desktop / camera-anchored pointer: use mouse left button.
                if input.mouse_pressed.contains(&MouseButton::Left)  { act.pressed.push(pointer_cid); }
                if input.mouse_down.contains(&MouseButton::Left)     { act.down.push(pointer_cid); }
                if input.mouse_released.contains(&MouseButton::Left) { act.released.push(pointer_cid); }
            }
        }

        act
    }
}
```

`PointerSystem` already owns `pointer_to_raycast` and can walk component topology.
`PointerTopologyContext` classification already exists in `raycast_system`; it should move to or
be shared with `PointerSystem` so `build_activations` can use it without crossing system boundaries.

## Changes to `GestureSystem`

`tick_with_rx` stops reading `input.mouse_pressed` etc. directly and instead iterates
`PointerActivations`. The gesture state machine runs once per pointer in `activations.pressed`,
using `PointerSystem::raycast_for_pointer` to map to the raycaster that `RayIntersected` events
were tagged with.

```rust
pub fn tick_with_rx(
    &mut self,
    visuals: &VisualWorld,
    input: &InputState,          // kept for screen-space cursor position only
    activations: &PointerActivations,
    pointer_system: &PointerSystem,
    rx: &mut RxWorld,
) { ... }
```

`input` is still passed because `cursor_pos` is needed to compute `screen_pos_px` on `DragStart`
and `screen_delta_px` on `DragMove` for desktop pointers. For XR pointers those fields will be
`None`.

### Multi-pointer gesture state

Currently `GestureSystem` has a single `GestureState`. With `PointerActivations` driving the loop,
per-pointer state is needed:

```rust
pub struct GestureSystem {
    states: HashMap<ComponentId, GestureState>,  // keyed by PointerComponent id
    ...
}
```

For now it is acceptable to only allow one pointer to start a gesture per frame (first-wins), but
the state map should be keyed by pointer from the start to avoid a bigger refactor later.

## Call site in `system_world`

```rust
let activations = self.pointer.build_activations(world, input, self.openxr.xr_input_state());
self.gesture.tick_with_rx(visuals, input, &activations, &self.pointer, &mut self.rx);
```

`system_world` passes through pre-built data; it does not make policy decisions about which
pointer maps to which trigger. That responsibility stays in `PointerSystem`.

## Dwell / gaze activation (future)

A gaze dwell timer would produce its own activation signal via the same `PointerActivations`
interface. `PointerSystem::build_activations` would check whether a pointer has a `has_xr_input_driver`
with no controller driver and apply a dwell threshold against elapsed hover time.

No changes to `GestureSystem` would be needed for that path.

## Summary of responsibilities

| Component | Responsibility |
|---|---|
| `OpenXRSystem` | Polls XR select action; writes `XrInputState` (pressed/down/released per hand) |
| `PointerSystem` | Owns `build_activations`; maps pointer topology → correct input source |
| `GestureSystem` | Consumes `PointerActivations`; drives drag/click per pointer; stays source-agnostic |
| `system_world` | Passes `xr_input_state()` to `build_activations`; passes result to gesture tick |

## What needs to change in source

1. **Confirm the XR action semantics and binding paths.**
   Verify that "select" is modeled as a boolean input action read through the action system, not
   as an event. Use the existing interaction-profile setup in `openxr_system.rs` and add select
   bindings for the supported profiles, starting with the simple-controller semantic paths
   `/user/hand/left|right/input/select/click`.

2. **Add `XrInputState` to `OpenXRSystem`.**
   Extend `ControllerInput` with a boolean `select` action, keep per-hand previous state, and
   expose a read accessor such as `xr_input_state()`. Reset to default when XR is unavailable or
   not running.

3. **Poll and derive trigger edges in `OpenXRSystem`.**
   During the normal XR tick, call `xrSyncActions`, read `xrGetActionStateBoolean` per hand, and
   populate `trigger_pressed`, `trigger_down`, and `trigger_released`.

4. **Add `PointerActivations` plus a desktop-only bridge in `PointerSystem`.**
   Introduce `PointerActivations` and `build_activations(world, input, xr)`, but initially make it
   reproduce current desktop behavior only. This establishes the architectural seam before XR is
   allowed to change runtime behavior.

5. **Move or share pointer-topology classification.**
   `PointerSystem` needs the same lineage classification currently living in
   `raycast_system.rs` (`PointerTopologyContext`) so it can decide whether a pointer should be
   paired with desktop mouse input, controller select input, or future gaze/dwell input.

6. **Refactor `GestureSystem` to consume `PointerActivations`.**
   Replace direct reads of `input.mouse_pressed / down / released` with activation-driven
   iteration. Change the internal state from a single `GestureState` to per-pointer state keyed by
   `PointerComponent` id.

7. **Wire controller-backed pointers to XR activations.**
   Resolve controller hand ownership via `ControllerXRComponent` ancestry and map left/right hands
   onto the correct pointer ids in `PointerActivations`.

8. **Handle XR raycast triggering for event-driven raycasters.**
   Current `RayCastMode::EventDriven` auto-casts only for desktop cursor-through-camera pointers.
   XR pointer activation is therefore not sufficient by itself. Decide and implement one of:
   XR-driven cast requests, continuous XR pointer raycasts, or a generalized pointer-owned cast
   trigger policy.

9. **Constrain the first XR gesture mode.**
   `GestureSystem::StartPlaneProjection` currently reconstructs drag motion from desktop cursor
   rays. For the first XR pass, either restrict XR pointers to `RequireTargetContact` or add a
   per-pointer ray reconstruction path that does not depend on `InputState.cursor_pos`.

10. **Leave dwell / gaze activation as a follow-up on the same seam.**
    Once pointer activations exist, dwell can be added as another activation producer without
    changing the gesture state machine again.
