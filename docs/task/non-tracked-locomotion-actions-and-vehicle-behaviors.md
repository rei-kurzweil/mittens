# Non-tracked locomotion actions and vehicle behavior assets

Status: design proposal, 2026-09-14. No engine or MMS-language implementation
is implied by this document.

## Problem

`mittens-corp.mms` and `mittens-corp-desktop.mms` demonstrate the same mounted
car behavior through different input surfaces:

- OpenXR supplies a continuous left-stick vector and scoped button events via
  `InputXRGamepad`.
- Desktop supplies individual W/A/S/D/Space keyboard edges through global MMS
  signals; the scene retains its own held-key state.

Both produce the same semantic values: steering, throttle, and a one-shot
fire action. Neither value comes from tracked head, hand, or controller pose.
The differences are device acquisition and button-to-axis composition, not car
physics or effects.

This proposal has two stages. The first deliberately preserves those distinct
input adapters. The later stage gives them one normalized, non-tracked
locomotion action surface.

## Terms

**Tracked pose input** is a transform observation, such as an HMD, hand, grip,
or aim pose. It is not this proposal's subject.

**Non-tracked locomotion input** is an authored movement command independent of
a tracked pose. Examples include keyboard keys, an XR thumbstick, a regular
desktop gamepad stick, accessibility switches, replay input, or a remote
controller. It has a normalized two-axis value and optional discrete actions.

**Automatic locomotion** is an input component's built-in direct transform
mapping, for example desktop `Input` WASD translation or
`InputXRGamepad.locomotion()`. It is distinct from observing the same
non-tracked input as an action.

## V1: device-specific behavior assets (implemented module layout)

V1 does not introduce a movement-authority system or a universal input event.
The mounted-car behavior now lives in the existing display-car module as two
explicit public constructors; see [Vehicle behaviors](../spec/vehicle_behaviors.md).
The two-file layout below remains a possible later organization, not the current
implementation.

```text
assets/components/
  vehicles/
    display_car.mms              spatial prefab: GLB + entry zone + mount points
  vehicle_behaviors/
    car.mms                      desktop/Winit keyboard adapter + car behavior
    car_xr.mms                   InputXRGamepad adapter + car behavior
```

`display_car()` remains input-neutral. It owns the car model and its authored
spatial affordances, including entry zone, mount anchor, and dismount anchor.
The module additionally exports explicit desktop and XR controlled constructors;
calling the base spatial factory does not attach any behavior.

`vehicle_behaviors/car.mms` is the desktop behavior asset. It may subscribe to
global `KeyDown`/`KeyUp`, retain W/A/S/D state, fire on `Space` down, and update
the supplied car's transform only while its supplied car is mounted.

`vehicle_behaviors/car_xr.mms` is the XR behavior asset. It may subscribe to a
supplied `InputXRGamepad`, retain `LeftStick`, fire from its XR button/chord
policy, and update the supplied car only while mounted.

If the two-file layout is chosen, both V1 assets own the same car-local concerns:

- mounted/unmounted lifecycle reset;
- planar steering and throttle integration;
- car-local position/yaw state until a live transform-direction API exists;
- car muzzle placement after GLTF bounds are known;
- replayable vehicle laser flash and beam animation.

They intentionally do **not** hide their different input subscriptions. A
desktop global-key subscriber and an XR component-scoped subscriber are not
interchangeable today. Keeping two small adapter-specific assets is more honest
than one nominally generic asset that embeds both paths.

V1 assumes one local desktop keyboard owner. It does not solve multiple local
players, keyboard focus policy beyond the existing global-key behavior, regular
gamepads, nested mounts, or control routing.

## Target action contract

Once the input layer can publish a normalized action independently of automatic
locomotion, the two behavior assets can converge on one
`vehicle_behaviors/car.mms`.

The minimal event is conceptually:

```text
Locomotion2DChanged {
  source_component: component
  value: Vec2                 // x = right, y = forward, each in [-1, 1]
}
```

The event is a command-space value, not a world-space direction and not a
tracked pose. A vehicle controller may interpret it as steering/throttle; a
walking controller may interpret it as lateral/forward movement; an aircraft
may interpret it differently.

The companion one-shot action is conceptually:

```text
ActionDown { source_component: component, action: "Primary" }
```

The exact public names remain open. `Locomotion2DChanged` and `Primary` are
used here to make the required semantics concrete, not to pre-empt the broader
input naming decision.

### Source normalization

All sources publish the same range and axes:

| Source | Input | normalized `Vec2` |
| --- | --- | --- |
| Keyboard | A/D and W/S held state | `x = right - left`, `y = forward - backward`; diagonal is clamped to length 1 |
| XR | selected thumbstick | native analog value after documented deadzone |
| Desktop gamepad | selected thumbstick | native analog value after documented deadzone |
| Replay/remote/accessibility | producer-defined | must provide the same `[-1, 1]` command range |

Keyboard events change the retained action value on down/up edges. Stick input
changes it as the analog value changes. A controller consumes the latest value
on `FrameTick`; it must not derive motion from key-repeat frequency or from the
rate of axis-change events.

Automatic locomotion and normalized action observation must be independently
configurable. Suppressing `Input` translation or `InputXRGamepad` locomotion
must not make the source's `Locomotion2DChanged`/`ActionDown` observations
disappear. That is what lets a mounted vehicle become a manual consumer while
pedestrian automatic movement is inactive.

This proposal does not decide how an engine-owned control-routing layer selects
the active consumer. The separate
[movement-authority and input-routing task](attachment-movement-authority-and-input-routing.md)
owns that broader problem. The normalized event is useful before that work, but
multiple active consumers need an explicit routing policy before it can be
treated as a general multiplayer or nested-vehicle solution.

## MMS `Vec2` values and composition

The current MMS arrays are suitable for raw transport (`event.value[0]` and
`event.value[1]`) but are intentionally not element-wise vectors. Do not add
array `+`, `-`, or `*`: an arbitrary numeric array has no stable geometric or
command-space meaning.

Instead, introduce a future first-class value type, tentatively `Vec2`, aligned
with the existing MMS nominal-type/struct direction. It needs named channels
and small, explicit operations:

```mms parse-only
let keyboard = Vec2(-1.0, 1.0)
let stick = event.value
let assisted = Vec2(0.0, 0.20)

let commanded = (keyboard + stick + assisted).clamp_length(1.0)
let steering = commanded.x
let throttle = commanded.y

if commanded.length() > 0.16 {
    // integrate the active controller's motion
}
```

The initial `Vec2` surface should include:

- construction: `Vec2(x, y)` and `Vec2.zero()`;
- named reads: `.x`, `.y`;
- `+`, `-`, unary `-`, and scalar `*`/`/`;
- `.length()`, `.length_squared()`, `.normalized_or_zero()`, and
  `.clamp_length(maximum)`;
- `.with_deadzone(minimum)` only if its semantics are precisely documented.

`event.value` for `Locomotion2DChanged` should be a `Vec2`, not a two-element
array, once the type exists. Until then, raw event payloads remain arrays and
the source/behavior assets retain explicit scalar conversion. This avoids
turning every MMS array into a JavaScript-style vector/object hybrid.

Vector composition does not by itself define authority. Adding keyboard,
thumbstick, scripted assistance, and remote input is only valid when the
consumer intentionally allows those sources to combine. A later action map can
choose `replace`, `sum_then_clamp`, priority, or another declared composition
policy.

## Converged car behavior

With a normalized source, the future common asset has one input-facing boundary:

```mms parse-only
on(controls, "Locomotion2DChanged", fn(event) {
    vehicle_state.command = event.value
})

on(controls, "ActionDown", fn(event) {
    if event.action == "Primary" && vehicle_state.mounted {
        fire_laser()
    }
})
```

Its `FrameTick` logic then reads `command.x` as steering and `command.y` as
throttle, while the laser presentation remains entirely car-local. The asset
still requires an authoritative mounted state and an explicit control source;
it must never infer authority from proximity, zone overlap, or an input
component's automatic-locomotion flag.

## Open questions and non-goals

- Is `Locomotion2DChanged` emitted by `Input`, `InputXRGamepad`, a new non-pose
  action-source component, or all of those behind one event contract?
- How is the selected desktop keyboard owner represented when several `Input`
  components exist?
- Which button mapping produces generic actions such as `Primary`, and how do
  XR controller chords map without losing raw XR events?
- Does deadzone belong to source normalization, action mapping, or controller
  policy? The initial recommendation is source normalization with documented
  values.
- How should focus loss, text input capture, device disconnect, and scene
  teardown reset action state?
- A reusable planar car still needs either retained yaw ownership or a future
  transform-direction API. This document does not introduce physics,
  collision, force integration, haptics, tracked-pose steering, or a general
  movement-authority stack.

## Related work

- [Keyboard and regular gamepad events](../desktop/keyboard-and-gamepad-input.md)
- [MMS keyboard and regular gamepad events](mms-keyboard-and-gamepad-events.md)
- [Mittens-corp mounted vehicle controls and laser first slice](mittens-corp-mounted-vehicle-controls-and-laser-first-slice.md)
- [Separate mounting from movement authority and input routing](attachment-movement-authority-and-input-routing.md)
- [MMS structs design draft](../meow_meow/draft/structs.md)
