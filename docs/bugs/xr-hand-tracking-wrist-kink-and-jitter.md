# XR hand tracking wrist orientation jitters and kinks the forearm on palm-down poses

## Status

Partially mitigated; runtime validation and forearm-twist work remain open.

Focused follow-up task:

- [docs/task/avc-forearm-roll-visual-hand-offset-alignment.md](../task/avc-forearm-roll-visual-hand-offset-alignment.md)

## Symptom

When XR hand tracking is active and takes over `CTLXR` pose driving, the resulting hand orientation
looks materially worse than the same headset/runtime path in SteamVR:

- hand/wrist orientation appears jittery in-engine
- the avatar hands can look correct in the neutral/default case, especially on a VRoid model
- but rotating the real hands into palm-down poses produces a visible kink/twist between forearm
  and wrist
- palm-up poses do not show the same obvious kink

The visual result is that the hand itself can appear approximately correct while the arm-to-wrist
connection looks anatomically wrong, as if the wrist rotated but the forearm did not follow.

## Repro

Current repro shape:

- XR scene using `InputXR`
- `CTLXR`-driven authored hand/controller transforms
- hand tracking active
- avatar driven through the current AVC/arm setup

Observed behavior:

- if controllers are not actively driving poses, hand tracking can take over `CTLXR`
- with a VRoid avatar, the resting hand orientation can look plausibly aligned by default
- when the user turns their real hands palm-up, the wrist/forearm transition still looks mostly
  acceptable
- when the user turns their real hands palm-down, the wrist appears to twist sharply relative to
  the forearm, producing a kink at the joint
- subjective orientation stability is worse than the same headset hand tracking as seen in SteamVR

## Expected behavior

- hand-tracked orientation should be temporally stable enough that wrist rotation does not visibly
  jitter during ordinary motion
- forearm and wrist should remain anatomically continuous through palm-up and palm-down rotations
- changing wrist rotation should not look like an isolated twist applied only to the hand end of
  the chain

## Current implementation detail that matters

The current engine hand-tracking path does **not** yet use full per-finger/per-joint armature
driving. It reduces hand tracking to a single per-hand root pose.

Current root-pose construction in
[src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs):

- prefer `WRIST` position and fall back to `PALM` position
- prefer a valid root reported as unobstructed tracking over emulated controller actions;
  controller-derived and unclassified joints retain controller priority
- prefer runtime `PALM` orientation, then `WRIST`, converting OpenXR joint axes into the
  per-side canonical hand frame
- when neither orientation is usable, synthesize from middle-metacarpal forward and
  little-to-index across the palm
- pass that canonical frame through the same retained avatar-basis correction as controller Aim

That means the visible hand-tracked `CTLXR` result is currently driven by a simplified root pose,
not by a stabilized full skeletal hand solve. The runtime root uses the specified OpenXR joint
axes rather than inferring rotation from knuckle positions. AVC/IK forearm twist handling remains
separate from root acquisition; full finger driving is still pending. See the
[current wrist-frame audit](../task/hand-tracking-wrist-arm-and-finger-basis-audit.md) for the
Focus 3 handoff and yaw/roll reports.

## Why this may be happening

Likely contributors include:

- tracked landmark positions may still require temporal filtering before basis construction
- the engine may need smoothing/filtering for hand-tracked orientation before applying it to the
  avatar
- the forearm may need to participate in the rotation chain more explicitly instead of treating the
  hand target as an isolated endpoint

## Investigation targets

- [src/engine/ecs/system/openxr_system.rs](../../src/engine/ecs/system/openxr_system.rs)
- [src/engine/ecs/system/avatar_control_system.rs](../../src/engine/ecs/system/avatar_control_system.rs)
- [src/engine/ecs/component/avatar_control.rs](../../src/engine/ecs/component/avatar_control.rs)
- [docs/spec/vr-input.md](../spec/vr-input.md)
- [docs/spec/hand-tracking-armature.md](../spec/hand-tracking-armature.md)

Questions to answer:

- is `WRIST` actually the right orientation source for the current avatar hand target, or would
  `PALM` or a synthesized frame be better?
- how much of the visible instability is raw hand-tracking jitter versus engine-side pose choice?
- are current AVC hand/wrist builder defaults compensating for one neutral pose while breaking
  palm-down rotation?
- should forearm twist be distributed through an arm chain rather than landing entirely at the
  wrist target?
- would a quaternion temporal filter materially improve the subjective hand-tracking result before
  any armature-level refactor?

## Notes

This bug is separate from the controller-action failure currently under investigation.

Even if controller buttons/sticks/triggers are dead, the hand-tracking pose path can still move
`CTLXR`, and that path needs to be evaluated on its own terms for:

- orientation stability
- wrist semantic correctness
- avatar forearm continuity
