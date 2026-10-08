# Epic: simple VR rhythm game prototype

Status: planning updated 2026-10-07. Reusable target/light-strip visuals exist;
the example and gameplay library remain to be built. Current work is documentation.

## Goal

Make a small, playable MMS course in this repository. A button starts a
four-beat countdown, then a music track and a beat-authored course begin from
one timeline. The player places their head, left hand, and right hand inside
visible targets at specified beats. Later courses can score feet and other
body parts when trustworthy tracking exists.

This prototype tests Mittens and the game design together. It should remain
easy to change, with at least one and probably two environments before any
move to the eventual private game repository. Venue art, track choice, and
final game presentation are open decisions. The first example is a generic
test venue rather than a commitment to the collaborator's finished game.

## Four choreographed streams

Every venue defines the same four streams against a common beat timeline:

1. **Player targets:** head and hand circles; later foot squares and possibly
   torso/pelvis targets. A target appears early, fills over one beat, and is
   judged at its due beat.
2. **Pictograms:** miniature skinned stick-figure humanoids winding along a
   readable track, fading in/out and demonstrating upcoming poses.
3. **Coach:** a fully detailed cartoon skinned humanoid character, generally
   front and center, demonstrating the player's intended poses on the same
   timeline. The coach should have a recognizable character appearance.
4. **Venue and atmosphere:** scenery, lighting, background motion, and other
   staged events that reinforce the music without affecting target judgment.

These are authoring streams, not necessarily four engine threads or four
`Animation` components. The detailed [stage contract](../rhythm-game-stage-choreography.md)
defines their independent offsets and what every venue must provide.

## Timing and interaction

Use MMS beat keyframes as the authored course vocabulary. One start action
must establish the countdown, playback, visual cues, and judging origin. A
target with due beat `b` approaches along the runway from `b - travel_beats`,
fills during `[b - 1, b]`, and is checked at `b`. Travel lead is configurable
and at least one beat in the initial proof. The four-beat countdown occurs
before course beat zero. The exact synchronization mechanism, pause/restart
policy, and latency calibration are specified in
[course timing and judging](../rhythm-game-course-timing-and-judging.md).

`Zone` already supplies detection-only sphere/cube geometry and point
classification. Course scoring should use those zone queries at the due beat
against resolved player head/hand points. The visual target and its hit zone
must share placement but keep distinct responsibilities. Foot squares can be
authored and animated now; mark them unscored until foot tracking is available.

## Repository layout

- First example: `examples/rhythm_game/minimal.mms`.
- Gameplay library: `examples/rhythm_game/lib/rhythm_game.mms`, owning course
  assembly/bindings, coach and pictogram setup, Start/Play-Pause/Reset controls,
  session lifecycle, cue travel/expiry, and later judging.
- Initial course: `examples/rhythm_game/lib/example_course.mms`, exporting a
  factory for a fresh paused animation with authored keyframes and pose cues.
- Further examples or venue variants: `examples/rhythm_game/`.
- Reusable prototype-specific MMS factories, including circles and squares:
  `assets/components/rhythm_game/`.
- Design records: this epic and the linked task documents.

The minimal venue uses four studio stages, two end to end on each side,
rotated about Y so their long local X axes run along world Z and their fronts
face the central runway. Player circles approach along world Z. See the
[stage layout](../rhythm-game-stage-choreography.md#minimal-venue-layout).

Start with the hand-authored animation in `example_course.mms`, imported by
`rhythm_game.mms`. Its keyframes use game actions/handles provided by the
library; venue construction and session control stay out of the course file.
Include coach and winding/fading pictograms in the first visual prototype,
with Play/Pause and Reset for debugging before adding music or scoring.

Later, DAW-exported MIDI files can supply keyframes through an explicit mapping
to the same course actions. General MIDI file loading and `MidiInput`/`MidiOutput`
components are tracked in the [MIDI epic](midi/README.md); they do not block
the first visual prototype.

## Tickets

- [ ] [Stage choreography contract for every venue](../rhythm-game-stage-choreography.md)
- [ ] [Course start, beat timing, and zone judging](../rhythm-game-course-timing-and-judging.md)
- [x] [Rename the annulus and add a filled circle renderable](../rename-annulus-and-add-filled-circle-2d.md)
- [ ] [Minimal example and delivery slices](../rhythm-game-prototype-slices.md)

## Proof-of-concept exit

The minimal scene can start, count four beats, play a track, show and judge
head/left/right targets, animate their one-beat fill, and report hits and
misses. A detailed cartoon coach and miniature skinned pictograms visibly
follow the same course poses; pictograms wind along a track and fade in/out.
Play/Pause preserves all game streams' phase and Reset clears the run and
restores its initial state. Foot-square cues are visible but excluded from
scoring. A second environment can reuse the course contract without copying
the timing/judging implementation.
