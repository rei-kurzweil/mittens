# Draft: MMS event signal pipelines

Companion to `docs/draft/event-signal-pipelines.md`.

This document describes how event projection/routing could surface in MMS.
It is intentionally scoped to:

- Rust API
- MMS imperative/reactive API

It leaves declarative component-operator syntax as an open question.

This draft deliberately does **not** re-specify general MMS handler syntax or the broader
`emit()` / `fire()` / `intent()` vocabulary debate. Those already live in:

- [docs/meow_meow/analysis/event-handlers.md](../analysis/event-handlers.md)
- [docs/meow_meow/analysis/signal-emission-in-mms.md](../analysis/signal-emission-in-mms.md)

This document is only about the additional case where a component listens upstream and
re-emits a semantic event on itself.

---

## 1. Goal

MMS needs a way for a component or script to:

1. listen to an upstream event that may happen on some other scope
2. transform that event into a more semantic event
3. emit the semantic event on the component that owns the behavior

The first motivating use case is scrolling:

- raw input event: `DragMove`
- raw event source: ancestor drag surface / background / clip renderable
- semantic owner: `ScrollingComponent`
- semantic output event: `Scrolling`

The author should not need to manually wire the drag plane every time.

---

## 2. What MMS should not require

MMS should not require authors to think in terms of:

- reverse scope propagation
- listening on descendants for ancestor-scoped events
- the exact drag-plane topology under every scrollable widget

Instead, authored components should be able to say, in effect:

- find the right upstream event source
- listen there
- map to my semantic event
- emit on me

---

## 3. Immediate recommendation

For the first working version, MMS does not need a new event-pipeline primitive.

The recommended shape is simply:

1. register a normal handler on the upstream event source
2. map that event into a semantic event
3. emit the semantic event scoped to the owning component

In other words, for scrolling-like behavior, start with:

- ordinary upstream handler registration
- ordinary local event emission

and treat generalized event pipelines as a later formalization.

---

## 4. Imperative MMS model

The first useful MMS-facing model should be imperative/reactive rather than declarative.

### 4.1 Desired authoring feel

The desired shape is roughly:

```mms
scrolling.listen(DragMove, drag_surface_ancestor)
scrolling.map_signal(fn(dm) {
    Scrolling {
        delta_world: dm.delta_world
        scroll_offset: ...
        max_scroll: ...
    }
})
scrolling.emit_to_self()
```

This is not final syntax. It expresses the semantics only.

### 4.2 Meaning of each step

- `listen(...)`
  - subscribe to a specific upstream event kind on a discovered or explicit source
- `map_signal(...)`
  - transform the upstream payload into a new event value
- `emit_to_self()`
  - scope the new event to the owning component

This is effectively an event projection pipeline.

---

## 5. Scrolling example

### 5.1 Semantic intent

Author writes a scrollable component or helper.

They want downstream code to listen to:

```mms
on scrolling.Scrolling(ev) {
    // update a scrollbar thumb, virtual list window, etc
}
```

They do **not** want downstream code to listen to:

```mms
on drag_plane.DragMove(dm) { ... }
```

because that leaks implementation detail.

### 5.2 Desired imperative implementation idea

```mms
fn attach_scrolling(scrolling, drag_source) {
    on drag_source.DragMove(dm) {
        let next = scrolling.project_drag(dm)
        fire_scoped(scrolling, Scrolling {
            delta_world: dm.delta_world
            scroll_offset: next.offset
            max_scroll: next.max_scroll
            viewport_height: next.viewport_height
            content_height: next.content_height
        })
    }
}
```

This captures the real requirement:

- upstream subscription
- local projection
- explicit local re-emission

---

## 6. Relationship to Rust API

The Rust API should come first.

MMS should map onto the same underlying concepts rather than inventing a separate runtime model.

### 6.1 Rust-first design rule

If Rust gets an event projection API like:

```rust
add_event_projection(source_scope, SignalKind::DragMove, ...)
```

then MMS should compile to that.

If Rust instead keeps using:

- normal handler registration on upstream scope
- explicit `emit.push_event(self_scope, ...)`

then MMS can compile to that as its first implementation.

### 6.2 What MMS does not need yet

MMS does not need a first-class declarative event-pipeline component syntax yet.

The first working target can simply be:

- imperative handler registration
- explicit signal mapping
- explicit scoped event emission

---

## 7. Event emission vocabulary

This draft assumes MMS should be able to emit events explicitly in handler/reactive code,
but it defers the actual vocabulary choice to
[docs/meow_meow/analysis/signal-emission-in-mms.md](../analysis/signal-emission-in-mms.md).

The only constraint this draft adds is:

- whatever the final verb is, it should support explicit scoped re-emission of a semantic
    event on the owning component

---

## 8. Open syntax questions

These are intentionally unresolved.

### 8.1 Explicit source binding

Should upstream listening look like:

```mms
on drag_surface.DragMove(dm) { ... }
```

or more pipeline-like:

```mms
drag_surface.DragMove -> fn(dm) { ... }
```

Both are plausible.

### 8.2 Explicit self-scoping

Should local semantic re-emission be:

```mms
fire_scoped(self, Scrolling { ... })
```

or implicit inside a handler owned by the component:

```mms
fire(Scrolling { ... })
```

Both are plausible. Explicit scope is easier to reason about at first.

### 8.3 Discovery of upstream source

For scrolling-like helpers, should the source be:

- explicit in MMS
- inferred by the runtime
- inferred unless overridden

Recommendation:

- runtime inference for built-in behavior components like `Scrolling`
- explicit source binding for general-purpose imperative scripts

---

## 9. Declarative event pipeline components

Open question only.

A future component-based syntax might look like:

```mms
Scrolling {
    EventPipeline {
        EventFromAncestor(kind="DragMove", ancestor="renderable")
        EventMap(fn(dm) { Scrolling { ... } })
        EventRouteSelf {}
    }
}
```

or possibly a more signal-operator-shaped syntax.

This may be useful later for authoring reusable event bridges declaratively, but it is
not required to make the model work.

Recommendation:

- do not standardize declarative event-pipeline components yet
- leave this as a future extension once the imperative/runtime model proves itself

---

## 10. Recommended MMS-facing design direction

### Near term

- let components like `Scrolling` own upstream subscription internally
- let them re-emit semantic events scoped to themselves
- let MMS listen to those semantic events normally

### After that

- expose imperative event projection in MMS handlers/scripts
- keep signal emission vocabulary explicit and separate from component spawning

### Later

- consider declarative component/operator syntax if the imperative model proves stable

---

## 11. Minimal success criterion

This draft is successful if MMS authors can eventually write code that feels like:

```mms
let scrolling = Scrolling.new(1.0, 100.0) { ... }

on scrolling.Scrolling(ev) {
    scrollbar.thumb_offset = ev.scroll_offset / ev.max_scroll
}
```

without needing to know what drag surface the scrolling component used internally.

That is the core semantic goal.
