# Task: input actions and per-binding overrides

Status: proposal, 2026-10-04. Record the input follow-up without blocking the
Gravity/Velocity demo and grounding work. Do not implement the general binding
system in that slice.

## Vocabulary and current behavior

An **input source** supplies device state or events: keyboard/mouse, an XR
controller's buttons and sticks, or an HMD/controller tracking pose. An
**action binding** maps a key, button, axis, or gesture to behavior. A **pose
driver** writes a transform; a **velocity driver** changes velocity. One
component can expose an input source and implement a default action/driver;
these roles should not be treated as synonyms.

`Input` currently combines desktop controls with direct transform movement.
Keyboard events are also available globally through KeyDown/KeyPress/KeyUp.
`InputXR` supplies tracked poses, while `InputXRGamepad` supplies controller
button/axis events and optional direct locomotion. Disabling Input to replace
one default behavior also disables unrelated controls. Input already exposes
translation/rotation toggles and XRGamepad can disable locomotion, but those
are coarser than per-binding overrides.

## Proposed behavior

- Provide useful automatic default bindings for Input and InputXRGamepad,
  with separately configurable movement, look, and button actions.
- Allow MMS to replace, disable, or restore one binding while leaving the
  input source, event delivery, and other bindings enabled.
- Decide whether replacement consumes the default action or adds a handler;
  default replacement should not execute both behaviors accidentally.
- Keep raw events available independently of enabled default motion behavior.
  Distinguish disabling a device source from disabling a binding or driver.
- Route actions to an explicit target/authority. Jump changes the falling
  Velocity's speed once; it does not move the tracked head transform, disable
  gravity, or alter secondary-motion spring gravity.
- Candidate jump defaults: Space on desktop, ButtonY on the left XR controller
  where that interaction profile exposes it. Define alternatives for devices
  without Y instead of assuming all controllers have that button.
- Define press/release/held/repeat behavior, focus/text-entry suppression,
  mounted authority, duplicate bindings, and serialization of authored edits.
- Default jump requires support contact and one press edge; decide aerial-jump,
  buffering, and coyote-time policies separately. A zero vertical speed alone
  is insufficient proof of support (it also occurs at an apex).

## Next slice after gravity demo validation

Audit existing default Input and InputXRGamepad behaviors and raw event routing;
settle action/binding names and MMS syntax before implementing them. Test a
single replaced key/button without disabling the component, retained movement
and look, text focus, controller profile fallback, and mounted/unmounted
routing. Until then demos can use existing keyboard/XrButtonDown handlers and
Velocity.translate_world for one-shot jump impulses.

Related: [driver terminology](../spec/physics/driver-terminology.md),
[gravity and acceleration](gravity-and-acceleration-velocity-drivers.md),
[attachment and input authority](attachment-movement-authority-and-input-routing.md),
and [XR gamepad refactor](xr-gamepad-and-hand-input-refactor.md).
