# Draft: event signal pipelines

## Status

Draft only. This document describes a likely next step for the engine signal layer.

It does **not** change the current runtime semantics in `docs/how_to/guide/signals.md`.
It proposes an additional routing/projection layer for **events**, analogous to the existing
intent pipeline.

Near-term recommendation:

- do **not** add new signal primitives or pipeline stages just to make scrolling work
- use a normal upstream handler on the discovered drag scope
- map the upstream `DragMove` into `EventSignal::Scrolling`
- emit that semantic event scoped to the `ScrollingComponent`

This draft is about how that pattern could later become a first-class event-routing layer.

---

## 1. Problem statement

The engine already has:

- **event dispatch** with ancestor bubbling
- **intent pipelines** that can rewrite intent recipients before execution

That is enough for many cases, but it leaves a gap for components like `ScrollingComponent`:

- the raw interaction event (`DragMove`) happens on some **ancestor drag surface**
- the component that wants to expose the semantic event (`Scrolling`) lives **below** that surface
- downstream consumers should listen to the **scrolling component**, not to the drag plane or
  clip/background renderable

Today this is solved manually:

1. `ScrollingComponent` (via `ScrollSystem`) discovers an ancestor drag scope
2. it registers a handler there
3. when that handler runs, it maps `DragMove` into `EventSignal::Scrolling`
4. it emits that new event scoped to the `ScrollingComponent`

That shape is correct, but the engine lacks a first-class concept for it.

The missing abstraction is not “reverse bubbling” or a second scope-propagation mode.
It is **event routing / projection**.

---

## 2. Current runtime semantics remain unchanged

This proposal intentionally keeps the current event model intact.

For the canonical runtime semantics, see the [MMS signal guide](../how_to/guide/signals.md).
The key point for this draft is only:

- current scoped dispatch is ancestor-bubbling only
- scrolling does not require a second propagation direction
- scrolling only needs upstream subscription plus local semantic re-emission

---

## 3. Concept: event signal pipeline

An **event signal pipeline** is a chain that:

1. subscribes to one or more upstream event kinds at some scope
2. optionally filters or maps the payload
3. emits a new event or intent on a chosen target scope

This is parallel to the existing intent pipeline, but applies to the **event lane**.

### 3.1 Mental model

Intent pipeline:

- “before executing an intent, rewrite who it targets”

Event pipeline:

- “when an upstream event is observed, transform it into another event/intent and re-emit it”

### 3.2 First target use case

`ScrollingComponent` is the motivating use case.

Desired behavior:

- discover the nearest drag surface ancestor
- subscribe to `DragMove` on that scope
- map drag delta into scroll state changes
- emit `EventSignal::Scrolling` scoped to the scrolling component

Then any other system can listen on the `ScrollingComponent` scope and treat scrolling
as a component-local semantic stream.

---

## 4. Proposed Rust-side API shape

The minimal useful Rust-side API should feel like this:

```rust
add_event_signal_handler(SignalKind::DragMove, drag_scope, handler)
    .map_event(|world, env| -> Option<EventSignal> {
        // inspect DragMove, update state if needed
        Some(EventSignal::Scrolling { ... })
    })
    .route_to(scrolling_component);
```

That exact builder syntax is only illustrative. The key capabilities are:

- subscribe to an upstream event kind
- choose a source scope
- run a closure that can inspect world state and produce:
  - no output
  - a new `EventSignal`
  - possibly a new `IntentSignal`
- choose the destination scope for the emitted signal

A more explicit non-builder form could be:

```rust
rx.add_event_projection(
    EventProjection {
        source_kind: SignalKind::DragMove,
        source_scope: drag_scope,
        destination_scope: scrolling_component,
        map: Box::new(|world, env| {
            // returns zero or more signals
        }),
    }
);
```

### 4.1 What the mapper must receive

The mapper likely needs:

- `&mut World`
- `&Signal` for the upstream event
- a `SignalEmitter` or return value describing new signals

Two plausible designs:

#### Option A — mapper returns signals

```rust
FnMut(&mut World, &Signal) -> Vec<Signal>
```

Pros:

- pure-ish and easy to reason about
- explicit emitted scope and payload

Cons:

- less ergonomic for multi-step logic
- awkward if code naturally wants `emit.push_event(...)`

#### Option B — mapper writes via emitter

```rust
FnMut(&mut World, &mut dyn SignalEmitter, &Signal)
```

Pros:

- mirrors current handler API
- easy to reuse existing handler logic

Cons:

- less explicit about what gets emitted
- more imperative

For consistency with current handlers, Option B is the most practical first step.

---

## 5. First implementation: no new primitives required

The first useful implementation should stay small.

For scrolling, we already have the right ingredients:

1. discover the ancestor drag scope
2. register a normal `DragMove` handler there
3. update scroll state in that handler
4. emit `EventSignal::Scrolling` scoped to the scrolling component

So the first production shape is just:

- upstream handler registration
- local event mapping
- explicit local re-emission

No new event-pipeline runtime stage is required to ship that.

## 6. Event pipeline processor

If the intent lane has `SignalPipelineProcessor`, the event lane should likely get a peer:

- `EventSignalPipelineProcessor`
- or a submodule of `SignalPipelineProcessor`

### 6.1 Placement in runtime flow

Current flow at a drain point:

1. ready events are drained
2. event handlers are dispatched by bubbling through the scope chain
3. handlers may emit follow-up signals

Proposed event-pipeline flow:

1. ready events are drained
2. event projections/pipelines are applied
3. resulting projected events/intents are enqueued
4. normal scoped handlers are dispatched for the original event
5. projected events are observed at later normal dispatch points according to existing rules

This keeps projections as a distinct stage from ordinary observation.

### 6.2 Alternative: projections as handlers

A smaller first implementation is to model event projections as a structured kind of handler
rather than a separate stage.

That is effectively what scrolling does today:

- handler installed on ancestor scope
- handler emits a new event on a different scope

Pros:

- minimal runtime change
- leverages existing handler machinery
- easiest path to production use

Cons:

- less explicit than a first-class event pipeline stage
- harder to inspect/debug as routing rather than arbitrary code

### 6.3 Recommended near-term approach

Near term:

- keep using handlers + explicit re-emission for real features like scrolling
- document this as the current recommended pattern
- design the event pipeline API to formalize that pattern later

That lets the engine gain the semantics now without waiting for a generalized pipeline stage.

---

## 7. Relationship to current intent pipelines

The current `SignalRouteUpward` pipeline only rewrites **intent recipient component ids**.
It does not:

- intercept `EventSignal`
- re-scope events
- transform event payloads
- emit new events from upstream ones

So the proposed event pipeline is complementary, not a reuse of the existing routing op.

### 7.1 Potential future symmetry

Long term, the signal layer could be described as having two pipeline families:

- **intent pipelines** — rewrite request recipients before execution
- **event pipelines** — project/route observations before handler dispatch or as structured handlers

The two lanes stay separate because intents and events have different semantics.

---

## 8. Scrolling as the reference design

Scrolling should be the reference example for this feature.

### 7.1 Desired authored contract

Author writes:

```mms
Scrolling.new(viewport_height, content_height) {
    ... content ...
}
```

The authored component should not need to know:

- which ancestor renderable captures drag
- whether the viewport is a layout-generated background quad or a manual renderable
- how drag deltas are mapped into `scroll_offset`

`ScrollingComponent` should own that.

### 7.2 Runtime behavior

On init / registration:

1. discover scroll track
2. discover drag-capture ancestor
3. subscribe to upstream drag events
4. update internal scroll state from those drag events
5. emit `EventSignal::Scrolling` scoped to the scrolling component

### 7.3 Downstream contract

Other systems may then subscribe to `SignalKind::Scrolling` on the scrolling component.

Examples:

- layout may react to scroll position changes
- widgets may show a scrollbar thumb
- diagnostics may observe scroll activity

They do not need to subscribe to the drag plane or background renderable directly.

---

## 9. Proposed event type shape

A dedicated scrolling event should carry semantic scroll information rather than raw drag only.

Example shape:

```rust
EventSignal::Scrolling {
    scroll_component: ComponentId,
    drag_scope: ComponentId,
    delta_world: [f32; 3],
    scroll_offset: f32,
    max_scroll: f32,
    viewport_height: f32,
    content_height: f32,
}
```

This is intentionally not just `DragMove` renamed.
It is a semantic event for the scrolling domain.

---

## 10. MMS imperative API direction

This document only sketches the MMS side briefly. The author-facing signal syntax and
event-emission vocabulary are covered in more detail by:

- [docs/meow_meow/analysis/event-handlers.md](../meow_meow/analysis/event-handlers.md)
- [docs/meow_meow/analysis/signal-emission-in-mms.md](../meow_meow/analysis/signal-emission-in-mms.md)
- [docs/meow_meow/draft/event-signal-pipelines.md](../meow_meow/draft/event-signal-pipelines.md)

For the first phase, the event projection concept only needs to work with:

- Rust API
- MMS imperative/reactive API

A plausible MMS-side imperative shape is:

```mms
on drag_surface.DragMove(dm) {
    emit scrolling.Scrolling {
        delta_world: dm.delta_world
        scroll_offset: compute_scroll(...)
    }
}
```

But that still exposes too much of the drag surface.

A better imperative abstraction would look closer to:

```mms
scrolling.listen(DragMove, drag_surface)
scrolling.map_signal(fn(dm) { Scrolling { ... } })
scrolling.emit_to_self()
```

This is not proposed as final syntax. The important semantic point is:

- the component owning the behavior should be able to subscribe upstream
- and emit its own semantic event locally

---

## 11. Declarative component-based event pipelines

This remains an open question.

A future declarative form might resemble the existing pipeline-operator pattern:

```mms
Scrolling {
    EventPipeline {
        EventFromAncestor(kind="DragMove", ancestor="renderable")
        EventMap(fn(dm) { Scrolling { ... } })
        EventRouteSelf {}
    }
}
```

or Rust-side component operators attached in topology.

This may be valuable eventually, but it is **not required** for the first useful version.

Recommendation:

- support the Rust API first
- support imperative MMS/reactive API second
- leave declarative component-based event-pipeline operators as an open design question

---

## 12. How well this fits the current engine

This proposal fits the current architecture well.

### 11.1 What already exists

- clear event vs intent lanes
- explicit drain points
- scoped handler registration
- handlers can already emit follow-up events/intents
- intent pipeline machinery establishes the precedent for a second routing stage

### 11.2 What is still missing

- a formal event projection API
- an event-pipeline processor or equivalent structured handler layer
- ergonomic author-facing MMS syntax for signal mapping/routing

### 11.3 Why scrolling can proceed now

Scrolling does not need the generalized event pipeline to exist first.

It can use the current pattern:

- subscribe upstream
- map in handler code
- emit semantic event locally

That means the engine can adopt the right semantics immediately while the generalized
signal-pipeline design remains a draft.

---

## 13. Recommended next steps

1. Keep `docs/how_to/guide/signals.md` focused on current runtime semantics.
2. Document event projection/routing separately as a draft.
3. Treat scrolling as the first production example of this pattern.
4. Later decide whether to formalize it as:
   - structured event projections built on handlers, or
   - a first-class `EventSignalPipelineProcessor` stage.
5. Add an MMS-side companion spec that explains the imperative authoring model.
