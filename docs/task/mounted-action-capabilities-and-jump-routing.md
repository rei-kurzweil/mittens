# Mounted action capabilities and jump routing

Status: deferred design; keep separate from the gravity and teleport demos.

A mount must explicitly advertise actions it handles (for example `jump`). A
jump request should route to a mounted car only when that car registers jump
support; otherwise the player remains the intended recipient. Being mounted
alone must not imply that the vehicle can jump.

Define how a player jump works while attached: temporary relative motion,
release-and-jump, or another explicit attachment policy. Do not silently move an
attached movement root independently of its mount. Preserve current car driving
and laser bindings until action routing is implemented.

Follow up with the input action/binding design in
[input-actions-and-per-binding-overrides.md](input-actions-and-per-binding-overrides.md):

- Input and InputXRGamepad expose button/key actions separately from pose drivers.
- Defaults can bind Space and controller Y to jump; MMS overrides each binding.
- Mounts register capabilities and handlers, with deterministic priority and fallback.
- Route one request once, including repeat/held behavior and airborne rejection.
- Test player-only, jump-capable and incapable mounts, dismount, and binding overrides.

The current display car has no jump capability. Its existing mounted Space
binding fires the laser. Demo player jump handlers must be suspended while the
player is attached; the fallback attachment policy above remains future work.
