# Spec: four beat-authored streams in each rhythm venue

Status: proposed for review. Parent: [VR rhythm game epic](epic/vr-rhythm-game-prototype.md).

## Common course language

A venue owns one beat-indexed course. Its authoring should describe *what is
due when* separately from how early each stream previews it. The same due
beat can cue a player target, a pictogram pose, a coach pose, and scenery. Each
stream may have its own lead offset, transition duration, and cleanup beat.
The gameplay library at `examples/rhythm_game/lib/rhythm_game.mms` imports a
fresh paused `Animation` from `lib/example_course.mms` and binds its keyframes
to target, coach, and pictogram actions. The course file owns authored timing
and pose cues; the game library owns setup, session transport, debug controls,
and cue lifetimes. Visual factories live in `assets/components/rhythm_game/`.
Later a DAW-exported MIDI file can supply keyframes through an explicit mapping
to the same course actions, optionally using intermediate course records.
General file loading is tracked in the [MIDI epic](epic/midi/README.md).

Use a single world/stage coordinate frame for authored target positions.
Describe left and right from the player's perspective. Distinguish the
player's body tracking origin from the coach's model origin. A venue may move
or rotate the course frame, but all target zones and cue visuals must follow
the same transform. Support standing play first; calibration and room-scale
placement remain explicit follow-up decisions.

| Stream | Authored event at due beat `b` | Preview and completion | First slice |
|---|---|---|---|
| Player targets | Body role, world/stage pose, zone size, whether scored | Spawn at `b - travel_beats`, approach along world Z; fill during `[b - 1, b]`; arrive/judge at `b`, then retire | Visual runway first; later score head/hand circles and show unscored foot squares |
| Pictograms | Pose and arrival/departure beats | Miniature skinned stick figures wind along a track, fade in on entry and out before removal | A few posed models on a winding track with controlled lifetimes |
| Coach | Demonstration pose and transition interval | Begin transition early enough to be readable; hold through due beat, then continue | One detailed cartoon skinned humanoid at front center with a few distinct poses |
| Venue/atmosphere | Cue and active interval | Art-directed; independent lead and duration | One modest environment with beat-timed background events |

## 1. Player targets

The first target kinds are `head`, `left_hand`, and `right_hand`, each shown as
a circle with an outer outline and inner fill. The moving outline appears at
`b - travel_beats`, with travel lasting at least one beat. The inner shape stays
small until `b - 1`, then fills over the final beat and reaches the outline at
the due beat as the cue reaches the arrival plane. A target's placement
and its `Zone.sphere(...)` should derive from the same authored cue. A target
may be simultaneous with others; hit results are per role and per cue.

Use `R.annulus_2d()` for the outline (inner radius 0.45, outer radius 0.5)
and the filled `R.circle_2d()` for the inner shape. The
[renderable naming task](rename-annulus-and-add-filled-circle-2d.md) supplied
both constructors. A prototype circle factory can give both surfaces the
same color and expand the disk from the center until its edge reaches the
annulus. Keep the disk slightly in front if coplanar depth causes flicker.

Add `left_foot` and `right_foot` squares in the later hands/feet slice, using
box-shaped zones and the same one-beat fill rule, with `scored = false`.
The initial visual runway proves circles first. Later tracked feet can
activate scoring without changing the visual course data. Torso and pelvis
are possible later roles; neither requires a
first-slice visual or judge. Define each role's measurement point and zone
size in the course timing task rather than relying on a model bone name.

## 2. Pictograms

Pictograms are miniature skinned stick-figure humanoid models showing upcoming
body arrangements. They travel along a winding track with authored fade-in
and fade-out intervals. Their poses correspond to the coach and target sets
at the same due beats; one pictogram can preview several simultaneous targets.
Track travel, pose timing, fades, and retirement all use the session timeline.
The first course needs only a few readable poses and overlapping previews.

Author the path independently from poses so another venue can change its bends
without rewriting the course. Keep lead/travel duration configurable and
separate from the targets' final one-beat fill. Define track coordinates,
orientation along the track, arrival/hold interval, and fade boundaries in
the game setup. Fade completion is followed by actual subtree removal.
Verify fading covers the complete skinned model and remains readable in VR.

`assets/models/capsule_stick_figure.glb` is a candidate existing asset. The
[retargeting laboratory](capsule-stick-figure-pose-retargeting.md) currently
documents a static A-pose baseline and unresolved cross-rig pose mapping.
Prove suitable skinning and pose application before depending on this model;
do not assume the coach's joint names can be reused. Small model-specific pose
factories or a verified humanoid mapping can supply equivalent poses initially.

In VR, test the track from the expected forward-facing play position and allow
head turning; never define timing by literal screen pixels. Pictogram travel
is separate from circles approaching along world Z.

## 3. Coach

The coach is a fully detailed cartoon skinned humanoid near front center,
with a recognizable character appearance rather than a blank mannequin.
An existing detailed model such as Bisket is a candidate; final asset choice
and any additional model remain open. Its poses
demonstrate what the player should do at target beats. The model, pose
transitions, and cue mapping should be reusable across courses. The first
visual slice uses a small pose library; it must show that coach transitions are
beat-aligned, rather than simply reacting after scoring. How tracked player
motion and coach animation coexist is outside this prototype's first slice.

## 4. Venue and atmosphere

The fourth stream covers lighting, backgrounds, props, and other non-scoring
events. It uses the same course clock but may react before, on, or after a
beat. A venue is free to omit most events in this stream. They must not block
judgment or alter target-zone placement accidentally.

## Venue template and open choices

### Minimal venue layout

Use exactly four instances of `studio_stage`: two end to end on the left and
two on the right of a central runway. The existing asset is 32 m long along
local X, with its open/stepped front toward local +Z and back wall toward
local -Z. Rotate the left row by +pi/2 about Y and the right row by -pi/2;
this aligns both long axes with world Z and turns the open fronts inward.
Keep the stage assets at their authored scale. Space each row's centers 32 m
apart along Z. Their 26 m trusses leave 6 m gaps that need connecting sections.

Player targets move along world Z; pictogram travel remains a separate stream.
As an initial placement convention, put the arrival plane at world `z = 0`,
have the player face toward -Z, and spawn circles at negative Z moving toward
zero. This sign convention is a proposed scene default, not a requirement of
the gameplay library. Markers face local +Z, which faces the player with this
layout. Place the stage rows beside the approach lane with their walls outside;
choose row X offsets, runway width, floor height, and arrival height together
so the decks/steps do not obstruct the lane. Keep Start within easy reach.
Final positions and lighting require desktop and headset inspection.

### Venue data

Each venue should specify:

- a stage/course frame and expected forward direction;
- music asset, BPM/beat grid, course length, and any lead-in;
- target cue list with role, due beat, placement, size, scored flag;
- winding pictogram path, fade intervals, rig/pose mapping, and coach model/poses;
- scenery cues and cleanup at course end;
- accessibility and calibration assumptions, including reach and height.

Venue one is the minimal test environment. Venue two should reuse the same
course/judging parts with different art or pictogram path to prove the boundary.
Names, art direction, and tracks are intentionally open for the review.
