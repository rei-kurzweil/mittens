# Signal emission in MMS: unified or separate?

This document explores whether the `emit()` builtin for component expressions should be the
same construct as emitting engine signals (intents, events) — or whether these are
fundamentally different things that happen to share a name.

---

## What `emit()` currently means in MMS

Today `emit(ce)` is the single emission primitive in MMS. It takes a `ComponentExpression`
and produces:

```
SpawnComponentTree { root: ce, parent: Option<ComponentId> }
```

which is an `IntentValue` — sent to the engine's signal pipeline, executed by
`RxIntentExecutor`. So `emit(T { })` is already, internally, **emitting an intent**.

The question is whether this should be surfaced uniformly in the language.

---

## The engine signal model (reference)

The engine has two signal kinds:

| Kind | Type | Semantics |
|------|------|-----------|
| **Intent** | `IntentValue` | A request for a side effect. Executed at drain points. Routable via `AtBeat`. |
| **Event** | `EventSignal` | A fact / observation. Dispatched to scoped handlers. Follow-up events deferred to next tick. |

Intents and events are both wrapped in `Signal { scope: ComponentId, intent, event }`.
The `SignalEmitter` trait exposes:

```rust
fn push_event(scope, EventSignal)
fn push_intent_now(scope, IntentValue)
fn push_intent_at_beat(scope, beat, IntentValue)
```

---

## Option A: `emit()` stays component-only; signals use separate syntax

`emit()` is the component spawning primitive. Emitting intents and events from MMS would
require separate builtins or keywords:

```mms
// component spawning
emit(T { R.cube() { C.rgba(1, 0, 0, 1) } })

// intent (separate builtin)
intent(SetColor { component_ids: [id], rgba: [1, 0, 0, 1] })
// or keyword:
send SetColor { component_ids: [id], rgba: [1, 0, 0, 1] }

// event (separate builtin)
event(Clicked { id: id })
```

**Arguments for:**
- `emit(ce)` has a well-established declarative feel — it means "place this in the world."
  Conflating it with arbitrary intent dispatch muddies that semantics.
- Events and intents are conceptually very different from spawning components. Different
  verbs are clearer: spawn / send / fire / signal.
- MMS is an authoring language first. Direct intent/event emission is an advanced/power-user
  feature that probably doesn't belong in scene files.

**Arguments against:**
- Two mechanisms for "produce a signal" adds conceptual surface area.
- `SpawnComponentTree` is already an intent — the boundary is arbitrary.

---

## Option B: unified `emit()` that accepts any signal value

`emit()` becomes polymorphic over signal types. Component expressions, intents, and events
are all first-class values that can be emitted:

```mms
// component expression → SpawnComponentTree intent
emit(T { R.cube() { C.rgba(1, 0, 0, 1) } })

// intent value (hypothetical MMS intent literal syntax)
emit(SetColor { component_ids: [id], rgba: [1, 0, 0, 1] })

// event value
emit(Event.Clicked { id: id })
```

The `emit()` builtin dispatches on the value type: `ComponentExpression` → wraps in
`SpawnComponentTree`; `IntentValue` → pushes directly; `EventSignal` → pushes as event.

**Arguments for:**
- Conceptually honest: `emit(T { })` was always "emit an intent" under the hood. Surface
  the generality.
- A single production point for all engine communication is a clean interface contract for
  MMS as a scripting language.
- Enables MMS scripts to participate in the reactive pipeline: emit an event that handlers
  elsewhere respond to.

**Arguments against:**
- Requires MMS to have a syntax for `IntentValue` and `EventSignal` literals — a large
  surface area to define and keep in sync with the engine enum.
- The distinction between intents (requests, can be scheduled) and events (observations,
  can't be scheduled) is real and should be preserved rather than flattened under one verb.
- If MMS scripts can emit arbitrary intents, they can do anything the engine can — including
  destructive ops like `RemoveSubtree`. This may or may not be desirable.

---

## Option C: `emit()` for components, `signal()` for the rest; component expressions are not intents

Treat component expressions as a **language-level construct** separate from the signal
system. `emit(ce)` is the language's spawn operator — not "emit a `SpawnComponentTree`
intent" but something more primitive that the runtime interprets. The signal system is an
engine-internal concern.

MMS scripts that need to interact with the signal system use a distinct `signal()` builtin
or a `@intent`/`@event` annotation, but this is explicitly a power-user escape hatch, not
part of the core authoring vocabulary.

```mms
// authoring: component expression placement
T { R.cube() { C.rgba(1, 0, 0, 1) } }

// power-user: direct signal (rare in .mms files; common in .mms handler scripts)
@intent SetColor { component_ids: [id], rgba: [1, 0, 0, 1] }
```

**Arguments for:**
- Cleanest separation of concerns. Scene authoring files don't need to know about
  `IntentValue`. Handler scripts (future) can opt into the signal vocabulary.
- `@intent` / `@event` markers make it visually clear this is engine-level plumbing.

**Arguments against:**
- Introduces a token or sigil (`@`) not currently in the lexer.
- "Component expressions are not intents" is a white lie — they compile to one.

---

## Key conceptual distinction: component expressions vs signal emission

These are related but not the same thing:

| | Component expression | Signal emission |
|---|---|---|
| **Source** | Declarative tree literal in source | Named intent/event value |
| **Author model** | "Describe what should exist in the world" | "Request a change / report a fact" |
| **Timing** | Executes during body evaluation | Intents can be `AtBeat`-scheduled; events are tick-deferred |
| **Receiver** | `SpawnComponentTree` executor | Any handler registered for that signal kind |
| **Scope** | Emit context stack (parent component) | `scope: ComponentId` |
| **Reversibility** | Spawns new components; undone by `RemoveSubtree` | Intent may be mutation, event is observation |

The authoring model of MMS (`T { R.cube() { ... } }`) maps cleanly onto the component
expression column. If MMS ever has reactive handler scripts (`on Clicked { ... }`), those
would map onto the signal emission column. These are different modes of the language.

---

## The `emit()` name conflict

If `emit()` means both "spawn a component tree" and "emit a signal", authors in a
reactive script context face ambiguity:

```mms
// inside an `on Clicked { }` handler:
emit(T { })        // spawn a new cube? or emit a T-typed event?
emit(Clicked { })  // emit a Clicked event? or spawn a component named Clicked?
```

This is resolvable with a type system, but adds friction in the current untyped v1 context.
Keeping `emit(ce)` strictly for `ComponentExpression` avoids this entirely.

---

## Recommendation (for v1/v2 planning)

| Decision | Rationale |
|----------|-----------|
| Keep `emit()` for `ComponentExpression` only | Preserves the established semantics; avoids the naming conflict above. |
| Introduce `intent(...)` builtin separately when scripted mutation is needed | Direct counterpart to `push_intent_now`. Clearly labeled. |
| Introduce `event(...)` or `fire(...)` builtin for reactive handler scripts | A future concern; MMS doesn't have handler scripts yet. |
| Do NOT unify under a single `emit()` | The component-expression authoring model and the signal-dispatch model are separate vocabulary. Mixing them makes scene files harder to read. |

The cleanest future state is probably:

```mms
// scene authoring layer — the common case
T { R.cube() { C.rgba(1, 0, 0, 1) } }   // spawn

// scripted mutation layer — power-user
intent(SetColor { component_ids: [cube_id], rgba: [1, 0, 0, 1] })

// reactive handler layer — future
on Event.Clicked(id) {
    fire(Event.PlaySound { id: id })
}
```

Three distinct vocabulary items (`emit`/bare-CE, `intent`, `fire`/`event`) for three
distinct roles — each clear at a glance.

---

## Open questions

- Should `intent()` in MMS support `AtBeat` scheduling? (`intent.at(beat, ...)`)
- Should `ComponentExpression` bare-statement syntax (`T { }`) remain the only way to
  spawn, or should `emit()` stay as an explicit alias?
- When MMS handler scripts arrive, should they share the `.mms` extension or use `.mmsh`
  (handler) to signal the different vocabulary?
- Is there a future where `ComponentExpression` and `IntentValue` unify from the other
  direction — i.e., all intents become first-class MMS values with a consistent literal
  syntax, and the language is just "intent expressions all the way down"?
