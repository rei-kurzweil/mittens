# Task: build the minimal rhythm game prototype in slices

Status: proposed delivery plan. Parent: [VR rhythm game epic](epic/vr-rhythm-game-prototype.md).

## Repository structure

Put the first example at `examples/rhythm_game/minimal.mms`. Put reusable MMS
target visuals and course helpers in `assets/components/rhythm_game/`,
starting with circle and square factories once their contract is agreed.
Keep venue-specific composition in the example. More examples may share the
factories without requiring a new Mittens release during co-development.

## Slices

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
   scoring in another environment. Try a curved guide approach from the
   right; keep it only if it reads clearly in VR.

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
