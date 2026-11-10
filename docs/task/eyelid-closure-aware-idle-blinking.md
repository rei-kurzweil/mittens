# Idle blinking when eyelid closure tracking is unavailable

Status: implementation inspection and follow-up design. No blink fallback or new
tracking/morph API is implemented by this change.

Paper-craft now applies `poses/mitten/relaxed-arms.pose.mms` once at GLTF startup
in both screen and XR modes. It changes only upper-arm rotations to an A-pose,
preserves Mitten's bind translations/scales, and leaves tracked arm IK free to
overwrite it. This is independent of the eyelid work below.

## What already exists

- `XREyeTracking.on().priority(...)` is a generic selector over HTC and VRChat
  OSC eye transports. `mediapipe` is accepted as a reserved, unavailable provider;
  the planned webcam component is described in
  [avatar tracking components and MediaPipe](avatar-tracking-components-and-mediapipe.md).
- The current runtime selects gaze and closure independently. It stores
  normalized `EyeClosureSample {left: Option<f32>, right: Option<f32>, sequence}`
  and a closure source independently of gaze. These are internal runtime fields,
  not a current MMS capability/status API.
- AVC consumes normalized closure and maps it to `left_eye_blink` and
  `right_eye_blink`, without requiring authored provider-specific blink logic.
  Disabling pupil-direction tracking does not disable closure tracking.
- Live closure writes `MorphFactorState.driver`; removing the driver restores
  the base value. Closure samples are cleared before each transport poll, so a
  frame without a packet is not itself proof that closure tracking is unsupported.
- Existing tracking-update events are transport-shaped. They are not a generic
  enumeration of available eyelid channels or an authoritative loss notification.

Source: `component/xr_eye_tracking.rs`, `system/xr_eye_tracking_system.rs`, and
`update_eye_blink` in `system/avatar_control_system.rs` under `src/engine/ecs/`.
The [one-shot source election proposal](one-shot-xr-eye-tracking-source-election.md)
would replace the current per-channel arbitration; it remains pending.

## The decision must concern closure, not eye tracking generally

A tracker may supply gaze but no eyelid measurement, or eyelid closure without
gaze. A live closure value of **0 means tracked and open**, not missing. Likewise,
an XR runtime, camera, MediaPipe process, or another tracking provider being
present does not establish that either eyelid channel is supported and usable.

Expose provider-independent semantic status for eyelid closure, with per-eye
support, sample validity, and freshness/liveness. Distinguish unsupported,
temporarily missing, and sustained loss. Use a suitable grace period before
changing fallback ownership so packet cadence does not toggle idle blinking.
Keep source-election policy separate from whether the chosen source supplies
closure. Do not hardcode HTC, OSC, webcam, or future phone-provider checks in AVC
or the authored blink behavior.

## Follow-up implementation

Add a semantic blink/closure or morph-weight authoring operation. `Animation`
keyframes can invoke MMS callbacks, but no direct morph-weight setter is currently
registered in the MMS component API. A scalar blink animation needs that bridge;
it does not require replacing the animation system.

Give procedural idle blinking fallback priority beneath a valid tracked closure
driver. Preserve authored base values and any higher-priority expression owner;
do not overwrite the tracking driver or compete with it. The exact fallback slot
and animation interpolation API still need to be chosen. Screen-mode Mitten can
use it when closure is unavailable, and an XR gaze-only source should also qualify.

Tests should distinguish gaze-only tracking, closure-only tracking, tracked-open
zero, unilateral channels, short packet gaps, sustained loss, tracker return,
missing blink morph mappings, and independent avatars. No automatic blinking is
enabled yet; first land the capability/status and morph authoring contracts.
