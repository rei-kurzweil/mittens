# Spec: four beat-authored streams in each rhythm venue

Status: proposed for review. Parent: [VR rhythm game epic](epic/vr-rhythm-game-prototype.md).

## Common course language

A venue owns one beat-indexed course. Its authoring should describe *what is
due when* separately from how early each stream previews it. The same due
beat can cue a player target, a guide pose, a coach pose, and scenery. Each
stream may have its own lead offset, transition duration, and cleanup beat.
The initial authoring vehicle is MMS `Animation`/`Keyframe.at` and small
factories in `assets/components/rhythm_game/`. If direct keyframe callbacks
become awkward for a long course, a later compiler can lower a declarative
course table into those same events; that is a design option, not phase-one
infrastructure.

Use a single world/stage coordinate frame for authored target positions.
Describe left and right from the player's perspective. Distinguish the
player's body tracking origin from the coach's model origin. A venue may move
or rotate the course frame, but all target zones and cue visuals must follow
the same transform. Support standing play first; calibration and room-scale
placement remain explicit follow-up decisions.

| Stream | Authored event at due beat `b` | Preview and completion | First slice |
|---|---|---|---|
| Player targets | Body role, world/stage pose, zone size, whether scored | Spawn/enable at `b - 1`; inner circle or square grows for one beat until it fills the outline at `b`; judge then retire | Head, left hand, right hand circles scored; foot squares visible and unscored |
| Pose guides | Pose and arrival/departure beats | Spawn to the right, advance toward a point slightly left of center, disappear after due beat | Simple posed stick figures on a straight track |
| Coach | Demonstration pose and transition interval | Begin transition early enough to be readable; hold through due beat, then continue | One skinned humanoid at front center with a few distinct poses |
| Venue/atmosphere | Cue and active interval | Art-directed; independent lead and duration | One modest environment with beat-timed background events |

## 1. Player targets

The first target kinds are `head`, `left_hand`, and `right_hand`, each shown as
a circle with an outer outline and inner fill. The inner shape starts small at
the preview beat and reaches the outline at the due beat. A target's placement
and its `Zone.sphere(...)` should derive from the same authored cue. A target
may be simultaneous with others; hit results are per role and per cue.

Author `left_foot` and `right_foot` squares from the start, using box-shaped
zones and the same one-beat fill rule, but set `scored = false` in the first
slice. Later tracked feet can activate scoring without changing the visual
course data. Torso and pelvis are possible later roles; neither requires a
first-slice visual or judge. Define each role's measurement point and zone
size in the course timing task rather than relying on a model bone name.

## 2. Pose guides

Guides are a sequence of posed stick figures or simple mannequins showing
the upcoming body arrangement. Initially they traverse a straight lane from
the player's right toward a point slightly left of center and disappear after
their due beat. Their pose should correspond to the same authored body-target
event, though a guide may preview multiple simultaneous targets. Keep guide
travel time configurable per venue; do not tie it to the target's exact
one-beat fill duration.

A curved path entering from the right is a later presentation slice. Author
the path independently from guide poses, so a different venue can choose a
curve without rewriting choreography. In VR, test the lane from the expected
forward-facing play position and allow head turning; never define timing by
literal screen pixels.

## 3. Coach

The coach is a more detailed skinned humanoid near front center. Its poses
demonstrate what the player should do at target beats. The model, pose
transitions, and cue mapping should be reusable across courses. The first
slice may use a small pose library; it must show that coach transitions are
beat-aligned, rather than simply reacting after scoring. How tracked player
motion and coach animation coexist is outside this prototype's first slice.

## 4. Venue and atmosphere

The fourth stream covers lighting, backgrounds, props, and other non-scoring
events. It uses the same course clock but may react before, on, or after a
beat. A venue is free to omit most events in this stream. They must not block
judgment or alter target-zone placement accidentally.

## Venue template and open choices

Each venue should specify:

- a stage/course frame and expected forward direction;
- music asset, BPM/beat grid, course length, and any lead-in;
- target cue list with role, due beat, placement, size, scored flag;
- guide path/pose mapping and coach model/pose mapping;
- scenery cues and cleanup at course end;
- accessibility and calibration assumptions, including reach and height.

Venue one is the minimal test environment. Venue two should reuse the same
course/judging parts with different art or guide path to prove the boundary.
Names, art direction, and tracks are intentionally open for the review.
