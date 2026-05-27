# MMS live query and handler bootstrap runs before emitted tree exists

## Status

Open bug / investigation note.

No source changes made yet.

## Symptom

In live MMS, a script can author a component tree and then immediately try to:

- query nodes from that tree at top level
- register handlers against nodes from that tree at top level
- branch on the presence of those queried values

Example shape:

```mms
let layout_root = null

T.position(-2.4, 1.8, 0.0).scale(0.06, 0.06, 0.06) {
    layout_root = LayoutRoot {
        T { name = "btn_a" }
    }

    layout_root
}

if layout_root {
    let btn_a = layout_root.query("#btn_a")
    if btn_a {
        on(btn_a, "Click", fn(event) {
            print("clicked")
        })
    }
}
```

The author expectation is usually:

1. the `LayoutRoot` exists by the time the bottom block runs
2. `layout_root.query("#btn_a")` can see the just-authored subtree
3. `on(btn_a, ...)` can attach a handler during the same bootstrap pass

That is not the current lifecycle.

## Repro

- [examples/query-demo.mms](../../examples/query-demo.mms)

Relevant authored pattern:

```mms
let layout_root = null

T.position(-2.4, 1.8, 0.0).scale(0.06, 0.06, 0.06) {
    layout_root = LayoutRoot {
        ...
    }

    layout_root
}

if (layout_root) {
    print("layout_root exists")

    if btn_a {
        print("btn_a")
        print(btn_a)
    }
}
```

## Expected behavior

If MMS allows top-level post-authoring bootstrap code, the semantics need to be clear and useful.

For the example above, an author would reasonably expect one of these models:

### Model A: immediate live availability

Top-level code after the authored tree runs only after the emitted tree has been spawned,
attached, initialized, and made queryable.

Then this is valid:

```mms
let btn_a = layout_root.query("#btn_a")
on(btn_a, "Click", fn(event) { ... })
```

### Model B: explicit post-load phase

Top-level authoring runs first, emitted trees are drained into the world, and a later
explicit lifecycle callback is used for queries and handler registration.

Then this is valid:

```mms
on("ContentLoaded", fn(event) {
    let btn_a = query("#btn_a")
    if btn_a {
        on(btn_a, "Click", fn(event) { ... })
    }
})
```

What should not happen is a silent mismatch where the script appears to be inspecting live
objects even though the emitted tree does not exist yet.

## Actual behavior

Current behavior is a mix of three separate facts:

1. top-level emitted component expressions are not spawned immediately
2. the remainder of the script continues evaluating before those emits are drained
3. undefined bare identifiers are treated as symbolic `Identifier` values, which are truthy

So in the repro:

- `if layout_root` can be true even though `layout_root` is only a `ComponentExpr`, not a live component
- `if btn_a` can also be true when `btn_a` was never bound at all
- any top-level query or handler-registration attempt that expects the emitted tree to already exist is running too early

## Root cause

### 1. Bare component statements are lifted into deferred emits

Free-standing component expressions are rewritten into `emit(ce)` calls.

Relevant code:

- [src/meow_meow/transform.rs](../../src/meow_meow/transform.rs#L10)

### 2. Top-level emits are flushed only after the whole script finishes evaluating

The evaluator accumulates emitted `SpawnComponentTree` intents while walking statements.
Those intents are only sent to the host after `eval_block_stmts` completes.

Relevant code:

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L303)
- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L334)

This means later top-level statements run before the emitted tree exists in the live ECS world.

### 3. Reassigning `layout_root = LayoutRoot { ... }` does not produce a live ComponentObject

The special live-world path only exists for `Statement::Assignment` (`let x = CE`).
The repro uses reassignment into a previously-declared variable:

```mms
let layout_root = null
layout_root = LayoutRoot { ... }
```

That path does not call `Register`; it simply stores a `Value::ComponentExpr`.

Relevant code:

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L389)
- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L411)

### 4. Unbound identifiers fall back to `Value::Identifier`, not an error

Expression identifier lookup currently does:

- resolve from `ObjectWorld` if bound
- otherwise produce `Value::Identifier(name)`

Relevant code:

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L711)

So `btn_a` in expression position is not rejected as undefined.

### 5. Truthiness treats every non-null, non-false value as true

`is_truthy` currently returns false only for:

- `Value::Null`
- `Value::Bool(false)`

Everything else is truthy, including:

- `Value::ComponentExpr(...)`
- `Value::Identifier("btn_a")`

Relevant code:

- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs#L1191)

## Why this matters

This creates a misleading bootstrap model for live MMS UI scripts.

Authors can write code that looks like a normal DOM-style load sequence:

1. author UI
2. query authored nodes
3. register handlers

But the current runtime does not support that ordering, and the fallback truthiness behavior can
mask the mistake instead of surfacing it early.

That makes query debugging harder than it should be and encourages scripts that appear valid but
are observing the wrong phase of the lifecycle.

## Proposed direction

The clean direction is to make the lifecycle explicit rather than trying to pretend authored
content is synchronously available during the same top-level evaluation pass.

Recommended shape:

1. keep top-level authoring as an emit-building phase
2. drain the initial emit queue into the world
3. fire a global system event once that first content pass is live
4. do queries and handler registration from that later phase

That follow-up design is captured in:

- [docs/task/mms-global-content-loaded-event.md](../task/mms-global-content-loaded-event.md)

## Related files

- [examples/query-demo.mms](../../examples/query-demo.mms)
- [src/meow_meow/transform.rs](../../src/meow_meow/transform.rs)
- [src/meow_meow/evaluator.rs](../../src/meow_meow/evaluator.rs)
- [src/meow_meow/runner.rs](../../src/meow_meow/runner.rs)
- [src/meow_meow/component_registry.rs](../../src/meow_meow/component_registry.rs)
