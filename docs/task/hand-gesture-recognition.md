# Task: hand gesture recognition

## Status

Proposed, not implemented. Hand joints are already tracked and drive avatar
fingers (`HandTracking`), but nothing classifies them into named gestures.
Consumer: [emotes and the emote wheel](emotes-and-emote-wheel.md), whose
emote instances map gestures to emotes.

## Outcome

Named hand gestures (thumbs-up, peace, wave, …) that fire `GestureStarted` /
`GestureEnded` events MMS can react to. Gestures are defined by recorded
examples, built-in and user-recorded alike, and improve from the user's
corrections. Learned models are small, run on the CPU on desktop and the
Focus 3, and are added only when simpler matching falls short.

## Existing pieces

- `HandTrackingFrame` (`src/engine/ecs/system/hand_tracking.rs`): one
  canonical 26-joint layout for both providers (OpenXR hand tracking,
  MediaPipe), with a canonical hand frame (-Z forward, +Y little-to-index
  width on both hands). Features computed from it work the same for either
  source.
- Paw capture (`src/engine/ecs/system/ik/body/paw_capture.rs`) is a precedent
  for recording labelled snapshots on demand from a HUD button.
- The Android host does not read hand tracking yet.

## Design

### 1. Hand features

Computed once per hand per frame from `HandTrackingFrame`, in the canonical
hand frame and normalised by hand size so examples transfer across hands,
avatars and providers:

- curl per finger (0 straight, 1 fist), from joint angles along each chain;
- thumb-to-fingertip pinch distances;
- finger spread (angles between adjacent fingers);
- palm and thumb direction relative to the head (toward, away, up, down,
  sideways).

Roughly 15–20 values. Missing or low-confidence joints mark the frame
unusable instead of producing a guess.

### 2. Gesture definitions

```mms
HandGesture {
    name = "thumbs_up"
    ?hand = "either"        // "left" | "right" | "either"
    ?hold = 0.25            // seconds the match must persist
    examples = "gestures/thumbs_up.json"
}
```

A gesture is a set of recorded feature examples, not hand-tuned thresholds,
so built-ins and user gestures behave the same. Ship built-ins as recorded
examples: fist, open palm, point, pinch, thumbs-up, peace, finger guns.

### 3. Static recognition: nearest examples (first)

- Each frame, find the nearest stored example per gesture (distance in
  feature space), with a per-gesture cutoff. No match within the cutoff
  means "none": most frames are not gestures.
- Stability: separate enter and exit cutoffs (hysteresis), plus the
  gesture's hold time before it fires.
- Negative examples ("this is nothing") sit beside positives and win when
  nearer, which stops ordinary hand shapes from matching.
- No training step: a recorded example takes effect on the next frame.

### 4. Events and suppression

- `GestureStarted { gesture, hand }` / `GestureEnded`, emitted on the
  tracked hand's owner.
- Suppress recognition while that hand's pointer is targeting UI (pinch is
  also a click) and while it is grabbing.

### 5. Recording and corrections

A gesture panel (later reachable from the emote panel):

- **Record**: hold the pose for ~2 s to add examples; thin near-duplicate
  frames so one hold does not add 30 identical examples.
- **Correct**: "that wasn't <gesture>" saves the triggering frames as
  negatives; "this should be <gesture>" while holding saves positives.
- **Implicit signals**: an emote cancelled within a fraction of a second, or
  repeated near-misses, are logged for review, never added automatically.
- User examples are stored separately from built-ins and can be viewed and
  reset per gesture, so bad corrections cannot silently ruin a built-in.

### 6. Moving gestures

Wave, beckon, salute and heart hands need a window of frames (~1 s).

- First: dynamic time warping against recorded feature sequences. Works with
  a handful of examples and needs no training.
- Then, if accuracy requires it: a small 1D temporal convolution or GRU over
  the feature window, trained offline in Python on collected recordings and
  shipped as a small weights file evaluated on the CPU. No ML runtime on the
  headset.

### 7. Learned static model (only if needed)

A small MLP (a few thousand weights) on the same features, outputting one
probability per gesture plus "none", trained offline on the recorded and
corrected examples. Adopt it only if it beats nearest-example matching on
the fixed evaluation set below.

Graph-convolutional skeleton models (ST-GCN) and transformers need far more
labelled data than this collects and are out of scope.

### 8. Controller hands (later)

Controllers that report finger touch or curl can feed the same finger
features, so some gestures work while holding a controller. Which gestures
are possible depends on the controller.

## Slices

1. Hand features from `HandTrackingFrame`, with a debug overlay showing the
   values per hand.
2. Nearest-example static recognition, gesture definitions, built-in
   examples, events and suppression.
3. Recording and correction panel; separate user example storage.
4. Android: read OpenXR hand tracking in the host and supply
   `HandTrackingFrame` (Focus 3 support to be confirmed).
5. Moving gestures with dynamic time warping.
6. Learned models (MLP, temporal conv/GRU), each only when the evaluation
   set shows the simpler method falling short.

## Validation

- A fixed evaluation set of recorded sessions, labelled, that is never used
  as examples: report per-gesture recall, false triggers per minute on
  "nothing" sessions (walking, grabbing, pointing at UI, talking with hands),
  and latency from pose to event.
- False triggers matter more than missed gestures; set cutoffs from the
  "nothing" sessions.
- Re-run the evaluation set after changing features, cutoffs or models.
- Unit tests: features from synthetic joint layouts (fist, flat hand) on both
  hands and both providers' frame conventions.
