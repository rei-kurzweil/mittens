# Gravity modes and body-local overrides

Gravity has three modes:

- `Gravity.on()` supplies its own coefficient (`1.0` by default).
- `Gravity.off()` explicitly blocks inherited gravity acceleration.
- `Gravity.auto()` defers to the next applicable provider.

Bare `Gravity {}` retains its existing explicit-on default. Coefficients can be
positive, zero or negative. Auto ignores its stored coefficient while inheriting;
switching to On uses that stored coefficient again. Off/Auto change acceleration
policy only: they do not erase a body's existing velocity.

Constructors, component-body builders and live MMS methods use the same names:

```mms
let local_gravity = Gravity.auto()
Gravity.coefficient(0.5) {
    Velocity {
        local_gravity
        T { name = "player_motion" }
    }
}

// These calls may also be made from event handlers or via a scoped query.
local_gravity.off()
local_gravity.auto()
local_gravity.on()
```

The existing `gravity.set_enabled(bool)` remains available and selects explicit
On or Off. The `enabled(bool)` and `coefficient(number)` builders remain valid.
Rust callers use `GravityComponent::{on,off,auto}()`, `mode()`, `set_mode()` with
`GravityMode::{On,Off,Auto}`, and `set_enabled()`.

Resolution checks direct Gravity children of a Velocity as body-local overrides
first, then walks ancestor providers. Auto is transparent in both locations.
Any explicit local Off wins over other local providers; this lets independent
features own separate blockers without enabling each other's gravity. With no
local explicit provider, the nearest explicit ancestor wins. An intervening
Velocity still ends ancestor lookup. Arbitrary descendant Gravity components do
not affect their ancestors. Several local explicit On providers should be
avoided: the first in child order wins when there is no Off provider.

Flight creates and owns an ordinary body-local Gravity component. While flying
it switches that instance to Off; returning to walking switches the same instance
to Auto. Disabling/removing the controller removes only its own temporary
provider. The component is excluded from filtered saves. Authored gravity and
other features' providers retain their values, including edits made during
flight. No avatar reparenting is needed for this override.

Velocity has no flight flag: it integrates whatever acceleration generic gravity
resolution supplies. Collision reports physical support independently. Flight
mode remains locomotion state for input and lower-body IK, so flying legs remain
unplanted even when a capsule touches the floor.

Tests cover constructors/builders/live methods, mode serialization, inherited
coefficient edits and inversion, independent body-local blockers, sibling bodies,
Velocity boundaries, momentum preservation, controller cleanup, and exclusion of
flight's temporary provider from saves.
