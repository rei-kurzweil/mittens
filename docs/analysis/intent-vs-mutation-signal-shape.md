# Intent vs mutation: enum split + Signal shape sketch

Date: 2026-03-07

This document is a forward-looking sketch only. It does **not** propose changing code immediately.

## Current state (today)

- We conceptually have three categories of signals:
  - **Events**: facts/observations, dispatched to handlers.
  - **Intents**: requests for side effects, executed at drain points.
  - **Mutations** (a.k.a. “low-level intents”): the canonical engine operations that actually mutate state (register/remove/update, etc.).

- In code, both “intent interpretation” and “low-level mutation execution” are currently encoded in a single enum (`IntentValue`) and then routed at drain-time to one of:
  - `RxIntentExecutor` (high-level interpretation: expands one intent into follow-up work)
  - `RxMutationExecutor` (low-level canonical operations)

This is good enough for the refactor, but the single enum mixes two layers.

## Why split “intent” vs “mutation”?

Splitting aims to make these invariants more explicit:

- **Intents are API / semantic requests** ("attach this", "set color", "schedule note").
- **Mutations are implementation details** ("register transform", "remove subtree", "update transform").

Benefits:

- Makes it harder to accidentally emit an internal mutation from user-facing code.
- Clarifies executor responsibilities and reduces the `handles_value(...)` routing heuristic.
- Allows different policy knobs (e.g. permissions, validation, logging, replay) at the intent vs mutation boundary.

Costs:

- Requires a migration pass across emit sites.
- Requires codec/story for serialization if either layer is saved/loaded.

## Design options

### Option A: keep `Signal` shape, split enums

Keep `Signal` having `event: Option<_>` and `intent: Option<_>` (or similar), but inside the intent payload split the value type:

- `IntentValue`: semantic/high-level
- `MutationValue`: low-level/implementation

Drain logic becomes:

1. Handlers run on events.
2. Drain ready **intents** and pass to `RxIntentExecutor`.
3. Drain ready **mutations** and pass to `RxMutationExecutor`.

This requires the rx queue to store two timed queues (or one queue with a tagged union).

Pros:

- Preserves the conceptual “execute at drain points” model.
- Keeps `Signal` relatively small.

Cons:

- You still need to decide where “mutation signals” live (see next section).

### Option B: add a separate `mutation` field to `Signal`

Make `Signal` structurally represent the three types:

```rust
pub struct Signal {
    pub scope_root: ComponentId,
    pub event: Option<EventSignal>,
    pub intent: Option<IntentSignal>,
    pub mutation: Option<MutationSignal>,
    pub at_beat: Option<f64>,
}

pub struct IntentSignal {
    pub value: IntentValue,
}

pub struct MutationSignal {
    pub value: MutationValue,
}
```

Pros:

- The kind of work is visible from the struct (no overloading of `intent`).
- Makes “what can be scheduled?” very explicit: events generally not scheduled; intents/mutations can be.

Cons:

- More shapes to plumb through `CommandQueue`, `RxWorld`, and codecs.
- Increases the number of “one-of” fields in `Signal`.

### Option C: use a single tagged `kind` + payload field

Instead of multiple optional fields, make one discriminated union:

```rust
pub enum SignalKind {
    Event(EventSignal),
    Intent(IntentValue),
    Mutation(MutationValue),
}

pub struct Signal {
    pub scope_root: ComponentId,
    pub kind: SignalKind,
    pub at_beat: Option<f64>,
}
```

Pros:

- Hard guarantee: a signal is exactly one kind.
- Much less “invalid state space” (no `event=None, intent=None, mutation=None`).

Cons:

- Requires refactoring call sites that currently build `Signal { event: ..., intent: ... }` style payloads.
- Might be noisier for ergonomics unless we add constructors.

## Scheduling semantics

Regardless of which executor handles it, the scheduling model can remain:

- **Timed scheduling** is a property of the *envelope* (the `Signal` having `at_beat` / timed metadata), not the value type.
- Therefore:
  - Intents can be scheduled.
  - Mutations can be scheduled.
  - Events are usually not scheduled (they’re observations), but nothing in principle prevents an “event replay” mechanism.

A split makes it easier to enforce policy like:

- "Only intents/mutations may be timed"
- "Events may be queued but not timed"

## Executor boundary

If we split the types, the clean conceptual boundary is:

- `RxIntentExecutor`: `IntentValue -> emits MutationValue (and sometimes follow-up IntentValue)`
- `RxMutationExecutor`: applies `MutationValue` to `World` + `VisualWorld` + `SystemWorld`

This supports a layered approach:

- L2 intent expands into multiple L1 intents.
- L1 intent expands into canonical mutations.

## Migration sketch (when we decide to do it)

1. Introduce `MutationValue` enum and move obvious low-level variants into it.
2. Teach `CommandQueue` / `RxWorld` to queue two kinds of executable work (intent + mutation) OR one tagged union.
3. Update `RxIntentExecutor` to emit mutations, not low-level intent values.
4. Update emit sites:
   - User-facing code emits `IntentValue`.
   - Engine internals may emit `MutationValue` directly only in tightly controlled places.
5. Update serialization boundaries (if any) to explicitly include/exclude mutations.

## Open questions

- Should replay/recording log **intents**, **mutations**, or both?
- Do we want to guarantee that intent interpretation is *pure* (no world mutation, only emits follow-ups), or allow limited mutation for convenience?
- Which values are “API surface” vs “internal” (e.g. `RemoveSubtree` feels semantic but also very low-level)?
