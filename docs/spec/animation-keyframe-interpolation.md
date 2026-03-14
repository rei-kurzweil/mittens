# Animation keyframe interpolation

## Goal

Add a first-class way for animations to produce **continuous transitions** between values rather than only firing discrete intents at keyframe times.

Current behavior is intentionally simple:

- `KeyframeComponent` only stores a `beat`.
- `AnimationSystem` decides when a keyframe becomes due.
- child `ActionComponent`s are fired once when that keyframe becomes active.
- the stored payload is usually a discrete intent such as `UpdateTransform`, `SetColor`, or `SetText`.

That model works for stepping between states, but it cannot express:

- “move from A to B over 0.5 beats”
- “ease in/out between rotations”
- “fade color over 1 second”
- “blend toward this pose until the next keyframe”

This doc proposes a spec that fits the current engine architecture without forcing keyframes themselves to become heavy.

## Current engine model

Today the animation stack is structurally:

- `AnimationComponent`: playback state (`Playing`, `Paused`, `Looping`)
- `KeyframeComponent`: a time marker (`beat`)
- `ActionComponent`: a stored `IntentValue`
- `AnimationSystem`: finds due keyframes and emits the child actions into `RxWorld`

Important current properties:

1. **Keyframes are stateless markers**
   - They do not know previous/next keyframes.
   - They do not know what property they affect.
   - They do not encode interpolation shape.

2. **Actions are semantic payloads**
   - An action is more than a raw mutation.
   - It already represents “what should happen when this timeline point is hit”.

3. **The signal pipeline already distinguishes intent layers**
   - Some intents are high-level/composed.
   - Some are low-level mutations like `UpdateTransform`.

That suggests interpolation should not be shoved into `KeyframeComponent` itself.

## Design constraints

A good interpolation design should:

- preserve `KeyframeComponent` as a simple timing marker
- reuse `ActionComponent` as the semantic unit of animation work
- avoid exploding the general signal system with unnecessary per-frame fan-out when a local runtime loop would do
- still use normal intents/mutations for actual property application so behavior stays consistent with non-animation paths
- support looping/restart semantics cleanly
- support transform interpolation correctly:
  - translation: lerp
  - scale: lerp
  - rotation: slerp / normalized quaternion blend

## Options considered

### Option A: put easing/interpolation fields on `KeyframeComponent`

Example concept:

- `KeyframeComponent { beat, interpolation, duration_beats }`

Why this is attractive:

- simple timeline-centric authoring
- resembles DCC tool keyframes

Why this is a poor fit right now:

- keyframes still do not know **what** they are interpolating
- one keyframe may contain multiple actions affecting unrelated properties
- different actions under the same keyframe may want different durations/easing
- it pushes semantic meaning into a component that is currently only a scheduler marker

Conclusion: not recommended for the current engine.

### Option B: infer interpolation from neighboring keyframes with matching action type

Example concept:

- two transform actions on adjacent keyframes implicitly define a segment
- the engine scans neighbors and interpolates between them

Why this is attractive:

- matches traditional animation tracks
- no extra component needed for simple cases

Why this is risky in the current architecture:

- requires pairing logic between actions across separate keyframes
- needs a stable notion of “same animated channel/property” that does not exist yet
- becomes fragile when one keyframe contains multiple actions or heterogeneous targets
- adds a lot of hidden magic

Conclusion: good long-term track-based direction, but too implicit for the current ECS topology.

### Option C: attach `TransitionComponent` under `ActionComponent`

Example topology:

- `AnimationComponent`
  - `KeyframeComponent { beat: 0.0 }`
    - `ActionComponent { signal: UpdateTransform { ...target value... } }`
      - `TransitionComponent { duration_beats: 0.5, easing: EaseInOutCubic }`

Why this fits the current engine well:

- `KeyframeComponent` remains only about time
- `ActionComponent` remains the semantic “thing to do”
- interpolation metadata lives exactly on the action it modifies
- different actions at the same keyframe can use different easing/durations
- the animation system can still trigger one semantic unit at the keyframe boundary

Conclusion: **recommended**.

## Recommended model

### Core idea

A `TransitionComponent` attaches to an `ActionComponent` and changes the meaning of that action from:

- **discrete fire-once mutation**

to:

- **start a time-bounded transition toward the action’s target value**

The keyframe still only says **when** the action starts.
The action still says **what target value / semantic effect** is desired.
The transition says **how the action unfolds over time**.

## Proposed topology

Recommended authoring shape:

- `AnimationComponent`
  - `KeyframeComponent`
    - `ActionComponent`
      - `TransitionComponent`

`TransitionComponent` is optional.

Behavior:

- no `TransitionComponent` → current behavior stays discrete
- with `TransitionComponent` → the action starts a transition runtime instance

## Proposed `TransitionComponent` fields

Initial minimal shape:

- `enabled: bool` (default `true`)
- `duration_beats: f64`
- `easing: TransitionEasing`
- `capture_from_current: bool` (default `true`)
- `replace: TransitionReplacePolicy` (default `ReplaceSameTarget`)

### `TransitionEasing`

Initial built-ins:

- `step`
- `linear`
- `ease_in_quad`
- `ease_out_quad`
- `ease_in_out_quad`
- `ease_in_cubic`
- `ease_out_cubic`
- `ease_in_out_cubic`
- `ease_in_out_sine`

Notes:

- `step` is mainly useful for explicit hold/compat behavior.
- We do not need custom curves in v1.

### `capture_from_current`

If `true`, when the keyframe fires the transition starts from the property’s **current live value**.

That is the right default for this engine because:

- gameplay, input, XR, and animation may all have touched the same value before the keyframe
- it avoids needing the previous keyframe to define the start value
- it works naturally for interrupted / restarted animations

If `false`, future versions may allow an explicit `from` payload, but that is not needed for v1.

### `replace`

Initial replacement policy options:

- `ReplaceSameTarget`: a new transition on the same target/property replaces the old one
- `AllowParallel`: multiple transitions may run concurrently if they affect distinct channels

For v1, `ReplaceSameTarget` should be the only implemented runtime behavior, even if the enum leaves room for expansion later.

## Runtime behavior

### New execution phase: transition runtime

Add a `TransitionSystem` (or animation-owned transition runtime) that manages active interpolations over time.

Responsibilities:

- receive “start transition” requests when a keyframed action fires
- snapshot the current source value
- store end value + easing + duration + start beat/time
- each tick, evaluate active transitions
- emit/apply the corresponding low-level mutation
- complete and remove finished transitions

This is intentionally different from sending a huge burst of micro-intents up front.

## Concrete execution model: micro-intent fan-out

The intended semantics are absolutely micro-step-like.

Conceptually, this:

- `KeyframeComponent`
  - `ActionComponent { signal: UpdateTransform { ...target... } }`
    - `TransitionComponent { easing: linear, duration_beats: 1.0 }`

means:

- at the keyframe boundary, start a transition
- during the transition, produce a stream of small `UpdateTransform` intents
- each small intent moves the target closer to the destination

So the conceptual expansion is:

```text
keyframe {
  action {
    intent = update_transform(target = translate(30, 0, 0))
    transition { linear(), duration_beats = 1.0 }
  }
}

=> [
  scheduled sample 0  -> update_transform(translate(...small step...))
  scheduled sample 1  -> update_transform(translate(...small step...))
  scheduled sample 2  -> update_transform(translate(...small step...))
  ...
  scheduled sample N  -> update_transform(translate(30, 0, 0))
]
```

That is the right mental model.

The main design question is: **what are those samples scheduled against?**

## Frame-based fan-out

The most literal interpretation is:

- `frame + 0`
- `frame + 1`
- `frame + 2`
- ...

This works if we decide that transitions are fundamentally display-sampled animation.

To do that, the engine needs an assumed sample rate:

$$
  ext{duration\_sec} = \text{duration\_beats} \cdot \frac{60}{\text{bpm}}
$$

$$
  ext{sample\_count} \approx \text{duration\_sec} \cdot \text{fps}
$$

Example:

- `duration_beats = 1.0`
- `bpm = 120`
- so duration is $0.5$ seconds

Then:

- at `60 Hz` → about `30` micro-intents
- at `90 Hz` → about `45` micro-intents
- at `120 Hz` → about `60` micro-intents

If the motion is a `30`-unit translation and we happened to choose `60` samples, then each micro-intent would move by about `0.5` units, which matches the intuition in your sketch.

So “1 beat at 120 bpm” is not intrinsically “60 micro-intents”.
It is only 60 samples if the sampling rate is 120 Hz.

## Problem: the engine does not really know the future frame rate

If we eagerly fan out into `frame + N` items at keyframe fire time, we are baking in an assumption about future frame cadence.

That is awkward because real runtime cadence may change due to:

- desktop frame drops
- VR reprojection / throttling
- window vs XR running at different rates
- pause/resume or frame hitches
- future variable-rate simulation/render loops

So a precomputed `frame + N` queue is only exact if the frame rate stays exactly what we assumed.

## Two ways to handle that

### Option 1: accept approximate frame-based fan-out

We can define transitions as:

- choose an FPS when the transition starts
- precompute `N` micro-intents
- schedule them for `frame + 0 ... frame + N-1`

Pros:

- easy to reason about
- feels close to your sketch
- makes micro-intents fully explicit as queued work

Cons:

- frame hitches distort timing
- if actual FPS changes, beat alignment drifts
- replacing/canceling transitions means editing/removing already-scheduled entries
- VR vs desktop cadence differences become authoring-visible in a bad way

This is viable, but it makes the animation system more frame-rate dependent than the rest of the beat-driven design.

### Option 2: schedule samples in beat/time space, emit when due

Instead of precommitting to `frame + N`, define the fan-out as a sequence of samples with target progress in beat/time domain.

Conceptually:

- start transition at beat $b_0$
- end transition at beat $b_1$
- on each frame, determine which sample(s) are due at the current beat/time
- emit the corresponding `UpdateTransform` intent(s) then

This keeps the micro-intent concept, but avoids pretending we know future frames.

In other words:

- the fan-out is **real conceptually**
- but it is **materialized lazily** as frames happen

That means the queue contains either:

- one compact active transition record, or
- an internal list of due sample indices / due beats

rather than 30–60 fully expanded `UpdateTransform` intents created all at once.

## Recommended execution semantics

Recommended model:

- treat transitions as generating a stream of micro-intents
- do **not** prebuild the whole stream at keyframe fire time
- instead, store enough information to derive the next micro-intent(s) on demand

So the runtime behavior becomes:

1. Keyframe fires.
2. `AnimationSystem` emits `StartTransitionFromAction`.
3. `TransitionSystem` snapshots:
   - start value
   - end value
   - start beat
   - end beat
   - easing
   - target channel
4. Each frame, `TransitionSystem` computes the current normalized progress.
5. It emits the one `UpdateTransform` / `SetColor` / `SetOpacity` intent appropriate for that frame.
6. On the final sample, it emits the exact destination value and retires the transition.

This is still fan-out.
It is just **incremental fan-out**, not **eager fan-out**.

## If we want explicit scheduled micro-intents as an engine concept

If the engine later wants this concept to be first-class, the better abstraction is not `frame + N`.

It is something like:

- `ScheduledIntent { due_beat, signal }`
- or `ScheduledIntent { due_time_sec, signal }`
- or an internal runtime record `PendingTransitionSample { sample_index, due_progress }`

Then per-frame processing can say:

- “what scheduled samples are due now?”

rather than:

- “we promised this would happen on frame 417, hope we actually reached that frame on time”

That keeps the execution aligned with beat-driven animation rather than display-frame prediction.

## Beat sampling vs continuous sampling

There are really two sub-models here.

### Continuous per-frame sampling

Each frame:

- compute progress directly from current beat/time
- emit exactly one micro-intent for the current state

This gives smooth results and automatically adapts to the actual frame rate.

### Quantized sample fan-out

Each transition chooses a fixed sample count, such as:

- `duration_sec * 60`
- or `duration_sec * display_refresh_guess`

and emits one sample per discrete step.

This is closer to offline baked keys.

I do **not** recommend this for v1 because it adds quantization policy and frame-rate guessing immediately.

## Recommendation for the spec

So the spec should define transitions as:

- semantically equivalent to a fan-out of micro-intents over the transition interval
- operationally implemented as per-frame sampling from a compact runtime transition record

That preserves the mental model you want without coupling animation correctness to a guessed future FPS.

## Why not fan out all micro-intents immediately?

A keyframed transition over 1 second at 90 Hz could mean ~90 micro-updates.
Generating them all at keyframe fire time has downsides:

- unnecessary queue pressure
- awkward cancellation/replacement behavior
- difficult interaction with variable frame rate
- wasted work if the animation is paused/stopped/restarted early

Instead, the runtime should store a compact active transition record and evaluate it incrementally each frame.

So the design is:

- **one start intent** at the keyframe boundary
- **one active transition record** in runtime state
- **one per-frame mutation** while active

That gives the ergonomic result of micro-intents without front-loading them.

## Intent shape

Introduce a high-level intent such as:

- `StartTransition { action_component, beat_context }`

or more explicit:

- `StartTransitionFromAction { action_component, animation_component, keyframe_component, beat_context }`

Recommended approach:

- `AnimationSystem` emits `StartTransitionFromAction` when it sees an `ActionComponent` with a child `TransitionComponent`
- otherwise it emits the stored action’s `IntentValue` directly, exactly as today

The start-transition intent is high-level. It should be interpreted by the transition runtime, not by the low-level mutation executor.

## Runtime state shape

Conceptual runtime record:

- target component ids
- animated property kind
- source value snapshot
- destination value
- start beat or start time
- duration beats or seconds
- easing function
- ownership/source action id

A transition record must be channel-specific, not just target-specific.

Examples of channels:

- transform translation
- transform rotation
- transform scale
- color rgba
- opacity
- audio parameter value

This is important so a transform translation tween does not automatically stomp an unrelated color tween on the same entity.

## Time domain

The animation system is currently beat-driven, so transition timing should also be beat-native in animation contexts.

Recommended v1 rule:

- `TransitionComponent.duration_beats` is the source-of-truth duration
- each frame, the runtime converts current animation beat into normalized progress

This keeps transitions consistent with tempo changes and loop semantics.

Longer-term, non-animation uses might also want:

- `duration_sec`
- unscaled vs scaled time
- manually driven progress

But for this doc’s scope, beat-driven is enough.

## Property support

### v1 supported payloads

The first implementation should be narrow and explicit.

Recommended v1 support:

- `IntentValue::UpdateTransform`
- `IntentValue::SetColor`
- `IntentValue::SetOpacity`

These are easy to define as interpolable numeric channels.

Recommended v1 exclusions:

- `SetText`
- topology intents (`Attach`, `Detach`, `RemoveSubtree`, ...)
- discrete state toggles
- audio scheduling intents that already use beat-exact scheduling semantics

Rule:

- if an action’s stored intent is not interpolable, `TransitionComponent` is ignored with a warning/log, or treated as invalid authoring.

I recommend **warning + discrete fallback disabled** only in dev logs, not panic.

## Transform interpolation details

For `UpdateTransform`:

- translation: linear interpolation per component
- scale: linear interpolation per component
- rotation: quaternion spherical interpolation (slerp)

Do not lerp quaternions naïvely unless normalized nlerp is an explicitly chosen approximation.

If the transform currently has a non-unit quaternion, normalize before blending.

## Data flow

Recommended playback flow:

1. `AnimationSystem` computes due keyframes as today.
2. For each child `ActionComponent`:
   - if no `TransitionComponent` child exists: emit stored intent immediately
   - if a `TransitionComponent` child exists: emit `StartTransitionFromAction`
3. `TransitionSystem` resolves the action payload into an interpolable runtime record.
4. Each frame, `TransitionSystem` evaluates progress $t \in [0,1]$.
5. It applies easing $e = f(t)$.
6. It computes the interpolated value.
7. It emits or applies the standard low-level mutation (`UpdateTransform`, `SetColor`, etc.).
8. At $t = 1$, it applies the exact destination value and removes the runtime record.

## Emit intents each frame vs direct mutation

There are two valid implementation strategies.

### Strategy 1: emit normal intents each frame

Example:

- transition runtime evaluates new transform
- emits `IntentValue::UpdateTransform`
- existing mutation executor performs the write and downstream refresh

Pros:

- reuses existing mutation path
- keeps behavior consistent with other callers
- easier to reason about

Cons:

- still incurs one intent per active transition per frame

### Strategy 2: mutate directly inside `TransitionSystem`

Example:

- transition runtime writes the transform component directly
- then explicitly calls the same downstream refresh hooks the normal mutation path would call

Pros:

- lower per-frame signal overhead

Cons:

- duplicates mutation semantics
- easier to drift from the canonical update path

## Recommendation

For v1, prefer **Strategy 1**.

Reasoning:

- the engine already treats intents as the canonical mutation route
- transition counts are likely modest at first
- correctness and consistency matter more than shaving a small amount of queue overhead

If profiling later shows this path is hot, optimize by moving only the hottest channels to a direct internal fast path while preserving identical semantics.

## Interaction with looping and restart

When an animation loops:

- keyframes may start the same transition again on the next cycle
- existing transitions from the prior cycle targeting the same channel should be replaced

When an animation restarts or changes state:

- transitions owned by that animation should be cancelable/resettable

Recommended ownership rule:

- each active transition record stores its source `animation_id` and `action_id`
- restarting an animation clears transitions owned by that animation

## Interaction with other systems

Because transitions default to `capture_from_current = true`, they compose reasonably with:

- input-driven transform changes
- XR/controller-driven transform changes
- gameplay scripts
- editor gizmos

But there is still a real conflict question: what if two systems write the same property in the same frame?

Recommended rule for now:

- the normal frame ordering decides the winner
- within transition runtime, replacement is deterministic by target/channel/action

Long-term, this could evolve into explicit write domains or blend layers, but that is outside v1.

## Authoring examples

### Discrete keyframe (current behavior)

- `KeyframeComponent { beat: 4.0 }`
  - `ActionComponent { signal: UpdateTransform { ... } }`

At beat 4.0, the transform snaps.

### Interpolated keyframe

- `KeyframeComponent { beat: 4.0 }`
  - `ActionComponent { signal: UpdateTransform { ...target pose... } }`
    - `TransitionComponent { duration_beats: 0.5, easing: ease_in_out_cubic }`

At beat 4.0, the system captures the current transform and animates toward the target over 0.5 beats.

## Why `TransitionComponent` belongs under `ActionComponent`

This is the main design choice.

Reasons:

- an action already represents semantic intent, not just raw data
- interpolation belongs to how that intent is executed
- different actions under the same keyframe can transition differently
- it avoids requiring keyframe-to-keyframe pairing logic
- it keeps the timeline node simple and generic

So yes: the recommended design is that `TransitionComponent` attaches to `ActionComponent`, not to `KeyframeComponent`.

## v1 implementation plan

1. Add `TransitionComponent` with:
   - `enabled`
   - `duration_beats`
   - `easing`
   - `capture_from_current`
2. Add a high-level start-transition intent.
3. Teach `AnimationSystem` to emit that intent when an action has a transition child.
4. Add `TransitionSystem` runtime storage for active transitions.
5. Support interpolating:
   - `UpdateTransform`
   - `SetColor`
   - `SetOpacity`
6. Apply results through the normal intent path each frame.
7. Add dev logging for unsupported transitioned action types.

## Non-goals for v1

- implicit neighbor-keyframe track solving
- arbitrary custom curves
- topology interpolation
- text interpolation semantics
- full animation-layer blending
- backward-compatibility aliases or multiple schema shapes

## Open questions

- Should the runtime live as a standalone `TransitionSystem` or inside `AnimationSystem`?
  - I prefer a standalone runtime once interpolation exists outside animation too.
- Should unsupported transitioned actions be ignored, logged, or hard-error?
  - I prefer logged and skipped.
- Should duration allow `0.0`?
  - Yes; treat as immediate completion.
- Should we add explicit `from` values later?
  - Probably, but only after `capture_from_current` proves insufficient.

## Recommendation summary

Recommended path:

- keep `KeyframeComponent` simple
- attach `TransitionComponent` to `ActionComponent`
- start transitions at keyframe fire time
- store compact runtime transition records
- apply normal low-level intents per frame while active

That matches the current engine shape better than making keyframes smart or trying to infer implicit tracks from neighboring actions.
