# Meow Meow Script (MMS) ᓚᘏᗢ

MMS is the scripting + scene-authoring language for cat-engine. A script
describes a component tree and wires up reactive behaviour; the evaluator
runs on a worker thread and emits intents to the engine.

For status, roadmap, and active task docs see [`task/status.md`](task/status.md).
For language goals see [`objectives.md`](objectives.md).

---

## Component expressions

A component expression (CE) is the core construct: a constructor head and an
optional body of children. Children are themselves CEs. A bare CE in
statement position emits — i.e. spawns and attaches.

```mms
T.position(0, 1, 0) {
    R.cube() {
        C.rgba(1, 0, 0, 1)
    }
}
```

`T` is a transform, `R.cube()` a renderable, `C.rgba(...)` a color. The body
contains both child CEs and pre-body builder calls (`name = "..."`,
constructor-style methods).

Bind a CE to a name with `let` to attach later, or to query/mutate it:

```mms
let panel = T.position(0, 0, -2) {
    R.cube() {}
}
panel    // bare reference in statement position → emits
```

See [`spec/component-expression-format.md`](spec/component-expression-format.md)
and [`assets/components/`](../../assets/components) for component definitions.

---

## Variables

`let` binds in the current frame. `=` reassigns an existing binding (walks
out through transparent block frames; stops at function boundaries).

```mms
let x = 5
let y = x + 1
x = x + y
```

Scope is a frame stack: blocks, loop bodies, if-bodies, and CE bodies push
transparent frames; function calls push a hard barrier. Closures capture the
visible env at definition time. See
[`spec/env-heap-object-world.md`](spec/env-heap-object-world.md).

---

## Conditionals

```mms
if hp <= 0 {
    R.cube() { C.rgba(1, 0, 0, 1) }
} else {
    R.cube() { C.rgba(0, 1, 0, 1) }
}
```

---

## Loops

`for ... in` over arrays or `range(n)`; `while` for predicate loops;
`break` / `continue` inside either. Loop bodies are transparent — accumulator
reassignments propagate after the loop.

```mms
let sum = 0
for i in range(10) {
    if i == 5 { continue }
    sum = sum + i
}
```

Examples: [`mms-loops.mms`](../../examples/mms-loops.mms),
[`mms-functions.mms`](../../examples/mms-functions.mms).

---

## Querying

Name a component with `name = "..."` inside its body, then look it up.
`query` returns one result, `query_all` returns all. The `->` operator
chains a selector to a handler or method.

```mms
T { name = "hero"; R.cube() {} }

let hero = query("#hero")
hero.set_color(0, 0, 1, 1)

query_all("enemy") -> set_color(0, 1, 0, 1)
```

Example: [`query-demo.mms`](../../examples/query-demo.mms).

---

## Signals

Components emit events; MMS registers handlers with the `on(target, event, fn)`
builtin. The fn is called with an `event` value when the target fires.

```mms
let cube = T.position(0, 0, 0) {
    R.cube() {
        C.rgba(0.25, 0.55, 1.0, 1.0)
        Raycastable.enabled()
    }
}

on(cube, "Click", fn(event) {
    print("clicked")
})
```

> TODO: in-body handler sugar (`on(Click) { ... }` inside a CE body) and the
> `selector -> handler` form are designed but not yet implemented. See
> [`analysis/event-handlers.md`](analysis/event-handlers.md).

Examples: [`signal-handler.mms`](../../examples/signal-handler.mms),
[`pipe-demo.mms`](../../examples/pipe-demo.mms).

---

## More examples

`examples/` contains end-to-end scripts: scene setup
([`vr-input.mms`](../../examples/vr-input.mms)), UI
([`ui-layout.mms`](../../examples/ui-layout.mms),
[`html-layout.mms`](../../examples/html-layout.mms)), composition
([`component-method-call.mms`](../../examples/component-method-call.mms),
[`mms-module-example.mms`](../../examples/mms-module-example.mms)),
and more.
