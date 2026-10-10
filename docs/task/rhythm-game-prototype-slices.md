# Task: build the minimal rhythm game prototype in slices

Status: revised delivery plan, 2026-10-07; reusable visuals exist, minimal
example remains to be built. Parent: [VR rhythm game epic](epic/vr-rhythm-game-prototype.md).

## Repository structure

Use three files with distinct responsibilities:

- `examples/rhythm_game/minimal.mms` composes the four-stage venue, lighting,
  player/camera setup, arrival plane, and placement of the game and controls.
- `examples/rhythm_game/lib/rhythm_game.mms` imports the course factory and
  sets up its bindings to targets, coach, and pictograms. It owns the session,
  live cue trees, coach setup, pictogram track, and reachable Start, Play/Pause,
  and Reset controls. Its initial focus is assembling and controlling a finite
  `Animation` with keyframes, using existing engine APIs.
- `examples/rhythm_game/lib/example_course.mms` exports a factory that returns
  a fresh paused course `Animation` with hand-authored `Keyframe.at(...)` blocks.
  It receives a context of game actions/handles rather than constructing a
  venue or debug buttons. Keyframes describe target sets and shared pose cues;
  timings and poses can be edited here without changing session behavior.

Keep reusable visual factories in `assets/components/rhythm_game/`. Course
events describe role, due beat, placement/pose, travel duration, size/color,
and feedback hold. Begin with a small explicit animation rather than requiring
a generic course-data compiler. Exact factory/action names will be settled
during implementation. Fresh runs get fresh session/cue trees and a fresh
course animation; the game library owns their lifecycle.

Later, a DAW-exported MIDI file can supply part of that cue data through an
explicit mapping. Importing MIDI should eventually produce an animation with
the same course actions/bindings used by `example_course.mms`, whether through
intermediate records or generated keyframes. MIDI parsing and device
I/O belong to the engine's [MIDI epic](epic/midi/README.md); mapping musical
events to body roles, poses, and venue events belongs to the game library.
The basic prototype and library come first, without a MIDI dependency.

## First visual runway slices

The first example establishes the venue, course animation, target travel,
coach, and winding pictogram track before adding music or judgment. A reachable
Start button begins a short, finite course animation. Play/Pause and Reset
are included for debugging, with the behavior described below. No target or
pictogram moves or spawns before Start; the coach can stand in its neutral pose.

1. **Venue, course wiring, and controls.** Create `minimal.mms` and the two
   library files. Import a fresh paused animation from `example_course.mms`
   into `rhythm_game.mms` and bind its actions to session-owned scene objects.
   Compose the scene with Bloom and a declared player arrival plane.
   LED strips are reserved for the standalone
   [custom-materials preparation example](custom-materials-led-strip-example-preparation.md)
   for now; they are not part of this first rhythm-game venue. Use exactly four
   `studio_stage` instances: two end to end on each side of a central runway.
   Rotate each stage about Y by a quarter-turn so its long local X axis runs
   along world Z. Use opposite rotations on the two sides so the open fronts
   face inward and the back walls sit outside the runway. Circles approach
   along world Z toward the player arrival plane. See the
   [layout contract](rhythm-game-stage-choreography.md#minimal-venue-layout).
   Repeat tripod lights along both outer walls, aimed into the runway.
   Each stage includes a 32 m wide wall and a 26 m truss, so repeating it
   every 32 m leaves truss gaps: add connecting truss sections or author a
   continuous truss separately.
   Keep the button and arrival area close to the player rather than scaling
   interaction distances with the scenery.
   Prove Start, Play/Pause, and Reset with one visible course action before
   expanding the choreography.
2. **One moving circle with owned expiry.** Add a cue factory in
   `examples/rhythm_game/lib/rhythm_game.mms` around `circle_pose_marker`.
   Its placement root owns a finite local animation:
   approach the arrival plane, begin the existing one-beat fill during the
   final beat before arrival, hold briefly for feedback, then remove the
   entire cue subtree, including its animation. Travel lasts a configurable
   number of beats; arrival is its due beat. Verify runtime spawning, linear
   beat-timed travel, and self-removal before relying on this factory for a
   full course.
3. **Repeated three-circle sets.** The imported example course has keyframes
   that spawn simultaneous `head`, `left_hand`, and `right_hand` cues through
   the same factory, with distinct colors and explicit role/position
   configuration. Author left and
   right from the player's perspective. Start with a few readable sets,
   including two sets in flight at once. Let the course finish only after the
   final set's expiry, then allow replay. Verify no cue trees accumulate
   across repeated runs. Squares/feet follow later.
4. **Coach and pictograms.** Bind a few shared pose cues to a detailed cartoon
   skinned humanoid coach and miniature skinned stick-figure pictograms.
   Pictograms wind along a configurable track, fade in on entry, demonstrate
   their associated pose, and fade out before subtree retirement. The coach
   stays near front center, clear of targets. Verify both models can display
   the intended poses; matching bone names across rigs is not assumed.
   Pause and Reset must cover these streams as well as targets.

This is a visual prototype: it establishes target travel, arrival, fill, and
lifetime alongside the coach and pictogram streams, without scoring or music.
The longer approach previews the target; the final one-beat fill keeps the
existing timing vocabulary. The stage and timing contracts
distinguish travel lead from this final fill interval.

## Cue ownership and implementation proof

The course factory returns a fresh animation for the current session context.
`rhythm_game.mms` attaches it and supplies the actions used by its keyframes.
Course content stays in `example_course.mms`; cue movement, coach/pictogram
setup, control handling, and cleanup stay in `rhythm_game.mms`.

Course keyframes should contain only new cue information: role, placement,
travel duration, size/color, and due beat (or spawn beat plus lead duration).
The cue factory owns its movement and retirement. Do not put removal of an
earlier note in a later course keyframe.

MMS already exposes `Animation`, `Keyframe`, `Transition`, runtime `attach`,
and `remove_subtree()`. First attempt a cue-owned animation using those APIs;
a new engine lifetime component is not assumed necessary. The proof must check
that a dynamically attached cue starts its timeline at spawn, transition
travel is linear, and removing a cue from its own final callback safely
cleans up keyframes and visual instances. A detached component is not an
expired component: use actual subtree removal.

Keep cue-set and pictogram roots under session-owned live roots. Reset removes
those roots and the old course animation; normal expiry remains each cue's
responsibility. A fresh cue instance on each spawn also avoids replaying the
existing marker's fill from its already expanded scale.

Acceptance for this first proof: before Start there are no live cues; during
playback incoming sets overlap, reach the same arrival plane at their authored
beats, fill during the last beat, and disappear after their configured hold;
after completion the live-cues root is empty and another Start reproduces the
sequence without stale animation callbacks or accumulating scene components.

## Debug transport contract

- **Start:** starts a fresh run from idle. Ignore repeated Start while running
  or paused. After completion it can start a fresh run.
- **Play/Pause:** pauses a running session and resumes a paused session at the
  same course phase. Before Start and after completion it is inactive; it
  does not silently create another run.
- **Reset:** works during playback, while paused, and after completion. Stop
  the old session, remove its course and live target/pictogram subtrees, cancel
  stale callbacks, restore the coach's neutral pose, and create a fresh paused
  course ready for Start. Reset does not auto-start.

Show the current state and course beat for debugging. Pausing must freeze
course events, cue travel/fill/expiry, coach pose transitions, and pictogram
travel/fades together. Pausing just the parent animation is insufficient if
child animations or transitions continue against the world clock.

Existing `Animation.play()` restarts at local beat zero; it is not a resume
operation. Prove how the session retains its phase before labeling a control
Resume. Prefer a shared session timeline or explicit phase-driven updates;
if existing APIs cannot support this, record and implement the smallest
required engine change as a focused task. Do not pause unrelated world input
or the player's tracked viewpoint. Later audio playback must join this same
transport policy.

Acceptance includes pausing mid-travel/mid-fill and during a pictogram fade,
waiting, then resuming without a jump, duplicate spawn, or early expiry. Reset
from each state must return to empty live roots and neutral coach; repeated
Start/Reset cycles must not accumulate callbacks or components.

## Later playable slices

1. **Start and transport proof.** Add a minimal stage, Start button, four-beat
   edge-of-view countdown, a short music clip, and a shared course epoch.
   Log/inspect visual beat and playback onset; establish restart behavior.
2. **One head cue.** Add circle outline/fill animation and a sphere `Zone`.
   Author a due beat in MMS, test the player's head point once at that beat,
   and show hit/miss/unavailable. This establishes cue identity and timing.
3. **Hands and feet presentation.** Add simultaneous left/right hand circles,
   per-role scoring, and foot-square visuals with scoring disabled. Extract
   reusable factories under `assets/components/rhythm_game/`.
4. **Coach/pictogram refinement.** Expand the initial pose vocabulary and tune
   winding-track readability, scale, fades, and preview intervals in-headset.
5. **Atmosphere.** Add a small set of beat-timed venue events.
6. **Second environment and path experiment.** Reuse timing, targets, and
   scoring through the same library in another environment. Vary the winding
   pictogram track without changing the imported course animation contract.

Each slice should leave a runnable example and a concrete observation. The
first three slices are the playable core. The pictograms, coach, and second venue
are needed for the broader epic proof, with art quality deliberately modest.

## Dependencies and boundaries

Existing pieces include MMS beat keyframes, `Clock.bpm`, a clickable button
factory, clip playback, skinned models, and detection-only `Zone` point
queries. Required integration work includes a shared course start, reliable
player body-point resolution, per-cue score ownership, and measuring
audio/visual/input timing. Do not count the existing Zone query as a complete
rhythm judge or the existing animation examples as proof of audio sync.

Review the [four-stream stage contract](rhythm-game-stage-choreography.md)
and [timing/judging contract](rhythm-game-course-timing-and-judging.md) before
implementing the first slice. Track any new reusable engine feature in its
own task once the example exposes a specific limitation.
