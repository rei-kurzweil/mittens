
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
- `value: SignalValue` — payload enum
- `when: SignalWhen` — optional timing metadata on the envelope

`SignalWhen::AtBeat(b)` means the signal is held until transport beat $\ge b$.

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

### Two-stage handling for `SignalKind::Action`

Today, the code treats `SignalKind::Action` specially at drain points:

- **Intent interpretation stage**: `rx::RxIntentExecutor`
  - Runs for values that are still “intent-ish” and need to expand into canonical operations.
  - Emits follow-up signals via `SignalEmitter`.

- **Default executor stage**: `SystemWorld::execute_action_signal(...)`
  - Applies canonical engine side effects (register/remove/update, topology ops that are executed directly, etc).

After execution, `RxWorld` dispatches handlers for observation.

Design goal: handlers should be observers/emitters, not “the place where mutations happen”.

### Scoped dispatch

Handlers are registered at `(SignalKind, scope_root)`.

When a signal with `scope = S` is dispatched, the engine walks ancestry:

`S, parent(S), parent(parent(S)), ...`

and invokes any handlers registered at any of those nodes.

This gives you “subscribe to a subtree” semantics without global filtering.

Example: listen for topology changes in a subtree:

```rust
use cat_engine::engine::ecs;

fn on_parent_changed(
  _world: &mut ecs::World,
  _emit: &mut dyn ecs::SignalEmitter,
  signal: &ecs::Signal,
) {
  if let ecs::SignalValue::ParentChanged { child, old_parent, new_parent } = &signal.value {
    println!("child={child:?} old={old_parent:?} new={new_parent:?}");
  }
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

Subtree deletion is represented by `SignalValue::RemoveSubtree { target: Vec<ComponentId> }`.

There is no `RemoveSubtreeImmediate` variant. Deletion happens at drain points via the default
executor, which:

- detaches the root (if still attached) and emits `ParentChanged`
- performs best-effort system teardown (renderables/collision/etc)
- removes the component subtree from `World`

This keeps “when does deletion happen?” aligned with drain points and avoids duplicated API
surface area.

## Unsettled decisions

- **Type shape**: keep one flat `SignalValue` enum + `SignalKind::Action`, or split into explicit `Intent`/`Event` (and maybe `Mutation`) enums?
- **Scheduling policy**: should we hard-forbid `AtBeat` for low-level internal ops and events at the type level?
- **Failure semantics**: when a scheduled signal references missing components, should we (a) silently no-op, (b) return an error somewhere, or (c) emit a structured failure event?
- **Re-entrancy**: if handlers emit more signals, do they run in the same drain point or always at the next one? What are the budgets/caps per stage?
- **Ordering guarantees**: do we need a single total order across “intent vs events”, or is staged ordering sufficient? Should signals carry a `seq: u64`?
- **Global handlers**: keep global handlers in `RxWorld`, or require explicit scope roots only?
- **Handler API**: keep public handlers as `fn` pointers, or move to `HandlerId` + boxed closures for ergonomics?
- **Where intent logic lives**: how far do we push `RxIntentExecutor` vs keeping an `ActionSystem`-style interpreter?

## Current status (2026-03-06)

- `SignalWhen::{Now, AtBeat}` exists and timed signals are held pending until `ClockSystem` beat is due.
- Drain-point execution lives in `SystemWorld::process_signals(...)`.
- `CommandQueue` is a transitional per-frame staging emitter (no raw pointers); it drains into `SystemWorld.rx` at drain points.
- `RemoveSubtreeImmediate` is gone; `RemoveSubtree { target }` is the one subtree deletion action.
- `SetTextImmediate` is gone; `SetText` executes at drain points and rebuilds the glyph subtree.
- Intent execution is in transition:
  - `RxIntentExecutor` exists and currently reuses `ActionSystem`’s interpretation logic for many `SignalKind::Action` values.
  - The default executor (`execute_action_signal`) applies canonical side effects.
