# CommandQueue Audit

## What it is now

`CommandQueue` (`src/engine/ecs/command_queue.rs`) is a `Vec<Signal>` that
implements `SignalEmitter`. It stages signals locally and drains them into
`RxWorld` at explicit drain points. The module doc says it plainly:

> *"Per-frame context + legacy name: used to be a command queue."*

It does not execute anything, does not know about intent types, and does not
route or dispatch. All of that moved to `RxWorld` + `RxIntentExecutor` /
`RxMutationExecutor` during the signal refactor.

## Why it still exists

A borrow-checker constraint. `RxWorld` lives inside `Universe::systems`. The
public-facing universe helpers (`add`, `attach`, `remove_child`, etc.) need to
emit signals, but cannot hold `&mut RxWorld` at the same time as `&mut World`.
`CommandQueue` sits alongside both as a neutral staging buffer: signals are
pushed into it, then drained into `RxWorld` at well-defined drain points.

## Actual data flow

```
universe.add(id) / universe.attach(...)
  └─ push_intent_now(...)
       └─ CommandQueue.queued.push(Signal { intent: ... })

tick start ─ SystemWorld::process_commands(world, visuals, command_queue)
  └─ command_queue.drain_into_rx(&mut rx)   // moves Vec<Signal> into RxWorld
  └─ process_signals(...)                    // RxIntentExecutor / RxMutationExecutor run
```

`flush()` on `CommandQueue` also calls `process_signals` directly, used at
ad-hoc drain points mid-setup (e.g. after `init_component_tree`).

## The circular-feeling call graph

`CommandQueue::flush` takes `&mut SystemWorld` and calls back into
`SystemWorld::process_signals`, passing itself as the emitter. It works, but
reads oddly — the queue is calling into the system that owns its drain logic.

## What it could be

The concept (staging signals before drain) is still valid. The name is not.
Honest alternatives:

- **`SignalBuffer`** — describes what it is.
- **`Vec<Signal>` directly on `Universe`** — no wrapper needed; `drain_into_rx`
  and `flush` could live as free functions or on `SystemWorld`.

## Verdict

Not dead code — it is load-bearing glue for the borrow-checker split between
`World` and `RxWorld`. But it is misnamed and its `flush` method creates a
confusing inversion of control. A rename + possible inlining into `Universe`
directly would clarify the architecture without changing behaviour.
