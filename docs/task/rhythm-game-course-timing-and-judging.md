# Spec: course start, music timing, and body-zone judging

Status: proposed for review. Parent: [VR rhythm game epic](epic/vr-rhythm-game-prototype.md).

## Start and countdown

The first example has a reachable `Start` button. Clicking it enters a
four-beat countdown and shows `4, 3, 2, 1` at the edge of the player's view.
The countdown is a test aid, not a final-game UI decision. On desktop it can
be screen-edge UI; in XR it should be a comfortable view-anchored indicator,
not a distant world object that disappears when the player looks away.
Button presses while a countdown or course is active need a defined response:
the first slice should ignore them and offer an explicit restart after finish.

At the end of the countdown, course beat zero begins and music playback is
scheduled from the same origin. Avoid separate wall-clock timers for music,
target spawning, animation, and judging. MMS beat keyframes express the
course schedule; one session-owned start/epoch maps beat positions to time.
`Clock.bpm(...)`, `Animation`/`Keyframe.at(...)`, and scheduled audio clip
playback exist, but the exact click-to-epoch and clip-start alignment must be
proven in the first slice. A callback that starts audible playback only after
a visual keyframe has fired may be late; test scheduling/lookahead rather
than assuming the two align.

The pre-roll needs enough time to show the first target: if a target is due
at course beat zero, it must appear during the final countdown beat (`-1`).
Treat countdown beats as negative course beats or equivalent pre-roll state,
so cue authors do not have to move the whole song four beats forward.

## Target event

For a cue due at beat `b`:

1. At `b - 1`, show its outline and a small inner shape; enable or place its
   detection-only `Zone` with the cue's stage transform.
2. Over `[b - 1, b]`, expand the inner shape to fill the outline. This visual
   may interpolate continuously; it does not decide the hit.
3. At `b`, sample the configured body-point world position and classify it
   against the cue's current zone. Record a hit or miss for scored roles.
4. Show brief result feedback and retire the cue after a short, authored
   interval. Never let an expired zone score a later cue.

The first point test can use the existing `Zone` sphere/cube constructors and
transform-aware point classifier. It must resolve head, left hand, and right
hand from the active player/avatar tracking source, not from the coach or a
render-only guide. Define behavior for missing tracking: show `unavailable`
or unscored for that cue, not an ordinary miss. Foot-square cues are always
unscored in the first slice. Later scoring may need foot/pelvis probes and
body-volume overlap rather than one point; preserve that distinction.

## Timing policy to measure

The initial rule is a single test at the due beat, matching the authored
keyframe. Measure input pose timestamp, simulation tick, rendered cue phase,
and audio playback position before choosing a forgiving timing window. If
latency makes a single sample unfair, add an explicit window or pose-history
query centered on the due beat; do not silently use whatever pose happens to
be available. Record the chosen window and calibration offset in a venue or
session policy, not in every target factory.

On pause, restart, device loss, or recenter, prevent duplicate judgments and
stale zones. The first playable slice may support only start, finish, and
restart, but it should identify the epoch and cue ID in every result so later
transport controls can be added safely. Report at least per-cue role, due
beat, hit/miss/unavailable, and aggregate score to a simple debug UI.

## Acceptance

1. Clicking Start gives four visible beats, then music and course beat zero
   start together; a beat-zero target gets its full one-beat preview.
2. Three simultaneous head/hand targets score independently from the correct
   tracked points and authored zones. A missing point is not counted as a
   miss; foot squares appear without contributing to score.
3. Repeated Start clicks, completion, and restart do not duplicate music,
   cues, or judgments. A cue is judged once at its epoch and due beat.
4. Capture the timing difference among audio onset, visual fill completion,
   and zone sampling on desktop and XR. Set an explicit tolerance after that
   measurement.
