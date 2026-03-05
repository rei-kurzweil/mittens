# Unified Signals TODO

Date: 2026-03-04

This doc is the short, actionable TODO list for finishing the “signals do everything” model:

- One unified signal stream (`RxWorld`)
- Deterministic **drain points** in `SystemWorld::tick()`
- A default executor stage for selected signal kinds (currently `SignalKind::Action`)
- No `CommandQueue` facade in the steady state (emit via `RxWorld` / `dyn SignalEmitter`)

Related background/spec: `docs/analysis/unified-signal-graph.md`.

---

## Current status snapshot

- ✅ `Signal` is `{ scope, value }` (no per-signal `immediate/direct` flag).
- ✅ Drain-point execution happens as a stage: `SystemWorld::process_signals` executes for `SignalKind::Action` then dispatches handlers.
- ✅ Handler API is `fn(&mut World, &mut dyn SignalEmitter, &Signal)`.
- ⚠️ `CommandQueue` still exists mainly to carry per-frame transport (`beat_now`/`bpm`) and is still threaded through many systems.
- ✅ Signal envelope supports optional timing metadata (`SignalWhen`).
- ✅ `RxWorld` has a timed holding-pen; due signals are promoted at drain points using the current transport beat.
- ⚠️ `ActionSystem` still uses `beat_now = 0.0` in its handler; transport is intentionally not available from handler context.

---

## TODO (checkboxes)

### 1) Transport + timing (beat/bpm)

- [x] Put optional timing info on the signal envelope (`SignalWhen`).
- [x] Implement a timed holding-pen in `RxWorld` and promote due signals at drain points.
- [ ] Decide the steady-state rule for timed signals:
    - either timed signals are a general mechanism (generic holding pen),
    - or timed signals are only used as an intermediate representation and always converted into subsystem schedules.
- [ ] Remove the last implicit beat usage inside handlers (`beat_now = 0.0` in `ActionSystem`) by converting those actions into envelope-timed signals or absolute-beat schedule ops.
- [ ] Remove `CommandQueue::{set_transport, beat_now, bpm}` usage (and then delete those APIs).

### 2) Facade removal — stop threading `CommandQueue`

- [ ] Replace `&mut CommandQueue` params with `&mut dyn SignalEmitter` / `&mut RxWorld` where the function only emits signals.
- [ ] Replace `queue.flush(...)` callsites with explicit `process_signals(...)` drain points (once the facade is gone).
- [ ] Remove `CommandQueue::bind_rx/unbind_rx` and any pointer-threading.
- [ ] Delete `CommandQueue` type entirely.

### 3) Semantics / invariants

- [ ] Decide (and document) the “steady state” split between:
  - **Intent** (user-facing requests)
  - **Mutate** (canonical engine mutations)
  - **Fact** (observations)
- [ ] Move intent execution out of handler dispatch where practical so handlers can be observe/derive.
- [ ] Add a small test that asserts drain ordering: executor stage runs before any handlers observe the same signal.
- [ ] Create/maintain a single table of “executed-by-default-executor” `SignalValue` variants and keep it in sync with `SystemWorld::execute_action_signal`.

---

## Transport holder sketches (potential shapes)

The goal is to provide a per-frame transport snapshot:

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportSnapshot {
    pub beat_now: f64,
    pub bpm: f64,
}
```

…and make it available without `CommandQueue`.

### A) Store `TransportSnapshot` directly in `World` (minimal, no signature churn)

Sketch:

```rust
pub struct World {
    // existing fields...
    transport: TransportSnapshot,
}

impl World {
    pub fn set_transport(&mut self, t: TransportSnapshot) { self.transport = t; }
    pub fn transport(&self) -> TransportSnapshot { self.transport }
}
```

Write point:
- `SystemWorld::tick()` sets `world.set_transport(TransportSnapshot { beat_now, bpm })` immediately after `ClockSystem::tick()`.

Read point:
- Any handler (e.g. `ActionSystem`) reads `world.transport().beat_now`.

Pros:
- Very small surface area change; no new plumbing through handler signatures.
- Makes transport available everywhere that has `&World`.

Cons:
- Adds a “resource-like” field to `World` without a broader resource system.

### B) Keep transport in `ClockSystem` (executor-only)

Sketch:

Executor-owned transport is already implemented by `ClockSystem`:

- It can be driven by different drivers (system clock, audio clock driver, etc).
- Systems that need transport are run from `SystemWorld::tick()` and can read `clock.beat_now()` / `clock.bpm()`.

Timed signals are represented via envelope metadata:

```rust
pub enum SignalWhen {
    Now,
    AtBeat(f64),
}
```

Write points:
- Producers (systems/executor) push timed signals via `push_at_beat(...)`.
- Drain points call `promote_due_signals(clock.beat_now())`.

Read points:
- The executor/drain stage reads transport from `ClockSystem`.
- Handlers remain transport-blind; if a signal needs timing, it should carry it (envelope timing, absolute beat in payload, or pre-resolved schedule ops).

Pros:
- No handler signature churn.
- Keeps transport centralized and consistent even when driven by the audio thread.

Cons:
- Timed holding-pen signals persist across frames; you must define cancellation/GC semantics if you create many future signals.

### C) Store transport on `RxWorld` and expose it via a new trait

Sketch:

```rust
pub trait TransportProvider {
    fn transport(&self) -> TransportSnapshot;
}

pub trait SignalContext: SignalEmitter + TransportProvider {}
```

…and update handler signature to use `&mut dyn SignalContext`.

Pros:
- Transport lives “next to” the signal stream.

Cons:
- Signature churn: handler type changes, and callers must supply a context object.
- Harder to get transport in places that only have `&World`.

### D) Put transport into signals that need it (data-carrying)

Example: make action variants that schedule audio include an explicit `beat_now` or a required `beat_context`.

Pros:
- Pure/explicit; no implicit global state.

Cons:
- Requires touching many `SignalValue` variants (schema churn), and may be noisy if many actions want “current beat”.

---

## Suggested near-term direction

If the priority is to keep transport executor-only and add general scheduling, option **B (ClockSystem transport + envelope timing holding pen)** fits best:

- Keep `ClockSystem` as the single source of transport time (even if the driver is audio-thread-derived).
- Use `SignalWhen::AtBeat` + the RxWorld holding pen for general scheduling.
- Remove implicit transport usage inside handlers (convert to envelope timing or absolute-beat schedule ops).
- Then remove transport APIs from `CommandQueue` and start the mechanical pass replacing `&mut CommandQueue` with `&mut dyn SignalEmitter` / `&mut RxWorld`.
