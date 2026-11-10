# Screen players and untracked holding pose drivers

Status: paper-craft's player-mode switch is implemented; procedural holding arms
are a future design goal, not implemented by this change.

## Try paper-craft without a headset

At the top of `examples/paper-craft.mms`, change just this line:

```javascript
let use_openxr = false
```

Run `cargo run --release -- load examples/paper-craft.mms` as usual. `true` retains
the existing OpenXR rig. `false` creates one screen rig with a first-person C3D
camera, keyboard/mouse Input, the mapped Mitten avatar, walking/collision/gravity,
and the same rooftop spawn and workshop. Screen mode explicitly disables XR and
does not instantiate the XR hands, XR camera, hand tracking, or calibration panel.
Cars use their desktop controls in this mode.

Screen controls today: WASD movement, right mouse drag to look, left mouse drag
to grab and hold props, release to drop, Space to jump when grounded, and F to
fire a held pistol. The weapon selector still chooses semi/burst/auto. F has no
effect on a pistol left on the table or released. Vehicle controls retain their
existing mount/power gating and keyboard bindings.

Current screen grabbing retains the generic camera-relative holding behavior.
It can pick up the gohei, but does not pose an avatar arm to hold it. Switching
player modes does not implement procedural arms or manufacture tracked hands.

## Naming and separate capabilities

Use **screen presentation** for a scene viewed through a conventional flat
display, including desktop windows and a possible future Android phone/tablet
window. The world can still be 3D. Winit is today's window/input backend, not
the general category; Android does not imply OpenXR or a headset.

Keep these choices independent when this grows beyond the example's boolean:

- Presentation: screen versus immersive/XR.
- Platform/backend: desktop Winit, a future Android screen backend, or an XR host.
- Input actions: keyboard/mouse, touch, gamepad, or tracked-controller buttons.
- Pose capabilities: separately available head and hand tracking, each with a
  provider and validity, plus procedural targets when explicitly enabled.

A screen player may have MediaPipe/webcam hand tracking. A future phone player
may have ARCore head/device tracking but no hand tracking. Neither should need
to pretend to be an XR controller. These are design cases, not newly supported
Android screen, MediaPipe, or ARCore integrations in this change.

## Desired holding behavior

When a pointer grabs a pistol, gohei, or another handled prop, and the player
has an avatar with validated upper-arm, lower-arm, and hand mappings, a native
procedural pose driver should be able to supply that hand's holding target.
AVC should route it through the existing arm IK machinery. This should work
without OpenXR and without any live hand tracking.

The resulting arm reaches for and holds the prop, rather than leaving it
floating in front of the camera while the avatar's arms remain in their base
pose. Looking, moving, aiming, firing, dropping, and switching props should
preserve a coherent hand/prop relationship, visible in mirrors and other views.

Current AVC eligibility explicitly requires an XR hand driver for arm IK; see
[AvatarControl](../spec/avatar-control.md). The future work must generalize pose
driver eligibility and target acquisition, rather than merely enabling that
existing XR-only check on desktop or fabricating identity targets.

## Proposed opt-in and ownership policy

Start with an explicit per-avatar opt-in for procedural holding, a selected
holding hand/slot per pointer, and a per-prop opt-out where ordinary dragging
is preferable. Exact MMS API names and whether this eventually becomes a
default remain undecided. Do not present a speculative builder as implemented.

A valid tracked hand remains authoritative. The procedural driver should own
only the selected untracked arm while holding; it must not overwrite the other
arm, tracked fingers, head, locomotion, or secondary motion. A missing avatar,
missing mappings, or explicit opt-out retains ordinary pointer holding. On
release, disable, avatar removal, or tracking takeover, blend ownership back
without leaving stale IK targets. Tracking-loss transitions need an explicit
policy rather than treating every missing sample as an immediate ownership flip.

Keep pointer aiming, palm orientation, and tool grip frames distinct. A palm
holding/punch direction is not the same frame as the pointing index finger,
and a pistol's diagonal handle differs from a straight gohei shaft. Author
tool-specific grip translation/orientation without rotating every pointer or
changing the established tracked-hand pose globally.

## Implementation direction for later

Generate a hand target from the player's head/body frame, selected pointer aim,
validated rest geometry, reach limits, and the prop's authored grip frame. Use
the shared two-bone arm solver and elbow/bend-plane tuning. Optional finger or
prop-specific holding poses can be layered separately.

Avoid a feedback loop: derive the desired target independently of the prop's
final hand-attached transform, solve the arm, then attach the prop at its grip.
Projectile launches should sample the resulting muzzle pose. Define that native
frame order alongside head/body solving, pose layers, and attachment updates.

Retain resolved joint/driver bindings and state per arm. Target updates belong
in the native pose/AVC pipeline, without repeated selector discovery, fake XR
input events, or one MMS FrameTick callback per hand. Capability and validity
should choose the source; platform names should not choose the solver.

Acceptance should include picking up and firing a pistol with keyboard/mouse,
holding the adjusted gohei, release/regrab, left/right hand selection, independent
avatars, missing-bone fallback, and tracked-hand takeover without pose fights.
Future touch and partial-tracking cases should exercise the same driver contract.

Related: [shared AVC setup and untracked walking](shared-avc-setup-and-untracked-walking.md),
[pointer input and gestures](../spec/pointer-input-ray-gesture.md), and
[locomotion/armature animation](../desktop/locomotion-and-armature-animation.md).
