# Desktop held-item avatar poses

Date: 2026-10-28

Status: open capability gap; user-reported during Sakura desktop smoke testing.

Related: [shared AVC setup](shared-avc-setup-and-untracked-walking.md),
[arm IK requires a real hand pose driver](avc-arm-ik-requires-hand-pose-driver.md),
[grab animation and pose transitions](grab-animation-and-pose-transitions.md).

## Observed behavior and intended result

In `examples/sakura-drone-conversion-desktop.mms`, grabbing the machine pistol
does not make the avatar hold it anatomically. The arms remain in their idle
rest/A-pose rather than reaching and adopting a weapon-holding pose.

This is a missing desktop holding capability, not evidence that the existing
XR hand tracking is broken. The user also wants a relaxed A-pose as the default
idle pose; that alone does not solve holding a gun.

Reproduce with:

```sh
cargo run --release -- load examples/sakura-drone-conversion-desktop.mms
```

Grab a machine pistol from the table and inspect the avatar/body in first person
and the mirror. Moving the held object currently does not supply the arms with
the appropriate holding targets or pose.

## Current implementation evidence

- The desktop player has Input, C3D and AVC, but no direct hand pose drivers.
- AVC intentionally creates arm IK only when usable hand targets exist. A mapped
  arm alone must not reactivate the old synthetic-world-origin target behavior.
- The pistol has a Grabbable grip/hand offset and grab callbacks for weapon use.
  Those item-placement and firing callbacks do not establish an avatar arm pose.
- The desktop Rei(mu) GLTF does not currently import an authored relaxed pose.
  Its actual idle pose and the appropriate compatible A-pose asset need checking.

## Investigation tracker

- [ ] Verify a suitable Rei(mu)-compatible relaxed A-pose and how the desktop
  player should select it by default without changing bind translations/scales.
- [ ] Trace desktop pointer grabbing, held-item placement and AVC arm-target
  ownership. Identify where desktop holding targets should enter the shared rig.
- [ ] Decide how weapon grip/contact frames determine the main hand and any
  supporting hand, including finger pose where suitable authored data exists.
- [ ] Define transitions between idle, grab, held/aiming and release, preserving
  head/neck look, locomotion and lower-body ownership.
- [ ] Implement and test the smallest existing-API path; document concrete API
  gaps before introducing a new general holding interface.

## Acceptance

- An idle desktop player defaults to the intended relaxed A-pose.
- Holding the machine pistol places the relevant hand(s) at believable grips
  with bounded arm motion; the arms do not remain in the idle pose.
- Looking, walking, moving the held item and viewing the mirror retain coherent
  item/hand placement without competing transform owners.
- Release restores the appropriate idle pose smoothly and clears holding state.
- Missing targets do not produce origin-reaching arms, and XR hand tracking
  retains its current authority and behavior.
- Validate real grab/release through the desktop scene, not only pose math in
  an isolated arm fixture.
