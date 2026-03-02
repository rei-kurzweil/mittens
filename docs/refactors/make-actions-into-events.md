# Make actions into events (proposal)

This doc proposes unifying “actions” and “events” into **one signal stream**, where “actions” become **user-facing event signals** that Cat Engine encourages game code to emit.

This is a **design sketch only**: no code changes.

Prerequisite: the immediate-mode signal graph proposal in `docs/refactors/immediate-mode-signal-graph.md`.


## Motivation

Current structure:

- `ActionSignal::Action(Action)` represents intent.
- `EventSignal::...` represents facts.
- `SignalValue` is `Action(...) | Event(...)`.
- `ActionSystem` executes actions by interpreting `ActionMethod` and mutating world state (mostly via `CommandQueue`).

Pain points:

- Two parallel “kinds” of signals increases complexity.
- Action execution is a special pathway (ActionSystem) rather than “just another consumer of signals”.
- It is awkward to move to an immediate-mode signal graph if the fundamental execution model is “enqueue actions, execute later”.

Desired direction:

- **One kind of signal.**
- “Actions” are simply a curated set of event signals the developer is encouraged to emit.
- The engine reacts to those signals via systems that listen to them.
- With immediate mode, as soon as a signal is fired, it can be handled (subject to explicit dispatch points).


## High-level proposal

### 1) Collapse “actions” into event signals

- Remove the *concept* of `ActionMethod` from the signal graph design.
- Replace it with **one signal per current ActionMethod**.

Example mapping:

- `ActionMethod::SetColor` → `EngineEvent::SetColor { target: Vec<ComponentId>, rgba: [f32; 4] }`
- `ActionMethod::Attach` → `EngineEvent::Attach { parent: ComponentId, child: ComponentId }`
- `ActionMethod::Raycast` → `EngineEvent::RequestRaycast { target: Vec<ComponentId> }`

These are “events” in the sense that they are signals in the stream.

### 2) ActionSystem becomes an event listener

Instead of:

- “ActionSystem executes actions”

We want:

- “ActionSystem listens for certain user-facing events and applies side effects”.

That makes ActionSystem a normal consumer in the signal graph.

### 3) Immediate-mode delivery

This proposal assumes we move toward the immediate-mode dispatch model:

- systems can register handlers for signal kinds
- the graph can be dispatched one or more times per frame
- **no waiting until end-of-frame** for action-like signals

Concretely: if the developer emits `SetPosition`, the engine should be able to observe that signal and enqueue/apply transform changes **in the same tick** (subject to `CommandQueue` flush timing).


## Signal type design

There are a few ways to represent “one kind of signal” while retaining the helpful grouping of “user-facing action-ish signals”.

### Option A: single `Signal` enum with nested groups

- `Signal` (the only kind of signal)
  - `Signal::User(UserSignal)`
  - `Signal::Internal(InternalSignal)`

Where:

- `UserSignal` is the curated set we encourage developers to use (today’s ActionMethod surface).
- `InternalSignal` is facts like ray hits, collisions, etc.

This keeps an ergonomic boundary without a separate “ActionSignal vs EventSignal” machinery.

### Option B: flat `Signal` enum

- `Signal` has variants for everything.

Pros: simplest runtime representation.

Cons: loses the “recommended public surface” grouping.

### Recommendation

Use Option A.

Rationale:

- It matches the intent: actions are not fundamentally different, but we still want to distinguish “public/encouraged API” from internal facts.
- It keeps documentation and discoverability cleaner.


## One signal per current ActionMethod

Below is a sketch of the kinds of `UserSignal` variants we would create.

### Transform / topology

- `SetPosition { target: Vec<ComponentId>, x: f32, y: f32, z: f32 }`
- `SetTransform { target: Vec<ComponentId>, translation: [f32; 3], rotation_quat_xyzw: [f32; 4], scale: [f32; 3] }`
- `Attach { parent: ComponentId, child: ComponentId }`
- `AttachClone { parent: ComponentId, prefab_root: ComponentId }`
- `Detach { target: Vec<ComponentId> }`
- `RemoveChild { parent: ComponentId, index: usize }`
- `RemoveChildren { parent: ComponentId }`
- `RemoveSubtree { target: Vec<ComponentId> }`

### Render/UI

- `SetColor { target: Vec<ComponentId>, rgba: [f32; 4] }`
- `SetText { target: Vec<ComponentId>, text: String }`

### Raycasting

- `RequestRaycast { target: Vec<ComponentId> }`

(Important: this is the path that eventually enables removing `RayCastComponent.cast_requests`.)

### Audio

- `AudioGraphRebuild { target: Vec<ComponentId> }`
- …and any other audio ops that are currently ActionMethods


## Side effects: where do they live?

The principle is:

- The effectful logic moves out of “action execution” and into normal signal consumers.

That does **not** require that everything is in one monolithic ActionSystem.

We can keep a structure like:

- `ActionSystem` listens to most `UserSignal` variants and translates them into `CommandQueue` operations.
- Some systems can listen directly when it is a better separation:
  - e.g. audio signals might be handled by AudioSystem
  - camera/mode changes by CameraSystem

This is a refactor decision, not required by the model.


## Scope semantics for user-facing signals

This is the trickiest part.

### What scope means today

`RxWorld` uses `Signal.scope` for handler dispatch via ancestry.

In many internal events (e.g. `RayIntersected`), scope is naturally the hit renderable, so listeners can attach at a gizmo handle root, etc.

### User signals are often “global intent”

User-facing signals are typically requests like:

- “set this transform” (targets known by id)
- “attach these nodes”

Those don’t have an obvious topology-derived scope.

### Proposal: treat user signals as global by default

For `UserSignal`, default scope could be a stable global root such as:

- the universe root component (if one exists), or
- a designated “root scope” constant stored in SystemWorld, or
- a dedicated `SignalScope::Global` concept.

Handlers that want to observe user signals can attach at that global scope.

### Alternative: scope by the primary target

For some signals, we could set scope to:

- the first target id, or
- each target id (emit one signal per target)

Pros:

- can leverage ancestry dispatch to route actions to a subtree

Cons:

- multi-target semantics become ambiguous
- topology might change as a side effect, complicating “where did this signal belong”

### Recommendation

- Keep user signals **global-scoped** by default.
- If a user signal is intended to be subtree-local, introduce a dedicated variant that carries an explicit `scope_root`.

Example:

- `UserSignal::SetColorScoped { scope_root: ComponentId, target: Vec<ComponentId>, rgba: [f32; 4] }`

(But don’t add this until a real use-case appears.)


## Serialization / API surface

Today, `ActionComponent` encodes an `Action { target, method, params }` schema.

In the new model:

- a `UserSignal` variant is already a self-describing “method + typed payload”.
- we can still support a component that stores a “signal to emit” (e.g. for animations/keyframes), but the stored payload would be the specific variant.

This suggests that “ActionComponent” can evolve into something like:

- `EmitSignalComponent { signal: UserSignal }`

…but that’s a later refactor.


## Interaction with immediate-mode dispatch

This proposal is strongest when paired with immediate-mode dispatch:

- Developer emits `UserSignal::RequestRaycast { ... }`.
- RayCastSystem receives it immediately (handler) and emits `InternalSignal::RayIntersected`.
- GestureSystem receives `RayIntersected` immediately (handler) and emits drag events.
- GizmoSystem receives drag events and mutates transforms.

This is the “signal graph” we want.


## Migration plan (conceptual)

1. Implement immediate-mode dispatch (or explicit dispatch points) so systems can listen without scanning signal buffers.
2. Introduce the new unified `Signal` enum shape (likely nested `User(...)` / `Internal(...)`).
3. Add `UserSignal` variants corresponding to the current ActionMethod set.
4. Teach ActionSystem (or other systems) to handle those signals.
5. Keep backward compatibility temporarily by translating old `ActionSignal::Action(Action)` into the new user signals.
6. Once stable, remove the legacy action method representation.


## Open questions

- Should user signals always be “trusted”, or do we want validation/permission checks?
- If a user signal targets missing components, should it be ignored, logged, or treated as an error signal?
- Do we want acknowledgement signals (e.g. `SetPositionApplied`) for debugging/recording?
- How do we represent “transactional” multi-step actions (e.g. attach + set transform) in a signal-only model?

