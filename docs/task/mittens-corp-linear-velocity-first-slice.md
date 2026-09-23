# Task: XR linear Velocity shell and button-driven test

Status: proposed, 2026-09-23. Design pass only; no `Velocity` runtime API or
`mittens-corp-linear-velocity.mms` scene has been implemented yet.

## Outcome and order

Build the MMS-facing shell first, then its Rust component/system support, then
`examples/mittens-corp-linear-velocity.mms`. This is the next active slice
after the inert grounding root in
[XR avatar grounding first slices](xr-avatar-grounding-first-slices.md).
Two XR-clickable buttons change Bisket's *velocity state* by reference. The
avatar moves without gravity, floor contact, or a physics body. Gravity and
contact are later independent velocity-driver/contact slices.

## Proposed MMS contract

Use a `Velocity` component directly under the transform it drives. The named
outer grounding root is the motion target, while gamepad XZ remains on the
inner locomotion root:

```mms
let vel = Velocity {} // retained live reference; zero linear velocity initially
ED.active() {
    T {
        name = "bisket_grounding_root"
        vel
        T.position(-5.0, 0.0, 0.0) {
            name = "bisket_locomotion_root"
            InputXR.on() { /* existing Bisket rig */ }
        }
    }
}

let forward = button("forward", { compact = true })
let back = button("back", { compact = true })
on(forward, "Click", fn(event) { vel.translate([0.0, 0.0, -0.25]) })
on(back, "Click", fn(event) { vel.translate([0.0, 0.0, 0.25]) })
```

`vel` is a retained *live component reference*, not a query that copies its
initial state. `translate(delta)` adds a linear-velocity change in m/s. It
does **not** translate the transform and does **not** multiply by `dt`: one
click changes velocity once. Repeated forward clicks accumulate speed; one
matching back click cancels one forward click. Motion persists after release
until another command changes it; there is no implicit friction or braking.
Reject non-finite deltas and overflow to non-finite state.

The unsuffixed `translate` command uses the driven transform's **local
orientation at command time**. Convert the local vector to a world-space
velocity delta, ignoring translation and scale, then add it to the stored
world-space linear velocity. This is a snapshot, not a continuously rotating
body-fixed velocity: subsequent yaw changes do not turn existing momentum.
Expose `translate_world(delta)` for intentional world-axis changes; do not
make authors rewrite/reparent transforms to obtain world-space behavior.
Later, `rotate(delta)` may analogously add local angular velocity in rad/s;
its contract must explicitly say it changes angular velocity rather than
rotating the linear-velocity vector. Angular integration is not required for
this two-button test. A persistent acceleration/throttle API is separate from
these one-shot velocity increments.

Important XR distinction: `bisket_grounding_root` does not rotate when the
HMD turns inside it. Therefore local `vel.translate([0, 0, -0.25])` follows
the grounding root's -Z, **not** the headset gaze. This first panel tests
component-local semantics. HMD-facing thrust later needs an explicit
orientation reference (or an HMD-forward vector converted into a world-space
delta); parenting the outer root under its tracked descendant would create a
cycle. Do not label this button "headset forward" until that policy exists.

## Proposed Rust boundary

`VelocityComponent` owns enabled state and finite `linear_world_mps: [f32; 3]`
with a zero default. The first slice resolves exactly one *immediate parent*
`TransformComponent` as its target and reports a missing/ambiguous authority
rather than searching arbitrary descendants. Its state is not hidden in
`CollisionResponseSystem`. The authored initial velocity is distinct from the
mutable runtime velocity so scene serialization does not capture incidental
button presses.

`VelocitySystem` owns the resolved component-to-transform binding, applies
validated `AddLinearLocal`, `AddLinearWorld`, and `SetLinearWorld` commands,
and integrates the stored world velocity once per fixed substep. Rust callers
should use explicit methods/command variants such as
`add_linear_local(velocity_id, delta_mps)` and
`add_linear_world(velocity_id, delta_mps)`; MMS exposes the shorter
`translate` and `translate_world`. Resolve the effective world rotation of
the driven transform when accepting a local command, then queue its converted
world delta for the next substep. Apply accepted commands exactly once before
that substep, never once per render frame or again on callback replay. The
same-frame scheduling requirement in the grounding task still applies to
world transform and XR camera publication.

Convert each integrated world displacement back through the target's parent
basis before changing its local translation, so parent rotation/scale do not
change commanded world speed. Reject a singular parent basis, non-finite
values, or another active writer on the *same transform/channel*. The
grounding root's XZ velocity can coexist with gamepad XZ on its *inner child*;
their motions compose through ancestry. Use the bounded deterministic
fixed-step policy described in the grounding task; report dropped time.

The public Rust component API should provide read access to the current
world velocity and an explicit zero/set path for tests and future contacts.
The system/command boundary should own local-to-world conversion because it
needs the live transform hierarchy; a bare component method must not guess a
basis from stale cached data. Keep the future gravity/force request boundary
open, but do not add mass, force accumulation, contact, or angular integration
to make this example work.

## XR example and acceptance

Create `examples/mittens-corp-linear-velocity.mms` from the XR Bisket/studio
parts of `mittens-corp.mms`: stage, lights, mirror, Bisket AVC/IK, colliders,
secondary motion, pose capture, XR camera/controllers, and gamepad locomotion.
No desktop camera/window and no vehicle/Rider interaction in this focused
test. Keep the authored grounding root as the outer transform. Use
`assets/components/ui/info_panel.mms` with a grabbable anchor and exactly two
content buttons, **forward** and **back**, from `button.mms`. Build the body
through a `make_content(vel)` function; the info panel removes its body on
minimize, so `AccordionRestoreRequested` must rebuild it with fresh click
handlers, without duplicating handlers on the old buttons.

Headless tests should prove default zero, live MMS reference dispatch, one
click = one velocity increment, back cancels forward, fixed-step integration
at different render rates, world-speed preservation under a rotated/scaled
parent, finite-value rejection, and composition with inner gamepad movement.
An XR smoke test should confirm both buttons are ray-clickable, the panel can
be grabbed/minimized/restored, Bisket moves along grounding-root -Z, normal
gamepad/HMD/hand motion continues, and the mirror/camera see the same-frame
outer-root motion. There is no floor stop in this slice; avoid claiming that
the avatar is grounded.

## Relationship to earlier design

The broader [scriptable Velocity pose-driver task](scriptable-velocity-pose-driver.md)
now uses the same single-component, direct-child attachment and
`translate`/`rotate` naming. This focused XR slice implements only linear
`translate` and integration; angular `rotate` follows later. Both documents
use local-default *change commands* over world-space stored velocity.
