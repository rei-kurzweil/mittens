# Task: top-level MMS component method dispatch

## Problem

Top-level MMS scripts and runtime callbacks do not execute component-object
methods under the same runtime model.

Today there are two distinct paths:

- Top-level MMS script evaluation runs inside the MeowMeow evaluator worker.
  In `eval_script(...)` the evaluator context is created with
  `host_world: None` and communicates with the engine through channels /
  emitted intents only.
- Runtime closures such as keyframe callbacks and signal handlers run later on
  the host side through `eval_runtime_closure(...)` / `eval_mms_fn(...)` and
  receive `host_world: Some(world)`.

That split is now causing real authoring failures:

- imported component factories can return a live `ComponentObject` whose outer
  node is a `TransformComponent`
- the author naturally expects `let x = some_factory(); x.update_transform(...)`
  at top level to work the same way it does inside a callback
- instead it fails unless the evaluator has a bespoke fallback path for that
  method

Recent examples:

- `examples/pride.mms` imports `rainbow_animated()` from
  `assets/components/animated.mms`, gets back a `T`, and then calls
  `rainbow_2.update_transform(...)`
- `set_intensity(...)` and `update_transform(...)` have already forced
  evaluator-specific treatment to make top-level usage work

This does not scale. If we keep the current design, every new component method
will need one of:

- a new evaluator special case
- a new intent-lowering path embedded directly in the evaluator
- or an inconsistent rule where the method works in callbacks but not at
  top level

## Goal

Make top-level imperative MMS code behave like runtime callbacks for
component-object method calls.

The key requirement is:

- `foo.method(...)` on a `Value::ComponentObject` should have one canonical
  engine-side implementation, regardless of whether it is called:
  - during top-level script execution
  - inside a signal handler
  - inside a keyframe callback
  - inside an imported factory function body

## Proposed direction

Do not keep growing `src/meow_meow/evaluator.rs`.

Instead, introduce a host-side component method dispatch layer, with a new file
such as:

- `src/meow_meow/component_method_registry.rs`

This registry should own live-world component method semantics for
`ComponentObject` calls.

### High-level shape

When the evaluator sees:

```mms
some_component_object.some_method(arg1, arg2)
```

and the callee is a live component object rather than a CE-builder call:

- top-level worker eval should send a host call like:

```rust
HostCallKind::InvokeComponentMethod {
    id,
    component_type,
    method,
    args,
}
```

- host-side code should route that into `component_method_registry.rs`
- the registry should execute the canonical behavior against the real world

Runtime closures can either:

- continue to call the same method logic directly through the registry with a
  live `World`
- or use the same host-dispatch shape if we want one path everywhere

Either way, the semantic implementation should live in one place.

## Why a registry file

`evaluator.rs` is already carrying too much responsibility:

- parsing-time expression evaluation
- CE materialization
- built-in functions
- query plumbing
- some live-world component behavior

Adding more per-component method logic there will make it harder to:

- reason about top-level vs runtime behavior
- test component methods independently
- keep canonical mutation paths aligned with `SystemWorld` / mutation executor

A dedicated `component_method_registry.rs` gives us:

- one place to define which component methods are live runtime methods
- one place to convert MMS args into engine calls
- one place to route into canonical system methods like:
  - `systems.update_transform(...)`
  - `systems.update_emissive_intensity(...)`
  - text update paths
  - future opacity / color transition-aware paths

## Initial scope

Phase 1 should cover the methods that already exposed the problem:

- `TransformComponent.update_transform(...)`
- `EmissiveComponent.set_intensity(...)`
- `EmissiveComponent.on()`
- `EmissiveComponent.off()`

These should all route into existing canonical engine mutation paths rather
than performing ad hoc world mutation in the evaluator.

## Expected behavior after refactor

These should all work the same way:

```mms
let widget = some_factory_returning_transform()
widget.update_transform([0, 1, 2], [0, 0, 0], [1, 1, 1])
```

```mms
on(button, "Click", fn(e) {
    widget.update_transform([0, 1, 2], [0, 0, 0], [1, 1, 1])
})
```

```mms
Animation.looping() {
    Keyframe.at(0.0) {
        glow.set_intensity(2.5)
    }
}
```

Top-level scripts should no longer require a special evaluator exception for
every component method that needs live-world semantics.

## Implementation notes

### 1. Add a host-call surface for live component methods

Extend the evaluator/runner host-call protocol with an
`InvokeComponentMethod` variant that carries:

- `ComponentId`
- component type string
- method name
- evaluated argument list

This is for live `ComponentObject` methods only, not CE-builder methods.

### 2. Add `src/meow_meow/component_method_registry.rs`

This module should:

- validate argument shapes
- validate the target component type against the real world when needed
- invoke canonical engine-side behavior
- return either:
  - immediate success / failure
  - optional value payload if we later support method return values

Keep the first version narrow and explicit; the goal is semantic unification,
not a generic reflection system.

### 3. Reduce evaluator-owned live mutation logic

Move live-world component method semantics out of `evaluator.rs` as methods are
ported to the registry.

The evaluator should remain responsible for:

- parsing and expression evaluation
- CE-builder behavior
- built-in functions
- deciding whether a call is:
  - a CE-builder call
  - a pure local function call
  - a live component-object method call that must go through the registry

### 4. Keep canonical system paths authoritative

The registry should not duplicate mutation logic that already exists elsewhere.

For example:

- transform updates should still go through `SystemWorld::update_transform`
- emissive updates should still go through
  `SystemWorld::update_emissive_intensity`

That keeps transitions, propagation, and future side effects consistent.

## Non-goals

- not a full reflection / scripting API redesign
- not a requirement that every MMS expression execute on the host thread
- not a requirement that CE materialization stop using the worker
- not a migration of every component method in one pass

## Success criteria

- imported factory functions can return a `TransformComponent` root and the
  caller can invoke `update_transform(...)` on that object at top level
- top-level and callback-time `set_intensity(...)` share the same semantic
  implementation path
- new component methods no longer require bespoke evaluator exceptions just to
  work at top level
- `src/meow_meow/evaluator.rs` shrinks in responsibility rather than growing

## Suggested follow-up tests

- MMS integration test:
  - import a factory that returns `T`
  - bind it to `let x = ...`
  - call `x.update_transform(...)` at top level
  - verify the transform changes through the canonical system path
- MMS integration test:
  - same object, same method, but called from a keyframe callback
  - verify identical runtime behavior
- MMS integration test:
  - top-level `glow.set_intensity(...)` on an attached emissive with child
    `Transition`
  - verify the transition runtime starts rather than jumping directly
