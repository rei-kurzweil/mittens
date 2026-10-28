# Shared AVC setup and untracked walking

Date: 2026-10-28

Status: implemented with open desktop follow-ups from user smoke testing.
Both Input and InputXR now use the shared head splice and body solve. Automated look-down
geometry and body-surface checks pass; user desktop retest reports improved visibility after
the camera-mount and near-plane correction. Desktop holding/idle-pose work remains open.

## Desktop smoke-test findings

User feedback on 2026-10-28: the change generally seems to work, with two issues:

- [Desktop held-item avatar poses](desktop-held-item-avatar-poses.md): grabbing
  a machine pistol leaves arms in their idle pose. Investigate proper holding
  targets/poses; a relaxed A-pose should also be the default idle posture.
- [First-person look-down head/neck regression](../bugs/sakura-desktop-first-person-neck-bend-regression.md):
  the original camera-rotation checks missed body-surface clipping. Shared head splicing,
  the calibrated camera mount and a 1 cm near plane are now in place; the user reports improvement.

## Baseline and implementation order

Commit `db0ce631` adds `examples/sakura-drone-conversion-desktop.mms` before the engine change,
including the pending Lumae asset migration and pacing controller dependencies. The desktop scene
has a first-person C3D on Rei(mu)'s mapped head, mouse/WASD input, capsule-gated gravity, rooftop
reset, mirror, NPC pedestal and existing gun fixtures. XR is off. The committed baseline passed
headless avatar/camera/capsule initialization before changing AVC.

Commit `1c3ef331` introduced shared NPC setup with an XR-only head splice. The subsequent revision
restores the same head splice for Input and InputXR, removing the attached-head desktop path while
retaining independent NPC setup. No new public component, signal, or host API is added.

## Avatar modes and ownership

- Every initialized AVC can share humanoid/model bindings, the generated capsule and lower-body
  rig/solver. Walking remains opt-out and observes actual supported capsule movement after contact.
- An Input or InputXR ancestor supplies the authoritative head target. Both initialize the same
  displaced-head splice and use the same geometric closure to calculate body placement from the
  head target and calibrated neck displacement.
- InputXR retains measured-pose authority and tracking-readiness gating. Invalid/disabled tracking
  still releases walking. Desktop Input derives a connected head target from look orientation,
  locomotion and calibrated geometry; that input adaptation does not require another head topology
  or a second head-local orientation writer.
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
- The desktop Sakura player initializes C3D without InputXR/CXR, splices the head to its driver, and
  responds to desktop movement/look while lower-body stepping remains active. The real scene
  additionally checks bounded neck flexion on look-down, neck/head geometric closure, a stable
  planar body anchor and return to neutral.
- A synthetic pose-driver test checks the same head splice and body closure for both input modes.

All 98 targeted checks pass: 23 lower-body, 14 head/neck, 33 AVC, 10 hand-tracking,
11 Sakura scene, four pacing, two untracked Lumae and one desktop integration test.
`cargo check --lib` and whitespace checks also pass. The desktop integration check additionally
verifies that opting out of native neck bending preserves look and does not accumulate body drift.
Those initial checks were CPU-only. A later clipping repro found the desktop scene's 10 cm near
plane discarding an outer shirt surface about 8 cm away. The first-person C3D now explicitly uses
`near(0.01)` and a single declared AVC camera wrapper, with its eye offset included in calibration.
Additional CPU-skinned surface checks cover downward pitch/roll, and inspected headless Vulkan
captures show the outer shirt/chest. The user reports improved desktop visibility; see the linked
bug for evidence and remaining visibility/tuning limits.

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
