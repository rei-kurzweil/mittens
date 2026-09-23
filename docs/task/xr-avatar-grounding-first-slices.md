# First testable slices: XR avatar grounding root

Status: proposed, 2026-09-23.

## Outcome

Bisket keeps its current XR gamepad locomotion, tracked HMD and controllers,
AVC, IK, animation, and pose capture. A new ancestor transform supplies vertical
motion from explicit velocity and gravity, then holds the avatar above the
stage. The inner systems continue to address their present transforms. The
first implementation proves that hierarchy and floor contact with one avatar
and one static floor; it does not claim to be a general rigid-body solver.

This is the focused implementation path for
[skinned-mesh grounding and floor contact](./skinned-mesh-grounding-and-floor-contact.md).
The capsule stick figure stays a separate static scale reference while the XR
Bisket path is established.
Use the [pose/velocity driver terminology](../spec/physics/driver-terminology.md)
when defining the new components and systems.

## Actual starting point

In [`examples/capsule-stick-figure.mms`](../../examples/capsule-stick-figure.mms),
`bisket_locomotion_root` is the transform immediately above `InputXR`.
`InputXRGamepadSystem::xr_locomotion_target_transform` chooses the nearest
transform ancestor of `InputXR`, so gamepad XZ movement currently lands there.
AVC uses the same resolver to route its generated capsule's
`CollisionResponse.slide()` correction. There is no `VelocityComponent` or
`VelocitySystem` yet. The visible stage deck has center Y = 0 and half-height
0.12, so its top is Y = 0.12, but it currently has no physical floor collider.

The current tick runs collision and response before camera/OpenXR, then applies
XR gamepad movement after the OpenXR pass. A grounding implementation must
resolve this order deliberately: a contact computed before that frame's
gamepad movement is not the final contact pose, and moving the outer root after
the XR eye pose is published may leave the rendered camera stale for a frame.

## Target hierarchy and authority

```text
bisket_grounding_root                  ← VelocitySystem + ground contact own Y
  └── bisket_locomotion_root           ← existing gamepad owns XZ
        └── InputXR                    ← tracked HMD/controller data
              └── bisket_xr_driver
                    └── AVC + GLTF     ← body/hand IK, animation, pose overlays
```

## Proposed opt-in components

The opt-in is attached to the **outer grounding transform**, regardless of
whether its descendant is a skinned model or an ordinary component tree.
Provisional MMS shape (these constructors do not exist yet):

```mms
T {
    name = "bisket_grounding_root"
    Velocity.vertical([0.0, 0.0, 0.0]) {}   // world-space velocity state and Y integration
    GravityAcceleration.world([0.0, -9.81, 0.0]) {} // opt-in acceleration provider
    GroundContact {
        proxy("#bisket_runtime_capsule")
        floor("#stage_deck_collision")
        movement_target("#bisket_grounding_root")
    }
    T { name = "bisket_locomotion_root" /* existing InputXR subtree */ }
}
```

The spelling and proxy reference mechanism are provisional. The ownership is
the contract:

| Component | First-slice responsibility |
| --- | --- |
| `VelocityComponent` | Holds the current world-space linear velocity and opts the named outer transform into integration; initially writes Y only. Together with `VelocitySystem`, it is the outer root's pose driver. It can also run with a commanded velocity and no gravity. |
| `GravityAccelerationComponent` | A linear velocity driver: applies configured downward acceleration to that velocity once per fixed substep. It does not move a transform itself. |
| `GroundContactComponent` | Names the floor/proxy and outer movement target. Contact correction writes the same outer root and cancels velocity into the floor normal. |
| `Collision`/capsule proxy | Supplies geometry for contact. It carries no gravity or private velocity. AVC's existing inferred capsule may be reused after its old response is explicitly disabled or migrated. |

This proposed gravity component is distinct from today's `GravityComponent`,
whose only current consumer is `CollisionResponseSystem`: that system searches
for an ancestor gravity field and stores velocity privately on each response.
Do not make the new velocity path inherit that behavior by accident. During
implementation, either migrate the existing `Gravity` API to the new
acceleration-provider contract with compatibility tests or give the new
component an explicit name and retire the old API with collision response.

For arbitrary physical forces, the next additive components would be a
`PhysicsBodyComponent` with mass/inverse mass and a per-step
`ForceAccumulatorComponent`. The future integration rule is
`acceleration = gravity + accumulated_force / mass`, followed by velocity and
position integration. The first test only needs uniform gravity, so it does
not introduce a mass value or pretend that gravity is already a general
force/impulse API. AVC itself needs no `Velocity` or force component.

`bisket_grounding_root` is a plain world transform. No XR, gamepad, AVC, or
skeletal component needs a reference to it for normal pose production.
Gamepad movement still resolves the nearer `bisket_locomotion_root`; the new
root must not become the implicit locomotion target. Vertical acceleration and
floor correction act on the outer root only. Tracking remains local to its
existing descendants, so a grounding displacement is inherited once.

An explicit contact-target policy supersedes AVC's old automatic response
target for this scene. During migration there must be exactly one floor
correction for Bisket. Either route the existing AVC proxy's contact to the
new outer root through an explicit target, or disable that response and use a
new proxy/constraint. Do not run two contact systems on one avatar or route
gravity through the old response's private velocity.

The vertical reference is the *avatar body/proxy bottom*, measured with the
model's effective scale. The HMD pose is tracking input; it is not a floor
probe. A one-time measured/calibrated relationship between tracking origin,
avatar body, and ground is needed so ordinary standing does not make the
virtual camera sink or rise. Room-scale HMD Y changes, crouching, and headset
recentering must not be mistaken for gravity impulses or teleports.

## Slice A — prove the extra transform boundary

Add the outer transform in one focused XR example. Initially it has zero
offset and no motion component. Keep the existing Bisket rig intact.

Tests and smoke checks:

- A headless topology test proves gamepad target resolution is still the inner
  locomotion transform and the grounding target is explicitly the outer root.
- An XR smoke test confirms gamepad movement, HMD movement, controller targets,
  hand IK, mirror view, and pose capture behave as before.
- A scripted small Y change to the outer root moves the whole avatar/camera
  once, while the inner gamepad transform and imported joint locals stay
  unchanged.

This slice can land independently. It verifies transform composition before
gravity and collision complicate diagnosis.

## Slice B — a usable vertical VelocitySystem

Add a first-class world-space `VelocityComponent` and dedicated
`VelocitySystem` for one explicitly nominated transform. This pair is the
outer root's **pose driver**; the first active version integrates only Y.
A standalone headless test
must also demonstrate commanded constant velocity without gravity, so the
velocity path is useful on its own.

Use a bounded fixed timestep and a deterministic update order. Convert the
world-space Y displacement into the root's parent-local translation if needed.
Document the accumulator limit and report dropped time; reject non-finite
values and two active Y writers on the same transform. Store current velocity
in `VelocityComponent`, not in a collision component or ad hoc system map.

Gravity is an opt-in **velocity driver**: a named acceleration provider that
changes linear velocity once per substep (`v_y += g * dt`) and never writes
the grounding transform. For this slice it may be a single uniform world
gravity configuration; a general force accumulator, mass, torque, and impulses
can follow when there is a second consumer. Mark that future extension with a
short `TODO` at the provider boundary if the code needs one. Gravity must not
be applied to XR tracking transforms, gamepad transforms, or bones.

Tests: equal elapsed simulation time at different render rates produces the
same vertical velocity/height within fixed-step tolerance; disabling gravity
leaves commanded velocity working; disabling the velocity driver stops its
transform writes without changing descendants' authored local transforms.

## Slice C — one floor and resting contact

Give the visible comparison deck one static collision surface at Y = 0.12.
Reuse the existing upright capsule inference and collision geometry only where
they yield a correct *effective world-space* proxy for Bisket. Verify the
proxy bottom against actual visual bounds; a scale-dropping transform stream
cannot be assumed correct for other models.

The ground contact step consumes the proposed velocity-driven root pose and
the current static floor. On a downward contact it moves the outer root up by
the required correction and removes only velocity into the floor normal.
Tangential/gamepad movement continues. The next fixed step must observe the
corrected velocity, so the avatar rests rather than gaining downward speed
every frame. A missing floor leaves the avatar falling; it does not pin the
root to an authored Y value.

The first implementation may use a narrow ground-only constraint around the
existing shape/contact queries. It should have a clearly named system boundary
and an explicit movement target. Document discrete-contact limitations in code
near the solver, including tunneling risk at large steps. A `TODO` for broader
dynamic contact solving is appropriate; a second general collision response
path is not.

Tests: start Bisket above the floor at two heights, let it settle, then verify
proxy bottom within 5 mm of Y = 0.12, vertical velocity near zero, and no
visible jitter over several seconds. Walk horizontally while grounded and
verify XZ movement and height remain stable. Stop the floor or move outside it
and verify falling resumes. Recenter the HMD and crouch without accumulating
false grounding velocity.

## Scheduling gate before XR acceptance

The implementation must identify the authoritative order for each frame:
XR tracking sample, gamepad locomotion, velocity substeps, contact correction,
transform propagation, camera/eye publication, AVC/IK, and final skinning.
Preserve the latest tracked input and ensure camera, controller rays, IK, and
rendering consume the corrected ancestor transform in the same presentation
frame. If OpenXR cannot split sampling from camera publication yet, use an
explicit post-grounding refresh and test it. This is a required scheduling
change, not an acceptable one-frame visual lag.

Add one integration test that moves the gamepad and grounding root in the same
frame and checks the final world positions of the locomotion root, XR camera,
head, and both hand targets. Test a floor contact in the same frame. The test
should fail if collision sees only the pre-gamepad pose or the camera keeps the
pre-grounding world transform.

## Minimum diagnostics

For the XR example, report or overlay grounding-root world Y, vertical
velocity, proxy bottom Y, floor top Y, grounded state, and correction amount.
Log the resolved gamepad and contact movement targets once at startup. This
makes authority mistakes and a real model-scale mismatch distinguishable.

## Mittens-corp example migration

After the focused XR scene works, migrate the four Bisket scenes that share
the studio stage:

- [`mittens-corp.mms`](../../examples/mittens-corp.mms) and
  [`mittens-corp-agc.mms`](../../examples/mittens-corp-agc.mms) use `InputXR`,
  `InputXRGamepad`, AVC, and a `Rider` anchor. Verify walking, mounting,
  dismounting, and vehicle control ownership with the new outer root.
- [`mittens-corp-desktop.mms`](../../examples/mittens-corp-desktop.mms) and
  [`mittens-corp-agc-desktop.mms`](../../examples/mittens-corp-agc-desktop.mms)
  use desktop input/camera topology. Reuse the grounding behavior, but resolve
  their locomotion targets explicitly instead of copying the XR hierarchy.

Keep authoring small: prefer one reusable grounding wrapper/factory or compact
component configuration that accepts the existing rig subtree, the movement
root, and the floor/proxy policy. A scene should not need to repeat fixed-step
settings, gravity math, capsule construction, and contact routing. Preserve
the existing scene-specific AVC, Rider, microphone, and editor configuration.
The visible deck should use one reusable static-floor definition so all four
scenes agree on its top Y. Only Bisket opts in; vehicles and stage props retain
their own motion policy.

Migration acceptance includes all four scenes loading, no duplicate avatar
capsule/contact responder, grounded pedestrian movement, and correct authority
handoff when a rider mounts or dismounts. If mounting intentionally suspends
gravity, the wrapper must expose that transition and define how vertical
velocity resumes; it cannot quietly keep integrating behind the mounted pose.
Add a short code `TODO` at a deliberately narrow migration seam only when its
later generalization is genuinely deferred.

## Deferred architecture

These slices establish a seam for later Newtonian systems. A future
`PhysicsSystem` may orchestrate gravity/force providers, velocity integration,
contact generation, and constraints at fixed steps. This first task needs a
dedicated `VelocitySystem`, one acceleration provider, and one static ground
constraint. It does not need mass, torque, general force accumulation,
body-to-body impulses, backend selection, or a pluggable solver interface.

Cross-model capsule/Bisket height comparison, generic skinned-model grounding,
and pose retargeting remain in the parent tasks. The focused XR slice should
not rescale either model to make the test pass.
