# Task: XR linear Velocity shell and button-driven test

Status: implemented for the linear/button slice, 2026-09-23; headless checks
pass and the scene runs in XR. Headset button clicks/readback still need the
[focused interaction follow-up](xr-linear-velocity-button-click-and-readback.md).
Gravity, contact, and angular motion are separate later slices.

Design correction: the follow-up records the intended **parent-local stored
velocity** invariant. The world-space storage described below documents the
current implementation, not the target contract. Reconcile that gap before
building more velocity APIs or physics drivers on top of it.

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
inner locomotion root. A retained `InputXR` component reference selects the
inner rig's current heading as the velocity command's rotation basis:

```mms
let xr_input = InputXR.on() { /* existing Bisket rig */ }
let vel = Velocity.rotation_basis(xr_input).horizontal() {}
ED.active() {
    T {
        name = "bisket_grounding_root"
        vel
        T.position(-5.0, 0.0, 0.0) {
            name = "bisket_locomotion_root"
            xr_input
        }
    }
}

let forward = button("forward", { /* colors and compact style */ })
let back = button("back", { /* colors and compact style */ })
on(forward, "Click", fn(event) {
    query("#bisket_grounding_root").query("Velocity").translate([0.0, 0.0, -0.25])
})
on(back, "Click", fn(event) {
    query("#bisket_grounding_root").query("Velocity").translate([0.0, 0.0, 0.25])
})
```

The authored `vel` variable configures and attaches the component. Click
handlers resolve the emitted component through a live query; the builder
value alone is not a runtime handle inside a callback. `translate(delta)`
adds a linear-velocity change in m/s. It
does **not** translate the transform and does **not** multiply by `dt`: one
click changes velocity once. Repeated forward clicks accumulate speed; one
matching back click cancels one forward click. Motion persists after release
until another command changes it; there is no implicit friction or braking.
Reject non-finite deltas and overflow to non-finite state.

Without `rotation_basis`, the unsuffixed `translate` command uses the driven
transform's local orientation. With `rotation_basis(xr_input)`, it instead
uses the referenced rig's **active published XR eye orientation** at command
time; `InputXR` itself is a pose-driver marker, not an orientation value.
The optional `.horizontal()` projects the resulting direction onto world XZ
and normalizes it, removing headset pitch/roll so a forward/back click adds
no vertical velocity. It changes command interpretation, not the velocity
integrator's allowed axes. Resolve the basis after the XR pose is valid;
reject an unavailable, disabled, or ambiguous XR source or degenerate
horizontal projection with a diagnostic, without silently falling back to
the grounding root or world -Z.

Convert the local vector to a world-space velocity delta, ignoring
translation and scale, then add it to the stored world-space linear velocity.
This is a snapshot, not a continuously rotating body-fixed velocity:
subsequent yaw changes do not turn existing momentum. Expose
`translate_world(delta)` for intentional world-axis changes; it bypasses the
configured rotation basis. A `ComponentRef` to a transform can use that
transform's effective world rotation as a non-XR basis, subject to the same
validity checks. Do not make authors rewrite/reparent transforms to obtain
world-space behavior.
Later, `rotate(delta)` may analogously add local angular velocity in rad/s;
its contract must explicitly say it changes angular velocity rather than
rotating the linear-velocity vector. Angular integration is not required for
this two-button test. A persistent acceleration/throttle API is separate from
these one-shot velocity increments.

This read-only reference from an ancestor's `Velocity` to a descendant's
`InputXR` creates no transform-parenting cycle: integration still moves only
the outer root. Because velocity is stored in world space, pressing **back**
after turning applies reverse thrust along the *new* heading; it is not a
general brake for existing velocity in another direction. While heading is
unchanged, one back click cancels one forward click.

## Proposed Rust boundary

`VelocityComponent` owns enabled state, finite `linear_world_mps: [f32; 3]`
with a zero default, an optional `rotation_basis: ComponentRef`, and a
horizontal-command flag. The first slice resolves exactly one *immediate
parent* `TransformComponent` as its target and reports a missing or ambiguous
authority rather than searching arbitrary descendants. Its state is not hidden in
`CollisionResponseSystem`. The authored initial velocity is distinct from the
mutable runtime velocity so scene serialization does not capture incidental
button presses.

`VelocitySystem` owns the resolved component-to-transform binding, resolves
the optional rotation-basis reference on each accepted local command, applies
validated `AddLinearLocal`, `AddLinearWorld`, and `SetLinearWorld` commands,
and integrates the stored world velocity once per fixed substep. Rust callers
should use explicit methods/command variants such as
`add_linear_local(velocity_id, delta_mps)` and
`add_linear_world(velocity_id, delta_mps)`; MMS exposes the shorter
`translate` and `translate_world`. Resolve the configured source when
accepting a local command: an `InputXR` reference selects the current active
published eye orientation for that rig (not another rig's active camera),
while the unconfigured default uses the driven transform's effective world
orientation. Reuse the authoritative horizontal XR basis policy already used
by gamepad locomotion rather than
reconstructing the rendered eye pose from an ECS ancestor chain. Queue the
converted world delta for the next substep. Apply accepted commands exactly
once before that substep, never once per render frame or again on callback
replay. The same-frame scheduling requirement in the grounding task still applies to
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
click = one velocity increment, back cancels forward at constant heading,
quarter-turn XR heading changes the world-space delta, pitch does not add Y
under `.horizontal()`, invalid/missing poses do not fall back silently,
fixed-step integration at different render rates, world-speed preservation
under a rotated/scaled parent, finite-value rejection, and composition with
inner gamepad movement.
An XR smoke test should confirm both buttons are ray-clickable, the panel can
be grabbed/minimized/restored, Bisket moves along the current horizontal XR
eye heading, normal
gamepad/HMD/hand motion continues, and the mirror/camera see the same-frame
outer-root motion. There is no floor stop in this slice; avoid claiming that
the avatar is grounded.

## Relationship to earlier design

The broader [scriptable Velocity pose-driver task](scriptable-velocity-pose-driver.md)
now uses the same single-component, direct-child attachment and
`translate`/`rotate` naming. This focused XR slice implements only linear
`translate` and integration; angular `rotate` follows later. Both documents
use local *change commands*, optionally relative to an explicitly referenced
rotation basis, over world-space stored velocity.
