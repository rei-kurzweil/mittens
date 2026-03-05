# Unified Signal Graph (drain points + executor)

Date: 2026-03-04

This doc is a status report + working spec for the “signals do everything” model.

In this model:

- There is **one transport**: `RxWorld`’s signal stream (`Signal { scope, value }`).
- Some signals are **executed** by an engine executor (direct calls into target systems).
- All signals can still be **observed** via scoped/global handlers; observation happens at explicit drain points in the frame loop.

Terminology note:

- Older versions used a per-signal `immediate` flag.
- The v1 posture is **no per-signal mode flag at all**: execution is a drain stage keyed off `SignalKind`.

> Terminology used here:
> - **execute**: call the engine/system function that applies the mutation/side effect
> - **observe**: run registered signal handlers (global + scoped)
> - **drain point**: a place in the frame loop where we process newly queued signals

---

## Bullet summary (current status vs goal)

- ✅ One unified stream: intent-ish actions + mutation-ish commands + facts are all `SignalValue` variants in the same `RxWorld` queue.
- ✅ Determinism comes from drain points: we already drain between dependent systems in `tick()`.
- ✅ A direct-call executor exists for many former command signals (`SystemWorld::execute_action_signal(...)`).

- 🎯 Goal (v1 posture): no per-signal “direct mode” at all.
  - Actions/commands execute at explicit drain points in `tick()` (executor stage), not at emission time.
  - The executor can still be simple/fast (direct function calls), but it only runs when we drain.
  - Handlers still fire at drain points (after execution) for observation/derivation.

- 🎯 Goal: remove the `CommandQueue` facade so we only use signal emitters.
  - Emitting signals should not require “being inside a signal handler”; you should be able to get an emitter from engine context (e.g. `RxWorld`).

- ⚠️ Current mismatch: `CommandQueue` is still threaded widely as a transport/context carrier (beat/bpm).

---

## What “execute + observe” means in code (today)

### Core types

- `SignalValue` is the single enum holding:
  - “intent/action-ish” variants (user-facing requests)
  - “command/mutation-ish” variants (former command-queue operations)
  - facts (what used to be “events”)

See:
- `SignalValue`, `SignalKind`, `Signal`: ../../src/engine/ecs/rx/signal.rs

The runtime carrier is:

```rust
pub struct Signal {
    pub scope: ComponentId,
    pub value: SignalValue,
}
```

Execution policy:

- Don’t use a per-signal flag to decide execution.
- Drain points run a fixed pipeline: for selected kinds (currently `SignalKind::Action`), execute via the default executor stage, then dispatch handlers (observer stage).

See: ../../src/engine/ecs/rx/rx_world.rs

### The dispatcher/executor loop

`SystemWorld::process_signals(...)` is the central “drain point” primitive.

It implements the key rule:

1) If `env.kind() == SignalKind::Action`, **execute** the engine default executor.
2) Then **observe** by dispatching handlers.

See: ../../src/engine/ecs/system/system_world.rs

Small excerpt (current behavior):

```rust
while let Some(env) = self.rx.take_next_undispatched() {
  if env.kind() == SignalKind::Action {
    self.execute_action_signal(world, visuals, queue, &env);
    }
    self.rx.dispatch_handlers(world, &env);
}
```

### Small diffs (high-signal deltas)

These are the key “shape changes” that make the unified signal graph work.

**Handler signature now uses a `SignalEmitter` (not `CommandQueue`)**

See: ../../src/engine/ecs/rx/signal.rs

```diff
-pub type SignalHandler = fn(&mut World, &mut CommandQueue, &Signal);
+pub type SignalHandler = fn(&mut World, &mut dyn SignalEmitter, &Signal);
```

**`CommandQueue::flush` now means “drain signals”**

See: ../../src/engine/ecs/command_queue.rs

```diff
-/// Flush used to apply queued commands.
+/// Flush used to apply queued commands; now it executes pending signals.
 pub fn flush(&mut self, world: &mut World, systems: &mut SystemWorld, visuals: &mut VisualWorld) {
-    /* drain command enum */
+    let _ = systems.process_signals(world, visuals, self, 100_000);
 }
```

### What signals are “executed directly” (current)

The default executor (`execute_action_signal`) matches on **typed** `SignalValue` variants and calls system entry points directly.

This currently includes (high-level grouping):

- Render registration & render attributes:
  - `RegisterRenderable`, `RemoveRenderable`
  - `RegisterColor`, `RegisterOpacity`, `RegisterTransparentCutout`, `RegisterEmissive`, …
- Transform/camera:
  - `RegisterTransform`, `UpdateTransform`, `RemoveTransform`
  - `RegisterCamera3d`, `RegisterCamera2d`, `MakeActiveCamera`
- Text/texture:
  - `RegisterText`, `SetTextImmediate`
  - `RegisterTexture`, `RegisterTextureFiltering`
- Collision/kinetics:
  - `RegisterCollision`, `RemoveCollision`
  - `RegisterKineticResponse`, `RemoveKineticResponse`
- XR and raycast:
  - `RegisterOpenxr`, `RegisterControllerXr`, `RemoveControllerXr`
  - `RegisterRaycast`, `RemoveRaycast`
- Animation:
  - `RegisterAnimation`, `RegisterKeyframe`
- Audio scheduling + graph dirtiness:
  - `RegisterAudioOutput`, `RegisterAudioOscillator`, `RegisterAudioBufferSize`
  - `AudioGraphDirtyImmediate`
  - `ScheduleAudioOp`, `ScheduleAudioGraphSwap`, plus pitch/gain/enabled convenience variants
- Structural delete:
  - `RemoveSubtreeImmediate`

See the match statement in: ../../src/engine/ecs/system/system_world.rs

> Practical implication: these are the signals that have the “different dispatch method that calls the target directly”.

Goal wording:

- “direct mode” is not about running *sooner*; it’s about using a known direct-call executor *when draining*.
- If a given signal has exactly one canonical engine function it should call, direct calls can be simpler than handler dispatch.

---

## How “signals do everything” is wired right now

### `CommandQueue` is now a SignalEmitter facade (current)

Even though the type name is still `CommandQueue`, it no longer stores a command enum.
It implements the old API surface (e.g. `register_transform`, `update_transform`, `remove_subtree`) by pushing typed `SignalValue` variants.

See: ../../src/engine/ecs/command_queue.rs

Key behaviors:

- `CommandQueue::bind_rx(&mut RxWorld)` stores a pointer to the `RxWorld`.
- `CommandQueue::flush(...)` now just calls `SystemWorld::process_signals(...)`.

This is the “low callsite churn” step: most callsites continue to accept `&mut CommandQueue`, but under the hood they emit signals.

Goal: remove the facade.

- Thread `&mut dyn SignalEmitter` (or `&mut RxWorld`) directly.
- Keep drain points explicit in `tick()`.

### Universe binds queue → rx

`Universe::new(...)` binds the queue to the systems’ `RxWorld`:

- ../../src/engine/universe.rs

This is required so component init paths that only have a `&mut CommandQueue` can still emit signals.

---

## Drain points in the frame loop (mutation/propagation barriers)

Even in a “signals-only” world, you still need explicit drain points for determinism and to keep per-frame caches coherent.

Today, the drain points are implemented using `queue.flush(...)` and/or `SystemWorld::process_signals(...)`.

See: ../../src/engine/ecs/system/system_world.rs (`tick` and `process_commands`).

Observed drain point pattern in `tick(...)`:

- After GLTF spawns / registrations: `queue.flush(...)`
- After `AnimationSystem` emits signals: `process_signals(...)` then `queue.flush(...)`
- After raycast emits hit signals: `process_signals(...)`
- After gesture produces drag signals: `process_signals(...)`
- After gizmos produce transform changes: `process_signals(...)` then `queue.flush(...)`

End-of-frame in `process_commands(...)`:

- `commands.flush(...)`
- `process_signals(...)` (catch any remaining undispatched)
- `rx.drain(); rx.begin_frame();`
- `commands.flush(...)` (handlers may have produced more mutation signals)
- `audio.rebuild_dirty_audio_graphs(...)`

---

## Mutation points (thinking ahead to “no mutation points” in v1)

If v1 “won’t have mutation points” as an explicit concept, we still need a concrete replacement concept, otherwise:

- ordering becomes ambiguous
- caches get out of sync (BVH, renderable instances, derived indexes)
- it’s hard to reason about “what state are handlers observing?”

### Proposed v1 mental model: “drain points are the only mutation boundaries”

Instead of “mutation points”, treat the engine as:

- Systems and handlers *produce* signals at any time.
- The world (and system caches) only become observably different when you call `process_signals` (or `queue.flush`, which is currently a wrapper around it).

This means it’s totally reasonable to “queue up world mutations” as signals as long as you place
drain points between dependent systems in `tick()`.

So we still have boundaries, we just name them by what they do:

- **drain points** (or “signal processing points”) are where mutations happen.

### Rule of thumb (to keep v1 sane)

- Fact-ish handlers should be **observe-only** (no `World` mutation), and if they need changes they should emit mutation signals.
- The default executor should be the “source of truth” for component registration/update/remove side effects.

Today, this is not fully enforced because `ActionSystem` still mutates `World` directly for some intent signals.

### The big design lever: run execution as a drain stage (not a signal property)

In v1, treat drain points as running a fixed pipeline:

1) execute action/command signals (executor stage; direct calls)
2) dispatch handlers (observer stage)

That gives us the ordering invariant (“execute then observe”) without needing a per-signal mode flag.

---

## TODO (checkboxes)

### Semantics / architecture

- [x] Make v1 “no per-signal mode”: remove per-signal direct/immediate and run execution as a drain stage.
- [ ] Align naming/docs around “direct calls” (stop using “immediate” terminology).
- [ ] Decide the long-term split: `Intent` vs `Command/Mutate` vs `Fact`.
- [ ] Move intent execution out of handler dispatch (so handlers can be observers).
- [ ] Replace `beat_now = 0.0` in `ActionSystem` by threading transport context into handlers (likely via a small per-frame resource, or by adding context to `Signal`).
- [ ] Clarify / enforce the v1 rule for fact-ish handlers (“observe-only; emit mutations via signals”).

### Code health / follow-ups

- [ ] Update docs/signals.md and docs/signal-emitter.md to match the new handler signature: `fn(&mut World, &mut dyn SignalEmitter, &Signal)`.
- [ ] Add a small test that asserts ordering at drain points: executor stage runs before any handlers observe.
- [ ] Audit all `queue.flush(...)` callsites and decide whether they should become explicit drain calls (once the facade is gone).
- [ ] Remove/resolve unused `CommandQueue` imports in components (warnings from `cargo test` output).
- [ ] Remove `CommandQueue` entirely (callsite churn pass) and thread a `SignalEmitter`/`RxWorld` instead.

### Inventory / scope control

- [ ] Make a single table of “executed-by-default-executor” variants and keep it in sync with `execute_action_signal`.
- [ ] Decide what to do with non-ECS “queue_*” helpers (XR/render backend queue plumbing) that currently live on `CommandQueue` but aren’t ECS mutations.
- [ ] Provide a non-handler way to obtain an emitter (e.g. `&mut SystemWorld::rx` or a dedicated `SignalSink`) so emitting signals doesn’t require being inside a handler.

---

## Quick file map (entry points)

- Signal definitions: ../../src/engine/ecs/rx/signal.rs
- Signal storage + handler dispatch: ../../src/engine/ecs/rx/rx_world.rs
- Default executor + drain points: ../../src/engine/ecs/system/system_world.rs
- Transitional facade emitting typed mutations: ../../src/engine/ecs/command_queue.rs
- Binding queue → rx and public handler API: ../../src/engine/universe.rs
- Intent/action handling (still partially mutates `World` directly): ../../src/engine/ecs/system/action_system.rs
