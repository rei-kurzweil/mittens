# Idle blinking when eyelid closure tracking is unavailable

Status: native opt-in idle blink and a semantic MMS closure-availability getter
are implemented. A general scriptable morph-weight setter remains future work.

Paper-craft opts into idle blinking in both screen and XR modes. Mitten also has
an arms-only startup relaxed pose, now 15 degrees from the sides (75 degrees
below horizontal), preserving its bind translations/scales and allowing tracked
arm IK to overwrite it afterward.

## Authoring and behavior

```javascript
AVC {
    idle_blink(true)
    blink_interval(4.0)
    blink_duration(0.18)
}
```

Intervals/durations are seconds. The native curve closes quickly and reopens
more slowly with smooth easing. Defaults are 4 seconds and 0.18 seconds; idle
blinking is disabled unless explicitly authored. Duration is capped at half
the interval. The timer is per-avatar and uses the simulation delta.

`avatar.eyelid_tracking()` returns `{left: bool, right: bool}` for usable closure
samples, with a 250 ms grace period across packet gaps. It reports closure,
not gaze and not merely the existence of a tracker component. A live zero
means tracked and open. Gaze-only tracking does not suppress idle blinking.

The existing generic selector handles configured provider priorities and feeds
normalized closure to AVC. Neither the authored scene nor the idle-blink curve
needs HTC, OSC, or future webcam/provider-specific availability checks. The
MediaPipe source is still a reserved, unavailable entry; this does not implement
a webcam or phone tracking backend.

## Ownership

Live eyelid closure is the primary morph driver. With idle blinking enabled,
the most recent closure value is held for up to 250 ms when packets are missing;
sustained loss releases that eye to the idle curve. A returning valid sample
immediately takes priority, including an open value of zero. Each eye is handled
independently after provider normalization.

`MorphFactorState.idle_blink` is a separate fallback from `driver`. It leaves
base factors intact; while untracked it can close an eye further than its base
value, and between blinks it restores the base. Tracked closure still wins over
the fallback. Disabling idle blinking clears the fallback. Avatars that do not
opt in retain the previous immediate-release tracking behavior.

Missing blink mappings do not fabricate targets. Paper-craft maps the semantic
`left_eye_blink` / `right_eye_blink` channels to Mitten's imported blink morphs.

## Current implementation and remaining work

- `component/eye_blink_state.rs` retains per-eye closure liveness and the curve.
- `system/avatar_eye_blink.rs` consumes normalized tracking and writes mapped
  morph drivers/fallbacks. `MorphFactorState::effective` composes the result.
- MMS exposes the three builders above and the semantic availability getter.
- The existing eye selector still chooses gaze and closure independently; the
  [one-shot election proposal](one-shot-xr-eye-tracking-source-election.md)
  remains pending. Availability is distinct from that source-election policy.

A fully general channel capability/status contract (support, provider identity,
validity and freshness) is still a future extension. The new getter answers
whether closure is currently usable; it does not enumerate every provider or
claim gaze availability implies eyelid support.

`Animation` callbacks could drive authored morph weights once a setter and its
ownership rules exist. This first idle blink uses a native AVC curve rather
than adding per-frame MMS callbacks or a general morph animation API.

Tests cover gaze-only input, tracked-open zero, independent eyes, short packet
gaps, sustained loss, base restoration, independent avatar clocks, and the real
paper-craft model closing and reopening its imported blink morph. Related:
[tracking components and MediaPipe](avatar-tracking-components-and-mediapipe.md).
