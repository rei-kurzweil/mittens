# Task: emotes, emote panel and emote wheel

## Status

Proposed, not implemented. Depends on
[transitionable avatar pose layers](avatar-pose-transition-layers.md) for
pose emotes and on morph API gaps noted below. Gesture mappings depend on
[hand gesture recognition](hand-gesture-recognition.md).

## Outcome

Emotes are named, reusable expressions an avatar can perform: morph weights,
a pose blended into any bones, or an animation clip, alone or combined. A
quick emote wheel triggers them in VR; an emote panel in the dock browses
them and assigns them to wheel slots, buttons and hand gestures.

## Model

The emote's name is its identity, not a morph target index.

```mms
// Definition: what the emote does. Shared, browsable.
Emote {
    name
    color                   // default colour wherever the emote is shown
    ?morph                  // { "<morph label or canonical channel>": weight, ... }
    ?pose                   // a PoseCapturePose, blended in
    ?clip                   // an Animation over bones and/or morphs
    ?blend_in, ?hold, ?blend_out   // seconds; hold = null means "while held"
    ?override_tracking      // default false: tracked head/eyes/hands win
}

// Instance: how this player/avatar triggers it.
EmoteInstance {
    emote                   // reference to an Emote definition
    ?wheel_index            // 0–5, or none
    mappings                // [ button combo | hand gesture name, ... ]
}
```

- A definition can have several instances; editing it changes every place
  it appears.
- `wheel_index` is optional: an emote can be gesture-only.
- Instances are per avatar and saved with the avatar's settings (next to the
  Humanoid calibration), so avatars can map the same input differently.
- Conflicts are rejected in the panel with a clear message, never resolved
  silently: two instances on one wheel index, or a mapping that reuses a
  bound input (grip grab, left menu button home, …).

## Emote playback

- **Morphs**: per-emote target weights, blended in and out. Needs two
  morph API changes (`src/scripting/morph_api.rs`):
  1. listing a model's morph targets (`model.morph_targets()`: label,
     node/primitive/target index, current weight), grouped by label since
     one label usually spans several primitives;
  2. driving any labelled target, not only the 17 canonical blink/viseme
     channels `MorphTargetMap` accepts today.
  Emote weights must compose with AVC's blink and viseme drivers; define the
  order (for example: emote weight is the floor, trackers add on top, or the
  emote suppresses visemes while active).
- **Poses**: blend a `PoseCapturePose` in over `blend_in`, hold, blend out.
  This is the pose layer from the transition-layers ticket; build on it
  instead of a separate mechanism. Any bones, including tails, ears and
  props.
- **Clips**: play an `Animation` once or while held.
- **Tracking precedence**: AVC's tracked head, eyes and hands win over
  emote-driven bones unless the emote sets `override_tracking`.
- **Networking**: replicate the active emote by name and start time, not
  per-frame bone or morph values.

## Emote wheel (VR)

- Six slots as partial annuli around the wheel centre, rainbow by default.
  The slot colour belongs to the wheel; an emote's colour can override it.
  Each slot shows the emote's name; the selected slot is highlighted.
- Input: the right stick's X axis already turns the rig (`InputXrGamepad`
  turn mode), so stick movement alone must not open the wheel.
  1. Hold a button (right stick click or B; to be decided) to open the wheel
     at the hand. Turning is suspended while it is open.
  2. Point the stick toward a slot.
  3. Trigger or grip fires the selected emote; releasing the open button
     without firing cancels.
- Desktop: a key opens the same wheel with mouse selection (to be decided).

## Emote panel

- A dock item after Mirror and Avatar, in that group.
- A scrolling list of emote definitions (built-in and custom), each showing
  name, colour and what it drives (morph / pose / clip).
- Select an emote, then a wheel slot, to assign it; assigning to a taken slot
  moves the previous instance off the wheel.
- Edit an instance's mappings: add a button combo, or pick or record a hand
  gesture (recording lives in the gesture work).
- Preview: perform the emote on the avatar (visible in the mirror).

## Slices

1. Morph listing and labelled morph control (API changes above).
2. `Emote` / `EmoteInstance` components and playback for morph emotes, with
   button-combo mappings.
3. Pose emotes on the pose-layer work; clip emotes.
4. Emote wheel (VR) with rainbow defaults and the open/select/fire input.
5. Emote panel in the dock: browse, assign to slots, edit mappings, preview.
6. Gesture mappings once hand gesture recognition exists.
7. Network replication of active emotes.

## Validation

- Headless tests: playback blends in, holds and blends out; conflicts are
  rejected; instances save and load per avatar; morph listing groups
  multi-primitive labels; emote weights compose with blinks and visemes in
  the chosen order.
- Wheel input tests: stick movement alone does not open the wheel; turning is
  suspended while open; cancel fires nothing.
- Headset: wheel readable at arm's length on the Focus 3 and desktop XR; an
  emote with a pose does not fight tracked hands unless it overrides them.
