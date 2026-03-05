# Unified Signals v2 — where CommandQueue ended up (and what still exists)

Date: 2026-03-04

This document is a “current status” snapshot answering:

- What `CommandQueue` used to do, vs what it does now
- Whether there is still a facade (yes)
- How systems currently build component trees (e.g. `TextComponent` spawning glyph subtrees)
- What role `ActionSystem` currently plays

It’s grounded in these files:

- `src/engine/ecs/command_queue.rs`
- `src/engine/ecs/rx/signal.rs`
- `src/engine/ecs/rx/rx_world.rs`
- `src/engine/ecs/system/system_world.rs`
- `src/engine/ecs/mod.rs` (`World::init_component_tree`)
- `src/engine/ecs/system/text_system.rs`

---

## 1) What CommandQueue used to be used for

Historically, `CommandQueue` was the engine’s *mutation transport*: many callsites pushed imperative commands into it (register renderables, update transforms, rebuild text, remove subtrees, etc.), and then an explicit `flush()` applied them into `SystemWorld` / `VisualWorld` in a deterministic place.

That transport role has now shifted to the unified signal stream:

- The “things that used to be commands” are now **typed `SignalValue` variants** (e.g. `RegisterTransform`, `UpdateTransform`, `RegisterRenderable`, …).
- Those signals are executed by the **default executor stage** during drain points.

Separately, `CommandQueue` was also carrying per-frame transport (`beat_now`, `bpm`). That’s still true today, but it’s increasingly a historical artifact.

---

## 2) Current status: is there still a facade?

Yes.

### What `CommandQueue` is today

`CommandQueue` is still a type that many systems/components accept, but it is no longer a “command enum queue” and it does **not** own engine state.

Instead:

- It implements `SignalEmitter`.
- It has a **local `queued: Vec<Signal>`** (signals staged locally).
- At drain points, the engine **drains** those staged signals into `SystemWorld.rx`.

Key property: it no longer stores raw pointers into `RxWorld`.

This matters because previously it attempted to hold a `*mut RxWorld`, which becomes **dangling** if the owning `Universe` is moved in memory (self-referential struct pitfall). The current design avoids that class of UB by keeping `CommandQueue` self-contained and transferring signals explicitly.

### What still looks “old”

The facade still exposes methods like:

- `register_transform`, `update_transform`, `remove_subtree`, …

…but these are now just convenience wrappers that enqueue typed `SignalValue` variants.

---

## 3) The new runtime pipeline (execute + observe)

The core drain-point loop lives in `SystemWorld::process_signals`.

At a high level, each drain point does:

1) Drain `CommandQueue`’s locally queued signals into `RxWorld`.
2) Promote due timed signals from `RxWorld.pending` into `RxWorld.signals`.
3) For each queued signal (cursor-based drain):
   - If `SignalKind::Action`: run the default executor (`execute_action_signal`).
   - Then dispatch scoped/global handlers (observers) via `RxWorld::dispatch_handlers`.
4) If the executor queued more signals into `CommandQueue`, drain them and keep going.

This is the critical property we want:

- Systems/handlers can *emit* follow-up work.
- The engine decides *when it becomes real* (at drain points), with deterministic ordering.

---

## 4) How systems “build component trees” now (Text glyph expansion)

There are two distinct operations that tend to get conflated:

1) **Building/spawning nodes in the world graph**
   - This is still done by directly mutating `World`:
     - `world.add_component_boxed_named(...)`
     - `world.add_child(...)`
     - etc.

2) **Initializing those nodes (running `Component::init`)**
   - This is now signal-emitter based:
     - `Component::init(&mut dyn SignalEmitter, ComponentId)`
   - The canonical helper is:
     - `World::init_component_tree(root, emit)`

### The `TextComponent` flow (current)

When a text node is registered:

- Executor stage executes `SignalValue::RegisterText { component }` by calling `SystemWorld::register_text(...)`.
- `register_text(...)` calls `TextSystem::register_text(...)`, which expands the `TextComponent` into glyph subtrees (adds transforms/renderables/etc). This mutates `World`.
- Then `register_text(...)` calls:

```rust
world.init_component_tree(component, queue);
```

Note:

- The emitter passed here is currently `queue: &mut CommandQueue`.
- Each newly spawned component’s `init(...)` typically emits registration/update signals into that emitter.
- `SystemWorld::process_signals(...)` drains those queued signals into `RxWorld` and continues executing them **in the same drain-point pass**.

So the loop looks like:

- spawn glyph nodes → init glyph nodes → init emits register signals → drain point executes register signals → visuals/caches become consistent.

### Why this is still a facade today

`init_component_tree` wants `&mut dyn SignalEmitter`.

- In a future steady state, you could pass `&mut SystemWorld.rx` directly (or another emitter), and eliminate `CommandQueue` entirely.
- Today, `CommandQueue` remains a convenient emitter that many callsites already have.

---

## 5) What role ActionSystem has currently

`ActionSystem` is currently best described as an **intent interpreter**.

There are (at least) two “classes” of `SignalKind::Action` signals in the unified stream:

1) **Intent / user-facing actions** (still handled by ActionSystem)
   - Examples:
     - `SetColor { rgba }`
     - `SetText { text }`
     - `Attach { child }`
     - `Detach { target }`
     - `RemoveSubtree { target }`
     - etc.

   These are not “directly executed” by `execute_action_signal` (it mostly executes the former command-queue variants).

   Instead:
   - executor runs (often does nothing for these intent variants)
   - then `ActionSystem`’s handler runs and translates intent into concrete mutation signals:
     - e.g. mutates component fields in `World`
     - emits `RegisterColor`, `UpdateTransform`, `RemoveSubtreeImmediate`, etc.

2) **Mutation / former-command signals** (executed by default executor)
   - Examples:
     - `RegisterRenderable`, `UpdateTransform`, `RegisterText`, …

   These are executed by `SystemWorld::execute_action_signal`.
   `ActionSystem` will still *observe* them (because it is installed as a handler for `SignalKind::Action`), but it generally ignores them.

### Current wart: transport inside ActionSystem

`ActionSystem` currently hardcodes `beat_now = 0.0` in its handler.

That means any “schedule at now” behavior inside ActionSystem is not using the real transport (`ClockSystem`).

The architecture direction remains:

- executor/stage owns transport (`ClockSystem`)
- handlers remain transport-blind
- timing is carried in the envelope (`SignalWhen`) and/or in explicit beat fields in signal payloads

So the remaining work is to remove that implicit/placeholder beat usage in ActionSystem.

---

## 6) Summary / current mental model

- `RxWorld` is the single signal stream (plus a timed holding pen).
- Drain points in `SystemWorld` are the “barriers” where signals are executed then observed.
- The old command queue functionality is now represented as typed `SignalValue` variants executed by the default executor.
- `CommandQueue` still exists as a compatibility facade and per-frame context carrier:
  - it collects emitted signals locally
  - drain points move them into `RxWorld`
- `World::init_component_tree` is how newly spawned subtrees (like text glyphs) get their components initialized, and that init path emits signals too.
- `ActionSystem` is the interpreter for intent-style action signals, converting them into canonical mutation signals.

---

## 7) Immediate next cleanup candidates

(These are still valid, but section 10 is the more concrete “do this in order” plan.)

- Remove `beat_now = 0.0` from `ActionSystem` by:
  - converting those cases into envelope-timed signals, or
  - moving transport-sensitive resolution into the executor stage.
- Gradually replace `&mut CommandQueue` parameters with `&mut dyn SignalEmitter` where a function only emits signals.

---

## 8) API inventory (core vs transitional vs vestigial)

The goal of this section is to make it easy to answer “what should I be using?” at a glance.

Legend:

- **Core**: intended steady-state API for the unified-signal architecture.
- **Transitional**: supported today, but meant to shrink/disappear.
- **Vestigial**: exists mainly for compatibility; avoid adding new callsites.

### A) Emitting + handling signals (top of the stack)

| API / entry point | Status | What it does today | Direction / replacement |
|---|---|---|---|
| `RxWorld::push(scope, value)` | **Core** | Adds an immediate signal to the per-frame queue. | Keep. |
| `RxWorld::push_at_beat(scope, beat, value)` | **Core** | Adds a timed signal into the holding pen (`pending`). | Keep; define cancellation/GC semantics if we schedule far-future work. |
| `RxWorld::promote_due_signals(now_beat)` | **Core** | Moves due timed signals from `pending` into the per-frame queue. | Keep; this is the “timed holding pen” boundary. |
| `SignalEmitter::{push, push_at_beat}` | **Core** | Minimal capability given to handlers/components to emit signals. | Keep. |
| `RxWorld::take_next_undispatched()` | Transitional | Cursor-based “pop next” used by drain points. | Fine to keep internal; drain points are the public model. |
| `RxWorld::dispatch_handlers(world, env)` | **Core** | Runs global + scoped handlers for a single signal. | Keep. |
| `RxWorld::dispatch_new_signals(world, max)` | Transitional | Convenience “dispatch from cursor” loop. | In practice `SystemWorld::process_signals` is the canonical drain point now. |
| `SystemWorld::process_signals(world, visuals, queue, max)` | **Core** | Drain point: execute default executor for `Action` signals, then notify handlers; also promotes timed signals and drains queued facade signals. | Keep; eventually remove the `queue` parameter once the facade is gone. |
| `SystemWorld::execute_action_signal(...)` | **Core** | Default executor for “former command-queue” mutation signals. | Keep; treat as the canonical mapping from typed signal → system call. |

### B) Mid-stack: compatibility facades + intent interpretation

| API / type | Status | What it does today | Direction / replacement |
|---|---|---|---|
| `CommandQueue` (type) | Transitional | A signal-emitter facade + per-frame transport carrier; stages `Vec<Signal>` locally and is drained at drain points. | Shrink and delete once callsites are migrated to `&mut dyn SignalEmitter`/`&mut RxWorld`. |
| `CommandQueue::drain_into_rx(&mut self, rx: &mut RxWorld)` | Transitional | Moves staged signals into `RxWorld` (preserves drain-point barriers without raw pointers). | Keep until facade is gone; then delete. |
| `CommandQueue::{set_transport, beat_now, bpm}` | Transitional | Threads per-frame beat/bpm to callsites that still need transport context. | Remove by moving transport-sensitive work into executor stage / ClockSystem-owned code paths. |
| `CommandQueue::flush(world, systems, visuals)` | Vestigial-ish | Calls `SystemWorld::process_signals(...)` (it is no longer a “flush commands into systems” primitive). | Replace callsites with explicit drain points (`systems.process_signals`). |
| `CommandQueue::{register_renderable, remove_renderable}` | Transitional | Enqueues `SignalValue::{RegisterRenderable, RemoveRenderable}` (→ `SignalKind::Action`). | Prefer emitting `rx.push(..., SignalValue::RegisterRenderable { .. })` directly. |
| `CommandQueue::{register_transform, update_transform}` | Transitional | Enqueues `SignalValue::{RegisterTransform, UpdateTransform}` (→ `SignalKind::Action`). | Prefer emitting directly; eventually delete facade methods. |
| `CommandQueue::{register_camera_3d, register_camera2d, make_active_camera}` | Transitional | Enqueues `SignalValue::{RegisterCamera3d, RegisterCamera2d, MakeActiveCamera}`. | Prefer emitting directly. |
| `CommandQueue::{register_text, set_text}` | Transitional | Enqueues `SignalValue::{RegisterText, SetTextImmediate}` (note: `set_text` is already “immediate text payload”, not the higher-level `SetText` intent). | Keep until Text/intent layering is clarified. |
| `CommandQueue::{register_collision, remove_collision}` | Transitional | Enqueues `SignalValue::{RegisterCollision, RemoveCollision}`. | Prefer emitting directly. |
| `CommandQueue::remove_subtree(root)` | Transitional | Enqueues `SignalValue::RemoveSubtreeImmediate` (note the “Immediate” payload name). | Prefer emitting directly. |
| `ActionSystem` | Transitional (but important) | Interprets intent-style `SignalKind::Action` and emits canonical mutation signals. | Long-term: move more intent resolution into executor stage or make intent signals more data-complete; reduce direct `World` mutation in handlers. |

### C) Bottom: world-graph construction (foundation layer)

These APIs are *not* message transport; they are the “physical graph” operations.

| API / entry point | Status | What it does today | Direction / replacement |
|---|---|---|---|
| `World::add_component<T: Component>(c)` | **Core** | Allocates a component node in the world graph (no parent). | Keep. |
| `World::add_component_boxed(c)` | **Core** | Same as above, but dynamic/boxed. | Keep. |
| `World::add_component_boxed_named(name, c)` | **Core** | Adds a boxed component with a human/debug name. | Keep. |
| `World::add_component_boxed_with_guid_named(guid, name, c)` | **Core** | Adds a boxed component with an explicit GUID + name (deserialization). | Keep. |
| `World::add_child(parent, child)` | **Core** | Attaches nodes; establishes ancestry used for scoping. | Keep. |
| `World::remove_component_subtree(root)` | **Core** | Deletes a subtree in the world graph. | Keep; ensure systems/visuals cleanup is driven by canonical mutation signals at drain points. |
| `World::init_component_tree(root, emit)` | **Core** | Calls `Component::init` for uninitialized descendants; init emits registration signals. | Keep; prefer `emit: &mut dyn SignalEmitter` (not `CommandQueue`) over time. |

---

## 9) Why does so much still rely on CommandQueue?

Two separate historical motivations have gotten bundled under the name “CommandQueue”, and we’re still unwinding them:

### 9.1 It used to be a *barrier/batching* mechanism

Yes: the original value was largely “batch and control *when* expensive cross-system work happens” (e.g. registering renderables into `VisualWorld`, BVH refits, text expansion, audio graph rebuild) rather than “prevent World mutation”.

The unified-signals architecture keeps that same idea, just with different plumbing:

- **Drain points** are now the barrier.
- The default executor is the “apply expensive work” step at the barrier.

So the batching/control concept still exists; the transport changed.

### 9.2 It’s still threaded everywhere as a *convenience emitter*

Even after commands became signals, we deliberately kept `CommandQueue` around to avoid a repo-wide mechanical signature rewrite.

Concrete reasons it still shows up today:

- A lot of callsites already accept `&mut CommandQueue` and only need “emit some mutations”. Keeping the facade kept the migration incremental.
- Component lifecycle (`Component::init/cleanup`) needs *some* emitter to enqueue “register me” signals. Today, that’s often the queue.
- Some systems still have helper APIs named `tick_with_queue(...)` etc.

### 9.3 Why it’s now safe (and why we changed it)

The previous implementation attempted to store a raw pointer into `RxWorld` inside `CommandQueue`.

That creates a self-referential-struct hazard: if the owning `Universe` moves, the pointer dangles and you get crashes/black screens.

Today, `CommandQueue` stages `Vec<Signal>` locally and drain points explicitly move them into `RxWorld`, keeping the “emit anywhere, apply at barriers” ergonomics without unsafe pointer aliasing.

---

## 10) Suggested next steps (concrete, in-order)

1) **Stop using `CommandQueue` as transport**
  - Remove remaining dependencies on `CommandQueue::{set_transport, beat_now, bpm}`.
  - Replace ActionSystem’s `beat_now = 0.0` behavior by making those actions data-complete (envelope timing or explicit beat fields) or resolving them in the executor stage where `ClockSystem` is available.

2) **Start deleting facade callsites opportunistically**
  - When a function only emits signals, switch `&mut CommandQueue` → `&mut dyn SignalEmitter`.
  - Prefer passing `&mut SystemWorld.rx` as the emitter from systems (when you’re not depending on the queue’s legacy API surface).

3) **Make drain points explicit everywhere**
  - Replace `queue.flush(...)` with `systems.process_signals(...)` at the callsites that currently use it as a barrier.
  - This makes it clear that “signals are applied at drain points”, not “commands are flushed”.

4) **Shrink/remove ActionSystem surface area**
  - Decide which intent signals should continue to exist vs which should be replaced by canonical mutation signals.
  - Move more “intent → canonical mutation” into executor stage (or into producers like `AnimationSystem`) so handlers trend toward observe/derive.

5) **Finally: delete `CommandQueue`**
  - Once nothing depends on it for emission or transport, remove the type and the compatibility methods.

---

## 11) Design exploration: removing `SignalKind::Action` (and making dispatch explicit)

You’re not imagining things: a lot of stuff is still “Action”. In code today:

- `SignalKind::Action` still exists.
- `SignalValue::kind()` maps a *large* set of “side-effectful / engine-defined” `SignalValue` variants to `SignalKind::Action`.
- `SystemWorld::process_signals` uses `env.kind() == SignalKind::Action` as the gate for the built-in executor (`execute_action_signal`).
- `ActionSystem` installs a *handler* for `SignalKind::Action` and interprets a subset of those values as intent.

So `SignalKind::Action` is currently doing double-duty:

1) **“Needs built-in executor work at drain points”**
2) **“A handler wants to observe and/or translate this”**

That overloading is what feels messy: intent-ish things like `SetColor` and canonical mutations like `RegisterTransform` are both “Action”, but they want different built-in behavior.

### 11.1 What CommandQueue methods really are

The `CommandQueue::{register_transform, update_transform, ...}` methods are not “methods of `SignalKind::Action`”; they are convenience wrappers that enqueue `SignalValue` variants which currently happen to map to `SignalKind::Action` via `SignalValue::kind()`.

### 11.2 Goal

If we want to truly “do away with Action”, we need to make these two decisions explicit per signal:

- **Executor routing**: does any built-in executor run for this signal at drain points? If yes, which one?
- **Handler routing**: which handler group(s) should see this signal (scoped/global observers)?

Right now, `SignalKind` is serving as the *only* routing key for both.

### 11.3 Minimal fix: split Action into two topics

The lowest-churn improvement is to replace `Action` with at least two coarse kinds:

- `SignalKind::Intent` (handled by a renamed `IntentSystem` — today’s `ActionSystem`)
- `SignalKind::Mutation` (engine-defined canonical mutations executed by the default executor)

Then:

- `SystemWorld::process_signals` runs `execute_*` only for `Mutation`.
- `IntentSystem` listens only to `Intent`.
- Intent handlers emit `Mutation` signals.

This keeps handler registration (a small enum key) and avoids exploding kinds into “one kind per variant”.

### 11.4 More explicit routing: add a dispatch field to the envelope

If you want the signal itself to carry “how should I dispatch?”, add routing metadata to the envelope.
For example:

```rust
pub enum BuiltinExecutor {
  WorldMutation,
  // (future) AudioScheduling,
  // (future) TopologyMutation,
}

pub enum SignalTopic {
  Any,
  // Facts:
  ParentChanged,
  RayIntersected,
  CollisionStarted,
  CollisionEnded,
  DragStart,
  DragMove,
  DragEnd,
  // Coarse buckets:
  Intent,
  WorldMutation,
}

pub struct SignalRoute {
  pub topic: SignalTopic,
  pub builtin: Option<BuiltinExecutor>,
}

pub struct Signal {
  pub scope: ComponentId,
  pub value: SignalValue,
  pub when: SignalWhen,
  pub route: SignalRoute,
}
```

Then the drain point becomes conceptually:

- If `env.route.builtin == Some(WorldMutation)`: execute built-in mutation executor.
- Dispatch handlers by `env.route.topic` (and optionally also by `Any`).

This decouples “executor selection” from “which handlers observe”, and makes it possible to delete `SignalKind::Action` entirely.

### 11.5 Where does `route` come from?

Two reasonable approaches:

1) **Computed default**: keep a `SignalValue::default_route()` (or similar) that chooses routing based on the variant.
2) **Constructor APIs**: avoid public `Signal { .. }` construction; expose helper constructors like:
   - `Signal::intent(scope, when, value)`
   - `Signal::mutation(scope, when, value)`
   so callsites don’t forget to set routing.

### 11.6 Migration sketch (no compatibility constraints)

Given the repo policy of *not* preserving backward-compat aliases for schema changes, we can do this cleanly:

1) Introduce `SignalTopic` + `SignalRoute`.
2) Switch handler maps in `RxWorld` from `HashMap<SignalKind, ...>` → `HashMap<SignalTopic, ...>`.
3) Switch drain-point executor gating from `SignalKind::Action` → `route.builtin`.
4) Split current “Action” values into either:
   - intent topic (handlers translate), or
   - mutation topic + builtin executor.
5) Delete `SignalKind::Action`.

This keeps the ergonomic “typed variants” model (each former command is still a distinct `SignalValue`), but removes the overloaded `Action` bucket and makes the execution model less surprising.
