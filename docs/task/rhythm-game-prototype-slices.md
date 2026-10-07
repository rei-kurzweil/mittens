# Task: build the minimal rhythm game prototype in slices

Status: revised delivery plan, 2026-10-07; reusable visuals exist, minimal
example remains to be built. Parent: [VR rhythm game epic](epic/vr-rhythm-game-prototype.md).

## Repository structure

Put the first example at `examples/rhythm_game/minimal.mms`. Define gameplay
in `examples/rhythm_game/lib/rhythm_game.mms`: course animation construction,
session start/completion/replay, moving cue creation and expiry, and later
transport and judging. Keep venue composition, player setup, and the Start
button in `minimal.mms`; the button delegates to the library's session API.
Keep reusable visual factories in `assets/components/rhythm_game/`.

The library should build a finite course animation from cue data, initially
hand-authored in MMS. Course records describe role, due beat, placement,
travel duration, size/color, and feedback hold. Exact exported function names
and record syntax will be settled during the first implementation proof.
Fresh runs get fresh session/cue trees; the library owns their lifecycle.

Later, a DAW-exported MIDI file can supply part of that cue data through an
explicit mapping. Importing MIDI should produce the same course records and
use the same animation builder as hand-authored data. MIDI parsing and device
I/O belong to the engine's [MIDI epic](epic/midi/README.md); mapping musical
events to body roles, poses, and venue events belongs to the game library.
The basic prototype and library come first, without a MIDI dependency.

## First visual runway slices

The first example should establish the venue and incoming cues before adding
music or judgment. A reachable Start button begins a short, finite course
animation. Start presses during playback are ignored; after every cue expires,
the button can start a fresh run. No notes move or spawn before Start.

1. **Venue and Start.** Create `examples/rhythm_game/minimal.mms` with Bloom,
   a declared player arrival plane, and a runway bordered by gold/yellow LED
   strips. Reuse `light_strip` with explicit warm color and strong emissive
   intensity, initially around 4.0 and then tuned in-headset. Use exactly four
   `studio_stage` instances: two end to end on each side of a central runway.
   Rotate each stage about Y by a quarter-turn so its long local X axis runs
   along world Z. Use opposite rotations on the two sides so the open fronts
   face inward and the back walls sit outside the runway. Circles approach
   along world Z toward the player arrival plane. See the
   [layout contract](rhythm-game-stage-choreography.md#minimal-venue-layout).
   Repeat tripod lights along both outer walls, aimed into the runway.
   Each stage includes a 32 m wide wall and a 26 m truss, so repeating it
   every 32 m leaves truss
   gaps: add connecting truss sections or author a continuous truss separately.
   Keep the button and arrival area close to the player rather than scaling
   interaction distances with the scenery.
2. **One moving circle with owned expiry.** Add a cue factory in
   `examples/rhythm_game/lib/rhythm_game.mms` around `circle_pose_marker`.
   Its placement root owns a finite local animation:
   approach the arrival plane, begin the existing one-beat fill during the
   final beat before arrival, hold briefly for feedback, then remove the
   entire cue subtree, including its animation. Travel lasts a configurable
   number of beats; arrival is its due beat. Verify runtime spawning, linear
   beat-timed travel, and self-removal before relying on this factory for a
   full course.
3. **Repeated three-circle sets.** The library builds a course animation whose
   keyframes spawn simultaneous `head`, `left_hand`, and `right_hand` cues
   through the same factory, with
   distinct colors and explicit role/position configuration. Author left and
   right from the player's perspective. Start with a few readable sets,
   including two sets in flight at once. Let the course finish only after the
   final set's expiry, then allow replay. Verify no cue trees accumulate
   across repeated runs. Squares/feet follow later.

This is a visual prototype: it establishes travel, arrival, fill, and lifetime
without scoring. The longer approach previews the target; the final one-beat
fill keeps the existing timing vocabulary. The stage and timing contracts
distinguish travel lead from this final fill interval.

## Cue ownership and implementation proof

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

Keep a cue-set root under a session-owned live-cues root. A future restart or
cancel may remove that root in one operation; normal expiry remains the cue's
responsibility. A fresh cue instance on each spawn also avoids replaying the
existing marker's fill from its already expanded scale.

Acceptance for this first proof: before Start there are no live cues; during
playback incoming sets overlap, reach the same arrival plane at their authored
beats, fill during the last beat, and disappear after their configured hold;
after completion the live-cues root is empty and another Start reproduces the
sequence without stale animation callbacks or accumulating scene components.

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
4. **Guide track.** Add simple posed stick figures on a linear right-to-left
   lane, synchronized with the same course cues and retired after due beats.
5. **Coach and atmosphere.** Add one skinned humanoid coach with several
   readable pose transitions and a small set of beat-timed venue events.
6. **Second environment and path experiment.** Reuse timing, targets, and
   scoring through the same library in another environment. Try a curved guide
   approach from the right; keep it only if it reads clearly in VR.

Each slice should leave a runnable example and a concrete observation. The
first three slices are the playable core. The guide, coach, and second venue
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
