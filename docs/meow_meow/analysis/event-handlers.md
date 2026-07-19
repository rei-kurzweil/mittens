# (ﾉ◕ヮ◕)ﾉ*:･ﾟ✧ MMS Event Handlers and Signal Wiring

Design analysis for reactive MMS — handlers that respond to engine events and signal
operators for wiring components together.

Future phase concern (depends on Phases 4, 5, 6). This is a design sketch, not a finalized
spec. The goal is to explore what the syntax *should feel like* before committing to any AST.

---

## Background: the engine signal model

(see `docs/how_to/guide/signals.md` for canonical detail)

The engine has two signal kinds:

| Kind | Type | Semantics |
|------|------|-----------|
| **Event** | `EventSignal` | An observation / fact. Dispatched to scoped handlers. Follow-up events deferred to next tick. |
| **Intent** | `IntentValue` | A request for a side effect. Executed at drain points. `AtBeat`-schedulable. |

Events include: `Clicked`, `DragStart`, `DragMove`, `DragEnd`, `ParentChanged`,
`CollisionStarted`, `CollisionEnded`, `RayIntersected`, `ValueChanged`, ...

Intents include: `UpdateTransform`, `SetColor`, `RemoveSubtree`, `RegisterRenderable`, ...

Today, MMS only produces one kind of intent — `SpawnComponentTree`. It has no handler
registration and no reactive wiring at all. Everything here is forward design.

---

## ✦ Core idea: handlers as component body items

The most natural place to declare a handler in MMS is **inside the component that emits
the event** — as a body item in the component expression.

Two equivalent forms:

### Call-style (preferred for named functions)

```mms
R {
    CUBE
    C.rgba(0.3, 0.6, 1, 1)
    Raycastable
    GestureStart(handle_button_press)
}
```

`GestureStart(handle_button_press)` is a `ComponentBodyItem::Call` — a builder call on
the enclosing component. `handle_button_press` is a function reference (a
`Value::Function` looked up from the env). The registry dispatches `GestureStart` as a
handler registration method.

This is the preferred form when the handler is a named top-level function defined elsewhere.

### Property-style (preferred for inline lambdas)

```mms
R {
    CUBE
    C.rgba(0.3, 0.6, 1, 1)
    Raycastable
    on_gesture_start = fn(e) { ... }
}
```

`on_gesture_start = expr` is a `ComponentBodyItem::NamedAssignment`. The registry
recognises `on_*` names as handler registration slots rather than field setters.
Preferred for short inline handlers that don't need a name.

Both forms register the same scoped handler. The component owns the handler registration
— when the component is removed, the handler is removed automatically.

**Why body-level handlers are better than a separate top-level `on` statement:**

- Handler is colocated with the component that emits it — impossible to orphan
- No Phase 6 (live `ComponentId`) needed at the call site; the component registers during
  `init()` using its own scope ID
- Survives hot-reload naturally: replacing the component replaces its handlers
- Works with the `fn name() {}` top-level function syntax cleanly — define the fn once,
  pass it by name to multiple components if needed

Handler functions are closures — they capture any `ComponentObject`s visible in the
enclosing scope at evaluation time (Phase 4 prerequisite). See
`component-addressing.md` for the capture ordering requirement when the handler needs to
reference other `ComponentObject`s by subscript.

---

## ✦ Signal operators: `->` and `<-`

For wiring components together without writing an explicit handler function, a signal
operator can express "when this emits, drive that":

```mms
slider.value -> light.intensity
```

`->` routes the output of the left side into the input of the right side. `<-` is the same
wiring expressed in the other reading direction:

```mms
light.intensity <- slider.value   // same thing
```

Neither is more "canonical" than the other — authors use whichever reads naturally for the
context. Both desugar to the same handler registration.

### Left side: signal sources

The left of `->` is a **signal source** — something that emits values over time:

| Expression | Signal produced |
|------------|----------------|
| `component.event_name` | Explicit event slot on the component |
| `component` (bare ref) | The component's **primary signal** (type-defined default) |
| `fn(x) { ... } -> next` | Transform stage in a pipeline |

A bare component reference coerces to its primary signal in a `->` context:

```mms
// button's primary signal is Clicked (bool payload, always true)
button -> fn(e) { ... }

// slider's primary signal is ValueChanged (f64 payload)
slider -> fn(v) { light.intensity = v }
```

The primary signal for a component type is declared in the component registry — it is
not inferred. Most interactive components have an obvious one (`RaycastableComponent →
Clicked`, `SliderComponent → ValueChanged`, `ColliderComponent → CollisionStarted`).

### Right side: signal sinks

The right of `->` is a **signal sink** — something that receives values:

| Expression | Effect |
|------------|--------|
| `fn(payload) { }` | Register a handler function |
| `component.property` | Emit the appropriate mutation intent when the source fires |
| `component` (bare ref) | Drive the component's **primary input** (type-defined) |

```mms
slider.value -> light.intensity          // source property → sink property
slider.value -> fn(v) { v * 2.0 } -> light.intensity   // with a transform stage
button -> fn(e) { panel.hidden = true }  // source → explicit handler
button -> panel.hidden                   // source → sink property (toggles on click)
```

### Transform stages in a pipeline

`fn(x) { expr }` in the middle of a `->` chain acts as a map:

```mms
slider.value
    -> fn(v) { v * v }             // square it
    -> fn(v) { clamp(v, 0, 1) }   // clamp
    -> light.intensity
```

This produces a chain of handler registrations. Each stage fires when the previous emits.
The final sink determines whether the chain terminates in a mutation intent or a spawn.

---

## ✦ Events vs intents: abstracted away

From the authoring perspective, the distinction between events and intents is an
implementation detail:

- **Writing a handler** (`on_clicked = fn(e) { ... }`) — the author thinks "when this
  happens, run this code." Whether the engine uses `EventSignal` dispatch internally is
  invisible.
- **Writing a sink** (`slider.value -> light.intensity`) — the author thinks "wire these
  together." The runtime decides whether to route via an event handler + intent, or
  directly via a mutation intent.

The signal operators abstract over the event/intent split. The runtime translates:

```
source event fires
    → payload flows through any transform stages
    → sink receives payload
        → if sink is a fn { }: call it (the fn may emit intents internally)
        → if sink is a component.property: emit the appropriate mutation intent
```

Authors writing `.mms` files should not need to think about `IntentValue` or `EventSignal`
variants. Those are exposed only in low-level handler function bodies when explicit engine
interaction is needed.

### Explicit signal emission inside handlers

Inside a handler function body, `emit()` sends a signal explicitly. The kind (event vs
intent) is determined by what's passed:

```mms
on_clicked = fn(e) {
    emit(SetColor { ids: [self], rgba: [1, 0.3, 0.3, 1] })   // intent
    emit(Clicked { id: self })                                  // event (re-fire)
}
```

`self` inside a handler function refers to the component that owns the handler slot.
`emit()` in handler context always scopes to `self` unless an explicit scope is given.

Whether `emit()` is the right verb here, or whether the handler body should use a
different word (`send`, `fire`, `push`) is still open — see §Open Questions.

---

## ✦ Combining property handlers and signal operators

The two forms are not exclusive. A component can declare some handlers inline and wire
others via `->`:

```mms
let slider = InputSlider {
    range = [0, 1]
    on_value_changed = fn(v) {
        // complex handler: update multiple things
        emit(SetColor { ids: [bg], rgba: [v, v, v, 1] })
    }
}

// simpler direct wiring elsewhere in the scene
slider.value -> light.intensity
slider.value -> fn(v) { v * 5.0 } -> spotlight.falloff
```

The inline `on_value_changed` and the `->` wires all register independent handlers on
`slider`. Multiple handlers for the same event fire in registration order.

---

## ✦ Top-level `on` as explicit form

For handlers that span components not defined together, the top-level `on` form is
still useful:

```mms
let a = R.square() { RC.enabled() }
let b = T.position(2, 0, 0) { R.sphere() {} }

on a.Clicked {
    // b isn't defined inside a — top-level `on` reaches both
    b.hidden = true
}
```

`on <source>.<EventName>(binding) { body }` is syntactic sugar for:

```mms
<source>.on_<event_name> = fn(binding) { body }
```

Both register the same scoped handler. The top-level `on` form is more readable when the
handler body is long or references multiple external components.

---

## ✦ Handler scope and lifetime

All handlers — whether declared as component properties or via `->` / top-level `on` —
are **scoped to the emitting component's lifetime**. When the component is removed
(`RemoveSubtree`), all handlers registered on its scope are removed automatically.

This mirrors the existing `RxWorld` scoped handler model used by gizmos and widgets.

The handler closure captures all `ComponentObject`s visible at registration time. Those
captured objects may outlive the handler's component (the closure holds a `ComponentId`,
not an owned reference), and accessing a removed component in a handler body is a
runtime error (returns `null` or errors, depending on policy).

---

## ✦ `emit()` in component expressions (not just handlers)

A component expression can proactively emit events at spawn time. This is unusual but
occasionally useful — for example, a component that announces itself to a parent listener:

```mms
T {
    MyWidget {
        on_ready = fn() {
            emit(WidgetReady { id: self })   // fires during init
        }
    }
}
```

`on_ready` would be the handler for the component's own initialization event (not yet
a real engine concept, but analogous to `Component::init()`). The MMS `emit()` here
produces an event, not a spawn intent — the distinction is made by value type, as
discussed in `signal-emission-in-mms.md`.

---

## ✦ Sketch: what would a reactive scene file look like?

```mms
// scene: a slider that controls a light

let light = DL {
    intensity(1.0)
    color(1.0, 0.98, 0.95)
}

let slider = T.position(-0.5, 1.2, -1.0) {
    InputSlider {
        range = [0, 3]
        value = 1.0
    }
}

// wire value changes to the light
slider.value -> light.intensity

// also update a label text when the value changes
let label = T.position(-0.5, 0.9, -1.0) {
    TXT { "intensity: 1.0" }
    C.rgba(0, 0, 0, 1)
}

slider.value -> fn(v) {
    label.text = "intensity: " + str(v)   // mutation API
}
```

No `on`, no `intent()`, no `EventSignal` in sight. Just wiring.

---

## Prerequisites

| Requirement | Phase |
|-------------|-------|
| Closures (handler fn captures env) | Phase 4 |
| `ComponentObject` as a live `Value` | Phase 6 (reply channel) |
| `name = fn(e) { }` in component body dispatched to handler slot | New (alongside Phase 6) |
| `->` / `<-` operator tokens and AST | New |
| `self` keyword inside handler bodies | New |
| Mutation API (`component.property = value`) | Phase 7 |
| `emit()` dispatching events vs intents by value type | New |
| Top-level `on` statement | Can be sugar over property form; add later |

---

## Open questions

| Question | Stakes |
|----------|--------|
| Primary signal convention — how is it declared for a component type? | Registry design |
| Does `->` need a new token (`Arrow`) or can it be `Gt` + `Minus`? | Tokenizer ambiguity with `>=` |
| `<-` vs `->` — keep both directions or pick one? | Authoring preference |
| `self` keyword inside handlers — is it always the handler-owning component? | Scope semantics |
| What verb inside handlers: `emit()`, `send()`, `fire()`? | Consistency with authoring vocabulary |
| `on_event_name` naming convention — snake-case with `on_` prefix? Or `Clicked = fn` matching the event name directly? | Readability |
| Can `->` chains span tick boundaries (async)? Or are they always synchronous within one drain? | Engine execution model |
| What happens when a sink property doesn't match the source payload type? Runtime error or coerce? | Type system interaction |
| `once` modifier — `on_clicked = once fn(e) { }` or `once slider.value -> sink`? | One-shot handler ergonomics |
| Should top-level `on` exist at all, or is the property form + `->` sufficient? | Syntax surface area |
