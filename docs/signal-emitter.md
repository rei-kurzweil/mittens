# SignalEmitter / Emitter

This document explains the `SignalEmitter` interface used by the ECS signal system (`RxWorld`), how the internal `Emitter` implementation works during dispatch, and what it buys us architecturally.

## The problem this solves

We want signal handlers to be able to **emit follow-up signals** while they are running.

Examples:
- An **action** signal like `Attach { ... }` mutates topology and should also emit a fact signal like `ParentChanged { ... }` so downstream systems (e.g. transform refresh, editor tools, etc.) can react.
- Gesture interpretation might want to emit higher-level drag signals derived from a raycast hit.

However, `RxWorld` stores handler lists inside itself (global/scoped maps). During dispatch, we must iterate those handler lists. If we also handed handlers a `&mut RxWorld`, they could call `rx.push(...)` (which is great) *but* they could also mutate handler maps while we are iterating them. That creates:
- Borrow-checker conflicts (we already mutably borrowed the handler list)
- Potential iterator invalidation / reallocation hazards if handlers add/remove handlers

So we need a way for handlers to emit signals without giving them full mutable access to `RxWorld`.

## The interface: `SignalEmitter`

`SignalEmitter` is a tiny trait:

- It exposes only one capability: **push a new signal**.
- It does *not* expose handler registration/removal or access to `RxWorld` internals.

Conceptually:

- `RxWorld` owns the signal queue (`Vec<Signal>`).
- Handlers get a restricted handle that can only append to that queue.

This is similar to passing a “sink” or “bus” instead of passing the whole world.

## The implementation: internal `Emitter`

Inside `RxWorld::dispatch_handlers`, we construct an internal `Emitter`:

- `Emitter` contains a raw pointer to `RxWorld.signals` (a `*mut Vec<Signal>`).
- When a handler calls `emit.push(scope, value)`, the `Emitter` appends to that vector.

Why a raw pointer?
- During dispatch, we already have a mutable borrow of (parts of) `RxWorld` to iterate handler vectors.
- A normal `&mut Vec<Signal>` borrow would overlap with those borrows and Rust won’t allow it.
- A raw pointer lets us safely *separate* “mutate the signals queue” from “iterate handler lists” without exposing the rest of `RxWorld`.

### Safety story

The safety claim is narrow:
- The `Emitter` pointer always points at the `signals` field inside the same `RxWorld`.
- The pointer is used only while `dispatch_handlers` is executing.
- Pushing onto a `Vec` can reallocate the *buffer*, but it does not move the `Vec` struct itself. Our pointer is to the `Vec` struct (the field), not the buffer.

What we *avoid* on purpose:
- Allowing handlers to mutate handler maps while they are being iterated. `SignalEmitter` doesn’t provide APIs for that.

## What this buys us

### 1) Actions can be “just signals”

With `SignalEmitter`, an ActionSystem handler can:
- Consume `SignalValue` action variants
- Mutate `World` / queue commands
- Emit additional fact signals (e.g. `ParentChanged`) in the same dispatch pass

That enables the desired semantics:
- “Events are signals.”
- “Actions are signals.”
- Systems react by listening rather than being called directly.

### 2) Immediate-mode signal graphs

`RxWorld::dispatch_new_signals` already supports dispatching signals multiple times per frame at explicit points (raycast → gesture → gizmo, etc.).

With `SignalEmitter`, handlers can produce downstream signals and they’ll be picked up by subsequent dispatch passes (or even the same pass if they’re appended before the cursor reaches them).

### 3) Keeps the handler API simple

Handler signature becomes:

- `fn(&mut World, &mut CommandQueue, &mut dyn SignalEmitter, &Signal)`

This keeps handlers:
- Mostly pure (World/Queue mutation is explicit)
- Able to extend the signal stream
- Not coupled to `RxWorld` internals

## Limitations / tradeoffs

- Handlers cannot register/unregister other handlers via the emitter (by design).
- You can still create feedback loops by emitting new signals that cause more signals. This is managed by dispatch limits:
  - `dispatch_new_signals(..., max_signals)` bounds work per dispatch call.
  - `SystemWorld` uses explicit dispatch points and caps.
- `SignalEmitter` is intentionally minimal. If we later need richer capabilities (e.g. “emit at same scope as current signal”, “defer until end-of-frame”), we can add small helper methods without exposing full `RxWorld`.

## Where to look in code

- Trait + handler type: `src/engine/ecs/rx/signal.rs`
- Dispatch + internal emitter: `src/engine/ecs/rx/rx_world.rs`
- Example immediate handler installation: `src/engine/ecs/system/gesture_system.rs` (`install_immediate_handlers`)
