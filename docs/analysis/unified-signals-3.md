# Unified Signals v3 — SignalClass + two executors (Intent → Mutation)

Date: 2026-03-05

This document proposes a cleaner successor to the current “everything side-effectful is `SignalKind::Action`” model.

Core idea:

- Every signal gets a **class tag**: `Intent`, `Mutation`, or `Event`.
- Only **Intent** signals carry scheduling information.
- Drain points run **two built-in executors**:
  1) an **Intent executor** that resolves intent (and scheduling) into mutation signals
  2) a **Mutation executor** that applies engine-defined mutations into `World` / `VisualWorld`
- `Event` signals are facts/observations and have no built-in executor.

This splits “what is this message?” from “who should observe it?”, and removes the overloading currently done by `SignalKind::Action`.

---

## 1) Motivation (what feels broken today)

Today, a huge set of `SignalValue` variants map to `SignalKind::Action`, but they don’t all want the same treatment:

- “Intent” values (e.g. set text/color, attach/detach) want interpretation and may schedule follow-up work.
- “Mutation” values (e.g. register/update/remove renderables/transforms/text) want the built-in executor to apply canonical engine operations.
- “Event/fact” values (ray hits, collisions, drags) want to be observed, not executed.

When these are all labeled `Action`, we end up with:

- executor gating: `if kind == Action { execute_action_signal(...) }`
- handler gating: `ActionSystem` installed as a global `Action` handler, then ignores most `Action` values

This works, but it is conceptually noisy and makes it hard to see where timing belongs.

---

## 2) Proposal: `SignalClass`

Make the envelope carry a simple class tag:

```rust
enum SignalClass {
    Intent,
    Mutation,
    Event,
}

struct Signal {
    scope: ComponentId,
    value: SignalValue,
    class: SignalClass,

    // Only meaningful for Intent (see below)
    when: SignalWhen,
}
```

Naming note:

- You suggested `signal_class` and also floated `SignalLayer` / `Signal::layer`.
- In this doc, “class” means “what semantic layer does it belong to?”, so `SignalClass` and `SignalLayer` are basically interchangeable.

Recommendation: use `SignalClass` for clarity (“tag attached to a message”).

---

## 3) Scheduling: Intent-only

Rule: **only Intent signals should have scheduling information**.

This is a strong design constraint with good consequences:

- Mutations are “apply this at the next drain point” and should not sit around in a timed holding pen.
- Events are facts about something that happened; scheduling them doesn’t make semantic sense.

### 3.1 Shape of `when`

There are two clean options:

**Option A (simple, keep `SignalWhen` on `Signal`)**

- Keep `Signal.when: SignalWhen` for all signals.
- Enforce: if `class != Intent`, then `when` must be `Now`.
  - Debug assertion in constructors.
  - Avoids `Option<SignalWhen>`.

**Option B (type-level clarity)**

- Move scheduling into a dedicated struct for intent signals:

```rust
struct IntentEnvelope {
    when: SignalWhen,
}

struct Signal {
    scope: ComponentId,
    value: SignalValue,
    class: SignalClass,
    intent: Option<IntentEnvelope>,
}
```

This makes the “Intent-only scheduling” rule impossible to violate accidentally.

---

## 4) Two built-in executors

Replace the single “execute action” stage with two executors.

### 4.1 `RxIntentExecutor`

Responsibilities:

- Owns the timed holding-pen behavior for intents:
  - promote due intents into the per-frame queue
- Interprets intent signals and emits canonical mutation signals
- Does **not** directly mutate `World` (ideally)
  - Its output is more signals (mutations)

Implementation shape:

```rust
struct RxIntentExecutor;

impl RxIntentExecutor {
    fn execute(
        &mut self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        env: &Signal,
        // (optional) transport context if needed
    ) {
        match &env.value {
            // e.g. SetText, SetColor, Attach, Detach, RemoveSubtree, ...
            _ => {}
        }
    }
}
```

### 4.2 `RxMutationExecutor`

Responsibilities:

- Applies canonical engine-defined mutations to `World` and/or `VisualWorld`
- Replaces the current “former command queue” execution path (`execute_action_signal`)

Implementation shape:

```rust
struct RxMutationExecutor;

impl RxMutationExecutor {
    fn execute(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        emit: &mut dyn SignalEmitter,
        env: &Signal,
    ) {
        match &env.value {
            // e.g. RegisterRenderable, UpdateTransform, RegisterText, ...
            _ => {}
        }
    }
}
```

### 4.3 Events

`Event` class signals have no built-in executor.

They are dispatched to handlers as facts.

---

## 5) The holding pen (timed queue)

Yes: under this model, the holding pen only applies to the **Intent** layer.

Practical outcome:

- `RxWorld.pending` becomes “pending intents”.
- Promotion step only scans intent signals.
- Mutation signals are always `Now` (per the rule) and go directly into the per-frame queue.

This also answers a subtle question: if an intent is scheduled “at beat X”, what actually happens at beat X?

- At beat X, the intent becomes runnable.
- Running the intent executor emits a set of mutation signals that are applied at that same drain point.

So the timeline becomes predictable:

- schedule intent at beat X
- at beat X: intent executes → emits mutation signals → mutation executor applies them

---

## 6) Drain point algorithm (proposed)

At each drain point:

1) Drain any facade-emitted signals into `RxWorld` (if a facade like `CommandQueue` still exists).
2) Promote due **Intent** signals from the holding pen into the runnable queue.
3) Loop through runnable signals (cursor-based):
   - If `env.class == Intent`:
     - run `RxIntentExecutor::execute(...)`
     - (optional) dispatch handlers for the intent as observers
   - If `env.class == Mutation`:
     - run `RxMutationExecutor::execute(...)`
     - (optional) dispatch handlers for the mutation as observers
   - If `env.class == Event`:
     - no built-in executor
   - Dispatch handlers (scoped + global) for observers
4) If executors/handlers emitted more signals, keep draining until stable or max-signal cap is hit.

Open choice: do we dispatch handlers once per signal, always after executor(s)?

Recommendation:

- Keep the existing invariant: **executors first, handlers observe after**.
- This keeps handler code from “racing” engine-defined caches.

---

## 7) Relationship to `SignalKind`

Under this proposal, `SignalKind` should no longer be used as an overloaded routing key for executors.

We have a few options:

### 7.1 Keep `SignalKind` only for events

- Keep `SignalKind` variants for domain events (ParentChanged, collisions, ray hits, drags, …).
- Make `SignalKind::Any` remain a handler wildcard.
- Do not use `SignalKind` for `Intent`/`Mutation` routing.

### 7.2 Replace `SignalKind` with `SignalTopic`

Alternatively, rename and reshape:

- `SignalTopic` is purely for handler routing.
- `SignalClass` is purely for executor routing + “timing allowed?” rules.

This makes the semantics explicit:

- topic: who observes
- class: how it is processed

---

## 8) Where does `class` come from?

We should avoid letting callsites construct inconsistent signals.

Preferred pattern: constructors that set class + scheduling invariants:

```rust
impl Signal {
    fn intent(scope: ComponentId, when: SignalWhen, value: SignalValue) -> Self { ... }
    fn mutation(scope: ComponentId, value: SignalValue) -> Self { ... /* when = Now */ }
    fn event(scope: ComponentId, value: SignalValue) -> Self { ... /* when = Now */ }
}
```

This also makes it easy to grep for “who is emitting intent vs mutation”.

---

## 9) Migration sketch (high-level)

1) Introduce `SignalClass` and constructors.
2) Decide what stays as “intent” (producer-facing) vs “mutation” (canonical engine mutation).
3) Move existing intent behavior out of “Action handler” and into `RxIntentExecutor`.
   - This is where we should fix transport usage (today’s `beat_now = 0.0` wart disappears).
4) Rename / replace `execute_action_signal` with `RxMutationExecutor`.
5) Update drain points to:
   - promote due intents
   - execute intent
   - execute mutation
   - then dispatch observers
6) Delete `SignalKind::Action` (and any “ActionSystem” concept) once no longer needed.

Note: this repo has an explicit policy of not keeping compatibility aliases for schema changes; that’s good here, because `Signal`/routing changes are much easier to reason about when we do them in one clean step.

---

## 10) Open questions / design choices to settle

- Should intent handlers exist at all, or should intent execution *only* happen in the intent executor?
  - Recommendation: keep intent execution in the executor; handlers observe.
- Should some “currently-mutation” signals be allowed to be timed? (e.g. audio scheduling)
  - Recommendation: represent those as **intent** signals that produce mutation + schedule ops.
- Do we need multiple mutation executors (graphics vs physics vs audio), or one big match?
  - Start with one executor, split only if it becomes unmaintainable.
