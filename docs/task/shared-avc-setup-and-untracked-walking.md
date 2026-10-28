# Shared AVC setup and untracked walking

Date: 2026-10-28

Status: implemented with open desktop follow-ups from user smoke testing.
Automated checks below pass, but do not close the reported head/neck issue.

## Desktop smoke-test findings

User feedback on 2026-10-28: the change generally seems to work, with two issues:

- [Desktop held-item avatar poses](desktop-held-item-avatar-poses.md): grabbing
  a machine pistol leaves arms in their idle pose. Investigate proper holding
  targets/poses; a relaxed A-pose should also be the default idle posture.
- [First-person look-down head/neck regression](../bugs/sakura-desktop-first-person-neck-bend-regression.md):
  the expected solver response is not visible. Investigate the recent desktop/XR
  split; the cause is not confirmed by the passing camera-rotation tests.

## Baseline and implementation order

Commit `db0ce631` adds `examples/sakura-drone-conversion-desktop.mms` before the engine change,
including the pending Lumae asset migration and pacing controller dependencies. The desktop scene
has a first-person C3D on Rei(mu)'s mapped head, mouse/WASD input, capsule-gated gravity, rooftop
reset, mirror, NPC pedestal and existing gun fixtures. XR is off. The committed baseline passed
headless avatar/camera/capsule initialization before changing AVC.

The subsequent working-tree change separates shared rig setup from tracked embodiment without
adding a public component, signal, or host API.

## Avatar modes and ownership

- Every initialized AVC can share humanoid/model bindings, the generated capsule and lower-body
  rig/solver. Walking remains opt-out and observes actual supported capsule movement after contact.
- An InputXR ancestor keeps tracking-readiness gating and the existing displaced-head splice.
  Invalid/disabled tracking still releases walking. XR head/camera placement remains authoritative.
- A desktop Input ancestor keeps the body-yaw and head/neck target machinery, but the imported head
  remains attached to its neck. The native constraint solves head-local orientation from a virtual
  look target and immutable head-rest calibration, so C3D follows mouse/arrow look without reparenting
  the head. The legacy opt-out path also reads the virtual target rather than feeding posed-head
  motion back into body placement.
- With no Input/InputXR ancestor, AVC binds an untracked rig. It does not generate a head mount,
  move the model to a head-height anchor, create a body-follow operator, or splice bones. Root
  transforms remain authored. Shared capsule creation explicitly propagates its world frame before
  contact consumes it, independently of head/body initialization side effects.

Lower-body eligibility accepts untracked AVCs and enabled desktop input while retaining XR validity,
mount, calibration and support gates. Actual X/Z motion can come from Velocity, scripts or ancestor
transforms. Mounted avatars, lost support, teleports and per-avatar walking opt-out retain the
existing release/reset behavior. Missing calibration is not treated as airborne movement.

## Lumae and Sakura

`assets/components/characters/lumae/avatar.mms` exports `lumae_avatar(label, material)` using the
shared AVC path, relaxed arms, collider preset and hair simulation. Its own GLB rest ankle frames
project vertically to the shoe-floor plane at model Y=0, giving local sole offsets
`[0, 0.0687271, 0.1129329]` for both feet; these are not the Rei(mu)/Bisket shoe offsets.

Both Sakura scenes now wrap spawned Lumae in the existing pacing controller and start it after
attachment. The named placement remains directly under the Lumae group; an inner motion root
receives Velocity movement and capsule correction. Initial yaw directs the first default walking
leg toward the roof interior. The timed behavior is not navigation/edge avoidance. Default cycles
wait, turn smoothly, walk, wait, and turn back; blocked movement does not itself trigger stepping.

The original scene still starts with zero Lumae. Add Lumae at the pedestal to exercise pacing.
Pacing also changes the opt-in CPU/GPU profiling workload; historical stationary-NPC measurements
remain historical and must not be presented as an identical current fixture.

## Validation

Targeted checks cover the existing lower-body, head/neck, AVC and hand-tracking regressions,
Sakura scene callbacks, and the following new behavior:

- Actual untracked Lumae retain model placement and neck/head hierarchy and get usable capsules.
- Two independent avatars alternate steps under Velocity and scripted ancestor movement; opting
  one out does not reset the other's gait.
- The real MMS pacing controller and Lumae prefab drive the shared walking solver together.
- The desktop Sakura player initializes C3D without InputXR/CXR, retains the head attachment, and
  responds to desktop movement/look while lower-body stepping remains active.
- A synthetic pose-driver test checks both desktop attachment preservation and XR head splicing.

All 98 targeted checks pass: 23 lower-body, 14 head/neck, 33 AVC, 10 hand-tracking,
11 Sakura scene, four pacing, two untracked Lumae and one desktop integration test.
`cargo check --lib` and whitespace checks also pass. The desktop integration check additionally
verifies that opting out of native neck bending preserves look and does not accumulate body drift.
No window/headset render was run for these automated checks.

## Smoke commands

```sh
cargo run --release -- load examples/sakura-drone-conversion-desktop.mms
cargo run --release -- load examples/sakura-drone-conversion.mms
```

On desktop, look up/down and around, walk, inspect the body/mirror and fall/reset. Add a Lumae and
wait through its turn/walk cycle. In XR, verify standing, crouching and walking still preserve the
tracked head/eye relationship, then inspect NPC stepping. Gun interaction remains available for
later testing; this change does not introduce new weapon mechanics.

[Airborne lower-body posture](airborne-lower-body-ik.md) remains a planned follow-up.
