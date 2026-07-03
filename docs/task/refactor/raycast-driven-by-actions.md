# Refactor: drive raycasts via actions/signals

This note now tracks the **remaining** work after the recent pointer/raycast cleanup.

Completed already:
- non-desktop event-driven pointers no longer auto-cast from desktop mouse input
- desktop cursor-through-camera pointers remain the only automatic mouse-driven path
- the interaction flow is now documented in `docs/spec/pointer-input-ray-gesture.md`
- the broader raycast behavior is documented in `docs/spec/bvh-and-raycast.md`

What remains open is the deeper architectural cleanup: make raycasts fully request-driven and remove `RayCastMode` entirely.

## Current state

### RayCastComponent still has mode-based policy

`RayCastComponent` still contains:
- `mode: RayCastMode` (`Continuous` or `EventDriven`)
- `cast_requests: u32` (incremented by a raycast-request intent)

After the recent fix, the runtime behavior is narrower than that old description:
- desktop cursor-through-camera pointers may auto-cast from mouse input
- non-desktop event-driven pointers only cast when `cast_requests > 0`

That means `mode` still mixes two concerns:
- source description (`cursor` vs `parent forward`)
- trigger policy (continuous vs requested)

### Intent-driven requests already exist

There is already an intent payload for this:
- `IntentValue::RequestRaycast { component_ids }`
- carried by `IntentSignal`

In the current intent execution path, `IntentValue::RequestRaycast` increments `RayCastComponent.cast_requests` on the addressed raycaster(s).

Then, inside `RayCastSystem::tick_with_queue`, `cast_requests > 0` is treated as one reason to cast this frame.

### Frame timing (important)

The system tick order in `SystemWorld::tick(...)` (simplified) is:

1. `input.process_input(...)`
2. `animation.tick_with_beat(...)` (emits and executes high-level intents before raycast)
3. transforms / bvh / collision / camera / openxr
4. `raycast.tick_with_queue(...)`
5. `gesture.tick_with_rx(...)`
6. `gizmo.tick_with_queue(...)`

So:
- intents triggered by animation/keyframes happen **before** raycasting this frame
- ray hits (`RayIntersected`) are emitted **before** gestures interpret them

This is good: it means intent-triggered raycasts can naturally feed gestures/gizmos in the same frame.

## Remaining refactor goal

### 1) Remove raycaster-local trigger policy

Remove `RayCastMode` and the `mode` field from `RayCastComponent`.

Raycasters should be *passive* descriptors:
- where does the ray come from? (cursor camera vs parent forward vs XR aim, etc.)
- how far does it cast? (`max_distance`)

The decision “should we cast this frame?” should be driven entirely by requests produced elsewhere.

### 2) Make requests first-class

We want raycasts to happen only when requested.

There are two good representations:

**Option A: keep `IntentValue::RequestRaycast` as the public request form**
- emit `IntentSignal::now(IntentValue::RequestRaycast { component_ids: ... })`
- the intent targets contain the raycaster ComponentId(s)

**Option B: introduce a more specific request intent shape**
- keep it in the intent lane, but rename/split it into something narrower like `IntentValue::RayIntersectRequest`
- `RayCastSystem` consumes those requests at the raycast stage and emits `EventSignal::RayIntersected`

Either way:
- `RayCastSystem` no longer reads mouse buttons directly to decide if a cast should occur
- desktop input becomes one producer of requests, not a special case inside raycast

Important distinction:
- `IntentSignal` / `IntentValue::RequestRaycast` = **request to perform intersection work**
- `EventSignal::RayIntersected` = **fact describing an intersection result**

We should not model both of those as the same kind of signal. Requests belong in the intent lane; hit facts belong in the event lane.

### 3) Stop using mouse edges inside `RayCastSystem`

With this refactor:
- `RayCastSystem::should_cast(...)` disappears
- `RayCastSystem::tick_with_queue(...)` casts only for the requested raycaster IDs this frame

This is the core “no special input device assumptions” change.

### 4) Replace “continuous” with explicit repeated requests

Today, “continuous” mode implies casting every frame.

After this change, if you want hover/picking every frame you do it by **requesting every frame**.

Examples:
- desktop editor: UserInput layer requests raycasts every frame while the cursor is in-window, or while a drag is active
- XR laser pointer: OpenXR layer requests raycasts every frame while laser is enabled
- AI tool: requests raycasts during its own update loop

This keeps behavior identical, but makes the *policy* live in the producer (user input / tool system) instead of inside raycast.

## What explicit desktop request production looks like

The missing piece is to move the current desktop mouse policy out of `RayCastSystem` and into an earlier producer stage.

Concretely, the desktop path becomes:

```text
input.process_input(...)
  → desktop pointer request producer
  → emit IntentSignal::now(IntentValue::RequestRaycast { component_ids: [pointer_raycaster] })
  → intent execution increments cast_requests
  → RayCastSystem consumes requests
  → EventSignal::RayIntersected
  → GestureSystem combines hit + mouse edge
```

That keeps the existing same-frame behavior, but makes the mouse path explicit instead of hidden inside `RayCastSystem::should_cast(...)`.

### Producer placement

The most natural place for desktop request production is immediately after input sampling, before the raycast stage.

Good candidates:
- a small desktop-pointer request pass inside the input layer
- a dedicated pointer-trigger system that runs after `input.process_input(...)`
- an intent-producing helper owned by `PointerSystem`

The important rule is not which system owns it, but that it runs early enough for `RayCastSystem` to see the request in the same frame.

If the producer emits a request signal directly, that request should still be an `IntentSignal`, not an `EventSignal`.

### Producer responsibility

The desktop producer should:
- find the raycasters driven by desktop cursor-through-camera pointers
- decide whether this frame needs hover sampling, click-start sampling, or drag-continuation sampling
- emit explicit request intents for those raycasters only

It should not:
- request casts for XR head pointers
- request casts for XR controller pointers
- infer ray origin/direction itself

That keeps responsibilities clean:
- producer answers **when should this pointer cast?**
- `RayCastSystem` answers **what did this requested cast hit?**

### Desktop request policy sketch

For editor-like desktop behavior, the producer policy can be simple:

- request every frame while the cursor is inside the window, so hover/selection previews continue to work
- request on left-press frames so click-start has a hit available
- request every frame while a desktop drag is active so drag updates stay continuous

In pseudocode:

```text
for each desktop cursor pointer raycaster:
  if cursor_in_window || left_pressed_this_frame || left_drag_active:
    emit IntentSignal::now(IntentValue::RequestRaycast { component_ids: [raycaster] })
```

This is intentionally boring: it recreates the current desktop experience, but in the correct layer.

### Minimal migration shape

The smallest refactor is:

1. Keep `IntentValue::RequestRaycast` and `cast_requests` for now.
2. Add a desktop request producer that emits `IntentSignal::now(IntentValue::RequestRaycast { ... })` for desktop pointers.
3. Remove mouse-edge checks from `RayCastSystem::should_cast(...)`.
4. Reduce `should_cast(...)` to `cast_requests > 0`.
5. Once that works, remove `RayCastMode` entirely and rename the API if desired.

This lets the architecture improve in two safe steps:
- first move policy out of raycast
- then delete the now-redundant mode model

### How desktop pointer discovery should work

The producer should reuse the same pointer classification used elsewhere:
- only pointers whose source resolves to `CursorThroughActiveCamera` are desktop mouse candidates
- `ParentForward` pointers are not implicitly desktop-driven, even if a mouse exists

This is the key behavior boundary that avoids reintroducing the mixed desktop/XR bug.

### Why this is better than keeping a mouse special-case in raycast

Moving the desktop path into explicit request production gives us:

- one place to define desktop hover/click/drag policy
- the same architectural shape that future XR dwell/controller producers will use
- a `RayCastSystem` that becomes a pure intersection stage
- easier debugging, because request producers can be logged/inspected directly

After this change, “desktop mouse behavior” is no longer a hidden fallback. It becomes a normal request producer, just like future XR and tool-driven paths.

## Timing implications

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

## Naming

The user-facing intent is closer to “intersect” than “raycast”:
- a request asks the engine to compute intersections and publish `RayIntersected`

So renaming is worth considering:
- `IntentValue::RequestRaycast` → `IntentValue::RequestRayIntersection` or `IntentValue::RayIntersectRequest`

This note uses “request” terminology to emphasize that this is not an immediate synchronous query.

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

## Suggested remaining steps

1. Route the current desktop mouse path through explicit request production.
2. Change `RayCastSystem` to cast only when requested.
3. Remove `RayCastMode` and the remaining `InputState` checks from `RayCastSystem`.
4. Update `RayCastComponent` docs, specs, and examples to match the request-only model.

## Follow-ups

This raycast refactor pairs naturally with the gesture refactor:
- gesture lifecycle should be driven by pointer/button signals, not mouse state inside `GestureSystem`
- `DragUpdatePolicy` should be per pointer/raycaster
- `StartPlaneProjection` should use the pointer’s ray source (not always cursor)

Those are tracked in `docs/spec/gestures-and-gizmos.md`.
