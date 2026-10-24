# MMS live component handles, deferred component expressions, and diagnostics

> **Dependency / caution:** This task must not be used to paper over a broken
> live-binding invariant. See
> [MMS component-reference provenance](mms-component-reference-provenance.md)
> first. A top-level live `let` that becomes a `ComponentExpr` in a callback is
> a bug to fix, not a condition authors should normally branch around.

## Problem

An eye-tracking visualizer exposed an ambiguous MMS value-lifecycle failure.
The scene bound a transform tree to a variable, later attached that tree under
another authored transform, and then used the variable from an event handler:

```mms
let marker = T.position(-0.72, 1.0, -1.3) { R.icosahedron() { ... } }

T { marker }

on(eyes, "XrEyeTrackingUpdated", fn(event) {
    marker.update_transform(...)
})
```

When ALVR emitted an eye event, MMS reported:

```text
method call 'update_transform': receiver is not a ComponentObject,
got ComponentExpr(MaterializedCE { component_type: "T", ... })
```

The scene could render the attached marker, but the captured variable was a
deferred `ComponentExpr` rather than a live `ComponentObject` handle. That
makes visual authoring look valid until the first reactive update arrives.

## Why this matters

- A component expression and a live component handle have materially different
  capabilities, but MMS currently exposes the difference only through an
  implementation-heavy runtime error.
- `let` plus later scene attachment reads naturally as “keep a handle for this
  scene object”, especially when the visual subtree does render.
- In a `load` invocation, initial MMS evaluation errors trigger the legacy demo
  scene fallback. Runtime handler errors do not. The console therefore needs to
  make those two failure modes unmistakably different.

## Desired outcome

Authors can identify a value's kind before trying a live method, and can turn a
component expression into an explicit, stable live handle using documented MMS
syntax. The error should recommend the relevant remedy rather than dumping the
full materialized component tree. This is chiefly valuable for intentionally
deferred expressions; it is not an alternative to preserving normal live
component handles across scene attachment and callback capture.

## Investigation / design tasks

1. Resolve the provenance/invariant tracker before exposing a language-level
   guard. Otherwise `type_of` or `match` would merely let users hide an engine
   defect with a fallback branch.
2. Document the lifecycle and ownership rules for:
   - `ComponentExpr` / deferred component trees;
   - authored scene attachment;
   - `ComponentObject` / live handles;
   - values captured by `on(...)` callbacks.
3. Follow the [runtime type-tag direction](../../crates/meow-meow-script/docs/draft/type-system-epic.md#runtime-type-tag-direction-2026-10-14)
   confirmed 2026-10-14 and revised 2026-10-23: proposed `typeof(value)` returns a first-class,
   immutable type-tag value. Tags are referred to through explicit imports
   from `"types"` in V1, with lowercase exports such as `str` and `f32` and normal
   import aliases. Compare tag identities, not strings or Rust debug output.
   No automatic type-tag/`Types` globals or package system are required;
   qualified `Types.str` module access is a follow-up. Freeze the component
   lifecycle/kind distinction and its tag or predicate spellings before
   implementing it; both deferred and live values must remain diagnosable.
   Concrete component type identity and deferred/live state are different
   questions. Keep the planned `node.type()` name-for-reflection contract
   separate from general tag inspection unless explicitly migrated.
4. Evaluate pattern matching as a more ergonomic alternative or complement:

   ```mms
   match marker {
       Component(handle) => handle.update_transform(...)
       ComponentExpr(_) => print("marker was not materialized")
   }
   ```

   Exact syntax is open; the essential requirement is safe branching on value
   kind without relying on failed method calls.
5. Decide whether a `materialize` / `spawn` / `attach` operation should return
   the live handle that it creates. If it already does internally, expose it to
   MMS consistently.
6. Improve the live-method error to say the receiver was a deferred component
   expression and name the supported materialization/attachment path.
7. Separate CLI messages for:
   - initial scene-load errors followed by demo-scene fallback; and
   - later handler errors in an otherwise successfully loaded scene.

## Acceptance tests

- A fixture can distinguish a deferred transform expression from its live,
  attached counterpart with the chosen introspection or match feature.
- Imported tag constants and aliases retain identity; `typeof(value)` returns
  comparable tags without allocating type-name strings. Unimported tag names
  produce actionable diagnostics rather than silently comparing identifiers.
- A callback can mutate the intended live transform after an explicit,
  documented materialization step.
- Calling a live method on a `ComponentExpr` produces a concise actionable
  error; it does not print the full `MaterializedCE` debug tree by default.
- A failing initial `load` clearly states that the demo fallback was selected;
  a handler failure clearly states that the loaded scene remains active.

## Context

This was discovered while implementing `examples/eye-tracking-example.mms`.
The OpenXR/ALVR transport itself continued working; the issue is MMS value
materialization and diagnostics, not eye tracking.

`is_string(value)` is implemented and remains appropriate for the current
string/component title factory. Primitive type-tag imports, values and
host/callback transport are implemented as of 2026-10-23. General `typeof`,
component/lifecycle inspection, annotations and pattern matching remain planned;
the tag-value slice does not implement those later features.
