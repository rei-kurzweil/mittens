# Refactor: drive raycasts via actions/signals (remove RayCastMode)

This doc proposes a refactor where raycasting is **only performed when explicitly requested**, rather than being controlled by a `mode` on each raycaster.

In other words:
- remove the concept of a raycaster being “continuous” vs “event driven”
- raycasters produce `RayIntersected` only when something requests an intersection test
- requests come from actions/signals (not directly from `InputState`)

This is a key step toward:
- arbitrary raycasters (XR controllers, AI, tools) that don’t depend on mouse input
- predictable timing (when does a request turn into a `RayIntersected`?)
- future generalization beyond “ray” (hands/contact could become another kind of hit source)

## Current state (what exists today)

### RayCastComponent + RayCastMode

`RayCastComponent` currently contains:
- `mode: RayCastMode` (`Continuous` or `EventDriven`)
- `cast_requests: u32` (incremented by `ActionMethod::Raycast`)

And the semantics are documented in the component itself as:
- “cast from the active window camera through the cursor”
- in event-driven mode: cast only when mouse is pressed / dragging

In code, `RayCastSystem::should_cast(...)` currently depends on `InputState` (mouse edges / dragging) even for non-cursor ray sources.

### Action-driven raycast requests

There is already an action method:
- `ActionMethod::Raycast`
- helper: `Action::raycast(raycaster_id)`

In `ActionSystem`, `ActionMethod::Raycast` increments `RayCastComponent.cast_requests` on the addressed raycaster(s).

Then, inside `RayCastSystem::tick_with_queue`, `cast_requests > 0` is treated as an additional reason to cast this frame.

### Frame timing (important)

The system tick order in `SystemWorld::tick(...)` (simplified) is:

1. `input.process_input(...)`
2. `animation.tick_with_beat(...)` (executes actions via `ActionSystem`)
3. transforms / bvh / collision / camera / openxr
4. `raycast.tick_with_queue(...)`
5. `gesture.tick_with_rx(...)`
6. `gizmo.tick_with_queue(...)`

So:
- actions triggered by animation/keyframes happen **before** raycasting this frame
- ray hits (`RayIntersected`) are emitted **before** gestures interpret them

This is good: it means action-triggered raycasts can naturally feed gestures/gizmos in the same frame.

## Proposed refactor

### 1) Remove raycaster “modes”

Remove `RayCastMode` and the `mode` field from `RayCastComponent`.

Raycasters should be *passive* descriptors:
- where does the ray come from? (cursor camera vs parent forward vs XR aim, etc.)
- how far does it cast? (`max_distance`)

The decision “should we cast this frame?” should be driven entirely by requests.

### 2) Make requests first-class (actions/signals)

We want raycasts to happen only when requested.

There are two good representations:

**Option A: keep ActionMethod-based requests**
- keep something like `ActionMethod::Raycast` (or rename to `ActionMethod::RayIntersectRequest`)
- the action targets contain the raycaster ComponentId(s)

**Option B: introduce a dedicated Rx signal**
- emit `EventSignal::RaycastRequested { raycaster }` into `RxWorld`
- `RayCastSystem` consumes requests at the raycast stage and emits `RayIntersected`

Either way:
- `RayCastSystem` no longer reads mouse buttons directly to decide if a cast should occur
- user input becomes *one producer* of requests, not a hidden requirement

### 3) Stop using mouse edges inside RayCastSystem

With this refactor:
- `RayCastSystem::should_cast(...)` disappears
- `RayCastSystem::tick_with_queue(...)` casts only for the requested raycaster IDs this frame

This is the core “no special input device assumptions” change.

### 4) How “continuous hover” works after this change

Today, “continuous” mode implies casting every frame.

After this change, if you want hover/picking every frame you do it by **requesting every frame**.

Examples:
- desktop editor: UserInput layer requests raycasts every frame while the cursor is in-window, or while a drag is active
- XR laser pointer: OpenXR layer requests raycasts every frame while laser is enabled
- AI tool: requests raycasts during its own update loop

This keeps behavior identical, but makes the *policy* live in the producer (user input / tool system) instead of inside raycast.

## Timing implications (the point of the refactor)

Because raycasting happens at a specific stage in the frame, requests must arrive before that stage to produce a `RayIntersected` in the same frame.

### Same-frame vs next-frame behavior

- If a request is created during:
  - `input.process_input(...)` (early), or
  - `animation.tick_with_beat(...)` (early)

  then `RayCastSystem` will see it and emit `RayIntersected` this frame.

- If a request is created by an event handler that runs after `raycast.tick_with_queue(...)` (e.g. in the post-mutation dispatch phase), it will not be visible until the **next** frame.

This is already true today for `cast_requests`: a late increment won’t affect the already-finished raycast stage.

### Recommendation: define the request phase

To make timing predictable, standardize:
- all systems that want ray hits must publish requests **before** the raycast stage
- or, if we need to support “immediate” requests from later systems, introduce a separate late-phase raycast pass (usually not desirable)

In practice, the simplest rule is:
- treat raycast requests as *inputs*, similar to how input devices are sampled, not as an effect of arbitrary late-stage event handlers

## API and naming

The user-facing intent is closer to “intersect” than “raycast”:
- a request asks the engine to compute intersections and publish `RayIntersected`

So, renaming is worth considering:
- `ActionMethod::Raycast` → `ActionMethod::RayIntersect` or `RayIntersectRequest`
- keep the serialized name `"raycast"` for backward compatibility if needed

The doc uses “request” terminology to emphasize that this is not an immediate synchronous query.

## Consequences

### Pros

- Fully decouples raycasting from mouse input
- Makes non-user raycasters first-class (AI, tools, XR)
- Makes “when does this happen?” explicit
- Lets different systems choose their own sampling rate (every frame, only while active, on-demand)

### Cons / tradeoffs

- Hover behavior becomes an explicit policy (must request each tick)
- Request plumbing becomes more important (must ensure the right requests happen at the right time)
- If everything requests every frame, you’ve recreated “continuous” — but now it’s explicit and controllable

## Migration plan (incremental)

1. Add a request signal type (or formalize action requests) and route the mouse path through it.
2. Change `RayCastSystem` to cast only when requested (keep existing hit emission semantics).
3. Remove `RayCastMode` + all `InputState` checks from `RayCastSystem`.
4. Update docs and examples.

## Follow-ups (gesture implications)

This raycast refactor pairs naturally with the gesture refactor:
- gesture lifecycle should be driven by pointer/button signals, not mouse state inside `GestureSystem`
- `DragUpdatePolicy` should be per pointer/raycaster
- `StartPlaneProjection` should use the pointer’s ray source (not always cursor)

Those are tracked in `docs/gestures-and-gizmos.md`.
