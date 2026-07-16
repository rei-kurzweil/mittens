# Meow Meow Script (MMS) ᓚᘏᗢ

MMS is a host-neutral language in the `meow-meow-script` crate, with scene
authoring capabilities supplied by `mittens-engine`. A script
describes a component tree and wires up reactive behaviour; the evaluator
runs through the host contract and emits requests to the engine adapter.

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

### Component Lifecycle

There are two distinct runtime modes for component expressions:

- `ComponentExpr`: a declarative component tree value that has not been given a
  live ECS id yet.
- `ComponentObject`: a live ECS component handle with a real `ComponentId`.

In live-world evaluation (`MeowMeowRunner::eval_with_world...`), `let x = SomeComponent...`
eagerly registers the component subtree immediately. That means `x` becomes a
`ComponentObject` right away, even before it is attached or initialized. MMS can
then safely do runtime receiver calls like:

```mms
let glow = Emissive.off()
glow.set_intensity(0.2)   // valid: live detached component

T {
    R.cube() { glow }     // attach later
}

glow.set_intensity(2.5)   // still valid after attach
```

The lifecycle in that mode is:

1. author CE syntax
2. eager `Register` on `let` / reassignment
3. get a live detached `ComponentObject`
4. attach it into another CE or as a root later
5. initialization runs on attach/root-emit

If the script also needs procedural `Renderable` constructors during live-world
evaluation, use the runner entry points that also provide `RenderAssets`
(`eval_with_world_and_assets...`), otherwise procedural `R.partial_annulus_2d(...)`,
`R.star(...)`, etc. cannot materialize during host `Register` / `Spawn`.

In fire-and-forget evaluation (`MeowMeowRunner::eval`, `eval_file`, etc.), there
is no live world during evaluation, so MMS cannot allocate real `ComponentId`s.
In that mode `let x = SomeComponent...` remains a `ComponentExpr`, and runtime
receiver calls like `x.set_intensity(...)` are not valid. Only CE builder syntax
such as `Emissive.on()`, `Renderable.star(...)`, or body calls like
`Emissive.on() { intensity(0.2) }` are valid there.

One more distinction: `Keyframe.at(...) { ... }` bodies are timed imperative
blocks. In live-world evaluation, they capture the surrounding lexical env and
run when the keyframe becomes due, so direct receiver calls like
`glow.set_intensity(2.5)` or `cube_t.update_transform([...], [...], [...])`
dispatch intents at that due frame.

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
