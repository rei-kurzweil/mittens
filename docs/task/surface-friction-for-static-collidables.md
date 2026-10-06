# Surface friction for static collidables

Status: initial implementation; original intermittent crate-climbing report
still needs interactive verification.

Friction belongs to the surface's physical response, not the passive Zone or a
hidden player damping rule:

```mms
Zone.cube([0.5, 0.5, 0.5]) {
    Collidable.static().friction(0.8) {}
}
```

The coefficient is finite, nonnegative and defaults to zero. Values above one
are allowed. The initial API accepts friction on static surfaces only; it does
not combine material coefficients from a moving collider. The MMS builder and
save/reload preserve the authored value.

## Response

Static contact still removes inward velocity from the Velocity directly owning
its movement target, in world coordinates. For a frictional surface, the change
in tangential speed is bounded by `mu * normal_impulse`, expressed in velocity
units. The tangential reduction cannot reverse direction or exceed the remaining
tangential speed. A zero coefficient preserves frictionless slide behavior.

Friction runs on fixed physics substeps, using the inward velocity removed by
that contact as its normal impulse. Gravity supplies support loading on a floor
on each substep, so residual horizontal speed can settle on an authored rough
floor. Extra pose-only contact passes do not apply additional friction budgets.
There is no global damping and no drag while airborne or away from contact.
With no compressive normal impulse (for example zero gravity and purely tangent
motion), this response supplies no friction impulse.

The secondary-motion desktop floor and movable crate surfaces explicitly author
`friction(0.8)`. Other surfaces remain frictionless unless configured; no global
friction default was changed. Input remains a separate pose driver: friction
changes owned physical Velocity, not keyboard movement speed.

## Report and validation

The crate-climbing drift report is tracked in
[player-keeps-drifting-after-jumping-onto-crates.md](../bugs/player-keeps-drifting-after-jumping-onto-crates.md).
An oblique edge normal can project falling speed into tangential lateral motion;
subsequent frictionless flat contacts preserve it. This identifies a plausible
mechanism, but does not establish the exact original interactive contact sequence.

Regression coverage compares rough and frictionless floors after this edge
projection, checks settling at 60/120/240 Hz render rates, preserves airborne
horizontal speed, bounds the friction impulse without reversal, and checks MMS
validation/roundtrip. Existing tangent-preservation and floor-sweep tests remain.

## Follow-ups

- Reproduce and retest the original crate-side climbing interaction manually.
- Decide whether separate static/dynamic coefficients are useful for a concrete
  demo; the current bounded response uses one coefficient.
- Define material combination rules when dynamic-body contacts are introduced.
- Investigate moving support, tilted surfaces and multiple-contact manifolds
  separately from the initial static contact solver.
