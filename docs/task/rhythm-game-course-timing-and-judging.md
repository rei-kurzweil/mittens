# Spec: course start, music timing, and body-zone judging

Status: proposed for review. Parent: [VR rhythm game epic](epic/vr-rhythm-game-prototype.md).

This describes the later playable slice. The initial visual prototype adds
target travel/fill, coach poses, winding/fading pictograms, and debug transport
first, without music, countdown, or scoring. Shared setup and session behavior
belong in `examples/rhythm_game/lib/rhythm_game.mms`; it imports a fresh paused
course animation from `lib/example_course.mms`.

## Start and countdown

The game library creates reachable Start, Play/Pause, and Reset controls.
In the playable slice, clicking Start enters a
four-beat countdown and shows `4, 3, 2, 1` at the edge of the player's view.
The countdown is a test aid, not a final-game UI decision. On desktop it can
be screen-edge UI; in XR it should be a comfortable view-anchored indicator,
not a distant world object that disappears when the player looks away.
Repeated Start presses while a countdown/course is running or paused are
ignored. Play/Pause preserves the session phase; Reset clears the session and
returns to idle with a neutral coach and a fresh paused course. The initial
[debug transport contract](rhythm-game-prototype-slices.md#debug-transport-contract)
also applies when countdown and music are introduced.

At the end of the countdown, course beat zero begins and music playback is
scheduled from the same origin. Avoid separate wall-clock timers for music,
target spawning, animation, and judging. MMS beat keyframes express the
course schedule; one session-owned start/epoch maps beat positions to time.
`Clock.bpm(...)`, `Animation`/`Keyframe.at(...)`, and scheduled audio clip
playback exist, but the exact click-to-epoch and clip-start alignment must be
proven in the first slice. A callback that starts audible playback only after
a visual keyframe has fired may be late; test scheduling/lookahead rather
than assuming the two align.

The pre-roll needs enough time for the first target's full travel: if a target
is due at course beat zero, it spawns at `-travel_beats` and starts filling at
`-1`. Keep the four-beat countdown; if travel exceeds four beats, add an
explicit earlier pre-roll or constrain the initial course lead to fit it.
Treat countdown beats as negative course beats or equivalent pre-roll state,
so cue authors do not have to move the whole song four beats forward.

## Target event

For a cue due at beat `b`:

1. At `b - travel_beats`, spawn its outline and small inner shape. Move along
   world Z toward the arrival plane over the configured travel duration
   (initially at least one beat). A detection-only `Zone` shares the cue's
   placement; the early preview cannot score before its due beat.
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

On pause, reset, device loss, or recenter, prevent duplicate judgments and
stale zones. Pause freezes the shared course phase, cue travel/fill/expiry,
coach transitions, pictogram travel/fades, and music when present. Resume
continues that phase without restarting keyframes. Reset cancels scheduled
work from the old epoch and removes its live trees before another Start.
Keep the player's tracking/input active while the game is paused. Identify
the epoch and cue ID in every result. Report at least per-cue role, due
beat, hit/miss/unavailable, and aggregate score to a simple debug UI.

## Acceptance

1. Clicking Start gives four visible beats, then music and course beat zero
   start together; a beat-zero target gets its full travel preview and final
   one-beat fill.
2. Three simultaneous head/hand targets score independently from the correct
   tracked points and authored zones. A missing point is not counted as a
   miss; foot squares appear without contributing to score.
3. Repeated Start clicks, completion, pause/resume, and Reset do not duplicate
   music, cues, pictograms, or judgments. A cue is judged once at its epoch and
   due beat. Resume keeps the phase; Reset restores the initial scene state
   and invalidates the previous epoch's scheduled work.
4. Capture the timing difference among audio onset, visual fill completion,
   and zone sampling on desktop and XR. Set an explicit tolerance after that
   measurement.
