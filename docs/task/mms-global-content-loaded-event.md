# MMS global ContentLoaded event

Date: 2026-05-06

This task proposes an explicit post-bootstrap lifecycle event for live MMS.

It is meant to resolve the current mismatch where top-level code can author component trees and
then immediately attempt queries and handler registration before those emitted trees have been
spawned and initialized.

---

## 1. Problem statement

Today, live MMS evaluates top-level statements in this rough order:

1. parse and transform the script
2. evaluate top-level statements on the MMS side
3. accumulate top-level `emit(...)` results as intents
4. only after evaluation finishes, send those intents to the host/world

That means authored trees from free-standing component expressions are not yet live while later
top-level statements are still running.

This makes patterns like these unreliable or invalid during the initial pass:

- `let btn = query("#btn")`
- `let btn = layout_root.query("#btn")`
- `on(btn, "Click", fn(event) { ... })`

The current failure mode is also too quiet because undefined identifiers and component-expression
values are truthy in `if` conditions.

## 2. Desired authoring model

We want a phase boundary that is explicit and easy to understand.

### Top-level phase

Top-level MMS should remain the place where scripts:

- declare values
- build emitted content
- configure immediate constant state

### Post-content phase

After the initial emitted content has been drained into the live world and initialized, scripts
should have a dedicated hook for:

- querying authored nodes
- wiring handlers
- performing bootstrap work that depends on the live ECS tree

## 3. Proposed API

Add a global event registration form:

```mms
on("ContentLoaded", fn(event) {
    let btn_a = query("#btn_a")
    if btn_a {
        on(btn_a, "Click", fn(event) {
            print("clicked")
        })
    }
})
```

Important difference from the current `on(component, "Click", fn(...))` form:

- there is no first `ComponentObject` scope argument
- the first argument is the global event name string
- this is a system/global lifecycle event, not a scoped component signal

## 4. Event semantics

`ContentLoaded` should mean:

- the initial top-level MMS evaluation pass has completed
- the emit queue produced by that pass has been drained at least once
- the spawned/attached trees from that drain are now in the world
- `init_component_tree` has run for those trees
- world queries can now see that initial content

This event should fire once per script evaluation/bootstrap pass.

For the first implementation, it does not need to imply that every deferred system-side spawn in
future ticks has settled. It only needs to guarantee that the initial MMS-authored content pass is
live and queryable.

## 5. Why a global event is the right shape

This avoids overloading the meaning of top-level statement order.

Without an explicit lifecycle hook, authors naturally assume that code appearing after authored
content in the file runs after that content exists. In the current architecture, that assumption is
false because top-level emits are deferred.

A global `ContentLoaded` hook makes the phase boundary visible in code.

It also lines up with the intended usage pattern:

1. author the scene/UI
2. wait for initial world availability
3. query and wire interactive behavior

## 6. Proposed implementation shape

The exact integration point needs to match the real evaluator host path, but the intended shape is:

1. evaluate the MMS script as usual
2. collect the initial emitted intents
3. drain those intents into the world
4. after that first drain completes, dispatch a global `ContentLoaded` lifecycle event
5. run registered MMS callbacks for that event

Two viable implementation strategies:

### Option A: evaluator-host lifecycle callback registry

Treat `on("ContentLoaded", fn(...))` as registration into a global lifecycle-callback list.

Then the host explicitly invokes those callbacks after the first emit drain.

Pros:

- direct fit for a non-component-scoped event
- keeps lifecycle wiring separate from `RxWorld` scoped handlers
- avoids inventing a fake component scope for a global event

### Option B: add a global/system signal path in RxWorld

Model `ContentLoaded` as a new signal kind that is dispatched globally rather than through a
component subtree scope.

Pros:

- consistent with the broader signal language of the engine
- may generalize to future lifecycle hooks

Cons:

- needs careful design because current `on(component, signal_kind, handler)` is scoped to a
  component subtree
- risks overcomplicating a simple bootstrap event if introduced too early

Current recommendation: start with Option A unless there is already an active plan to add more
global MMS lifecycle signals.

## 7. Parser and evaluator changes

Current built-in `on(...)` only accepts:

```text
on(component_object, "SignalKind", fn(event) { ... })
```

Relevant code:

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L817)

We need an additional accepted form:

```text
on("ContentLoaded", fn(event) { ... })
```

Recommended evaluator behavior:

1. if arg0 is `ComponentObject`, preserve the current scoped-handler path
2. if arg0 is `String` and arg1 is `Function`, treat it as a global lifecycle registration
3. reject unknown global event names with a clear error
4. reject ambiguous mixed forms with a clear error

This should be a clean additive API, not a compatibility alias maze.

## 8. Minimal first milestone

The smallest useful version is:

1. support `on("ContentLoaded", fn(event) { ... })`
2. fire it once after the first emit drain from the initial script evaluation
3. guarantee that `query(...)` can see nodes authored by that initial pass
4. use a minimal event payload, possibly just `null` for the first version

That is already enough to make `examples/query-demo.mms` work in a lifecycle-correct way.

## 9. Open questions

- Which host path is the canonical live MMS bootstrap path in the running engine, not just in
  `MeowMeowRunner::eval_with_world`?
- Should the `event` payload eventually include metadata such as the script path or created roots?
- Should lifecycle callbacks run only once, or once per script reload / hot reload as well?
- If the callback emits more content, should that content be allowed to query the initial tree
  immediately during the callback body?
- Do we also want a later `ContentReady` / `AfterInit` distinction, or is one post-drain event
  enough for now?

## 10. Non-goals for this task

- changing general truthiness rules
- changing undefined identifier fallback behavior
- changing the meaning of `let x = CE` versus `x = CE`
- adding backward-compatibility aliases for multiple lifecycle event names

Those may be related cleanup tasks, but they should not block landing a clear bootstrap hook.

## 11. Related docs

- [docs/bugs/mms-live-query-and-handler-bootstrap-runs-before-emitted-tree-exists.md](../bugs/mms-live-query-and-handler-bootstrap-runs-before-emitted-tree-exists.md)
- [examples/query-demo.mms](../../examples/query-demo.mms)
- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs)
- [src/meow_meow/transform.rs](../../src/meow_meow/transform.rs)
- [src/meow_meow/runner.rs](../../src/meow_meow/runner.rs)
