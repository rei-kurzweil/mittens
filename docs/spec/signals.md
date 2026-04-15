
# Signals

This is the canonical doc for the engine’s “signals-first” layer.

The doc is intentionally split into three parts:

1) Current design / goal (what to rely on)
2) Unsettled decisions (what’s still in flux)
3) Current status (what is implemented today)

## Current design / goal

### What a signal is

A `Signal` is a typed message with:

- `scope: ComponentId` — where it “happened” (used for subtree-scoped dispatch)
- `event: Option<EventSignal>` — fact/observation payload (routed to handlers)
- `intent: Option<IntentSignal>` — side-effect request payload (executed at drain points)

Timing (`SignalWhen`) lives on `IntentSignal` (not events).

`SignalWhen::AtBeat(b)` means the signal is held until transport beat $\ge b$.

### Intent/event model replaces the older action/event split

The canonical model is now:

- `IntentSignal` = side-effect request
- `EventSignal` = observed fact

Older docs may refer to an “action” layer or `ActionMethod` as if it were the public conceptual API. That is historical terminology only.

Implementation note:

- `ActionComponent` / `ActionSystem` may still appear in code as legacy plumbing reused by intent execution
- but the architectural model to document and build on is **intent vs event**, not **action vs event**

### One stream, explicit drain points

The core invariant is:

- Signals are **emitted** freely during systems/ticks.
- Signals are **executed and observed** only at explicit **drain points**.

Drain points are implemented by `SystemWorld::process_signals(...)`.

At a high level, each drain point does:

1. Move locally-staged signals into the main bus.
2. Promote due timed signals (`AtBeat`) into the ready queue.
3. For each ready signal (up to a cap):
   - run the engine’s execution stages
   - then dispatch handlers as observers

### Two-stage intent execution

Intent execution is intentionally split into two layers:

- **Intent interpretation stage**: `rx::RxIntentExecutor`
  - Runs for high-level `IntentValue`s that expand into follow-up intents/events.
  - Emits follow-up work via `SignalEmitter`.

- **Default executor stage**: `SystemWorld::execute_intent_signal(...)`
  - Applies canonical engine side effects (register/remove/update, system registration, etc).

After execution, `RxWorld` dispatches handlers for observation.

Design goal: handlers should be observers/emitters, not “the place where mutations happen”.

#### Guideline: where intent logic lives

- If an intent can be fulfilled with a **small amount of code** and is **not system-specific** (e.g. topology helpers like attach/detach/remove), implement it directly in the **IntentExecutor**.
- If fulfilling an intent is more than a few lines, or clearly belongs to a system, the **IntentExecutor should still “own” fulfilling the intent**, but it should delegate to the appropriate system.
  - Example: an intent that affects rendering should delegate to Renderable/Texture/Visual systems.
  - Example: an intent that affects physics should delegate to Collision/KineticResponse systems.

### Scoped dispatch

Handlers are registered at `(SignalKind, scope_root)`.

When a signal with `scope = S` is dispatched, the engine walks ancestry:

`S, parent(S), parent(parent(S)), ...`

and invokes any handlers registered at any of those nodes.

This gives you “subscribe to a subtree” semantics without global filtering.

Important clarification:

- this is **ancestor-bubbling only**
- a parent can observe child-scoped events
- a child cannot observe parent-scoped events just by registering a scoped handler

So the current runtime does **not** provide a second propagation mode such as
"child listens to parent events".

If a component needs to react to an upstream event and expose a component-local semantic
event (for example `ScrollingComponent` projecting ancestor `DragMove` into a local
`Scrolling` event), the current model is:

- register a handler at the upstream scope
- map the upstream event in handler code
- emit a new event scoped to the component that owns the behavior

See [docs/draft/event-signal-pipelines.md](../draft/event-signal-pipelines.md) for the draft
proposal to formalize that pattern as an event routing/projection layer.

Example: listen for topology changes in a subtree:

```rust
use cat_engine::engine::ecs;

fn on_parent_changed(
  _world: &mut ecs::World,
  _emit: &mut dyn ecs::SignalEmitter,
  signal: &ecs::Signal,
) {
  let Some(ecs::EventSignal::ParentChanged { child, old_parent, new_parent }) = signal.event.as_ref() else {
    return;
  };
  println!("child={child:?} old={old_parent:?} new={new_parent:?}");
}

fn setup(universe: &mut cat_engine::engine::Universe, scope_root: ecs::ComponentId) {
  universe.add_signal_handler(ecs::SignalKind::ParentChanged, scope_root, on_parent_changed);
}
```

### Scheduling: can attach/detach/remove be timed?

Yes: signals carry `when`, and `RxWorld` supports a holding pen for `SignalWhen::AtBeat`.

Practical semantics (important): timing delays *eligibility*; resolution happens at execution time.
So if you schedule something structural like `Attach` / `Detach` / `RemoveSubtree`:

- It executes at the due drain point.
- It is best-effort with respect to world state at that time.
  - If the referenced `ComponentId`s no longer exist, the operation should effectively no-op.
  - If topology has changed, the operation applies to the current topology.

Design constraint / goal (not fully enforced yet):

- Only **intent-ish** operations should be scheduled.
- Facts/events (e.g. `ParentChanged`) should not be scheduled.
- Low-level internal registrations (e.g. `RegisterRenderable`) should not be scheduled.

### Subtree deletion: no `*Immediate`

Subtree deletion is represented by `IntentValue::RemoveSubtree { target: Vec<ComponentId> }`.

There is no `RemoveSubtreeImmediate` variant. Deletion happens at drain points via the default
executor, which:

- detaches the root (if still attached) and emits `ParentChanged`
- performs best-effort system teardown (renderables/collision/etc)
- removes the component subtree from `World`

This keeps “when does deletion happen?” aligned with drain points and avoids duplicated API
surface area.

## Unsettled decisions

- **Type shape**: keep one flat `IntentValue` enum that mixes user intents and internal mutations, or split into explicit `UserIntent`/`EcsMutation` enums?
- **Scheduling policy**: should we hard-forbid `AtBeat` for low-level internal ops and events at the type level?
- **Failure semantics**: when a scheduled signal references missing components, should we (a) silently no-op, (b) return an error somewhere, or (c) emit a structured failure event?
- **Re-entrancy**: if handlers emit more signals, do they run in the same drain point or always at the next one? What are the budgets/caps per stage?
- **Ordering guarantees**: do we need a single total order across “intent vs events”, or is staged ordering sufficient? Should signals carry a `seq: u64`?
- **Global handlers**: keep global handlers in `RxWorld`, or require explicit scope roots only?
- **Handler API**: keep public handlers as `fn` pointers, or move to `HandlerId` + boxed closures for ergonomics?
- **Where intent logic lives**: how far do we push `RxIntentExecutor` vs keeping a legacy interpreter layer around system-owned mutations?

## Current status (2026-03-06)

- `SignalWhen::{Now, AtBeat}` exists and timed signals are held pending until `ClockSystem` beat is due.
- Drain-point execution lives in `SystemWorld::process_signals(...)`.
- `CommandQueue` is a transitional per-frame staging emitter (no raw pointers); it drains into `SystemWorld.rx` at drain points.
- `RemoveSubtreeImmediate` is gone; `RemoveSubtree { target }` is the one subtree deletion action.
- `SetTextImmediate` is gone; `SetText` executes at drain points and rebuilds the glyph subtree.
- Intent execution is in transition:
  - `RxIntentExecutor` exists and currently reuses some legacy `ActionSystem` interpretation logic for many high-level `IntentValue`s.
    - The default intent executor (`execute_intent_signal`) applies canonical side effects.
